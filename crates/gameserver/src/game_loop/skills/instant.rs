//! The instant (one-shot) skill effects — Java's `AbstractEffect.instant()`
//! implementations, one function per [`crate::model::skill::SkillEffect`] variant.
//!
//! These were the fat arms of [`super::effects::apply_skill_effects`]'s match.
//! They are pure extractions: each body is its original arm verbatim, with the
//! loop's `continue` (skip to the next effect) rewritten as `return`, which is
//! equivalent because the match was the last statement in the effect loop.

use crate::game_loop::helpers::client_for_player;
use crate::model::components::{BaseStats, Buffs, CombatStats, RegionCell, StatModifiers, Vitals};
use crate::model::formulas;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::world::World;

use super::effects::{
    apply_skill_damage, attribute_mod, broadcast_social_action, broadcast_vitals,
    calc_general_trait_bonus, caster_display_name, caster_level, confuse_chance_passes,
    creature_level, defence_after_shield, handle_buff_expire, max_recoverable, pvp_pve_bonus,
    roll_magic_failure, send_sm, servitor_owner_of, skill_power_mul, skill_trait_mod, target_m_def,
    target_p_def,
};
use crate::network::server_packets::{SmParam, sm_ids};

/// The per-cast state the instant effects share, computed once by
/// `apply_skill_effects` before it walks the effect list.
#[derive(Clone, Copy)]
pub(super) struct CastCtx {
    pub caster_oid: i32,
    pub target_oid: i32,
    /// Magic crit for this cast — Java rolls once per `instant()`; skills carry
    /// a single instant effect, so one roll covers them all.
    pub mcrit: bool,
    /// Soulshot charged (physical/thrown skills).
    pub ss: bool,
    /// Spiritshot / blessed spiritshot charged (magic skills).
    pub sps: bool,
    pub bss: bool,
    /// The spiritshot damage multiplier those two flags resolve to: 1, 2 or 4.
    pub magic_shots_bonus: f64,
    /// Stands in for Java `isMageClass()` in the heal static bonus.
    pub caster_is_player: bool,
}

pub(super) fn magical_attack(world: &mut World, ctx: &CastCtx, skill: &Skill, power: f64) {
    let CastCtx {
        caster_oid,
        target_oid,
        mcrit,
        magic_shots_bonus,
        ..
    } = *ctx;
    let (m_atk, caster_name) = {
        let m_atk = world
            .objects
            .get_component::<CombatStats>(&caster_oid)
            .map(|c| c.m_atk)
            .unwrap_or(0.0);
        (m_atk, caster_display_name(world, caster_oid))
    };
    let m_def = target_m_def(world, target_oid);
    let failure = roll_magic_failure(world, caster_oid, target_oid, skill, false);
    // `calcMagicDam`'s `attributeMod` term (Volcano's FIRE 20 vs
    // the target's fire resistance).
    let damage = formulas::calc_magic_dam(
        m_atk,
        m_def,
        power,
        mcrit,
        crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, true),
        magic_shots_bonus,
        failure,
    ) * attribute_mod(world, caster_oid, target_oid, skill)
        * skill_trait_mod(world, caster_oid, target_oid, skill, false)
        // `calcMagicDam`'s own tail:
        // `damage *= getValue(Stat.MAGICAL_SKILL_POWER, 1)`.
        * skill_power_mul(world, caster_oid, true)
        * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));
    apply_skill_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        mcrit,
        true,
        &caster_name,
        skill.over_hit,
        false,
        skill.id,
    );
}

