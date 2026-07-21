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
        levels: [(
            1,
            crate::data::pet_data::PetLevel {
                max_meal: 248,
                consume_meal_in_normal: 10,
                consume_meal_in_battle: 15,
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
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

// ---------------------------------------------------------------------------
// Pet persistence (slice 7)
// ---------------------------------------------------------------------------

use crate::model::components::{PetOf, PlayerPets};

/// Give the Wolf template a level-2 row so level-dependent lookups have
/// somewhere to move to — with a single level every "restored at level N"
/// assertion would pass vacuously.
fn add_wolf_level_2(world: &mut World) {
    let mut t = world.data.pet_data.get(WOLF_NPC).unwrap().clone();
    t.levels.insert(
        2,
        crate::data::pet_data::PetLevel { max_meal: 300, exp: 5_000, ..Default::default() },
    );
    world.data.pet_data.insert_for_test(t);
}

fn saved_row(collar_oid: i32, level: i32, exp: i64, fed: i32, cur_hp: f64) -> crate::db::PetRow {
    crate::db::PetRow { collar_object_id: collar_oid, name: "Wolf".into(), level, cur_hp, cur_mp: 10.0, exp, sp: 7, fed }
}

fn put_saved(world: &mut World, row: crate::db::PetRow) {
    world.objects.get_component_mut::<PlayerPets>(&OWNER).unwrap().0.insert(row.collar_object_id, row);
}

/// With no saved row the pet is brand new: template level, a full food bar and
/// full vitals — Java's two-arg `Pet` constructor.
#[test]
fn a_pet_with_no_saved_row_is_fresh() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    add_wolf_level_2(&mut world);
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 1, "fresh pet takes the template level");
    assert_eq!(pet.fed, pet.max_fed, "fresh pet starts fed");
    assert_eq!(pet.max_fed, 248, "max_meal for level 1");
    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "fresh pet spawns at full HP");
}

/// A saved row is what the pet comes back as — the whole point of the table.
#[test]
fn a_saved_pet_is_restored_from_its_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    add_wolf_level_2(&mut world);
    put_saved(&mut world, saved_row(collar, 2, 6_000, 90, 42.0));
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "restored at the saved level, not the template's");
    assert_eq!(pet.exp, 6_000);
    assert_eq!(pet.sp, 7);
    assert_eq!(pet.fed, 90, "the food bar carries over — it does not refill on summon");
    assert_eq!(pet.max_fed, 300, "max_meal follows the restored level");
    assert_eq!(world.objects.get_component::<Vitals>(&pet_oid).unwrap().cur_hp, 42.0, "wounded pet stays wounded");
}

/// Java's "avoiding pet delevels due to exp per level values changed": a stored
/// exp below what the pet's level now costs is raised to that level's floor,
/// rather than the pet silently dropping a level when the curve is retuned.
#[test]
fn restored_exp_is_floored_at_the_level_cost() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    add_wolf_level_2(&mut world);
    // Level 2 now costs 5000 exp; this row predates that and holds only 100.
    put_saved(&mut world, saved_row(collar, 2, 100, 90, 42.0));
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "the pet keeps its level");
    assert_eq!(pet.exp, 5_000, "exp is raised to the level's floor instead");
}

/// A food bar saved above the level's capacity is clamped, not carried —
/// otherwise a datapack nerf would leave pets permanently over-full.
#[test]
fn restored_fed_is_clamped_to_max_meal() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    put_saved(&mut world, saved_row(collar, 1, 0, 9_999, 42.0));

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.fed, 248, "clamped to level 1's max_meal");
}

/// `sync_pet_row` is what makes any of this reach the DB: it folds the live
/// pet's state back into `PlayerPets`, which the character flush reads.
#[test]
fn syncing_writes_live_pet_state_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();

    // The pet takes a beating and burns some food.
    world.objects.get_component_mut::<Vitals>(&pet_oid).unwrap().cur_hp = 33.0;
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 12;

    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    let row = world.objects.get_component::<PlayerPets>(&OWNER).unwrap().0.get(&collar).unwrap().clone();
    assert_eq!(row.cur_hp, 33.0, "the wound is what gets saved");
    assert_eq!(row.fed, 12);
    assert_eq!(row.collar_object_id, collar, "keyed by the collar, as the table is");
}

