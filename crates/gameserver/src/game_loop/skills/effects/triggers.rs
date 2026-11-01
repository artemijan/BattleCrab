use super::apply_skill_effects;
use crate::game_loop::combat::DamageHit;
use crate::game_loop::skills::skill_by_id;
use crate::model::Player;
use crate::model::components::skills::Buffs;
use crate::model::components::stats::Vitals;
use crate::model::skill::Skill;
use crate::model::skill::effects::{AttackerType, DamageTrigger, SkillEffect};
use crate::model::skill::target::TargetType;
use crate::world::World;

fn known_buffs(buffs: &Buffs) -> Vec<(i32, i32)> {
    buffs
        .0
        .iter()
        .filter(|a| !a.passive)
        .map(|a| (a.skill_id, a.skill_level))
        .collect()
}
/// Java's `CommonSkill` enum — the ids `TriggerSkillByMagicType` refuses to
/// react to: system casts (curses, grade penalties, crafting, transforms…)
/// that are not the bearer *acting*.
const COMMON_SKILL_IDS: [i32; 42] = [
    4215, 4515, 246, 247, 4289, 325, 5965, 2025, 5182, 3630, 3631, 5074, 5075, 2139, 6209, 6213,
    172, 194, 239, 248, 617, 1320, 1405, 2341, 14571, 19034, 19035, 32117, 32119, 32120, 32122,
    19009, 17192, 17943, 17966, 6054, 4270, 6201, 6202, 6203, 18103, 5103,
];

/// `OnCreatureSkillFinishCast` — every `TriggerSkillByMagicType` started on the
/// caster hears the finished cast (`TriggerSkillByMagicType.onSkillUseEvent`).
/// `target` is the event's target: the cast's own target for a real cast, the
/// trigger's target for a `triggerCast`.
pub(crate) fn fire_magic_type_triggers(
    world: &mut World,
    caster: i32,
    target: i32,
    cast_skill: &Skill,
) {
    let listeners: Vec<SkillEffect> = started_skills(world, caster)
        .into_iter()
        .flat_map(|s| s.effects)
        .filter(|e| matches!(e, SkillEffect::TriggerSkillByMagicType { .. }))
        .collect();
    for effect in listeners {
        let SkillEffect::TriggerSkillByMagicType {
            magic_types,
            chance,
            skill_id,
            skill_level,
            skill_level_scale_to,
            target_type,
            replace,
        } = effect
        else {
            continue;
        };
        // `onStart` never subscribes such an effect.
        if chance == 0 || skill_id == 0 || skill_level == 0 || magic_types.is_empty() {
            continue;
        }
        // `target == null || !target.isCreature()`.
        if crate::game_loop::skills::cast::target_state(world, target).is_none() {
            continue;
        }
        if !magic_types.contains(&cast_skill.magic_type) {
            continue;
        }
        if chance < 100 && world.roll(100) > chance {
            continue;
        }
        let Some(base) = skill_by_id(world, skill_id, skill_level) else {
            continue;
        };
        let Some(resolved) = crate::game_loop::skills::cast::resolve_handler_target(
            world,
            caster,
            target,
            target_type,
            &base,
            false,
        ) else {
            continue;
        };
        // Checked after the roll and the target, as Java orders it.
        if COMMON_SKILL_IDS.contains(&cast_skill.id) {
            continue;
        }
        let held = (skill_level_scale_to > 0)
            .then(|| crate::game_loop::skills::cast::buff_level(world, resolved, skill_id))
            .flatten();
        let fired = match held {
            None => base,
            Some(have) => {
                let Some(stepped) =
                    skill_by_id(world, skill_id, skill_level_scale_to.min(have + 1))
                else {
                    continue;
                };
                let held_disabled = skill_by_id(world, skill_id, have).is_some_and(|h| {
                    crate::game_loop::skills::cast::is_skill_disabled(world, caster, &h)
                });
                if held_disabled {
                    // At the cap and still cooling: the topped-out copy is
                    // taken off rather than left to run.
                    if replace && have == skill_level_scale_to {
                        super::expire_buffs_where(world, resolved, |_, b| b.skill_id == skill_id);
                    }
                    continue;
                }
                stepped
            }
        };
        // `replace`: the target's copy comes off silently first, so the cast
        // below restarts it instead of being refused as a non-upgrade.
        if replace && !crate::game_loop::skills::cast::is_skill_disabled(world, caster, &fired) {
            super::expire_buffs_where(world, resolved, |_, b| b.skill_id == skill_id);
        }
        crate::game_loop::skills::cast::trigger_cast(world, caster, resolved, fired);
    }
}