pub(super) fn magical_attack_mp(
    world: &mut World,
    ctx: &CastCtx,
    skill: &Skill,
    power: f64,
    critical: bool,
    critical_limit: f64,
) {
    let CastCtx {
        caster_oid,
        target_oid,
        mcrit,
        magic_shots_bonus,
        ..
    } = *ctx;
    // `calcSuccess`: `isMpBlocked()` refuses outright.
    if crate::game_loop::abnormal::is_mp_blocked(world, target_oid) {
        return;
    }
    let m_atk = world
        .objects
        .get_component::<CombatStats>(&caster_oid)
        .map(|c| c.m_atk)
        .unwrap_or(0.0);
    let m_def = target_m_def(world, target_oid);
    // `calcMagicAffected`: `defence` is the target's mDef only for
    // an *active bad* skill — all four of these are.
    let defence = if skill.is_bad() { m_def } else { 0.0 };
    let gaussian = world.roll_gaussian();
    if !formulas::calc_magic_affected(m_atk, defence, gaussian) {
        // Java messages both sides and bails.
        if let Some(cid) = client_for_player(world, caster_oid)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::YOUR_ATTACK_HAS_FAILED,
                &[],
            ));
        }
        if let Some(cid) = client_for_player(world, target_oid)
            && let Some(cs) = world.clients.get(&cid)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::C1_RESISTED_C2_S_DRAIN,
                &[
                    SmParam::Text(caster_display_name(world, target_oid)),
                    SmParam::Text(caster_display_name(world, caster_oid)),
                ],
            ));
        }
        return;
    }

    // `calcShldUse` — a perfect block cuts the drain to 1.
    let (shield_def, shield_rate, con_bonus) =
        crate::game_loop::combat::shield_stats(world, target_oid);
    let (rate_roll, perfect_roll) = (world.roll(100), world.roll(100));
    let shield = formulas::calc_shield_use(
        shield_rate,
        con_bonus,
        false,
        false,
        rate_roll,
        perfect_roll,
    );

    // Java: `mcrit = _critical && Formulas.calcCrit(skill.getMagicCriticalRate(), …)`.
    // All four skills are `<isMagic>1</isMagic>`, and `calcCrit`'s
    // magic branch **discards the rate it was passed** and reads
    // the caster's `MAGIC_CRITICAL_RATE` stat instead — so
    // `<magicCriticalRate>` is dead input here, and the roll is
    // exactly the per-cast `mcrit` already computed above (same
    // stat, same `min(rate, isBad ? 200 : 320) > Rnd.get(1000)`).
    // Only the effect's own `critical` flag gates it.
    let drain_crit = critical && mcrit;
    let target_max_mp = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.max_mp as f64)
        .unwrap_or(0.0);
    let failure = roll_magic_failure(world, caster_oid, target_oid, skill, false);
    let damage = if shield == formulas::SHIELD_PERFECT {
        1.0
    } else {
        formulas::calc_mana_dam(
            m_atk,
            m_def
                + if shield == formulas::SHIELD_SUCCEED {
                    shield_def
                } else {
                    0.0
                },
            target_max_mp,
            power,
            magic_shots_bonus,
            failure,
            drain_crit,
            critical_limit,
        ) * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill))
    };

    // `mp = Math.min(effected.getCurrentMp(), damage)` — you cannot
    // drain more than is there, and the *reported* figure is the
    // clamped one.
    let drained = {
        let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
            return;
        };
        let drained = v.cur_mp.min(damage.max(0.0));
        if damage > 0.0 {
            v.cur_mp -= drained;
        }
        drained
    };
    if drain_crit
        && let Some(cid) = client_for_player(world, caster_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(server_packets::system_message_with(sm_ids::M_CRITICAL, &[]));
    }
    if let Some(cid) = client_for_player(world, target_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(server_packets::system_message_with(
            sm_ids::S2_S_MP_HAS_BEEN_DRAINED_BY_C1,
            &[
                SmParam::Text(caster_display_name(world, caster_oid)),
                SmParam::Int(drained as i32),
            ],
        ));
    }
    if let Some(cid) = client_for_player(world, caster_oid)
        && let Some(cs) = world.clients.get(&cid)
    {
        cs.send(server_packets::system_message_with(
            sm_ids::YOUR_OPPONENT_S_MP_WAS_REDUCED_BY_S1,
            &[SmParam::Int(drained as i32)],
        ));
    }
    broadcast_vitals(world, target_oid);
}

pub(super) fn blow(
    world: &mut World,
    ctx: &CastCtx,
    skill: &Skill,
    power: f64,
    chance_boost: f64,
    critical_chance: Option<f64>,
    backstab: bool,
) {
    let CastCtx {
        caster_oid,
        target_oid,
        ss,
        ..
    } = *ctx;
    use crate::model::components::Position as PosComp;
    // Attacker position relative to the target's facing (for the
    // land roll's positional bonus, the blow's back/side damage
    // bonus, and Backstab's flank requirement).
    let (Some(a), Some(t)) = (
        world.objects.get_component::<PosComp>(&caster_oid).copied(),
        world.objects.get_component::<PosComp>(&target_oid).copied(),
    ) else {
        return;
    };
    let position = crate::model::movement::get_position(a.x, a.y, t.x, t.y, t.heading);

    // Backstab must land from outside the target's front arc
    // (`!isInFrontOf`). A front Backstab silently fails, like Java's
    // `calcSuccess == false` — no `doAttack`, no message.
    if backstab && position == crate::model::movement::Position::Front {
        return;
    }

    let (p_atk, crit_rate, str_bonus, random_dmg, blow_rate_mod, caster_name) = {
        let cs = world.objects.get_component::<CombatStats>(&caster_oid);
        let p_atk = cs.map(|c| c.p_atk).unwrap_or(0.0);
        let crit_rate = cs.map(|c| c.crit_hit).unwrap_or(0.0);
        let random_dmg = cs.map(|c| c.random_dmg).unwrap_or(0);
        let str_bonus = world
            .objects
            .get_component::<BaseStats>(&caster_oid)
            .map(|b| {
                world
                    .data
                    .stat_bonus
                    .bonus(crate::model::stats::BaseStat::Str, b.str_)
            })
            .unwrap_or(1.0);
        // `Stat.BLOW_RATE` (`FatalBlowRate` — Focus Death, Critical
        // Blow, Mortal Strike, Assassination), default 1.0.
        let blow_rate_mod = world
            .objects
            .get_component::<StatModifiers>(&caster_oid)
            .and_then(|m| m.mul.get(&crate::model::stats::Stat::BlowRate).copied())
            .unwrap_or(1.0);
        let name = caster_display_name(world, caster_oid);
        (p_atk, crit_rate, str_bonus, random_dmg, blow_rate_mod, name)
    };

    // `calcBlowSuccess`: does the blow land? A miss is silent
    // (Java's `calcSuccess == false` skips the whole effect).
    let landed = formulas::calc_blow_success(
        crit_rate / 10.0,
        position,
        crate::game_loop::combat::crit_rate_position_mul(world, caster_oid, position),
        a.z,
        t.z,
        chance_boost,
        blow_rate_mod,
        world.cfg.character.blow_rate_chance_limit,
        world.roll(100),
    );
    if !landed {
        return;
    }

    // `calcBlowDamage` opens on the shield switch: a normal block
    // adds the shield's sDef, a perfect one `return 1` outright.
    // Blows carry no `ignoreShieldDefence` — the parameter does not
    // exist on this formula, so the roll always happens.
    let defence = defence_after_shield(world, target_oid, target_p_def(world, target_oid), false);
    let rand_roll = if random_dmg > 0 {
        world.roll(2 * random_dmg + 1) - random_dmg
    } else {
        0
    };
    let mut damage = match defence {
        None => 1.0,
        Some(defence) => {
            let mut d = formulas::calc_blow_damage(
                p_atk,
                power,
                defence,
                position,
                formulas::random_damage_multiplier(rand_roll),
                ss,
            );
            // `calcBlowDamage`'s `attributeMod` + trait terms.
            d *= attribute_mod(world, caster_oid, target_oid, skill);
            d *= skill_trait_mod(world, caster_oid, target_oid, skill, true);
            d *= pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));
            d
        }
    };
    // FatalBlow/Backstab double on a `calcCrit` roll; SoulBlow
    // (`critical_chance == None`) doesn't. Java rolls this *after*
    // the perfect-block shortcut, but on that path the 1 is
    // returned before the crit is ever consulted — so the roll is
    // kept here (it stays in the RNG stream either way) and simply
    // has nothing to double.
    if let Some(cc) = critical_chance
        && formulas::calc_physical_skill_crit(cc, str_bonus, world.roll(100))
        && defence.is_some()
    {
        damage *= 2.0;
    }
    // Java passes `critical = true` to `doAttack` for every blow, so
    // it always shows as a critical hit.
    apply_skill_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        true,
        false,
        &caster_name,
        skill.over_hit,
        false,
        skill.id,
    );
}