/// The round trip the gate actually asks for: summon, take damage, log out,
/// summon again — the pet comes back as it was left.
#[test]
fn a_pet_survives_an_unsummon_round_trip() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world.objects.get_component_mut::<Vitals>(&pet_oid).unwrap().cur_hp = 25.0;
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 60;

    // Owner logs out: state is captured, then the pet leaves the world.
    crate::game_loop::servitor::on_owner_leave_world(&mut world, OWNER);
    assert!(pet_of(&mut world, OWNER).is_none(), "the pet is gone with its owner");

    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.fed, 60, "it comes back as hungry as it was left");
    assert_eq!(world.objects.get_component::<Vitals>(&pet_oid).unwrap().cur_hp, 25.0, "and as wounded");
}

/// Destroying the collar destroys the pet bound to it — Java unsummons it and
/// deletes the row. Object ids are recycled, so a surviving row would
/// eventually hand a stale pet to an unrelated item.
#[test]
fn destroying_the_collar_drops_the_saved_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let _ = summon_pet(&mut world, OWNER).unwrap();
    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    assert!(world.objects.get_component::<PlayerPets>(&OWNER).unwrap().0.contains_key(&collar));

    let mut body = Vec::new();
    body.extend_from_slice(&collar.to_le_bytes());
    body.extend_from_slice(&1i64.to_le_bytes());
    crate::game_loop::items::handle_request_destroy_item(&mut world, CID, &body);

    assert!(pet_of(&mut world, OWNER).is_none(), "the summoned pet is unsummoned with its collar");
    assert!(
        !world.objects.get_component::<PlayerPets>(&OWNER).unwrap().0.contains_key(&collar),
        "and its saved row goes with it"
    );
}

// ---------------------------------------------------------------------------
// Pet feeding (slice 8)
// ---------------------------------------------------------------------------

use crate::model::inventory::PetInventory;

const WOLF_FOOD: i32 = 2515;
/// The Wolf Food skill (2048) — a single `Feed` effect restoring 100.
const WOLF_FOOD_SKILL: i32 = 2048;

/// Register the food item + its `Feed` skill so the eat path has something
/// real to run. Without the skill the item would be consumed for nothing,
/// which is exactly the bug the `Feed` parse arm fixes.
fn register_food(world: &mut World, restores: i32) {
    let mut item = crate::data::item_data::ItemTemplate::default();
    item.item_id = WOLF_FOOD;
    item.name = "Wolf Food".into();
    item.is_stackable = true;
    item.item_skills = vec![(WOLF_FOOD_SKILL, 1)];
    world.data.item_data.insert_for_test(item);

    let skill = crate::model::skill::Skill {
        id: WOLF_FOOD_SKILL,
        level: 1,
        effects: vec![crate::model::skill::SkillEffect::Feed { normal: restores }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill);
}

fn put_food_in_pet(world: &mut World, count: i64) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .add_item(&data.item_data, 7_200_001, WOLF_FOOD, count);
}

fn fed(world: &World, pet_oid: i32) -> i32 {
    world.objects.get_component::<PetOf>(&pet_oid).unwrap().fed
}

/// A summoned pet burns food on every tick — the drain that makes feeding
/// necessary at all.
#[test]
fn the_feed_tick_burns_food() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    assert_eq!(fed(&world, pet_oid), 248, "starts full");

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(fed(&world, pet_oid), 238, "one normal-rate helping burned");
}

/// The bar is not allowed below zero — Java's `fed > consume ? fed - consume : 0`.
#[test]
fn the_feed_tick_floors_at_zero() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 4;

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(fed(&world, pet_oid), 0, "cost exceeded the bar — floored, not negative");
    assert!(crate::game_loop::servitor::is_uncontrollable(&world, pet_oid), "an empty bar means starving");
}

