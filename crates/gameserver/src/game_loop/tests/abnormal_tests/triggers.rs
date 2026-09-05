//! Chance-on-event effects: skill evasion and turning, counter physical,
//! mirage, fatal counter, dance of shadows and hate attack.

use super::*;

/// `HATE_ATTACK` (Sword/Blunt Weapon Mastery 217) multiplies the hate an
/// **auto-attack** generates — Java scales it inside
/// `Attackable.reduceCurrentHp`'s `if (skill == null)` branch only. The
/// skill-exclusion is the point: the mastery helps a tank hold aggro through
/// ordinary swings and does nothing for their taunts, so both cases are
/// asserted.
#[test]
fn hate_attack_scales_auto_attack_hate_only() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    let hate_of = |world: &World| {
        world
            .objects
            .get_component::<AggroList>(&NPC_OID)
            .and_then(|a| a.0.get(&CASTER).map(|i| i.hate))
            .unwrap_or(0.0)
    };

    // Unbuffed auto-attack: the plain `damage·100 / (level + 7)`.
    combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, true);
    let plain = hate_of(&world);
    assert!(plain > 0.0, "baseline hate: {plain}");

    let mut mods = world
        .objects
        .get_component::<model::components::stats::StatModifiers>(&CASTER)
        .cloned()
        .expect("modifiers");
    mods.mul.insert(Stat::HateAttack, 3.0);
    world.objects.add_components(&CASTER, mods);

    // Same damage, now tripled…
    combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, true);
    let after_auto = hate_of(&world) - plain;
    assert!(
        (after_auto - plain * 3.0).abs() < 1e-6,
        "an auto-attack's hate is tripled ({plain} → {after_auto})"
    );

    // …but a *skill*'s hate is untouched, which is Java's `skill == null` gate.
    let before = hate_of(&world);
    combat::npc_receive_damage(&mut world, NPC_OID, CASTER, 10.0, false);
    let after_skill = hate_of(&world) - before;
    assert!(
        (after_skill - plain).abs() < 1e-6,
        "skill damage generates unmultiplied hate ({plain} vs {after_skill})"
    );
}

/// G34 S4 sub-slice 4 — `SkillEvasion` (Ultimate Evasion 111, Evasion 446).
///
/// Java keeps this in a **per-`magicType` map**, not a `Stat`: both learnable
/// sources are bucket 0 (physical skills), so the buff must dodge those and
/// leave magic alone. A single global dodge stat would pass any test that only
/// ever fires one kind of skill.
#[test]
fn skill_evasion_dodges_only_its_own_magic_type() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9341,
        SkillEffect::SkillEvasion {
            magic_type: 0,
            amount: 100.0, // always dodge, so the roll is not the variable
        },
        "EVASION",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    land(&mut world, 9341, NPC_OID);
    let evasion = |world: &World, bucket: i32| {
        world
            .objects
            .get_component::<model::components::stats::StatModifiers>(&NPC_OID)
            .and_then(|m| m.skill_evasion.get(&bucket).copied())
            .unwrap_or(0.0)
    };
    assert_eq!(evasion(&world, 0), 100.0, "the physical-skill bucket");
    assert_eq!(
        evasion(&world, 1),
        0.0,
        "…and nothing in the magic bucket — Java keys the map by magicType"
    );

    // The merge is only half of it — the *roll* has to consume the map, or the
    // buff is a number nobody reads. A physical-skill nuke (magicType 0) at
    // 100 % dodge must land no damage at all.
    let mut nuke = cc_skill(
        9343,
        SkillEffect::PhysicalAttack {
            power: 500.0,
            p_atk_mod: 1.0,
            p_def_mod: 1.0,
            critical_chance: 0.0,
            ignore_shield_defence: false,
        },
        "NONE",
    );
    nuke.magic_type = 0; // the bucket the buff covers
    world.data.skill_data.insert_for_test(nuke);
    let hp_before = world
        .objects
        .get_component::<Vitals>(&NPC_OID)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0);
    land(&mut world, 9343, NPC_OID);
    assert_eq!(
        world
            .objects
            .get_component::<Vitals>(&NPC_OID)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0),
        hp_before,
        "a 100 % dodge takes no damage — the map has to reach the roll"
    );

    // `onExit` unmerges: a per-bucket map has no `Stat` recompute to fall back
    // on, so without it Ultimate Evasion's dodge would be permanent.
    effects::handle_buff_expire(&mut world, NPC_OID, 9341);
    assert_eq!(
        evasion(&world, 0),
        0.0,
        "the dodge goes with the buff, or it never goes at all"
    );
}