pub(super) fn lethal(
    world: &mut World,
    ctx: &CastCtx,
    skill: &Skill,
    full_lethal: f64,
    half_lethal: f64,
) {
    let CastCtx {
        caster_oid,
        target_oid,
        ..
    } = *ctx;
    // `skill.getMagicLevel() < effected.getLevel() - 6`: silently
    // refused against a target too far above the skill's level.
    let target_level = creature_level(world, target_oid);
    if skill.magic_level < target_level - 6 {
        return;
    }
    // `isLethalable()`: raid bosses are immune — the same check
    // `apply_mute_interrupt` already uses — as is anything a script
    // exempted (`setLethalable(false)`: the siege Headquarters).
    // Grand-boss/door immunity isn't modeled, so it's not checked.
    let is_raid = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_raid());
    if is_raid
        || world
            .objects
            .has_component::<crate::model::components::NotLethalable>(&target_oid)
    {
        return;
    }
    // `isHpBlocked()` (Celestial Shield, …): a landed `DamageBlock`
    // refuses this too, now that it's modeled.
    if crate::game_loop::abnormal::is_hp_blocked(world, target_oid) {
        return;
    }
    // `INSTANT_KILL_RESIST` is never set by anything in this
    // datapack (like `MAX_MOMENTUM`), so Java's resist roll would
    // always lose against a 0 stat — not rolled here at all.
    // None of the four outcome SystemMessages below take
    // parameters (`"Lethal Strike!"`, `"Half-Kill!"`, …).
    let caster_client = client_for_player(world, caster_oid);
    let is_player_target = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .is_some();
    // `Lethal.instant`'s `chanceMultiplier` — **both** halves:
    // `calcAttributeBonus * calcGeneralTraitBonus(…, false)`. It
    // scales the full- and half-kill chances alike, so a victim
    // resisting the skill's element or trait is correspondingly
    // harder to execute.
    let lethal_amod = attribute_mod(world, caster_oid, target_oid, skill)
        * calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false);
    if world.roll(100) < ((full_lethal) * lethal_amod) as i32 {
        if is_player_target {
            if let Some(v) = world
                .objects
                .get_component_mut::<crate::model::components::PlayerVitals>(&target_oid)
            {
                v.cur_cp = 1.0;
            }
            if let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) {
                v.cur_hp = 1.0;
            }
            if let Some(client_id) = client_for_player(world, target_oid)
                && let Some(cs) = world.clients.get(&client_id)
            {
                cs.send(server_packets::system_message_with(
                    sm_ids::LETHAL_STRIKE,
                    &[],
                ));
            }
        } else if crate::game_loop::combat::is_npc_oid(target_oid)
            && let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid)
        {
            v.cur_hp = 1.0;
        }
        broadcast_vitals(world, target_oid);
        if let Some(client_id) = caster_client
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(server_packets::system_message_with(
                sm_ids::HIT_WITH_LETHAL_STRIKE,
                &[],
            ));
        }
    } else if world.roll(100) < ((half_lethal) * lethal_amod) as i32 {
        if is_player_target {
            if let Some(v) = world
                .objects
                .get_component_mut::<crate::model::components::PlayerVitals>(&target_oid)
            {
                v.cur_cp = 1.0;
            }
            if let Some(client_id) = client_for_player(world, target_oid)
                && let Some(cs) = world.clients.get(&client_id)
            {
                cs.send(server_packets::system_message_with(sm_ids::HALF_KILL, &[]));
                cs.send(server_packets::system_message_with(
                    sm_ids::YOUR_CP_WAS_DRAINED_BECAUSE_YOU_WERE_HIT_WITH_A_HALF_KILL_SKILL,
                    &[],
                ));
            }
        } else if crate::game_loop::combat::is_npc_oid(target_oid)
            && let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid)
        {
            v.cur_hp *= 0.5;
        }
        broadcast_vitals(world, target_oid);
        if let Some(client_id) = caster_client
            && let Some(cs) = world.clients.get(&client_id)
        {
            cs.send(server_packets::system_message_with(sm_ids::HALF_KILL, &[]));
        }
    }
}

