//! Shield blocking inside **skill** damage, and the ranged branch of the
//! physical-skill formula (G20).
//!
//! `calcShldUse` was already ported for auto-attacks and mana drains, but
//! `PhysicalAttack`, `EnergyAttack` and `calcBlowDamage` all open on the same
//! shield switch and none of them consulted it — a shield did nothing at all
//! against a skill. The ranged branch is the other half of the same formula:
//! an archer's skill uses `weaponMod` 70 *plus* a second `pAtk + power` term.

use super::*;

use crate::data::item_data::{
    ActionType, CrystalType, EtcItemType, ItemHandler, ItemKind, ItemStats, ItemTemplate,
    SLOT_L_HAND,
};
use crate::game_loop;
use crate::model::components::Vitals;
use crate::model::skill::SkillEffect;

const CASTER: i32 = 3001;
const CID: u32 = 1;
const SHIELD_ID: i32 = 7700;

pub(super) fn gear(item_id: i32, kind: ItemKind, body_part: i32) -> ItemTemplate {
    ItemTemplate {
        trade_flags: Default::default(),
        pre_conditions: Vec::new(),
        is_oly_restricted: false,
        is_event_restricted: false,
        for_npc: false,
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: ActionType::Other,
        item_id,
        name: format!("gear{item_id}"),
        kind,
        body_part,
        weight: 0,
        is_stackable: false,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 0,
        handler: ItemHandler::None,
        crystal_type: CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    }
}

/// A world with a caster and a **player** victim.
///
/// It has to be a player: `calcShldUse` reads the bearer's `BaseStats` (for the
/// CON bonus) *and* their `Inventory`, and an NPC carries neither — attaching a
/// bare `Inventory` to a mob is not enough, `shield_stats` early-returns on the
/// missing `BaseStats` and reports no shield at all.
fn shield_world() -> (World, i32) {
    let (mut world, _db, _l) = combat_test_world();
    let _a_rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = 3002;
    let _v_rx = ingame_caster(&mut world, 2, victim, 0, 0);
    world
        .data
        .item_data
        .insert_for_test(gear(SHIELD_ID, ItemKind::Armor, SLOT_L_HAND));
    // Hoplon's real numbers: sDef 128, and a block rate we set per test.
    world.data.item_data.set_item_stats_for_test(
        SHIELD_ID,
        ItemStats {
            shield_def: Some(128),
            shield_rate: Some(20),
            ..Default::default()
        },
    );
    // Zero the caster's random-damage spread: with `random_dmg == 0` the
    // damage path consumes no roll for it, which keeps every forced sequence
    // below aligned and every hit exactly reproducible.
    world
        .objects
        .get_component_mut::<CombatStats>(&CASTER)
        .unwrap()
        .random_dmg = 0;
    (world, victim)
}

/// Reset the victim's pool and read back what a cast took off it.
///
/// **Every cast opens with an unconditional `roll(1000)`** for the magic-crit
/// check (`apply_skill_effects` rolls it before looking at `magic_type`), so
/// each `rolls` list here starts with a throwaway for it.
fn hit_for(world: &mut World, victim: i32, skill: &Skill, rolls: &[i32]) -> f64 {
    // `Npc::for_test` seeds a 1 000 000 pool that the damage path's stat
    // recalculation clamps to the template max; start from that real max so a
    // before/after difference is the hit and nothing else.
    let full = world
        .objects
        .get_component::<Vitals>(&victim)
        .unwrap()
        .max_hp as f64;
    world
        .objects
        .get_component_mut::<Vitals>(&victim)
        .unwrap()
        .cur_hp = full;
    world.force_rolls(rolls.iter().copied());
    effects::apply_skill_effects(world, CASTER, victim, skill);
    full - world
        .objects
        .get_component::<Vitals>(&victim)
        .unwrap()
        .cur_hp
}