/// A hungry pet with food in *its own* inventory eats without being told.
/// `hungry_limit` is 55%, so the bar must be under 136 for this to fire.
#[test]
fn a_hungry_pet_eats_from_its_own_inventory() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 2);
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 100;

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    // 100 - 10 burned = 90, hungry (< 136), so it eats one 100-point helping.
    assert_eq!(fed(&world, pet_oid), 190, "burned 10, then ate 100");
    assert_eq!(
        world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(WOLF_FOOD),
        1,
        "exactly one helping consumed"
    );
}

/// A pet that is not hungry leaves its food alone — otherwise a full bar would
/// eat through the whole stack.
#[test]
fn a_full_pet_does_not_eat() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 2);

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(
        world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(WOLF_FOOD),
        2,
        "full pet leaves the stack alone"
    );
}

/// Feeding is capped at the level's `max_meal` — Java's `setCurrentFed` clamp.
/// Measured from a bar with room in it, so the clamp is what's under test
/// rather than an already-full bar.
#[test]
fn feeding_is_capped_at_max_meal() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 200;

    crate::game_loop::servitor::apply_feed(&mut world, pet_oid, 100);
    assert_eq!(fed(&world, pet_oid), 248, "200 + 100 clamped to max_meal, not banked");
}

/// Food reaches the pet by transfer from the owner — the client's only route,
/// since Java's `PetFood` handler refuses an unmounted player.
#[test]
fn food_transfers_to_the_pet_and_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let _ = summon_pet(&mut world, OWNER).unwrap();

    let food_oid = {
        let World { data, objects, .. } = &mut world;
        objects.get_component_mut::<crate::model::inventory::Inventory>(&OWNER).unwrap().add_item(
            &data.item_data,
            7_300_001,
            WOLF_FOOD,
            5,
        )
    };

    let mut body = Vec::new();
    body.extend_from_slice(&food_oid.to_le_bytes());
    body.extend_from_slice(&3i64.to_le_bytes());
    crate::game_loop::servitor::handle_give_item_to_pet(&mut world, CID, &body);

    assert_eq!(world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(WOLF_FOOD), 3);
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&OWNER).unwrap().count_of(WOLF_FOOD),
        2,
        "the owner keeps the remainder"
    );

    // And back again.
    let pet_food_oid =
        world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.items()[0].object_id;
    let mut body = Vec::new();
    body.extend_from_slice(&pet_food_oid.to_le_bytes());
    body.extend_from_slice(&3i64.to_le_bytes());
    crate::game_loop::servitor::handle_get_item_from_pet(&mut world, CID, &body);
    assert_eq!(world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(WOLF_FOOD), 0);
    assert_eq!(
        world.objects.get_component::<crate::model::inventory::Inventory>(&OWNER).unwrap().count_of(WOLF_FOOD),
        5,
        "all five back with the owner"
    );
}

/// The collar can't be stored inside the pet it summons — it would become
/// unreachable the moment the pet is unsummoned.
#[test]
fn the_collar_cannot_be_given_to_its_own_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let _ = summon_pet(&mut world, OWNER).unwrap();

    let mut body = Vec::new();
    body.extend_from_slice(&collar.to_le_bytes());
    body.extend_from_slice(&1i64.to_le_bytes());
    crate::game_loop::servitor::handle_give_item_to_pet(&mut world, CID, &body);

    assert_eq!(world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(WOLF_COLLAR), 0);
}

/// Manual feeding through the pet window, and the refusal for anything the
/// species does not eat.
#[test]
fn the_owner_can_feed_the_pet_by_hand() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 1);
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 50;

    let food_oid = world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.items()[0].object_id;
    let body = food_oid.to_le_bytes().to_vec();
    crate::game_loop::servitor::handle_pet_use_item(&mut world, CID, &body);

    assert_eq!(fed(&world, pet_oid), 150, "hand-fed one helping");
    assert_eq!(world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(WOLF_FOOD), 0);
}