pub(super) fn hp_drain(
    world: &mut World,
    ctx: &CastCtx,
    skill: &Skill,
    power: f64,
    percentage: f64,
) {
    let CastCtx {
        caster_oid,
        target_oid,
        mcrit,
        magic_shots_bonus,
        ..
    } = *ctx;
    let (m_atk, caster_name) = {
        let m_atk = world
            .objects
            .get_component::<CombatStats>(&caster_oid)
            .map(|c| c.m_atk)
            .unwrap_or(0.0);
        (m_atk, caster_display_name(world, caster_oid))
    };
    let m_def = target_m_def(world, target_oid);
    // `is_drain` swaps the caster-side failure lines for the drain
    // wording (Java checks `skill.hasEffectType(HP_DRAIN)`).
    let failure = roll_magic_failure(world, caster_oid, target_oid, skill, true);
    let damage = formulas::calc_magic_dam(
        m_atk,
        m_def,
        power,
        mcrit,
        crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, true),
        magic_shots_bonus,
        failure,
    ) * attribute_mod(world, caster_oid, target_oid, skill)
        * skill_trait_mod(world, caster_oid, target_oid, skill, false)
        // `MAGICAL_SKILL_POWER` lives *inside* Java's `calcMagicDam`,
        // so every caller gets it — HpDrain included, even though
        // its own handler never mentions the stat.
        * skill_power_mul(world, caster_oid, true)
        * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));

    // `HpDrain.instant()`: the drained HP is what's actually removed
    // — CP absorbs first (player targets only; NPCs have no CP),
    // then it's clamped to the target's remaining HP. Java reads both
    // as truncated ints, pre-damage.
    let cur_hp = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.cur_hp.floor())
        .unwrap_or(0.0);
    let cur_cp = world
        .objects
        .get_component::<crate::model::components::PlayerVitals>(&target_oid)
        .map(|v| v.cur_cp.floor())
        .unwrap_or(0.0);
    let drain = if cur_cp > 0.0 {
        if damage < cur_cp {
            0.0
        } else {
            damage - cur_cp
        }
    } else if damage > cur_hp {
        cur_hp
    } else {
        damage
    };
    // Heal the caster by `percentage`% of the drain, overheal-clamped.
    let heal = (percentage / 100.0) * drain;
    if heal > 0.0 {
        if let Some(v) = world.objects.get_component_mut::<Vitals>(&caster_oid) {
            v.cur_hp = (v.cur_hp + heal).min(v.max_hp as f64);
        }
        if let Some(client_id) = client_for_player(world, caster_oid) {
            let cur = world
                .objects
                .get_component::<Vitals>(&caster_oid)
                .map(|v| v.cur_hp as i32)
                .unwrap_or(0);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::status_update(
                    caster_oid,
                    &[(server_packets::status_update_type::CUR_HP, cur)],
                ));
            }
            crate::game_loop::party::notify_party_vitals(world, caster_oid);
        }
    }
    apply_skill_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        mcrit,
        true,
        &caster_name,
        skill.over_hit,
        false,
        skill.id,
    );
}

pub(super) fn open_door(
    world: &mut World,
    ctx: &CastCtx,
    _skill: &Skill,
    chance: i32,
    is_item: bool,
) {
    let CastCtx {
        caster_oid,
        target_oid,
        ..
    } = *ctx;
    let Some(door_id) = world
        .objects
        .get_component::<crate::model::door::Door>(&target_oid)
        .map(|d| d.door_id)
    else {
        return;
    };
    if crate::game_loop::helpers::instance_of(world, caster_oid)
        != crate::game_loop::helpers::instance_of(world, target_oid)
    {
        return;
    }
    let openable_by_skill = world
        .data
        .door_data
        .get(door_id)
        .is_some_and(|t| t.open_method == crate::data::door_data::DoorOpenMethod::BySkill);
    // Java also refuses when `door.getFort() != null`. This port
    // has no fort system, so that half cannot be evaluated — and
    // for the *skill* path it is vacuous on this dist anyway: none
    // of the 34 `BY_SKILL` doors is a fort door (they are Cruma,
    // Devil's Isle, the Water Garden, Rune ToH and the Four
    // Sepulchers). It is **not** vacuous for an item-cast unlock,
    // which skips the `BY_SKILL` gate entirely.
    // SKIP(off-chronicle): the fort half of Java's gate. Fort sieges are an
    // explicit scope-out for this build (PORTING_STATUS.md), so "once forts
    // exist" is not a milestone that can arrive — and the argument above
    // already shows the skill path is vacuous here regardless: none of the 34
    // `BY_SKILL` doors belongs to a fort.
    if !openable_by_skill && !is_item {
        send_sm(world, caster_oid, sm_ids::THIS_DOOR_CANNOT_BE_UNLOCKED);
        return;
    }
    let already_open = world.geo.doors.is_open(door_id);
    if world.roll(100) < chance && !already_open {
        crate::game_loop::doors::open_door(world, target_oid);
    } else {
        send_sm(
            world,
            caster_oid,
            sm_ids::YOU_HAVE_FAILED_TO_UNLOCK_THE_DOOR,
        );
    }
}

pub(super) fn open_chest(world: &mut World, ctx: &CastCtx) {
    let CastCtx {
        caster_oid,
        target_oid,
        ..
    } = *ctx;
    let is_chest = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
        .is_some_and(|t| t.type_name == "Chest");
    let dead = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_some_and(|v| v.dead);
    if !is_chest
        || dead
        || crate::game_loop::helpers::instance_of(world, caster_oid)
            != crate::game_loop::helpers::instance_of(world, target_oid)
    {
        return;
    }
    let player_level = creature_level(world, caster_oid);
    let chest_level = creature_level(world, target_oid);
    let band = if player_level <= 77 { 6 } else { 5 };
    if (chest_level - player_level).abs() <= band {
        broadcast_social_action(world, caster_oid, 3);
        if let Some(n) = world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&target_oid)
        {
            n.special_drop = true;
            n.must_reward_exp_sp = false;
        }
        let max_hp = world
            .objects
            .get_component::<Vitals>(&target_oid)
            .map(|v| v.max_hp as f64)
            .unwrap_or(0.0);
        crate::game_loop::combat::npc_receive_damage(world, target_oid, caster_oid, max_hp, false);
    } else {
        // Out of band the box is a mimic: Java gives it a single
        // point of hate and points its AI at the caster.
        broadcast_social_action(world, caster_oid, 13);
        crate::game_loop::minions::add_hate(world, target_oid, caster_oid, 1.0);
    }
}

