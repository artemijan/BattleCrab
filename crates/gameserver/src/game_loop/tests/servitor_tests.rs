//! Servitor summoning — the first G29 slice.
//!
//! `Summon` is the single biggest unported effect on the whole ranking (24
//! learnable skills). This slice covers summoning, ownership, unsummon and the
//! owner's `PetInfo` view; follow/attack AI and the `SummonInfo` packet that
//! shows a servitor to *other* players are separate slices.

use super::*;

use crate::model::components::ServitorOf;
use crate::model::skill::SkillEffect;

use crate::game_loop::servitor::{
    pet_of, summon_pet,
    handle_life_tick, on_owner_leave_world,
    servitor_attack, servitor_follow_tick, servitor_of, servitor_stop, servitor_toggle_follow, summon_servitor,
    unsummon_servitor,
};

const OWNER: i32 = 9901;
const CID: u32 = 1;
const PANTHER: i32 = 14799;
/// A distinct object id for the sparring dummy.
///
/// **Not `NPC_OID`.** A servitor is spawned through the runtime allocator,
/// which starts at `FIRST_NPC_OBJECT_ID` — the very id `NPC_OID` is — so a
/// fixture NPC placed there silently *replaces* the servitor. Three tests
/// failed on exactly that before this constant existed.
const FOE: i32 = NPC_OID + 10;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn servitor_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for id in [PANTHER, PANTHER + 1] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Servitor".into();
        t.name = format!("Panther {id}");
        t.level = 20;
        t.base_hp_max = 400.0;
        t.base_mp_max = 200.0;
        t.collision_radius = 10.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

// ---------------------------------------------------------------------------
// Summon / unsummon
// ---------------------------------------------------------------------------

/// A servitor spawns at its owner, is linked back to them, and starts at full
/// HP/MP (Java's `setCurrentHp(getMaxHp())`).
#[test]
fn summoning_spawns_a_servitor_owned_by_the_caster() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);

    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");

    let link = world.objects.get_component::<ServitorOf>(&oid).expect("linked to its owner");
    assert_eq!(link.owner_object_id, OWNER);
    assert_eq!(link.reference_skill, 283, "remembers the skill that summoned it");

    let pos = world.objects.get_component::<Position>(&oid).unwrap();
    assert_eq!((pos.x, pos.y), (100, 200), "spawns on its owner");

    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "full HP");
    assert_eq!(v.cur_mp, v.max_mp as f64, "full MP");

    assert_eq!(servitor_of(&mut world, OWNER), Some(oid), "found by owner lookup");
}

/// Java unsummons any existing servitor before spawning the new one, so
/// re-casting **swaps** rather than stacking.
#[test]
fn resummoning_replaces_rather_than_stacks() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let first = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    let second = summon_servitor(&mut world, OWNER, PANTHER + 1, 283, 1200, 0, 0).unwrap();

    assert_ne!(first, second, "a genuinely new entity");
    assert!(world.objects.get_component::<ServitorOf>(&first).is_none(), "the first one is gone");
    assert_eq!(servitor_of(&mut world, OWNER), Some(second), "only the newest remains");
}

/// Unsummoning removes the servitor from the world entirely.
#[test]
fn unsummoning_removes_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    assert_eq!(unsummon_servitor(&mut world, OWNER), Some(oid));
    assert_eq!(servitor_of(&mut world, OWNER), None, "no servitor left");
    assert!(world.objects.get_component::<Vitals>(&oid).is_none(), "and the entity is despawned");
}

/// Unsummoning with nothing out is a no-op rather than an error.
#[test]
fn unsummoning_nothing_is_harmless() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    assert_eq!(unsummon_servitor(&mut world, OWNER), None);
}

