use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::send_sm_to_player;

/// `Player.doDie`: mark dead, stop everything, apply the XP penalty,
/// broadcast `Die` with the to-village flag.
pub(crate) fn player_do_die(world: &mut World, player_oid: i32, killer_oid: i32) {
    // Every consequence of this death is Java's `killer.getActingPlayer()`, not
    // "the killer object": a kill landed by someone's **summon** carries the
    // same PK counter, karma, clan-war credit and exp-penalty relief as one
    // they landed themselves.
    //
    // Resolved once at the top rather than at each site. It was previously
    // shadowed part-way down, which happened to cover everything below it —
    // but any code added *above* that point would silently have used the raw
    // id, and there is no signal when that goes wrong.
    let killer_oid = crate::game_loop::pvp::acting_player(world, killer_oid);
    {
        let Some((p, mut vitals)) = world
            .objects
            .get_many_mut::<(&mut crate::model::Player, &mut Vitals)>(&player_oid)
        else {
            return;
        };
        if vitals.dead {
            return;
        }
        vitals.dead = true;
        vitals.cur_hp = 0.0;
        drop((p, vitals));
        world.objects.remove_component::<Movement>(&player_oid);
        world.objects.remove_component::<Intent>(&player_oid);
        world
            .objects
            .remove_component::<crate::model::components::QueuedAction>(&player_oid);
        if let Some(t) = world
            .objects
            .get_component_mut::<crate::model::components::TargetRef>(&player_oid)
        {
            t.0 = None;
        }
    }
    // Any cast dies with the caster (`abortCast`; also stops pre-launch
    // packets via the seq mismatch).
    crate::game_loop::skills::cast::abort_cast(world, player_oid);

    // `Playable.doDie`'s buff block: death normally strips everything, unless
    // Noblesse Blessing is up — then only the blessing goes.
    stop_effects_on_death(world, player_oid);

    // `Player.doDie`'s `stopWaterTask()`: a corpse doesn't drown. Without this
    // the breath gauge would keep ticking damage into a dead body, and the bar
    // would still be on screen at the death dialog.
    crate::game_loop::water::stop_water_task(world, player_oid);

    // `Player.doDie`'s reputation block: a player killer takes the PvP/PK
    // consequences (counters, karma) for this death.
    if world
        .objects
        .has_component::<crate::model::Player>(&killer_oid)
    {
        crate::game_loop::pvp::on_kill_update_pvp_reputation(world, killer_oid, player_oid);
        // `Player.doDie`'s pvp/pk item reward (`Custom/PvpRewardItem.ini`),
        // paid to the killer and chosen by whether the victim was flagged. Its
        // own zone/instance guards live in the config, so it runs even where
        // the reputation block above bails out.
        crate::game_loop::pvp::pay_kill_reward(world, killer_oid, player_oid);
    }

    // Java `Player.doDie`: losing a cursed weapon on death is an if/else-if
    // chain with the ordinary item drop — a cursed wielder drops *the weapon*
    // (or it vanishes on the disappear roll) and never scatters their bag.
    let was_cursed = world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.cursed_weapon_equipped_id != 0);
    if was_cursed {
        crate::game_loop::cursed_weapon::on_wielder_death(world, player_oid, killer_oid);
    } else {
        // `onDieDropItem` — a PK (or anyone a monster killed) can scatter part
        // of their inventory on the ground. Runs before the XP penalty.
        on_die_drop_item(world, player_oid, killer_oid);
    }

    // Clan-war kill bookkeeping (Java `Player.doDie` → `ClanWar.onKill`):
    // only outside PVP/siege zones, killer and victim both clanned players.
    // (`clan_war_on_kill` itself exempts academy members, as Java does; the
    // AntiFeed check stays unported.)

    // Death XP penalty — Java skips it entirely when the victim died inside a
    // PVP or siege zone (`!isLucky() && !insidePvpZone && !isOnEvent()`).
    // Arena and siege deaths are free.
    let in_free_death_zone = world
        .objects
        .get_component::<crate::model::components::ZoneFlags>(&player_oid)
        .is_some_and(|f| {
            f.contains(crate::data::zone_data::ZoneKind::Pvp)
                || f.contains(crate::data::zone_data::ZoneKind::Siege)
        });
    if !in_free_death_zone
        && world
            .objects
            .has_component::<crate::model::Player>(&killer_oid)
    {
        crate::game_loop::clans::clan_war_on_kill(world, killer_oid, player_oid);
    }
    if !in_free_death_zone {
        // Java `calculateDeathExpPenalty(killer)` quarters the loss when the
        // killer is a clan-war enemy (`atWarWith`, any war state).
        let at_war = {
            let kc = clan_of_or_zero(world, killer_oid);
            let vc = clan_of_or_zero(world, player_oid);
            crate::game_loop::clans::at_war_between(world, kc, vc)
        };
        apply_death_exp_penalty_ex(world, player_oid, at_war, Some(killer_oid));
    }

    let opts = die_options(world, player_oid);
    broadcast_including_self(world, player_oid, &server_packets::die(player_oid, opts));
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[(server_packets::status_update_type::CUR_HP, 0)],
        ),
    );

    // TvT scoring + respawn (Java's `onPlayerDeath` `ON_CREATURE_DEATH`
    // listener): a kill on the enemy team scores, and the victim is queued for
    // a timed arena respawn. `killer_oid` is already the acting player.
    crate::game_loop::events::tvt::on_player_death(world, player_oid, killer_oid);
}