pub(super) fn unsummon(world: &mut World, ctx: &CastCtx, skill: &Skill, chance: i32) {
    let CastCtx {
        caster_oid,
        target_oid,
        ..
    } = *ctx;
    // `canStart`: the *effected* must be a summon. The port keys
    // ownership the other way (owner → `SummonRef`), so find the
    // owner by asking the target's own back-reference.
    let Some(owner) = servitor_owner_of(world, target_oid) else {
        // Not a servitor — Java's `canStart` refuses outright.
        return;
    };
    // `calcSuccess`: a negative chance always lands; otherwise the
    // magic-level gate `(effected.getLevel() - 9) <= magicLevel`
    // has to pass first.
    if chance >= 0 {
        let target_level = creature_level(world, target_oid);
        if skill.magic_level > 0 && (target_level - 9) > skill.magic_level {
            return;
        }
        let rate = chance as f64
            * attribute_mod(world, caster_oid, target_oid, skill)
            * calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false);
        if rate < 100.0 && rate <= world.roll(100) as f64 {
            return;
        }
    }
    crate::game_loop::servitor::unsummon_servitor(world, owner);
}

pub(super) fn death_link(world: &mut World, ctx: &CastCtx, skill: &Skill, power: f64) {
    let CastCtx {
        caster_oid,
        target_oid,
        mcrit,
        magic_shots_bonus,
        ..
    } = *ctx;
    let Some(v) = world.objects.get_component::<Vitals>(&caster_oid).copied() else {
        return;
    };
    if v.dead {
        return;
    }
    let scaled = power * (-((v.cur_hp * 2.0) / v.max_hp as f64) + 2.0);
    let m_atk = world
        .objects
        .get_component::<CombatStats>(&caster_oid)
        .map(|c| c.m_atk)
        .unwrap_or(0.0);
    let m_def = target_m_def(world, target_oid);
    let caster_name = caster_display_name(world, caster_oid);
    let failure = roll_magic_failure(world, caster_oid, target_oid, skill, false);
    let damage = formulas::calc_magic_dam(
        m_atk,
        m_def,
        scaled,
        mcrit,
        crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, true),
        magic_shots_bonus,
        failure,
    ) * attribute_mod(world, caster_oid, target_oid, skill)
        * skill_trait_mod(world, caster_oid, target_oid, skill, false)
        * skill_power_mul(world, caster_oid, true)
        * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill));
    apply_skill_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        mcrit,
        true,
        &caster_name,
        skill.over_hit,
        false,
        skill.id,
    );
}

pub(super) fn cp_heal_percent(world: &mut World, ctx: &CastCtx, power: f64) {
    let CastCtx { target_oid, .. } = *ctx;
    use crate::model::components::PlayerVitals;
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_none_or(|v| v.dead)
        || world
            .objects
            .has_component::<crate::model::door::Door>(&target_oid)
        || crate::game_loop::abnormal::is_hp_blocked(world, target_oid)
    {
        return;
    }
    let Some(cp) = world
        .objects
        .get_component::<PlayerVitals>(&target_oid)
        .copied()
    else {
        // NPCs have no CP pool at all.
        return;
    };
    let max_cp = cp.max_cp as f64;
    let amount = if power == 100.0 {
        max_cp
    } else {
        max_cp * power / 100.0
    };
    let ceiling = max_recoverable(
        world,
        target_oid,
        crate::model::stats::Stat::MaxRecoverableCp,
        max_cp,
    );
    let amount = amount.min((ceiling - cp.cur_cp).max(0.0));
    if amount > 0.0 {
        if let Some(v) = world.objects.get_component_mut::<PlayerVitals>(&target_oid) {
            v.cur_cp += amount;
        }
        broadcast_vitals(world, target_oid);
    }
}