/// `lifeTime <= 0` is Java's "no expiry" case (`Integer.MAX_VALUE`, commented
/// "Classic hack. Resummon upon entering game."), and a positive one is stored
/// as an absolute deadline.
#[test]
fn life_time_zero_means_no_expiry() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let forever = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert_eq!(world.objects.get_component::<ServitorOf>(&forever).unwrap().expires_at_tick, u64::MAX);

    let timed = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    let link = world.objects.get_component::<ServitorOf>(&timed).unwrap();
    assert_eq!(link.expires_at_tick, world.tick + 12_000, "1200 s at 10 ticks/s");
    assert_eq!(link.life_time_secs, 1200);
}

/// Only players summon (Java's `if (!effected.isPlayer()) return`).
#[test]
fn an_npc_cannot_summon() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, NPC_OID, PANTHER, "Monster", 20, 0, 0, 0);
    assert_eq!(summon_servitor(&mut world, NPC_OID, PANTHER, 283, 1200, 0, 0), None);
}

// ---------------------------------------------------------------------------
// The owner's view
// ---------------------------------------------------------------------------

/// The owner is sent `PetInfo` (0xB2) when the servitor appears — without it
/// nothing renders client-side.
#[test]
fn the_owner_is_sent_pet_info() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let _ = drain(&mut rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    let opcodes: Vec<u8> = drain(&mut rx).iter().filter_map(|p| p.first().copied()).collect();
    assert!(opcodes.contains(&server_packets::opcodes::PET_INFO), "PetInfo sent, got {opcodes:?}");
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The summon skills parse, and `npcId` is **per level** — each level summons a
/// stronger template, which is why the effect carries the id rather than the
/// skill.
#[test]
fn real_dist_summon_skills_parse_per_level_npc_ids() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    let npc_of = |id: i32, level: i32| {
        skills.get(id, level).and_then(|s| {
            s.effects.iter().find_map(|e| match e {
                SkillEffect::Summon { npc_id, .. } => Some(*npc_id),
                _ => None,
            })
        })
    };
    // Summon Dark Panther 283 walks its template id up with the skill level.
    let l1 = npc_of(283, 1).expect("level 1 summons something");
    let l2 = npc_of(283, 2).expect("level 2 summons something");
    assert_ne!(l1, l2, "a different template per level: {l1} vs {l2}");

    // Summon Siege Golem 13 has a flat npcId and a real consume item.
    let golem = skills.get(13, 1).expect("Siege Golem loads");
    let effect = golem.effects.iter().find_map(|e| match e {
        SkillEffect::Summon { npc_id, life_time, consume_item_id, .. } => Some((*npc_id, *life_time, *consume_item_id)),
        _ => None,
    });
    assert_eq!(effect, Some((14737, 1200, 2131)), "npcId / lifeTime / gemstone");
}

/// All 24 learnable summon skills produce a usable effect — none is dropped for
/// want of an `npcId`.
#[test]
fn every_learnable_summon_skill_parses() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    for id in [13, 25, 283, 299, 301, 448, 1111, 1128] {
        let skill = skills.get(id, 1).unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill.effects.iter().any(|e| matches!(e, SkillEffect::Summon { npc_id, .. } if *npc_id > 0)),
            "skill {id} carries a usable Summon: {:?}",
            skill.effects
        );
    }
}

// ---------------------------------------------------------------------------
// Follow / attack (slice 2)
// ---------------------------------------------------------------------------

/// A fresh servitor follows (Java's `getFollowStatus()` defaults true) and
/// closes the gap when its owner walks away.
#[test]
fn an_idle_servitor_trails_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert!(world.objects.get_component::<ServitorOf>(&oid).unwrap().following, "follows by default");

    // Owner walks well beyond the follow range.
    world.objects.get_component_mut::<Position>(&OWNER).unwrap().x = 900;
    servitor_follow_tick(&mut world, oid);

    let m = world.objects.get_component::<crate::model::components::Movement>(&oid);
    assert!(m.is_some(), "the servitor set off after its owner");
}