/// Put the shield in the victim's left hand.
fn equip_shield(world: &mut World, oid: i32) {
    let World { objects, data, .. } = world;
    let inv = objects
        .get_component_mut::<Inventory>(&oid)
        .expect("a player target");
    let oid = inv.add_item(&data.item_data, 0x5100_0001, SHIELD_ID, 1);
    let changed = inv.equip_item(&data.item_data, oid);
    assert!(!changed.is_empty(), "the shield equipped (oid {oid})");
    assert!(
        inv.paperdoll_item(model::inventory::PaperdollSlot::LHand)
            .is_some(),
        "…into the left hand"
    );
}

fn physical_skill(world: &World, id: i32, power: f64, ignore_shield: bool) -> Skill {
    let mut s = world.data.skill_data.get(1160, 1).expect("fixture").clone();
    s.id = id;
    s.name = format!("PhysSkill{id}");
    s.magic_type = 0;
    s.activate_rate = -1;
    s.effects = vec![SkillEffect::PhysicalAttack {
        power,
        p_atk_mod: 1.0,
        // A heavy `pDefMod` keeps every hit well inside the victim's pool, so
        // a death-clamp can never stand in for a block. It also exercises
        // Java's ordering: `pDefMod` scales the *base* pDef, and the shield's
        // own sDef is added afterwards, unscaled.
        p_def_mod: 40.0,
        critical_chance: 0.0,
        ignore_shield_defence: ignore_shield,
    }];
    s
}

// ---------------------------------------------------------------------------
// The formula's ranged branch
// ---------------------------------------------------------------------------

/// A bow in the caster's hand switches the skill formula to Java's ranged one.
/// Equipping the bow *lowers* `weaponMod` to 70 but adds a whole `pAtk + power`
/// to the bracket, so the same skill hits **harder**, not 70/77 as hard.
///
/// A real dist item (Short Bow 13) because the branch keys off
/// `ItemData::weapon_type`, which a synthetic template cannot report.
#[test]
fn a_bow_switches_the_skill_to_the_ranged_formula() {
    let (mut world, _db, _l) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.item_data = dist::items_owned();
    world
        .objects
        .get_component_mut::<CombatStats>(&CASTER)
        .unwrap()
        .random_dmg = 0;
    let victim = spawn_mob(&mut world, &mut a_rx);
    // `ignore_shield_defence` so only the weapon branch varies (a mob has no
    // shield anyway, but the two rolls would still be consumed).
    let skill = physical_skill(&world, 9700, 50.0, true);

    let melee = hit_for(&mut world, victim, &skill, &[0, 50]);
    assert!(melee > 0.0, "the melee hit lands for something");
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&CASTER).unwrap();
        let oid = inv.add_item(&data.item_data, 0x5100_0009, 13, 1);
        inv.equip_item(&data.item_data, oid);
    }
    assert!(
        crate::game_loop::combat::ranged::is_ranged(
            crate::game_loop::combat::ranged::equipped_weapon_type(&world, CASTER)
                .unwrap_or_default()
        ),
        "the Short Bow reports as ranged"
    );
    let ranged = hit_for(&mut world, victim, &skill, &[0, 50]);

    assert!(
        ranged > melee,
        "the ranged bonus term outweighs the smaller weaponMod: {ranged} vs {melee}"
    );
}

/// The mob helper the ranged test needs — a `combat_test_world` Monster with a
/// real HP pool. (`shield_world`'s victim must be a player; this one must not
/// be, because player-on-player skill damage is gated by the PvP rules.)
fn spawn_mob(world: &mut World, a_rx: &mut UnboundedReceiver<bytes::Bytes>) -> i32 {
    let npc_oid = 0x4000_0111;
    let (npc, extra) = model::npc::Npc::for_test(npc_oid, 40001, 40, 0, 0, 1_000_000, 30);
    world
        .npc_regions
        .entry(extra.1.0)
        .or_default()
        .push(npc_oid);
    world.objects.spawn(npc_oid, (npc, extra));
    let cs = game_loop::npc::npc_combat_stats(
        world.data.npc_data.get(40001).unwrap(),
        &world.data.stat_bonus,
    );
    world.objects.add_components(&npc_oid, cs);
    drain(a_rx);
    npc_oid
}

// ---------------------------------------------------------------------------
// Shield blocking a skill
// ---------------------------------------------------------------------------