/// `Playable.doDie`'s effect block.
///
/// Java: a `NOBLESS_BLESSING` (or `RESURRECTION_SPECIAL`) holder stops *only*
/// that effect and keeps the rest of its buffs through death and the following
/// resurrection; everyone else runs
/// `stopAllEffectsExceptThoseThatLastThroughDeath`, which strips every active
/// buff whose skill isn't `<stayAfterDeath>`.
///
/// Both flags spare the buff list as of G34 S4: `ResurrectionSpecial`
/// (Salvation 1410, Soul of the Phoenix 438) landed the second source, and
/// being stripped here is exactly what fires its revive proposal.
///
/// Passive entries are skipped: Java's sweep runs over `EffectList._actives`
/// only, while this port parks the grade-penalty passives in the same `Buffs`
/// vec — dropping those would silently unwind a passive's stat pump on death.
#[cfg(test)]
pub(crate) fn stop_effects_on_death_for_test(world: &mut World, player_oid: i32) {
    stop_effects_on_death(world, player_oid);
}

fn stop_effects_on_death(world: &mut World, player_oid: i32) {
    use crate::model::skill::effect_flag;

    // Java tests the two flags separately but does the same thing for each:
    // stop that one effect, keep the rest.
    let sparing = effect_flag::NOBLESS_BLESSING | effect_flag::RESURRECTION_SPECIAL;
    let blessed = crate::game_loop::abnormal::flags_of(world, player_oid) & sparing != 0;
    crate::game_loop::skills::effects::expire_buffs_where(world, player_oid, |world, buff| {
        !buff.passive
            && if blessed {
                // `stopEffects(EffectFlag.NOBLESS_BLESSING)` /
                // `stopEffects(EffectFlag.RESURRECTION_SPECIAL)` — that effect
                // and nothing else.
                buff.effect_flags & sparing != 0
            } else {
                !world
                    .data
                    .skill_data
                    .get(buff.skill_id, buff.skill_level)
                    .is_some_and(|s| s.stay_after_death)
            }
    });
}

/// Java `Player.isLucky()` — `getLevel() <= 9 && isAffectedBySkill(194)`.
///
/// The `Lucky` effect itself is empty in Java (its handler carries only a
/// `canStart` guard), so the buff's **presence** is the whole mechanic: it
/// exempts a newbie from the death exp penalty. Java's other reader is
/// `PlayerStat.updateVitalityPoints`, where being lucky skips vitality
/// consumption outright; `vitality::update_vitality_points` calls this for
/// exactly that.
pub(crate) fn is_lucky(world: &World, player_oid: i32) -> bool {
    /// `CommonSkill.LUCKY`.
    const LUCKY_SKILL_ID: i32 = 194;
    world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.level <= 9)
        && has_buff(world, player_oid, LUCKY_SKILL_ID)
}

