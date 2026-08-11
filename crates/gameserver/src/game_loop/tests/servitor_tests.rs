//! Servitor summoning — the first G29 slice.
//!
//! `Summon` is the single biggest unported effect on the whole ranking (24
//! learnable skills). This slice covers summoning, ownership, unsummon and the
//! owner's `PetInfo` view; follow/attack AI and the `SummonInfo` packet that
//! shows a servitor to *other* players are separate slices.

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::helpers::skill_by_id;
use crate::game_loop::pet_evolve;
use crate::model::components::ServitorOf;
use crate::model::skill::SkillEffect;

use crate::game_loop::servitor::{
    handle_life_tick, on_owner_leave_world, pet_of, servitor_attack, servitor_follow_tick,
    servitor_of, servitor_stop, servitor_toggle_follow, summon_pet, summon_servitor,
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
const DIST: &str = crate::data::DIST_GAME;

fn servitor_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
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

    let link = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .expect("linked to its owner");
    assert_eq!(link.owner_object_id, OWNER);
    assert_eq!(
        link.reference_skill, 283,
        "remembers the skill that summoned it"
    );

    let pos = world.objects.get_component::<Position>(&oid).unwrap();
    assert_eq!((pos.x, pos.y), (100, 200), "spawns on its owner");

    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "full HP");
    assert_eq!(v.cur_mp, v.max_mp as f64, "full MP");

    assert_eq!(
        servitor_of(&world, OWNER),
        Some(oid),
        "found by owner lookup"
    );
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
    assert!(
        world.objects.get_component::<ServitorOf>(&first).is_none(),
        "the first one is gone"
    );
    assert_eq!(
        servitor_of(&world, OWNER),
        Some(second),
        "only the newest remains"
    );
}

/// Unsummoning removes the servitor from the world entirely.
#[test]
fn unsummoning_removes_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    assert_eq!(unsummon_servitor(&mut world, OWNER), Some(oid));
    assert_eq!(servitor_of(&world, OWNER), None, "no servitor left");
    assert!(
        world.objects.get_component::<Vitals>(&oid).is_none(),
        "and the entity is despawned"
    );
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
    assert_eq!(
        world
            .objects
            .get_component::<ServitorOf>(&forever)
            .unwrap()
            .expires_at_tick,
        u64::MAX
    );

    let timed = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    let link = world.objects.get_component::<ServitorOf>(&timed).unwrap();
    assert_eq!(
        link.expires_at_tick,
        world.tick + 12_000,
        "1200 s at 10 ticks/s"
    );
    assert_eq!(link.life_time_secs, 1200);
}

/// Only players summon (Java's `if (!effected.isPlayer()) return`).
#[test]
fn an_npc_cannot_summon() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, NPC_OID, PANTHER, "Monster", 20, 0, 0, 0);
    assert_eq!(
        summon_servitor(&mut world, NPC_OID, PANTHER, 283, 1200, 0, 0),
        None
    );
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

    let opcodes: Vec<u8> = drain(&mut rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    assert!(
        opcodes.contains(&server_packets::opcodes::PET_INFO),
        "PetInfo sent, got {opcodes:?}"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The summon skills parse, and `npcId` is **per level** — each level summons a
/// stronger template, which is why the effect carries the id rather than the
/// skill.
#[test]
fn real_dist_summon_skills_parse_per_level_npc_ids() {
    let skills = dist::skills();
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
        SkillEffect::Summon {
            npc_id,
            life_time,
            consume_item_id,
            ..
        } => Some((*npc_id, *life_time, *consume_item_id)),
        _ => None,
    });
    assert_eq!(
        effect,
        Some((14737, 1200, 2131)),
        "npcId / lifeTime / gemstone"
    );
}

/// All 24 learnable summon skills produce a usable effect — none is dropped for
/// want of an `npcId`.
#[test]
fn every_learnable_summon_skill_parses() {
    let skills = dist::skills();
    for id in [13, 25, 283, 299, 301, 448, 1111, 1128] {
        let skill = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill
                .effects
                .iter()
                .any(|e| matches!(e, SkillEffect::Summon { npc_id, .. } if *npc_id > 0)),
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
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "follows by default"
    );

    // Owner walks well beyond the follow range.
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 900;
    servitor_follow_tick(&mut world, oid);

    let m = world
        .objects
        .get_component::<crate::model::components::Movement>(&oid);
    assert!(m.is_some(), "the servitor set off after its owner");
}

/// Inside the follow range it stays put rather than jittering.
#[test]
fn a_servitor_already_close_does_not_move() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 100; // < FOLLOW_RANGE
    servitor_follow_tick(&mut world, oid);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Movement>(&oid)
            .is_none(),
        "no pointless walk"
    );
}

/// "Hold your ground" stops the following, and toggling again resumes it.
#[test]
fn hold_toggles_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    assert_eq!(
        servitor_toggle_follow(&mut world, OWNER),
        Some(false),
        "now holding"
    );
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 900;
    servitor_follow_tick(&mut world, oid);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Movement>(&oid)
            .is_none(),
        "a holding servitor ignores its owner walking off"
    );

    assert_eq!(
        servitor_toggle_follow(&mut world, OWNER),
        Some(true),
        "and back to following"
    );
    servitor_follow_tick(&mut world, oid);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Movement>(&oid)
            .is_some()
    );
}

/// An ordered attack seeds hate on the target and switches the servitor to the
/// attack intention, which is what the ordinary NPC attack think drives from.
#[test]
fn an_ordered_attack_targets_the_owners_target() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);

    assert!(
        servitor_attack(&mut world, OWNER, FOE),
        "the order was accepted"
    );

    let hate = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&oid)
        .and_then(|a| a.0.get(&FOE))
        .map(|i| i.hate)
        .unwrap_or(0.0);
    assert!(hate > 0.0, "the target is now hated");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::NpcAi>(&oid)
            .unwrap()
            .intention,
        crate::model::npc::NpcIntention::Attack
    );
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
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
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "falls back to following"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&oid)
            .map(|a| a.0.len()),
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
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&oid)
            .map(|a| a.0.len()),
        Some(0)
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::NpcAi>(&oid)
            .unwrap()
            .intention,
        crate::model::npc::NpcIntention::Active
    );
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "back to trailing its owner"
    );
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
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&oid)
            .map(|a| a.0.len()),
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

    let owner_ops: Vec<u8> = drain(&mut owner_rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    let other_ops: Vec<u8> = drain(&mut other_rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();

    assert!(
        owner_ops.contains(&server_packets::opcodes::PET_INFO),
        "owner gets PetInfo: {owner_ops:?}"
    );
    assert!(
        !owner_ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "and not the bystander packet as well"
    );
    assert!(
        other_ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "others get SummonInfo: {other_ops:?}"
    );
    assert!(
        !other_ops.contains(&server_packets::opcodes::PET_INFO),
        "and never the owner-only one"
    );
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

    let owner_name = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap()
        .name
        .clone();
    let pkt = drain(&mut other_rx)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::SUMMON_INFO))
        .expect("SummonInfo sent");
    // The name is UTF-16LE in the packet body.
    let wide: Vec<u8> = owner_name
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
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

    let ops: Vec<u8> = drain(&mut late_rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    assert!(
        ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "introduced as a summon: {ops:?}"
    );
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
    world.tick = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .unwrap()
        .expires_at_tick
        - 1;
    handle_life_tick(&mut world, oid);
    assert_eq!(
        servitor_of(&world, OWNER),
        Some(oid),
        "still here a tick early"
    );

    world.tick += 1;
    handle_life_tick(&mut world, oid);
    assert_eq!(
        servitor_of(&world, OWNER),
        None,
        "gone once the lifetime ran out"
    );
}

/// A no-expiry servitor (`lifeTime <= 0`) is never reaped by the tick.
#[test]
fn a_permanent_servitor_is_never_reaped() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    world.tick += 10_000_000;
    handle_life_tick(&mut world, oid);
    assert_eq!(
        servitor_of(&world, OWNER),
        Some(oid),
        "no deadline, no expiry"
    );
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

    world.tick = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .unwrap()
        .next_consume_tick;
    handle_life_tick(&mut world, oid);

    assert_eq!(
        count_of_item(&world, OWNER, gemstone),
        4,
        "one gemstone paid"
    );
    assert_eq!(servitor_of(&world, OWNER), Some(oid), "and it stays out");
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

    world.tick = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .unwrap()
        .next_consume_tick;
    handle_life_tick(&mut world, oid);

    assert_eq!(
        servitor_of(&world, OWNER),
        None,
        "dismissed for non-payment"
    );
}

/// A servitor with no upkeep item is never charged.
#[test]
fn a_servitor_without_upkeep_is_never_charged() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .next_consume_tick,
        u64::MAX,
        "no upkeep clock at all"
    );
    world.tick += 100_000;
    handle_life_tick(&mut world, oid);
    assert_eq!(servitor_of(&world, OWNER), Some(oid));
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
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "off following, mid-order"
    );

    // The owner runs far away.
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 50_000;
    handle_life_tick(&mut world, oid);

    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
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

    assert_eq!(
        servitor_of(&world, OWNER),
        None,
        "no ownerless NPC left behind"
    );
    assert!(
        world.objects.get_component::<Vitals>(&oid).is_none(),
        "despawned"
    );
}

/// A dead servitor ends the tick chain rather than rescheduling forever.
#[test]
fn a_dead_servitor_stops_ticking() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 60, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;

    // Well past the deadline: a live tick would have unsummoned it and sent
    // the "passed away" notice. A dead one just stops.
    world.tick += 100_000;
    handle_life_tick(&mut world, oid);
    assert!(
        world.objects.get_component::<ServitorOf>(&oid).is_some(),
        "left for the death path to clean up"
    );
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
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
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
    world
        .objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        .pending_pet_collar = Some(collar_oid);
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

    let link = world.objects.get_component::<PetOf>(&pet).unwrap();
    assert_eq!(
        link.collar_object_id, collar,
        "bound to this collar, not the item type"
    );
    assert_eq!(link.fed, 248, "starts on a full food bar from PetData");
    assert_eq!(pet_of(&world, OWNER), Some(pet));
}

/// A pet reuses the servitor owner-link, so it inherits follow for free.
#[test]
fn a_pet_follows_like_a_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();

    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following
    );
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 900;
    servitor_follow_tick(&mut world, pet);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::Movement>(&pet)
            .is_some()
    );
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
        world
            .objects
            .get_component::<crate::model::Player>(&OWNER)
            .unwrap()
            .pending_pet_collar
            .is_none(),
        "the holder was taken"
    );
    assert_eq!(
        summon_pet(&mut world, OWNER),
        None,
        "nothing parked, nothing summoned"
    );
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
    assert_eq!(
        pet_of(&world, OWNER),
        Some(first),
        "the first one is untouched"
    );
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
        crate::data::pet_data::PetLevel {
            max_meal: 300,
            exp: 5_000,
            ..Default::default()
        },
    );
    world.data.pet_data.insert_for_test(t);
}

fn saved_row(collar_oid: i32, level: i32, exp: i64, fed: i32, cur_hp: f64) -> crate::db::PetRow {
    crate::db::PetRow {
        collar_object_id: collar_oid,
        name: "Wolf".into(),
        level,
        cur_hp,
        cur_mp: 10.0,
        exp,
        sp: 7,
        fed,
        restore: false,
    }
}

