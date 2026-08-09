//! Carried weight: the CON-derived limit, Java's penalty ladder, the 4270
//! passive, and the gates that were blocked on all of it.

use super::*;

use crate::game_loop::weight;
use crate::model::components::{BaseStats, Buffs};
use crate::model::inventory::Inventory;

const PLAYER: i32 = 4400;
const CID: u32 = 1;
const DIST: &str = crate::data::DIST_GAME;
/// A heavy, stackable, non-quest item to load the character up with.
const BRICK: i32 = 9600;

fn weight_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = cast_test_world();
    let mut t = crate::data::item_data::ItemTemplate::default();
    t.item_id = BRICK;
    t.name = "Brick".into();
    t.weight = 1000;
    t.is_stackable = true;
    world.data.item_data.insert_for_test(t);
    // The 4270 passive as the dist writes it: the level-4 rung halves nothing
    // and stops the character dead (`Speed -100%`).
    world.data.skill_data = crate::data::skill_data::SkillData::load_from(DIST);
    // The synthetic world uses `StatBonus::empty()`, where every bonus is 1.0 —
    // so the CON-derived limit could not vary and the test below would be
    // asserting against a constant.
    world.data.stat_bonus = crate::data::StatBonus::load_from(DIST);
    (world, db, l)
}

fn carry(world: &mut World, count: i64) {
    let World { data, objects, .. } = world;
    objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .add_item(&data.item_data, 7_400_001, BRICK, count);
}

// ---------------------------------------------------------------------------
// The ladder (pure)
// ---------------------------------------------------------------------------

/// Java's exact bands, including the boundaries — the interesting part, since
/// each rung is `<` not `<=`.
#[test]
fn the_penalty_ladder_matches_javas_bands() {
    let max = 1000;
    for (load, expected) in [
        (0, 0),
        (499, 0),
        (500, 1),
        (665, 1),
        (666, 2),
        (799, 2),
        (800, 3),
        (999, 3),
        (1000, 4),
        (5000, 4),
    ] {
        assert_eq!(
            weight::penalty_level(load, max, false),
            expected,
            "{load}/1000 permille"
        );
    }
}

/// `//diet` collapses the whole ladder to 0 — the GM immunity that had no
/// reader in this port until the weight calc existed.
#[test]
fn diet_mode_suppresses_every_band() {
    for load in [500, 800, 1000, 100_000] {
        assert_eq!(
            weight::penalty_level(load, 1000, true),
            0,
            "diet mode is immune at {load}"
        );
    }
}

/// A zero limit yields no penalty rather than dividing by zero (Java's
/// `if (maxLoad > 0)` guard).
#[test]
fn a_zero_limit_is_not_a_division_by_zero() {
    assert_eq!(weight::penalty_level(10_000, 0, false), 0);
}

// ---------------------------------------------------------------------------
// The limit and the passive
// ---------------------------------------------------------------------------

/// `maxLoad` scales with CON, and honours `AltWeightLimit` — **3** on this
/// dist, so a hard-coded 1.0 would leave everyone overloaded.
#[test]
fn the_limit_scales_with_con_and_the_config_multiplier() {
    let (mut world, _db, _l) = weight_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);

    world.cfg.character.alt_weight_limit = 1.0;
    let single = weight::max_load(&world, PLAYER);
    world.cfg.character.alt_weight_limit = 3.0;
    let triple = weight::max_load(&world, PLAYER);

    assert!(single > 0, "a real limit is derived from CON");
    assert_eq!(triple, single * 3, "AltWeightLimit multiplies it");

    // Raising CON raises the limit.
    let before = triple;
    world
        .objects
        .get_component_mut::<BaseStats>(&PLAYER)
        .unwrap()
        .con += 10;
    assert!(
        weight::max_load(&world, PLAYER) > before,
        "more CON carries more"
    );
}

/// Loading a character up applies skill 4270 at the matching level, and
/// unloading removes it. The passive is what carries the speed malus, so this
/// is the observable half of the feature.
#[test]
fn crossing_a_band_applies_and_removes_the_4270_passive() {
    let (mut world, _db, _l) = weight_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let max = weight::max_load(&world, PLAYER);
    assert!(max > 0);

    let has_4270 = |w: &World| {
        w.objects.get_component::<Buffs>(&PLAYER).and_then(|b| {
            b.0.iter()
                .find(|a| a.skill_id == 4270)
                .map(|a| a.skill_level)
        })
    };

    weight::refresh_weight_penalty(&mut world, PLAYER);
    assert_eq!(has_4270(&world), None, "an empty bag carries no penalty");

    // Just over half the limit → band 1.
    carry(&mut world, (max as i64 / 1000 / 2) + 1);
    weight::refresh_weight_penalty(&mut world, PLAYER);
    assert_eq!(has_4270(&world), Some(1), "past 50% → penalty level 1");

    // Past the limit entirely → band 4, and overloaded.
    carry(&mut world, max as i64 / 1000);
    weight::refresh_weight_penalty(&mut world, PLAYER);
    assert_eq!(has_4270(&world), Some(4), "past 100% → penalty level 4");
    assert!(weight::is_overloaded(&world, PLAYER), "and overloaded");

    // Drop it all: the passive goes away rather than lingering at level 4.
    world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap()
        .remove_by_object_id(7_400_001, i64::MAX);
    weight::refresh_weight_penalty(&mut world, PLAYER);
    assert_eq!(has_4270(&world), None, "unloading clears the penalty");
    assert!(!weight::is_overloaded(&world, PLAYER));
}

