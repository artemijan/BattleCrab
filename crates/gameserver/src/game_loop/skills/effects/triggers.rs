use super::apply_skill_effects;
use crate::game_loop::skills::skill_by_id;
use crate::model::components::Buffs;
use crate::model::components::Vitals;
use crate::model::skill::Skill;
use crate::model::skill::effects::SkillEffect;
use crate::world::World;

fn known_buffs(buffs: &Buffs) -> Vec<(i32, i32)> {
    buffs
        .0
        .iter()
        .filter(|a| !a.passive)
        .map(|a| (a.skill_id, a.skill_level))
        .collect()
}
/// `TriggerSkillByDamage`'s `onDamageReceivedEvent` — the mirror of
/// [`fire_attack_triggers`], evaluated for every hit the **bearer takes**.
///
/// Same subscription-versus-scan trade as the attack side: Java attaches a
/// listener when the carrying buff starts, this port scans the victim's skill
/// book at damage time.
///
/// Java's gates in order: not a DoT tick, no self-hits, the attacker level
/// window, the damage floor, the chance roll, the `hpPercent` **upper** bound
/// on the bearer's HP share, and the `attackerType` narrowing (Mirage takes
/// `Playable`, so a mob hitting you never sets it off).
pub(crate) fn fire_damage_received_triggers(
    world: &mut World,
    victim_oid: i32,
    attacker_oid: i32,
    damage: i32,
    is_dot: bool,
) {
    // `event.isDamageOverTime()` and `attacker == target` both bail.
    if is_dot || victim_oid == attacker_oid {
        return;
    }
    // Java attaches the listener to the **buff**, so the carriers here are the
    // bearer's live effects — not their skill book. That is the opposite of
    // `fire_attack_triggers`, whose carriers are passives folded into
    // `StatModifiers` and therefore absent from the buff list; knowing Mirage
    // and being under it are different things.
    let Some(buffs) = world.objects.get_component::<Buffs>(&victim_oid) else {
        return;
    };
    let known: Vec<(i32, i32)> = known_buffs(buffs);

    let attacker_is_playable = crate::game_loop::helpers::is_playable(world, attacker_oid);

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = skill_by_id(world, skill_id, skill_level) else {
            continue;
        };
        for effect in &carrier.effects {
            let SkillEffect::TriggerSkillByDamage {
                min_damage,
                chance,
                skill_id: trigger_id,
                skill_level: trigger_level,
                hp_percent,
                attacker_playable_only,
                on_attacker,
            } = effect
            else {
                continue;
            };
            if *chance == 0 || *trigger_id == 0 || *trigger_level == 0 {
                continue;
            }
            if damage < *min_damage {
                continue;
            }
            if *attacker_playable_only && !attacker_is_playable {
                continue;
            }
            // `hpPercent` is an *upper* bound: Java bails when the bearer is
            // healthier than it. 100 (the default) can never bail.
            if *hp_percent < 100 {
                let share = world
                    .objects
                    .get_component::<Vitals>(&victim_oid)
                    .filter(|v| v.max_hp > 0)
                    .map(|v| v.cur_hp * 100.0 / v.max_hp as f64)
                    .unwrap_or(100.0);
                if share > *hp_percent as f64 {
                    continue;
                }
            }
            // `Rnd.get(100) > _chance` bails — `chance` itself passes.
            if *chance < 100 && world.roll(100) > *chance {
                continue;
            }
            fired.push((*trigger_id, *trigger_level, *on_attacker));
        }
    }

    for (trigger_id, trigger_level, on_attacker) in fired {
        let Some(trigger) = skill_by_id(world, trigger_id, trigger_level) else {
            continue;
        };
        // `targetType`: ENEMY casts back at whoever hit you, SELF on yourself.
        let target = if on_attacker {
            attacker_oid
        } else {
            victim_oid
        };
        // Java's `triggerCast(event.getAttacker(), target, skill)` — note the
        // *attacker* is the caster of the counter-trigger, not the bearer.
        cast_trigger_on(world, attacker_oid, &[target], &trigger);
    }
}

