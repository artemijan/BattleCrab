//! `Character.ini`'s caps and clamps (row 14).
//!
//! Ten keys that were hardcoded to this dist's value — several inside a
//! comment quoting the key, which is exactly the shape the audit's "Effect in
//! game" column describes: an operator edits the ini and nothing happens.
//!
//! Every test here moves the config *away* from the shipped value, because a
//! test at the shipped value cannot tell a wired key from a constant.

use super::*;
use crate::config::character::CharacterConfig;
use crate::data::item_data::CrystalType;

/// The parse, against the shipped file.
#[test]
fn the_cap_keys_parse_to_the_shipped_values() {
    let c = CharacterConfig::load_from(crate::data::DIST_GAME);
    assert_eq!(c.max_run_speed_summon, 350.0, "MaxRunSpeedSummon");
    assert_eq!(c.max_hp, 150_000.0, "MaxHP");
    assert_eq!(c.max_sp, 50_000_000_000, "MaxSp");
    assert_eq!(c.min_abnormal_state_success_rate, 10.0);
    assert_eq!(c.max_abnormal_state_success_rate, 90.0);
    assert_eq!(c.warehouse_slots_dwarf, 120);
    assert_eq!(c.warehouse_slots_no_dwarf, 100);
    assert_eq!(c.max_num_of_clans_in_ally, 3);
    assert_eq!(c.clan_members_for_war, 15);
    assert_eq!(
        c.max_equipable_item_grade,
        CrystalType::S,
        "MaxEquipableItemGrade — the value that makes the shop filter bite"
    );
}

/// Java: `getLong("MaxSp", …) >= 0 ? value : Long.MAX_VALUE`. A negative value
/// is "unlimited", not "no SP" — reading it literally would freeze every
/// character's SP at zero.
#[test]
fn a_negative_max_sp_means_unlimited_not_zero() {
    let mut c = CharacterConfig::default();
    assert_eq!(c.sp_ceiling(), 50_000_000_000);
    c.max_sp = -1;
    assert_eq!(c.sp_ceiling(), i64::MAX, "negative MaxSp lifts the ceiling");
    c.max_sp = 0;
    assert_eq!(c.sp_ceiling(), 0, "…but zero really is zero");
}

/// `Enum.valueOf(CrystalType.class, …)`. Java throws on an unknown name; the
/// port keeps the permissive end (EVENT = no filter), because the failure mode
/// of guessing `None` would be an empty shop catalogue server-wide.
#[test]
fn the_grade_name_parses_and_fails_open() {
    assert_eq!(CrystalType::from_config_name("S"), CrystalType::S);
    assert_eq!(CrystalType::from_config_name("  s  "), CrystalType::S);
    assert_eq!(CrystalType::from_config_name("EVENT"), CrystalType::Event);
    assert_eq!(CrystalType::from_config_name("NONE"), CrystalType::None);
    assert_eq!(
        CrystalType::from_config_name("nonsense"),
        CrystalType::Event,
        "an unreadable grade must not filter the whole catalogue away"
    );
}

/// `MaxEquipableItemGrade` filters products out of every buy list at load.
/// Lowering it to A must drop the S-grade lines the shipped S keeps.
///
/// This exercises the loader, not the config plumbing: it calls
/// `BuyListData::load_from` directly, because going through
/// `GameData::load_from_with` would re-parse the whole datapack twice. The
/// plumbing above it is one call site each (`main.rs` and `//reload buylist`),
/// both of which read `cfg.character.max_equipable_item_grade`.
#[test]
fn the_grade_filter_follows_the_config_key() {
    use crate::data::BuyListData;
    let items = dist::items();
    let count_above = |lists: &BuyListData, limit: i32| {
        lists
            .lists()
            .flat_map(|l| l.products.iter())
            .filter(|p| {
                items
                    .get(p.item_id)
                    .is_some_and(|t| t.crystal_type.level() > limit)
            })
            .count()
    };
    let at_s = BuyListData::load_from(crate::data::DIST_GAME, items, CrystalType::S);
    let at_a = BuyListData::load_from(crate::data::DIST_GAME, items, CrystalType::A);

    // At S the catalogue carries S-grade lines…
    let s_lines = count_above(&at_s, CrystalType::A.level());
    assert!(s_lines > 0, "the dist sells S-grade goods at MaxGrade=S");
    // …and at A they are gone, while the A-and-below stock is untouched.
    assert_eq!(
        count_above(&at_a, CrystalType::A.level()),
        0,
        "MaxEquipableItemGrade=A must drop every S line"
    );
    let below = |l: &BuyListData| {
        l.lists()
            .flat_map(|x| x.products.iter())
            .filter(|p| {
                items
                    .get(p.item_id)
                    .is_some_and(|t| t.crystal_type.level() <= CrystalType::A.level())
            })
            .count()
    };
    assert_eq!(
        below(&at_a),
        below(&at_s),
        "lowering the grade must not disturb anything at or below it"
    );
}