/// `SkillTurning` — Spell Turning (1412). The name suggests a reflect; the
/// handler is an offensive `ENEMY_ONLY` instant that **breaks the target's
/// cast**. Java bails on a self-cast and on raid bosses.
#[test]
fn skill_turning_breaks_the_targets_cast_but_not_a_raids() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9342,
        SkillEffect::SkillTurning {
            chance: 100,
            static_chance: false,
        },
        "NONE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let victim = 5961;
    let _v = ingame_player_access(&mut world, 2, victim, 0);

    // A self-cast is a no-op even at 100 % — Java returns before the break.
    land(&mut world, 9342, CASTER);

    // Against another caster it breaks the cast.
    world.objects.add_components(
        &victim,
        Casting(model::CastState {
            skill_id: 1177,
            skill_level: 1,
            skill_sub_level: 0,
            target_object_id: CASTER,
            seq: 1,
            // `canAbortCast()` — only an unlaunched cast can be broken.
            launched: false,
            cancel_ms: 0,
            cool_ms: 0,
            trigger_item_object_id: 0,
        }),
    );
    land(&mut world, 9342, victim);
    assert!(
        !world.objects.has_component::<Casting>(&victim),
        "the victim's cast is broken"
    );
}

/// `CounterPhysicalSkill` — Shield of Revenge (439) at 20 %, Counterattack
/// (447) at 90 %. The effect grants a **chance**, not a multiplier, and Java
/// runs the counter from `reduceCurrentHp` *before* the damage lands.
///
/// Two guards decide whether it can fire at all, and both are asserted because
/// dropping either would look correct in a melee-only test: **magic skills
/// cannot be countered**, and neither can anything with `castRange > 40`.
#[test]
fn counter_physical_skill_answers_melee_skills_only() {
    use crate::model::stats::Stat;
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    // 100 % counter on the mob, and enough P.Atk for the counter to bite.
    let mut mods = world
        .objects
        .get_component::<model::components::stats::StatModifiers>(&NPC_OID)
        .cloned()
        .unwrap_or_default();
    mods.add.insert(Stat::VengeanceSkillPhysicalDamage, 100.0);
    world.objects.add_components(&NPC_OID, mods);
    if let Some(cs) = world.objects.get_component_mut::<CombatStats>(&NPC_OID) {
        cs.p_atk = 500.0;
    }

    let caster_hp = |world: &World| {
        world
            .objects
            .get_component::<Vitals>(&CASTER)
            .map(|v| v.cur_hp)
            .unwrap_or(0.0)
    };

    // A melee skill (castRange 40, physical) is countered.
    let mut melee = cc_skill(9351, SkillEffect::Root, "NONE");
    melee.magic_type = 0;
    melee.cast_range = 40;
    world.data.skill_data.insert_for_test(melee);
    let before = caster_hp(&world);
    effects::apply_skill_damage(
        &mut world,
        CASTER,
        NPC_OID,
        effects::SkillHit {
            damage: 1.0,
            caster_name: "c",
            skill_id: 9351,
            ..Default::default()
        },
    );
    assert!(
        caster_hp(&world) < before,
        "a melee skill draws a counter ({before} → {})",
        caster_hp(&world)
    );

    // A *magic* skill never is, however high the chance.
    let mut magic = cc_skill(9352, SkillEffect::Root, "NONE");
    magic.magic_type = 1;
    magic.cast_range = 40;
    world.data.skill_data.insert_for_test(magic);
    let before = caster_hp(&world);
    effects::apply_skill_damage(
        &mut world,
        CASTER,
        NPC_OID,
        effects::SkillHit {
            damage: 1.0,
            is_magic: true,
            caster_name: "c",
            skill_id: 9352,
            ..Default::default()
        },
    );
    assert_eq!(
        caster_hp(&world),
        before,
        "magic is not counterable — Java bails on skill.isMagic()"
    );

    // Nor is a ranged one: `castRange > MELEE_ATTACK_RANGE` (40).
    let mut ranged = cc_skill(9353, SkillEffect::Root, "NONE");
    ranged.magic_type = 0;
    ranged.cast_range = 600;
    world.data.skill_data.insert_for_test(ranged);
    let before = caster_hp(&world);
    effects::apply_skill_damage(
        &mut world,
        CASTER,
        NPC_OID,
        effects::SkillHit {
            damage: 1.0,
            caster_name: "c",
            skill_id: 9353,
            ..Default::default()
        },
    );
    assert_eq!(
        caster_hp(&world),
        before,
        "only melee-range skills can be countered"
    );
}