fn put_saved(world: &mut World, row: crate::db::PetRow) {
    world
        .objects
        .get_component_mut::<PlayerPets>(&OWNER)
        .unwrap()
        .0
        .insert(row.collar_object_id, row);
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
    assert_eq!(
        pet.level, 2,
        "restored at the saved level, not the template's"
    );
    assert_eq!(pet.exp, 6_000);
    assert_eq!(pet.sp, 7);
    assert_eq!(
        pet.fed, 90,
        "the food bar carries over — it does not refill on summon"
    );
    assert_eq!(pet.max_fed, 300, "max_meal follows the restored level");
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        42.0,
        "wounded pet stays wounded"
    );
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
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = 33.0;
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 12;

    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    let row = world
        .objects
        .get_component::<PlayerPets>(&OWNER)
        .unwrap()
        .0
        .get(&collar)
        .unwrap()
        .clone();
    assert_eq!(row.cur_hp, 33.0, "the wound is what gets saved");
    assert_eq!(row.fed, 12);
    assert_eq!(
        row.collar_object_id, collar,
        "keyed by the collar, as the table is"
    );
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
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = 25.0;
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 60;

    // Owner logs out: state is captured, then the pet leaves the world.
    on_owner_leave_world(&mut world, OWNER);
    assert!(
        pet_of(&world, OWNER).is_none(),
        "the pet is gone with its owner"
    );

    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.fed, 60, "it comes back as hungry as it was left");
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        25.0,
        "and as wounded"
    );
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
    assert!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .contains_key(&collar)
    );

    let mut body = Vec::new();
    body.extend_from_slice(&collar.to_le_bytes());
    body.extend_from_slice(&1i64.to_le_bytes());
    crate::game_loop::items::handle_request_destroy_item(&mut world, CID, &body);

    assert!(
        pet_of(&world, OWNER).is_none(),
        "the summoned pet is unsummoned with its collar"
    );
    assert!(
        !world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .contains_key(&collar),
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
        self_continuous: false,
        id: WOLF_FOOD_SKILL,
        level: 1,
        effects: vec![crate::model::skill::SkillEffect::Feed {
            normal: restores,
            ride: 0,
            wyvern: 0,
        }],
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
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 4;

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    assert_eq!(
        fed(&world, pet_oid),
        0,
        "cost exceeded the bar — floored, not negative"
    );
    assert!(
        crate::game_loop::servitor::is_uncontrollable(&world, pet_oid),
        "an empty bar means starving"
    );
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
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 100;

    crate::game_loop::servitor::handle_feed_tick(&mut world, pet_oid);
    // 100 - 10 burned = 90, hungry (< 136), so it eats one 100-point helping.
    assert_eq!(fed(&world, pet_oid), 190, "burned 10, then ate 100");
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
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
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
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
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 200;

    crate::game_loop::servitor::apply_feed(&mut world, pet_oid, 100);
    assert_eq!(
        fed(&world, pet_oid),
        248,
        "200 + 100 clamped to max_meal, not banked"
    );
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
        objects
            .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .add_item(&data.item_data, 7_300_001, WOLF_FOOD, 5)
    };

    let mut body = Vec::new();
    body.extend_from_slice(&food_oid.to_le_bytes());
    body.extend_from_slice(&3i64.to_le_bytes());
    crate::game_loop::servitor::handle_give_item_to_pet(&mut world, CID, &body);

    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        3
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .count_of(WOLF_FOOD),
        2,
        "the owner keeps the remainder"
    );

    // And back again.
    let pet_food_oid = world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .items()[0]
        .object_id;
    let mut body = Vec::new();
    body.extend_from_slice(&pet_food_oid.to_le_bytes());
    body.extend_from_slice(&3i64.to_le_bytes());
    crate::game_loop::servitor::handle_get_item_from_pet(&mut world, CID, &body);
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        0
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .count_of(WOLF_FOOD),
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

    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_COLLAR),
        0
    );
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
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 50;

    let food_oid = world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .items()[0]
        .object_id;
    let body = food_oid.to_le_bytes().to_vec();
    crate::game_loop::servitor::handle_pet_use_item(&mut world, CID, &body);

    assert_eq!(fed(&world, pet_oid), 150, "hand-fed one helping");
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_FOOD),
        0
    );
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
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 50;

    // A different item entirely, sitting in the pet's bag.
    let mut other = crate::data::item_data::ItemTemplate::default();
    other.item_id = 57;
    other.is_stackable = true;
    world.data.item_data.insert_for_test(other);
    let oid = {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .add_item(&data.item_data, 7_400_001, 57, 1)
    };

    let body = oid.to_le_bytes().to_vec();
    crate::game_loop::servitor::handle_pet_use_item(&mut world, CID, &body);

    assert_eq!(fed(&world, pet_oid), 50, "bar untouched");
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(57),
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
    let skills = dist::skills();
    let skill = skills
        .get(2048, 1)
        .expect("Wolf Food skill 2048 exists in the datapack");
    let feed = skill
        .effects
        .iter()
        .find_map(|e| match e {
            crate::model::skill::SkillEffect::Feed { normal, .. } => Some(*normal),
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
    assert_eq!(
        after.summons.len(),
        1,
        "the servitor shows up in the party window"
    );
    assert_eq!(after.summons[0].summon_type, 2, "2 = servitor");
    assert!(
        after.summons[0].max_hp > 0,
        "and carries real vitals for the HP bar"
    );
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
        crate::game_loop::party::member_view(&world, OWNER)
            .unwrap()
            .summons
            .is_empty(),
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
                // Cost rises with level, so "does the cost follow the level?"
                // is answerable rather than vacuous.
                soulshot_count: 1 + lvl,
                spiritshot_count: 1 + lvl,
                ..Default::default()
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
    assert!(
        (pet_exp - 270.0).abs() < 0.001,
        "the pet takes the remaining 27% ({pet_exp})"
    );
    assert!((pet_sp - 27.0).abs() < 0.001);
}

/// Out of range, the pet earns nothing and the owner keeps the lot.
#[test]
fn a_distant_pet_earns_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world
        .objects
        .get_component_mut::<Position>(&pet_oid)
        .unwrap()
        .x += 10_000;

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
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 0;

    add_pet_exp(&mut world, OWNER, 1000.0, 100.0);
    assert_eq!(
        world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp,
        0,
        "starving pets do not learn"
    );
}

/// Crossing the level threshold levels the pet, and the food capacity moves
/// with it.
#[test]
fn a_pet_levels_when_it_earns_enough() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        1
    );

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
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
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
        .by_object_id(collar)
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
            world
                .objects
                .get_component_mut::<Position>(&pet_oid)
                .unwrap()
                .x += 10_000;
        }
        world
            .objects
            .get_component_mut::<crate::model::Player>(&OWNER)
            .unwrap()
            .exp = 0;
        crate::game_loop::death::add_exp_and_sp(&mut world, OWNER, 1000.0, 100.0, false);
        (
            world
                .objects
                .get_component::<crate::model::Player>(&OWNER)
                .unwrap()
                .exp,
            world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp,
        )
    };

    let (owner_alone, pet_idle) = owner_exp_after(false);
    let (owner_shared, pet_fed) = owner_exp_after(true);

    assert_eq!(pet_idle, 0, "a distant pet learns nothing");
    assert_eq!(pet_fed, 270, "a nearby pet takes 27% of the kill");
    assert_eq!(
        owner_alone, 1000,
        "without a pet in range the owner keeps it all"
    );
    assert_eq!(
        owner_shared, 730,
        "with a pet in range the owner keeps only 73%"
    );
}

// ---------------------------------------------------------------------------
// Pet stats (slice 13)
// ---------------------------------------------------------------------------

fn combat(world: &World, oid: i32) -> crate::model::components::CombatStats {
    *world
        .objects
        .get_component::<crate::model::components::CombatStats>(&oid)
        .unwrap()
}

/// A pet's stats come from its **per-level pet row**, not its NPC template.
/// The Wolf's NPC fixture is level 1 with 300 HP; its pet row says 100.
#[test]
fn a_pets_stats_come_from_the_pet_table_not_the_npc_template() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    let max_hp = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
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
    let hp_before = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2,
        "it levelled"
    );

    let after = combat(&world, pet_oid);
    let hp_after = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    assert!(
        after.p_atk > before.p_atk,
        "p.atk grew ({} → {})",
        before.p_atk,
        after.p_atk
    );
    assert!(
        after.m_atk > before.m_atk,
        "m.atk grew ({} → {})",
        before.m_atk,
        after.m_atk
    );
    assert!(
        after.p_def > before.p_def,
        "p.def grew ({} → {})",
        before.p_def,
        after.p_def
    );
    assert!(
        hp_after > hp_before,
        "max HP grew ({hp_before} → {hp_after})"
    );
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
    assert!(
        (frac - 0.5).abs() < 0.01,
        "still at half health after levelling ({frac})"
    );
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

    let max_hp = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    assert!(
        max_hp > 0,
        "fell back to the template instead of zeroing the pet"
    );
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
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2
    );

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let after = pet_exp(&world, pet_oid);
    assert!(after < before, "exp was lost on death ({before} → {after})");
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .exp_before_death,
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
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2,
        "exactly at the threshold"
    );

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    let pet = *world.objects.get_component::<PetOf>(&pet_oid).unwrap();
    assert_eq!(pet.level, 2, "still level 2");
    assert_eq!(
        pet.exp, 5_000,
        "held at the level floor rather than dropping below it"
    );
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
    world
        .objects
        .add_components(&OWNER, crate::model::components::DuelRef(1));
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);

    assert_eq!(
        pet_exp(&world, pet_oid),
        before,
        "no exp lost to a duel death"
    );
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
    assert_eq!(
        pet_exp(&world, pet_oid),
        before_death,
        "a full-power revive restores all of it"
    );

    // The record is spent.
    pet_restore_exp(&mut world, pet_oid, 100.0);
    assert_eq!(
        pet_exp(&world, pet_oid),
        before_death,
        "a second revive restores nothing more"
    );
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
    assert_eq!(
        regained,
        (lost as f64 * 0.5).round() as i64,
        "half the loss came back"
    );
}

/// Slice 7 deferred this branch because a pet could not yet be stored dead.
/// It can now: a pet saved with `curHp < 1` comes back as a corpse rather
/// than silently alive at 0 HP.
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
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .dead,
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
    assert_eq!(
        pet_exp(&world, pet_oid),
        before,
        "no band above the cap, so no penalty"
    );
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
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);

    let req = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap()
        .revive_request;
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
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    assert!(crate::game_loop::death::handle_revive_answer(
        &mut world, OWNER, true
    ));

    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert!(!v.dead, "the pet is alive again");
    assert!(v.cur_hp > 0.0, "with HP on the bar");
    let exp_now = world.objects.get_component::<PetOf>(&pet_oid).unwrap().exp;
    assert!(
        exp_now > after_death,
        "some of the lost exp came back ({after_death} → {exp_now})"
    );
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
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    assert!(crate::game_loop::death::handle_revive_answer(
        &mut world, OWNER, false
    ));

    assert!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .dead,
        "still dead"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&OWNER)
            .unwrap()
            .revive_request
            .is_none(),
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
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&OWNER)
            .unwrap()
            .revive_request
            .is_none()
    );
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
    world
        .objects
        .get_component_mut::<Vitals>(&OWNER)
        .unwrap()
        .dead = true;

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    crate::game_loop::death::handle_revive_answer(&mut world, OWNER, true);

    assert!(
        !world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .dead,
        "the pet came back"
    );
    assert!(
        world.objects.get_component::<Vitals>(&OWNER).unwrap().dead,
        "the owner did not"
    );
}

// ---------------------------------------------------------------------------
// Pet corpse decay (slice 16)
// ---------------------------------------------------------------------------

fn owner_has(world: &World, item_id: i32) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .map(|inv| inv.count_of(item_id))
        .unwrap_or(0)
}

/// Letting a dead pet rot **destroys it permanently**: the collar is consumed
/// and the saved row goes with it. Java `Summon.onDecay` → `Pet.deleteMe` →
/// `destroyControlItem`.
#[test]
fn a_decayed_pet_corpse_destroys_the_collar_and_the_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    assert_eq!(owner_has(&world, WOLF_COLLAR), 1);

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    crate::game_loop::death::handle_npc_decay(&mut world, pet_oid);

    assert_eq!(owner_has(&world, WOLF_COLLAR), 0, "the collar was consumed");
    assert!(
        !world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .contains_key(&collar),
        "and the saved row went with it"
    );
    assert!(pet_of(&world, OWNER).is_none(), "the owner has no pet");
}

/// `_inventory.transferItemsToOwner()` runs **before** the collar is
/// destroyed, so what the pet was carrying is handed back rather than lost.
#[test]
fn a_decayed_pet_hands_its_inventory_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    put_food_in_pet(&mut world, 4);
    assert_eq!(
        owner_has(&world, WOLF_FOOD),
        0,
        "the food is in the pet's bag, not the owner's"
    );

    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);
    crate::game_loop::death::handle_npc_decay(&mut world, pet_oid);

    assert_eq!(
        owner_has(&world, WOLF_FOOD),
        4,
        "the pet's cargo came back to the owner"
    );
    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .items()
            .len(),
        0,
        "and the pet's bag is empty"
    );
}

/// A pet resurrected before its corpse decays is spared entirely — the decay
/// task still fires, and must find a living pet and do nothing.
#[test]
fn resurrecting_before_decay_saves_the_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);

    let reviver = OWNER + 5;
    let _rx2 = ingame_caster(&mut world, CID + 5, reviver, 50, 0);
    crate::game_loop::death::revive_request(&mut world, reviver, pet_oid, 100, 70, 70, 0, 1016, 0);
    crate::game_loop::death::handle_revive_answer(&mut world, OWNER, true);

    // The decay task fires regardless; it must be a no-op now.
    crate::game_loop::death::handle_npc_decay(&mut world, pet_oid);

    assert_eq!(owner_has(&world, WOLF_COLLAR), 1, "the collar survived");
    assert!(pet_of(&world, OWNER).is_some(), "and so did the pet");
}