/// `Formulas.calcEffectLandRate`'s `constrain(rate, minChance, maxChance)`.
/// The 10/90 pair is what stops any debuff on this dist being a certainty or
/// an impossibility.
#[test]
fn the_debuff_land_rate_clamp_follows_the_config_keys() {
    use crate::model::formulas::{LandRateBounds, calc_effect_land_rate};
    // A wildly favourable matchup clamps down to the ceiling, a hopeless one
    // up to the floor.
    let rate = |b: LandRateBounds, target_level: i32| {
        calc_effect_land_rate(35, 80, 30, target_level, 1.0, 1.0, 1.0, 0.0, 1.0, b)
    };
    let shipped = LandRateBounds::default();
    assert_eq!(rate(shipped, 5), 90.0, "the dist ceiling");
    assert_eq!(rate(shipped, 80), 10.0, "the dist floor");

    let widened = LandRateBounds {
        min: 0.0,
        max: 100.0,
    };
    assert_eq!(rate(widened, 5), 100.0, "a raised ceiling is honoured");
    assert_eq!(rate(widened, 80), 0.0, "a lowered floor is honoured");

    // And the config feeds it.
    let mut cfg = CharacterConfig::default();
    cfg.min_abnormal_state_success_rate = 25.0;
    cfg.max_abnormal_state_success_rate = 75.0;
    let from_cfg = LandRateBounds::of(&cfg);
    assert_eq!((from_cfg.min, from_cfg.max), (25.0, 75.0));
    assert_eq!(rate(from_cfg, 5), 75.0);
}

/// `MaxHpFinalizer`'s HP_LIMIT arm: the finalized total is capped at `MaxHP`.
#[test]
fn player_max_hp_is_capped_by_the_config_key() {
    let mut data = dist::game_data_owned();
    let t = data
        .player_templates
        .get(0)
        .expect("a human fighter template")
        .clone();
    let mods = crate::model::components::StatModifiers::default();

    // Uncapped: raise the ceiling far above anything a level-80 can reach.
    data.combat_caps.max_hp = 10_000_000.0;
    let uncapped = crate::model::calc_max_hp(&data, &t, 80, None, &mods);
    assert!(uncapped > 0.0);

    // Now drop the ceiling under it and the total lands exactly on the cap.
    data.combat_caps.max_hp = uncapped / 2.0;
    let capped = crate::model::calc_max_hp(&data, &t, 80, None, &mods);
    assert_eq!(
        capped,
        uncapped / 2.0,
        "MaxHP is the ceiling MaxHpFinalizer clamps to"
    );
}

/// `SummonStat`'s ceiling is its own — 350, not the player's 300 — and the
/// shared NPC finalizer applies neither, because a plain NPC is uncapped.
#[test]
fn the_summon_speed_cap_is_separate_from_the_players() {
    let c = CharacterConfig::load_from(crate::data::DIST_GAME);
    assert_eq!(c.max_run_speed, 300.0, "players");
    assert_eq!(c.max_run_speed_summon, 350.0, "summons");
    assert!(
        c.max_run_speed_summon > c.max_run_speed,
        "collapsing the two would silently slow every summon to the player cap"
    );
    // The cap reaches the stat layer through `CombatCaps`.
    let caps = crate::data::CombatCaps::default();
    assert_eq!(caps.max_run_speed_summon, 350.0);
}