/// A pet only eats its own species' food (Java `canEatFoodId`).
#[test]
fn a_pet_refuses_food_it_does_not_eat() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 50;

    // A different item entirely, sitting in the pet's bag.
    let mut other = crate::data::item_data::ItemTemplate::default();
    other.item_id = 57;
    other.is_stackable = true;
    world.data.item_data.insert_for_test(other);
    let oid = {
        let World { data, objects, .. } = &mut world;
        objects.get_component_mut::<PetInventory>(&OWNER).unwrap().0.add_item(&data.item_data, 7_400_001, 57, 1)
    };

    let body = oid.to_le_bytes().to_vec();
    crate::game_loop::servitor::handle_pet_use_item(&mut world, CID, &body);

    assert_eq!(fed(&world, pet_oid), 50, "bar untouched");
    assert_eq!(
        world.objects.get_component::<PetInventory>(&OWNER).unwrap().0.count_of(57),
        1,
        "and the item is not consumed"
    );
}

/// The fixture above uses a hand-built skill, so it cannot catch a parse-arm
/// mistake. This reads the **real** Wolf Food skill out of the datapack: if
/// `<effect name="Feed"><normal>100</normal>` stops reaching `SkillEffect::Feed`,
/// every pet food in the game silently restores nothing.
#[test]
fn the_real_wolf_food_skill_parses_its_feed_value() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    let skill = skills.get(2048, 1).expect("Wolf Food skill 2048 exists in the datapack");
    let feed = skill
        .effects
        .iter()
        .find_map(|e| match e {
            crate::model::skill::SkillEffect::Feed { normal } => Some(*normal),
            _ => None,
        })
        .expect("Wolf Food carries a Feed effect");
    assert_eq!(feed, 100, "the <normal> value from 2048");
}

// ---------------------------------------------------------------------------
// Client-visibility gaps (slice 10)
// ---------------------------------------------------------------------------

/// A party member's summon must appear in everyone else's party window. The
/// count was hard-coded to 0, so it never did — the third hard-coded-zero
/// count found by the sweep that started with `CharInfo`'s cubics.
#[test]
fn the_party_window_carries_a_members_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let before = crate::game_loop::party::member_view(&world, OWNER).unwrap();
    assert!(before.summons.is_empty(), "no summon, no rows");

    summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    let after = crate::game_loop::party::member_view(&world, OWNER).unwrap();
    assert_eq!(after.summons.len(), 1, "the servitor shows up in the party window");
    assert_eq!(after.summons[0].summon_type, 2, "2 = servitor");
    assert!(after.summons[0].max_hp > 0, "and carries real vitals for the HP bar");
}

/// A pet is reported with the pet discriminator, not the servitor one — the
/// client uses it to decide what the party window row looks like.
#[test]
fn a_pet_reports_the_pet_summon_type_in_the_party_window() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    let view = crate::game_loop::party::member_view(&world, OWNER).unwrap();
    assert_eq!(view.summons.len(), 1);
    assert_eq!(view.summons[0].summon_type, 1, "1 = pet");
}

/// The owner→summon link is what makes the lookup readable from `&World`.
/// Unsummoning must clear it, or the party window would keep showing a
/// creature that no longer exists.
#[test]
fn unsummoning_clears_the_owner_link() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    assert!(servitor_of(&world, OWNER).is_some());

    unsummon_servitor(&mut world, OWNER);
    assert!(servitor_of(&world, OWNER).is_none(), "link cleared");
    assert!(
        crate::game_loop::party::member_view(&world, OWNER).unwrap().summons.is_empty(),
        "and the party window row goes with it"
    );
}

// ---------------------------------------------------------------------------
// Pet experience (slice 12)
// ---------------------------------------------------------------------------

use crate::game_loop::servitor::{add_pet_exp, split_exp_with_pet};

