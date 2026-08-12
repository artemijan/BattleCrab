//! Port of `gameserver/util/FloodProtectorAction.java` + `FloodProtectors.java`
//! — the per-client packet rate limiter.
//!
//! # Where the check lives
//!
//! Java calls `getClient().getFloodProtectors().canX()` from inside **41
//! separate client-packet handlers**. This port has one dispatch site where
//! Java has 41, so the check is table-driven instead: [`action_for_opcode`] /
//! [`action_for_ex_opcode`] map an opcode onto its protector slot and
//! [`super::dispatch::on_packet`] consults it once, before the handler runs.
//! Same slots, same shared counters (all 24 of Java's `canPerformTransaction`
//! call sites share one `Transaction` protector, and so do their opcodes here),
//! but a missed call site is a missing table row rather than a silently
//! unguarded packet.
//!
//! **The one behavioural consequence** of hoisting the check: Java runs each
//! handler's own early-return validation *first* (`player == null`, an HTML
//! action that no longer validates, …) and only then consults the protector, so
//! a packet rejected by that validation never charges the interval. Here it
//! does. The direction is strictly stricter, and only for packets that were
//! going to be dropped anyway; no accepted action becomes rejected.
//!
//! # What is deliberately not modelled
//!
//! Java's `_punishmentInProgress` flag guards against two threads punishing the
//! same client concurrently. The game loop is single-threaded and the
//! punishment is applied synchronously inside the same call, so the flag can
//! never be observed set by another caller — it is left out rather than carried
//! as a field that is always `false` at every read.

use crate::config::flood_protector::{
    FloodAction, FloodProtectorConfig, FloodProtectorsConfig, FloodPunishment,
};
use crate::game_loop::helpers::disconnect_player;
use crate::model::Player;
use crate::model::punishment::{PunishmentAffect, PunishmentType};
use crate::network::client_packets::{ex_opcodes as exop, opcodes as cop};
use crate::world::World;

/// Java `GameTimeTaskManager.MILLIS_IN_TICK` — and the game loop's own `TICK`.
///
/// Java's `getGameTicks()` is `World::tick` here: the loop's monotonic 100 ms
/// counter, which this port already keeps in place of a dedicated game-time
/// thread. The gate reads *that* rather than the wall clock — as Java does — so
/// a loop that has overrun its budget does not silently widen every window.
const MILLIS_IN_TICK: i64 = 100;

/// What the caller must do with a request (Java's `boolean` plus the punishment
/// branch, which needs `&mut World` and so cannot happen inside the counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloodOutcome {
    /// Java `canPerformAction() == true`.
    Allow,
    /// Flooded, under the punishment limit: drop the packet silently.
    Reject,
    /// Flooded and at/over `PunishmentLimit`: drop the packet *and* apply this.
    RejectAndPunish(FloodPunishment),
}

impl FloodOutcome {
    pub fn is_allowed(self) -> bool {
        matches!(self, FloodOutcome::Allow)
    }
}

/// One protector slot's mutable state (Java `FloodProtectorAction`'s three
/// fields). Held per client, per slot.
#[derive(Debug, Clone, Copy, Default)]
struct ProtectorState {
    /// Java `_nextGameTick`. `0` = never used, which always passes since the
    /// tick counter is positive (Java seeds it with the construction-time tick,
    /// which likewise always passes on the first call).
    next_tick: i64,
    /// Java `_count` — requests refused inside the current interval.
    count: i32,
    /// Java `_logged` — log only the first refusal of an interval.
    logged: bool,
}

/// Java `FloodProtectors`: the whole set of protectors for one client. Lives on
/// the `Session`, which — like Java's `GameClient` — outlives the connection
/// state transitions, so a flood cannot be reset by bouncing to the lobby and
/// back.
#[derive(Debug, Clone)]
pub struct FloodProtectors {
    slots: [ProtectorState; FloodAction::COUNT],
}

impl Default for FloodProtectors {
    fn default() -> Self {
        Self::new()
    }
}

impl FloodProtectors {
    pub fn new() -> Self {
        Self {
            slots: [ProtectorState::default(); FloodAction::COUNT],
        }
    }

