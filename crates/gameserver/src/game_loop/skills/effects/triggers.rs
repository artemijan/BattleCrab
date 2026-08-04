use super::*;

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
    let known: Vec<(i32, i32)> = buffs
        .0
        .iter()
        .filter(|a| !a.passive)
        .map(|a| (a.skill_id, a.skill_level))
        .collect();

    let attacker_is_playable = world
        .objects
        .has_component::<crate::model::Player>(&attacker_oid)
        || world
            .objects
            .has_component::<crate::model::components::PetOf>(&attacker_oid)
        || world
            .objects
            .has_component::<crate::model::components::ServitorOf>(&attacker_oid);

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
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
        let Some(trigger) = world
            .data
            .skill_data
            .get(trigger_id, trigger_level)
            .cloned()
        else {
            continue;
        };
        // `targetType`: ENEMY casts back at whoever hit you, SELF on yourself.
        let target = if on_attacker {
            attacker_oid
        } else {
            victim_oid
        };
        let already = world
            .objects
            .get_component::<Buffs>(&target)
            .is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == trigger_id && x.skill_level >= trigger_level)
            });
        if already {
            continue;
        }
        // Java's `triggerCast(event.getAttacker(), target, skill)` — note the
        // *attacker* is the caster of the counter-trigger, not the bearer.
        apply_skill_effects(world, attacker_oid, target, &trigger);
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
    let known: Vec<(i32, i32)> = buffs
        .0
        .iter()
        .filter(|a| !a.passive)
        .map(|a| (a.skill_id, a.skill_level))
        .collect();

    let mut fired: Vec<(i32, i32, bool)> = Vec::new();
    for (skill_id, skill_level) in known {
        let Some(carrier) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
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
        let Some(trigger) = world
            .data
            .skill_data
            .get(trigger_id, trigger_level)
            .cloned()
        else {
            continue;
        };
        // Java resolves the trigger's own `targetType` against the *triggering
        // cast's* target, not the bearer — so the default `TARGET` lands on
        // whoever was just hit, and `MY_PARTY` on the caster's party.
        let targets = if on_party {
            world
                .objects
                .get_component::<crate::model::components::PartyRef>(&caster_oid)
                .and_then(|r| world.parties.get(&r.0))
                .map(|p| p.members.clone())
                .unwrap_or_else(|| vec![caster_oid])
        } else {
            vec![cast_target_oid]
        };
        for t in targets {
            let already = world.objects.get_component::<Buffs>(&t).is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == trigger_id && x.skill_level >= trigger_level)
            });
            if already {
                continue;
            }
            apply_skill_effects(world, caster_oid, t, &trigger);
        }
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
        let Some(carrier) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
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
        let Some(trigger) = world
            .data
            .skill_data
            .get(trigger_id, trigger_level)
            .cloned()
        else {
            continue;
        };
        // `targetType`: SELF or MY_PARTY. The party case reduces to the caster
        // when unpartied, which is how Java's PARTY target handler behaves too.
        let mut targets = vec![attacker_oid];
        if on_party {
            // Java's PARTY target handler treats an unpartied caster as a
            // party of one, which is also what `skills::affect` does.
            targets = world
                .objects
                .get_component::<crate::model::components::PartyRef>(&attacker_oid)
                .and_then(|r| world.parties.get(&r.0))
                .map(|p| p.members.clone())
                .unwrap_or_else(|| vec![attacker_oid]);
        }
        for t in targets {
            // Java's refresh guard: `if (buffInfo == null || buffInfo.getSkill()
            // .getLevel() < triggerSkill.getLevel())` — don't re-cast while the
            // same buff is already up at that level or higher.
            let already = world.objects.get_component::<Buffs>(&t).is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.skill_id == trigger_id && x.skill_level >= trigger_level)
            });
            if already {
                continue;
            }
            // `SkillCaster.triggerCast` — no cast time, no MP, no reuse.
            apply_skill_effects(world, attacker_oid, t, &trigger);
        }
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
