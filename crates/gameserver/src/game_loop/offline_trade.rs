//! Offline shops — port of `util/OfflineTradeUtil` + `data/sql/OfflineTraderTable`
//! and the `.offline` voiced command (`handlers/voicedcommandhandlers/Offline`).
//!
//! An **offline trader** is a character whose client is gone but whose `Player`
//! stays in the world with its private store open, so the shop keeps trading
//! while its owner is away. Java models this as a *detached* `GameClient`: the
//! socket closes, `Player.setClient(null)`, and `deleteMe()` is skipped, so the
//! player object simply loses its packet sink. The port has no session at all
//! for such a player — [`helpers::client_for_player`](super::helpers::client_for_player)
//! returns `None`, which makes every `send`/`broadcast` path a no-op for them,
//! exactly like writing to a detached client.
//!
//! Because "which players exist" is answered from `world.clients` in the
//! visibility paths, session-less players need their own index: `World::
//! offline_traders`. It is the *only* set of players in the world with no
//! session, so the visibility scans add it as a second subject source rather
//! than becoming registry-driven wholesale.
//!
//! Persistence lives in the two Java tables (`character_offline_trade` /
//! `character_offline_trade_items`), written after each transaction when
//! `StoreOfflineTradeInRealtime` is on (it is, on this dist) and restored at
//! boot by [`restore_offline_traders`].
//!
//! The NPC-side visibility divergence this header once carried has closed
//! under it: the aggro scans became region-index-driven
//! (`World::players_visible_from`), whose plain form deliberately yields
//! unattended shops — a monster notices an offline store exactly like Java's
//! registry-driven knownlist. Only the "active region" seeding
//! (`occupied_player_cells`) and the teleport-home check still distinguish
//! connected players, and each says why at its site.

use crate::game_loop::helpers::player_name_or_empty;
use tracing::info;

use crate::db;
use crate::model::Player;
use crate::model::components::{ManufactureStore, PrivateBuyStore, PrivateStore, ZoneFlags};
use crate::network::server_packets as sp;
use crate::session::ClientSession;
use crate::world::World;

/// Java `PrivateStoreType` ids (the CharInfo/UserInfo store byte).
pub(crate) const STORE_NONE: u8 = 0;
pub(crate) const STORE_SELL: u8 = 1;
pub(crate) const STORE_BUY: u8 = 3;
pub(crate) const STORE_MANUFACTURE: u8 = 5;
pub(crate) const STORE_PACKAGE_SELL: u8 = 8;

/// One session-less shop owner still standing in the world (Java: a `Player`
/// whose `GameClient.isDetached()`).
#[derive(Debug, Clone, Copy)]
pub struct OfflineTrader {
    /// Java `Player.getOfflineStartTime()` — when the client detached. Kept
    /// across a restart so `OfflineMaxDays` measures from the *first* time the
    /// shop went offline, not from the last boot.
    pub start_time_millis: i64,
}

pub(crate) fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Java `Player.isInOfflineMode()` — is this player one of the session-less
/// shops? Used to keep the disconnect path from deleting them.
pub(crate) fn is_offline_trader(world: &World, object_id: i32) -> bool {
    world.offline_traders.contains_key(&object_id)
}

/// The player's store type byte, or `STORE_NONE`.
fn store_type(world: &World, object_id: i32) -> u8 {
    world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| p.store_type)
        .unwrap_or(STORE_NONE)
}