/// `Player.calculateDeathExpPenalty` + `PlayableStat.removeExp` (with the
/// `Delevel`/`DelevelMinimum` clamping) + the SM 539 notice.
/// `calculateDeathExpPenalty`'s killer branch — which of the three
/// `REDUCE_EXP_LOST_BY_*` stats scales the loss. Java's `if/else if` order is
/// raid → monster → playable, and a `null` killer skips all three.
fn reduce_exp_lost_mul(world: &World, player_oid: i32, killer_oid: Option<i32>) -> f64 {
    use crate::model::stats::Stat;
    let Some(killer) = killer_oid else {
        return 1.0;
    };
    let template = world
        .objects
        .get_component::<crate::model::npc::Npc>(&killer)
        .and_then(|n| world.data.npc_data.get(n.npc_id));
    let stat = if template.is_some_and(|t| t.is_raid()) {
        Stat::ReduceExpLostByRaid
    } else if template.is_some_and(|t| t.is_monster()) {
        Stat::ReduceExpLostByMob
    } else if crate::game_loop::helpers::is_playable(world, killer) {
        Stat::ReduceExpLostByPvp
    } else {
        return 1.0;
    };
    world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&player_oid)
        .map(|m| crate::model::finalize(m, stat, 1.0))
        .unwrap_or(1.0)
}

pub(crate) fn apply_death_exp_penalty(world: &mut World, player_oid: i32) {
    apply_death_exp_penalty_ex(world, player_oid, false, None);
}

/// The killer-aware variant: `at_war_with_killer` quarters the loss (Java's
/// `lostExp /= 4` for a clan-war death).
pub(crate) fn apply_death_exp_penalty_ex(
    world: &mut World,
    player_oid: i32,
    at_war_with_killer: bool,
    // Java `calculateDeathExpPenalty(killer)` — the killer picks which of the
    // three `REDUCE_EXP_LOST_BY_*` stats applies. `None` is Java's
    // `killer == null`, which skips the whole branch.
    killer_oid: Option<i32>,
) {
    // `Player.doDie`: "Should not penalize player when lucky, in a PvP zone or
    // event" — `isLucky()` is `getLevel() <= 9 && isAffectedBySkill(LUCKY)`,
    // i.e. the newbie Lucky (194) buff. Ported here rather than at the call
    // site because every caller of this function is a death penalty (G34 S4).
    if is_lucky(world, player_oid) {
        return;
    }
    let (level, exp) = {
        let Some(p) = world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        (p.level, p.exp)
    };
    let max_level = world.data.experience.max_level as i32;
    let percent = world.data.xp_lost.xp_percent(level);
    let (lo, hi) = if level < max_level {
        (
            world.data.experience.exp_for_level(level),
            world.data.experience.exp_for_level(level + 1),
        )
    } else {
        (
            world.data.experience.exp_for_level(max_level - 1),
            world.data.experience.exp_for_level(max_level),
        )
    };
    // `calculateDeathExpPenalty`'s killer branch: a raid, an ordinary monster
    // or a playable each scale the lost *percentage* by their own stat
    // (`Residence Death Fortune` 610 grants the mob one at ×0.88).
    let mut percent = percent * reduce_exp_lost_mul(world, player_oid, killer_oid);
    // `if (getReputation() < 0) percentLost *= Config.RATE_KARMA_EXP_LOST;` —
    // a PK can be made to lose more (or less) than everyone else. 1 here.
    if world
        .objects
        .get_component::<crate::model::Player>(&player_oid)
        .is_some_and(|p| p.reputation < 0)
    {
        percent *= world.cfg.rates.rate_karma_exp_lost;
    }
    let mut lost = (((hi - lo) as f64) * percent / 100.0).round() as i64;
    if at_war_with_killer {
        lost /= 4;
    }

    // `removeExp`'s delevel clamp: without delevel (or at/below the floor)
    // exp can't drop below the current level's threshold.
    let can_delevel =
        world.cfg.character.player_delevel && level > world.cfg.character.delevel_minimum;
    if !can_delevel {
        lost = lost.min(exp - world.data.experience.exp_for_level(level));
    }
    lost = lost.min(exp - 1).max(0);
    if lost == 0 {
        return;
    }

    let new_exp = exp - lost;
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
    {
        p.exp = new_exp;
        // Java keeps `_expBeforeDeath` and subtracts; the difference is the
        // only thing a resurrection reads, so record that directly.
        p.lost_exp_on_death = lost;
    }
    send_sm_to_player(
        world,
        player_oid,
        sm_ids::YOUR_XP_HAS_DECREASED_BY_S1,
        &[SmParam::Long(lost)],
    );
    let new_level = level_for_exp(world, new_exp, max_level);
    if new_level != level {
        set_level(world, player_oid, new_level);
    }
}