/// The skills whose effects are *started* on `oid`, in a stable order. Java
/// subscribes a damage trigger in its effect's `onStart`, so these are the
/// carriers: every live non-passive buff (toggles included), and every
/// passive skill the creature holds whose `<passiveConditions>` pass —
/// `Creature.addSkill` starts a passive's effects through `applyEffects(…,
/// passive = true)`, which checks `PASSIVE` conditions first.
///
/// A player's passives come from the book plus the transient clan and augment
/// grants; an NPC's (servitors and pets included) from its template's skill
/// list, with no inventory to satisfy an equipment condition.
fn started_skills(world: &World, oid: i32) -> Vec<Skill> {
    use crate::model::components::skills::{ClanSkills, OptionSkills, SkillBook};
    use crate::model::skill::target::OperateType;

    let mut out: Vec<Skill> = world
        .objects
        .get_component::<Buffs>(&oid)
        .map(known_buffs)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(id, level)| skill_by_id(world, id, level))
        .collect();

    let mut held: Vec<(i32, i32)> = Vec::new();
    if let Some(book) = world.objects.get_component::<SkillBook>(&oid) {
        held.extend(book.0.iter().map(|(&id, &lvl)| (id, lvl)));
        if let Some(c) = world.objects.get_component::<ClanSkills>(&oid) {
            held.extend(c.0.iter().map(|(&id, &lvl)| (id, lvl)));
        }
        if let Some(o) = world.objects.get_component::<OptionSkills>(&oid) {
            held.extend(o.0.iter().map(|(&id, &lvl)| (id, lvl)));
        }
    } else if let Some(t) = crate::game_loop::npc::npc_template(world, oid) {
        held.extend(t.skill_list.iter().copied());
    }
    held.sort_unstable();
    let no_items = crate::model::inventory::Inventory::default();
    let inventory = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&oid)
        .unwrap_or(&no_items);
    out.extend(held.into_iter().filter_map(|(id, level)| {
        world
            .data
            .skill_data
            .get(id, level)
            .filter(|s| s.operate_type == OperateType::Passive)
            .filter(|s| {
                crate::game_loop::skills::conditions::passive_stat_gate(
                    s,
                    inventory,
                    &world.data.item_data,
                )
            })
            .cloned()
    }));
    out
}

/// `event.getAttacker().getInstanceType().isType(_attackerType)`.
fn attacker_type_matches(world: &World, oid: i32, wanted: AttackerType) -> bool {
    match wanted {
        AttackerType::Creature => true,
        AttackerType::Playable => crate::game_loop::helpers::is_playable(world, oid),
        AttackerType::Attackable => {
            crate::game_loop::npc::npc_template(world, oid)
            .is_some_and(|t| t.is_attackable_class())
            // A summon is an NPC entity here but `Summon`, not `Attackable`,
            // in Java's hierarchy.
            && !crate::game_loop::helpers::is_playable(world, oid)
        }
    }
}

/// `getAttacker().getLevel()` against the `min`/`maxAttackerLevel` window.
fn attacker_level_fits(world: &World, oid: i32, trigger: &DamageTrigger) -> bool {
    let level = super::creature_level(world, oid);
    level >= trigger.min_attacker_level && level <= trigger.max_attacker_level
}

/// The skill both handlers hand to `TargetHandler.getTarget`: the plain
/// `skillId`, or the ladder's first rung.
fn targeting_skill(world: &World, trigger: &DamageTrigger) -> Option<Skill> {
    let (id, level) = trigger
        .trigger_skills
        .first()
        .copied()
        .unwrap_or((trigger.skill_id, trigger.skill_level));
    skill_by_id(world, id, level)
}