/// Java `OfflineTradeUtil.offlineMode(player)`: may this player stay behind as
/// an unattended shop?
///
/// Note Java's switch: **`MANUFACTURE` is gated by `OfflineTradeEnable`**, not
/// by `OfflineCraftEnable` — the craft flag covers only the `default:` branch
/// (`isCrafting()`, the recipe window open with no store). The port keeps that
/// as-is; it is not a typo we get to fix.
pub(crate) fn can_enter_offline_mode(world: &World, object_id: i32) -> bool {
    let cfg = &world.cfg.offline_trade;
    // Java: `isInOlympiadMode() || isRegisteredOnEvent() || isJailed() ||
    // getVehicle() != null`. Boats are the port's only vehicle and a boat
    // passenger has no store open, so that arm folds into the store check.
    let in_olympiad = world.olympiad.is_registered(object_id)
        || world
            .olympiad
            .matches
            .iter()
            .any(|m| m.player_a == object_id || m.player_b == object_id)
        || super::olympiad::is_observing(world, object_id);
    let on_event = world.events.tvt.player_list.contains(&object_id);
    let jailed = world
        .objects
        .get_component::<Player>(&object_id)
        .is_some_and(|p| p.jailed);
    if in_olympiad || on_event || jailed {
        return false;
    }
    let mut can_set_shop = match store_type(world, object_id) {
        STORE_SELL | STORE_PACKAGE_SELL | STORE_BUY | STORE_MANUFACTURE => cfg.trade_enable,
        // Java's `default:` — no store, but a craft in progress:
        // `canSetShop = OFFLINE_CRAFT_ENABLE && player.isCrafting()`.
        //
        // Java `player.isCrafting()`: only the `AltGameCreation` staged craft
        // can be observed mid-flight by another packet — the default inline
        // craft finishes inside its own handler. With the staged machinery
        // ported (`crafting::ActiveCraft` + the pass/finish tasks), this gate
        // is real whenever an operator flips the config on.
        _ => crate::game_loop::crafting::is_crafting(world, object_id),
    };
    if cfg.mode_in_peace_zone
        && !world
            .objects
            .get_component::<ZoneFlags>(&object_id)
            .is_some_and(|z| z.contains(crate::data::zone_data::ZoneKind::Peace))
    {
        can_set_shop = false;
    }
    can_set_shop
}

/// Java `OfflineTradeUtil.enteredOfflineMode(player)`: drop the session but
/// keep the player (and their shop) in the world. `false` when they don't
/// qualify, in which case the caller runs the normal logout.
pub(crate) fn enter_offline_mode(world: &mut World, client_id: u32) -> bool {
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return false;
    };
    let object_id = s.player_object_id();
    if !can_enter_offline_mode(world, object_id) {
        return false;
    }

    // `client.close(ServerClose)` + `setDetached(true)`: dropping the session
    // flushes the queued packet and closes the socket. The player entity stays.
    if let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) {
        s.send(sp::leave_world());
    }
    // The account is no longer *playing*, so the login server must be told —
    // Java `GameClient.onDisconnection` sends the logout even in offline mode.
    release_account(world, client_id);

    // `leaveParty()` + olympiad unregister; the shop stands alone.
    super::party::on_player_leave_world(world, object_id);
    super::olympiad::unregister(world, object_id);
    // Pets/servitors do not stand around unattended: Java unsummons them with
    // `setRestoreSummon(true)` so they come back when the owner really logs in.
    super::servitor::on_owner_leave_world(world, object_id);
    super::cubic::on_owner_leave_world(world, object_id);
    // **No friend/clan "logged off" notice, deliberately.** Those fire from
    // Java's `Player.deleteMe()`, which `enteredOfflineMode` skips — the
    // character is still in `World.getPlayers()` and still counts as online to
    // its friends and clan. The account logout above goes to the *login*
    // server; the game-world status does not change.

    if world.cfg.offline_trade.set_name_color
        && let Some(p) = world.objects.get_component_mut::<Player>(&object_id)
    {
        p.name_color = world.cfg.offline_trade.name_color;
    }
    // `startAbnormalVisualEffect(OFFLINE_ABNORMAL_EFFECTS.get(Rnd.get(size)))`
    // — **one** of the configured effects, chosen at random, not all of them.
    // It rides on `AdminVisuals` because that is the port's home for a visual
    // with no buff behind it, which is exactly what this is: the shop shows
    // the marker without gaining an effect.
    if !world.cfg.offline_trade.abnormal_effects.is_empty() {
        let n = world.cfg.offline_trade.abnormal_effects.len();
        let idx = world.roll(n as i32) as usize;
        let pick = world.cfg.offline_trade.abnormal_effects[idx];
        let mut visuals = world
            .objects
            .get_component::<crate::model::components::AdminVisuals>(&object_id)
            .cloned()
            .unwrap_or_default();
        visuals.0.push(pick);
        world.objects.add_components(&object_id, visuals);
    }
    // Everyone nearby re-renders the shop with its new name colour.
    super::visibility::refresh_char_info(world, object_id);

    // Java `sitDown()`s the trader — an unattended shop sits behind its wares.
    super::sit_stand::sit_down(world, object_id);

    let start_time_millis = now_millis();
    world
        .offline_traders
        .insert(object_id, OfflineTrader { start_time_millis });

    // `STORE_OFFLINE_TRADE_IN_REALTIME` → write the shop now, so an unexpected
    // shutdown still finds it. Otherwise only the shutdown sweep stores it.
    if world.cfg.offline_trade.store_in_realtime {
        store_trader(world, object_id);
    }
    super::net::store_player_now(world, object_id); // `player.storeMe()`

    let name = player_name_or_empty(world, object_id);
    info!("GameLoop: '{name}' entered offline shop mode.");
    true
}