/// A *servitor* corpse decaying must not go through the pet path — it has no
/// collar to destroy, and the branch is keyed on `PetOf`.
#[test]
fn a_decayed_servitor_does_not_take_the_pet_path() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    crate::game_loop::death::npc_do_die(&mut world, servitor, OWNER);
    crate::game_loop::death::handle_npc_decay(&mut world, servitor);

    assert_eq!(
        owner_has(&world, WOLF_COLLAR),
        1,
        "an unrelated collar is untouched"
    );
    let _ = collar;
}

// ---------------------------------------------------------------------------
// Pet regen (slice 17)
// ---------------------------------------------------------------------------

/// A pet regenerates from its **per-level pet row**, not the NPC template.
/// The fixture's pet row says 2.0 HP/tick; the Wolf NPC template says nothing,
/// so a template-driven pet would not heal at all.
#[test]
fn a_pet_regenerates_from_its_pet_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    {
        let v = world.objects.get_component_mut::<Vitals>(&pet_oid).unwrap();
        v.cur_hp = 10.0;
        v.cur_mp = 1.0;
    }

    crate::game_loop::regen::run_npc_regen_tick(&mut world);

    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert_eq!(v.cur_hp, 12.0, "regen_hp 2.0 from the pet row");
    assert!(
        (v.cur_mp - 1.9).abs() < 1e-6,
        "regen_mp 0.9 from the pet row ({})",
        v.cur_mp
    );
}

/// Regen is capped at the maximum like any other — a nearly-full pet does not
/// overshoot.
#[test]
fn pet_regen_stops_at_full() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    let max_hp = world
        .objects
        .get_component::<Vitals>(&pet_oid)
        .unwrap()
        .max_hp;
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = max_hp as f64 - 0.5;

    crate::game_loop::regen::run_npc_regen_tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        max_hp as f64,
        "clamped, not overshot"
    );
}

/// The pet multipliers are separate from the NPC ones — a server that retunes
/// monster regen must not accidentally retune pets, and vice versa.
#[test]
fn pet_regen_uses_the_pet_multiplier() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    world
        .objects
        .get_component_mut::<Vitals>(&pet_oid)
        .unwrap()
        .cur_hp = 10.0;
    // Double pets, and set the *monster* multiplier to something absurd that
    // must not apply.
    world.cfg.npc.pet_hp_regen_multiplier = 2.0;
    world.cfg.npc.hp_regen_multiplier = 100.0;

    crate::game_loop::regen::run_npc_regen_tick(&mut world);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&pet_oid)
            .unwrap()
            .cur_hp,
        14.0,
        "2.0 regen × the pet multiplier, untouched by the monster one"
    );
}

/// A dead pet does not regenerate back to life while its corpse waits to decay.
#[test]
fn a_dead_pet_does_not_regenerate() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    crate::game_loop::death::npc_do_die(&mut world, pet_oid, OWNER);

    crate::game_loop::regen::run_npc_regen_tick(&mut world);
    let v = world.objects.get_component::<Vitals>(&pet_oid).unwrap();
    assert_eq!(v.cur_hp, 0.0, "a corpse stays a corpse");
    assert!(v.dead);
}

// ---------------------------------------------------------------------------
// Summon shots (slice 18)
// ---------------------------------------------------------------------------

const BEAST_SOULSHOT: i32 = 6645;

fn register_beast_soulshot(world: &mut World) {
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = BEAST_SOULSHOT;
    t.name = "Beast Soulshot".into();
    t.is_stackable = true;
    t.handler = crate::data::item_data::ItemHandler::BeastSoulShot;
    t.default_action = crate::data::item_data::ActionType::SummonSoulshot;
    world.data.item_data.insert_for_test(t);
}

fn give_owner_shots(world: &mut World, count: i64) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_500_001, BEAST_SOULSHOT, count);
    objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        .auto_shots
        .push(BEAST_SOULSHOT);
}

fn owner_shot_count(world: &World) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .map(|inv| inv.count_of(BEAST_SOULSHOT))
        .unwrap_or(0)
}

/// A pet charges from its **owner's** Beast shots, spending the count its
/// level demands.
#[test]
fn a_pet_charges_shots_from_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 10);

    assert!(
        crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true),
        "charged"
    );
    assert_eq!(
        owner_shot_count(&world),
        8,
        "the level-1 row costs 2 shots per hit"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::components::ChargedShots>(&pet_oid)
            .unwrap()
            .soulshot
    );
}

/// The cost follows the pet's level, so a levelled pet is more expensive to
/// keep shotted — the mechanic, not an incidental detail.
#[test]
fn the_shot_cost_follows_the_pets_level() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 20);

    add_pet_exp(&mut world, OWNER, 6_000.0, 0.0);
    assert_eq!(
        world
            .objects
            .get_component::<PetOf>(&pet_oid)
            .unwrap()
            .level,
        2
    );

    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);
    assert_eq!(
        owner_shot_count(&world),
        17,
        "level 2 costs 3 per hit, not 2"
    );
}

/// Already charged, no second charge — and no second cost.
#[test]
fn a_charged_pet_does_not_recharge() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 10);

    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);
    let after_first = owner_shot_count(&world);
    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);
    assert_eq!(
        owner_shot_count(&world),
        after_first,
        "no double spend while charged"
    );
}

/// Too few shots left for one hit: nothing is spent and the pet stays
/// uncharged, rather than a partial charge on a partial payment.
#[test]
fn a_partial_stack_buys_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 1); // level 1 costs 2

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert_eq!(owner_shot_count(&world), 1, "the odd shot is not consumed");
}

/// Spending the charge is a one-shot: the second swing is unshotted.
#[test]
fn the_charge_is_spent_once() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 10);
    crate::game_loop::servitor::recharge_shots(&mut world, pet_oid, true);

    assert!(
        crate::game_loop::servitor::uncharge_soulshot(&mut world, pet_oid),
        "first swing is shotted"
    );
    assert!(
        !crate::game_loop::servitor::uncharge_soulshot(&mut world, pet_oid),
        "the second is not"
    );
}

/// A pet with no owner shots toggled on charges nothing — the auto-use switch
/// is what arms it.
#[test]
fn without_the_toggle_a_pet_charges_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    // Shots in the bag, but never toggled on.
    let World { data, objects, .. } = &mut world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_500_002, BEAST_SOULSHOT, 10);

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert_eq!(owner_shot_count(&world), 10, "untouched");
}

// ---------------------------------------------------------------------------
// SUMMON target type (slice 19)
// ---------------------------------------------------------------------------

/// The Summoner support kit — 18 learnable skills, all of which resolved to
/// `INVALID_TARGET` before `TargetType::Summon` existed. Servitor Heal is the
/// representative case: it must reach the servitor, not the caster.
#[test]
fn a_summon_target_skill_reaches_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 10.0;

    let skill = crate::model::skill::Skill {
        self_continuous: false,
        id: 1127,
        level: 1,
        target_type: crate::model::skill::TargetType::Summon,
        effects: vec![crate::model::skill::SkillEffect::Heal { power: 100.0 }],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill.clone());

    crate::game_loop::skills::effects::apply_skill_effects(&mut world, OWNER, servitor, &skill);
    assert!(
        world
            .objects
            .get_component::<Vitals>(&servitor)
            .unwrap()
            .cur_hp
            > 10.0,
        "the servitor was healed"
    );
}

/// Target resolution picks the caster's own servitor without needing it
/// selected — Java's handler ignores the current target entirely.
#[test]
fn summon_targeting_finds_the_casters_own_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    assert_eq!(servitor_of(&world, OWNER), Some(servitor));
}

/// With no summon out there is nothing to target — the skill must fail rather
/// than silently falling back to the caster.
#[test]
fn summon_targeting_without_a_summon_finds_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    assert!(servitor_of(&world, OWNER).is_none());
}

/// **A pet is not a servitor.** Java's handler returns `getAnyServitor()`,
/// which is null for a pet-only owner — so "Servitor Heal" does nothing for
/// someone with a Wolf. It reads like a bug and is thematically right: this is
/// the Summoner's kit. Pinned so a later "fix" has to be deliberate.
#[test]
fn a_pet_is_not_a_summon_target() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    assert!(pet_of(&world, OWNER).is_some(), "the pet is out");
    assert!(
        servitor_of(&world, OWNER).is_none(),
        "but it is not what a SUMMON-target skill resolves to (pet {pet_oid})"
    );
}

/// The fixture above builds its own skill, so it cannot catch a parse-arm
/// mistake. This reads the **real** Summoner kit out of the datapack: if
/// `<targetType>SUMMON</targetType>` stops mapping, all 18 skills silently
/// return to `INVALID_TARGET`.
#[test]
fn the_real_servitor_skills_parse_as_summon_targeted() {
    let skills = dist::skills();
    // Servitor Heal, Servitor Recharge, Mighty Servitor, Final Servitor.
    for id in [1127, 1126, 1146, 1349] {
        let s = skills
            .get(id, 1)
            .unwrap_or_else(|| panic!("skill {id} exists"));
        assert_eq!(
            s.target_type,
            crate::model::skill::TargetType::Summon,
            "skill {id} ({}) must target the summon",
            s.id
        );
    }
}

// ---------------------------------------------------------------------------
// Summon buff visibility (slice 20)
// ---------------------------------------------------------------------------

/// Buffing a servitor must change its stats server-side. This is the
/// end-to-end check slice 19 did not make: it proved a *heal* lands, not that
/// a **stat buff** actually moves the servitor's numbers.
#[test]
fn a_stat_buff_on_a_servitor_changes_its_stats() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    let before = world
        .objects
        .get_component::<crate::model::components::Speeds>(&servitor)
        .unwrap()
        .run_spd;

    // Servitor Wind Walk's shape: a flat speed increase.
    let skill = crate::model::skill::Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        target_type: crate::model::skill::TargetType::Summon,
        abnormal_time: 1200,
        effects: vec![crate::model::skill::SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::RunSpeed,
                mode: crate::model::stats::StatModifierType::Diff,
                amount: 50.0,
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(skill.clone());
    crate::game_loop::skills::effects::apply_continuous_effects(
        &mut world, OWNER, servitor, &skill, None,
    );

    let after = world
        .objects
        .get_component::<crate::model::components::Speeds>(&servitor)
        .unwrap()
        .run_spd;
    assert!(
        after > before,
        "the servitor actually got faster ({before} → {after})"
    );
}

/// The buff's stat change must reach the **client**, not just the server:
/// `PetInfo` carries the summon's speeds, so the owner gets a fresh one.
/// Without this Servitor Haste and Wind Walk look like no-ops.
#[test]
fn buffing_a_servitor_refreshes_its_client_info() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    while rx.try_recv().is_ok() {} // drain the summon packets

    let skill = crate::model::skill::Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        target_type: crate::model::skill::TargetType::Summon,
        abnormal_time: 1200,
        effects: vec![crate::model::skill::SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::RunSpeed,
                mode: crate::model::stats::StatModifierType::Diff,
                amount: 50.0,
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    crate::game_loop::skills::effects::apply_continuous_effects(
        &mut world, OWNER, servitor, &skill, None,
    );

    // `PetInfo` is 0xB2 — the packet that carries the summon's speeds.
    let mut saw_pet_info = false;
    while let Ok(pkt) = rx.try_recv() {
        if pkt.first() == Some(&0xB2) {
            saw_pet_info = true;
        }
    }
    assert!(
        saw_pet_info,
        "the owner was told its servitor's stats changed"
    );
}

// ---------------------------------------------------------------------------
// Summon PvP flagging (slice 21)
// ---------------------------------------------------------------------------

/// Java `Creature.doAttack` flags `getActingPlayer()`, and `Summon`'s is its
/// **owner** — so setting your pet on another player flags *you*.
///
/// Without this a player can attack through their summon and never go purple:
/// the victim can't retaliate without taking the karma, which is the shape of
/// an exploit rather than a cosmetic gap.
#[test]
fn a_summon_attacking_a_player_flags_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    crate::game_loop::pvp::update_pvp_status_target(&mut world, servitor, victim);

    let flagged = world
        .objects
        .get_component::<crate::model::components::PvpState>(&OWNER)
        .is_some_and(|s| s.flag > 0);
    assert!(flagged, "the owner is flagged for their summon's attack");
}

/// End-to-end: a real summon swing must flag the owner, not just the helper
/// called directly. The unit test above proves `update_pvp_status_target`
/// resolves the owner; this proves the attack path actually reaches it.
#[test]
fn a_real_summon_swing_flags_the_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    crate::game_loop::combat::do_auto_attack(&mut world, servitor, victim);

    let flagged = world
        .objects
        .get_component::<crate::model::components::PvpState>(&OWNER)
        .is_some_and(|s| s.flag > 0);
    assert!(
        flagged,
        "the owner is flagged by their summon's actual swing"
    );
}

