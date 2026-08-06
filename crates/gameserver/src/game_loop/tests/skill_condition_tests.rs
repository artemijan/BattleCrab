//! Skill conditions — G34 S1 (`skills::conditions`, PLAN_G34_SKILL_PARITY.md).
//!
//! These run against the **real dist skills**, not fixtures, because the point
//! of the slice is that the datapack's own `<conditions>` blocks are now
//! honoured. Before S1 every one of these casts succeeded.

use super::*;
use crate::game_loop::skills::cast::handle_request_magic_skill_use;
use crate::model::components::{SkillBook, Vitals};
use crate::model::inventory::Inventory;
use crate::network::server_packets::sm_ids;

const CID: u32 = 1;
const CASTER: i32 = 5901;

/// Long Sword (SWORD), Bone Dagger (DAGGER) — two weapon types the dist's
/// `EquipWeapon` conditions discriminate between.
const LONG_SWORD: i32 = 2;
const BONE_DAGGER: i32 = 11;

/// Sonic Focus: `EquipWeapon` DUAL/DUALBLUNT/SWORD/BLUNT + `OpEnergyMax`.
const SONIC_FOCUS: i32 = 8;
/// Revival: `RemainHpPer` `LESS 10` on the **caster**.
const REVIVAL: i32 = 181;

fn dist_world() -> (World, impl Sized, impl Sized, impl Sized) {
    let (mut world, a, b, c) = test_world();
    world.data =
        crate::data::GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    (world, a, b, c)
}

fn teach(world: &mut World, skill_id: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(skill_id, 1);
}

/// Put `item_id` in the right hand. The object id is derived from the item id
/// so two calls in one test don't collide in the container — swapping weapons
/// mid-test is the point of the `EquipWeapon` case.
fn arm(world: &mut World, item_id: i32) {
    let mut inv = world
        .objects
        .get_component::<Inventory>(&CASTER)
        .expect("inventory")
        .clone();
    let oid = inv.add_item(&world.data.item_data, 0x5900_0000 + item_id, item_id, 1);
    inv.equip_item(&world.data.item_data, oid);
    world.objects.add_components(&CASTER, inv);
}

fn cast(world: &mut World, skill_id: i32) {
    handle_request_magic_skill_use(world, CID, &magic_skill_use_body(skill_id, false));
    advance_world(world, 30);
}

/// `EquipWeapon` (88 learnable skills — the largest single condition on this
/// dist): Sonic Focus refuses bare-handed and with the wrong weapon class, and
/// lands with a sword. Java `EquipWeaponSkillCondition` tests the *equipped*
/// weapon's type mask, so a dagger is not "a weapon" for a sword skill.
#[test]
fn equip_weapon_refuses_the_wrong_weapon_and_bare_hands() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 0);
    teach(&mut world, SONIC_FOCUS);

    // Bare-handed: refused, and told why.
    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    let refused = drain(&mut rx);
    assert!(
        has_system_message(&refused, sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS),
        "bare-handed Sonic Focus is refused with the generic condition message"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        0,
        "…and builds no Force"
    );

    // A dagger is a weapon, but not one of DUAL/DUALBLUNT/SWORD/BLUNT.
    arm(&mut world, BONE_DAGGER);
    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        0,
        "a dagger does not satisfy a SWORD/BLUNT/DUAL condition"
    );

    // The listed type does.
    arm(&mut world, LONG_SWORD);
    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        1,
        "with a sword equipped the skill lands (level-1 cap is 1 charge)"
    );
}

/// `OpEnergyMax` is the *inverse* of `EnergySaved`: it refuses once the caster
/// is **at** the cap, and sends its own "force has reached maximum capacity"
/// ahead of the generic refusal — Java sends both.
#[test]
fn op_energy_max_refuses_at_the_cap_with_its_own_message() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 0);
    teach(&mut world, SONIC_FOCUS);
    arm(&mut world, LONG_SWORD);
    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .charges = 1; // the level-1 cap

    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    let refused = drain(&mut rx);
    assert!(
        has_system_message(&refused, sm_ids::YOUR_FORCE_HAS_REACHED_MAXIMUM_CAPACITY),
        "the condition's own message"
    );
    assert!(
        has_system_message(&refused, sm_ids::S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS),
        "…then the generic one — Java's `checkCondition` sends both"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        1,
        "no further Force gained"
    );
}