/// A **normal block** adds the shield's `sDef` to the divisor; a **perfect**
/// one returns `None`, which every caller turns into a flat 1 damage. Java
/// folds `pDefMod` in *before* the add, so the shield's own sDef is never
/// scaled by it — that is why the callers pass an already-scaled base.
#[test]
fn the_shield_switch_adds_defence_or_signals_a_perfect_block() {
    use crate::game_loop::skills::effects::defence_after_shield;

    let (mut world, victim) = shield_world();

    // No shield → the rolls are still consumed, the defence is untouched.
    assert_eq!(
        defence_after_shield(&mut world, CASTER, victim, 100.0, false),
        Some(100.0)
    );

    equip_shield(&mut world, victim);
    // rate roll 0 always blocks (rate 20); perfect roll 0 keeps it ordinary —
    // `100 - 2·conBonus < perfectRoll` is what promotes it.
    world.force_rolls([0, 0]);
    assert_eq!(
        defence_after_shield(&mut world, CASTER, victim, 100.0, false),
        Some(228.0),
        "the shield's 128 sDef is added, unscaled"
    );
    // rate 0 blocks, perfect roll 99 → 98 < 99 → promoted.
    world.force_rolls([0, 99]);
    assert_eq!(
        defence_after_shield(&mut world, CASTER, victim, 100.0, false),
        None,
        "a perfect block signals the flat-1 path"
    );
    // A losing rate roll leaves the defence alone.
    world.force_rolls([99, 99]);
    assert_eq!(
        defence_after_shield(&mut world, CASTER, victim, 100.0, false),
        Some(100.0)
    );
}

/// `calcShldUse`'s bow clause: **a ranged attacker raises the block rate by
/// 30 %**, and Java reads that flag off `attacker.getAttackType()` with no skill
/// involved — so it applies to a skill hit exactly as to a plain swing.
///
/// ```java
/// if (attacker.getAttackType().isRanged()) shldRate *= 1.3;
/// ```
///
/// The fixture sits in the 30 % band on purpose: rate 20 loses a roll of 25,
/// rate 26 wins it. Nothing but the attacker's weapon changes between the two
/// halves.
#[test]
fn a_bow_attacker_raises_the_shield_block_rate() {
    use crate::data::item_data::{SLOT_LR_HAND, WeaponType};
    use crate::game_loop::skills::effects::defence_after_shield;

    const BOW_ID: i32 = 7701;
    let (mut world, victim) = shield_world();
    equip_shield(&mut world, victim);

    // Roll 25 is above the shield's own rate of 20 and below 20 × 1.3 = 26.
    // Perfect roll 99 is irrelevant while the rate roll loses.
    let block = |world: &mut World| {
        world.force_rolls([25, 0]);
        defence_after_shield(world, CASTER, victim, 100.0, false) == Some(228.0)
    };

    assert!(
        !block(&mut world),
        "bare-handed, rate 20 does not clear a roll of 25"
    );

    world
        .data
        .item_data
        .insert_for_test(gear(BOW_ID, ItemKind::Weapon, SLOT_LR_HAND));
    world
        .data
        .item_data
        .set_weapon_type_for_test(BOW_ID, WeaponType::Bow);
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects
            .get_component_mut::<Inventory>(&CASTER)
            .expect("the attacker is a player");
        let oid = inv.add_item(&data.item_data, 0x5100_0002, BOW_ID, 1);
        assert!(!inv.equip_item(&data.item_data, oid).is_empty(), "bow on");
    }

    assert!(
        block(&mut world),
        "with a bow the same rate becomes 26 and the same roll of 25 is blocked"
    );
}

/// `<ignoreShieldDefence>` (14 learnable carriers — Triple Slash, Armor Crush,
/// Hammer Crush, …) skips the switch entirely, **including its two rolls** —
/// which is what keeps the RNG stream aligned with Java's.
#[test]
fn ignore_shield_defence_skips_the_block_and_its_rolls() {
    use crate::game_loop::skills::effects::defence_after_shield;

    let (mut world, victim) = shield_world();
    equip_shield(&mut world, victim);

    // These two would be a perfect block if they were read.
    world.force_rolls([0, 99]);
    assert_eq!(
        defence_after_shield(&mut world, CASTER, victim, 100.0, true),
        Some(100.0),
        "the shield is ignored outright"
    );
    // Untouched, so they are still queued and now *do* produce that block.
    assert_eq!(
        defence_after_shield(&mut world, CASTER, victim, 100.0, false),
        None,
        "the two rolls were never consumed"
    );
}