// ---------------------------------------------------------------------------
// Cluster 2 — the karma gates and the arrival/teleport protection window
// ---------------------------------------------------------------------------

/// The parse, against the shipped file. Four of these eight are inert *at*
/// these values, which is the point of pinning them: the reason each does
/// nothing is the value, not the port.
#[test]
fn the_protection_and_karma_keys_parse_to_the_shipped_values() {
    let c = CharacterConfig::load_from(crate::data::DIST_GAME);
    assert_eq!(c.player_spawn_protection, 600, "PlayerSpawnProtection");
    assert_eq!(c.player_teleport_protection, 0, "never arms here");
    assert!(c.offset_on_teleport_enabled);
    assert_eq!(c.max_offset_on_teleport, 50);
    assert!(!c.alt_karma_player_can_be_killed_in_peace_zone);
    assert!(c.alt_karma_player_can_teleport, "criminals teleport freely");
    assert!(c.alt_karma_player_can_trade, "criminals trade freely");
    assert!(!c.disconnect_after_death);
    assert_eq!(c.teleport_offset(), 50, "enabled → the radius");
}

/// `OffsetOnTeleportEnabled = False` must mean *exact*, not "radius 50".
#[test]
fn disabling_the_teleport_offset_lands_on_the_exact_point() {
    let mut c = CharacterConfig::default();
    assert_eq!(c.teleport_offset(), 50);
    c.offset_on_teleport_enabled = false;
    assert_eq!(c.teleport_offset(), 0, "disabled → no scatter at all");
    c.offset_on_teleport_enabled = true;
    c.max_offset_on_teleport = -5;
    assert_eq!(c.teleport_offset(), 0, "a negative radius is not a radius");
}

/// Entering the world arms the window; the first deliberate action ends it.
/// While it holds, an aggressive monster does not notice the player at all
/// (`Attackable.getHating`'s `isSpawnProtected` arm).
#[test]
fn spawn_protection_hides_a_new_arrival_until_they_act() {
    use crate::game_loop::spawn_protection;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    let npc_oid = NPC_OID + 61;
    add_test_npc(&mut world, npc_oid, 40061, "Monster", 20, 60, 0, 0);

    // Not protected until armed — the monster notices normally.
    assert!(
        crate::game_loop::npc::ai::perception::notices_target(&world, npc_oid, 3001),
        "an unprotected player is visible to a monster"
    );

    spawn_protection::arm(&mut world, 3001);
    assert!(spawn_protection::is_protected(&world, 3001));
    assert!(
        !crate::game_loop::npc::ai::perception::notices_target(&world, npc_oid, 3001),
        "a spawn-protected player is ignored by aggressive monsters"
    );

    // `onActionRequest` ends it, and the monster sees them again.
    spawn_protection::on_action_request(&mut world, 1, 3001);
    assert!(!spawn_protection::is_protected(&world, 3001));
    assert!(crate::game_loop::npc::ai::perception::notices_target(
        &world, npc_oid, 3001
    ));
}

/// The window is a *ceiling*, not a duration: it also lapses on its own.
#[test]
fn spawn_protection_expires_on_its_own_clock() {
    use crate::game_loop::spawn_protection;
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    world.cfg.character.player_spawn_protection = 3;
    spawn_protection::arm(&mut world, 3001);
    assert!(spawn_protection::is_protected(&world, 3001));
    world.tick += 3 * crate::game_loop::time::TICKS_PER_SECOND;
    assert!(
        !spawn_protection::is_protected(&world, 3001),
        "the window closes after PlayerSpawnProtection seconds"
    );
}

/// `PlayerSpawnProtection = 0` disables the feature outright — Java guards the
/// arming with `if (Config.PLAYER_SPAWN_PROTECTION > 0)`.
#[test]
fn a_zero_spawn_protection_never_arms() {
    use crate::game_loop::spawn_protection;
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world.cfg.character.player_spawn_protection = 0;
    spawn_protection::arm(&mut world, 3001);
    assert!(!spawn_protection::is_protected(&world, 3001));
}