/// Extend the Wolf with a level-2 row that costs 5000 exp and a real
/// `get_exp_type`, so the split and the level-up both have somewhere to go.
fn wolf_with_exp_curve(world: &mut World) {
    let mut t = world.data.pet_data.get(WOLF_NPC).unwrap().clone();
    // Three levels, not two: with only two, a level-2 pet sits at the species
    // cap and the death penalty's level *band* is empty — which made the first
    // draft of the death tests measure nothing.
    for (lvl, exp, meal) in [(1, 0i64, 248), (2, 5_000, 300), (3, 20_000, 340)] {
        t.levels.insert(
            lvl,
            crate::data::pet_data::PetLevel {
                max_meal: meal,
                consume_meal_in_normal: 10,
                consume_meal_in_battle: 15,
                exp,
                // The owner keeps 73%, so the pet takes 27% — the real value
                // on this species.
                owner_exp_taken: 73,
                // Level 2 is strictly stronger, so "did levelling do anything?"
                // is answerable rather than vacuous.
                p_atk: 10.0 * lvl as f64,
                m_atk: 8.0 * lvl as f64,
                p_def: 20.0 * lvl as f64,
                m_def: 15.0 * lvl as f64,
                max_hp: 100.0 * lvl as f64,
                max_mp: 50.0 * lvl as f64,
                regen_hp: 2.0,
                regen_mp: 0.9,
            },
        );
    }
    world.data.pet_data.insert_for_test(t);
}

fn summoned_pet(world: &mut World) -> i32 {
    let collar = give_collar(world);
    wolf_with_exp_curve(world);
    park_collar(world, collar);
    summon_pet(world, OWNER).unwrap()
}

/// The pet's cut comes **out of** the owner's award, not on top of it.
#[test]
fn a_nearby_pet_takes_its_cut_from_the_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summoned_pet(&mut world);

    let (owner_ratio, pet_exp, pet_sp) = split_exp_with_pet(&world, OWNER, 1000.0, 100.0);
    assert_eq!(owner_ratio, 0.73, "the owner keeps get_exp_type percent");
    assert!((pet_exp - 270.0).abs() < 0.001, "the pet takes the remaining 27% ({pet_exp})");
    assert!((pet_sp - 27.0).abs() < 0.001);
}

/// Out of range, the pet earns nothing and the owner keeps the lot.
#[test]
fn a_distant_pet_earns_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world.objects.get_component_mut::<Position>(&pet_oid).unwrap().x += 10_000;

    let (owner_ratio, pet_exp, _) = split_exp_with_pet(&world, OWNER, 1000.0, 100.0);
    assert_eq!(owner_ratio, 1.0, "the owner keeps everything");
    assert_eq!(pet_exp, 0.0);
}

/// With no pet at all the owner's award is untouched — the guard that keeps
/// this change invisible to every player without one.
#[test]
fn no_pet_means_no_split() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let (owner_ratio, pet_exp, pet_sp) = split_exp_with_pet(&world, OWNER, 1000.0, 100.0);
    assert_eq!((owner_ratio, pet_exp, pet_sp), (1.0, 0.0, 0.0));
}

/// **A starving pet earns nothing** — Java's `isUncontrollable()` guard in
/// `PetStat.addExp`. This is the link between the feeding loop and
/// progression: let the food bar hit zero and the pet stops growing.
#[test]
fn a_starving_pet_earns_no_exp() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world.objects.get_component_mut::<PetOf>(&pet_oid).unwrap().fed = 0;

    add_pet_exp(&mut world, OWNER, 1000.0, 100.0);
    assert_eq!(world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp, 0, "starving pets do not learn");
}

/// Crossing the level threshold levels the pet, and the food capacity moves
/// with it.
#[test]
fn a_pet_levels_when_it_earns_enough() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    assert_eq!(world.objects.get_component::<PetOf>(&pet_oid).unwrap().level, 1);

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "crossed the 5000-exp threshold");
    assert_eq!(pet.max_fed, 300, "food capacity follows the level");
}