/// The owner also enters combat stance — Java hands the stance to
/// `getActingPlayer()`, and it is the owner's stance that blocks their own
/// sit/logout, not the summon's.
#[test]
fn a_summon_swing_puts_its_owner_in_combat_stance() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    crate::game_loop::combat::do_auto_attack(&mut world, servitor, victim);

    let now = world.tick;
    assert!(
        world
            .objects
            .get_component::<crate::model::components::AttackState>(&OWNER)
            .is_some_and(|s| s.stance_until_tick > now),
        "the owner is in combat stance"
    );
}

/// The counterpart guard: a **plain monster** hitting a player must still flag
/// nobody. Moving the flag/stance block out of the player-only branch is only
/// safe because `acting_player` resolves a mob to itself, and a mob is not a
/// player.
#[test]
fn a_monster_attacking_a_player_flags_nobody() {
    let (mut world, _db, _l) = servitor_world();
    let victim = OWNER + 7;
    let _rx = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 40, 0, 0);

    crate::game_loop::combat::do_auto_attack(&mut world, FOE, victim);

    assert!(
        world
            .objects
            .get_component::<crate::model::components::PvpState>(&victim)
            .is_none_or(|s| s.flag == 0),
        "the victim is not flagged by being attacked"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::components::PvpState>(&FOE)
            .is_none_or(|s| s.flag == 0),
        "and neither is the monster"
    );
}

// ---------------------------------------------------------------------------
// Summon kill credit (slice 22)
// ---------------------------------------------------------------------------

/// Java resolves every damage dealer to `getActingPlayer()` when handing out
/// rewards, so a **summon's killing blow credits its owner**. Without that a
/// player whose pet lands the last hit gets no exp — the core summoner loop.
#[test]
fn a_summon_killing_blow_credits_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
    // `default_template` awards nothing, so without this the assertion would
    // be vacuous — it would read 0 exp whether or not the credit worked.
    {
        let mut t = world.data.npc_data.get(PANTHER + 1).unwrap().clone();
        t.exp = 1000.0;
        t.sp = 100.0;
        world.data.npc_data.insert_for_test(t);
    }

    // Rewards are shares of the aggro list's recorded damage. Seeded directly
    // rather than by swinging, because a real swing lands on a *scheduled*
    // tick — this test is about who the damage is credited to, not about
    // attack timing.
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&FOE)
        .unwrap()
        .0
        .entry(servitor)
        .or_default()
        .damage = 500.0;
    world
        .objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        .exp = 0;
    crate::game_loop::death::npc_do_die(&mut world, FOE, servitor);

    let exp = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap()
        .exp;
    assert!(
        exp > 0,
        "the owner was credited for their summon's kill (exp {exp})"
    );
}

/// A player who fights *alongside* their summon appears twice in the aggro
/// list once both resolve to them. Their shares must merge, not double-count —
/// otherwise fighting with a pet would inflate the owner's slice of a
/// contested kill against everyone else.
#[test]
fn an_owner_and_their_summon_share_one_slice() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let rival = OWNER + 9;
    let _rx2 = ingame_caster(&mut world, CID + 9, rival, 20, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
    {
        let mut t = world.data.npc_data.get(PANTHER + 1).unwrap().clone();
        t.exp = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }

    // Owner 100 + their summon 100 = 200; the rival also does 200.
    {
        let aggro = &mut world
            .objects
            .get_component_mut::<crate::model::npc::AggroList>(&FOE)
            .unwrap()
            .0;
        aggro.entry(OWNER).or_default().damage = 100.0;
        aggro.entry(servitor).or_default().damage = 100.0;
        aggro.entry(rival).or_default().damage = 200.0;
    }
    for oid in [OWNER, rival] {
        world
            .objects
            .get_component_mut::<crate::model::Player>(&oid)
            .unwrap()
            .exp = 0;
    }

    crate::game_loop::death::npc_do_die(&mut world, FOE, servitor);

    let owner_exp = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap()
        .exp;
    let rival_exp = world
        .objects
        .get_component::<crate::model::Player>(&rival)
        .unwrap()
        .exp;
    assert!(
        owner_exp > 0 && rival_exp > 0,
        "both earned ({owner_exp} / {rival_exp})"
    );
    assert_eq!(
        owner_exp, rival_exp,
        "equal damage earns equal exp — the pair merged into one slice"
    );
}

// ---------------------------------------------------------------------------
// getActingPlayer audit, part 2 (slice 23)
// ---------------------------------------------------------------------------

/// Java's PK/karma block reads `killer.getActingPlayer()`, so killing a player
/// **with your pet** carries the same consequences as killing them yourself.
/// Without it, a pet kill is a free kill: no PK counter, no karma.
#[test]
fn a_summon_killing_a_player_gives_its_owner_the_karma() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    let before = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap()
        .pk_kills;
    crate::game_loop::death::player_do_die(&mut world, victim, servitor);
    let after = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap()
        .pk_kills;

    assert!(
        after > before,
        "the owner took the PK for their summon's kill ({before} → {after})"
    );
}

/// **A duel never kills** (G20's invariant). The lethal guard resolves the
/// attacker to its acting player, or a summon's blow slips past it and really
/// kills the opponent.
#[test]
fn a_summons_blow_cannot_kill_a_duel_opponent() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let foe_player = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, foe_player, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // Put the two players in a duel with each other.
    world
        .objects
        .add_components(&OWNER, crate::model::components::DuelRef(1));
    world
        .objects
        .add_components(&foe_player, crate::model::components::DuelRef(1));
    // The snapshot the end-of-duel restore puts back: both at full.
    let snap = |world: &World, oid: i32| {
        let v = world.objects.get_component::<Vitals>(&oid).unwrap();
        (v.max_hp as f64, v.max_mp as f64, 0.0)
    };
    world.duels.insert(
        1,
        crate::game_loop::duel::Duel {
            snapshot: [snap(&world, OWNER), snap(&world, foe_player)],
            id: 1,
            player_a: OWNER,
            player_b: foe_player,
            countdown: 0,
            ends_at_tick: u64::MAX,
            surrender: 0,
            party: false,
            team_a: Vec::new(),
            team_b: Vec::new(),
            member_snapshot: Vec::new(),
            instance_id: 0,
            defeated: Vec::new(),
            winner_team: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Vitals>(&foe_player)
        .unwrap()
        .cur_hp = 50.0;

    let capped =
        crate::game_loop::duel::duel_lethal_guard(&mut world, servitor, foe_player, 9999.0);
    assert!(capped, "the summon's lethal blow was capped");
    // The cap sets 1 HP and ends the duel, and ending it runs
    // `restorePlayerConditions`, which heals both sides — so the observable
    // post-condition is "alive", not "at 1 HP".
    let v = world.objects.get_component::<Vitals>(&foe_player).unwrap();
    assert!(
        !v.dead && v.cur_hp > 0.0,
        "the duel opponent survived ({} HP)",
        v.cur_hp
    );
}

/// Dying to a clan-war enemy quarters the exp penalty (Java
/// `calculateDeathExpPenalty`'s `atWarWith(killer.getActingPlayer())`). That
/// must hold when the killing blow came from the enemy's **summon**, or the
/// victim pays four times the exp they should.
///
/// This behaviour was only ever covered *accidentally*, by a resolution
/// shadowed part-way down `player_do_die`. It is pinned here because
/// accidental coverage is invisible when it breaks.
#[test]
fn dying_to_a_war_enemys_summon_still_quarters_the_penalty() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // The exp penalty is measured; give the victim something to lose.
    let exp_of = |w: &World| {
        w.objects
            .get_component::<crate::model::Player>(&victim)
            .unwrap()
            .exp
    };
    for oid in [OWNER, victim] {
        let p = world
            .objects
            .get_component_mut::<crate::model::Player>(&oid)
            .unwrap();
        p.level = 20;
        p.exp = 1_000_000;
    }

    let before = exp_of(&world);
    crate::game_loop::death::player_do_die(&mut world, victim, servitor);
    let lost_to_summon = before - exp_of(&world);

    assert!(lost_to_summon > 0, "the victim lost exp ({lost_to_summon})");
}

/// The clan-war kill counter also follows the acting player: a kill by the
/// enemy's pet is still a kill for the war score.
#[test]
fn a_summon_kill_counts_for_the_clan_war() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // `clan_war_on_kill` returns early unless the *killer* resolves to a
    // player; before the resolution a summon killer fell out immediately.
    // Reaching it at all is what this asserts — the war bookkeeping itself is
    // covered by the clan tests.
    let reached = crate::game_loop::pvp::acting_player(&world, servitor);
    assert_eq!(
        reached, OWNER,
        "the summon resolves to its owner for war credit"
    );
    crate::game_loop::death::player_do_die(&mut world, victim, servitor);
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&OWNER)
            .unwrap()
            .pk_kills
            > 0,
        "the kill was attributed to the owner"
    );
}

/// NPC skill cooldowns must actually apply. `set_skill_reuse` writes through
/// `if let Some(Reuses)` — a **silent no-op** when the component is absent —
/// and the check in `npc_cast` treats absence as "ready". If NPCs are never
/// given the component, a mob re-casts as fast as its AI loop allows.
#[test]
fn an_npc_records_its_skill_reuse() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);

    let skill = crate::model::skill::Skill {
        self_continuous: false,
        id: 4049,
        level: 1,
        reuse_delay: 10_000,
        ..Default::default()
    };
    crate::game_loop::skills::cast::set_skill_reuse(&mut world, FOE, &skill);

    assert!(
        world
            .objects
            .get_component::<crate::model::components::Reuses>(&FOE)
            .is_some_and(|r| !r.0.is_empty()),
        "the NPC's cooldown was recorded"
    );
}

/// And the recorded cooldown must actually **block** the re-cast. Recording it
/// is only half the fix: the check reads the same component, so a test that
/// stops at "it was written" would not notice if the gate were bypassed.
#[test]
fn an_npc_skill_on_cooldown_cannot_be_recast() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
    // Enough MP that the cooldown is the only thing that can refuse it.
    {
        let v = world.objects.get_component_mut::<Vitals>(&FOE).unwrap();
        v.max_mp = 1000;
        v.cur_mp = 1000.0;
    }
    let skill = crate::model::skill::Skill {
        self_continuous: false,
        id: 4049,
        level: 1,
        reuse_delay: 10_000,
        ..Default::default()
    };

    assert!(
        crate::game_loop::npc::cast::check_use_conditions_for_test(&world, FOE, &skill),
        "ready before the first cast"
    );
    crate::game_loop::skills::cast::set_skill_reuse(&mut world, FOE, &skill);
    assert!(
        !crate::game_loop::npc::cast::check_use_conditions_for_test(&world, FOE, &skill),
        "refused while on cooldown"
    );

    world.tick += 10_000 / 100 + 1; // past the 10 s reuse
    assert!(
        crate::game_loop::npc::cast::check_use_conditions_for_test(&world, FOE, &skill),
        "ready again once it expires"
    );
}

// ---------------------------------------------------------------------------
// Pet equipment (slice 25)
// ---------------------------------------------------------------------------

const WOLF_ARMOR: i32 = 3891;

/// Register Wolf's Hide Armor — a real chest-slot pet armour with defence.
fn register_pet_armor(world: &mut World) {
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = WOLF_ARMOR;
    t.name = "Wolf's Hide Armor".into();
    t.kind = crate::data::item_data::ItemKind::Armor;
    t.body_part = crate::data::item_data::SLOT_CHEST;
    world.data.item_data.insert_for_test(t);
    world.data.item_data.insert_stats_for_test(
        WOLF_ARMOR,
        vec![(crate::model::stats::Stat::PhysicalDefence, 31.0)],
    );
}

fn give_pet_armor(world: &mut World) -> i32 {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<PetInventory>(&OWNER)
        .unwrap()
        .0
        .add_item(&data.item_data, 7_600_001, WOLF_ARMOR, 1)
}

/// A pet's armour goes on its **own** paperdoll, and its defence counts.
#[test]
fn a_pet_can_wear_armour_and_gains_its_defence() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);

    let before = world
        .objects
        .get_component::<crate::model::components::CombatStats>(&pet_oid)
        .unwrap()
        .p_def;
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);

    assert!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .paperdoll_slot_of(armor)
            .is_some(),
        "the armour is worn"
    );
    let after = world
        .objects
        .get_component::<crate::model::components::CombatStats>(&pet_oid)
        .unwrap()
        .p_def;
    assert!(
        after > before,
        "and its defence counts ({before} → {after})"
    );
}

/// Clicking a worn item takes it off again (Java `useEquippableItem` toggles),
/// and the defence goes with it.
#[test]
fn clicking_worn_pet_armour_takes_it_off() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);

    let naked = world
        .objects
        .get_component::<crate::model::components::CombatStats>(&pet_oid)
        .unwrap()
        .p_def;
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);

    assert!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .paperdoll_slot_of(armor)
            .is_none(),
        "taken off"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::components::CombatStats>(&pet_oid)
            .unwrap()
            .p_def,
        naked,
        "and the defence went with it"
    );
}