/// The shared tail of both handlers once the target is resolved: pick the
/// skill to fire and `triggerCast` it. `caster_of` is the one difference —
/// the dealt side always casts as the attacker, the received side as the
/// target when the fired skill is `SELF`-targeted.
///
/// Without `triggerSkills`: the target's existing copy of the skill decides —
/// it is re-cast only at a higher level, and `skillLevelScaleTo` steps that
/// level up one per fire. With a ladder: the target's current rung is removed
/// silently and the next one cast; standing on the top rung fires nothing.
fn fire_trigger(
    world: &mut World,
    trigger: &DamageTrigger,
    target: i32,
    // `Some(attacker)` on the dealt side: Java checks the attacker's
    // `isSkillDisabled(buffInfo.getSkill())` before scaling there only.
    scale_gate: Option<i32>,
    caster_of: impl Fn(&Skill) -> i32,
) {
    let skill = if trigger.trigger_skills.is_empty() {
        let buff_level =
            crate::game_loop::skills::cast::buff_level(world, target, trigger.skill_id);
        let skill = match buff_level {
            Some(have) if trigger.skill_level_scale_to > 0 => {
                if let Some(attacker) = scale_gate
                    && skill_by_id(world, trigger.skill_id, have).is_some_and(|held| {
                        crate::game_loop::skills::cast::is_skill_disabled(world, attacker, &held)
                    })
                {
                    return;
                }
                skill_by_id(
                    world,
                    trigger.skill_id,
                    trigger.skill_level_scale_to.min(have + 1),
                )
            }
            _ => skill_by_id(world, trigger.skill_id, trigger.skill_level),
        };
        let Some(skill) = skill else {
            return;
        };
        if buff_level.is_some_and(|have| have >= skill.level) {
            return;
        }
        skill
    } else {
        let ladder = &trigger.trigger_skills;
        let mut next = ladder[0];
        for (i, &(id, _)) in ladder.iter().enumerate() {
            if crate::game_loop::skills::cast::buff_level(world, target, id).is_none() {
                continue;
            }
            let Some(&up) = ladder.get(i + 1) else {
                return;
            };
            // `stopSkillEffects(SkillFinishType.SILENT, id)`.
            super::expire_buffs_where(world, target, |_, b| b.skill_id == id);
            next = up;
            break;
        }
        let Some(skill) = skill_by_id(world, next.0, next.1) else {
            return;
        };
        skill
    };
    let caster = caster_of(&skill);
    crate::game_loop::skills::cast::trigger_cast(world, caster, target, skill);
}

/// `OnCreatureDamageDealt` — every `TriggerSkillByDamageDealt` started on the
/// attacker hears the hit (`TriggerSkillByDamageDealt.onAttackEvent`).
pub(crate) fn fire_damage_dealt_triggers(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
    hit: DamageHit,
) {
    let listeners: Vec<SkillEffect> = started_skills(world, attacker)
        .into_iter()
        .flat_map(|s| s.effects)
        .filter(|e| matches!(e, SkillEffect::TriggerSkillByDamageDealt { .. }))
        .collect();
    for effect in listeners {
        let SkillEffect::TriggerSkillByDamageDealt {
            trigger,
            is_critical,
            allow_normal_attack,
            allow_skill_attack,
            allow_reflect,
            allow_weapons,
        } = effect
        else {
            continue;
        };
        if hit.dot
            || trigger.chance == 0
            || (trigger.trigger_skills.is_empty()
                && (trigger.skill_id == 0 || trigger.skill_level == 0))
            || (!allow_normal_attack && !allow_skill_attack)
        {
            continue;
        }
        // Equality, not "crits allowed": `isCritical=false` fires on
        // non-crits only.
        if is_critical != hit.critical {
            continue;
        }
        if (!allow_skill_attack && hit.skill) || (!allow_normal_attack && !hit.skill) {
            continue;
        }
        if !allow_reflect && hit.reflect {
            continue;
        }
        if attacker == target || !attacker_level_fits(world, attacker, &trigger) {
            continue;
        }
        // One short-circuiting condition in Java, so the roll is spent only
        // when the damage floor passes — and `Rnd.get(100) > chance` lets
        // `chance` itself through.
        if damage < f64::from(trigger.min_damage)
            || world.roll(100) > trigger.chance
            || !attacker_type_matches(world, attacker, trigger.attacker_type)
        {
            continue;
        }
        // `getActiveWeaponItem()` is null for an NPC, so a weapon-gated
        // trigger never fires from one.
        if allow_weapons > 0 && !attacker_weapon_allowed(world, attacker, allow_weapons) {
            continue;
        }
        let Some(skill) = targeting_skill(world, &trigger) else {
            continue;
        };
        let Some(resolved) = crate::game_loop::skills::cast::resolve_handler_target(
            world,
            attacker,
            target,
            trigger.target_type,
            &skill,
            false,
        ) else {
            continue;
        };
        fire_trigger(world, &trigger, resolved, Some(attacker), |_| attacker);
    }
}

