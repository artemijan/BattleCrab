//! What an abnormal does to HP, MP and CP: the regeneration and restore
//! ceilings, LimitHp, the heal pools, death link, balance life, elixirs and
//! MP vampiric.

use super::*;

/// `MpVampiricAttack` (Weapon Mastery 250) — the MP twin of the HP drain, and
/// **its config gate is shaped the opposite way**, which is the whole point of
/// this test. HP vampirism asks `skill == null || WORKS_WITH_SKILLS`: melee by
/// default. MP vampirism asks `skill != null || WORKS_WITH_MELEE`: *skills* by
/// default. Both configs are off on this dist, so Weapon Mastery drains MP on
/// skill hits and nothing at all on a melee swing.
#[test]
fn mp_vampiric_drains_on_skills_not_melee() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);

    // 10 % of damage, and a `sum` chosen to make the chance exactly 1.0 so the
    // test is about the *gate*, not the roll: the finalizer is
    // `min(1, sum / (percent × 100) / 100)`, so `sum = 0.1 × 100 × 100 = 1000`.
    // (Weapon Mastery's own `amount 10` gives sum 300 → **0.3**, which is
    // Java's own "Classic: 30% chance" comment — using it here made the first
    // draft of this test fail 70 % of the time.)
    let mut mods = world
        .objects
        .get_component::<model::components::stats::StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.add.insert(Stat::AbsorbManaDamagePercent, 0.1);
    mods.add.insert(Stat::MpVampiricSum, 1000.0);
    world.objects.add_components(&CASTER, mods);
    // Room to drain into, and something to drain from.
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
        v.max_mp = 10_000;
        v.cur_mp = 0.0;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&NPC_OID) {
        v.max_mp = 10_000;
        v.cur_mp = 10_000.0;
        v.max_hp = 1_000_000;
        v.cur_hp = 1_000_000.0;
    }
    let mp = |world: &World| {
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_mp)
            .unwrap_or(0.0)
    };

    // A melee swing (`skill_magic == None`) drains nothing on this dist.
    combat::apply_attack_damage(&mut world, CASTER, NPC_OID, 500.0, false, None);
    assert_eq!(
        mp(&world),
        0.0,
        "MpVampiricAttackWorkWithMelee is False here, so melee drains nothing"
    );

    // A skill hit does. `apply_physical_damage`'s `from_skill` is the same
    // discriminator, so drive it through the skill-damage entry point.
    combat::apply_attack_damage(&mut world, CASTER, NPC_OID, 500.0, false, Some(false));
    assert!(
        mp(&world) > 0.0,
        "a skill hit drains 10 % of the damage into MP: {}",
        mp(&world)
    );
}

/// The same `MAX_RECOVERABLE_HP` ceiling, on the **over-time** side. Java reads
/// it in `HealOverTime.onActionTime` and in `Relax.onActionTime`:
///
/// ```java
/// double hp = effected.getCurrentHp();
/// final double maxhp = effected.getMaxRecoverableHp();
/// if (_power > 0) { if (hp >= maxhp) return false; }
/// …
/// hp = Math.min(hp, maxhp);
/// ```
///
/// The port clamped both to the raw pool, so a regeneration under Noblesse
/// Harmony kept ticking past the cap that its own instant heals respected —
/// the two halves of the same skill disagreeing about the same number.
#[test]
fn a_regeneration_stops_at_the_recoverable_ceiling_too() {
    use crate::model::components::stats::StatModifiers;
    use crate::model::stats::Stat;

    const HOT: i32 = 9394;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    // A plain regeneration: +50 HP a tick for a long while.
    let mut hot = cc_skill(
        HOT,
        SkillEffect::HealOverTime {
            power: 50.0,
            ticks: 1,
        },
        "HP_RECOVER",
    );
    hot.effect_point = 100;
    hot.is_debuff = false;
    hot.abnormal_time = 600;
    world.data.skill_data.insert_for_test(hot.clone());

    assert!(
        effects::apply_continuous_effects(&mut world, CASTER, CASTER, &hot, None),
        "the regeneration buff landed"
    );
    // Both of these go on **after** the buff: landing one rebuilds
    // `StatModifiers` from the live buff list (dropping a hand-placed entry)
    // and runs `recompute_max_vitals` (recomputing `max_hp` off the template).
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    // Noblesse Harmony's `PER −30` on `MAX_RECOVERABLE_HP`.
    mods.mul.insert(Stat::MaxRecoverableHp, 0.7);
    world.objects.add_components(&CASTER, mods);
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
        v.max_hp = 1000;
        v.cur_hp = 100.0;
    }

    // Let it run well past the point where it would fill an uncapped bar.
    advance_ticks(&mut world, 400);

    let hp = world
        .objects
        .get_component::<Vitals>(&CASTER)
        .map(|v| v.cur_hp)
        .expect("alive");
    assert!(
        (hp - 700.0).abs() < 1.0,
        "the regeneration stops at the 70 % recoverable ceiling, got {hp}"
    );
}