    /// Java `FloodProtectorAction.canPerformAction`, minus the GM
    /// cond-override test (the caller does that — it needs the world) and minus
    /// applying the punishment (returned instead).
    ///
    /// `client` only labels log lines.
    pub fn can_perform(
        &mut self,
        action: FloodAction,
        cfg: &FloodProtectorConfig,
        now_tick: i64,
        client: u32,
    ) -> FloodOutcome {
        // `interval <= 0` disables the slot: `next_tick` would never advance
        // past `now_tick`, so every request passes. The dist ships `UseItem` at
        // 0 "to match retail". Short-circuited rather than left to the
        // arithmetic so the intent is legible; the two are equivalent.
        if !cfg.is_enabled() {
            return FloodOutcome::Allow;
        }

        let slot = &mut self.slots[action.index()];
        if now_tick < slot.next_tick {
            if cfg.log_flooding && !slot.logged {
                let early = (cfg.interval as i64 - (slot.next_tick - now_tick)) * MILLIS_IN_TICK;
                tracing::warn!(
                    "{}: client {client} called command ~{early} ms after previous command",
                    action.type_name()
                );
                slot.logged = true;
            }
            slot.count += 1;

            // Java: `PUNISHMENT_LIMIT > 0 && _count >= PUNISHMENT_LIMIT`. The
            // count is *not* reset afterwards, so every further flooded request
            // re-fires the punishment — kept, since `start_punishment` is
            // idempotent for an existing ban/jail and a kicked client is gone.
            if cfg.punishment_limit > 0
                && slot.count >= cfg.punishment_limit
                && cfg.punishment_type != FloodPunishment::None
            {
                return FloodOutcome::RejectAndPunish(cfg.punishment_type);
            }
            return FloodOutcome::Reject;
        }

        if slot.count > 0 && cfg.log_flooding {
            tracing::warn!(
                "{}: client {client} issued {} extra requests within ~{} ms",
                action.type_name(),
                slot.count,
                cfg.interval as i64 * MILLIS_IN_TICK
            );
        }
        slot.next_tick = now_tick + cfg.interval as i64;
        slot.logged = false;
        slot.count = 0;
        FloodOutcome::Allow
    }

    /// The normal entry point: same check, with the slot's config looked up.
    pub fn check(
        &mut self,
        action: FloodAction,
        cfg: &FloodProtectorsConfig,
        now_tick: i64,
        client: u32,
    ) -> FloodOutcome {
        self.can_perform(action, cfg.get(action), now_tick, client)
    }
}

/// The protector slot guarding a plain (single-byte) opcode, or `None` if Java
/// does not flood-protect that packet.
///
/// Every row is one Java `getFloodProtectors().canX()` call site. Four Java
/// sites have no row because the port does not dispatch the packet:
/// `RequestCrystallizeEstimate`, `RequestCrystallizeItemCancel`,
/// `RequestPreviewItem` (Transaction) and `RequestOneDayRewardReceive`
/// (PlayerAction) — each unported for its own reasons, and each will need a row
/// here when it lands.
pub fn action_for_opcode(opcode: u8) -> Option<FloodAction> {
    Some(match opcode {
        // --- Transaction (Java: 24 sites, one shared protector) -------------
        cop::REQUEST_BUY_ITEM
        | cop::REQUEST_BUY_SEED
        | cop::REQUEST_CRYSTALLIZE_ITEM
        | cop::REQUEST_DESTROY_ITEM
        | cop::REQUEST_GET_ITEM_FROM_PET
        | cop::REQUEST_GIVE_ITEM_TO_PET
        | cop::REQUEST_HENNA_EQUIP
        | cop::REQUEST_HENNA_REMOVE
        | cop::REQUEST_PACKAGE_SEND
        | cop::REQUEST_PRIVATE_STORE_BUY
        | cop::REQUEST_PRIVATE_STORE_SELL
        | cop::REQUEST_RECIPE_BOOK_DESTROY
        | cop::REQUEST_SELL_ITEM
        | cop::SEND_WARE_HOUSE_DEPOSIT_LIST
        | cop::SEND_WARE_HOUSE_WITH_DRAW_LIST
        | cop::TRADE_DONE => FloodAction::Transaction,

        // --- PlayerAction ---------------------------------------------------
        // `ATTACK` (0x01) and `ATTACK_REQUEST` (0x32) share one handler here;
        // Java flood-protects `AttackRequest`, and the other opcode reaches the
        // same code, so both are charged.
        cop::ACTION | cop::ATTACK | cop::ATTACK_REQUEST => FloodAction::PlayerAction,

        // --- CharacterSelect ------------------------------------------------
        cop::CHARACTER_DELETE | cop::CHARACTER_RESTORE | cop::CHARACTER_SELECT => {
            FloodAction::CharacterSelect
        }

        // --- UseItem (disabled on this dist: interval 0) --------------------
        cop::USE_ITEM | cop::REQUEST_PET_USE_ITEM => FloodAction::UseItem,

        // --- Manufacture ----------------------------------------------------
        cop::REQUEST_RECIPE_ITEM_MAKE_SELF | cop::REQUEST_RECIPE_SHOP_MAKE_ITEM => {
            FloodAction::Manufacture
        }

        // --- one-site slots -------------------------------------------------
        cop::REQUEST_DROP_ITEM => FloodAction::DropItem,
        cop::REQUEST_BYPASS_TO_SERVER => FloodAction::ServerBypass,
        cop::MULTI_SELL_CHOOSE => FloodAction::MultiSell,

        _ => return None,
    })
}