/// A pet cannot pass the top level its species table defines — every per-level
/// lookup would fall off the end.
#[test]
fn a_pet_stops_at_its_species_max_level() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    add_pet_exp(&mut world, OWNER, 10_000_000.0, 0.0);
    assert_eq!(
        world.objects.get_component::<PetOf>(&pet_oid).unwrap().level,
        3,
        "capped at the highest level the table defines"
    );
}

/// Java `getControlItem().setEnchantLevel(getLevel())` — the collar's enchant
/// level *is* the pet's level, which is how a collar advertises its pet
/// without being summoned.
#[test]
fn levelling_stamps_the_pets_level_onto_its_collar() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let enchant = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.object_id == collar)
        .unwrap()
        .enchant_level;
    assert_eq!(enchant, 2, "the collar reads +2 once the pet hits level 2");
}

/// End-to-end through the real reward path: the helper being right is not
/// enough if `add_exp_and_sp` never calls it. A pet out of range and a pet
/// beside its owner must produce *different* owner awards from the same kill.
#[test]
fn the_reward_path_actually_splits_with_the_pet() {
    let owner_exp_after = |pet_nearby: bool| {
        let (mut world, _db, _l) = servitor_world();
        let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
        let pet_oid = summoned_pet(&mut world);
        if !pet_nearby {
            world.objects.get_component_mut::<Position>(&pet_oid).unwrap().x += 10_000;
        }
        world.objects.get_component_mut::<crate::model::Player>(&OWNER).unwrap().exp = 0;
        crate::game_loop::death::add_exp_and_sp(&mut world, OWNER, 1000.0, 100.0, false);
        (
            world.objects.get_component::<crate::model::Player>(&OWNER).unwrap().exp,
            world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp,
        )
    };

    let (owner_alone, pet_idle) = owner_exp_after(false);
    let (owner_shared, pet_fed) = owner_exp_after(true);

    assert_eq!(pet_idle, 0, "a distant pet learns nothing");
    assert_eq!(pet_fed, 270, "a nearby pet takes 27% of the kill");
    assert_eq!(owner_alone, 1000, "without a pet in range the owner keeps it all");
    assert_eq!(owner_shared, 730, "with a pet in range the owner keeps only 73%");
}

// ---------------------------------------------------------------------------
// Pet stats (slice 13)
// ---------------------------------------------------------------------------

fn combat(world: &World, oid: i32) -> crate::model::components::CombatStats {
    *world.objects.get_component::<crate::model::components::CombatStats>(&oid).unwrap()
}

/// A pet's stats come from its **per-level pet row**, not its NPC template.
/// The Wolf's NPC fixture is level 1 with 300 HP; its pet row says 100.
#[test]
fn a_pets_stats_come_from_the_pet_table_not_the_npc_template() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let max_hp = world.objects.get_component::<Vitals>(&pet_oid).unwrap().max_hp;
    let template_hp = world.data.npc_data.get(WOLF_NPC).unwrap().base_hp_max;
    assert_ne!(
        max_hp as f64, template_hp,
        "the NPC template's HP ({template_hp}) must not be what the pet uses"
    );
    assert!(max_hp > 0, "and the pet has real HP ({max_hp})");
}

/// The point of the whole slice: levelling has to make the pet *stronger*.
/// Before this, the level number moved and every combat stat stayed put.
#[test]
fn levelling_makes_the_pet_stronger() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let before = combat(&world, pet_oid);
    let hp_before = world.objects.get_component::<Vitals>(&pet_oid).unwrap().max_hp;

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    assert_eq!(world.objects.get_component::<PetOf>(&pet_oid).unwrap().level, 2, "it levelled");

    let after = combat(&world, pet_oid);
    let hp_after = world.objects.get_component::<Vitals>(&pet_oid).unwrap().max_hp;
    assert!(after.p_atk > before.p_atk, "p.atk grew ({} → {})", before.p_atk, after.p_atk);
    assert!(after.m_atk > before.m_atk, "m.atk grew ({} → {})", before.m_atk, after.m_atk);
    assert!(after.p_def > before.p_def, "p.def grew ({} → {})", before.p_def, after.p_def);
    assert!(hp_after > hp_before, "max HP grew ({hp_before} → {hp_after})");
}