/// Stepping between bands must not stack both levels' maluses — the reason the
/// apply path removes before it re-adds.
#[test]
fn stepping_between_bands_replaces_rather_than_stacks() {
    let (mut world, _db, _l) = weight_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let max = weight::max_load(&world, PLAYER);

    carry(&mut world, (max as i64 / 1000 / 2) + 1);
    weight::refresh_weight_penalty(&mut world, PLAYER);
    carry(&mut world, max as i64 / 1000);
    weight::refresh_weight_penalty(&mut world, PLAYER);

    let count = world
        .objects
        .get_component::<Buffs>(&PLAYER)
        .map_or(0, |b| b.0.iter().filter(|a| a.skill_id == 4270).count());
    assert_eq!(count, 1, "exactly one 4270 entry, not one per band crossed");
}

/// A dieting GM is never overloaded, however much they carry.
#[test]
fn a_dieting_gm_is_never_overloaded() {
    let (mut world, _db, _l) = weight_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    let max = weight::max_load(&world, PLAYER);
    carry(&mut world, (max as i64 / 1000) * 4);
    assert!(
        weight::is_overloaded(&world, PLAYER),
        "baseline: this load overloads a normal character"
    );

    let mut flags = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&PLAYER)
        .copied()
        .unwrap_or_default();
    flags.diet = true;
    world.objects.add_components(&PLAYER, flags);

    assert!(!weight::is_overloaded(&world, PLAYER), "//diet is immune");
    assert_eq!(weight::current_penalty(&world, PLAYER), 0);
}

/// `isInventoryUnder80` counts **slots**, not weight — a different limit from
/// everything above, and the one TvT's registration gate reads.
#[test]
fn the_inventory_slot_gate_is_about_slots_not_weight() {
    let (mut world, _db, _l) = weight_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    assert!(
        weight::is_inventory_under_80(&world, PLAYER),
        "an empty bag is under 80%"
    );

    // One very heavy stack is one slot: heavy, but not *full*.
    carry(&mut world, 10_000);
    assert!(
        weight::is_inventory_under_80(&world, PLAYER),
        "weight does not fill slots"
    );
    assert!(
        weight::is_overloaded(&world, PLAYER),
        "though it certainly overloads"
    );

    // Fill past 80% of the limit with distinct items instead.
    let limit = weight::inventory_limit(&world, PLAYER);
    for i in 0..=(limit * 8 / 10) {
        let id = BRICK + 1 + i;
        let mut t = crate::data::item_data::ItemTemplate::default();
        t.item_id = id;
        t.name = format!("Trinket {i}");
        world.data.item_data.insert_for_test(t);
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&PLAYER)
            .unwrap()
            .add_item(&data.item_data, 7_500_000 + i, id, 1);
    }
    assert!(
        !weight::is_inventory_under_80(&world, PLAYER),
        "enough distinct items does fill it"
    );
}

/// Being overloaded **roots you** — Java folds `_isOverloaded` into
/// `Creature.isMovementDisabled()` beside the crowd-control flags. The 4270
/// passive only slows; this is what actually stops a mule.
#[test]
fn being_overloaded_disables_movement() {
    let (mut world, _db, _l) = weight_world();
    let _out = ingame_caster(&mut world, CID, PLAYER, 0, 0);
    assert!(
        !crate::game_loop::abnormal::is_movement_disabled(&world, PLAYER),
        "baseline: an unloaded character may move"
    );

    let max = weight::max_load(&world, PLAYER);
    carry(&mut world, (max as i64 / 1000) + 1);

    assert!(
        crate::game_loop::abnormal::is_movement_disabled(&world, PLAYER),
        "over the limit, movement is disabled"
    );
}

/// G34 S4 — `WeightLimit` (Weight Limit 150, Quiver of Holding 418, Super Haste
/// 7029) multiplies the CON-derived cap, and `WeightPenalty` (Decrease Weight
/// 1257, Master's Blessing 7049) comes off the **carried weight**.
///
/// The second is the one to get right: the effect is *named* for the penalty
/// band, but every Java caller subtracts it from `getCurrentLoad()` —
/// `weightproc = (getCurrentLoad() - getBonusWeightPenalty()) * 1000 / getMaxLoad()`
/// — and the datapack settles it, since Decrease Weight grants 3000/6000/9000,
/// which are weight units. Ported as the code behaves, not as the name reads.
#[test]
fn the_weight_stats_scale_the_cap_and_discount_the_load() {
    use crate::model::stats::Stat;
    let (mut world, ..) = test_world();
    let oid = 6101;
    let _rx = ingame_player_access(&mut world, 1, oid, 0);

    let base_max = crate::game_loop::weight::max_load(&world, oid);
    assert!(base_max > 0, "a CON-derived cap to scale");

    // `WeightLimit` is `PER` on every source: Weight Limit 3 is +300 %, i.e. ×4.
    let mut mods = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&oid)
        .cloned()
        .expect("stat modifiers");
    mods.mul.insert(Stat::WeightLimit, 4.0);
    world.objects.add_components(&oid, mods.clone());
    assert_eq!(
        crate::game_loop::weight::max_load(&world, oid),
        base_max * 4,
        "the cap scales with the stat"
    );

    // `WeightPenalty` is `DIFF`, and it discounts weight — base 1 with no
    // skill, so the unbuffed discount is 1.
    assert_eq!(
        crate::game_loop::weight::bonus_weight_penalty(&world, oid),
        1,
        "Java's base is 1, not 0"
    );
    mods.add.insert(Stat::WeightPenalty, 9000.0);
    world.objects.add_components(&oid, mods);
    assert_eq!(
        crate::game_loop::weight::bonus_weight_penalty(&world, oid),
        9001,
        "Decrease Weight 3 discounts 9000 units of carried weight"
    );
}