/// Tell the login server the account left, and forget the client — the tail of
/// `net::on_disconnect` that still applies when the body stays behind.
fn release_account(world: &mut World, client_id: u32) {
    world.hwids.remove(&client_id);
    let account = world
        .login
        .accounts_in_gameserver
        .iter()
        .find(|(_, id)| **id == client_id)
        .map(|(a, _)| a.clone());
    if let Some(account) = account {
        world.login.accounts_in_gameserver.remove(&account);
        world.login.waiting.remove(&account);
        let _ = world
            .login
            .link
            .send(crate::loginlink::LoginLinkCommand::PlayerLogout { account });
    }
}

/// Java `OfflineTraderTable.removeTrader` + `Disconnection.storeMe().deleteMe()`
/// — the shop leaves the world for good (sold out, owner logged back in, or the
/// same account logged in elsewhere). Safe to call for a player who is not an
/// offline trader: it then only clears any stale rows.
pub(crate) fn remove_trader(world: &mut World, object_id: i32) {
    let was_offline = world.offline_traders.remove(&object_id).is_some();
    let _ = world
        .db
        .send(db::DbCommand::ClearOfflineTrader { char_id: object_id });
    if was_offline {
        // Java `deleteMe()` — the same teardown a logout runs.
        super::net::store_and_remove_player(world, object_id);
    }
}

/// Java `Player.setPrivateStoreType`'s `OFFLINE_DISCONNECT_FINISHED` branch: an
/// unattended shop whose store type just dropped to NONE (it sold out, or the
/// buy store filled) leaves the world instead of standing there empty. Called
/// from every path that closes a store.
pub(crate) fn on_store_type_cleared(world: &mut World, object_id: i32) {
    if !is_offline_trader(world, object_id) {
        return;
    }
    if world.cfg.offline_trade.disconnect_finished {
        remove_trader(world, object_id);
    } else if world.cfg.offline_trade.store_in_realtime {
        // The shop stays, but its rows must stop advertising a store it no
        // longer has.
        let _ = world
            .db
            .send(db::DbCommand::ClearOfflineTrader { char_id: object_id });
    }
}