/// Levelling must neither heal nor wound the pet — Java's stat recompute keeps
/// the bar where it was, and a level-up that silently full-heals would be a
/// free heal on demand.
#[test]
fn levelling_preserves_the_hp_fraction() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    {
        let v = world.objects.get_component_mut::<Vitals>(&pet_oid).unwrap();
        v.cur_hp = v.max_hp as f64 / 2.0;
    }

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    let frac = v.cur_hp / v.max_hp as f64;
    assert!((frac - 0.5).abs() < 0.01, "still at half health after levelling ({frac})");
}

/// A row missing a stat falls back to the NPC template rather than zeroing it.
/// Without this a single datapack gap gives the pet 0 max HP — which is how
/// this guard was found, when the shared fixture (no `org_hp`) produced a pet
/// that restored at 0 HP.
#[test]
fn a_missing_stat_row_falls_back_to_the_npc_template() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    // `give_collar`'s fixture carries no combat stats at all.
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();

    let max_hp = world.objects.get_component::<Vitals>(&pet_oid).unwrap().max_hp;
    assert!(max_hp > 0, "fell back to the template instead of zeroing the pet");
}

// ---------------------------------------------------------------------------
// Pet death (slice 14)
// ---------------------------------------------------------------------------

use crate::game_loop::servitor::pet_restore_exp;

fn pet_exp(world: &World, pet_oid: i32) -> i64 {
    world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp
}

/// Dying costs the pet experience — `percentLost = -0.07 × level + 6.5` of the
/// current level band.
#[test]
fn a_dying_pet_loses_experience() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before = pet_exp(&world, pet_oid);
    assert_eq!(world.objects.get_component::<PetOf>(&pet_oid).unwrap().level, 2);

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let after = pet_exp(&world, pet_oid);
    assert!(after < before, "exp was lost on death ({before} → {after})");
    assert_eq!(
        world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp_before_death,
        before,
        "the pre-death total is recorded for a later resurrection"
    );
}

/// The penalty can never drop the pet below its current level's floor —
/// otherwise dying would de-level it, and Java's `addExp(-lost)` does not.
#[test]
fn the_death_penalty_cannot_delevel_a_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 5_000.0, 0.0);
    assert_eq!(world.objects.get_component::<PetOf>(&pet_oid).unwrap().level, 2, "exactly at the threshold");

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "still level 2");
    assert_eq!(pet.exp, 5_000, "held at the level floor rather than dropping below it");
}

/// A duel death costs nothing — Java skips the penalty entirely there.
#[test]
fn a_duel_death_costs_the_pet_no_experience() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before = pet_exp(&world, pet_oid);

    // `is_in_duel` is presence of `DuelRef`, so marking the owner is the whole
    // condition Java tests.
    world.objects.add_components(&OWNER, crate::model::components::DuelRef(1));
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);

    assert_eq!(pet_exp(&world, pet_oid), before, "no exp lost to a duel death");
}

/// Resurrection hands back a share of what death took, and consumes the
/// record so a second revive restores nothing.
#[test]
fn resurrection_restores_a_share_of_the_lost_experience() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before_death = pet_exp(&world, pet_oid);

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let after_death = pet_exp(&world, pet_oid);
    let lost = before_death - after_death;
    assert!(lost > 0);

    pet_restore_exp(&mut world, pet_oid, 100.0);
    assert_eq!(pet_exp(&world, pet_oid), before_death, "a full-power revive restores all of it");

    // The record is spent.
    pet_restore_exp(&mut world, pet_oid, 100.0);
    assert_eq!(pet_exp(&world, pet_oid), before_death, "a second revive restores nothing more");
}

/// A partial-power resurrection restores proportionally.
#[test]
fn a_partial_resurrection_restores_part_of_the_loss() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before_death = pet_exp(&world, pet_oid);

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let after_death = pet_exp(&world, pet_oid);
    let lost = before_death - after_death;

    pet_restore_exp(&mut world, pet_oid, 50.0);
    let regained = pet_exp(&world, pet_oid) - after_death;
    assert_eq!(regained, (lost as f64 * 0.5).round() as i64, "half the loss came back");
}