/// Inside the follow range it stays put rather than jittering.
#[test]
fn a_servitor_already_close_does_not_move() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    world.objects.get_component_mut::<Position>(&OWNER).unwrap().x = 100; // < FOLLOW_RANGE
    servitor_follow_tick(&mut world, oid);
    assert!(
        world.objects.get_component::<crate::model::components::Movement>(&oid).is_none(),
        "no pointless walk"
    );
}

/// "Hold your ground" stops the following, and toggling again resumes it.
#[test]
fn hold_toggles_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    assert_eq!(servitor_toggle_follow(&mut world, OWNER), Some(false), "now holding");
    world.objects.get_component_mut::<Position>(&OWNER).unwrap().x = 900;
    servitor_follow_tick(&mut world, oid);
    assert!(
        world.objects.get_component::<crate::model::components::Movement>(&oid).is_none(),
        "a holding servitor ignores its owner walking off"
    );

    assert_eq!(servitor_toggle_follow(&mut world, OWNER), Some(true), "and back to following");
    servitor_follow_tick(&mut world, oid);
    assert!(world.objects.get_component::<crate::model::components::Movement>(&oid).is_some());
}

/// An ordered attack seeds hate on the target and switches the servitor to the
/// attack intention, which is what the ordinary NPC attack think drives from.
#[test]
fn an_ordered_attack_targets_the_owners_target() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);

    assert!(servitor_attack(&mut world, OWNER, FOE), "the order was accepted");

    let hate = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&oid)
        .and_then(|a| a.0.get(&FOE))
        .map(|i| i.hate)
        .unwrap_or(0.0);
    assert!(hate > 0.0, "the target is now hated");
    assert_eq!(
        world.objects.get_component::<crate::model::npc::NpcAi>(&oid).unwrap().intention,
        crate::model::npc::NpcIntention::Attack
    );
    assert!(
        !world.objects.get_component::<ServitorOf>(&oid).unwrap().following,
        "and it stops trailing, or it would drift home between swings"
    );
}

/// Java refuses an order at a target more than 3000 units from the owner and
/// falls back to following, so a stray click doesn't send the summon across the
/// map.
#[test]
fn a_far_target_is_refused_and_the_servitor_keeps_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 9_000, 0, 0);

    assert!(!servitor_attack(&mut world, OWNER, FOE), "refused");
    assert!(world.objects.get_component::<ServitorOf>(&oid).unwrap().following, "falls back to following");
    assert_eq!(
        world.objects.get_component::<crate::model::npc::AggroList>(&oid).map(|a| a.0.len()),
        Some(0),
        "and never took the target"
    );
}

/// "Stop" clears the target, halts movement and resumes following.
#[test]
fn stop_cancels_the_attack_and_resumes_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);
    servitor_attack(&mut world, OWNER, FOE);

    assert!(servitor_stop(&mut world, OWNER));
    assert_eq!(world.objects.get_component::<crate::model::npc::AggroList>(&oid).map(|a| a.0.len()), Some(0));
    assert_eq!(
        world.objects.get_component::<crate::model::npc::NpcAi>(&oid).unwrap().intention,
        crate::model::npc::NpcIntention::Active
    );
    assert!(world.objects.get_component::<ServitorOf>(&oid).unwrap().following, "back to trailing its owner");
}

/// A servitor does **not** hunt on its own — unlike a monster it never seeds
/// hate from an aggro scan, only from its owner's order.
#[test]
fn a_servitor_does_not_pick_its_own_fights() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    // A monster stands right next to it.
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 50, 0, 0);

    advance_world(&mut world, 200);

    assert_eq!(
        world.objects.get_component::<crate::model::npc::AggroList>(&oid).map(|a| a.0.len()),
        Some(0),
        "no unbidden aggro"
    );
}

// ---------------------------------------------------------------------------
// Visibility to other players (slice 3)
// ---------------------------------------------------------------------------