/// Java `OfflineTraderTable.onTransaction(trader, false, …)`: rewrite this
/// trader's rows from its live store. Called on entering offline mode and after
/// every transaction against an unattended shop, when realtime storing is on.
pub(crate) fn store_trader(world: &World, object_id: i32) {
    let Some(&OfflineTrader { start_time_millis }) = world.offline_traders.get(&object_id) else {
        return;
    };
    let store_type = store_type(world, object_id);
    let (title, items) = match store_type {
        STORE_SELL | STORE_PACKAGE_SELL => {
            let Some(store) = world.objects.get_component::<PrivateStore>(&object_id) else {
                return;
            };
            (
                store.title.clone(),
                // Java stores the *object* id for a sell store — the exact
                // instance on offer, enchant and all.
                store
                    .items
                    .iter()
                    .map(|i| (i.object_id, i.count, i.price))
                    .collect(),
            )
        }
        STORE_BUY => {
            let Some(store) = world.objects.get_component::<PrivateBuyStore>(&object_id) else {
                return;
            };
            (
                store.title.clone(),
                // A buy store holds no instances yet, so Java stores item ids.
                store
                    .items
                    .iter()
                    .map(|i| (i.item_id, i.count, i.price))
                    .collect(),
            )
        }
        STORE_MANUFACTURE => {
            let Some(store) = world.objects.get_component::<ManufactureStore>(&object_id) else {
                return;
            };
            (
                store.title.clone(),
                // Recipe id + price; Java writes count 0 for these lines.
                store
                    .items
                    .iter()
                    .map(|&(recipe_id, cost)| (recipe_id, 0, cost))
                    .collect(),
            )
        }
        _ => return,
    };
    let _ = world.db.send(db::DbCommand::StoreOfflineTrader {
        char_id: object_id,
        time: start_time_millis,
        store_type: store_type as i32,
        title,
        items,
    });
}

/// `RequestPrivateStoreBuy`/`Sell`'s tail: `onTransaction(storePlayer,
/// itemCount == 0, false)` — rewrite the seller's rows after a sale, or clear
/// them when the shop is now empty. No-op for an online seller.
pub(crate) fn on_transaction(world: &mut World, object_id: i32) {
    if !is_offline_trader(world, object_id) || !world.cfg.offline_trade.store_in_realtime {
        return;
    }
    if store_type(world, object_id) == STORE_NONE {
        // The store closed itself; `on_store_type_cleared` already ran.
        return;
    }
    store_trader(world, object_id);
}

/// Java `EnterWorld`'s `onTransaction(player, true, false)` — a character that
/// logs in for real drops any offline-shop rows it left behind. Also covers the
/// `OFFLINE_DISCONNECT_SAME_ACCOUNT` case, where the shop is still standing.
pub(crate) fn on_enter_world(world: &mut World, object_id: i32) {
    if !world.cfg.offline_trade.any_enabled() {
        return;
    }
    let _ = world
        .db
        .send(db::DbCommand::ClearOfflineTrader { char_id: object_id });
}

/// Java `CharSelectionInfo`'s `OFFLINE_DISCONNECT_SAME_ACCOUNT` branch: seeing
/// the character list for an account evicts that account's unattended shops.
pub(crate) fn on_character_list(world: &mut World, char_ids: &[i32]) {
    if !world.cfg.offline_trade.disconnect_same_account {
        return;
    }
    for &char_id in char_ids {
        if is_offline_trader(world, char_id) {
            remove_trader(world, char_id);
        }
    }
}

/// Java `Shutdown`'s `storeOffliners()`: with realtime storing **off**, the
/// rows are only written here, at shutdown. With it on (this dist) the rows are
/// already current and Java skips this entirely.
pub(crate) fn store_offliners(world: &World) {
    let cfg = &world.cfg.offline_trade;
    if !cfg.any_enabled() || !cfg.restore_offliners || cfg.store_in_realtime {
        return;
    }
    let ids: Vec<i32> = world.offline_traders.keys().copied().collect();
    for id in &ids {
        store_trader(world, *id);
    }
    if !ids.is_empty() {
        info!("GameLoop: stored {} offline shop(s).", ids.len());
    }
}

// ---------------------------------------------------------------------------
// The `.offline` voiced command (`handlers/voicedcommandhandlers/Offline`)
// ---------------------------------------------------------------------------

/// Java `Offline.useVoicedCommand`: check the player is in store mode and may
/// log out, then ask for confirmation. The offline switch itself happens on the
/// `DlgAnswer` reply, not here.
pub(crate) fn handle_voiced_offline(world: &mut World, client_id: u32) {
    let cfg = &world.cfg.offline_trade;
    if !cfg.enable_offline_command || !cfg.any_enabled() {
        return;
    }
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = s.player_object_id();
    if store_type(world, object_id) == STORE_NONE {
        // Java: "Private store already closed." + ActionFailed.
        s.send(sp::system_message_with(
            sp::sm_ids::PRIVATE_STORE_ALREADY_CLOSED,
            &[],
        ));
        s.send(sp::action_failed());
        return;
    }
    // Java: `isInInstance() || isInVehicle() || !canLogout()`.
    if super::helpers::instance_of(world, object_id) != 0
        || super::combat::has_attack_stance(world, object_id)
    {
        s.send(sp::action_failed());
        return;
    }
    s.send(sp::confirm_dlg(
        sp::sm_ids::DO_YOU_WISH_TO_EXIT_THE_GAME as i32,
    ));
}

