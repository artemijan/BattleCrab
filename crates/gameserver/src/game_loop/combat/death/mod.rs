//! Death, decay/respawn, rewards (XP/SP/level-ups, drops), and the
//! die → "to village" → teleport → revive loop (G9).
//!
//! Java counterparts: `Creature.doDie`/`Npc.doDie`/`Player.doDie`,
//! `DecayTaskManager`/`RespawnTaskManager`/`Spawn.decreaseCount`,
//! `Attackable.calculateRewards` + `NpcTemplate.calculateDrops`,
//! `PlayerStat.addExpAndSp`/`addLevel`, `Player.calculateDeathExpPenalty`,
//! `RequestRestartPoint`/`Appearing`/`Player.doRevive`.
//!
//! This file keeps the NPC side of dying — `npc_do_die`, decay, respawn,
//! relocate. The rest is split by phase and re-exported, so callers keep
//! saying `death::…`:
//!
//! - `rewards` — `calculate_rewards`: XP/SP shares, drops, spoil, and the
//!   corpse item drop.
//! - `progression` — exp/SP award and loss, level changes, and the skill
//!   grants/removals that follow a level change.
//! - `player_death` — `player_do_die` and the death XP penalty.
//! - `restart` — the "to village" choice: die options, clan-hall and siege
//!   restart points, the teleport itself and its watchdog.
//! - `resurrect` — revive requests/answers, the restore percentages, pet
//!   revive, and raid points.

mod player_death;
mod progression;
mod restart;
mod resurrect;
mod rewards;

#[cfg(test)]
pub(crate) use player_death::apply_death_exp_penalty_ex;

#[cfg(test)]
pub(crate) use player_death::stop_effects_on_death_for_test;
pub(crate) use player_death::{apply_death_exp_penalty, is_lucky, player_do_die};

#[cfg(test)]
pub(crate) use progression::check_player_skills;
pub(crate) use progression::{
    add_exp_and_sp, cap_level, consume_kill_vitality, level_for_exp, maybe_skill_remove_on_delevel,
    overhit_bonus, remove_exp_and_sp, reward_skill_grants, reward_skills, set_level,
};
pub(crate) use restart::{
    TELEPORT_WATCHDOG_PERIOD, die_options, handle_appearing, handle_request_restart_point,
    teleport_player, teleport_player_scattered, teleport_to_object, teleport_to_town,
    teleport_watchdog_tick,
};
#[cfg(test)]
pub(crate) use resurrect::do_revive_with;

pub(crate) use resurrect::{award_raid_points, do_revive, handle_revive_answer, revive_request};

#[cfg(test)]
pub(crate) use rewards::{PremiumDropRate, premium_drop_mult};

#[cfg(test)]
pub(crate) use rewards::{
    auto_loots_for_test, chest_drop_template_for_test, roll_champion_drops_for_test,
    roll_drops_for_test, roll_spoil_drops_for_test,
};
pub(crate) use rewards::{calculate_rewards, give_item, on_die_drop_item};