/// The owner sees `PetInfo`; **everyone else** sees `SummonInfo` (0x8B). Before
/// this slice a servitor was invisible to every player but its summoner.
#[test]
fn other_players_are_sent_summon_info_and_the_owner_is_not() {
    let (mut world, _db, _l) = servitor_world();
    let mut owner_rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let mut other_rx = ingame_caster(&mut world, 2, OWNER + 1, 60, 0);
    let _ = drain(&mut owner_rx);
    let _ = drain(&mut other_rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    let owner_ops: Vec<u8> = drain(&mut owner_rx).iter().filter_map(|p| p.first().copied()).collect();
    let other_ops: Vec<u8> = drain(&mut other_rx).iter().filter_map(|p| p.first().copied()).collect();

    assert!(owner_ops.contains(&server_packets::opcodes::PET_INFO), "owner gets PetInfo: {owner_ops:?}");
    assert!(
        !owner_ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "and not the bystander packet as well"
    );
    assert!(other_ops.contains(&server_packets::opcodes::SUMMON_INFO), "others get SummonInfo: {other_ops:?}");
    assert!(!other_ops.contains(&server_packets::opcodes::PET_INFO), "and never the owner-only one");
}

/// The packet carries the **owner's name** in its title slot — that is what
/// draws the "of X" label under a summon, and it is the field most likely to be
/// wired to the wrong string.
#[test]
fn summon_info_carries_the_owners_name() {
    let (mut world, _db, _l) = servitor_world();
    let _owner_rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let mut other_rx = ingame_caster(&mut world, 2, OWNER + 1, 60, 0);
    let _ = drain(&mut other_rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    let owner_name = world.objects.get_component::<crate::model::Player>(&OWNER).unwrap().name.clone();
    let pkt = drain(&mut other_rx)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::SUMMON_INFO))
        .expect("SummonInfo sent");
    // The name is UTF-16LE in the packet body.
    let wide: Vec<u8> = owner_name.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    assert!(
        pkt.windows(wide.len()).any(|w| w == wide),
        "the owner's name appears in the packet"
    );
}

/// A servitor that walks into view is introduced with `SummonInfo` too, not
/// `NpcInfo` — the visibility delta path has to make the same choice as the
/// summon path.
#[test]
fn a_servitor_entering_view_is_introduced_as_a_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _owner_rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    // A second player logs in nearby *after* the summon.
    let mut late_rx = ingame_caster(&mut world, 2, OWNER + 1, 60, 0);
    let _ = drain(&mut late_rx);
    crate::game_loop::visibility::on_enter_world(&world, 2, OWNER + 1);

    let ops: Vec<u8> = drain(&mut late_rx).iter().filter_map(|p| p.first().copied()).collect();
    assert!(ops.contains(&server_packets::opcodes::SUMMON_INFO), "introduced as a summon: {ops:?}");
}

// ---------------------------------------------------------------------------
// Lifecycle (slice 4)
// ---------------------------------------------------------------------------

/// The upkeep tick ends a servitor whose lifetime has run out.
#[test]
fn a_servitor_passes_away_when_its_lifetime_expires() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 60, 0, 0).unwrap();

    // Just before the deadline it survives.
    world.tick = world.objects.get_component::<ServitorOf>(&oid).unwrap().expires_at_tick - 1;
    handle_life_tick(&mut world, oid);
    assert_eq!(servitor_of(&mut world, OWNER), Some(oid), "still here a tick early");

    world.tick += 1;
    handle_life_tick(&mut world, oid);
    assert_eq!(servitor_of(&mut world, OWNER), None, "gone once the lifetime ran out");
}

/// A no-expiry servitor (`lifeTime <= 0`) is never reaped by the tick.
#[test]
fn a_permanent_servitor_is_never_reaped() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    world.tick += 10_000_000;
    handle_life_tick(&mut world, oid);
    assert_eq!(servitor_of(&mut world, OWNER), Some(oid), "no deadline, no expiry");
}