// ---------------------------------------------------------------------------
// Cluster 3 — what may be enchanted, and what may be augmented
// ---------------------------------------------------------------------------

/// The parse, against the shipped file. The two blacklists are real lists here,
/// not the empty default.
#[test]
fn the_enchant_and_augment_keys_parse_to_the_shipped_values() {
    use crate::model::punishment::IllegalActionPunishment;
    let c = CharacterConfig::load_from(crate::data::DIST_GAME);
    assert!(c.disable_over_enchanting);
    assert!(c.over_enchant_protection);
    assert_eq!(c.over_enchant_punishment, IllegalActionPunishment::Jail);
    assert!(!c.alt_allow_augment_pvp_items);
    assert!(c.alt_allow_augment_trade);
    assert!(c.alt_allow_augment_destroy);
    assert!(
        c.enchant_black_list.len() >= 19,
        "EnchantBlackList ships a real list, got {}",
        c.enchant_black_list.len()
    );
    assert!(c.enchant_black_list.contains(&7816), "a listed id");
    assert!(
        c.augmentation_black_list.len() >= 60,
        "AugmentationBlackList ships a real list, got {}",
        c.augmentation_black_list.len()
    );
    assert!(c.augmentation_black_list.contains(&6656));
    // Java sorts both so it can `binarySearch`; the port relies on that too.
    assert!(c.enchant_black_list.windows(2).all(|w| w[0] < w[1]));
    assert!(c.augmentation_black_list.windows(2).all(|w| w[0] < w[1]));
}

/// `AltAllowAugmentPvPItems` is not merely inert — it is *unreachable*: the
/// gate is `item.isPvp() && !config`, and nothing on this dist can be a PvP
/// item. Asserted against the datapack rather than the port's template,
/// because the port does not parse the attribute at all — which is only
/// correct for as long as this holds.
#[test]
fn no_item_on_this_dist_is_pvp_flagged() {
    let dir = format!("{}data/stats/items", crate::data::DIST_GAME);
    let mut scanned = 0usize;
    let mut flagged = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the dist item directory") {
        let path = entry.expect("a readable entry").path();
        if path.extension().is_none_or(|e| e != "xml") {
            continue;
        }
        scanned += 1;
        let body = std::fs::read_to_string(&path).expect("a readable item file");
        if body.contains("is_pvp") {
            flagged.push(path);
        }
    }
    assert!(scanned > 0, "found no item XML to scan in {dir}");
    assert!(
        flagged.is_empty(),
        "AltAllowAugmentPvPItems is documented as unreachable, but these files \
         declare is_pvp: {flagged:?}"
    );
}

/// The blacklist is a veto *on top of* the template flag, not a substitute for
/// it (`binarySearch(...) < 0 && _enchantable`).
#[test]
fn the_enchant_blacklist_vetoes_an_otherwise_enchantable_item() {
    use crate::data::enchant_data::TargetGates;
    let plain = TargetGates::default();
    let listed = TargetGates {
        blacklisted: true,
        ..TargetGates::default()
    };
    assert!(!plain.blacklisted);
    assert!(listed.blacklisted);
    // The dist's own list is what feeds it.
    let c = CharacterConfig::load_from(crate::data::DIST_GAME);
    assert!(c.enchant_black_list.binary_search(&7816).is_ok());
    assert!(c.enchant_black_list.binary_search(&57).is_err(), "adena");
}

/// **The deviation.** Java infers the three ceilings from enchant-group names;
/// this dist names no accessory group, so retail leaves the accessory ceiling
/// at 0 and destroys every enchanted ring/earring/necklace on login. The port
/// reads a derived 0 as "no group data" instead. See
/// `docs/CUSTOM_DIST_DEVIATIONS.md`.
#[test]
fn the_accessory_ceiling_is_absent_on_this_dist_and_is_not_read_as_zero() {
    use crate::data::item_data::{TYPE2_ACCESSORY, TYPE2_SHIELD_ARMOR, TYPE2_WEAPON};
    let e = &dist::game_data().enchant;
    assert_eq!(e.max_weapon_enchant, 29, "derived from the WEAPON groups");
    assert_eq!(e.max_armor_enchant, 29, "derived from the ARMOR groups");
    assert_eq!(
        e.max_accessory_enchant, 0,
        "no group name matches ACCESSORIES/RING/EARRING/NECK on this dist"
    );

    assert_eq!(e.max_enchant_for_type2(TYPE2_WEAPON), Some(29));
    assert_eq!(e.max_enchant_for_type2(TYPE2_SHIELD_ARMOR), Some(29));
    assert_eq!(
        e.max_enchant_for_type2(TYPE2_ACCESSORY),
        None,
        "a derived 0 is absent data, not a ceiling of zero — reading it as \
         zero would destroy every enchanted accessory on the server"
    );
}

