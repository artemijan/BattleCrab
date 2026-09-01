use super::apply_block_actions_interrupt;
use super::apply_buff_to_npc;
use super::apply_mute_interrupt;
use super::attribute_mod;
use super::calc_general_trait_bonus;
use super::calc_skill_mastery;
use super::casting_resists_abnormal;
use super::creature_level;
use super::creature_name;
use super::merge_attack_traits;
use super::merge_defence_traits;
use super::merge_skill_rates;
use super::recompute_max_vitals;
use super::schedule_dam_over_time;
use crate::game_loop::helpers;
use crate::game_loop::space::position::maybe_position;

use crate::model::components::Buffs;
use crate::model::formulas;
use crate::model::skill::ActiveBuff;
use crate::model::skill::Skill;
use crate::model::skill::SkillEffect;
use crate::model::skill::abnormal_type_client_id;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// The continuous half of Java `Skill.applyEffects` — everything that turns a
/// cast into one timed `ActiveBuff` on the target — split out from the instant
/// (damage/heal) half above so it can be driven on its own.
///
/// `abnormal_time_override` is Java's `abnormalTime` parameter: `None` uses the
/// skill's own `abnormalTime`, `Some(secs)` overrides it. Buff restore at login
/// is the caller that passes it, mirroring Java `restoreEffects`'
/// `skill.applyEffects(this, this, false, remainingTime)` — the `instant =
/// false` there is exactly why this half has to be separable, since a restored
/// buff must not re-fire the skill's damage or heal.
/// `Grow.onStart` / `Grow.onExit` — swap an NPC's collision cylinder between
/// its template's normal and `grown` measurements.
///
/// Java reads the template on both edges rather than remembering what it
/// replaced, so a mob that grew keeps its *template's* normal size on exit even
/// if something else changed the cylinder meanwhile. Ported as written: it is
/// the same read either way, just a different pair of fields.
///
/// A template with no `grown` values (0.0) is left alone — Java would shrink it
/// to nothing, which is plainly not the intent and which no carrier hits, since
/// every NPC that casts a `Grow` skill declares both.
pub(crate) fn set_collision_grown(world: &mut World, npc_oid: i32, grown: bool) {
    let Some(size) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
        .map(|t| {
            if grown {
                (t.collision_radius_grown, t.collision_height_grown)
            } else {
                (t.collision_radius, t.collision_height)
            }
        })
        .filter(|(r, h)| *r > 0.0 && *h > 0.0)
    else {
        return;
    };
    if let Some(c) = world
        .objects
        .get_component_mut::<crate::model::components::Collision>(&npc_oid)
    {
        c.radius = size.0;
        c.height = size.1;
    }
}