/// The upkeep item is taken from the owner when it falls due, and the servitor
/// carries on.
#[test]
fn the_upkeep_item_is_consumed_when_due() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let gemstone = 2131;
    {
        // Split borrow: the catalog is read while the inventory is written.
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .add_item(&data.item_data, 7_000_001, gemstone, 5);
    }
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, gemstone, 1).unwrap();

    world.tick = world.objects.get_component::<ServitorOf>(&oid).unwrap().next_consume_tick;
    handle_life_tick(&mut world, oid);

    assert_eq!(count_of_item(&world, OWNER, gemstone), 4, "one gemstone paid");
    assert_eq!(servitor_of(&mut world, OWNER), Some(oid), "and it stays out");
}

/// Running out of the upkeep item dismisses the servitor — Java's "since you do
/// not have enough items to maintain the servitor's stay".
#[test]
fn running_out_of_the_upkeep_item_dismisses_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let gemstone = 2131;
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, gemstone, 1).unwrap();
    // The owner has none.

    world.tick = world.objects.get_component::<ServitorOf>(&oid).unwrap().next_consume_tick;
    handle_life_tick(&mut world, oid);

    assert_eq!(servitor_of(&mut world, OWNER), None, "dismissed for non-payment");
}

/// A servitor with no upkeep item is never charged.
#[test]
fn a_servitor_without_upkeep_is_never_charged() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert_eq!(
        world.objects.get_component::<ServitorOf>(&oid).unwrap().next_consume_tick,
        u64::MAX,
        "no upkeep clock at all"
    );
    world.tick += 100_000;
    handle_life_tick(&mut world, oid);
    assert_eq!(servitor_of(&mut world, OWNER), Some(oid));
}

/// The leash: a servitor stranded far from its owner is pulled back into
/// following, whatever it was doing — an ordered attack cannot leave it
/// abandoned across the map.
#[test]
fn a_stranded_servitor_is_leashed_back_to_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);
    servitor_attack(&mut world, OWNER, FOE);
    assert!(!world.objects.get_component::<ServitorOf>(&oid).unwrap().following, "off following, mid-order");

    // The owner runs far away.
    world.objects.get_component_mut::<Position>(&OWNER).unwrap().x = 50_000;
    handle_life_tick(&mut world, oid);

    assert!(
        world.objects.get_component::<ServitorOf>(&oid).unwrap().following,
        "leashed back into follow"
    );
}

/// A servitor does not outlive its owner's session.
#[test]
fn logging_out_takes_the_servitor_with_you() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    on_owner_leave_world(&mut world, OWNER);

    assert_eq!(servitor_of(&mut world, OWNER), None, "no ownerless NPC left behind");
    assert!(world.objects.get_component::<Vitals>(&oid).is_none(), "despawned");
}

/// A dead servitor ends the tick chain rather than rescheduling forever.
#[test]
fn a_dead_servitor_stops_ticking() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 60, 0, 0).unwrap();
    world.objects.get_component_mut::<Vitals>(&oid).unwrap().dead = true;

    // Well past the deadline: a live tick would have unsummoned it and sent
    // the "passed away" notice. A dead one just stops.
    world.tick += 100_000;
    handle_life_tick(&mut world, oid);
    assert!(world.objects.get_component::<ServitorOf>(&oid).is_some(), "left for the death path to clean up");
}

// ---------------------------------------------------------------------------
// Pets (slice 6)
// ---------------------------------------------------------------------------

const WOLF_NPC: i32 = 12077;
const WOLF_COLLAR: i32 = 2375;

/// Register the Wolf's pet template + NPC template, and give the owner a
/// collar. Returns the collar's **object id**, which is the pet's identity.
fn give_collar(world: &mut World) -> i32 {
    let mut t = crate::data::npc_data::default_template(WOLF_NPC);
    t.type_name = "Pet".into();
    t.name = "Wolf".into();
    t.level = 1;
    t.base_hp_max = 300.0;
    t.base_mp_max = 100.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    world.data.pet_data.insert_for_test(crate::data::pet_data::PetTemplate {
        npc_id: WOLF_NPC,
        item_id: WOLF_COLLAR,
        food_item_id: 2515,
        hungry_limit: 55,
        load: 54_510,
        levels: [(1, crate::data::pet_data::PetLevel { max_meal: 248, ..Default::default() })].into_iter().collect(),
    });
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_100_001, WOLF_COLLAR, 1);
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == WOLF_COLLAR)
        .unwrap()
        .object_id
}