/// `RemainHpPer` with `percentType LESS` / `affectType CASTER`: Revival is the
/// emergency self-heal and Java refuses it above 10 % HP. The port used to cast
/// it at any HP, which is what made it an unconditional full heal.
#[test]
fn remain_hp_per_gates_revival_on_the_casters_own_hp() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 0);
    teach(&mut world, REVIVAL);
    let max_hp = pvit(&world, CASTER).max_hp as f64;

    // 20 % — above the threshold, so Java refuses and no healing happens.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_hp = max_hp * 0.2;
    drain(&mut rx);
    cast(&mut world, REVIVAL);
    assert!(
        (pvit(&world, CASTER).cur_hp - max_hp * 0.2).abs() < 1.0,
        "above 10 % the cast is refused, HP unchanged: {}",
        pvit(&world, CASTER).cur_hp
    );

    // 5 % — inside the band, the heal lands.
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .cur_hp = max_hp * 0.05;
    drain(&mut rx);
    cast(&mut world, REVIVAL);
    assert!(
        pvit(&world, CASTER).cur_hp > max_hp * 0.5,
        "at 5 % the emergency heal is allowed: {}",
        pvit(&world, CASTER).cur_hp
    );
}

/// The GM bypass: Java's `checkCondition` returns `true` outright for a
/// character that can override `PlayerCondOverride.SKILL_CONDITIONS`, so a GM
/// casts a sword skill bare-handed. Guards against the engine being wired in
/// ahead of that check.
#[test]
fn a_gm_skips_every_skill_condition() {
    let (mut world, _a, _b, _c) = dist_world();
    let mut rx = ingame_player_access(&mut world, CID, CASTER, 100);
    teach(&mut world, SONIC_FOCUS);

    drain(&mut rx);
    cast(&mut world, SONIC_FOCUS);
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .unwrap()
            .charges,
        1,
        "a GM builds Force with no weapon equipped"
    );
}

// `<passiveConditions>` has **no observable test on this dist**, and that is a
// finding rather than an omission.
//
// Only two learnable skills declare the block:
//
// * **Sword/Blunt Weapon Mastery (205)** — `EquipWeapon` SWORD/BLUNT. Its
//   `PAtk` effect *also* carries its own `<weaponType>SWORD,BLUNT`, which the
//   effect-level `weapon_condition` filter has honoured since G14. The skill
//   condition is therefore redundant here: disabling `passive_stat_gate`
//   entirely changes no stat on this datapack (verified by sabotage — the
//   dagger delta stayed 0 either way). An earlier version of this test claimed
//   the block was "the only thing tying the bonus to a sword"; that came from a
//   grep truncated before the `<weaponType>` at the end of the effect.
// * **Inner Rhythm (428)** — `TargetMyParty` in a passive block, which Java's
//   own handler answers `false` to (no target), disabling the passive outright.
//   Deliberately not reproduced; see `passive_stat_gate`'s note.
//
// So the gate is wired and correct, and inert on this content. Anything that
// makes it observable — a datapack effect losing its own `<weaponType>`, or a
// new passive condition kind — needs a test here at that point.

// ---------------------------------------------------------------------------
// `CallPc.checkSummonTargetStatus` — the recall gate
// ---------------------------------------------------------------------------