pub(crate) fn apply_continuous_effects(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    abnormal_time_override: Option<i32>,
) -> bool {
    // Continuous effects → one ActiveBuff on the target (`applyEffects`).
    let buff_effects = skill.stat_modifier_effects();
    // A `DamOverTime` (poison/bleed) debuff has no stat modifier but still
    // lands as a timed buff (for the icon + expiry) whose ticks are armed
    // below — so it must not bail here on an empty `buff_effects`.
    // Any effect whose whole job happens on the periodic tick chain: it carries
    // no stat modifier, but the buff must still land (for the icon, the expiry
    // and — crucially — to keep the tick chain alive, which stops the moment
    // the buff is gone).
    let has_periodic = skill.effects.iter().any(|e| {
        matches!(
            e,
            SkillEffect::DamOverTime { .. }
                | SkillEffect::HealOverTime { .. }
                | SkillEffect::ManaDamOverTime { .. }
                | SkillEffect::ManaHealOverTime { .. }
                | SkillEffect::MpConsumePerLevel { .. }
                | SkillEffect::Relax { .. }
                | SkillEffect::ChameleonRest { .. }
                | SkillEffect::Fear { .. }
                | SkillEffect::FakeDeath { .. }
        )
    });
    // Blessing of Protection, DefenceTrait (Mental Shield / Resist Shock) and
    // VampiricAttack (Vampiric Rage) likewise carry no `StatModifier` and must
    // still land as a timed buff (their abnormal + duration). That is a
    // statement about the *shape* of their effects, not about coverage: two of
    // the three are fully modelled — `DefenceTrait` merges into
    // `DefenceTraits` further down this function, and `VampiricAttack` pumps
    // `AbsorbDamagePercent`/`VampiricSum`, which `combat::damage`'s
    // `absorb_damage_to_hp` reads. They just do it outside the buff's stat map.
    // Stun/sleep/paralyze/root carry no stat modifier either — their whole
    // mechanic is the abnormal flag — so they must survive this guard too.
    // State-only effects carry no stat modifier: the CC flags, and
    // `BlockAbnormalSlot`'s blocked-type set. Both must survive the
    // empty-effects guard or the buff is dropped whole and never lands.
    let has_state_flag = skill.effect_flags() != 0 || !skill.blocked_abnormals().is_empty();
    // `Transformation` also carries no stat modifier of its own (the transform
    // template's stat/speed overrides apply separately) but must still land as
    // a timed `TRANSFORM` buff — that buff's expiry is what drives the revert.
    let has_iconless_buff = skill.effects.iter().any(|e| {
        matches!(
            e,
            SkillEffect::ProtectionBlessing
                | SkillEffect::DefenceTrait { .. }
                | SkillEffect::VampiricAttack { .. }
                | SkillEffect::MagicMpCost { .. }
                | SkillEffect::Reuse { .. }
                | SkillEffect::DamageShield { .. }
                | SkillEffect::Transform { .. }
                | SkillEffect::AttackTrait { .. }
                // `TargetMe` carries no stat modifier and stamps no
                // `effect_flag` — its whole mechanic is the `LockedTarget`
                // component, which `handle_buff_expire` clears. Without this
                // the buff is dropped by the guard, the expiry hook never
                // runs, and the taunt lock becomes **permanent**. Fifth slice
                // caught by this guard; any new modifier-less effect must join
                // one of its three categories.
                | SkillEffect::TargetMe
                // `SkillEvasion` likewise: its contribution lives in a
                // per-magicType map that only `handle_buff_expire` unmerges,
                // so a dropped buff makes the dodge chance permanent.
                | SkillEffect::SkillEvasion { .. }
            // `Lucky` is an empty effect in Java too — `Player.isLucky()` asks
            // whether the buff is *present*, so landing is the whole job.
            | SkillEffect::Lucky
            // `Grow` is modifier-less in the same way: the swell is applied by
            // its `onStart` and undone by `handle_buff_expire`, so a buff
            // dropped by this guard would leave the mob permanently grown.
            // Every real carrier also has a stat half (Might 4028 pumps PAtk),
            // which is why this was survivable — but relying on a *sibling*
            // effect to keep the buff alive is exactly the trap the comments
            // above record, so `Grow` joins the list on its own account.
            | SkillEffect::Grow
            // Its grant is written *after* the buff lands (by `night_stats`),
            // so at guard time it looks modifier-less. Tenth slice caught here.
            | SkillEffect::NightStatModify { .. }
            // The two listener-shaped triggers: Java attaches their listener to
            // the **buff**, and this port finds them by scanning the bearer's
            // buff list, so a dropped buff means the trigger never fires at
            // all. Seventh and eighth slices caught by this guard.
            | SkillEffect::TriggerSkillByDamage { .. }
            | SkillEffect::TriggerSkillByMagicType { .. }
        )
    });
    if buff_effects.is_empty() && !has_periodic && !has_iconless_buff && !has_state_flag {
        return false;
    }

    // Debuff landing roll — Java `Formulas.calcEffectSuccess`. A bad skill with
    // an `activateRate` (≠ -1) can be resisted: compute the chance, roll it, and
    // report the outcome to the caster with the computed chance baked in — a
    // "landed with X% chance on <target>" line on success, or a
    // "<target> has resisted <skill>: X%" line on a failed roll (which also skips
    // the buff and its DoT ticks). Self-targeted casts never resist (Java's
    // `target != attacker`). Buffs and always-land debuffs (`-1`) fall straight
    // through. `activateRate == -1` is filtered here so those consume no roll
    // (keeps the ordering of the remaining rolls stable). Both lines are
    // single-target only so an AoE debuff doesn't spam one line per target.
    // `calcEffectSuccess`'s first resist clause, ahead of the roll: a target
    // that is **casting** a skill whose `<abnormalResists>` names this skill's
    // `abnormalType` shrugs it off outright. That is what makes the long-ritual
    // skills uninterruptible — 176 skills declare a list, 146 of them the full
    // crowd-control set.
    if skill.is_debuff
        && caster_oid != target_oid
        && casting_resists_abnormal(world, target_oid, &skill.abnormal_type)
    {
        return false;
    }
    // Java gates this on **`activateRate != -1` alone** — not on `isBad()`.
    // `Skill.applyEffects` runs `_operateType.isContinuous() && calcEffectSuccess(…)`
    // for every continuous skill, and `calcEffectSuccess` returns early only for
    // the `-1` sentinel. Three learnable skills on this dist fall in the gap an
    // `isBad()` gate would open:
    //
    // - **Veil (106)** — `isDebuff`, `activateRate 70`, trait `DERANGEMENT`, and
    //   *no* `<effectPoint>` at all, so `effectPoint < 0` is false. Gated on
    //   `isBad()` it would be an unresistable mesmerize.
    // - **Greater Heal (1217)** / **Greater Group Heal (1219)** — `activateRate 0`
    //   with no `<lvlBonusRate>`, so `baseMod` is a flat 30 and the
    //   `LIFE_FORCE_OTHERS` regeneration rides along only ~30 % of the time when
    //   the heal is cast on *someone else*. The instant heal is a separate
    //   (non-continuous) effect and always lands; it is the over-time half that
    //   rolls. Surprising, but it is what the formula says.
    if caster_oid != target_oid && skill.activate_rate != -1 {
        let target_level = creature_level(world, target_oid);
        // Java: `skill.isDebuff() ? target.getStat().getValue(RESIST_ABNORMAL_DEBUFF, 1) : 1`.
        let debuff_resist_mod = if skill.is_debuff {
            helpers::stat_mul(
                world,
                target_oid,
                crate::model::stats::Stat::ResistAbnormalDebuff,
            )
        } else {
            1.0
        };
        let rate = formulas::calc_effect_land_rate(
            skill.magic_level,
            skill.activate_rate,
            skill.lvl_bonus_rate,
            target_level,
            debuff_resist_mod,
            // `calcEffectSuccess`'s `elementMod` — an elemental debuff lands
            // more easily on a target weak to its element.
            attribute_mod(world, caster_oid, target_oid, skill),
            calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false),
            // The two `<basicProperty>` terms — a stat subtracted inside
            // `baseMod`, and the mesmerizing-debuff chain multiplied after the
            // clamp (G34 S2, `game_loop::stats::basic_property`).
            crate::game_loop::stats::basic_property::abnormal_resist(
                world,
                target_oid,
                skill.basic_property,
            ),
            crate::game_loop::stats::basic_property::resist_bonus(
                world,
                target_oid,
                skill.basic_property,
            ),
            formulas::LandRateBounds::of(&world.cfg.character),
        );
        // Java: resisted when `finalRate <= Rnd.get(100)` (0-99). Roll before the
        // message so the outcome line reflects it and the roll order stays stable.
        let resisted = rate <= world.roll(100) as f64;
        if skill.affect_scope == crate::model::skill::AffectScope::Single {
            // Two of this server's own messages (ids 9000/9001), so the client
            // renders and colours them like any other rather than receiving a
            // sentence we formatted. They only display once the client table
            // has been rebuilt — `l2r-tools client-dat sync-messages`.
            use commons::system_messages::SmValue;
            use commons::system_messages::generated::{
                C1_HAS_RESISTED_S2_CHANCE_WAS_S3, S1_LANDED_ON_C2_CHANCE_WAS_S3,
            };
            let target_name = creature_name(world, target_oid);
            let spell = SmValue::Skill {
                id: skill.id,
                level: skill.level,
            };
            let chance = rate as i32;
            let message = if resisted {
                C1_HAS_RESISTED_S2_CHANCE_WAS_S3::new(target_name, spell, chance)
            } else {
                S1_LANDED_ON_C2_CHANCE_WAS_S3::new(spell, target_name, chance)
            };
            helpers::send_to_player(world, caster_oid, server_packets::system_message(&message));
        }
        if resisted {
            return false;
        }
    }
    // Java `EffectList` only schedules a stop task when the effect's time is
    // positive; a toggle or a 0-`abnormalTime` buff (e.g. Super Haste 7029,
    // `operateType=T`) persists until it's toggled/removed. Model that as a
    // sentinel expiry with no `BuffExpire` schedule, else it would vanish the
    // same tick it lands.
    // `Formulas.calcMagicAffected`: a target under `DEBUFF_BLOCK` (Mystic
    // Immunity, Celestial Shield) refuses every incoming debuff outright — no
    // roll, no partial landing. Self-cast is exempt for the same reason the
    // resist roll is: Java compares `target != attacker`.
    if skill.is_debuff
        && caster_oid != target_oid
        && crate::game_loop::abnormal::is_debuff_blocked(world, target_oid)
    {
        return false;
    }
    // The mirror image, from `EffectList.add`:
    // `if (info.getEffected().isBuffBlocked() && !skill.isBad()) return;`.
    // Note it keys on `isBad()` (effectPoint < 0) rather than `isDebuff()`, and
    // has **no self-cast exemption** — Dance of Medusa stops the victim
    // buffing themselves too, which is the point of it (G34 S3).
    if !skill.is_bad() && crate::game_loop::abnormal::is_buff_blocked(world, target_oid) {
        return false;
    }

    // `EffectList.addActive`'s blocked-slot gate: a buff whose abnormal type is
    // in the target's blocked set (from a live `BlockAbnormalSlot`) can't land
    // at all. This is what keeps two Prophecies off the same character.
    // "NONE" is the no-abnormal sentinel and is never blockable.
    if skill.abnormal_type != "NONE" {
        let blocked = world
            .objects
            .get_component::<Buffs>(&target_oid)
            .is_some_and(|b| {
                b.0.iter()
                    .any(|x| x.blocked_abnormals.contains(&skill.abnormal_type))
            });
        if blocked {
            return false;
        }
    }

    // `Formulas.calcEffectAbnormalTime`, which every `BuffInfo` runs through its
    // constructor:
    //
    // ```java
    // int time = (skill == null) || skill.isPassive() || skill.isToggle() ? -1 : skill.getAbnormalTime();
    // if ((skill != null) && !skill.isStatic() && calcSkillMastery(caster, skill)) time *= 2;
    // ```
    //
    // **A Skill Mastery proc doubles the duration.** This is a second, wholly
    // independent roll from the one `apply_reuse` makes to collapse the
    // cooldown, and — unlike that one — it is *not* gated to `operateType A1`:
    // Java excludes only static skills here. So an Eva's Saint who learns Skill
    // Mastery (331, level 77) rolls it on every buff they land and sometimes
    // gets twice the duration, which is the whole reason the skill is worth
    // 11 M SP to a healer. `magic_type == 2` is `isStatic()`.
    //
    // The passive/toggle `-1` branch is shape, not behaviour: no toggle on this
    // dist declares an `abnormalTime`, so it is already 0 → permanent.
    let mastered = skill.magic_type != 2 && calc_skill_mastery(world, caster_oid);
    let base_abnormal_time = if mastered {
        skill.abnormal_time * 2
    } else {
        skill.abnormal_time
    };
    // Java `BuffInfo.setAbnormalTime` is applied only for a *positive* override
    // ("if equal or lesser than zero will be ignored"), so a bad stored value
    // falls back to the skill's own duration rather than making the buff permanent.
    // It also lands *after* the constructor, so an override beats the doubling.
    let abnormal_time = abnormal_time_override
        .filter(|&t| t > 0)
        .unwrap_or(base_abnormal_time);
    let permanent = abnormal_time <= 0;
    let expires_at_tick = if permanent {
        u64::MAX
    } else {
        world.tick + abnormal_time as u64 * 10
    };
    let buff = ActiveBuff {
        skill_id: skill.id,
        skill_level: skill.level,
        abnormal_type_client_id: abnormal_type_client_id(&skill.abnormal_type),
        abnormal_type: skill.abnormal_type.clone(),
        abnormal_level: skill.abnormal_level,
        slot: skill.buff_slot(),
        expires_at_tick,
        // `BuffInfo.isDisplayedForEffected()`. A self-continuous skill that
        // also has `<selfEffects>` shows no icon to anyone but the caster.
        displayed: !skill.self_continuous
            || caster_oid == target_oid
            || skill.self_effects.is_empty(),
        passive: false,
        effect_flags: skill.effect_flags(),
        blocked_abnormals: skill.blocked_abnormals(),
        abnormal_visuals: skill.abnormal_visuals.clone(),
        effects: buff_effects,
    };

    // Java `Skill.applyEffects`, inside the `if (addContinuousEffects)` branch
    // and immediately after `EffectList.add(info)`: "Check for mesmerizing
    // debuffs and increase resist level." Position matters — it is on the
    // *landed* path, past the resist roll, so a debuff that keeps failing never
    // builds the resistance that would lock it out (G34 S2).
    //
    // `addContinuousEffects` is `isToggle() || (isContinuous() && …)`, so an
    // instant-only debuff does not accrue; `increase_resist_level` filters the
    // `NONE` property and the can't-accrue targets (every player on this dist).
    if skill.is_debuff
        && (skill.is_continuous || skill.operate_type == crate::model::skill::OperateType::Toggle)
    {
        crate::game_loop::stats::basic_property::increase_resist_level(
            world,
            target_oid,
            skill.basic_property,
        );
    }

    // Arm the poison/bleed damage-over-time ticks (Java `BuffInfo.
    // scheduleEffects` → `scheduleAtFixedRate`). The recurring `DamOverTimeTick`
    // self-terminates once this buff's `BuffExpire` removes it or the target
    // dies; done here so it covers both NPC and player targets.
    schedule_dam_over_time(world, caster_oid, target_oid, skill);

    // `DefenceTrait.onStart` — merge the buff's per-trait resistances. Done
    // here, above the NPC/player split, because a resisted mob is as real as a
    // resisted player.
    for effect in &skill.effects {
        match effect {
            SkillEffect::DefenceTrait { traits } => merge_defence_traits(world, target_oid, traits),
            // `AttackTrait.onStart` — the attacker-side twin. Note it merges
            // onto the **effected**, which for these self-buffs is the caster.
            SkillEffect::AttackTrait { traits } => merge_attack_traits(world, target_oid, traits),
            // `Grow.onStart` — `setCollisionHeight/Radius(getTemplate()
            // .getCollision*Grown())`, NPCs only.
            SkillEffect::Grow => set_collision_grown(world, target_oid, true),
            _ => {}
        }
    }
    // `MagicMpCost.onStart` / `Reuse.onStart` — same place, same reasoning.
    merge_skill_rates(world, target_oid, skill);

    // NPC target: buffs modify the mob's server-side stats (no buff icons —
    // those are self-only — and no NpcInfo re-broadcast, so a speed change
    // isn't reflected client-side until respawn; the combat math uses it now).
    if crate::game_loop::combat::is_npc_oid(target_oid) {
        apply_buff_to_npc(world, target_oid, buff, skill.id);
        if skill.effect_flags() & crate::model::skill::effect_flag::BLOCK_ACTIONS != 0 {
            apply_block_actions_interrupt(world, target_oid);
        }
        apply_mute_interrupt(world, target_oid, skill);
        if !permanent {
            world.scheduler.schedule(
                expires_at_tick,
                ScheduledTask::BuffExpire {
                    player_object_id: target_oid,
                    skill_id: skill.id,
                },
            );
        }
        // The NPC branch is the *success* tail, not a guard: the buff was
        // applied, so an `onStart` side effect keyed on landing (the DoT
        // magic-crit burst) is due.
        return true;
    }
    {
        // A target missing any of the seven components can't hold a buff, which
        // is the same "didn't land" answer a refusal gives.
        let landed = crate::game_loop::stats::context::with_stat_ctx(world, target_oid, |ctx| {
            ctx.apply(buff)
        })
        .unwrap_or(false);
        // A refused buff (a same-type buff of equal/higher level is already up)
        // changes nothing — don't schedule its expiry (a stale `BuffExpire` on a
        // shared skill id would drop the surviving buff early) or rebroadcast.
        if !landed {
            return false;
        }
        if !permanent {
            world.scheduler.schedule(
                expires_at_tick,
                ScheduledTask::BuffExpire {
                    player_object_id: target_oid,
                    skill_id: skill.id,
                },
            );
        }
        let now = world.tick;
        if let Some(buffs) = world.objects.get_component::<Buffs>(&target_oid) {
            helpers::send_to_player(
                world,
                target_oid,
                crate::network::enter_world::abnormal_status_update(buffs, now),
            );
        }
        // Max HP/MP/CP live on a separate path from `recalculate_stats`; fold
        // the buff's MaxHp/MaxMp/MaxCp modifiers into them too (e.g. a +MP buff).
        recompute_max_vitals(world, target_oid);
        if skill.effect_flags() & crate::model::skill::effect_flag::BLOCK_ACTIONS != 0 {
            apply_block_actions_interrupt(world, target_oid);
        }
        apply_mute_interrupt(world, target_oid, skill);
        // A stat buff changed pAtk/pDef/speed/…; Java's `recalculateStats(true)`
        // follows with `broadcastUserInfo()`. Without this the client shows the
        // buff icon but never the changed stats or movement speed (and other
        // players never see the speed change).
        crate::game_loop::character::player_info::broadcast_user_info(world, target_oid);
        // Java pushes the visual set only from `startAbnormalVisualEffect` /
        // `stopAbnormalVisualEffect`, i.e. only when the set actually changed —
        // not on every buff. A skill with no `<abnormalVisualEffect>` can't have
        // changed anything, so it sends nothing.
        if !skill.abnormal_visuals.is_empty() {
            refresh_abnormal_visuals(world, target_oid);
        }
        // `Transformation` landed: the `UserInfo`/`CharInfo` broadcast above
        // already carries the new display id, but the client also needs the
        // self-only `ExUserInfoAbnormalVisualEffect` (transform display id) and
        // a refreshed `SkillList` for the transform's granted skills to show up
        // — the two extras `admin::transforms::apply_transform`'s broadcast
        // sends on top of `broadcast_user_info`.
        if skill
            .effects
            .iter()
            .any(|e| matches!(e, SkillEffect::Transform { .. }))
        {
            crate::game_loop::admin::transforms::refresh_transform_visuals(world, target_oid);
        }
    }
    true
}