/// The protector slot guarding a `0xD0` extended sub-opcode.
pub fn action_for_ex_opcode(sub: u16) -> Option<FloodAction> {
    Some(match sub {
        exop::REQUEST_BID_ITEM_AUCTION
        | exop::REQUEST_CANCEL_POST_ATTACHMENT
        | exop::REQUEST_POST_ATTACHMENT
        | exop::REQUEST_REFUND_ITEM
        | exop::REQUEST_REJECT_POST_ATTACHMENT => FloodAction::Transaction,

        exop::REQUEST_EX_ENCHANT_SKILL => FloodAction::PlayerAction,
        exop::REQUEST_INFO_ITEM_AUCTION => FloodAction::ItemAuction,
        exop::REQUEST_SEND_POST => FloodAction::SendMail,

        _ => return None,
    })
}

/// The dispatch gate: `true` = run the handler, `false` = drop the packet.
///
/// Java's `canPerformAction` opens with the GM escape hatch
/// (`canOverrideCond(PlayerCondOverride.FLOOD_CONDITIONS)`); this port has no
/// per-flag cond-override table, and `is_gm()` stands in for "all overrides
/// enabled" everywhere else, so it stands in here too.
pub(crate) fn gate(world: &mut World, client_id: u32, action: FloodAction) -> bool {
    // Java tests `_client.getPlayer() != null` first — a client with no player
    // attached is never exempt, however privileged the account.
    let gm = world
        .player_oid(client_id)
        .and_then(|oid| world.objects.get_component::<Player>(&oid))
        .is_some_and(|p| p.is_gm(&world.data));
    if gm {
        return true;
    }

    let now_tick = world.tick as i64;
    let outcome = match world.clients.get_mut(&client_id) {
        // `world.clients` and `world.cfg` are distinct fields, so the mutable
        // session borrow and the shared config borrow do not overlap.
        Some(session) => {
            session
                .flood_mut()
                .check(action, &world.cfg.flood_protector, now_tick, client_id)
        }
        // No session (already disconnected): nothing to rate-limit.
        None => return true,
    };

    match outcome {
        FloodOutcome::Allow => true,
        FloodOutcome::Reject => false,
        FloodOutcome::RejectAndPunish(punishment) => {
            apply_punishment(world, client_id, action, punishment);
            false
        }
    }
}

/// Java `FloodProtectorAction.kickPlayer/banAccount/jailChar`.
fn apply_punishment(
    world: &mut World,
    client_id: u32,
    action: FloodAction,
    punishment: FloodPunishment,
) {
    let type_name = action.type_name();
    let time_millis = world.cfg.flood_protector.get(action).punishment_time_millis;
    // Java's `PUNISHMENT_TIME` is already an absolute duration in millis; the
    // punishment manager wants an absolute expiry, with 0 meaning forever.
    let expiration = if time_millis > 0 {
        commons::util::now_millis() + time_millis
    } else {
        0
    };
    let forever_or = |suffix: &str| {
        if time_millis <= 0 {
            "forever".to_string()
        } else {
            format!("for {} mins{suffix}", time_millis / 60_000)
        }
    };

    match punishment {
        FloodPunishment::Kick => {
            tracing::warn!("{type_name}: client {client_id} kicked for flooding");
            kick(world, client_id);
        }
        FloodPunishment::Ban => {
            let Some(account) = world.clients.get(&client_id).and_then(|s| s.account()) else {
                tracing::warn!(
                    "{type_name}: client {client_id} flooded past the punishment limit \
                     but has no account to ban"
                );
                return;
            };
            let account = account.to_string();
            tracing::warn!(
                "{type_name}: account {account} banned for flooding {}",
                forever_or("")
            );
            super::punishment::start_punishment(
                world,
                account,
                PunishmentAffect::Account,
                PunishmentType::Ban,
                expiration,
                String::new(),
                type_name.to_string(),
            );
        }
        FloodPunishment::Jail => {
            // Java `jailChar` returns early when no player is attached.
            let Some(char_id) = world.player_oid(client_id).filter(|id| *id > 0) else {
                return;
            };
            tracing::warn!(
                "{type_name}: character {char_id} jailed for flooding {}",
                forever_or("")
            );
            super::punishment::start_punishment(
                world,
                char_id.to_string(),
                PunishmentAffect::Character,
                PunishmentType::Jail,
                expiration,
                String::new(),
                type_name.to_string(),
            );
        }
        FloodPunishment::None => {}
    }
}