pub(super) fn heal(world: &mut World, ctx: &CastCtx, skill: &Skill, power: f64) {
    let CastCtx {
        caster_oid,
        target_oid,
        mcrit,
        sps,
        bss,
        caster_is_player,
        ..
    } = *ctx;
    let m_atk = world
        .objects
        .get_component::<CombatStats>(&caster_oid)
        .map(|c| c.m_atk)
        .unwrap_or(0.0);
    let mut amount = formulas::calc_heal(
        power,
        m_atk,
        mcrit,
        sps,
        bss,
        skill.mp_consume,
        caster_is_player,
    );
    // Java `Heal`: `amount *= effected.HEAL_EFFECT; amount +=
    // effected.HEAL_EFFECT_ADD` — the *recipient's* stats decide
    // how much of the heal they actually get.
    if let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&target_oid)
    {
        amount *= mods
            .mul
            .get(&crate::model::stats::Stat::HealEffect)
            .copied()
            .unwrap_or(1.0);
        amount += mods
            .add
            .get(&crate::model::stats::Stat::HealEffectAdd)
            .copied()
            .unwrap_or(0.0);
    }
    if crate::game_loop::combat::is_npc_oid(target_oid) {
        // Healing an NPC: clamp and update, no system messages
        // (nobody to send them to).
        let hp = {
            let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
                return;
            };
            if vitals.dead {
                return;
            }
            vitals.cur_hp = (vitals.cur_hp + amount).min(vitals.max_hp as f64);
            (vitals.cur_hp as i32, vitals.max_hp)
        };
        // `broadcastStatusUpdate` — refresh the HP bar for everyone
        // watching the mob; without this the server-side heal is
        // invisible to clients (the bar never moves).
        if let Some(region) = world
            .objects
            .get_component::<RegionCell>(&target_oid)
            .map(|r| r.0)
        {
            crate::game_loop::helpers::broadcast_near_region(
                world,
                region,
                &server_packets::status_update(
                    target_oid,
                    &[
                        (server_packets::status_update_type::MAX_HP, hp.1),
                        (server_packets::status_update_type::CUR_HP, hp.0),
                    ],
                ),
            );
        }
        return;
    }
    // `Heal.java`: `min(amount, max(0, getMaxRecoverableHp() -
    // getCurrentHp()))` — the ceiling is the *recoverable* cap, not
    // the pool, which is what Noblesse Harmony/Symphony lower.
    let ceiling = {
        let base = world
            .objects
            .get_component::<Vitals>(&target_oid)
            .map(|v| v.max_hp as f64)
            .unwrap_or(0.0);
        max_recoverable(
            world,
            target_oid,
            crate::model::stats::Stat::MaxRecoverableHp,
            base,
        )
    };
    let healed = {
        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
            return;
        };
        let amount = amount.min((ceiling - vitals.cur_hp).max(0.0));
        vitals.cur_hp += amount;
        amount
    };
    let caster_name = caster_display_name(world, caster_oid);
    if let Some(client_id) = client_for_player(world, target_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            if target_oid != caster_oid {
                cs.send(server_packets::system_message_with(
                    sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1,
                    &[
                        SmParam::PlayerName(caster_name),
                        SmParam::Int(healed as i32),
                    ],
                ));
            } else {
                cs.send(server_packets::system_message_with(
                    sm_ids::S1_HP_HAS_BEEN_RESTORED,
                    &[SmParam::Int(healed as i32)],
                ));
            }
            let cur_hp = world
                .objects
                .get_component::<Vitals>(&target_oid)
                .map(|v| v.cur_hp as i32)
                .unwrap_or(0);
            cs.send(server_packets::status_update(
                target_oid,
                &[(server_packets::status_update_type::CUR_HP, cur_hp)],
            ));
        }
        crate::game_loop::party::notify_party_vitals(world, target_oid);
    }
}

pub(super) fn heal_percent(world: &mut World, ctx: &CastCtx, skill: &Skill, power: f64) {
    let CastCtx {
        caster_oid,
        target_oid,
        ..
    } = *ctx;
    let Some(max_hp) = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.max_hp as f64)
    else {
        return;
    };
    // Java `full = power == 100.0`, else `maxHp * power / 100`. No
    // `HealEffect`/`HealEffectAdd` recipient scaling (unlike `Heal`).
    let amount = if power == 100.0 {
        max_hp
    } else {
        max_hp * power / 100.0
    };
    if amount < 0.0 {
        // A negative-power instance (none learnable today) is
        // damage, not healing — Java's `reduceCurrentHp` +
        // `sendDamageMessage`, reusing the shared damage path.
        let caster_name = caster_display_name(world, caster_oid);
        apply_skill_damage(
            world,
            caster_oid,
            target_oid,
            -amount,
            false,
            skill.magic_type == 1,
            &caster_name,
            false,
            false,
            skill.id,
        );
        return;
    }
    // `isHpBlocked()`: a landed `DamageBlock` refuses a positive
    // heal too (the damage branch above already gets this for
    // free through `apply_skill_damage`).
    if crate::game_loop::abnormal::is_hp_blocked(world, target_oid) {
        return;
    }
    if crate::game_loop::combat::is_npc_oid(target_oid) {
        let hp = {
            let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
                return;
            };
            if vitals.dead {
                return;
            }
            vitals.cur_hp = (vitals.cur_hp + amount).min(vitals.max_hp as f64);
            (vitals.cur_hp as i32, vitals.max_hp)
        };
        if let Some(region) = world
            .objects
            .get_component::<RegionCell>(&target_oid)
            .map(|r| r.0)
        {
            crate::game_loop::helpers::broadcast_near_region(
                world,
                region,
                &server_packets::status_update(
                    target_oid,
                    &[
                        (server_packets::status_update_type::MAX_HP, hp.1),
                        (server_packets::status_update_type::CUR_HP, hp.0),
                    ],
                ),
            );
        }
        return;
    }
    let healed = {
        let Some(vitals) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
            return;
        };
        let amount = amount.min((vitals.max_hp as f64 - vitals.cur_hp).max(0.0));
        vitals.cur_hp += amount;
        amount
    };
    let caster_name = caster_display_name(world, caster_oid);
    if let Some(client_id) = client_for_player(world, target_oid) {
        if let Some(cs) = world.clients.get(&client_id) {
            if target_oid != caster_oid {
                cs.send(server_packets::system_message_with(
                    sm_ids::S2_HP_HAS_BEEN_RESTORED_BY_C1,
                    &[
                        SmParam::PlayerName(caster_name),
                        SmParam::Int(healed as i32),
                    ],
                ));
            } else {
                cs.send(server_packets::system_message_with(
                    sm_ids::S1_HP_HAS_BEEN_RESTORED,
                    &[SmParam::Int(healed as i32)],
                ));
            }
            let cur_hp = world
                .objects
                .get_component::<Vitals>(&target_oid)
                .map(|v| v.cur_hp as i32)
                .unwrap_or(0);
            cs.send(server_packets::status_update(
                target_oid,
                &[(server_packets::status_update_type::CUR_HP, cur_hp)],
            ));
        }
        crate::game_loop::party::notify_party_vitals(world, target_oid);
    }
}