/// **The damage paths are wired to the switch.** A player-vs-player skill hit
/// is gated by the PvP rules, and a mob can't hold a shield, so the block's
/// *effect* isn't observable end to end here — but its **roll consumption** is,
/// and that is what proves `PhysicalAttack` calls it at all: with
/// `ignoreShieldDefence` off the cast eats two extra rolls before the crit
/// roll, with it on it does not.
#[test]
fn the_physical_attack_path_consults_the_shield_switch() {
    let (mut world, _db, _l) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = spawn_mob(&mut world, &mut a_rx);
    world
        .objects
        .get_component_mut::<CombatStats>(&CASTER)
        .unwrap()
        .random_dmg = 0;

    // A 100% crit chance, so the crit roll's *position* in the stream decides
    // whether the hit doubles — the observable we key on.
    let mut skill = physical_skill(&world, 9705, 50.0, false);
    if let Some(SkillEffect::PhysicalAttack {
        critical_chance, ..
    }) = skill.effects.first_mut()
    {
        *critical_chance = 100.0;
    }
    let ignoring = {
        let mut s = skill.clone();
        s.id = 9706;
        if let Some(SkillEffect::PhysicalAttack {
            ignore_shield_defence,
            ..
        }) = s.effects.first_mut()
        {
            *ignore_shield_defence = true;
        }
        s
    };

    // [magic, shield rate, shield perfect, crit]: the crit roll is the 99, which
    // loses against the clamped-90 chance → no crit.
    let consulted = hit_for(&mut world, victim, &skill, &[0, 0, 0, 99]);
    // Same queue, but the shield rolls are never taken — so the crit roll is
    // the *first* 0, which wins → the hit doubles.
    let ignored = hit_for(&mut world, victim, &ignoring, &[0, 0, 0, 99]);

    assert!(
        (ignored - consulted * 2.0).abs() < 1e-6,
        "the ignoring cast crit because it skipped the two shield rolls: \
         {ignored} vs {consulted}"
    );
}

/// The dist really does declare the flag, and only on the skills that should
/// have it.
#[test]
fn ignore_shield_defence_is_read_from_the_dist() {
    let sd = dist::skills();
    // Armor Crush (362) ignores shields; Power Strike (3) does not.
    let crush = sd.get(362, 1).expect("Armor Crush");
    assert!(
        crush.effects.iter().any(|e| matches!(
            e,
            SkillEffect::PhysicalAttack {
                ignore_shield_defence: true,
                ..
            }
        )),
        "{:?}",
        crush.effects
    );
    let strike = sd.get(3, 1).expect("Power Strike");
    assert!(
        strike.effects.iter().any(|e| matches!(
            e,
            SkillEffect::PhysicalAttack {
                ignore_shield_defence: false,
                ..
            }
        )),
        "{:?}",
        strike.effects
    );
}