/// Java `Player.restoreEffects` (buff half): re-apply the buffs a character was
/// carrying at logout, each with the remaining time that was stored — the
/// countdown resumes where it stopped rather than accounting for the time spent
/// offline, which is what makes an hour-long buff still an hour long after an
/// overnight logout.
///
/// Runs after the character is spawned, since applying a buff touches the live
/// stat/scheduler/packet paths. Each row goes through
/// [`apply_continuous_effects`] with the stored duration as Java's custom
/// `abnormalTime`, self-cast (`effector == effected`, matching Java's
/// `applyEffects(this, this, …)`), which also means the debuff resist roll is
/// skipped — a debuff that was up at logout comes back rather than getting a
/// second chance to be resisted.
///
/// A row whose skill no longer exists (datapack change, skill removed) is
/// dropped silently, like Java's `skill == null` continue.
pub(crate) fn restore_persisted_buffs(
    world: &mut World,
    object_id: i32,
    rows: &[crate::db::SkillBuffRow],
) {
    for row in rows {
        let Some(skill) = helpers::skill_by_id(world, row.skill_id, row.skill_level) else {
            continue;
        };
        apply_continuous_effects(
            world,
            object_id,
            object_id,
            &skill,
            Some(row.remaining_time_secs),
        );
    }
}