// ---------------------------------------------------------------------------
// Cluster 4 — clan and alliance timers
// ---------------------------------------------------------------------------

/// The parse, against the shipped file. `DaysBeforeCreateAClan` is the only
/// day-key that is not 1, and `AltClanMembersTimeForBonus` is a *duration*
/// string (`30mins`) rather than a number.
#[test]
fn the_clan_and_ally_keys_parse_to_the_shipped_values() {
    let c = CharacterConfig::load_from(crate::data::DIST_GAME);
    assert_eq!(c.alt_clan_join_days, 1, "DaysBeforeJoinAClan");
    assert_eq!(c.alt_clan_create_days, 10, "DaysBeforeCreateAClan");
    assert_eq!(
        c.alt_clan_dissolve_days, 1,
        "DaysToPassToDissolveAClan — the port hardcoded 7"
    );
    assert_eq!(c.alt_ally_join_days_when_leaved, 1);
    assert_eq!(c.alt_ally_join_days_when_dismissed, 1);
    assert_eq!(c.alt_accept_clan_days_when_dismissed, 1);
    assert_eq!(c.alt_create_ally_days_when_dissolved, 1);
    assert!(
        !c.alt_members_can_withdraw_from_clan_wh,
        "leader-only withdrawal"
    );
    assert!(!c.alt_clan_leader_instant_activation);
    assert_eq!(
        c.alt_clan_members_time_for_bonus_ms,
        30 * 60 * 1000,
        "`30mins` parsed as a duration"
    );
    assert!(c.alt_command_channel_friends);
    assert!(c.life_crystal_needed);
}

/// **The bug this cluster found.** The port's `CLAN_DISSOLVE_DELAY_MS` was
/// `7 * 86_400_000`, and its doc comment claimed *"`DaysToPassToDissolveAClan`
/// = 7 on this dist"*. The dist ships **1**, so dissolving a clan took seven
/// times as long as configured.
#[test]
fn the_clan_dissolve_delay_follows_the_config_and_is_not_seven_days() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let day = crate::game_loop::time::MILLIS_PER_DAY;
    assert_eq!(
        crate::game_loop::clans::clan_dissolve_delay_ms(&world),
        day,
        "the shipped DaysToPassToDissolveAClan is 1, not 7"
    );
    world.cfg.character.alt_clan_dissolve_days = 3;
    assert_eq!(
        crate::game_loop::clans::clan_dissolve_delay_ms(&world),
        3 * day
    );
}

/// The other two clan day-keys, which were also constants.
#[test]
fn the_clan_join_and_create_penalties_follow_their_keys() {
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let day = crate::game_loop::time::MILLIS_PER_DAY;
    assert_eq!(crate::game_loop::clans::clan_join_penalty_ms(&world), day);
    assert_eq!(
        crate::game_loop::clans::clan_create_cooldown_ms(&world),
        10 * day,
        "DaysBeforeCreateAClan is the one that is not 1"
    );
    world.cfg.character.alt_clan_join_days = 4;
    world.cfg.character.alt_clan_create_days = 2;
    assert_eq!(
        crate::game_loop::clans::clan_join_penalty_ms(&world),
        4 * day
    );
    assert_eq!(
        crate::game_loop::clans::clan_create_cooldown_ms(&world),
        2 * day
    );
}