/// `Cp.instant` reads **`getMaxRecoverableCp()`** for its headroom and bails on
/// the same three states its neighbour `CpHealPercent` does:
///
/// ```java
/// if (effected.isDead() || effected.isDoor() || effected.isHpBlocked()) return;
/// case DIFF: amount = Math.min(basicAmount, Math.max(0, effected.getMaxRecoverableCp() - effected.getCurrentCp()));
/// ```
///
/// `LimitCp`'s learnable carriers are Noblesse Harmony (1326) and Noblesse
/// Symphony (1327) at `PER −40`, so under either aura a CP restore has to stop
/// at 60 % — which the *percent* variant already did and the flat one did not.
#[test]
fn a_flat_cp_restore_stops_at_the_recoverable_ceiling() {
    use crate::model::components::stats::{PlayerVitals, StatModifiers};
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let set_pools = |world: &mut World| {
        if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&CASTER) {
            pv.max_cp = 1000;
            pv.cur_cp = 100.0;
        }
    };
    let cur_cp = |world: &World| {
        world
            .objects
            .get_component::<PlayerVitals>(&CASTER)
            .map(|pv| pv.cur_cp)
            .unwrap_or(0.0)
    };

    // Uncapped: a huge flat restore fills the pool.
    set_pools(&mut world);
    effects::cp(&mut world, CASTER, 100_000.0, false);
    assert_eq!(cur_cp(&world), 1000.0, "no cap → restore to full");

    // Noblesse Harmony's `PER −40` on `MAX_RECOVERABLE_CP`.
    let mut mods = world
        .objects
        .get_component::<StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.mul.insert(Stat::MaxRecoverableCp, 0.6);
    world.objects.add_components(&CASTER, mods);

    set_pools(&mut world);
    effects::cp(&mut world, CASTER, 100_000.0, false);
    assert_eq!(
        cur_cp(&world),
        600.0,
        "the same restore now stops at 60 % — the cap is the point of the aura"
    );

    // And the three-way bail: an HP-blocked target takes no CP either (Java
    // reads `isHpBlocked` on the *CP* effect, which is not a typo on its part).
    set_pools(&mut world);
    let mut blocker = cc_skill(
        9396,
        SkillEffect::DamageBlock {
            block_hp: true,
            block_mp: false,
        },
        "DAMAGE_BLOCK",
    );
    blocker.effect_point = 100;
    blocker.is_debuff = false;
    blocker.abnormal_time = 600;
    world.data.skill_data.insert_for_test(blocker.clone());
    effects::apply_continuous_effects(&mut world, CASTER, CASTER, &blocker, None);
    set_pools(&mut world);
    effects::cp(&mut world, CASTER, 100_000.0, false);
    assert_eq!(cur_cp(&world), 100.0, "HP-blocked, so no CP is restored");
}

/// G34 S4 sub-slice 8 — `LimitHp`/`LimitCp` (`MAX_RECOVERABLE_HP`/`_CP`), the
/// ceiling a **heal** may restore to.
///
/// The learnable sources are *restrictions*: Noblesse Harmony (1326) and
/// Symphony (1327) grant them `PER −30` / `−40`, so under those auras you can
/// only be healed back to 70 % HP and 60 % CP. A port that clamps heals to the
/// raw pool — as this one did — behaves identically until someone casts them.
#[test]
fn limit_hp_caps_how_far_a_heal_can_restore() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut heal = cc_skill(9391, SkillEffect::Heal { power: 10_000.0 }, "NONE");
    heal.effect_point = 100;
    heal.is_debuff = false;
    world.data.skill_data.insert_for_test(heal);

    let set_hp = |world: &mut World, cur: f64| {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 1000;
            v.cur_hp = cur;
        }
    };
    let hp = |world: &World| {
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };

    // Unlimited: a huge heal fills the pool.
    set_hp(&mut world, 100.0);
    land(&mut world, 9391, CASTER);
    assert_eq!(hp(&world), 1000.0, "no cap → heal to full");

    // Noblesse Harmony's `PER −30` → `mul` 0.7 on MAX_RECOVERABLE_HP.
    let mut mods = world
        .objects
        .get_component::<model::components::stats::StatModifiers>(&CASTER)
        .cloned()
        .unwrap_or_default();
    mods.mul.insert(Stat::MaxRecoverableHp, 0.7);
    world.objects.add_components(&CASTER, mods);
    set_hp(&mut world, 100.0);
    land(&mut world, 9391, CASTER);
    assert_eq!(
        hp(&world),
        700.0,
        "the same heal now stops at 70 % — the cap is the point of the skill"
    );

    // Already above the cap: the heal restores nothing rather than draining.
    set_hp(&mut world, 900.0);
    land(&mut world, 9391, CASTER);
    assert_eq!(hp(&world), 900.0, "over the cap, a heal is a no-op");
}