/// `OnCreatureDamageReceived` — every `TriggerSkillByDamageReceived` started
/// on the victim hears the hit (`onDamageReceivedEvent`). The target handler
/// runs from the *victim's* side with the attacker as the selection, so `SELF`
/// is the bearer and `ENEMY` whoever hit them.
pub(crate) fn fire_damage_received_triggers(
    world: &mut World,
    victim: i32,
    attacker: i32,
    damage: f64,
    hit: DamageHit,
) {
    let listeners: Vec<SkillEffect> = started_skills(world, victim)
        .into_iter()
        .flat_map(|s| s.effects)
        .filter(|e| matches!(e, SkillEffect::TriggerSkillByDamageReceived { .. }))
        .collect();
    for effect in listeners {
        let SkillEffect::TriggerSkillByDamageReceived {
            trigger,
            hp_percent,
        } = effect
        else {
            continue;
        };
        if hit.dot
            || trigger.chance == 0
            || (trigger.trigger_skills.is_empty()
                && (trigger.skill_id == 0 || trigger.skill_level == 0))
        {
            continue;
        }
        if attacker == victim || !attacker_level_fits(world, attacker, &trigger) {
            continue;
        }
        if damage < f64::from(trigger.min_damage) {
            continue;
        }
        // Unlike the dealt side, a certain trigger spends no roll.
        if trigger.chance < 100 && world.roll(100) > trigger.chance {
            continue;
        }
        // `hpPercent` is an *upper* bound: the bearer must be at or below it.
        if hp_percent < 100 {
            let share = world
                .objects
                .get_component::<Vitals>(&victim)
                .filter(|v| v.max_hp > 0)
                .map(|v| v.cur_hp * 100.0 / v.max_hp as f64)
                .unwrap_or(100.0);
            if share > f64::from(hp_percent) {
                continue;
            }
        }
        if !attacker_type_matches(world, attacker, trigger.attacker_type) {
            continue;
        }
        let Some(skill) = targeting_skill(world, &trigger) else {
            continue;
        };
        let Some(resolved) = crate::game_loop::skills::cast::resolve_handler_target(
            world,
            victim,
            attacker,
            trigger.target_type,
            &skill,
            false,
        ) else {
            continue;
        };
        // `triggerCast(triggerSkill.getTargetType() == SELF ? target :
        // event.getAttacker(), target, triggerSkill)`.
        fire_trigger(world, &trigger, resolved, None, |fired| {
            if fired.target_type == TargetType::Self_ {
                resolved
            } else {
                attacker
            }
        });
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
    crate::model::equip_conditions::weapon_condition_passes(mask, inv, &world.data.item_data)
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
///
/// Each proc is `SkillCaster.triggerCast(caster, target, skill, null, false)`:
/// it waits in the hit target's queue, and the fired skill resolves its own
/// `targetType` against the bearer's selection — a self-buff proc lands on
/// the bearer, a debuff proc on whoever they have selected.
fn fire_option_triggers(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    want: impl Fn(crate::data::option_data::OptionSkillType) -> bool,
) {
    let Some(reg) = world
        .objects
        .get_component::<crate::model::components::skills::OptionTriggers>(&caster_oid)
    else {
        return;
    };
    if reg.0.is_empty() {
        return;
    }
    let candidates: Vec<crate::data::option_data::OptionTrigger> =
        reg.0.values().filter(|t| want(t.kind)).copied().collect();

    for t in candidates {
        if t.skill_id == 0 || t.skill_level == 0 {
            continue;
        }
        if (world.roll(100) as f64) >= t.chance {
            continue;
        }
        let Some(skill) = skill_by_id(world, t.skill_id, t.skill_level) else {
            continue;
        };
        crate::game_loop::skills::cast::trigger_cast_at_own_target(
            world, caster_oid, target_oid, skill,
        );
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

/// `SkillCaster.callSkill`'s trigger loop — the finished-cast half, run once
/// per affected target before the skill's own effects land. `magic_type` is
/// the cast skill's, so `1` is magic and `0` physical; a **static** skill (2)
/// fires nothing, which is Java's `!skill.isStatic()` gate around the block.
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

/// `CallSkill.instant` — trigger-cast another skill at the effected.
///
/// A called skill with a `hitTime` plays its animation now and lands that long
/// after (`ThreadPool.schedule(() -> triggerCast(...), hitTime)`); one without
/// lands at once.
#[allow(clippy::too_many_arguments)]
pub(super) fn call_skill(
    world: &mut World,
    effector: i32,
    effected: i32,
    skill: &Skill,
    skill_id: i32,
    skill_level: i32,
    skill_level_scale_to: i32,
    chance: i32,
) {
    if chance < 100 && world.roll(100) > chance {
        return;
    }
    let called = if skill_level_scale_to <= 0 {
        if skill_level == 0 {
            // Mobius: "Use 0 to trigger max effector learned skill level."
            let Some(known) = learned_level(world, effector, skill_id).filter(|&l| l > 0) else {
                return;
            };
            skill_by_id(world, skill_id, known)
        } else {
            skill_by_id(world, skill_id, skill_level)
        }
    } else {
        match crate::game_loop::skills::cast::buff_level(world, effected, skill_id) {
            Some(have) => skill_by_id(world, skill_id, skill_level_scale_to.min(have + 1)),
            None => skill_by_id(world, skill_id, skill_level),
        }
    };
    let Some(called) = called else {
        return;
    };
    // "Prevent infinite loop."
    if skill.id == called.id && skill.level == called.level {
        return;
    }
    if called.hit_time > 0 {
        if crate::game_loop::skills::cast::is_skill_disabled(world, effector, &called) {
            return;
        }
        if let (Some(from), Some(to)) = (
            crate::game_loop::space::position::maybe_position(world, effector),
            crate::game_loop::space::position::maybe_position(world, effected),
        ) {
            // `new MagicSkillUse(effector, effected, displayId, level,
            // hitTime, 0)`.
            let pkt = crate::network::server_packets::magic_skill_use_raw(
                (effector, from.x, from.y, from.z),
                (effected, to.x, to.y, to.z),
                called.id,
                called.level,
                called.hit_time,
            );
            crate::game_loop::net::broadcast::broadcast_including_self(world, effector, &pkt);
        }
        world.scheduler.schedule(
            world.tick + crate::scheduler::ms_to_ticks(called.hit_time),
            crate::scheduler::ScheduledTask::CallSkillCast {
                caster: effector,
                target: effected,
                skill_id: called.id,
                skill_level: called.level,
            },
        );
    } else {
        crate::game_loop::skills::cast::trigger_cast(world, effector, effected, called);
    }
}

/// The `CallSkill` cast that waited out its `hitTime`.
pub(crate) fn handle_call_skill_cast(
    world: &mut World,
    caster: i32,
    target: i32,
    skill_id: i32,
    skill_level: i32,
) {
    if let Some(skill) = skill_by_id(world, skill_id, skill_level) {
        crate::game_loop::skills::cast::trigger_cast(world, caster, target, skill);
    }
}

/// `Creature.getSkillLevel(id)` — a player's known level, or the level on an
/// NPC's template skill list.
fn learned_level(world: &World, oid: i32, skill_id: i32) -> Option<i32> {
    if world.objects.has_component::<Player>(&oid) {
        return crate::game_loop::skills::cast::known_skill_level(world, oid, skill_id);
    }
    crate::game_loop::npc::npc_template(world, oid)?
        .skill_list
        .iter()
        .find(|&&(id, _)| id == skill_id)
        .map(|&(_, level)| level)
}

/// `CallSkillOnActionTime.onActionTime` — one pulse of an aura toggle.
///
/// A synergy (`A6`) pulse first lands on the effector itself. Then, for every
/// creature within the pulsed skill's `affectRange` of the effector, Java
/// resolves that skill's own `targetType` against the creature
/// (`getTarget(effector, c, false, false, false)`) and `triggerCast`s at
/// whatever it yields — which expands the skill's affect scope around that
/// target. Nothing is deduplicated: a party-scope aura is re-applied once per
/// nearby creature, which is what keeps it refreshed, and a debuff aura rolls
/// to land once per attempt.
pub(crate) fn call_skill_on_action_time(
    world: &mut World,
    effector_oid: i32,
    skill_id: i32,
    skill_level: i32,
) {
    let Some(trigger) = skill_by_id(world, skill_id, skill_level) else {
        return;
    };
    if trigger.synergy {
        apply_skill_effects(world, effector_oid, effector_oid, &trigger);
    }
    let range = trigger.affect_range as f64;
    let nearby: Vec<i32> =
        crate::game_loop::space::visibility::visible_creatures(world, effector_oid)
            .into_iter()
            .filter(|&c| {
                crate::geo::distance::distance_3d(world, effector_oid, c)
                    .is_some_and(|d| d <= range)
            })
            .collect();
    for creature in nearby {
        let Some(target) = crate::game_loop::skills::cast::resolve_handler_target(
            world,
            effector_oid,
            creature,
            trigger.target_type,
            &trigger,
            false,
        ) else {
            continue;
        };
        crate::game_loop::skills::cast::trigger_cast(world, effector_oid, target, trigger.clone());
    }
}