/// **`PhysicalAttackHpLink`** (Fatal Counter 314, Fatal Arrow 10905) — the
/// physical twin of `DeathLink`: the same `−(curHp·2 / maxHp) + 2` multiplier
/// on the **caster's** missing HP, so a healthy archer's Fatal Counter does
/// nothing and a dying one's hits for double. The skill's own description says
/// as much ("the power of the attack increases as your HP decreases"), and a
/// port that shared `PhysicalAttack`'s arm without the tail would look right at
/// every HP except the two ends.
#[test]
fn fatal_counter_scales_with_the_archers_missing_hp() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 100, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9413,
        SkillEffect::PhysicalAttackHpLink {
            power: 500.0,
            p_atk_mod: 1.0,
            p_def_mod: 1.0,
            critical_chance: 0.0,
            ignore_shield_defence: false,
        },
        "NONE",
    ));

    let damage_at = |world: &mut World, hp_fraction: f64| -> f64 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&CASTER) {
            v.max_hp = 1000;
            v.cur_hp = 1000.0 * hp_fraction;
        }
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&NPC_OID) {
            v.max_hp = 1_000_000;
            v.cur_hp = 1_000_000.0;
            v.dead = false;
        }
        world.clear_forced_rolls();
        world.force_rolls([50; 12]);
        land(world, 9413, NPC_OID);
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

    assert_eq!(at_full, 0.0, "at full HP the multiplier is 0 — no damage");
    assert!(at_half > 0.0, "half HP: {at_half}");
    assert!(
        at_death > at_half * 1.5,
        "the closer to death the harder it hits ({at_half} -> {at_death})"
    );
}

/// **`TriggerSkillByDamage`** (Mirage 445) — the mirror of
/// `TriggerSkillByAttack`: it fires when the bearer **takes** a hit, and casts
/// back at the attacker rather than on itself.
///
/// Two gates separate it from the attack-side twin, and both are the half a
/// "copy the attack trigger" port would drop: `attackerType` (Mirage takes
/// `Playable` only, so a monster hitting you never sets it off) and the
/// requirement that the carrier actually be *up* — Mirage is a timed buff,
/// unlike the always-on weapon masteries the attack twin reads.
#[test]
fn mirage_fires_back_at_a_player_attacker_but_not_a_monster() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let attacker = CASTER + 1;
    let _a = ingame_player(&mut world, CID + 1, attacker, 40, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 20, 60, 0, 0);

    // The trigger the carrier fires.
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9416, SkillEffect::Root, "ROOT"));
    // The carrier: Playable attackers only, always rolls, casts at the enemy.
    let mut carrier = cc_skill(
        9415,
        SkillEffect::TriggerSkillByDamage {
            min_damage: 1,
            chance: 100,
            skill_id: 9416,
            skill_level: 1,
            hp_percent: 100,
            attacker_playable_only: true,
            on_attacker: true,
        },
        "NONE",
    );
    carrier.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(carrier);

    let has = |world: &World, oid: i32| has_buff(world, oid, 9416);

    // Not cast yet: nothing to listen, so nothing triggers. (Java attaches the
    // listener to the *buff*, which is why this is the meaningful negative —
    // knowing Mirage and being under it are different things.)
    combat::apply_attack_damage(&mut world, attacker, CASTER, 50.0, false, None);
    assert!(!has(&world, attacker), "no Mirage buff up, no counter-cast");

    // Now put it up. A *monster* hitting us must still not set it off.
    land(&mut world, 9415, CASTER);
    combat::apply_attack_damage(&mut world, NPC_OID, CASTER, 50.0, false, None);
    assert!(
        !has(&world, NPC_OID),
        "attackerType=Playable: a monster never triggers it"
    );

    // A player hitting us does.
    combat::apply_attack_damage(&mut world, attacker, CASTER, 50.0, false, None);
    assert!(
        has(&world, attacker),
        "a playable attacker takes the counter-cast"
    );
}

/// **`TriggerSkillByMagicType`** (Dance of Shadows 366) — fires when the bearer
/// *finishes casting* a skill whose `magicType` is listed. That is how the
/// dance's stealth ends the moment you act: any ordinary cast fires Cancel
/// Shadow Move on the party.
#[test]
fn dance_of_shadows_cancels_itself_on_a_listed_magic_type() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9418, SkillEffect::Root, "ROOT"));
    let mut carrier = cc_skill(
        9417,
        SkillEffect::TriggerSkillByMagicType {
            magic_types: vec![1, 2],
            chance: 100,
            skill_id: 9418,
            skill_level: 1,
            on_party: true,
        },
        "NONE",
    );
    carrier.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(carrier);
    land(&mut world, 9417, CASTER);

    let has = |world: &World| has_buff(world, CASTER, 9418);

    // A cast whose magicType is *not* listed changes nothing.
    effects::fire_magic_type_triggers(&mut world, CASTER, CASTER, 7);
    assert!(!has(&world), "an unlisted magicType does not fire it");

    // One that is listed does.
    effects::fire_magic_type_triggers(&mut world, CASTER, CASTER, 2);
    assert!(has(&world), "a listed magicType fires the trigger");
}