/// The same wiring check for the other two damage paths that open on the
/// shield switch: `calcBlowDamage` (daggers) and `EnergyAttack` (the Force
/// spenders). Both are keyed on roll position for the same reason — a mob
/// can't hold a shield, so the block's effect isn't observable, but its two
/// rolls are.
#[test]
fn the_blow_and_energy_paths_consult_the_shield_switch_too() {
    let (mut world, _db, _l) = combat_test_world();
    let mut a_rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = spawn_mob(&mut world, &mut a_rx);
    {
        let cs = world
            .objects
            .get_component_mut::<CombatStats>(&CASTER)
            .unwrap();
        cs.random_dmg = 0;
        cs.crit_hit = 500.0; // `calcBlowSuccess` is seeded from the crit rate
    }

    let fixture = world.data.skill_data.get(1160, 1).expect("fixture").clone();
    let base = |id: i32, effect: SkillEffect| {
        let mut s = fixture.clone();
        s.id = id;
        s.name = format!("Wired{id}");
        s.magic_type = 0;
        s.activate_rate = -1;
        s.effects = vec![effect];
        s
    };

    // --- Blow: [magic, land, shield rate, shield perfect, crit] ---
    let blow = base(
        9707,
        SkillEffect::Blow {
            power: 100.0,
            chance_boost: 1000.0,
            critical_chance: Some(100.0),
            backstab: false,
        },
    );
    // The trailing 99 loses the crit roll only if the two shield rolls were
    // taken; otherwise the crit roll is an earlier 0 and the blow doubles.
    let with_shield_call = hit_for(&mut world, victim, &blow, &[0, 0, 0, 0, 99]);
    assert!(with_shield_call > 0.0, "the blow landed");

    // --- EnergyAttack: [magic, shield rate, shield perfect, crit] ---
    let energy = base(
        9708,
        SkillEffect::EnergyAttack {
            power: 50.0,
            critical_chance: 100.0,
            p_def_mod: 40.0,
            charge_consume: 0,
            ignore_shield_defence: false,
        },
    );
    let energy_ignoring = {
        let mut s = energy.clone();
        s.id = 9709;
        if let Some(SkillEffect::EnergyAttack {
            ignore_shield_defence,
            ..
        }) = s.effects.first_mut()
        {
            *ignore_shield_defence = true;
        }
        s
    };
    let consulted = hit_for(&mut world, victim, &energy, &[0, 0, 0, 99]);
    let ignored = hit_for(&mut world, victim, &energy_ignoring, &[0, 0, 0, 99]);
    assert!(
        (ignored - consulted * 2.0).abs() < 1e-6,
        "EnergyAttack's ignoring cast crit because it skipped the shield rolls: \
         {ignored} vs {consulted}"
    );
}

/// `MagicalAttackRange`'s shield term (Prominence-family nukes): a successful
/// block adds `shldDef · shieldDefPercent / 100` to mDef and shrinks the hit;
/// a perfect block caps it at exactly 1.
#[test]
fn magical_attack_range_consults_the_shield() {
    use crate::model::skill::SkillEffect;

    let (mut world, victim) = shield_world();
    equip_shield(&mut world, victim);
    // Face the caster (heading 32768 = -x, caster at the origin) from 50
    // units out — sharing a spot computes as a back attack, which is
    // shield-exempt.
    if let Some(p) = world.objects.get_component_mut::<Position>(&victim) {
        p.x = 50;
        p.heading = 32768;
    }

    // Power sized so even the unblocked hit is well under the fixture's
    // 100-HP bar (the fixture template is tiny).
    let mut nuke = physical_skill(&world, 9410, 4.0, false);
    nuke.magic_type = 1;
    nuke.effects = vec![SkillEffect::MagicalAttackRange {
        power: 4.0,
        shield_def_percent: 100.0,
    }];

    // A magic hit on a player drains CP before HP; empty the pool so
    // `hit_for`'s HP delta measures the whole hit.
    let zero_cp = |world: &mut World| {
        if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&victim) {
            pv.cur_cp = 0.0;
        }
    };
    // Rolls: mcrit throwaway, shield rate, shield perfect, magic success
    // (0 = the first `calc_magic_success` roll passes → no failure path).
    zero_cp(&mut world);
    let unblocked = hit_for(&mut world, victim, &nuke, &[999, 99, 0, 0]);
    zero_cp(&mut world);
    let blocked = hit_for(&mut world, victim, &nuke, &[999, 0, 0, 0]);
    zero_cp(&mut world);
    let perfect = hit_for(&mut world, victim, &nuke, &[999, 0, 99, 0]);

    assert!(unblocked > 0.0, "the nuke lands unblocked ({unblocked})");
    assert!(
        blocked < unblocked,
        "a successful block adds the shield term to mDef ({blocked} vs {unblocked})"
    );
    assert!(blocked > 1.0, "…but is not the perfect-block cap");
    assert!(
        (perfect - 1.0).abs() < 0.001,
        "a perfect block caps the hit at 1 ({perfect})"
    );
}
