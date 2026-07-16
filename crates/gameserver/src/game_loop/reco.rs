//! The recommendation ("vote") system: recommending another player
//! (`RequestVoteNew`), the over-time grant of recommendations-to-give
//! (`RecoGiveTask`), and the daily reset (`DailyTaskManager.resetRecommends`).
//!
//! Recommendation counts live on the `Player` component (`rec_have`/`rec_left`)
//! and are persisted in `character_reco_bonus` by the memory-first autosave; see
//! `db::load_reco_bonus` / `store_player_tx` / `create_character`.

use commons::network::PacketReader;

use crate::model::components::TargetRef;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::client_for_player;

/// Java `RecoGiveTask` initial delay: 2 h (`scheduleAtFixedRate(…, 7_200_000,
/// …)`), in 100 ms ticks.
pub(crate) const RECO_GIVE_INITIAL_DELAY: u64 = 72_000;
/// Java `RecoGiveTask` period: 1 h (`…, 3_600_000`), in 100 ms ticks.
const RECO_GIVE_PERIOD: u64 = 36_000;
/// 24 h between daily resets, in 100 ms ticks.
const DAILY_RESET_PERIOD: u64 = 864_000;
/// 06:30, as milliseconds past midnight — Java `DailyTaskManager` schedules the
/// reset here (in server-local time; the port uses UTC — see `schedule_initial_daily_reset`).
const DAILY_RESET_MS_OF_DAY: i64 = (6 * 3600 + 30 * 60) * 1000;

/// Java `Player.setRecomHave`/`setRecomLeft`: clamp to `0..=255`.
fn clamp_reco(value: i32) -> i32 {
    value.clamp(0, 255)
}

fn send_to_player(world: &World, object_id: i32, packet: Vec<u8>) {
    if let Some(cid) = client_for_player(world, object_id) {
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
}

fn send_sm(world: &World, object_id: i32, message_id: i16, params: &[SmParam]) {
    send_to_player(world, object_id, server_packets::system_message_with(message_id, params));
}

/// Java `Player.updateUserInfo()` — a fresh `UserInfo` to the player themselves
/// (no `CharInfo` broadcast; that's `broadcastUserInfo`).
fn update_user_info(world: &World, object_id: i32) {
    let Some(v) = crate::model::PlayerView::of(&world.objects, object_id) else { return };
    let relation = super::party::calculate_relation(world, v.p);
    send_to_player(
        world,
        object_id,
        crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, relation),
    );
}

fn send_ex_vote(world: &World, object_id: i32) {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else { return };
    let (rec_left, rec_have) = (p.rec_left, p.rec_have);
    send_to_player(world, object_id, server_packets::ex_vote_system_info(rec_left, rec_have));
}

/// Port of `clientpackets/RequestVoteNew` — recommend the targeted player.
pub(crate) fn handle_request_vote_new(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let Some(target_id) = PacketReader::new(body).read_i32() else { return };

    // `player.getTarget()`.
    let target = world.objects.get_component::<TargetRef>(&player).and_then(|t| t.0);
    let Some(target) = target else {
        // Java: `object == null` → SELECT_TARGET.
        send_sm(world, player, sm_ids::SELECT_TARGET, &[]);
        return;
    };
    // Java: `object instanceof Player`. A non-player target (NPC) falls to the
    // else branch → THAT_IS_AN_INCORRECT_TARGET.
    // TODO(reco): Java also has a fake-player branch here (recommend a talkable
    // fake player, decrementing rec_left with no rec_have target) — fake players
    // aren't ported.
    if world.objects.get_component::<Player>(&target).is_none() {
        send_sm(world, player, sm_ids::THAT_IS_AN_INCORRECT_TARGET, &[]);
        return;
    }

    // `target.getObjectId() != _targetId` → silent drop (stale packet).
    if target != target_id {
        return;
    }
    if target == player {
        send_sm(world, player, sm_ids::YOU_CANNOT_RECOMMEND_YOURSELF, &[]);
        return;
    }
    let rec_left = world.objects.get_component::<Player>(&player).map_or(0, |p| p.rec_left);
    if rec_left <= 0 {
        send_sm(world, player, sm_ids::YOU_ARE_OUT_OF_RECOMMENDATIONS_TRY_AGAIN_LATER, &[]);
        return;
    }
    let target_rec_have = world.objects.get_component::<Player>(&target).map_or(0, |p| p.rec_have);
    if target_rec_have >= 255 {
        send_sm(world, player, sm_ids::YOUR_SELECTED_TARGET_CAN_NO_LONGER_RECEIVE_A_RECOMMENDATION, &[]);
        return;
    }

    // `player.giveRecom(target)`: target.incRecomHave() + player.decRecomLeft().
    if let Some(t) = world.objects.get_component_mut::<Player>(&target) {
        if t.rec_have < 255 {
            t.rec_have += 1;
        }
    }
    let player_rec_left = {
        let p = world.objects.get_component_mut::<Player>(&player).expect("player online");
        if p.rec_left > 0 {
            p.rec_left -= 1;
        }
        p.rec_left
    };

    // "You have recommended $c1. You have $s2 recommendations left."
    let target_name = world.objects.get_component::<Player>(&target).map(|p| p.name.clone()).unwrap_or_default();
    send_sm(
        world,
        player,
        sm_ids::YOU_HAVE_RECOMMENDED_C1_YOU_HAVE_S2_RECOMMENDATIONS_LEFT,
        &[SmParam::PlayerName(target_name), SmParam::Int(player_rec_left)],
    );
    // "You have been recommended by $c1." to the target.
    let player_name = world.objects.get_component::<Player>(&player).map(|p| p.name.clone()).unwrap_or_default();
    send_sm(world, target, sm_ids::YOU_HAVE_BEEN_RECOMMENDED_BY_C1, &[SmParam::PlayerName(player_name)]);

    update_user_info(world, player);
    super::party::broadcast_user_info(world, target);
    send_ex_vote(world, player);
    send_ex_vote(world, target);
}