/// `TriggerSkillByMagicType`'s `onSkillUseEvent` — fires when the bearer
/// finishes casting a skill whose `magicType` is in the list.
///
/// Dance of Shadows (366) is the learnable carrier: any ordinary cast fires
/// Cancel Shadow Move (7097) on the party, which is how the dance's stealth
/// ends the moment you do something.
pub(crate) fn fire_magic_type_triggers(
    world: &mut World,
    caster_oid: i32,
    cast_target_oid: i32,
    cast_magic_type: i32,
) {
    // Carriers are live buffs, not book entries — see the note on
    // `fire_damage_received_triggers`.
    let Some(buffs) = world.objects.get_component::<Buffs>(&caster_oid) else {
        return;
    };
    let known: Vec<(i32, i32)> = known_buffs(buffs);

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = skill_by_id(world, skill_id, skill_level) else {
            continue;
        };
        for effect in &carrier.effects {
            let SkillEffect::TriggerSkillByMagicType {
                magic_types,
                chance,
                skill_id: trigger_id,
                skill_level: trigger_level,
                on_party,
            } = effect
            else {
                continue;
            };
            if *chance == 0 || *trigger_id == 0 || *trigger_level == 0 || magic_types.is_empty() {
                continue;
            }
            if !magic_types.contains(&cast_magic_type) {
                continue;
            }
            if *chance < 100 && world.roll(100) > *chance {
                continue;
            }
            fired.push((*trigger_id, *trigger_level, *on_party));
        }
    }

    for (trigger_id, trigger_level, on_party) in fired {
        let Some(trigger) = skill_by_id(world, trigger_id, trigger_level) else {
            continue;
        };
        // Java resolves the trigger's own `targetType` against the *triggering
        // cast's* target, not the bearer — so the default `TARGET` lands on
        // whoever was just hit, and `MY_PARTY` on the caster's party.
        let targets = if on_party {
            crate::game_loop::party::group_or_self(world, caster_oid)
        } else {
            vec![cast_target_oid]
        };
        cast_trigger_on(world, caster_oid, &targets, &trigger);
    }
}

/// `TriggerSkillByAttack`'s `onAttackEvent`, evaluated for every hit the
/// attacker lands (`combat::handle_attack_hit`).
///
/// Java subscribes each effect to `OnCreatureDamageDealt` when the carrying
/// skill starts. These carriers are *passives* (weapon masteries), whose
/// effects this port folds into `StatModifiers` rather than keeping as a live
/// effect list — so instead of a subscription the attacker's skill book is
/// scanned at hit time. That is a handful of `HashMap` lookups per swing; if it
/// ever shows up in a profile it should become a cached index like
/// `NpcAiSkillIndex`, not a behavioural change.
///
/// Ported gates, in Java's order: damage floor, **criticality equality**
/// (`isCritical != event.isCritical()` bails — so an `isCritical=false` trigger
/// fires only on non-crits), no self-hits, the chance roll, and the
/// `allowWeapons` mask. `allowSkillAttack` defaults to false and this is the
/// normal-attack path, so the skill-attack clause is satisfied by construction.
pub(crate) fn fire_attack_triggers(
    world: &mut World,
    attacker_oid: i32,
    target_oid: i32,
    damage: i32,
    crit: bool,
) {
    // `event.getAttacker() == event.getTarget()` bails.
    if attacker_oid == target_oid {
        return;
    }
    // Only players carry these skills on this dist (the three learnable
    // carriers are all class passives/dances).
    let Some(book) = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&attacker_oid)
    else {
        return;
    };
    let known: Vec<(i32, i32)> = book.0.iter().map(|(&id, &lvl)| (id, lvl)).collect();

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = skill_by_id(world, skill_id, skill_level) else {
            continue;
        };
        for effect in &carrier.effects {
            let SkillEffect::TriggerSkillByAttack {
                min_damage,
                chance,
                skill_id: trigger_id,
                skill_level: trigger_level,
                on_party,
                is_critical,
                allow_weapons,
            } = effect
            else {
                continue;
            };
            if *chance == 0 || damage < *min_damage || *is_critical != crit {
                continue;
            }
            // `Rnd.get(100) > _chance` bails — note `>`, so `chance` itself
            // still fires (a 100 chance is certain).
            if world.roll(100) > *chance {
                continue;
            }
            if *allow_weapons != 0 && !attacker_weapon_allowed(world, attacker_oid, *allow_weapons)
            {
                continue;
            }
            fired.push((*trigger_id, *trigger_level, *on_party));
        }
    }

    for (trigger_id, trigger_level, on_party) in fired {
        let Some(trigger) = skill_by_id(world, trigger_id, trigger_level) else {
            continue;
        };
        // `targetType`: SELF or MY_PARTY. The party case reduces to the caster
        // when unpartied, which is how Java's PARTY target handler behaves too.
        let mut targets = vec![attacker_oid];
        if on_party {
            // Java's PARTY target handler treats an unpartied caster as a
            // party of one, which is also what `skills::affect` does.
            targets = crate::game_loop::party::group_or_self(world, attacker_oid);
        }
        cast_trigger_on(world, attacker_oid, &targets, &trigger);
    }
}