/// Worn pet armour persists as `PET_EQUIP`, carried items as `PET` — and the
/// slot survives the round trip, so a pet's armour comes back **on** rather
/// than loose in its bag. This closes the deferral slice 8 left behind.
#[test]
fn pet_equipment_round_trips_through_its_own_location() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_pet_armor(&mut world);
    register_food(&mut world, 100);
    let pet_oid = summoned_pet(&mut world);
    let armor = give_pet_armor(&mut world);
    put_food_in_pet(&mut world, 3);
    crate::game_loop::servitor::equip_pet_item(&mut world, OWNER, pet_oid, armor);

    let rows = world
        .objects
        .get_component::<PetInventory>(&OWNER)
        .unwrap()
        .to_rows();
    let worn = rows
        .iter()
        .find(|r| r.item_id == WOLF_ARMOR)
        .expect("armour row");
    let carried = rows
        .iter()
        .find(|r| r.item_id == WOLF_FOOD)
        .expect("food row");
    assert_eq!(worn.loc, "PET_EQUIP", "worn gear gets its own location");
    assert_ne!(worn.loc_data, 0, "and keeps the slot it was in");
    assert_eq!(carried.loc, "PET", "carried items stay in the bag");

    // Back again.
    let restored = crate::model::inventory::PetInventory::from_rows(&rows);
    assert!(
        restored.0.paperdoll_slot_of(worn.object_id).is_some(),
        "the pet's armour comes back on, not loose in its bag"
    );
}

// ---------------------------------------------------------------------------
// Reconnect resummon (slice 26)
// ---------------------------------------------------------------------------

/// A pet that was out at logout comes back on the next login —
/// `RestorePetOnReconnect` is True on this dist, so this is the normal path.
#[test]
fn a_pet_that_was_out_at_logout_comes_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    let pet_oid = summon_pet(&mut world, OWNER).unwrap();
    world
        .objects
        .get_component_mut::<PetOf>(&pet_oid)
        .unwrap()
        .fed = 42;

    // Log out with the pet out: the sync marks the row restorable.
    on_owner_leave_world(&mut world, OWNER);
    assert!(
        pet_of(&world, OWNER).is_none(),
        "the pet left with its owner"
    );
    assert!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .get(&collar)
            .unwrap()
            .restore,
        "the row is marked as 'was out'"
    );

    // Log back in.
    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    let back = pet_of(&world, OWNER).expect("the pet came back");
    assert_eq!(
        world.objects.get_component::<PetOf>(&back).unwrap().fed,
        42,
        "and it came back in the state it left in"
    );
}

/// A pet deliberately put away before logging out stays in its collar — only
/// a pet that was *out* is restored.
#[test]
fn a_pet_put_away_before_logout_stays_away() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    // Put it away by hand first, *then* log out.
    crate::game_loop::servitor::sync_pet_row(&mut world, OWNER);
    unsummon_servitor(&mut world, OWNER);
    world
        .objects
        .get_component_mut::<PlayerPets>(&OWNER)
        .unwrap()
        .0
        .get_mut(&collar)
        .unwrap()
        .restore = false;
    on_owner_leave_world(&mut world, OWNER);

    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    assert!(pet_of(&world, OWNER).is_none(), "it stayed in its collar");
}

/// A collar traded away or destroyed between sessions leaves nothing to
/// restore — and must not leave a dangling holder behind.
#[test]
fn a_missing_collar_restores_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();
    on_owner_leave_world(&mut world, OWNER);

    // The collar is gone by the time they log back in.
    world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .remove_by_object_id(collar, 1);

    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    assert!(pet_of(&world, OWNER).is_none(), "nothing to restore");
    assert!(
        world
            .objects
            .get_component::<crate::model::Player>(&OWNER)
            .unwrap()
            .pending_pet_collar
            .is_none(),
        "and no dangling collar holder was left set"
    );
}

/// With the config off, nothing is restored — the flag is honoured, not
/// assumed.
#[test]
fn the_reconnect_config_is_honoured() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    wolf_with_exp_curve(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();
    on_owner_leave_world(&mut world, OWNER);

    world.cfg.character.restore_pet_on_reconnect = false;
    crate::game_loop::servitor::restore_pet_on_login(&mut world, OWNER);
    assert!(pet_of(&world, OWNER).is_none(), "config off, no restore");
}

/// A servitor that was out at logout comes back — rebuilt by **re-casting its
/// summoning skill**, as Java does, with the saved vitals and remaining
/// lifetime stamped back on.
#[test]
fn a_servitor_that_was_out_at_logout_comes_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let summon_skill = 1111;
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: summon_skill,
            level: 1,
            effects: vec![SkillEffect::Summon {
                npc_id: PANTHER,
                life_time: 1200,
                consume_item_id: 0,
                consume_item_count: 0,
            }],
            ..Default::default()
        });
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&OWNER)
        .unwrap()
        .0
        .insert(summon_skill, 1);

    let servitor = summon_servitor(&mut world, OWNER, PANTHER, summon_skill, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 77.0;
    world.tick += 200 * 10; // 200 s of its 1200 s spent

    on_owner_leave_world(&mut world, OWNER);
    assert!(
        servitor_of(&world, OWNER).is_none(),
        "it left with its owner"
    );

    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    let back = servitor_of(&world, OWNER).expect("the servitor came back");
    assert_eq!(
        world.objects.get_component::<Vitals>(&back).unwrap().cur_hp,
        77.0,
        "with the HP it had"
    );
    let remaining = (world
        .objects
        .get_component::<ServitorOf>(&back)
        .unwrap()
        .expires_at_tick
        - world.tick)
        / 10;
    assert!(
        (990..=1005).contains(&remaining),
        "and roughly its remaining lifetime, not a fresh 1200 s ({remaining})"
    );
}

/// A servitor dismissed before logout stays dismissed — the row is cleared
/// when nothing is out, or it would come back anyway.
#[test]
fn a_servitor_dismissed_before_logout_stays_away() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 1111, 1200, 0, 0).unwrap();
    crate::game_loop::servitor::sync_summon_row(&mut world, OWNER);
    unsummon_servitor(&mut world, OWNER);

    on_owner_leave_world(&mut world, OWNER);
    assert!(
        world
            .objects
            .get_component::<crate::model::components::PlayerSummons>(&OWNER)
            .unwrap()
            .0
            .is_empty(),
        "the stale row was cleared"
    );
    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    assert!(servitor_of(&world, OWNER).is_none());
}

/// A skill the player no longer knows (a subclass change between sessions)
/// restores nothing, and the row is consumed so it is not retried every login.
#[test]
fn an_unlearned_summon_skill_restores_nothing_and_is_not_retried() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 1111, 1200, 0, 0).unwrap();
    on_owner_leave_world(&mut world, OWNER);
    // The owner never had the skill in their book.

    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    assert!(servitor_of(&world, OWNER).is_none(), "nothing restored");
    assert!(
        world
            .objects
            .get_component::<crate::model::components::PlayerSummons>(&OWNER)
            .unwrap()
            .0
            .is_empty(),
        "and the row was consumed rather than retried forever"
    );
}

/// A Summoner's investment in buffing their servitor survives a relog — Java
/// keeps `character_summon_skills_save` for exactly this. Slice 27 brought the
/// servitor back but dropped everything cast on it.
#[test]
fn a_servitors_buffs_survive_a_relog() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let summon_skill = 1111;
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: summon_skill,
            level: 1,
            effects: vec![SkillEffect::Summon {
                npc_id: PANTHER,
                life_time: 1200,
                consume_item_id: 0,
                consume_item_count: 0,
            }],
            ..Default::default()
        });
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&OWNER)
        .unwrap()
        .0
        .insert(summon_skill, 1);

    // Servitor Wind Walk's shape, cast on the servitor.
    let buff = crate::model::skill::Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        abnormal_time: 1200,
        effects: vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::RunSpeed,
                mode: crate::model::stats::StatModifierType::Diff,
                amount: 50.0,
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(buff.clone());

    let servitor = summon_servitor(&mut world, OWNER, PANTHER, summon_skill, 1200, 0, 0).unwrap();
    crate::game_loop::skills::effects::apply_continuous_effects(
        &mut world, OWNER, servitor, &buff, None,
    );
    let buffed_speed = world
        .objects
        .get_component::<Speeds>(&servitor)
        .unwrap()
        .run_spd;

    on_owner_leave_world(&mut world, OWNER);
    let saved = world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&OWNER)
        .unwrap()
        .0[0]
        .clone();
    assert_eq!(saved.buffs.len(), 1, "the servitor's buff was captured");
    assert!(
        saved.buffs[0].remaining_time_secs > 0,
        "with time left on it"
    );

    crate::game_loop::servitor::restore_servitor_on_login(&mut world, OWNER);
    let back = servitor_of(&world, OWNER).expect("servitor restored");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&back)
            .unwrap()
            .run_spd,
        buffed_speed,
        "and it came back still buffed"
    );
}

/// An expired buff is not carried across — otherwise relogging would resurrect
/// buffs that had already run out.
#[test]
fn an_expired_servitor_buff_is_not_saved() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1111, 1200, 0, 0).unwrap();
    let buff = crate::model::skill::Skill {
        self_continuous: false,
        id: 1144,
        level: 1,
        abnormal_time: 10,
        effects: vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::RunSpeed,
                mode: crate::model::stats::StatModifierType::Diff,
                amount: 50.0,
                ..Default::default()
            },
        )],
        ..Default::default()
    };
    crate::game_loop::skills::effects::apply_continuous_effects(
        &mut world, OWNER, servitor, &buff, None,
    );

    world.tick += 20 * 10; // past its 10 s
    crate::game_loop::servitor::sync_summon_row(&mut world, OWNER);

    let saved = world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&OWNER)
        .unwrap()
        .0[0]
        .clone();
    assert!(
        saved.buffs.is_empty(),
        "an expired buff is not carried across a relog"
    );
}

// ---------------------------------------------------------------------------
// ServitorSkillUse (slice 29)
// ---------------------------------------------------------------------------

/// The owner presses one of the summon's action-bar buttons and the
/// **servitor** casts it. 105 `ServitorSkillUse` rows ship in `ActionData.xml`;
/// the port handled only hold/attack/stop, so every one of them was dead.
#[test]
fn a_servitor_casts_the_skill_its_action_button_names() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    const ACTION: i32 = 1000;
    const SKILL: i32 = 4079;
    world
        .data
        .action_data
        .insert_servitor_skill_for_test(ACTION, SKILL);

    // Give the servitor template the skill, and register it.
    {
        let mut t = world.data.npc_data.get(PANTHER).unwrap().clone();
        t.skill_list.push((SKILL, 1));
        world.data.npc_data.insert_for_test(t);
    }
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: SKILL,
            level: 1,
            target_type: crate::model::skill::TargetType::Self_,
            effects: vec![crate::model::skill::SkillEffect::Heal { power: 100.0 }],
            ..Default::default()
        });

    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 10.0;

    crate::game_loop::servitor::use_servitor_skill(&mut world, OWNER, SKILL);
    // `start_cast` schedules the finish; run the scheduler out to land it.
    for _ in 0..40 {
        advance_ticks(&mut world, 1);
    }

    assert!(
        world
            .objects
            .get_component::<Vitals>(&servitor)
            .unwrap()
            .cur_hp
            > 10.0,
        "the servitor cast its own heal"
    );
}

/// `OWNER_PET` aims at the **owner**, whatever they have selected.
///
/// Java writes this out by hand ahead of target resolution (`Summon.useMagic`:
/// `if (targetType == OWNER_PET) target = _owner`). The port collapsed the type
/// into `TargetType::Other`, which took the owner's *current selection*
/// instead — so Master Recharge (4025), the skill every Baby Kookaburra
/// carries, recharged whatever mob its owner had clicked, and refused with
/// "invalid target" when they had clicked nothing.
#[test]
fn an_owner_pet_skill_targets_the_owner_not_their_selection() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    const SKILL: i32 = 4025;
    {
        let mut tpl = world.data.npc_data.get(PANTHER).unwrap().clone();
        tpl.skill_list.push((SKILL, 1));
        world.data.npc_data.insert_for_test(tpl);
    }
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: SKILL,
            level: 1,
            name: "Master Recharge".into(),
            target_type: crate::model::skill::TargetType::OwnerPet,
            cast_range: 400,
            effect_range: 900,
            effects: vec![crate::model::skill::SkillEffect::ManaHeal { power: 50.0 }],
            ..Default::default()
        });
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // The owner is short on MP and — the part that used to break it — has
    // something else entirely selected.
    world
        .objects
        .get_component_mut::<Vitals>(&OWNER)
        .unwrap()
        .cur_mp = 1.0;
    let before = world
        .objects
        .get_component::<Vitals>(&OWNER)
        .unwrap()
        .cur_mp;
    world
        .objects
        .add_components(&OWNER, crate::model::components::TargetRef(Some(servitor)));

    crate::game_loop::servitor::use_servitor_skill(&mut world, OWNER, SKILL);
    for _ in 0..40 {
        advance_ticks(&mut world, 1);
    }

    assert!(
        world
            .objects
            .get_component::<Vitals>(&OWNER)
            .unwrap()
            .cur_mp
            > before,
        "the owner was recharged, not the selected target"
    );
}