/// Java `DlgAnswer`'s `DO_YOU_WISH_TO_EXIT_THE_GAME` branch: on "yes", re-run
/// the same gates (the state may have changed while the dialog was open) and go
/// offline — falling back to a normal logout when the player no longer
/// qualifies, exactly as Java's `if (!enteredOfflineMode(player))` does.
///
/// Returns `true` when the reply was this dialog's, so the shared `DlgAnswer`
/// dispatch stops looking for another owner.
pub(crate) fn handle_exit_game_answer(world: &mut World, client_id: u32, yes: bool) -> bool {
    let cfg = &world.cfg.offline_trade;
    if !cfg.enable_offline_command || !cfg.any_enabled() {
        return false;
    }
    if !yes {
        return true; // Java returns without doing anything on answer 0.
    }
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return true;
    };
    let object_id = s.player_object_id();
    if store_type(world, object_id) == STORE_NONE {
        s.send(sp::system_message_with(
            sp::sm_ids::PRIVATE_STORE_ALREADY_CLOSED,
            &[],
        ));
        return true;
    }
    if super::helpers::instance_of(world, object_id) != 0
        || super::combat::has_attack_stance(world, object_id)
    {
        return true;
    }
    super::olympiad::unregister(world, object_id);
    if !enter_offline_mode(world, client_id) {
        super::net::handle_logout(world, client_id);
    }
    true
}

// ---------------------------------------------------------------------------
// Boot restore (`OfflineTraderTable.restoreOfflineTraders`)
// ---------------------------------------------------------------------------

/// Java `GameServer.main`'s `restoreOfflineTraders()`: bring every stored shop
/// back into the world with its store re-opened, so a restart is invisible to
/// shoppers.
///
/// The rows are always loaded (the DB thread has no config), so this is also
/// where a server that turned the feature off clears them — Java gets the same
/// end state by never reading them and letting `storeOffliners` overwrite.
pub(crate) fn restore_offline_traders(
    world: &mut World,
    traders: Vec<crate::db::OfflineTraderRow>,
) {
    let cfg = world.cfg.offline_trade.clone();
    if !cfg.any_enabled() || !cfg.restore_offliners {
        return;
    }
    let now = now_millis();
    let mut restored = 0usize;
    for row in traders {
        // Java `OFFLINE_MAX_DAYS`: a shop older than the limit is not brought
        // back (0 disables the check).
        if cfg.max_days > 0 && row.time + (cfg.max_days as i64) * 86_400_000 <= now {
            let _ = world.db.send(db::DbCommand::ClearOfflineTrader {
                char_id: row.char.object_id,
            });
            continue;
        }
        let store_type = row.store_type as u8;
        if store_type == STORE_NONE {
            continue; // Java: `type == NONE` → skip.
        }
        if restore_one(world, row, store_type) {
            restored += 1;
        }
    }
    if restored > 0 {
        info!("GameLoop: restored {restored} offline shop(s).");
    }
}