/// Every branch of Java's gate, in Java's order.
///
/// The order is the point. Several of these states co-occur, and Java resolves
/// them by position, not by precedence rules: a **rooted player inside a
/// running olympiad match** reads the *combat* line, because the root branch
/// comes first. Fold the branches together and that message silently changes.
///
/// Also pinned: the two "in an area which blocks" strings are different ids
/// (1895 zone, 1908 observer) carrying the same text, and Java feeds them
/// `addString` where the first three branches use `addPcName`.
#[test]
fn the_recall_gate_refuses_each_state_with_javas_message_and_order() {
    use crate::game_loop::skills::effects::control::check_summon_target_status;
    use crate::model::skill::{ActiveBuff, BuffSlot, effect_flag};
    use commons::system_messages::SmParam;

    const MEMBER: i32 = 5921;

    fn world_with_member() -> World {
        let (mut world, _a, _b, _c) = test_world();
        ingame_player_access(&mut world, 2, MEMBER, 0);
        world
    }

    fn flag(world: &mut World, flags: u32) {
        let mut buffs = crate::model::components::Buffs::default();
        buffs.0.push(ActiveBuff {
            displayed: true,
            skill_id: 1,
            skill_level: 1,
            abnormal_type_client_id: 0,
            abnormal_type: "NONE".to_string(),
            abnormal_level: 0,
            slot: BuffSlot::Uncapped,
            expires_at_tick: u64::MAX,
            passive: false,
            effect_flags: flags,
            blocked_abnormals: Vec::new(),
            abnormal_visuals: Vec::new(),
            effects: Vec::new(),
        });
        world.objects.add_components(&MEMBER, buffs);
    }

    // A healthy party member passes — the zero case, without which every
    // assertion below would hold for a gate that refused unconditionally.
    let mut world = world_with_member();
    assert!(check_summon_target_status(&world, MEMBER).is_none());

    // Dead.
    world
        .objects
        .get_component_mut::<Vitals>(&MEMBER)
        .unwrap()
        .dead = true;
    assert_eq!(
        check_summon_target_status(&world, MEMBER).map(|(id, _)| id),
        Some(sm_ids::C1_IS_DEAD_AT_THE_MOMENT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED)
    );

    // Faking it counts as dead — Java's `isAlikeDead()`, not `isDead()`.
    let mut world = world_with_member();
    flag(&mut world, effect_flag::FAKE_DEATH);
    assert_eq!(
        check_summon_target_status(&world, MEMBER).map(|(id, _)| id),
        Some(sm_ids::C1_IS_DEAD_AT_THE_MOMENT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED),
        "fake death blocks the recall too"
    );

    // Rooted reads the *combat* line, and does so even mid-olympiad: the root
    // branch sits above the olympiad one.
    let mut world = world_with_member();
    flag(&mut world, effect_flag::ROOTED);
    world
        .olympiad
        .matches
        .push(crate::model::olympiad::OlympiadMatch {
            player_a: MEMBER,
            player_b: 999,
            arena: 0,
            instance_id: 0,
            deadline_tick: u64::MAX,
            return_a: (0, 0, 0),
            return_b: (0, 0, 0),
        });
    assert_eq!(
        check_summon_target_status(&world, MEMBER).map(|(id, _)| id),
        Some(sm_ids::C1_IS_ENGAGED_IN_COMBAT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED),
        "root wins over the olympiad — Java's branch order, not a precedence rule"
    );

    // In a match, unrooted: now the olympiad line, and with no parameters.
    let mut world = world_with_member();
    world
        .olympiad
        .matches
        .push(crate::model::olympiad::OlympiadMatch {
            player_a: MEMBER,
            player_b: 999,
            arena: 0,
            instance_id: 0,
            deadline_tick: u64::MAX,
            return_a: (0, 0, 0),
            return_b: (0, 0, 0),
        });
    let (id, params) = check_summon_target_status(&world, MEMBER).expect("refused");
    assert_eq!(
        id,
        sm_ids::A_USER_PARTICIPATING_IN_THE_OLYMPIAD_CANNOT_USE_SUMMONING_OR_TELEPORTING
    );
    assert!(params.is_empty(), "Java sends this one with no name");

    // Merely *registered* is a different message from being in a match.
    let mut world = world_with_member();
    world.olympiad.non_class_registers.insert(MEMBER);
    let (id, params) = check_summon_target_status(&world, MEMBER).expect("refused");
    assert_eq!(
        id,
        sm_ids::C1_IS_IN_AN_AREA_WHICH_BLOCKS_SUMMONING_OR_TELEPORTING_2
    );
    assert!(
        matches!(params.first(), Some(SmParam::Text(_))),
        "addString, not addPcName — a real wire difference: {params:?}"
    );

    // Jailed stands in for the JAIL zone, and takes the *other* id of the pair.
    let mut world = world_with_member();
    world
        .objects
        .get_component_mut::<crate::model::Player>(&MEMBER)
        .unwrap()
        .jailed = true;
    let (id, params) = check_summon_target_status(&world, MEMBER).expect("refused");
    assert_eq!(
        id,
        sm_ids::C1_IS_IN_AN_AREA_WHICH_BLOCKS_SUMMONING_OR_TELEPORTING,
        "1895, not the 1908 the observer branch uses"
    );
    assert!(matches!(params.first(), Some(SmParam::Text(_))));

    // On a wyvern: the area line, no name.
    let mut world = world_with_member();
    world
        .objects
        .get_component_mut::<crate::model::Player>(&MEMBER)
        .unwrap()
        .mount_type = 2;
    let (id, params) = check_summon_target_status(&world, MEMBER).expect("refused");
    assert_eq!(
        id,
        sm_ids::YOU_CANNOT_USE_SUMMONING_OR_TELEPORTING_IN_THIS_AREA
    );
    assert!(params.is_empty());

    // …but a strider is not flying, so it does not block.
    let mut world = world_with_member();
    world
        .objects
        .get_component_mut::<crate::model::Player>(&MEMBER)
        .unwrap()
        .mount_type = 1;
    assert!(
        check_summon_target_status(&world, MEMBER).is_none(),
        "only the wyvern is `isFlyingMounted()`"
    );
}