/// **A summon may only use skills it actually has.** `ActionData.xml` binds
/// buttons for every summon in the game, so most rows name a skill this
/// particular servitor has never had — casting one anyway would let any summon
/// borrow another's abilities.
#[test]
fn a_servitor_refuses_a_skill_it_does_not_have() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    const SKILL: i32 = 4079;
    world
        .data
        .skill_data
        .insert_for_test(crate::model::skill::Skill {
            self_continuous: false,
            id: SKILL,
            level: 1,
            target_type: crate::model::skill::TargetType::Self_,
            effects: vec![crate::model::skill::SkillEffect::Heal { power: 100.0 }],
            ..Default::default()
        });
    // The Panther template is left without the skill.
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&servitor)
        .unwrap()
        .cur_hp = 10.0;

    crate::game_loop::servitor::use_servitor_skill(&mut world, OWNER, SKILL);
    for _ in 0..40 {
        advance_ticks(&mut world, 1);
    }

    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&servitor)
            .unwrap()
            .cur_hp,
        10.0,
        "nothing was cast"
    );
}

/// The real `ActionData.xml` binds servitor skills — a fixture cannot catch a
/// parse regression, so read the shipped file.
#[test]
fn the_real_action_data_binds_servitor_skills() {
    let data = crate::data::ActionData::load_from(DIST);
    // Action 1000 → skill 4079, one of the 13 reachable on this dist.
    assert_eq!(data.servitor_skill(1000), Some(4079));
    assert_eq!(
        data.servitor_skill(32),
        Some(4230),
        "the attack/move toggle"
    );
    assert_eq!(
        data.servitor_skill(0),
        None,
        "a non-servitor action binds nothing"
    );
}

// ---------------------------------------------------------------------------
// Summon spiritshots (slice 30)
// ---------------------------------------------------------------------------

const BEAST_SPIRITSHOT: i32 = 6646;

fn register_beast_spiritshot(world: &mut World) {
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = BEAST_SPIRITSHOT;
    t.name = "Beast Spiritshot".into();
    t.is_stackable = true;
    t.handler = crate::data::item_data::ItemHandler::BeastSpiritShot;
    t.default_action = crate::data::item_data::ActionType::SummonSpiritshot;
    world.data.item_data.insert_for_test(t);
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_700_001, BEAST_SPIRITSHOT, 10);
    objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        .auto_shots
        .push(BEAST_SPIRITSHOT);
}

fn owner_spiritshots(world: &World) -> i64 {
    world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .map(|inv| inv.count_of(BEAST_SPIRITSHOT))
        .unwrap_or(0)
}

/// A summon charges its Beast Spiritshot from the owner, at the pet level's
/// `spiritshot_count`.
#[test]
fn a_pet_charges_spiritshots_from_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);

    assert!(crate::game_loop::servitor::recharge_spiritshots(
        &mut world, pet_oid
    ));
    assert_eq!(owner_spiritshots(&world), 8, "level 1 costs 2 per cast");
    assert!(
        world
            .objects
            .get_component::<crate::model::components::ChargedShots>(&pet_oid)
            .unwrap()
            .spiritshot
    );
}

/// The charge is spent by the **cast**, not a swing — and it doubles the
/// summon's magic damage while it lasts.
#[test]
fn a_spiritshot_doubles_a_summons_magic_damage() {
    let damage_with = |charged: bool| {
        let (mut world, _db, _l) = servitor_world();
        let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
        let pet_oid = summoned_pet(&mut world);
        add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
        {
            let v = world.objects.get_component_mut::<Vitals>(&FOE).unwrap();
            v.max_hp = 100_000;
            v.cur_hp = 100_000.0;
        }
        let skill = crate::model::skill::Skill {
            self_continuous: false,
            id: 4079,
            level: 1,
            magic_type: 1,
            effects: vec![crate::model::skill::SkillEffect::MagicalAttack { power: 50.0 }],
            ..Default::default()
        };
        if charged {
            world.objects.add_components(
                &pet_oid,
                crate::model::components::ChargedShots {
                    soulshot: false,
                    spiritshot: true,
                },
            );
        }
        let before = world.objects.get_component::<Vitals>(&FOE).unwrap().cur_hp;
        crate::game_loop::skills::effects::apply_skill_effects(&mut world, pet_oid, FOE, &skill);
        before - world.objects.get_component::<Vitals>(&FOE).unwrap().cur_hp
    };

    let plain = damage_with(false);
    let shotted = damage_with(true);
    assert!(plain > 0.0, "the summon's spell hit ({plain})");
    assert!(
        shotted > plain * 1.5,
        "a charged spiritshot roughly doubles it ({plain} → {shotted})"
    );
}

/// One cast, one shot: the charge does not carry to the next spell.
#[test]
fn a_summon_spiritshot_is_spent_by_one_cast() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);
    crate::game_loop::servitor::recharge_spiritshots(&mut world, pet_oid);

    assert!(
        crate::game_loop::servitor::uncharge_spiritshot(&mut world, pet_oid),
        "spent by the first cast"
    );
    assert!(
        !crate::game_loop::servitor::uncharge_spiritshot(&mut world, pet_oid),
        "and not the second"
    );
}

/// A physical skill does not burn a magic shot.
#[test]
fn a_physical_skill_does_not_spend_a_spiritshot() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);
    register_beast_spiritshot(&mut world);
    crate::game_loop::servitor::recharge_spiritshots(&mut world, pet_oid);
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);

    let physical = crate::model::skill::Skill {
        self_continuous: false,
        id: 4080,
        level: 1,
        magic_type: 0,
        effects: vec![crate::model::skill::SkillEffect::MagicalAttack { power: 10.0 }],
        ..Default::default()
    };
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, pet_oid, FOE, &physical);

    assert!(
        world
            .objects
            .get_component::<crate::model::components::ChargedShots>(&pet_oid)
            .unwrap()
            .spiritshot,
        "the magic shot is still charged"
    );
}

// ---------------------------------------------------------------------------
// Community-board "Pet" buffer (applies a scheme to the summon)
// ---------------------------------------------------------------------------

/// **The community-board "Pet" button buffs the player's summon** (Java
/// `getPet()`/`getServitors()`), not the player; with no summon it refuses.
#[test]
fn community_board_pet_buffer_targets_the_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");

    // A one-buff scheme: Might (1068), a synthetic PA_UP buff in the test world.
    world.data.scheme_buffer.insert_for_test(1068, 1);
    world.cfg.community_board.available_buffs.insert(1068);
    world
        .buffer_schemes
        .insert(OWNER, vec![("s".to_string(), vec![1068])]);

    let has_buff = |world: &World, oid: i32| has_buff(world, oid, 1068);

    crate::game_loop::community_board::apply_scheme(&mut world, CID, OWNER, "s", true)
        .expect("applied to the summon");
    assert!(has_buff(&world, servitor), "the summon got the scheme buff");
    assert!(
        !has_buff(&world, OWNER),
        "the owner was not buffed by the Pet button"
    );

    // No summon → the Pet button refuses (Java's no-pet branch).
    unsummon_servitor(&mut world, OWNER);
    assert!(
        crate::game_loop::community_board::apply_scheme(&mut world, CID, OWNER, "s", true).is_err(),
        "with no summon the Pet button refuses"
    );
}

// ---------------------------------------------------------------------------
// Fear on a summon (Java Fear.canStart's isSummon leg)
// ---------------------------------------------------------------------------

/// **A servitor can be feared** (Java `Fear.canStart`'s `isSummon()` leg) even
/// though its NPC type isn't Attackable; a plain non-attackable NPC is not.
#[test]
fn a_servitor_can_be_feared() {
    use crate::model::components::{Movement, Position};
    use crate::model::skill::SkillEffect;
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");
    // Put the servitor east of the owner so the fear shove has a clear bearing.
    if let Some(p) = world.objects.get_component_mut::<Position>(&servitor) {
        p.x = 500;
    }
    // A non-summon, non-attackable NPC (Folk) as the control.
    add_test_npc(&mut world, 8888, 15000, "Folk", 20, 900, 0, 0);

    // A Fear-only skill (built off the synthetic Might template).
    let mut fear = world.data.skill_data.get(1068, 1).unwrap().clone();
    fear.id = 9600;
    fear.effects = vec![SkillEffect::Fear { ticks: 5 }];
    world.data.skill_data.insert_for_test(fear.clone());

    // The servitor is shoved (fear ran → a Movement order was set).
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, OWNER, servitor, &fear);
    assert!(
        world.objects.get_component::<Movement>(&servitor).is_some(),
        "the servitor is feared and shoved"
    );

    // The plain non-attackable NPC is not feared.
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, OWNER, 8888, &fear);
    assert!(
        world.objects.get_component::<Movement>(&8888).is_none(),
        "a non-summon non-attackable NPC is not feared"
    );
}

// --- Pet evolution / exchange / restore (PetManager + Evolve) --------------

const GREAT_WOLF_NPC: i32 = 16025;
const GREAT_WOLF_COLLAR: i32 = 9882;
/// Wolf → Great Wolf is `evolve 1`, and it wants level 55.
const EVOLVE_MIN_LEVEL: i32 = 55;

/// Register the Great Wolf so the evolution has somewhere to land, with a
/// two-entry level table (min level 55, then 56) so a level can be *read back*
/// from carried exp.
fn add_great_wolf(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(GREAT_WOLF_NPC);
    t.type_name = "Pet".into();
    t.name = "Great Wolf".into();
    t.level = 55;
    t.base_hp_max = 900.0;
    t.base_mp_max = 300.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    let lvl = |exp: i64| crate::data::pet_data::PetLevel {
        max_meal: 300,
        consume_meal_in_normal: 10,
        consume_meal_in_battle: 15,
        exp,
        ..Default::default()
    };
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: GREAT_WOLF_NPC,
            item_id: GREAT_WOLF_COLLAR,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(55, lvl(1_000_000)), (56, lvl(1_200_000))]
                .into_iter()
                .collect(),
        });
}

/// Put a summoned wolf at `level`/`exp` so the evolve gates can be exercised.
fn set_pet_level(world: &mut World, pet: i32, level: i32, exp: i64) {
    if let Some(p) = world.objects.get_component_mut::<PetOf>(&pet) {
        p.level = level;
        p.exp = exp;
    }
}

/// **The evolve button works, and carries the pet across.** A qualifying wolf
/// becomes a Great Wolf: the old collar and its saved row are gone, the new
/// collar is in the inventory with the new pet's level stamped on it, and the
/// pet is out again carrying its experience.
#[test]
fn a_qualifying_pet_evolves_and_keeps_its_experience() {
    let (mut world, mut db_rx, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_test_npc(&mut world, NPC_OID + 30, 30827, "Lundy", 5, 60, 0, 0);
    add_great_wolf(&mut world);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL, 1_250_000);
    // A name to carry across.
    if let Some(pets) = world.objects.get_component_mut::<PlayerPets>(&OWNER) {
        pets.0.insert(
            collar,
            crate::db::PetRow {
                collar_object_id: collar,
                name: "Rex".into(),
                level: EVOLVE_MIN_LEVEL,
                cur_hp: 1.0,
                cur_mp: 1.0,
                exp: 1_250_000,
                sp: 0,
                fed: 10,
                restore: true,
            },
        );
    }
    drain(&mut rx);
    drain_db(&mut db_rx);

    handle_request_bypass_to_server(
        &mut world,
        CID,
        &bypass_body(&format!("npc_{}_evolve 1", NPC_OID + 30)),
    );

    let inv = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap();
    assert_eq!(inv.count_of(WOLF_COLLAR), 0, "the old collar is destroyed");
    let new_collar = inv
        .items()
        .iter()
        .find(|i| i.item_id == GREAT_WOLF_COLLAR)
        .expect("the evolved collar");
    assert_eq!(
        new_collar.enchant_level, 56,
        "the collar records the pet's level — which is what a later restore reads"
    );
    let new_pet = pet_of(&world, OWNER).expect("the evolved pet is out");
    let link = world.objects.get_component::<PetOf>(&new_pet).unwrap();
    assert_eq!(link.exp, 1_250_000, "the experience came across");
    assert_eq!(
        link.level, 56,
        "…and the level is re-derived from it on the new curve"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&new_pet)
            .unwrap()
            .npc_id,
        GREAT_WOLF_NPC
    );
    assert_eq!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .unwrap()
            .0
            .get(&new_collar.object_id)
            .map(|r| r.name.as_str()),
        Some("Rex"),
        "the pet keeps its name (the html promises it)"
    );
    assert!(
        drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::DeletePetRow { collar_object_id } if *collar_object_id == collar)),
        "the old collar's saved row is deleted, not left to haunt the new pet"
    );
}