/// `CpHealPercent` (Victories of Pa'agrio 1414 at 20 %) restores a share of
/// **max CP** and honours `MAX_RECOVERABLE_CP`; `HpByLevel` (Life Scavenge 46,
/// Corpse Life Drain 1151) heals the **effector** — the caster, not the target.
///
/// The `HpByLevel` direction is the trap: every other heal in the family reads
/// `effected`, and pointing this one at the target would heal the corpse you
/// are draining.
#[test]
fn cp_heal_percent_and_hp_by_level_hit_the_right_pools() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = 5971;
    let _v = ingame_player_access(&mut world, 2, victim, 0);

    let mut cp_heal = cc_skill(9392, SkillEffect::CpHealPercent { power: 20.0 }, "NONE");
    cp_heal.effect_point = 100;
    cp_heal.is_debuff = false;
    world.data.skill_data.insert_for_test(cp_heal);
    let mut drain = cc_skill(9393, SkillEffect::HpByLevel { power: 260.0 }, "NONE");
    drain.effect_point = 100;
    drain.is_debuff = false;
    world.data.skill_data.insert_for_test(drain);

    // CP heal lands on the *target*.
    if let Some(v) = world.objects.get_component_mut::<PlayerVitals>(&victim) {
        v.max_cp = 1000;
        v.cur_cp = 0.0;
    }
    land(&mut world, 9392, victim);
    assert_eq!(
        world
            .objects
            .get_component::<PlayerVitals>(&victim)
            .map(|v| v.cur_cp),
        Some(200.0),
        "20 % of max CP"
    );

    // `HpByLevel` lands on the *caster*, whatever the target is.
    for oid in [CASTER, victim] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = 10_000;
            v.cur_hp = 1_000.0;
        }
    }
    land(&mut world, 9393, victim);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp),
        Some(1_260.0),
        "the caster is healed"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&victim)
            .map(|v| v.cur_hp),
        Some(1_000.0),
        "…and the target — the corpse being drained — is not"
    );
}

/// G34 S4 sub-slice 9 — `DeathLink` (Curse Death Link 1159). The power scales
/// with how close the **caster** is to death: `power × (2 − 2·curHp/maxHp)`,
/// so it is ×2 at 0 HP and **×0 at full**. Casting it healthy does literally
/// nothing, which is the opposite of how every other nuke behaves and the
/// reason to assert the full-HP case explicitly.
#[test]
fn death_link_scales_with_the_casters_missing_hp() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);
    let mut link = cc_skill(9401, SkillEffect::DeathLink { power: 100.0 }, "NONE");
    link.magic_type = 1;
    // The magic-failure roll floors a failed cast at 1 damage regardless of
    // power, which would swamp the multiplier we are measuring here.
    world.cfg.character.magic_failures = false;
    world.data.skill_data.insert_for_test(link);

    let damage_at = |world: &mut World, hp_fraction: f64| -> f64 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 1000;
            v.cur_hp = 1000.0 * hp_fraction;
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&NPC_OID) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
        }
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        land(world, 9401, NPC_OID);
        1_000_000.0
            - world
                .objects
                .get_component::<Vitals>(&NPC_OID)
                .map(|v| v.cur_hp)
                .unwrap_or(0.0)
    };

    let at_full = damage_at(&mut world, 1.0);
    let at_half = damage_at(&mut world, 0.5);
    let at_death = damage_at(&mut world, 0.01);

    // At full HP the multiplier is 0, so the nuke does nothing at all.
    assert_eq!(at_full, 0.0, "at full HP the multiplier is 0 — no damage");
    assert!(at_half > 0.0, "half HP: {at_half}");
    assert!(
        at_death > at_half * 1.5,
        "the closer to death the harder it hits ({at_half} → {at_death})"
    );
}