/// Spawn one stored shop back into the world. `false` when the character can't
/// be rebuilt (Java logs and disconnects the half-loaded player).
fn restore_one(world: &mut World, row: crate::db::OfflineTraderRow, store_type: u8) -> bool {
    use crate::model::components::StoreItem;
    use crate::model::inventory::Inventory;

    let object_id = row.char.object_id;
    if world.objects.get_component::<Player>(&object_id).is_some() {
        return false; // already in the world (shouldn't happen at boot)
    }

    // Same construction as character-select → enter-world: the bundle carries
    // every child collection, and the store byte rides `Player.store_type`.
    let mut bundle = crate::model::Player::from_char(&world.data, &row.char);
    bundle.restore_reuses(&row.char, world.tick, now_millis());
    bundle.restore_buffs(&row.char);
    bundle.player.store_type = store_type;
    if world.cfg.offline_trade.set_name_color {
        bundle.player.name_color = world.cfg.offline_trade.name_color;
    }
    let pending_buffs = std::mem::take(&mut bundle.pending_buffs);
    bundle.spawn_into(world);

    // The same post-spawn wiring enter-world does, minus everything that talks
    // to a client: the stat pumps must run or the shop stands with raw base
    // stats, and `restoreEffects` is in Java's restore path too.
    super::expertise::refresh_expertise_penalty(world, object_id);
    super::weight::refresh_weight_penalty(world, object_id);
    super::passive_skills::refresh_conditioned_passives(world, object_id);
    super::skills::effects::restore_persisted_buffs(world, object_id, &pending_buffs);
    super::zones::revalidate_zone(world, object_id, true);

    // Re-open the store from the stored lines.
    match store_type {
        STORE_SELL | STORE_PACKAGE_SELL => {
            // A sell line names an *object* id: the instance must still be in
            // the seller's inventory (Java's `addItem` returns null otherwise
            // and the line is dropped).
            let items: Vec<StoreItem> = row
                .items
                .iter()
                .filter_map(|&(object_id_of_line, count, price)| {
                    let inv = world.objects.get_component::<Inventory>(&object_id)?;
                    let it = inv.by_object_id(object_id_of_line)?;
                    Some(StoreItem {
                        object_id: it.object_id,
                        item_id: it.item_id,
                        count: count.min(it.count),
                        price,
                        enchant: it.enchant_level,
                    })
                })
                .collect();
            world.objects.add_components(
                &object_id,
                PrivateStore {
                    items,
                    title: row.title,
                    packaged: store_type == STORE_PACKAGE_SELL,
                },
            );
        }
        STORE_BUY => {
            let items = row
                .items
                .iter()
                .map(
                    |&(item_id, count, price)| crate::model::components::WantedItem {
                        item_id,
                        count,
                        price,
                        enchant: 0,
                    },
                )
                .collect();
            world.objects.add_components(
                &object_id,
                PrivateBuyStore {
                    items,
                    title: row.title,
                },
            );
        }
        STORE_MANUFACTURE => {
            let items = row
                .items
                .iter()
                .map(|&(recipe_id, _, cost)| (recipe_id, cost))
                .collect();
            world.objects.add_components(
                &object_id,
                ManufactureStore {
                    items,
                    title: row.title,
                },
            );
        }
        _ => return false,
    }

    world.offline_traders.insert(
        object_id,
        OfflineTrader {
            // Java `setOfflineStartTime(time)` — the *stored* time, so
            // `OfflineMaxDays` keeps counting from the original detach.
            start_time_millis: row.time,
        },
    );
    // Java's `PlayerAutoSaveTaskManager` sweeps `World.getPlayers()`, offline
    // shops included, so the restored player joins the autosave rotation.
    let due = world.tick + world.cfg.character.character_data_store_interval_ticks;
    world.player_autosave_due.insert(object_id, due);
    true
}

/// `PlayerStatus.reduceHp`'s `OFFLINE_MODE_NO_DAMAGE` branch: an unattended
/// shop takes no damage at all.
///
/// Java re-tests the store type here and its list is **narrower** than the one
/// that let the player go offline: only `SELL`/`BUY` under `OfflineTradeEnable`
/// and `MANUFACTURE`/crafting under `OfflineCraftEnable`. A `PACKAGE_SELL` shop
/// is therefore killable in Java even though it could go offline — kept as-is.
pub(crate) fn is_damage_immune(world: &World, object_id: i32) -> bool {
    let cfg = &world.cfg.offline_trade;
    if !cfg.mode_no_damage || !is_offline_trader(world, object_id) {
        return false;
    }
    match store_type(world, object_id) {
        STORE_SELL | STORE_BUY => cfg.trade_enable,
        STORE_MANUFACTURE => cfg.craft_enable,
        _ => false,
    }
}