// ---------------------------------------------------------------------------
// Summon Friend (`CallPc`, the player half)
// ---------------------------------------------------------------------------

/// Summon Friend 1403: charge the target a Summoning Crystal, prompt them, and
/// teleport only if they say yes to *this* summoner.
///
/// The order is Java's and is worth pinning: the toll is charged to the
/// **target** and charged *before* the prompt, so declining still costs the
/// item. Charging on accept would make a declined summon free.
#[test]
fn summon_friend_charges_the_target_prompts_them_and_teleports_on_accept() {
    use crate::game_loop::skills::effects::control::{accept_summon_request, call_pc_player};
    use crate::model::components::Position;
    use crate::model::inventory::Inventory;
    use crate::network::server_packets::opcodes;

    const TARGET: i32 = 5931;
    const TCID: u32 = 2;
    /// Summoning Crystal, the toll Summon Friend declares.
    const CRYSTAL: i32 = 8615;

    let build = |crystals: i64| {
        let (mut world, _a, _b, _c) = dist_world();
        let crx = ingame_player_access(&mut world, CID, CASTER, 0);
        let trx = ingame_player_access(&mut world, TCID, TARGET, 0);
        if crystals > 0 {
            let mut inv = world
                .objects
                .get_component::<Inventory>(&TARGET)
                .expect("inventory")
                .clone();
            inv.add_item(&world.data.item_data, 0x7700_0000, CRYSTAL, crystals);
            world.objects.add_components(&TARGET, inv);
        }
        // Somewhere distinct, so a teleport is visible.
        if let Some(p) = world.objects.get_component_mut::<Position>(&CASTER) {
            p.x = 15_000;
            p.y = 15_000;
            p.z = -2_000;
        }
        (world, crx, trx)
    };
    let at_caster = |w: &World| {
        let p = w.objects.get_component::<Position>(&TARGET).unwrap();
        (p.x, p.y) == (15_000, 15_000)
    };
    let prompted = |packets: &[Vec<u8>]| packets.iter().any(|p| p[0] == opcodes::CONFIRM_DLG);

    // No crystal: refused, told so, and no prompt.
    let (mut world, _crx, mut trx) = build(0);
    drain(&mut trx);
    call_pc_player(&mut world, CASTER, TARGET, CRYSTAL, 1);
    let out = drain(&mut trx);
    assert!(
        has_system_message(&out, sm_ids::S1_IS_REQUIRED_FOR_SUMMONING),
        "the *target* is told they are short, not the summoner"
    );
    assert!(!prompted(&out), "and nothing is asked of them");

    // With the crystal: charged up front, then prompted.
    let (mut world, _crx, mut trx) = build(1);
    drain(&mut trx);
    call_pc_player(&mut world, CASTER, TARGET, CRYSTAL, 1);
    let out = drain(&mut trx);
    assert!(prompted(&out), "the target is asked");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&TARGET)
            .unwrap()
            .count_of(CRYSTAL),
        0,
        "and paid before answering — declining does not refund"
    );
    assert!(!at_caster(&world), "nobody has moved yet");

    // Answering someone else's prompt does nothing…
    assert!(accept_summon_request(&mut world, TARGET, CASTER + 99, true));
    assert!(
        !at_caster(&world),
        "the echoed requester id has to match the stashed summoner"
    );

    // …and the request is consumed either way, so the right answer now finds
    // nothing — a stale prompt cannot be replayed.
    assert!(!accept_summon_request(&mut world, TARGET, CASTER, true));
    assert!(!at_caster(&world));

    // The accepting path.
    let (mut world, _crx, _trx) = build(1);
    call_pc_player(&mut world, CASTER, TARGET, CRYSTAL, 1);
    assert!(accept_summon_request(&mut world, TARGET, CASTER, true));
    assert!(at_caster(&world), "yes teleports them to the cast site");

    // Declining consumes the request and moves nobody.
    let (mut world, _crx, _trx) = build(1);
    call_pc_player(&mut world, CASTER, TARGET, CRYSTAL, 1);
    assert!(accept_summon_request(&mut world, TARGET, CASTER, false));
    assert!(!at_caster(&world), "no means no");
}