/// **`RebalanceHP`** (Balance Life 1043) — pool the party's HP and set everyone
/// to the party average *percentage*. It is a redistribution, not a heal: the
/// total is unchanged, so the healthy pay for the dying. That is the half a
/// "heal the party" implementation would get wrong in the most visible way —
/// the caster at full HP is supposed to come out of it *worse*.
#[test]
fn balance_life_averages_the_party_and_costs_the_healthy() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let ally = CASTER + 1;
    let _ally_out = ingame_player(&mut world, CID + 1, ally, 50, 0, 0);
    let mut skill = cc_skill(9406, SkillEffect::RebalanceHp, "NONE");
    skill.affect_range = 900;
    world.data.skill_data.insert_for_test(skill);
    make_party(&mut world, &[CASTER, ally], LootRule::Random);

    // Same pool, wildly different fills: 100 % and 20 % → a 60 % average.
    for (oid, cur) in [(CASTER, 1000.0), (ally, 200.0)] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = 1000;
            v.cur_hp = cur;
        }
    }
    let total_before = 1000.0 + 200.0;

    land(&mut world, 9406, CASTER);

    let hp_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Vitals>(&oid)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };
    assert_eq!(hp_of(&world, ally), 600.0, "the dying ally is pulled up");
    assert_eq!(
        hp_of(&world, CASTER),
        600.0,
        "and the healthy caster is pulled *down* — this is not a heal"
    );
    assert_eq!(
        hp_of(&world, CASTER) + hp_of(&world, ally),
        total_before,
        "the party's total HP is conserved"
    );
}

/// Java guards the whole effect with `if (party != null)`, so an unpartied
/// Balance Life is simply wasted — it does **not** fall back to the "party of
/// one" reading every other party-scoped effect uses.
///
/// The caster alone cannot show this: with one member the average *is* their
/// own percentage, so the maths is a no-op either way and the guard is
/// invisible. A **pet** is what makes the difference observable — under the
/// fallback the pair would rebalance against each other, under Java's guard
/// neither of them moves.
#[test]
fn balance_life_without_a_party_does_nothing() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut skill = cc_skill(9407, SkillEffect::RebalanceHp, "NONE");
    skill.affect_range = 900;
    world.data.skill_data.insert_for_test(skill);

    // A pet at a very different fill from its (unpartied) owner.
    let pet = NPC_OID;
    add_test_npc(&mut world, pet, 20001, "Monster", 20, 60, 0, 0);
    world.objects.add_components(
        &CASTER,
        model::components::summons::SummonRef {
            servitor: None,
            pet: Some(pet),
        },
    );
    for (oid, max, cur) in [(CASTER, 1000, 250.0), (pet, 1000, 1000.0)] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = max;
            v.cur_hp = cur;
        }
    }

    land(&mut world, 9407, CASTER);

    let hp_of = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Vitals>(&oid)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };
    assert_eq!(
        hp_of(&world, CASTER),
        250.0,
        "solo, the caster is untouched"
    );
    assert_eq!(
        hp_of(&world, pet),
        1000.0,
        "and so is their pet — no party, no rebalance"
    );
}

/// **`Hp`** — the raw instant HP change behind Elixir of Life (2287) and the
/// food items, which parsed to *nothing* before. It is not a `Heal`: no
/// `calcHeal`, no healing-stat scaling. Java's guard list is dead / door /
/// HP-blocked / **raid**, that last one being the clause the `Heal` family
/// does not have.
#[test]
fn an_elixir_restores_hp_but_never_a_raid_bosss() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9441,
        SkillEffect::Hp {
            amount: 250.0,
            percent: false,
        },
        "NONE",
    ));
    let boss = NPC_OID;
    add_test_npc(&mut world, boss, 90301, "RaidBoss", 40, 100, 0, 0);

    for (oid, cur, max) in [(CASTER, 100.0, 1000), (boss, 100.0, 1000)] {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
            v.max_hp = max;
            v.cur_hp = cur;
        }
    }

    land(&mut world, 9441, CASTER);
    land(&mut world, 9441, boss);

    let hp = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Vitals>(&oid)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };
    assert_eq!(hp(&world, CASTER), 350.0, "a flat 250 restored");
    assert_eq!(
        hp(&world, boss),
        100.0,
        "a raid boss is exempt — the clause `Heal` does not have"
    );
}

/// The gain is clamped to the **recoverable** headroom, so an aura that caps
/// how far you can be healed caps an elixir too.
#[test]
fn an_elixir_honours_the_recoverable_ceiling() {
    use crate::model::stats::Stat;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9442,
        SkillEffect::Hp {
            amount: 900.0,
            percent: false,
        },
        "NONE",
    ));
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
        v.max_hp = 1000;
        v.cur_hp = 100.0;
    }
    // Noblesse Harmony's shape: heals may only reach 70 % of the pool.
    if let Some(m) = world
        .objects
        .get_component_mut::<model::components::stats::StatModifiers>(&CASTER)
    {
        *m.mul.entry(Stat::MaxRecoverableHp).or_insert(1.0) *= 0.7;
    }

    land(&mut world, 9442, CASTER);

    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp)
            .unwrap(),
        700.0,
        "clamped to the recoverable ceiling, not the raw pool"
    );
}