pub(super) fn energy_attack(
    world: &mut World,
    ctx: &CastCtx,
    skill: &Skill,
    power: f64,
    critical_chance: f64,
    p_def_mod: f64,
    charge_consume: i32,
    ignore_shield_defence: bool,
) {
    let CastCtx {
        caster_oid,
        target_oid,
        ss,
        ..
    } = *ctx;
    // `charge = min(chargeConsume, player.charges)` — pre-clamped,
    // so Java's `decreaseCharges` (which only fails when asked to
    // remove more than the player has) never actually refuses here.
    let charge = {
        let cur = world
            .objects
            .get_component::<crate::model::Player>(&caster_oid)
            .map(|p| p.charges)
            .unwrap_or(0);
        (charge_consume).min(cur)
    };
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&caster_oid)
    {
        p.charges -= charge;
    }
    if let Some(client_id) = client_for_player(world, caster_oid) {
        crate::game_loop::helpers::send_etc_status_update(world, client_id, caster_oid);
    }
    let (p_atk, level, str_bonus, caster_name) = {
        let p_atk = world
            .objects
            .get_component::<CombatStats>(&caster_oid)
            .map(|c| c.p_atk)
            .unwrap_or(0.0);
        let str_bonus = world
            .objects
            .get_component::<BaseStats>(&caster_oid)
            .map(|b| {
                world
                    .data
                    .stat_bonus
                    .bonus(crate::model::stats::BaseStat::Str, b.str_)
            })
            .unwrap_or(1.0);
        (
            p_atk,
            caster_level(world, caster_oid),
            str_bonus,
            caster_display_name(world, caster_oid),
        )
    };
    let base_defence = target_p_def(world, target_oid) * p_def_mod;
    let defence = defence_after_shield(world, target_oid, base_defence, ignore_shield_defence);
    let crit = formulas::calc_physical_skill_crit(critical_chance, str_bonus, world.roll(100));
    // `energyChargesBoost = 1 + (charge * 0.1)` — 10% bonus damage
    // per charge spent, the whole point of building Force first.
    let energy_charges_boost = 1.0 + charge as f64 * 0.1;
    let damage = match defence {
        None => 1.0,
        Some(defence) => {
            formulas::calc_physical_skill_damage(
                p_atk,
                1.0, // no separate pAtkMod term in Java's EnergyAttack formula
                defence,
                1.0, // already folded into `defence` above
                power,
                formulas::level_mod(level),
                1.0, // no random-damage term in Java's EnergyAttack formula
                crit,
                crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, false),
                ss,
                // Java's EnergyAttack has no ranged branch at all —
                // its `weaponMod` is a flat 77.
                false,
            ) * energy_charges_boost
                // `EnergyAttack.instant`'s `attributeMod` + trait terms.
                * attribute_mod(world, caster_oid, target_oid, skill)
                * skill_trait_mod(world, caster_oid, target_oid, skill, true)
                * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill))
        }
    };
    apply_skill_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        crit,
        false,
        &caster_name,
        skill.over_hit,
        false,
        skill.id,
    );
}

pub(super) fn hp(world: &mut World, ctx: &CastCtx, amount: f64, percent: bool) {
    let CastCtx { target_oid, .. } = *ctx;
    let Some(v) = world.objects.get_component::<Vitals>(&target_oid).copied() else {
        return;
    };
    let is_raid = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
        .is_some_and(|t| t.is_raid());
    if v.dead
        || is_raid
        || world
            .objects
            .has_component::<crate::model::door::Door>(&target_oid)
        || crate::game_loop::abnormal::is_hp_blocked(world, target_oid)
    {
        return;
    }
    let basic = if percent {
        v.max_hp as f64 * amount / 100.0
    } else {
        amount
    };
    let ceiling = max_recoverable(
        world,
        target_oid,
        crate::model::stats::Stat::MaxRecoverableHp,
        v.max_hp as f64,
    );
    let gain = basic.min((ceiling - v.cur_hp).max(0.0));
    if gain > 0.0 {
        if let Some(vit) = world.objects.get_component_mut::<Vitals>(&target_oid) {
            vit.cur_hp = (vit.cur_hp + gain).min(vit.max_hp as f64);
        }
        broadcast_vitals(world, target_oid);
    }
}