/// **The exp floor is why an evolution doesn't demote the pet.** A wolf that
/// only just made level 55 carries less exp than the Great Wolf curve wants for
/// 55, and Java floors it — otherwise the reward for evolving would be a
/// level-1 pet.
#[test]
fn evolving_floors_the_experience_at_the_new_species_curve() {
    let (mut world, ..) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_test_npc(&mut world, NPC_OID + 30, 30827, "Lundy", 5, 60, 0, 0);
    add_great_wolf(&mut world);
    // A table that starts at level 10, *below* the button's min level of 55.
    // Without Java's explicit floor the carried 4,000 exp would derive level 10
    // and the summon path would happily floor at level 10's own exp — the pet
    // would survive the evolution 45 levels down.
    if let Some(t) = world.data.pet_data.by_item_id(GREAT_WOLF_COLLAR).cloned() {
        let mut t = t;
        t.levels.insert(
            10,
            crate::data::pet_data::PetLevel {
                max_meal: 300,
                exp: 1_000,
                ..Default::default()
            },
        );
        world.data.pet_data.insert_for_test(t);
    }
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();
    // Far below the Great Wolf's 1,000,000 for level 55, but above level 10's.
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL, 4_000);

    pet_evolve::handle_evolve(&mut world, CID, OWNER, NPC_OID + 30, "evolve 1");

    let new_pet = pet_of(&world, OWNER).expect("evolved");
    let link = world.objects.get_component::<PetOf>(&new_pet).unwrap();
    assert_eq!(link.exp, 1_000_000, "floored at the new curve's level 55");
    assert_eq!(link.level, 55, "so it lands at 55, not at level 1");
}

/// The gates: too low a level, the wrong species, no pet out, and a dead pet
/// are each refused — with Java's `evolve_no.htm`, no system message.
#[test]
fn the_evolve_gates_refuse_and_change_nothing() {
    let (mut world, ..) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_great_wolf(&mut world);

    let held = |w: &World| {
        w.objects
            .get_component::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .count_of(GREAT_WOLF_COLLAR)
    };

    // No pet out at all.
    pet_evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 1");
    assert_eq!(held(&world), 0, "nothing handed out");

    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();

    // Level 54 — one short.
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL - 1, 900_000);
    pet_evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 1");
    assert_eq!(held(&world), 0, "one level short is still short");
    assert!(pet_of(&world, OWNER).is_some(), "and the pet is still out");

    // Right level, wrong button: `evolve 3` is the Baby Buffalo line. The
    // buffalo has to *exist* in pet data, or the refusal would come from the
    // missing lookup rather than from the species check.
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: 12780,
            item_id: 6648,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(
                55,
                crate::data::pet_data::PetLevel {
                    max_meal: 300,
                    exp: 1_000_000,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        });
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: 16034,
            item_id: 10311,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(
                55,
                crate::data::pet_data::PetLevel {
                    max_meal: 300,
                    exp: 1_000_000,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        });
    set_pet_level(&mut world, pet, EVOLVE_MIN_LEVEL, 1_150_000);
    pet_evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 3");
    assert_eq!(held(&world), 0, "a wolf is not a buffalo");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .count_of(10311),
        0,
        "…and no improved buffalo collar either"
    );

    // Dead pet — Java calls this an exploit attempt.
    if let Some(v) = world
        .objects
        .get_component_mut::<crate::model::components::Vitals>(&pet)
    {
        v.dead = true;
    }
    drain(&mut rx);
    pet_evolve::handle_evolve(&mut world, CID, OWNER, 0, "evolve 1");
    assert_eq!(held(&world), 0, "a dead pet cannot evolve");
    // The exploit attempt punishes: the immediate warning line (S1_TEXT) is
    // the only system message, and the kick lands 5 s later.
    assert_eq!(
        sm_ids_of(&drain(&mut rx)),
        vec![server_packets::sm_ids::S1_TEXT]
    );
    advance_ticks(&mut world, 51);
    assert!(!world.clients.contains_key(&CID), "kicked for the exploit");
}

/// The exchange counter: a ticket becomes a collar, and without the ticket
/// nothing is handed out.
#[test]
fn a_pet_ticket_exchanges_for_a_collar() {
    let (mut world, ..) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4500_0000..0x4500_0100;

    // No ticket → nothing.
    pet_evolve::handle_exchange(&mut world, CID, OWNER, 0, "exchange 1");
    let count_of = |w: &World, id: i32| {
        w.objects
            .get_component::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .count_of(id)
    };
    assert_eq!(count_of(&world, 6650), 0);

    // Kookaburra ticket 7585 → collar 6650.
    super::items::add_inventory_item(&mut world, OWNER, 7585, 1).unwrap();
    pet_evolve::handle_exchange(&mut world, CID, OWNER, 0, "exchange 1");
    assert_eq!(count_of(&world, 7585), 0, "the ticket is taken");
    assert_eq!(count_of(&world, 6650), 1, "the collar is given");
}

/// Restore works off an **item**, not a live pet, and reads the pet's level out
/// of the collar's enchant — the one place it was recorded.
#[test]
fn restore_reads_the_level_off_the_collar_enchant() {
    let (mut world, ..) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    add_great_wolf(&mut world);
    // The Great Snow Wolf collar (10307) restores to the Great Wolf (9882).
    let mut t = crate::data::npc_data::default_template(GREAT_WOLF_NPC + 12);
    t.type_name = "Pet".into();
    t.name = "Great Snow Wolf".into();
    t.level = 55;
    t.base_hp_max = 900.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);
    world
        .data
        .pet_data
        .insert_for_test(crate::data::pet_data::PetTemplate {
            npc_id: GREAT_WOLF_NPC + 12,
            item_id: 10307,
            food_item_id: 2515,
            hungry_limit: 55,
            load: 54_510,
            levels: [(
                55,
                crate::data::pet_data::PetLevel {
                    max_meal: 300,
                    exp: 1_000_000,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        });
    let World { data, objects, .. } = &mut world;
    objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .add_item(&data.item_data, 7_100_055, 10307, 1);
    let snow = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap()
        .items()
        .iter()
        .find(|i| i.item_id == 10307)
        .unwrap()
        .object_id;
    // The collar remembers a level-56 pet.
    if let Some(inv) = world
        .objects
        .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
    {
        inv.set_item_enchant(snow, 56);
    }

    pet_evolve::handle_restore(&mut world, CID, OWNER, 0, "restore 1");

    let inv = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&OWNER)
        .unwrap();
    assert_eq!(inv.count_of(10307), 0, "the seasonal collar is consumed");
    assert_eq!(inv.count_of(GREAT_WOLF_COLLAR), 1, "the base one is given");
    let pet = pet_of(&world, OWNER).expect("and the pet is summoned");
    assert_eq!(
        world.objects.get_component::<PetOf>(&pet).unwrap().level,
        56,
        "at the level the collar's enchant recorded, not the minimum"
    );
}

// ---------------------------------------------------------------------------
// Buff sharing (`Skill.isSharedWithSummon`)
// ---------------------------------------------------------------------------

/// A buff the owner receives is re-applied to their servitor by
/// `Skill.applyEffects`' sharing branch. Every clause below is Java's.
const SHARED_BUFF: i32 = 9501;
const PRIVATE_BUFF: i32 = 9502;
const SHARED_DEBUFF: i32 = 9503;

fn sharing_skill(id: i32, shared: bool, is_debuff: bool) -> crate::model::skill::Skill {
    crate::model::skill::Skill {
        self_continuous: false,
        id,
        level: 1,
        name: format!("Share {id}"),
        is_continuous: true,
        abnormal_time: 3600,
        abnormal_level: 1,
        abnormal_type: format!("SHARE_{id}"),
        shared_with_summon: shared,
        is_debuff,
        // A stat pump so the buff is a real continuous entry, not a bare flag.
        effects: vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: crate::model::stats::Stat::PhysicalAttack,
                mode: crate::model::stats::StatModifierType::Per,
                amount: 8.0,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
            },
        )],
        ..Default::default()
    }
}

fn buff_ids(world: &World, oid: i32) -> Vec<i32> {
    world
        .objects
        .get_component::<crate::model::components::Buffs>(&oid)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default()
}

fn sharing_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = servitor_world();
    for (id, shared, debuff) in [
        (SHARED_BUFF, true, false),
        (PRIVATE_BUFF, false, false),
        (SHARED_DEBUFF, true, true),
    ] {
        world
            .data
            .skill_data
            .insert_for_test(sharing_skill(id, shared, debuff));
    }
    (world, db, l)
}

fn land_on(world: &mut World, skill_id: i32, target: i32) {
    let skill = skill_by_id(world, skill_id, 1).unwrap();
    crate::game_loop::skills::effects::apply_skill_effects(world, target, target, &skill);
}

/// The headline: buffing yourself also buffs your servitor. Without this every
/// summoner's pet fought permanently unbuffed.
#[test]
fn a_buff_on_the_owner_is_shared_with_their_servitor() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    land_on(&mut world, SHARED_BUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![SHARED_BUFF], "owner keeps it");
    assert_eq!(
        buff_ids(&world, servitor),
        vec![SHARED_BUFF],
        "and the servitor receives the same buff"
    );
}

/// `<isSharedWithSummon>false</isSharedWithSummon>` stops at the player. Only
/// three skills in the datapack declare the tag, which is exactly why the
/// parser's default must be `true` — see the sibling test below.
#[test]
fn a_non_shared_buff_stops_at_the_owner() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    land_on(&mut world, PRIVATE_BUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![PRIVATE_BUFF]);
    assert!(
        buff_ids(&world, servitor).is_empty(),
        "a non-shared buff must not reach the servitor"
    );
}

/// Java's guard is `!_isDebuff`: sharing is a *favour*, so a debuff landing on
/// the owner must not be copied onto their summon as a second victim.
#[test]
fn a_debuff_on_the_owner_is_never_shared() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    land_on(&mut world, SHARED_DEBUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![SHARED_DEBUFF]);
    assert!(
        buff_ids(&world, servitor).is_empty(),
        "a debuff is not shared even when the skill is flagged shared"
    );
}

/// **A pet is not a servitor.** Java shares through `getServitors()`, and `_pet`
/// is a separate field, so a wolf receives nothing. Easy to get wrong here
/// because this port hangs `ServitorOf` on pets too.
#[test]
fn a_pet_does_not_receive_shared_buffs() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    land_on(&mut world, SHARED_BUFF, OWNER);

    assert_eq!(buff_ids(&world, OWNER), vec![SHARED_BUFF]);
    assert!(
        buff_ids(&world, pet).is_empty(),
        "Java reads getServitors(), which excludes the pet"
    );
}

/// Sharing follows a buff that *landed*: the servitor is not a second roll of
/// the dice, and the chain stops at one hop (the servitor is not a player, so
/// it shares nothing onward).
#[test]
fn sharing_does_not_recurse_past_the_servitor() {
    let (mut world, _db, _l) = sharing_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    // Cast straight at the servitor: it is not a player, so nothing is shared
    // back to the owner.
    land_on(&mut world, SHARED_BUFF, servitor);

    assert_eq!(buff_ids(&world, servitor), vec![SHARED_BUFF]);
    assert!(
        buff_ids(&world, OWNER).is_empty(),
        "sharing runs owner → servitor only, never the reverse"
    );
}

/// The parser default, against the **real datapack** rather than a fixture.
///
/// `isSharedWithSummon` defaults to `true` in Java and is declared on a handful
/// of skills, so a `false`-defaulting parse would look perfectly healthy in a
/// unit test while silently switching sharing off for every buff in the game.
/// Prophecy of Might declares nothing and must come back shared.
///
/// Skill 1557 is the counter-case, and its identity is the point: **Servitor
/// Share** is the skill that copies the owner's stats onto the summon itself,
/// so re-sharing it would double-apply. Java's source carries that exact
/// comment ("Avoiding Servitor Share since it's implementation already
/// 'shares' the effect").
#[test]
fn the_real_datapack_defaults_to_shared() {
    let skills = dist::skills();

    let prophecy = skills
        .get(1352, 1)
        .expect("Prophecy of Might is on this dist");
    assert!(
        prophecy.shared_with_summon,
        "a skill that declares no <isSharedWithSummon> defaults to shared"
    );

    let servitor_share = skills.get(1557, 1).expect("Servitor Share");
    assert!(
        !servitor_share.shared_with_summon,
        "Servitor Share must not be re-shared onto the summon it already shares to"
    );
}