/// Java `Disconnection.of(client).defaultSequence(LeaveWorld)`, which works in
/// every connection state. An in-game client goes through the full teardown
/// (persist + despawn); anything earlier just gets the packet and has its
/// session dropped, which closes the socket when the outbound sender goes.
fn kick(world: &mut World, client_id: u32) {
    match world.player_oid(client_id) {
        Some(oid) => disconnect_player(world, oid),
        None => {
            if let Some(session) = world.clients.remove(&client_id) {
                session.send(crate::network::server_packets::leave_world());
            }
        }
    }
}

/// Slots with no opcode row, and why. Asserted by a test so the list cannot
/// quietly rot: three are dead in Java too, `Subclass` is driven from the
/// village-master bypass rather than from an opcode.
#[cfg(test)]
const UNMAPPED_ACTIONS: [FloodAction; 5] = [
    // Java configures these four but never calls them from anywhere.
    FloodAction::RollDice,
    FloodAction::ItemPetSummon,
    FloodAction::HeroVoice,
    FloodAction::GlobalChat,
    // Java calls this from `VillageMaster`'s subclass bypass, not a packet.
    FloodAction::Subclass,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::flood_protector::ALL_FLOOD_ACTIONS;

    fn cfg(interval: i32) -> FloodProtectorConfig {
        FloodProtectorConfig {
            interval,
            ..Default::default()
        }
    }

    /// The core contract: one request per interval, the rest refused until the
    /// window rolls over.
    #[test]
    fn one_request_per_interval() {
        let mut fp = FloodProtectors::new();
        let c = cfg(10);
        let a = FloodAction::Transaction;

        assert_eq!(fp.can_perform(a, &c, 1000, 1), FloodOutcome::Allow);
        assert_eq!(
            fp.can_perform(a, &c, 1000, 1),
            FloodOutcome::Reject,
            "same tick"
        );
        assert_eq!(
            fp.can_perform(a, &c, 1009, 1),
            FloodOutcome::Reject,
            "one tick short of the window"
        );
        assert_eq!(
            fp.can_perform(a, &c, 1010, 1),
            FloodOutcome::Allow,
            "the window opens exactly at start + interval"
        );
    }

    /// `interval = 0` is the dist's way of turning a protector off (`UseItem`).
    #[test]
    fn interval_zero_never_refuses() {
        let mut fp = FloodProtectors::new();
        let c = cfg(0);
        for _ in 0..100 {
            assert_eq!(
                fp.can_perform(FloodAction::UseItem, &c, 500, 1),
                FloodOutcome::Allow
            );
        }
    }

    /// Slots are independent — flooding one must not spend another's budget.
    #[test]
    fn slots_do_not_share_state() {
        let mut fp = FloodProtectors::new();
        let c = cfg(10);
        assert!(
            fp.can_perform(FloodAction::Transaction, &c, 100, 1)
                .is_allowed()
        );
        assert!(
            !fp.can_perform(FloodAction::Transaction, &c, 100, 1)
                .is_allowed()
        );
        assert!(
            fp.can_perform(FloodAction::DropItem, &c, 100, 1)
                .is_allowed(),
            "a different protector has its own window"
        );
    }

    /// The punishment fires on the Nth *refused* request inside one interval,
    /// not the Nth request overall (Java counts only the refusals).
    #[test]
    fn punishment_fires_at_the_limit_of_refusals() {
        let mut fp = FloodProtectors::new();
        let c = FloodProtectorConfig {
            interval: 10,
            punishment_limit: 3,
            punishment_type: FloodPunishment::Kick,
            ..Default::default()
        };
        let a = FloodAction::Transaction;

        assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Allow);
        assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Reject); // count 1
        assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Reject); // count 2
        assert_eq!(
            fp.can_perform(a, &c, 100, 1),
            FloodOutcome::RejectAndPunish(FloodPunishment::Kick),
            "third refusal reaches the limit"
        );
    }

    /// `PunishmentLimit = 0` (what every slot ships as) means never punish, no
    /// matter how hard the client floods.
    #[test]
    fn punishment_limit_zero_never_punishes() {
        let mut fp = FloodProtectors::new();
        let c = FloodProtectorConfig {
            interval: 10,
            punishment_limit: 0,
            punishment_type: FloodPunishment::Ban,
            ..Default::default()
        };
        let a = FloodAction::Transaction;
        assert!(fp.can_perform(a, &c, 100, 1).is_allowed());
        for _ in 0..50 {
            assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Reject);
        }
    }

    /// A type of `none` disables the punishment even with a limit set — Java
    /// tests the string against kick/ban/jail and falls through otherwise.
    #[test]
    fn punishment_type_none_never_punishes() {
        let mut fp = FloodProtectors::new();
        let c = FloodProtectorConfig {
            interval: 10,
            punishment_limit: 1,
            punishment_type: FloodPunishment::None,
            ..Default::default()
        };
        let a = FloodAction::Transaction;
        assert!(fp.can_perform(a, &c, 100, 1).is_allowed());
        assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Reject);
    }

    /// The refusal counter resets when the window rolls over, so a client that
    /// floods a little, waits, then floods a little again is never punished.
    #[test]
    fn the_refusal_count_resets_with_the_window() {
        let mut fp = FloodProtectors::new();
        let c = FloodProtectorConfig {
            interval: 10,
            punishment_limit: 3,
            punishment_type: FloodPunishment::Kick,
            ..Default::default()
        };
        let a = FloodAction::Transaction;

        assert!(fp.can_perform(a, &c, 100, 1).is_allowed());
        assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Reject);
        assert_eq!(fp.can_perform(a, &c, 100, 1), FloodOutcome::Reject); // count 2

        assert!(fp.can_perform(a, &c, 110, 1).is_allowed(), "window rolls");
        assert_eq!(
            fp.can_perform(a, &c, 110, 1),
            FloodOutcome::Reject,
            "count restarted at 1, well under the limit of 3"
        );
    }

    /// Every Java call site must be reachable from the tables, and every slot
    /// must be either mapped or explicitly accounted for as unmapped.
    #[test]
    fn every_slot_is_mapped_or_explicitly_unmapped() {
        let mapped: std::collections::HashSet<FloodAction> = (0u8..=255)
            .filter_map(action_for_opcode)
            .chain((0u16..=0x200).filter_map(action_for_ex_opcode))
            .collect();
        for action in ALL_FLOOD_ACTIONS {
            let is_unmapped = UNMAPPED_ACTIONS.contains(&action);
            assert_eq!(
                mapped.contains(&action),
                !is_unmapped,
                "{} is in neither the opcode table nor the documented-unmapped list",
                action.type_name()
            );
        }
    }

    /// Spot-check the mapping against the Java call sites it was derived from.
    #[test]
    fn opcodes_map_to_the_java_call_sites() {
        assert_eq!(
            action_for_opcode(cop::TRADE_DONE),
            Some(FloodAction::Transaction)
        );
        assert_eq!(
            action_for_opcode(cop::MULTI_SELL_CHOOSE),
            Some(FloodAction::MultiSell)
        );
        assert_eq!(
            action_for_opcode(cop::REQUEST_BYPASS_TO_SERVER),
            Some(FloodAction::ServerBypass)
        );
        assert_eq!(
            action_for_ex_opcode(exop::REQUEST_SEND_POST),
            Some(FloodAction::SendMail)
        );
        assert_eq!(
            action_for_ex_opcode(exop::REQUEST_INFO_ITEM_AUCTION),
            Some(FloodAction::ItemAuction)
        );
        // The ex-packet envelope itself must not be charged, or every extended
        // packet would pay twice — once for 0xD0, once for its sub-opcode.
        assert_eq!(action_for_opcode(cop::EX_PACKET), None);
        // A packet Java does not protect stays unprotected.
        assert_eq!(action_for_opcode(cop::SAY2), None);
    }
}