/// Cast `trigger` on each of `targets`, skipping anyone already carrying it at
/// that level or higher.
///
/// That skip is Java's refresh guard — `if (buffInfo == null || buffInfo
/// .getSkill().getLevel() < triggerSkill.getLevel())` — and it is the whole
/// reason this is a function rather than three loops: getting it wrong the
/// permissive way re-casts an equal-level buff and resets its duration on every
/// swing, which reads as "the buff never expires" rather than as a bug.
///
/// `SkillCaster.triggerCast`: no cast time, no MP, no reuse. The caster is
/// passed in because it is not always the bearer — the counter-trigger fires
/// with the *attacker* as caster.
fn cast_trigger_on(world: &mut World, caster_oid: i32, targets: &[i32], trigger: &Skill) {
    for &t in targets {
        let already = world.objects.get_component::<Buffs>(&t).is_some_and(|b| {
            b.0.iter()
                .any(|x| x.skill_id == trigger.id && x.skill_level >= trigger.level)
        });
        if already {
            continue;
        }
        apply_skill_effects(world, caster_oid, t, trigger);
    }
}

/// `event.getAttacker().getActiveWeaponItem().getItemType().mask() & _allowWeapons`.
fn attacker_weapon_allowed(world: &World, attacker_oid: i32, mask: u32) -> bool {
    let Some(inv) = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
    else {
        return false;
    };
    crate::model::weapon_condition_passes(mask, inv, &world.data.item_data)
}

/// The augment **activation** skills — Java's `_triggerSkills` map, checked at
/// its two firing sites.
///
/// Java writes the same loop twice with different predicates, so this takes the
/// predicate as `want`:
///
/// - `Creature.onHitTarget` (auto-attack): `ATTACK` on a **non-critical** hit,
///   `CRITICAL` on a critical one — so the two are mutually exclusive there and
///   a crit never fires an `ATTACK` proc.
/// - `SkillCaster` (a finished cast, `!skill.isStatic()`): `MAGIC` when the cast
///   skill is magic, `ATTACK` when it is physical.
///
/// `ATTACK` therefore fires from both sites, which is Java's shape and not a
/// duplicate: a physical *skill* and a plain swing are different events.
///
/// The roll is Java's `Rnd.get(100) < chance` — an integer roll against a
/// `double` chance, so a `chance` of 1.0 is a 1-in-100 proc, not 1-in-1.
fn fire_option_triggers(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    want: impl Fn(crate::data::option_data::OptionSkillType) -> bool,
) {
    let Some(reg) = world
        .objects
        .get_component::<crate::model::components::OptionTriggers>(&caster_oid)
    else {
        return;
    };
    if reg.0.is_empty() {
        return;
    }
    let candidates: Vec<crate::data::option_data::OptionTrigger> =
        reg.0.values().filter(|t| want(t.kind)).copied().collect();

    let mut fired: Vec<(i32, i32)> = Vec::new();
    for t in candidates {
        if t.skill_id == 0 || t.skill_level == 0 {
            continue;
        }
        if (world.roll(100) as f64) < t.chance {
            fired.push((t.skill_id, t.skill_level));
        }
    }
    for (skill_id, skill_level) in fired {
        let Some(skill) = skill_by_id(world, skill_id, skill_level) else {
            continue;
        };
        // `SkillCaster.triggerCast(this, target, skill, null, false)` — the
        // trigger lands on whoever was hit / was the cast's target, not the
        // bearer.
        apply_skill_effects(world, caster_oid, target_oid, &skill);
    }
}

/// `Creature.onHitTarget`'s trigger loop — the auto-attack half.
pub(crate) fn fire_option_attack_triggers(
    world: &mut World,
    attacker_oid: i32,
    target_oid: i32,
    crit: bool,
) {
    use crate::data::option_data::OptionSkillType;
    fire_option_triggers(world, attacker_oid, target_oid, |kind| {
        if crit {
            kind == OptionSkillType::Critical
        } else {
            kind == OptionSkillType::Attack
        }
    });
}

/// `SkillCaster`'s trigger loop — the finished-cast half. `magic_type` is the
/// cast skill's, so `1` is magic and `0` physical; a **static** skill (2) fires
/// nothing, which is Java's `!skill.isStatic()` gate around the whole block.
pub(crate) fn fire_option_cast_triggers(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    magic_type: i32,
) {
    use crate::data::option_data::OptionSkillType;
    if magic_type == 2 {
        return;
    }
    fire_option_triggers(world, caster_oid, target_oid, |kind| match kind {
        OptionSkillType::Magic => magic_type == 1,
        OptionSkillType::Attack => magic_type == 0,
        OptionSkillType::Critical => false,
    });
}