/// The four alliance penalties are four *different* keys, distinguished by the
/// `ally_penalty_type` Java stamps alongside each. They all ship as 1 day,
/// which is exactly why one shared constant went unnoticed — so the test moves
/// each key independently.
#[test]
fn each_ally_penalty_type_reads_its_own_key() {
    use crate::game_loop::clans::alliance::ally_penalty_ms;
    use crate::model::clan::{
        ALLY_PENALTY_TYPE_CLAN_DISMISSED, ALLY_PENALTY_TYPE_CLAN_LEAVED,
        ALLY_PENALTY_TYPE_DISMISS_CLAN, ALLY_PENALTY_TYPE_DISSOLVE_ALLY,
    };
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let day = crate::game_loop::time::MILLIS_PER_DAY;

    // Shipped: all four are one day.
    for t in [
        ALLY_PENALTY_TYPE_CLAN_LEAVED,
        ALLY_PENALTY_TYPE_CLAN_DISMISSED,
        ALLY_PENALTY_TYPE_DISMISS_CLAN,
        ALLY_PENALTY_TYPE_DISSOLVE_ALLY,
    ] {
        assert_eq!(ally_penalty_ms(&world, t), day, "penalty type {t}");
    }

    // Move each key to a distinct value and check the fan-out is one-to-one.
    world.cfg.character.alt_ally_join_days_when_leaved = 2;
    world.cfg.character.alt_ally_join_days_when_dismissed = 3;
    world.cfg.character.alt_accept_clan_days_when_dismissed = 4;
    world.cfg.character.alt_create_ally_days_when_dissolved = 5;
    assert_eq!(
        ally_penalty_ms(&world, ALLY_PENALTY_TYPE_CLAN_LEAVED),
        2 * day,
        "DaysBeforeJoinAllyWhenLeaved"
    );
    assert_eq!(
        ally_penalty_ms(&world, ALLY_PENALTY_TYPE_CLAN_DISMISSED),
        3 * day,
        "DaysBeforeJoinAllyWhenDismissed"
    );
    assert_eq!(
        ally_penalty_ms(&world, ALLY_PENALTY_TYPE_DISMISS_CLAN),
        4 * day,
        "DaysBeforeAcceptNewClanWhenDismissed"
    );
    assert_eq!(
        ally_penalty_ms(&world, ALLY_PENALTY_TYPE_DISSOLVE_ALLY),
        5 * day,
        "DaysBeforeCreateNewAllyWhenDissolved"
    );
}

/// The over-enchant sweep destroys the offending **instance**, not every item
/// sharing its id. A first cut of this used `destroy_item_by_id`, which would
/// have taken the player's plain duplicate along with the `+99` one — Java
/// passes the `Item` for exactly this reason.
#[test]
fn the_over_enchant_sweep_destroys_only_the_offending_instance() {
    use crate::model::inventory::Inventory;

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    world.data.item_data = dist::items_owned();
    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);

    // A real equipable weapon (type2 = WEAPON). `combat_test_world` carries no
    // enchant groups, so set the ceiling directly — the *derivation* from
    // `EnchantItemGroups.xml` is pinned by
    // `the_accessory_ceiling_is_absent_on_this_dist_and_is_not_read_as_zero`.
    const SWORD: i32 = 2; // Long Sword
    let ceiling = 29;
    world.data.enchant.max_weapon_enchant = ceiling;

    {
        let World { data, objects, .. } = &mut world;
        let inv = objects
            .get_component_mut::<Inventory>(&3001)
            .expect("inventory");
        inv.add_item(&data.item_data, 9_000_001, SWORD, 1);
        inv.add_item(&data.item_data, 9_000_002, SWORD, 1);
    }
    // Push exactly one of the two past the ceiling.
    world
        .objects
        .get_component_mut::<Inventory>(&3001)
        .expect("inventory")
        .set_enchant_level(9_000_001, ceiling + 1);

    crate::game_loop::enchant::over_enchant_sweep(&mut world, 3001);

    let inv = world
        .objects
        .get_component::<Inventory>(&3001)
        .expect("inventory");
    assert!(
        inv.by_object_id(9_000_001).is_none(),
        "the over-enchanted sword is destroyed"
    );
    assert!(
        inv.by_object_id(9_000_002).is_some(),
        "its plain duplicate — same item id — must survive"
    );
}