/// Push the creature's current abnormal-visual set to their **own** client
/// (`ExUserInfoAbnormalVisualEffect`). The set other people see rides on the
/// `CharInfo` that `broadcast_user_info` already sends; this is the self-facing
/// half, without which a stunned player sees no swirl on themselves.
pub(crate) fn refresh_abnormal_visuals(world: &World, object_id: i32) {
    let Some(client_id) = helpers::client_for_player(world, object_id) else {
        return;
    };
    let visuals = crate::game_loop::abnormal::visual_effects(world, object_id);
    let invisible = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&object_id)
        .is_some_and(|f| f.hidden);
    let transform = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map_or(0, |p| p.transform_display_id);
    helpers::send_to_client(
        world,
        client_id,
        crate::network::user_info::ex_user_info_abnormal_visual_effect(
            object_id, invisible, transform, &visuals,
        ),
    );
}

/// `broadcastPacket(new ChangeWaitType(creature, moveType))` — the fake-death
/// pose, sent to observers **and** the player themselves (Java's `Player`
/// override makes `broadcastPacket` include self).
pub(crate) fn broadcast_change_wait_type(world: &mut World, object_id: i32, move_type: i32) {
    let Some(pos) = maybe_position(world, object_id) else {
        return;
    };
    let pkt = server_packets::change_wait_type(object_id, move_type, pos.x, pos.y, pos.z);
    crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
}

/// `Creature.stopFakeDeath` — get back up: tell every client to end the pose
/// and re-`Revive` the body (Java sends both, with a comment about a client
/// quirk that needs the second one).
///
/// Java also calls `setRecentFakeDeath(true)` here, starting the
/// `isRecentFakeDeath()` grace period during which mobs still ignore you.
/// **`PlayerFakeDeathUpProtection = 0` on this dist**, so that window is zero
/// seconds wide and the flag can never read true — not ported, matching the
/// `MP_BLOCK`/`MAX_MOMENTUM` precedent for config-disabled behaviour.
pub(crate) fn stop_fake_death(world: &mut World, object_id: i32) {
    broadcast_change_wait_type(world, object_id, server_packets::wait_type::STOP_FAKEDEATH);
    let pkt = server_packets::revive(object_id);
    crate::game_loop::helpers::broadcast_including_self(world, object_id, &pkt);
}