pub(super) fn dispel_by_slot(
    world: &mut World,
    ctx: &CastCtx,
    _skill: &Skill,
    dispel: &[(String, i32)],
) {
    let CastCtx { target_oid, .. } = *ctx;
    // Java `DispelBySlot.instant`: stop each active effect whose
    // originating skill's `<abnormalType>` is in the dispel set and
    // whose `abnormalLevel` is at or below the listed level (a
    // negative level dispels every level). We look each active buff's
    // source skill back up in `skill_data` for its type/level, then
    // route removals through `handle_buff_expire` — which drops the
    // buff, reverts its stats, and rebroadcasts the abnormal icons
    // for both player and NPC targets; the DoT tick chain (e.g.
    // Poison) self-terminates once its buff is gone. Buff snapshot is
    // collected first to avoid overlapping borrows of `world`.
    let candidates: Vec<(i32, i32)> = world
        .objects
        .get_component::<Buffs>(&target_oid)
        .map(|buffs| {
            buffs
                .0
                .iter()
                .map(|b| (b.skill_id, b.skill_level))
                .collect()
        })
        .unwrap_or_default();
    let to_dispel: Vec<i32> = candidates
        .into_iter()
        .filter(|&(sid, slvl)| {
            world.data.skill_data.get(sid, slvl).is_some_and(|bs| {
                dispel.iter().any(|(ty, lvl)| {
                    bs.abnormal_type == *ty && (*lvl < 0 || *lvl >= bs.abnormal_level)
                })
            })
        })
        .map(|(sid, _)| sid)
        .collect();
    for skill_id in to_dispel {
        handle_buff_expire(world, target_oid, skill_id);
    }
    // Java `DispelBySlot.instant` also dispels a *non-buff*
    // transformation ("Dispel transformations (buff and by GM)"):
    // a TRANSFORM entry matching the current transform id (or the
    // catch-all negative level, e.g. Dismount 839's `TRANSFORM,-1`)
    // calls `stopTransformation(true)`. That's the only revert path
    // for `//transform`/`//ride_bike`, which set the transform
    // directly with no backing buff. Buff-backed transforms are
    // already reverted by the `handle_buff_expire` sweep above, so
    // only act if still transformed. (Java guards the whole method
    // with `hasAbnormalType(...)`, which would make this branch
    // unreachable for GM transforms — an upstream quirk we don't
    // reproduce; the dist skill data's intent is that Dismount
    // always ends the ride.)
    let transform_id = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
        .map_or(0, |p| p.transform_id);
    if transform_id != 0
        && dispel
            .iter()
            .any(|(ty, lvl)| ty == "TRANSFORM" && (*lvl < 0 || *lvl == transform_id))
    {
        crate::game_loop::admin::transforms::remove_transform(world, target_oid);
    }
}

pub(super) fn dispel_by_slot_probability(
    world: &mut World,
    ctx: &CastCtx,
    _skill: &Skill,
    dispel: &[String],
    rate: i32,
) {
    let CastCtx { target_oid, .. } = *ctx;
    // Java `DispelBySlotProbability.instant`: the same cleanse as
    // `DispelBySlot`, except the `rate`% roll is evaluated **per
    // buff** inside the predicate — so a 40% Mass Warrior Bane
    // strips roughly two of five matching buffs rather than all or
    // nothing. The spec carries no per-type level, so every level
    // of a listed abnormal type is a candidate.
    //
    // Java also skips `isIrreplacableBuff()` effects. Not modelled
    // and not a gap: the tag appears only in the 22800+/23200+/
    // 27800+ skill files, all off-chronicle for Interlude.
    //
    // Note this path deliberately does *not* consult the target's
    // `ResistDispelBuff`: Java reads that stat only in
    // `Formulas.calcCancelSuccess` (the `Cancel` skill family,
    // unported), never in the Bane handler.
    let candidates: Vec<(i32, i32)> = world
        .objects
        .get_component::<Buffs>(&target_oid)
        .map(|buffs| {
            buffs
                .0
                .iter()
                .map(|b| (b.skill_id, b.skill_level))
                .collect()
        })
        .unwrap_or_default();
    let mut to_dispel: Vec<i32> = Vec::new();
    for (sid, slvl) in candidates {
        let matches = world
            .data
            .skill_data
            .get(sid, slvl)
            .is_some_and(|bs| dispel.contains(&bs.abnormal_type));
        // Roll per candidate, and only for candidates that match —
        // keeping the roll count (and so the RNG stream) tied to the
        // buffs actually at risk, as in Java's predicate.
        if matches && world.roll(100) < rate {
            to_dispel.push(sid);
        }
    }
    for skill_id in to_dispel {
        handle_buff_expire(world, target_oid, skill_id);
    }
}

pub(super) fn target_cancel(world: &mut World, ctx: &CastCtx, skill: &Skill, chance: i32) {
    let CastCtx {
        caster_oid,
        target_oid,
        ..
    } = *ctx;
    // `calcSuccess`: an invincible target is never shaken off its
    // mark. Java names the three abnormal types directly rather
    // than going through an effect flag, so this reads the live
    // buffs' `abnormalType` the same way.
    const INVINCIBLE: [&str; 3] = [
        "ABNORMAL_INVINCIBILITY",
        "INVINCIBILITY_SPECIAL",
        "INVINCIBILITY",
    ];
    if world
        .objects
        .get_component::<Buffs>(&target_oid)
        .is_some_and(|b| {
            b.0.iter()
                .any(|x| INVINCIBLE.contains(&x.abnormal_type.as_str()))
        })
    {
        return;
    }
    // Java gates this on `Formulas.calcProbability`, not on the raw
    // percentage — so the victim's **level** counts, and Shield
    // Bash slides off a target well above the skill's magic level.
    if !confuse_chance_passes(world, caster_oid, target_oid, skill, chance) {
        return;
    }
    // `setTarget(null)` — the Player override broadcasts
    // `TargetUnselected` with includeSelf, which is what clears the
    // client's selection ring.
    if let Some(client_id) = client_for_player(world, target_oid) {
        crate::game_loop::target::set_target(world, client_id, target_oid, None);
    } else if let Some(t) = world
        .objects
        .get_component_mut::<crate::model::components::TargetRef>(&target_oid)
    {
        t.0 = None; // NPC: no client to notify
    }
    // `abortAttack()` / `abortCast()`.
    world
        .objects
        .remove_component::<crate::model::components::Intent>(&target_oid);
    if world
        .objects
        .has_component::<crate::model::components::Casting>(&target_oid)
    {
        crate::game_loop::skills::cast::stop_casting(world, target_oid);
    }
}