/// Slice 7 left this branch as a `TODO(G29)` because a pet could not yet be
/// stored dead. It can now: a pet saved with `curHp < 1` comes back as a
/// corpse rather than silently alive at 0 HP.
#[test]
fn a_pet_stored_dead_is_restored_dead() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    put_saved(&mut world, saved_row(collar, 1, 0, 100, 0.0));
    park_collar(&mut world, collar);

    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    assert!(
        world.objects.get_component::<Vitals>(&pet_oid).unwrap().dead,
        "a pet stored with no HP comes back dead"
    );
}

/// At the species' top level there is no next-level band, so the penalty
/// computes to zero rather than to garbage. Java would throw here (its
/// `getExpForLevel(level + 1)` has no row and it logs an NPE); a max-level pet
/// simply losing nothing is the safer reading, and it is pinned because the
/// death tests silently measured *only* this case until the fixture grew a
/// third level.
#[test]
fn a_max_level_pet_loses_nothing_on_death() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 10_000_000.0, 0.0);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 3, "at the species cap");
    let before = pet.exp;

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    assert_eq!(pet_exp(&world, pet_oid), before, "no band above the cap, so no penalty");
}

// ---------------------------------------------------------------------------
// Pet resurrection (slice 15)
// ---------------------------------------------------------------------------

/// Casting a resurrection on a dead pet puts the dialog in front of its
/// **owner** — Java's `effected.getActingPlayer().reviveRequest(…, isPet, …)`.
#[test]
fn reviving_a_pet_asks_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0);

    let req = world.objects.get_component::<crate::model::Player>(&OWNER).unwrap().revive_request;
    let req = req.expect("the owner holds the proposal, not the pet");
    assert!(req.is_pet, "and it is flagged as a pet revival");
}

/// Accepting revives the pet and restores its lost experience.
#[test]
fn accepting_revives_the_pet_and_restores_its_exp() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    let before_death = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let after_death = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    assert!(after_death < before_death, "the penalty applied");

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0);
    assert!(crate::game_loop::death::handle_revive_answer(&mut world, OWNER, true));

    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert!(!v.dead, "the pet is alive again");
    assert!(v.cur_hp > 0.0, "with HP on the bar");
    let exp_now = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    assert!(exp_now > after_death, "some of the lost exp came back ({after_death} → {exp_now})");
}

/// Declining leaves the pet dead — and, as for a player, consumes the
/// proposal so it can be offered again.
#[test]
fn declining_leaves_the_pet_dead() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0);
    assert!(crate::game_loop::death::handle_revive_answer(&mut world, OWNER, false));

    assert!(world.objects.get_component::<Vitals>(&pet_oid).unwrap().dead, "still dead");
    assert!(
        world.objects.get_component::<crate::model::Player>(&OWNER).unwrap().revive_request.is_none(),
        "the proposal was consumed either way"
    );
}

/// A live pet is not a resurrection target.
#[test]
fn a_living_pet_is_not_proposed_for_resurrection() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0);
    assert!(world.objects.get_component::<crate::model::Player>(&OWNER).unwrap().revive_request.is_none());
}

/// Reviving the pet must not revive the *owner*: one field on the player
/// carries both cases, so the flag has to steer the outcome.
#[test]
fn a_pet_revival_does_not_revive_the_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    // Kill the owner too, so "did the wrong one revive?" is answerable.
    world.objects.get_component_mut::<Vitals>(&OWNER).unwrap().dead = true;

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0);
    crate::game_loop::death::handle_revive_answer(&mut world, OWNER, true);

    assert!(!world.objects.get_component::<Vitals>(&pet_oid).unwrap().dead, "the pet came back");
    assert!(world.objects.get_component::<Vitals>(&OWNER).unwrap().dead, "the owner did not");
}