/// Port of `handlers/effecthandlers/GiveRecommendation.instant` — grant the
/// effected player up to `amount` recommendations received (`rec_have`, capped
/// at 255). When the target is already maxed, the effector is told "Nothing
/// happened."
pub(crate) fn apply_give_recommendation(world: &mut World, effector_oid: i32, effected_oid: i32, amount: i32) {
    // Java: only players receive recommendations.
    let Some(rec_have) = world.objects.get_component::<Player>(&effected_oid).map(|p| p.rec_have) else {
        return;
    };
    let recommendations_given = if rec_have + amount >= 255 { 255 - rec_have } else { amount };
    if recommendations_given > 0 {
        if let Some(p) = world.objects.get_component_mut::<Player>(&effected_oid) {
            p.rec_have = clamp_reco(p.rec_have + recommendations_given);
        }
        send_sm(world, effected_oid, sm_ids::YOU_OBTAINED_S1_RECOMMENDATION_S, &[SmParam::Int(recommendations_given)]);
        update_user_info(world, effected_oid);
        send_ex_vote(world, effected_oid);
    } else if world.objects.get_component::<Player>(&effector_oid).is_some() {
        send_sm(world, effector_oid, sm_ids::NOTHING_HAPPENED, &[]);
    }
}

/// `ScheduledTask::RecoGive` — Java `RecoGiveTask.run`. Hands out 10
/// recommendations-to-give on the first firing (2 h online), then 1 each
/// subsequent hour, and reschedules itself while the player stays online.
pub(crate) fn handle_reco_give(world: &mut World, player_object_id: i32, seq: u64) {
    // Stale (logged out, or relogged with a fresh seq) → no-op, cancelling the
    // per-session fixed-rate task.
    let Some(p) = world.objects.get_component::<Player>(&player_object_id) else { return };
    if p.reco_give_seq != seq {
        return;
    }

    let reco_to_give = {
        let p = world.objects.get_component_mut::<Player>(&player_object_id).expect("checked");
        // 10 to give out after 2 h logged in, then 1 more every hour.
        let amount = if !p.reco_two_hours_given {
            p.reco_two_hours_given = true;
            10
        } else {
            1
        };
        p.rec_left = clamp_reco(p.rec_left + amount);
        amount
    };

    send_sm(world, player_object_id, sm_ids::YOU_OBTAINED_S1_RECOMMENDATION_S, &[SmParam::Int(reco_to_give)]);
    update_user_info(world, player_object_id);

    world
        .scheduler
        .schedule(world.tick + RECO_GIVE_PERIOD, ScheduledTask::RecoGive { player_object_id, seq });
}

/// Start the per-player `RecoGiveTask` at enter-world (Java `restore` →
/// `startRecoGiveTask`). A fresh `reco_give_seq` invalidates any task left over
/// from a previous session on this object id.
pub(crate) fn start_reco_give_task(world: &mut World, player_object_id: i32) {
    let seq = world.next_reco_give_seq();
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_object_id) {
        p.reco_give_seq = seq;
    }
    world.scheduler.schedule(
        world.tick + RECO_GIVE_INITIAL_DELAY,
        ScheduledTask::RecoGive { player_object_id, seq },
    );
}

/// `ScheduledTask::DailyRecoReset` — Java `DailyTaskManager.resetRecommends`.
/// Zeroes rec_left and decays rec_have for online players (in memory, plus a
/// packet refresh) and offline characters (via a DB command), then reschedules
/// itself 24 h out.
pub(crate) fn handle_daily_reco_reset(world: &mut World) {
    // Offline population (Java's two UPDATE statements).
    let _ = world.db.send(crate::db::DbCommand::ResetRecommends);

    // Online players: `setRecomLeft(0)`, `setRecomHave(rec_have - 20)`, then
    // ExVoteSystemInfo + broadcastUserInfo.
    let online: Vec<i32> = world
        .clients
        .values()
        .filter_map(|cs| match cs {
            ClientSession::InGame(s) => Some(s.player_object_id()),
            _ => None,
        })
        .collect();
    for oid in online {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.rec_left = 0;
            p.rec_have = clamp_reco(p.rec_have - 20);
        }
        send_ex_vote(world, oid);
        super::party::broadcast_user_info(world, oid);
    }

    world.scheduler.schedule(world.tick + DAILY_RESET_PERIOD, ScheduledTask::DailyRecoReset);
}

/// Schedule the first `DailyRecoReset` for the next 06:30 (Java
/// `DailyTaskManager`'s constructor). Called once at game-loop start.
///
/// TODO(reco): fidelity gaps vs Java `DailyTaskManager` (not yet ported):
///   * 06:30 is computed in UTC, not server-local time.
///   * no `GlobalVariablesManager.DAILY_TASK_RESET` catch-up — a reset missed
///     while the server was down runs at the next 06:30 rather than on boot.
///   * `resetRecommends` is the only reset ported here; the manager also drives
///     vitality/clan-bonus/daily-skills/etc. resets.
pub(crate) fn schedule_initial_daily_reset(world: &mut World) {
    let now = commons::util::now_millis();
    let ms_of_day = now.rem_euclid(86_400_000);
    let mut delay_ms = DAILY_RESET_MS_OF_DAY - ms_of_day;
    if delay_ms < 0 {
        delay_ms += 86_400_000;
    }
    let delay_ticks = (delay_ms / 100) as u64;
    world.scheduler.schedule(world.tick + delay_ticks, ScheduledTask::DailyRecoReset);
}