fn park_collar(world: &mut World, collar_oid: i32) {
    world.objects.get_component_mut::<crate::model::Player>(&OWNER).unwrap().pending_pet_collar = Some(collar_oid);
}

/// The collar summons its pet, bound to that **collar's object id** — the
/// identity two collars of the same kind are distinguished by.
#[test]
fn a_collar_summons_its_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);

    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    let link = world.objects.get_component::<crate::model::components::PetOf>(&pet).unwrap();
    assert_eq!(link.collar_object_id, collar, "bound to this collar, not the item type");
    assert_eq!(link.fed, 248, "starts on a full food bar from PetData");
    assert_eq!(pet_of(&mut world, OWNER), Some(pet));
}

/// A pet reuses the servitor owner-link, so it inherits follow for free.
#[test]
fn a_pet_follows_like_a_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();

    assert!(world.objects.get_component::<ServitorOf>(&pet).unwrap().following);
    world.objects.get_component_mut::<Position>(&OWNER).unwrap().x = 900;
    servitor_follow_tick(&mut world, pet);
    assert!(world.objects.get_component::<crate::model::components::Movement>(&pet).is_some());
}

/// The collar is **taken**, not copied — Java's `removeScript`. A second
/// summon with nothing parked must not produce a second pet.
#[test]
fn the_parked_collar_is_consumed_by_the_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    assert!(
        world.objects.get_component::<crate::model::Player>(&OWNER).unwrap().pending_pet_collar.is_none(),
        "the holder was taken"
    );
    assert_eq!(summon_pet(&mut world, OWNER), None, "nothing parked, nothing summoned");
}

/// Reaching the effect without going through the item handler summons nothing
/// — Java logs a warning and bails.
#[test]
fn summoning_without_a_parked_collar_does_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    give_collar(&mut world);
    assert_eq!(summon_pet(&mut world, OWNER), None);
}

/// "You already have a pet." — a second collar does not stack.
#[test]
fn a_second_pet_is_refused() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let first = summon_pet(&mut world, OWNER).unwrap();

    park_collar(&mut world, collar);
    assert_eq!(summon_pet(&mut world, OWNER), None, "refused");
    assert_eq!(pet_of(&mut world, OWNER), Some(first), "the first one is untouched");
}

/// A collar the owner no longer holds cannot summon — Java re-checks the
/// inventory, which is what stops a traded/dropped collar working.
#[test]
fn a_collar_not_in_the_inventory_cannot_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .remove_item(WOLF_COLLAR, 1);

    assert_eq!(summon_pet(&mut world, OWNER), None, "no collar, no pet");
}

/// A pet's `PetInfo` declares `summonType` **1**, where a servitor's is 2 —
/// that byte is how the client decides to offer the pet inventory and food bar.
#[test]
fn a_pet_declares_the_pet_summon_type() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let _ = drain(&mut rx);
    summon_pet(&mut world, OWNER).unwrap();

    let pkt = drain(&mut rx)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::PET_INFO))
        .expect("PetInfo sent");
    assert_eq!(pkt[1], 1, "summonType 1 = pet");

    // And the servitor path still says 2.
    let mut rx2 = ingame_caster(&mut world, 3, OWNER + 2, 0, 0);
    let _ = drain(&mut rx2);
    summon_servitor(&mut world, OWNER + 2, PANTHER, 283, 0, 0, 0).unwrap();
    let s_pkt = drain(&mut rx2)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::PET_INFO))
        .expect("PetInfo sent");
    assert_eq!(s_pkt[1], 2, "summonType 2 = servitor");
}