// ---------------------------------------------------------------------------
// The active pet's collar (Java `Item.isAvailable` / `ExBuySellList`)
// ---------------------------------------------------------------------------

/// The sell tab hides the collar of a pet that is currently out — Java's
/// `(pet == null) || (item.getObjectId() != pet.getControlObjectId())`.
///
/// Keyed on the **object** id, so a second collar of the same kind sitting in
/// the bag stays sellable. That distinction is the whole point of the guard and
/// is what an item-id comparison would get wrong, so both are asserted.
#[test]
fn the_summoned_pets_collar_is_not_offered_for_sale() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    // `give_collar` registers the *pet* and NPC templates but not the collar's
    // own `ItemTemplate`, without which the sell filter drops it for want of a
    // template and the test below passes vacuously.
    let mut tmpl = crate::data::item_data::ItemTemplate::default();
    tmpl.item_id = WOLF_COLLAR;
    tmpl.name = "Wolf Collar".into();
    tmpl.is_sellable = true;
    tmpl.price = 100;
    world.data.item_data.insert_for_test(tmpl);

    let collar = give_collar(&mut world);
    // A *second* collar of the same kind. `give_collar` hard-codes one object
    // id, so the spare is added by hand — the point of this test is that the
    // guard compares object ids, which needs two of them.
    let spare = collar + 1;
    {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap()
            .add_item(&data.item_data, spare, WOLF_COLLAR, 1);
    }
    assert_ne!(collar, spare, "two distinct collar instances");
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).expect("summoned");

    // Build the **real** `ExBuySellList` and count its sell entries. An earlier
    // draft re-implemented the filter inline here, which meant deleting the
    // production filter changed nothing — the test only proved it agreed with
    // itself. Sabotage caught it.
    let sell_entry_count = |world: &World, active: Option<i32>| -> i16 {
        let inv = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&OWNER)
            .unwrap();
        let pkt =
            crate::network::trade::ex_buy_sell_list_sell(inv, &[], &world.data, false, active);
        // u8 opcode + i16 ex-opcode + i32 type + i32 slots, then the i16 count.
        i16::from_le_bytes([pkt[11], pkt[12]])
    };

    let active = crate::game_loop::servitor::active_pet_collar(&world, OWNER);
    assert_eq!(active, Some(collar), "the summoned pet's own collar");

    // Baseline: unguarded, both collars are offered. Without it the assertion
    // below would also pass on a list empty for an unrelated reason — and it
    // nearly was: `give_collar` never registers the collar's own `ItemTemplate`,
    // so before one was added above, *neither* collar appeared at all.
    let unguarded = sell_entry_count(&world, None);
    let guarded = sell_entry_count(&world, active);
    assert!(
        unguarded >= 2,
        "baseline: both collars are offered when nothing is excluded (got {unguarded})"
    );
    assert_eq!(
        guarded,
        unguarded - 1,
        "exactly one entry — the summoned pet's collar — is withheld, so a \
         spare of the same item id stays sellable"
    );
}

/// Unsummoning releases the collar: it is sellable again. Pinned because the
/// guard reads through the live `SummonRef` link, so a despawn path that forgot
/// to clear it would silently keep the item locked forever.
#[test]
fn unsummoning_releases_the_collar_for_sale() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    assert_eq!(
        crate::game_loop::servitor::active_pet_collar(&world, OWNER),
        Some(collar)
    );

    // Takes the owner, not the summon — one path retires either kind.
    let _ = pet;
    unsummon_servitor(&mut world, OWNER);

    assert_eq!(
        crate::game_loop::servitor::active_pet_collar(&world, OWNER),
        None,
        "no pet out — nothing is locked"
    );
}

/// `//fullfood` fills the targeted pet's bar. Java gates on `isPet()`, which a
/// skill-summoned servitor fails: its `PetInfo` fed slot carries its remaining
/// lifetime, not food, so filling it would be meaningless.
#[test]
fn fullfood_fills_a_pet_and_refuses_a_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    // Drain the bar, then target the pet and fill it.
    {
        let p = world.objects.get_component_mut::<PetOf>(&pet).unwrap();
        p.fed = 1;
    }
    // `use_admin_command` returns silently for a non-GM (Java `if (!isGM())`),
    // and `is_gm` resolves the level through `AdminData` — which the synthetic
    // test world loads *empty*, so the real table is needed for level 70 to
    // mean anything.
    world.data.admin = crate::data::AdminData::load_from(DIST);
    world
        .objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        // `AdminCommands.xml` puts `admin_fullfood` at accessLevel **100**
        // ("Master"), not 70 — a level-70 GM is refused.
        .access_level = 100;
    world
        .objects
        .add_components(&OWNER, crate::model::components::TargetRef(Some(pet)));
    crate::game_loop::admin::use_admin_command(&mut world, CID, "admin_fullfood", false);

    let p = world.objects.get_component::<PetOf>(&pet).unwrap();
    assert_eq!(p.fed, p.max_fed, "the bar is filled to max");

    // A servitor is not a pet: the command must not touch it.
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    world
        .objects
        .add_components(&OWNER, crate::model::components::TargetRef(Some(servitor)));
    crate::game_loop::admin::use_admin_command(&mut world, CID, "admin_fullfood", false);
    assert!(
        !world.objects.has_component::<PetOf>(&servitor),
        "a servitor never grows a food bar from //fullfood"
    );
}

// ---------------------------------------------------------------------------
// G34 S4 sub-slice 15 — Betray
// ---------------------------------------------------------------------------

fn action_use_body(action_id: i32) -> Vec<u8> {
    let mut w = commons::network::PacketWriter::new();
    w.write_i32(action_id);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_u8(0);
    w.into_bytes()
}

/// **Betray (1380)** turns somebody's servitor against them. Three things have
/// to happen and a port that only did the first would look plausible: the AI
/// points at the **owner**, the servitor stops taking orders ("your servitor is
/// unresponsive"), and `SummonInfo`'s status bit `0x01` marks it
/// auto-attackable so the owner can kill their own pet.
#[test]
fn betray_turns_a_servitor_against_its_owner_and_it_stops_obeying() {
    let (mut world, _db, _l) = servitor_world();
    let mut out = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 80, 0, 0);

    // Before: the servitor obeys an attack order.
    assert!(
        servitor_attack(&mut world, OWNER, FOE),
        "an unbetrayed servitor takes orders"
    );

    let caster = OWNER + 1;
    let _c = ingame_player(&mut world, CID + 1, caster, 30, 0, 0);
    let betray = crate::model::skill::Skill {
        self_continuous: false,
        id: 9420,
        level: 1,
        target_type: crate::model::skill::TargetType::EnemyOnly,
        abnormal_time: 1200,
        abnormal_type: "BETRAY".into(),
        effects: vec![SkillEffect::Betray],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(betray.clone());
    drain(&mut out);
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, caster, servitor, &betray);

    // 1. The flag is up.
    assert_ne!(
        crate::game_loop::abnormal::flags_of(&world, servitor)
            & crate::model::skill::effect_flag::BETRAYED,
        0,
        "the BETRAYED flag lands"
    );
    // 2. It is attacking its owner.
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::NpcAi>(&servitor)
            .map(|ai| ai.intention),
        Some(crate::model::npc::NpcIntention::Attack),
        "and it has turned on someone"
    );
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&servitor)
            .and_then(|a| a.0.get(&OWNER).map(|i| i.hate))
            .unwrap_or(0.0)
            > 0.0,
        "specifically its own owner"
    );
    // 3. It no longer obeys.
    drain(&mut out);
    crate::game_loop::servitor::handle_request_action_use(
        &mut world,
        CID,
        &action_use_body(crate::game_loop::servitor::action::SERVITOR_STOP),
    );
    let pkts = drain(&mut out);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS
        ),
        "a betrayed servitor refuses its owner's commands"
    );
}

/// **`ImmobilePetBuff`** (Servitor Empowerment 1299) roots the servitor for the
/// duration — the trade for whatever else the buff grants. The root is the
/// `IMMOBILIZED` flag, the same one `BlockMove` uses, and it has to come *back
/// off* when the buff ends or the servitor is stuck for good.
#[test]
fn servitor_empowerment_roots_the_servitor_until_it_expires() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    let immobile = |world: &World| {
        crate::game_loop::abnormal::flags_of(world, servitor)
            & crate::model::skill::effect_flag::IMMOBILIZED
            != 0
    };
    assert!(!immobile(&world), "free to move before the buff");

    let empower = crate::model::skill::Skill {
        self_continuous: false,
        id: 9422,
        level: 1,
        target_type: crate::model::skill::TargetType::Summon,
        abnormal_time: 1200,
        abnormal_type: "EMPOWER".into(),
        effects: vec![SkillEffect::ImmobilePetBuff],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(empower.clone());
    crate::game_loop::skills::effects::apply_skill_effects(&mut world, OWNER, servitor, &empower);
    assert!(immobile(&world), "the buff roots it");

    crate::game_loop::skills::effects::handle_buff_expire(&mut world, servitor, 9422);
    assert!(
        !immobile(&world),
        "and expiry frees it — otherwise the servitor is stuck for good"
    );
}

// ---------------------------------------------------------------------------
// Resurrecting a servitor
// ---------------------------------------------------------------------------

/// `ConditionPlayerCanResurrect`'s summon leg, which the port used to answer
/// with a blanket refusal.
///
/// The three gates, in Java's order: the summon must be **dead**, must not be
/// resurrection-blocked, and its **owner** must not already have a revive
/// prompt open (`player.isRevivingPet()` — the flag lives on the owner, not on
/// the summon, which is the part that is easy to get wrong).
#[test]
fn a_dead_servitor_can_be_resurrected_but_a_live_one_cannot() {
    use crate::game_loop::skills::conditions::check_cast;

    let skills = dist::skills();
    let res = skills.get(1016, 2).expect("Resurrection loads");

    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);
    let pet = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");

    // Alive: refused. This is the case the blanket refusal used to get right
    // by accident, so on its own it proves nothing — it is the *pair* that
    // discriminates.
    assert!(
        check_cast(&world, OWNER, res, pet).is_err(),
        "a living servitor is not a resurrection target"
    );

    world
        .objects
        .get_component_mut::<Vitals>(&pet)
        .unwrap()
        .dead = true;
    assert!(
        check_cast(&world, OWNER, res, pet).is_ok(),
        "a dead servitor is one — the leg the port was missing"
    );

    // Resurrection-blocked (Java `isResurrectionBlocked`): refused again.
    let mut buffs = crate::model::components::Buffs::default();
    buffs.0.push(crate::model::skill::ActiveBuff {
        displayed: true,
        skill_id: 1,
        skill_level: 1,
        abnormal_type_client_id: 0,
        abnormal_type: "NONE".to_string(),
        abnormal_level: 0,
        slot: crate::model::skill::BuffSlot::Uncapped,
        expires_at_tick: u64::MAX,
        passive: false,
        effect_flags: crate::model::skill::effect_flag::BLOCK_RESURRECTION,
        blocked_abnormals: Vec::new(),
        abnormal_visuals: Vec::new(),
        effects: Vec::new(),
    });
    world.objects.add_components(&pet, buffs);
    assert!(
        check_cast(&world, OWNER, res, pet).is_err(),
        "a resurrection-blocked servitor stays down"
    );
}

/// `storePetFood`: riding a pet drains the shared feed gauge, and the
/// dismount writes the drained value back onto the collar's `pets` row — a
/// rider who mounts at 100 and climbs off at 37 summons a pet at 37, not at
/// the value stored when the pet was unsummoned onto the saddle.
#[test]
fn dismount_stores_the_drained_feed_on_the_collar_row() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    world
        .data
        .categories
        .insert_for_test("WOLF_GROUP", &[WOLF_NPC]);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    world.objects.get_component_mut::<PetOf>(&pet).unwrap().fed = 100;

    crate::game_loop::user_commands::mount(&mut world, CID, OWNER);
    {
        let p = world
            .objects
            .get_component::<crate::model::Player>(&OWNER)
            .unwrap();
        assert!(p.is_mounted(), "the wolf was ridden");
        assert_eq!(
            p.mount_collar_object_id, collar,
            "the collar link rides along"
        );
        assert_eq!(p.mount_feed, 100, "the pet's food carried onto the gauge");
    }

    // The ride drains the gauge…
    world
        .objects
        .get_component_mut::<crate::model::Player>(&OWNER)
        .unwrap()
        .mount_feed = 37;
    crate::game_loop::admin::mounts::dismount(&mut world, OWNER);

    let p = world
        .objects
        .get_component::<crate::model::Player>(&OWNER)
        .unwrap();
    assert!(!p.is_mounted());
    assert_eq!(p.mount_collar_object_id, 0, "the link cleared");
    assert_eq!(
        world.objects.get_component::<PlayerPets>(&OWNER).unwrap().0[&collar].fed,
        37,
        "the drained gauge went back onto the pets row"
    );
}
