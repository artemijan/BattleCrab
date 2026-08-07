//! Service-event handling and session lifecycle: network connect/disconnect
//! events, the login-link and DB results, and restart/logout/kick handling.
//! [`handle_game_event`] routes each unified-channel event to its handler.

use tracing::{debug, error, info, warn};

use crate::db::{self, DbEvent};
use crate::events::GameEvent;
use crate::loginlink::{LoginLinkCommand, LoginLinkEvent};
use crate::network::{NetEvent, server_packets};
use crate::session::{ClientSession, Session};
use crate::world::World;

use super::dispatch::on_packet;

/// Route one unified-channel event to its service's handler. Called by the
/// game loop both from the boundary drain and from the between-ticks sleep
/// (`recv_timeout`), so an event runs the moment it arrives.
pub(crate) fn handle_game_event(world: &mut World, event: GameEvent) {
    match event {
        GameEvent::Net(e) => handle_net_event(world, e),
        GameEvent::Login(e) => handle_login_link_event(world, e),
        GameEvent::Db(e) => handle_db_event(world, e),
        GameEvent::Path(e) => super::position::handle_path_result(world, e),
    }
}

/// The per-packet counter, resolved once. Looking a metric up by name takes the
/// registry lock, so the hot path holds the handle instead — after the first
/// call this is a relaxed atomic add and nothing else.
fn packets_handled() -> &'static commons::metrics::Counter {
    static C: std::sync::OnceLock<commons::metrics::Counter> = std::sync::OnceLock::new();
    C.get_or_init(|| commons::metrics::counter("packets_handled"))
}

/// Players currently connected, refreshed as connections come and go.
fn players_online() -> &'static commons::metrics::Gauge {
    static G: std::sync::OnceLock<commons::metrics::Gauge> = std::sync::OnceLock::new();
    G.get_or_init(|| commons::metrics::gauge("players_online"))
}

/// Registers the metrics above at boot so they read `0` from the first snapshot
/// instead of being *absent* until the first packet arrives. An absent series
/// and a zero one graph very differently, and "no players yet" is exactly the
/// state worth being able to see.
pub fn register_metrics() {
    packets_handled();
    players_online().set(0);
    super::tick_busy_micros().set(0);
    crate::network::register_metrics();
}

/// One network event: connect, inbound packet (dispatched under the
/// per-packet panic guard), or disconnect.
pub(crate) fn handle_net_event(world: &mut World, event: NetEvent) {
    match event {
        NetEvent::Connected {
            client_id,
            out,
            addr,
        } => {
            world.clients.insert(
                client_id,
                ClientSession::Connecting(Session::new(client_id, out, addr)),
            );
            debug!(
                "GameLoop: client {client_id} connected from {addr} ({} online).",
                world.clients.len()
            );
        }
        // `_permit` is the connection's in-flight slot: holding it to the end
        // of this arm keeps the packet "in flight" until it is fully handled.
        NetEvent::Received {
            client_id,
            data,
            permit: _permit,
        } => {
            // Java `ExecuteThread`/`PacketHandler` catches Throwable around
            // each packet's run(), so one bad packet (an admin command with
            // missing args, a malformed bypass…) must not take the whole
            // game thread down. `World` is a single-thread structure with
            // no lock poisoning to worry about, but the handler may have
            // died mid-mutation, so the offending client's session state is
            // suspect: disconnect them (persist + clean removal) so they
            // come back clean while everyone else plays on.
            let opcode = data.first().copied();
            packets_handled().incr();
            // Correlation span: every log line emitted while handling this
            // packet inherits these fields, which turns "what happened to
            // this player" into one query over the JSON log instead of a
            // manual reconstruction from interleaved lines.
            //
            // Deliberately allocation-free. This is the game thread and the
            // span is built per packet, so the fields are `i32`s only — a
            // `char_name` here would mean a `String` clone per packet. The
            // name lives on the audit records, which carry it already, and
            // `oid` is the join key between the two.
            let span = tracing::info_span!(
                "packet",
                client_id,
                oid = world.player_oid(client_id),
                opcode = opcode
            );
            let _entered = span.enter();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                on_packet(world, client_id, data);
            }))
            .is_err()
            {
                error!(
                    "GameLoop: panic while handling packet {:#04x?} from client {client_id}; disconnecting that client.",
                    opcode.unwrap_or(0)
                );
                // If the save path trips over the same corrupted state,
                // fall back to dropping the raw session (closes the
                // socket, skips the store).
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    on_disconnect(world, client_id);
                }))
                .is_err()
                {
                    error!(
                        "GameLoop: panic in the disconnect path for client {client_id}; dropping the session unsaved."
                    );
                    world.clients.remove(&client_id);
                }
            }
        }
        NetEvent::ProtocolVersion { client_id, version } => {
            world.protocol_versions.insert(client_id, version);
        }
        NetEvent::Disconnected { client_id } => {
            on_disconnect(world, client_id);
        }
    }
    players_online().set(world.clients.len() as u64);
}

/// Take the player out of the world and persist them — Java
/// `Disconnection.storeMe().deleteMe()`. Shared by restart, logout, and
/// unexpected disconnects. Scheduled tasks holding the dead object id no-op.
pub(crate) fn store_and_remove_player(world: &mut World, player_object_id: i32) {
    // deleteMe → leaveParty (DISCONNECTED semantics: leadership transfers)
    // + pending party/friend request cleanup on both sides.
    super::party::on_player_leave_world(world, player_object_id);
    super::party_room::on_player_leave_world(world, player_object_id);
    // deleteMe → notifyFriends(MODE_OFFLINE).
    super::friends::on_leave_world(world, player_object_id);
    // The `Item._published` flags of this player's items die with the `Item`
    // instances, so their chat links stop resolving (Java: the objects leave
    // the world with them).
    super::chat::on_player_leave_world(world, player_object_id);
    // A servitor does not outlive its owner's session. Java stores it in
    // `CharSummonTable` for `RestoreServitorOnReconnect`; persistence is a
    // later slice, so for now it simply goes away with them — which is at
    // least better than leaking an ownerless NPC into the world.
    super::servitor::on_owner_leave_world(world, player_object_id);
    // Cubics do not outlive their owner; nothing persists them.
    super::cubic::on_owner_leave_world(world, player_object_id);
    // deleteMe → clan.broadcastToOnlineMembers(PledgeShowMemberListUpdate offline).
    {
        let clan_id = world
            .objects
            .get_component::<crate::model::Player>(&player_object_id)
            .map(|p| p.clan_id)
            .unwrap_or(0);
        super::clans::on_leave_world(world, player_object_id, clan_id);
    }
    // deleteMe → World.removeVisibleObject: DeleteObject to everyone watching.
    super::visibility::on_leave_world(world, player_object_id);
    // `.apon` does not survive a logout — Java's task manager holds `Player`
    // references and drops anyone offline on its next sweep.
    super::auto_potions::remove(world, player_object_id);
    super::auto_play::remove(world, player_object_id);
    // A buff shop dies with its seller — the flag also gates `canOpenPrivateStore`,
    // so a stale one would follow the character into their next session.
    super::sell_buffs::clear(world, player_object_id);
    // Stop tracking the player for the periodic autosave; the logout flush below
    // is the final save.
    world.player_autosave_due.remove(&player_object_id);
    // Java `Player.stopAllTasks()` cancels `_teleportWatchdog` on disconnection:
    // a teleport still in flight dies with the session rather than firing at a
    // despawned (or re-used) object id.
    world.teleport_watchdog_due.remove(&player_object_id);
    // Shadow-item `_consumingMana` is a field on Java's `Item`, and the logout
    // throws every one of those away; ours is keyed by an object id the next
    // login reads straight back out of the `items` table, so it has to be
    // dropped by hand or the weapon's 60 s beat never re-arms again. Runs
    // before the despawn below — it reads the inventory.
    super::item_mana::on_player_leave_world(world, player_object_id);
    // Gather everything persistence needs before despawn — components drop
    // with the entity (PLAN_ECS_STAGE2 §7 risk 3).
    if let Some(save) = build_save_data(world, player_object_id) {
        // Index entry goes just before the despawn, while the `RegionCell` is
        // still there to locate it — and only on the branch that actually
        // despawns, so a player left in the world keeps receiving broadcasts.
        world.unindex_player(player_object_id);
        world.objects.despawn(&player_object_id);
        let _ = world.db.send(db::DbCommand::StorePlayer { save });
    }
}

/// Gather a player's full persistable state into a [`db::PlayerSaveData`] for a
/// flush — the char row plus every in-memory child collection (inventory,
/// skills, shortcuts, macros, quests). `None` when the core components are
/// missing (not a live player); absent child collections default to empty. This
/// is the single gather point for all four flush triggers: the periodic
/// autosave, logout, class-transfer, and shutdown save-all. Because gameplay
/// only mutates these components (never the DB directly), one flush captures
/// everything the player did since the last one.
pub(crate) fn build_save_data(world: &World, object_id: i32) -> Option<db::PlayerSaveData> {
    use crate::model::components::{
        Macros, PlayerVitals, Position, Quests, Shortcuts, SkillBook, Vitals,
    };
    use crate::model::inventory::Inventory;

    let p = world
        .objects
        .get_component::<crate::model::Player>(&object_id)?;
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    let pvitals = world.objects.get_component::<PlayerVitals>(&object_id)?;
    let base = db::PlayerSnapshot::of(p, pos, vitals, pvitals);

    // The whole persisted item set = inventory + warehouse + freight (the save
    // deletes any `items` row not present, so every container must be included).
    let mut items = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(Inventory::to_rows)
        .unwrap_or_default();
    if let Some(wh) = world
        .objects
        .get_component::<crate::model::inventory::Warehouse>(&object_id)
    {
        items.extend(wh.to_rows());
    }
    if let Some(fr) = world
        .objects
        .get_component::<crate::model::inventory::Freight>(&object_id)
    {
        items.extend(fr.to_rows());
    }
    // Pet-held items persist against the player (Java `PetInventory.getOwnerId`
    // returns the *owner's* id), so they join the same reconciled set.
    if let Some(pi) = world
        .objects
        .get_component::<crate::model::inventory::PetInventory>(&object_id)
    {
        items.extend(pi.to_rows());
    }
    let skill_enchants = world
        .objects
        .get_component::<crate::model::components::SkillEnchants>(&object_id)
        .map(|e| e.0.clone())
        .unwrap_or_default();
    let skills = world
        .objects
        .get_component::<SkillBook>(&object_id)
        .map(|s| {
            s.0.iter()
                // Transform-granted skills (Dismount 839, Dissonance 5437, …)
                // sit in the live book while transformed but are session-only
                // (Java `_transformSkills`, never written by `storeSkills`) —
                // a flush mid-transform must not turn them into learned rows.
                .filter(|(id, _)| !world.data.transforms.is_transform_skill(**id))
                // The GM convenience kits are the same shape: granted at
                // enter-world with Java's `addSkill(skill, false)`, so they
                // must not survive as learned rows — otherwise turning
                // `GMGiveSpecialSkills` back off leaves every GM who ever
                // logged in still holding Super Haste.
                .filter(|(id, _)| !world.data.skill_trees.is_gm_skill(**id))
                .map(|(id, lvl)| (*id, *lvl, skill_enchants.get(id).copied().unwrap_or(0)))
                .collect()
        })
        .unwrap_or_default();
    let shortcuts = world
        .objects
        .get_component::<Shortcuts>(&object_id)
        .map(|s| s.0.values().cloned().collect())
        .unwrap_or_default();
    let macros = world
        .objects
        .get_component::<Macros>(&object_id)
        .map(|m| m.entries.clone())
        .unwrap_or_default();
    let quests = world
        .objects
        .get_component::<Quests>(&object_id)
        .map(|q| q.0.clone())
        .unwrap_or_default();

    let skill_reuses = reuses_to_save(
        world,
        world
            .objects
            .get_component::<crate::model::components::Reuses>(&object_id),
    );
    let skill_buffs = buffs_to_save(
        world,
        world
            .objects
            .get_component::<crate::model::components::Buffs>(&object_id),
    );

    let hennas = world
        .objects
        .get_component::<crate::model::components::HennaSlots>(&object_id)
        .map(henna_rows)
        .unwrap_or_default();

    // Registered recipes as (list_id, is_dwarven) — the component already keeps
    // the two books split, so the flag is known without a RecipeData lookup.
    let recipe_book = world
        .objects
        .get_component::<crate::model::components::RecipeBook>(&object_id)
        .map(|rb| {
            rb.dwarven
                .iter()
                .map(|&id| (id, true))
                .chain(rb.common.iter().map(|&id| (id, false)))
                .collect()
        })
        .unwrap_or_default();

    // A live servitor's row is captured the same way the pet's is: by the
    // caller, before the summon leaves the world.
    let summons = world
        .objects
        .get_component::<crate::model::components::PlayerSummons>(&object_id)
        .map(|s| s.0.clone())
        .unwrap_or_default();

    // `PlayerPets` is expected to already carry the live pet's state: callers
    // run `servitor::sync_pet_row` first (it needs `&mut World` for the store
    // sweep, which this read-only builder does not have).
    let pets = world
        .objects
        .get_component::<crate::model::components::PlayerPets>(&object_id)
        .map(|p| p.0.values().cloned().collect())
        .unwrap_or_default();

    // `PlayerVariables.storeMe` — the whole map, flushed with the character.
    let variables = world
        .objects
        .get_component::<crate::model::components::PlayerVariables>(&object_id)
        .map(|v| {
            v.0.iter()
                .map(|(k, val)| (k.clone(), val.clone()))
                .collect()
        })
        .unwrap_or_default();

    let (mut skills_by_index, mut hennas_by_index, mut shortcuts_by_index, class_index) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
        .map(|p| {
            (
                p.skills_by_index.clone(),
                p.hennas_by_index.clone(),
                p.shortcuts_by_index.clone(),
                p.class_index,
            )
        })
        .unwrap_or_default();
    // Same session-only filter as the active book above, for banked subclass
    // books (a transform active at subclass-swap time would bank its skills).
    for book in skills_by_index.values_mut() {
        book.retain(|&(id, _, _)| !world.data.transforms.is_transform_skill(id));
    }
    // The banked maps are a **login-time** snapshot, refreshed only when a
    // subclass swap banks the outgoing slot (`subclass::do_switch`) — so the
    // entry for the class currently being played is stale from the moment
    // anything changes. `store_player` sweeps the child tables and then inserts
    // *both* lists, so a stale entry silently re-adds whatever the live
    // component no longer has: the cursed-weapon passive after `//cw_remove`
    // (`3629` came back with its `MaxCp` pump on the next relog), a skill lost
    // to a delevel, a cleared henna slot, a deleted shortcut. The live
    // component is authoritative for the active index — drop the banked copy
    // whenever that component is actually present (absent, the banked list is
    // all we have and must survive).
    if world.objects.has_component::<SkillBook>(&object_id) {
        skills_by_index.remove(&class_index);
    }
    if world
        .objects
        .has_component::<crate::model::components::HennaSlots>(&object_id)
    {
        hennas_by_index.remove(&class_index);
    }
    if world.objects.has_component::<Shortcuts>(&object_id) {
        shortcuts_by_index.remove(&class_index);
    }

    Some(db::PlayerSaveData {
        base,
        pets,
        summons,
        items,
        skills,
        skills_by_index,
        hennas_by_index,
        shortcuts_by_index,
        class_index,
        hennas,
        recipe_book,
        variables,
        shortcuts,
        macros,
        quests,
        skill_reuses,
        skill_buffs,
    })
}

/// Worn henna dyes → `character_hennas` rows as `(slot 1-3, dye_id)`.
fn henna_rows(henna: &crate::model::components::HennaSlots) -> Vec<(i32, i32)> {
    henna
        .0
        .iter()
        .enumerate()
        .filter_map(|(i, dye)| dye.map(|id| (i as i32 + 1, id)))
        .collect()
}

/// Skill reuse cooldowns → `character_skills_save` rows (Java `storeEffect`,
/// reuse half), gated by `StoreSkillCooltime`. `until_tick` is server-uptime
/// relative, so persist an absolute wall-clock end time that survives a
/// relog/restart; only cooldowns with time still left are written. Empty (which
/// clears the DB rows on flush) when the config is off or there's no map.
fn reuses_to_save(
    world: &World,
    reuses: Option<&crate::model::components::Reuses>,
) -> Vec<db::SkillReuseRow> {
    let Some(reuses) = reuses.filter(|_| world.cfg.character.store_skill_cooltime) else {
        return Vec::new();
    };
    let now_tick = world.tick;
    let now_ms = commons::util::now_millis();
    reuses
        .0
        .iter()
        .filter_map(|(&reuse_key, sr)| {
            let remaining_ticks = sr.until_tick.saturating_sub(now_tick);
            (remaining_ticks > 0).then_some(db::SkillReuseRow {
                reuse_key,
                skill_level: sr.skill_level,
                reuse_delay: sr.total_ms,
                systime_ms: now_ms + remaining_ticks as i64 * 100,
            })
        })
        .collect()
}

/// Active buffs → `character_skills_save` rows (Java `storeEffect`, buff half),
/// gated by `StoreSkillCooltime` like the reuse half.
///
/// Stores the **remaining seconds**, not an end instant: a buff's countdown is
/// frozen while the character is offline (Java's `restoreEffects` hands this
/// value straight to `applyEffects` as a custom `abnormalTime`), unlike a
/// cooldown, which keeps decaying. Java's skip list is reproduced here:
///
/// * dances/songs, unless `AltStoreDances` — not kept in retail;
/// * toggles (Java `isToggle() && !isNecessaryToggle()`) — modelled here as
///   buffs with no expiry, which is also what a 0-`abnormalTime` skill looks
///   like; neither should come back on its own after a relog;
/// * `LIFE_FORCE_OTHERS` — Java refuses to persist heal-over-time herbs;
/// * one row per skill id, first occurrence wins (Java dedupes on
///   `getReuseHashCode()`).
///
/// Passive stand-in entries (the grade-penalty pumps) are skipped too: they
/// carry no real buff, and enter-world re-derives them via
/// `refresh_expertise_penalty` — persisting them would double-apply the pump.
///
/// Java also skips `isDeleteAbnormalOnLeave()` skills. Not ported, and not a
/// gap: the whole datapack declares `<deleteAbnormalOnLeave>` on eight skills —
/// 8244, 6035/6036 (the TvT team transforms), 23019/23022 and 23387–23389 —
/// every one of them off-chronicle or event-only, and none reachable as a buff
/// a player could still be holding at logout. Parse the flag here if a
/// reachable carrier ever appears.
fn buffs_to_save(
    world: &World,
    buffs: Option<&crate::model::components::Buffs>,
) -> Vec<db::SkillBuffRow> {
    use crate::model::skill::BuffSlot;
    let Some(buffs) = buffs.filter(|_| world.cfg.character.store_skill_cooltime) else {
        return Vec::new();
    };
    let now_tick = world.tick;
    let mut seen = std::collections::HashSet::new();
    buffs
        .0
        .iter()
        .filter(|b| !b.passive)
        .filter(|b| b.slot != BuffSlot::Dance || world.cfg.character.alt_store_dances)
        .filter(|b| b.abnormal_type != "LIFE_FORCE_OTHERS")
        .filter_map(|b| {
            // `u64::MAX` is the no-expiry sentinel (toggle / 0-`abnormalTime`);
            // `saturating_sub` keeps it enormous, and the `> 0` seconds check
            // below can't reject it, so screen it out explicitly.
            if b.expires_at_tick == u64::MAX {
                return None;
            }
            let remaining_time_secs = (b.expires_at_tick.saturating_sub(now_tick) / 10) as i32;
            if remaining_time_secs <= 0 || !seen.insert(b.skill_id) {
                return None;
            }
            Some(db::SkillBuffRow {
                skill_id: b.skill_id,
                skill_level: b.skill_level,
                remaining_time_secs,
            })
        })
        .collect()
}

/// Flush a player who stays in the world — the periodic autosave and changes
/// that shouldn't wait for logout (class transfers).
pub(crate) fn store_player_now(world: &mut World, player_object_id: i32) {
    // Fold the live pet's state into `PlayerPets` before the snapshot, or the
    // autosave persists the row as it was at summon time and discards
    // everything the pet did this session.
    crate::game_loop::servitor::sync_pet_row(world, player_object_id);
    if let Some(save) = build_save_data(world, player_object_id) {
        let _ = world.db.send(db::DbCommand::StorePlayer { save });
    }
}

/// Server-shutdown save-all (Java `Shutdown` → `GameServer` disconnect-all →
/// `Disconnection.storeMe()` for every online player). In the memory-first model
/// all character state (level/exp/position/vitals, items, skills, shortcuts,
/// macros, quests) lives only in memory between the periodic autosave flushes,
/// so this final full flush is what keeps a restart from reverting everyone to
/// their last autosave/logout. Runs once after the game loop stops; the DB
/// thread drains these before it's told to shut down (`main` sends
/// `DbCommand::Shutdown` only after this thread joins).
pub(crate) fn save_all_players(world: &mut World) {
    let mut ids = Vec::new();
    world
        .objects
        .for_each_mut::<&crate::model::Player>(|p| ids.push(p.object_id));
    let count = ids.len();
    for oid in ids {
        store_player_now(world, oid);
    }
    if count > 0 {
        info!("GameLoop: saved {count} online player(s) on shutdown.");
    }
}

/// Port of `Player.canLogout`: refuse a restart/logout while the player is
/// fighting. Java also blocks on a pending item request, a subclass-change lock,
/// and event registration — none of those systems are ported yet, so combat
/// stance (`AttackStanceTaskManager.hasAttackStanceTask`) is the only guard.
fn can_logout(world: &World, object_id: i32) -> bool {
    !super::combat::has_attack_stance(world, object_id)
}

/// Port of `clientpackets/RequestRestart.runImpl`: save + leave the world, drop
/// the session back to the character-selection lifecycle, and re-send the
/// character list. Olympiad/instance handling doesn't apply yet.
pub(crate) fn handle_request_restart(world: &mut World, client_id: u32) {
    let Some(ClientSession::InGame(s)) = world.clients.get(&client_id) else {
        return; // Java gates by IN_GAME
    };
    // `!canLogout()` → RestartResponse.FALSE + ActionFailed, keep the player in.
    if !can_logout(world, s.player_object_id()) {
        s.send(server_packets::restart_response(false));
        s.send(server_packets::action_failed());
        return;
    }
    // Java: `if (!enteredOfflineMode(player)) { storeMe().deleteMe(); }` — a
    // player with a store open stays behind as an unattended shop. Java then
    // still writes RestartResponse/CharSelectionInfo to the now-closed client;
    // the port simply stops here, which is the same observable outcome.
    if super::offline_trade::enter_offline_mode(world, client_id) {
        return;
    }
    let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) else {
        unreachable!("checked above");
    };
    store_and_remove_player(world, s.player_object_id());
    info!(
        "GameLoop: '{}' logged out to character selection.",
        s.account()
    );

    // Java: setConnectionState(AUTHENTICATED) + RestartResponse.TRUE, then a
    // freshly restored CharSelectionInfo. The reload arrives through the normal
    // Authenticated → InLobby path (`on_characters_loaded`, send_list=true) and
    // is ordered after the StorePlayer above on the DB channel.
    let s = s.into_authenticated();
    s.send(server_packets::restart_response(true));
    let account = s.account().to_string();
    world
        .clients
        .insert(client_id, ClientSession::Authenticated(s));
    let _ = world
        .db
        .send(db::DbCommand::LoadCharacters { client_id, account });
}

/// Port of `clientpackets/Logout.runImpl`: save + leave the world, acknowledge
/// with `LeaveWorld`, and close. Valid from the lobby too (Java gates by
/// AUTHENTICATED + IN_GAME), where it just disconnects. In-game, `canLogout`
/// gates it the same way as `handle_request_restart`.
pub(crate) fn handle_logout(world: &mut World, client_id: u32) {
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => {
            // `!canLogout()` → just ActionFailed, no LeaveWorld, stay in-game.
            if !can_logout(world, s.player_object_id()) {
                s.send(server_packets::action_failed());
                return;
            }
            // Java `Logout`: a player with a store open becomes an offline
            // shop instead of leaving the world.
            if super::offline_trade::enter_offline_mode(world, client_id) {
                return;
            }
            let Some(ClientSession::InGame(s)) = world.clients.remove(&client_id) else {
                unreachable!("checked above");
            };
            // TvT: drop a logging-out participant + forfeit if a team emptied
            // (Java's `onPlayerLogout` listener). No-op off-event.
            super::events::tvt::on_player_logout(world, s.player_object_id());
            store_and_remove_player(world, s.player_object_id());
            info!("GameLoop: '{}' logged out.", s.account());
            // Dropping the session closes the socket after the queued packet
            // is flushed; the resulting `Disconnected` event runs the login
            // notify in `on_disconnect`.
            s.send(server_packets::leave_world());
        }
        Some(_) => {
            // No player: Java `client.disconnect()`.
            world.clients.remove(&client_id);
        }
        None => {}
    }
}

/// Clean up a disconnected client and inform the login server.
pub(crate) fn on_disconnect(world: &mut World, client_id: u32) {
    // Flush any item losses this player noted before the session is torn down —
    // after removal the inventory is gone and the per-tick drain would never
    // see them. The last thing someone does before vanishing is exactly what an
    // audit gets asked about.
    super::items::drain_item_audit(world);

    // Java `GameClient.onDisconnection` → the `accounting` logger. Recorded
    // first, while the account and character are still reachable through the
    // session. Ungated on purpose: unlike chat and items, Java has no config
    // switch for accounting — who connected, and when, is always kept.
    {
        let (account, char_name) = match world.clients.get(&client_id) {
            Some(ClientSession::InGame(s)) => (
                Some(s.account().to_string()),
                world
                    .objects
                    .get_component::<crate::model::Player>(&s.player_object_id())
                    .map(|p| p.name.clone()),
            ),
            Some(ClientSession::Entering(s)) => (Some(s.account().to_string()), None),
            _ => (None, None),
        };
        commons::audit::record(
            commons::audit::Category::Accounting,
            serde_json::json!({
                "event": "disconnect",
                "account": account,
                "char_name": char_name,
                "client_id": client_id,
            }),
        );
    }

    // Unexpected disconnect while a character is loaded: persist it (Java
    // `GameClient.onDisconnection` → `Disconnection.storeMe().deleteMe()`).
    // In `Entering` the Player is still held by the session, not the world.
    match world.clients.get(&client_id) {
        Some(ClientSession::InGame(s)) => {
            let oid = s.player_object_id();
            // Java `GameClient.onDisconnection`: the account logout is sent
            // either way, but a player already in offline mode is *not*
            // deleted. The session is gone before the socket event in the
            // port's own offline path, so this only guards a redundant event.
            if !super::offline_trade::is_offline_trader(world, oid) {
                // TvT: same participant-drop / forfeit on an unexpected disconnect.
                super::events::tvt::on_player_logout(world, oid);
                store_and_remove_player(world, oid);
            }
        }
        Some(ClientSession::Entering(s)) => {
            // The Player is still held by the session, not the world store, so
            // build the full save straight from the loaded `PlayerData`. It must
            // carry every child collection: `store_player` reconciles them, so an
            // items/skills-empty save here would wipe the just-loaded character.
            let b = s.player();
            let _ = world.db.send(db::DbCommand::StorePlayer {
                save: db::PlayerSaveData {
                    base: db::PlayerSnapshot::of(
                        &b.player,
                        &b.position,
                        &b.vitals,
                        &b.player_vitals,
                    ),
                    // No summon can exist before entering the world, so the
                    // rows loaded at login are still current — but they must be
                    // written back, since `store_player` reconciles.
                    pets: b.pets.0.values().cloned().collect(),
                    summons: b.summons.0.clone(),
                    items: b
                        .inventory
                        .to_rows()
                        .into_iter()
                        .chain(b.warehouse.to_rows())
                        .chain(b.freight.to_rows())
                        .chain(b.pet_inventory.to_rows())
                        .collect(),
                    skills: b
                        .skills
                        .0
                        .iter()
                        .map(|(id, lvl)| {
                            (*id, *lvl, b.skill_enchants.0.get(id).copied().unwrap_or(0))
                        })
                        .collect(),
                    skills_by_index: Default::default(),
                    hennas_by_index: Default::default(),
                    shortcuts_by_index: Default::default(),
                    class_index: 0,
                    hennas: henna_rows(&b.henna),
                    recipe_book: b
                        .recipe_book
                        .dwarven
                        .iter()
                        .map(|&id| (id, true))
                        .chain(b.recipe_book.common.iter().map(|&id| (id, false)))
                        .collect(),
                    variables: b
                        .variables
                        .0
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                    shortcuts: b.shortcuts.0.values().cloned().collect(),
                    macros: b.macros.entries.clone(),
                    quests: b.quests.0.clone(),
                    skill_reuses: reuses_to_save(world, Some(&b.reuses)),
                    // This character never spawned, so its buffs were never
                    // applied — they're still the untouched rows the select path
                    // loaded. Write them straight back: running them through
                    // `buffs_to_save` (which reads the empty live `Buffs`
                    // component) would silently drop every buff of anyone who
                    // disconnects between char-select and enter-world.
                    skill_buffs: b.pending_buffs.clone(),
                },
            });
        }
        _ => {}
    }
    world.clients.remove(&client_id);
    world.hwids.remove(&client_id); // Java `GameClient` hardware info dies with the connection (G31).
    world.protocol_versions.remove(&client_id); // same lifetime — it is the connection's, not the character's.
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
            .send(LoginLinkCommand::PlayerLogout { account });
    }
    debug!(
        "GameLoop: client {client_id} disconnected ({} online).",
        world.clients.len()
    );
}

/// One login-link event: registration, auth response, kick, char-count ask.
pub(crate) fn handle_login_link_event(world: &mut World, event: LoginLinkEvent) {
    match event {
        LoginLinkEvent::Registered {
            server_id,
            server_name,
        } => {
            info!("GameLoop: registered as Server {server_id}: {server_name}.");
            world.login.server_id = Some(server_id);
            world.login.server_name = Some(server_name);
        }
        LoginLinkEvent::PlayerAuthResponse { account, authed } => {
            handle_player_auth_response(world, account, authed);
        }
        LoginLinkEvent::KickPlayer { account } => handle_kick(world, account),
        LoginLinkEvent::RequestCharacters { account } => {
            // Ask the DB thread; reply on the CharCount event.
            let _ = world.db.send(db::DbCommand::CountCharacters { account });
        }
        LoginLinkEvent::Failed { reason } => {
            warn!("GameLoop: login-server registration failed (reason {reason}).");
        }
    }
}

/// Port of the `PlayerAuthResponse` (0x03) branch of `LoginServerThread.run`.
pub(crate) fn handle_player_auth_response(world: &mut World, account: String, authed: bool) {
    let Some(waiting) = world.login.waiting.remove(&account) else {
        return;
    };
    let client_id = waiting.client_id;
    if authed {
        let _ = world.login.link.send(LoginLinkCommand::PlayerInGame {
            accounts: vec![account.clone()],
        });
        if let Some(ClientSession::Connecting(s)) = world.clients.remove(&client_id) {
            let s = s.into_authenticated(account.clone(), waiting.session_key);
            s.send(server_packets::login_success());
            info!(
                "GameLoop: client {} authenticated as '{}'.",
                s.client_id,
                s.account()
            );
            world
                .clients
                .insert(client_id, ClientSession::Authenticated(s));
            // Load the character list; CharSelectionInfo is sent on the result.
            let _ = world
                .db
                .send(db::DbCommand::LoadCharacters { client_id, account });
        }
    } else {
        warn!("GameLoop: session key incorrect, closing connection for account {account}.");
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::login_fail(0, 1)); // SYSTEM_ERROR_LOGIN_LATER
        }
        world.login.accounts_in_gameserver.remove(&account);
        world.clients.remove(&client_id); // disconnect after the queued packet
        let _ = world
            .login
            .link
            .send(LoginLinkCommand::PlayerLogout { account });
    }
}

/// One DB result: a boot-load table landing, or a mid-session read's
/// continuation (character list, name check, id block…).
pub(crate) fn handle_db_event(world: &mut World, event: DbEvent) {
    match event {
        DbEvent::CharactersLoaded {
            client_id,
            account,
            chars,
            send_list,
        } => {
            on_characters_loaded(world, client_id, account, chars, send_list);
        }
        DbEvent::CharacterCreated { client_id, result } => {
            use crate::db::CreateResult::*;
            let body = match result {
                Ok => server_packets::char_create_ok(),
                // NAME_ALREADY_EXISTS=2, TOO_MANY=1, CREATION_FAILED=0.
                NameExists => server_packets::char_create_fail(2),
                TooMany => server_packets::char_create_fail(1),
                Fail => server_packets::char_create_fail(0),
            };
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(body);
            }
        }
        DbEvent::CharCount {
            account,
            count,
            del_times,
        } => {
            let _ = world.login.link.send(LoginLinkCommand::ReplyCharacters {
                account,
                chars: count,
                del_times,
            });
        }
        DbEvent::NameCreatable { client_id, result } => {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_is_char_name_creatable(result));
            }
        }
        DbEvent::IdBlock { start, end } => {
            world.id_pool = start..end;
        }
        DbEvent::GlobalVariablesLoaded { entries } => {
            tracing::info!("GameLoop: loaded {} global variables.", entries.len());
            world.global_vars = entries.into_iter().collect();
            super::four_sepulchers::restore_entry_times(world);
            // Re-derive upgraded castle-door HP now that the ratios are known
            // (the doors spawned before this table landed) — Java's
            // `loadDoorUpgrade` at castle load.
            super::castle::apply_door_upgrades_at_boot(world);
        }
        DbEvent::PremiumLoaded { entries } => {
            tracing::info!("GameLoop: loaded {} premium accounts.", entries.len());
            world.premium = entries.into_iter().collect();
        }
        DbEvent::LotteryLoaded { row, draws } => {
            super::lottery::on_loaded(world, row, draws);
        }
        DbEvent::LotteryTicketsLoaded { round, rows } => {
            super::lottery::finish_complete(world, round, rows);
        }
        DbEvent::MdtLoaded { history, bets } => {
            super::monster_race::on_mdt_loaded(world, history, bets);
        }
        DbEvent::MailLoaded {
            messages,
            attachments,
            char_ids_by_name,
        } => {
            super::mail::on_loaded(world, messages, attachments, char_ids_by_name);
        }
        DbEvent::ItemAuctionsLoaded {
            next_auction_id,
            auctions,
        } => {
            super::item_auction::on_loaded(world, next_auction_id, auctions);
        }
        DbEvent::PunishmentsLoaded {
            next_id,
            punishments,
        } => {
            super::punishment::on_loaded(world, next_id, punishments);
        }
        DbEvent::BotReportsLoaded { rows } => {
            let last_reset = super::bot_report::last_reset_millis(
                &world.cfg.bot_report,
                commons::util::now_millis(),
            );
            super::bot_report::on_loaded(world, rows, last_reset);
        }
        DbEvent::BufferSchemesLoaded { entries } => {
            // Java `SchemeBufferTable.load` drops any saved skill id no longer
            // in `_availableBuffs`; the buffer table lives here on the game
            // thread, so the filter runs at insert time (like grand bosses).
            for (object_id, scheme_name, skills) in entries {
                let skills: Vec<i32> = skills
                    .into_iter()
                    .filter(|id| world.data.scheme_buffer.contains(*id))
                    .collect();
                world
                    .buffer_schemes
                    .entry(object_id)
                    .or_default()
                    .push((scheme_name, skills));
            }
            tracing::info!(
                "GameLoop: loaded buffer schemes for {} characters.",
                world.buffer_schemes.len()
            );
        }
        DbEvent::FavoritesLoaded { entries } => {
            // `favId` is a table-wide AUTOINCREMENT PK; seed the game-thread
            // allocator past the highest loaded id so new favorites stay unique.
            let mut max_id = 0;
            for (player_id, fav_id, title, bypass, add_date) in entries {
                max_id = max_id.max(fav_id);
                world
                    .bbs_favorites
                    .entry(player_id)
                    .or_default()
                    .push(crate::world::Favorite {
                        fav_id,
                        title,
                        bypass,
                        add_date,
                    });
            }
            world.next_fav_id = max_id + 1;
            tracing::info!(
                "GameLoop: loaded favorites for {} characters.",
                world.bbs_favorites.len()
            );
        }
        DbEvent::NpcRespawnsLoaded { rows } => {
            // Settle the `dbSave` spawns the static pass deferred (Java's
            // `DBSpawnManager.load` + the `spawnNpc` hand-off).
            super::boss_respawn::resolve_boot(world, rows);
        }
        DbEvent::OfflineTradersLoaded { traders } => {
            // `GameServer.main`'s `OfflineTraderTable.restoreOfflineTraders()`.
            super::offline_trade::restore_offline_traders(world, traders);
        }
        DbEvent::GrandBossesLoaded { bosses } => {
            // Java skips rows whose NPC template is missing (`NpcData
            // .getTemplate(bossId) != null`); the datapack lives here on the
            // game thread, so the filter runs at insert time.
            world.grand_bosses = bosses
                .into_iter()
                .filter(|b| world.data.npc_data.get(b.boss_id).is_some())
                .map(|b| (b.boss_id, b))
                .collect();
            tracing::info!(
                "GameLoop: loaded {} grand bosses.",
                world.grand_bosses.len()
            );
            // Spawn the ones that are up, arm timers for the rest, and
            // immediately respawn any whose window elapsed while the server
            // was down. Must run *here*, once the data has landed — the
            // static world (`spawn_all`, geo) is already up before the loop.
            super::grand_boss::resolve_at_boot(world);
            super::dr_chaos::resolve_at_boot(world);
        }
        DbEvent::CursedWeaponsLoaded { rows } => {
            // Build from the XML config, compute each skill's max level, then
            // overlay the persisted wielder state (Java `restore` →
            // `reActivate`). The default table is empty, so both usually
            // start inactive.
            let mut weapons = world.data.cursed_weapons.weapons.clone();
            for cw in &mut weapons {
                cw.skill_max_level = (1..=100)
                    .take_while(|l| world.data.skill_data.get(cw.skill_id, *l).is_some())
                    .last()
                    .unwrap_or(1);
                if let Some(row) = rows.iter().find(|r| r.item_id == cw.item_id) {
                    cw.player_id = row.char_id;
                    cw.player_reputation = row.player_reputation;
                    cw.player_pk_kills = row.player_pk_kills;
                    cw.nb_kills = row.nb_kills;
                    cw.end_time = row.end_time;
                    cw.is_activated = true;
                }
            }
            tracing::info!("GameLoop: loaded {} cursed weapons.", weapons.len());
            world.cursed_weapons = weapons;
            // Java `restore()` → `reActivate()`: a weapon that survived a
            // restart gets its `RemoveTask` armed again. Without this the
            // restored curse is immortal — the wielder keeps it forever,
            // since only this timer ever calls `endOfLife`. One whose
            // deadline passed while the server was down fires immediately
            // (`arm_expiry` clamps the delay at 0, `handle_expiry`
            // re-checks `end_time`).
            for idx in 0..world.cursed_weapons.len() {
                if world.cursed_weapons[idx].is_activated {
                    super::cursed_weapon::arm_expiry(world, idx);
                }
            }
        }
        DbEvent::CastlesLoaded { castles } => {
            tracing::info!("GameLoop: loaded {} castles.", castles.len());
            world.castles = castles;
        }
        DbEvent::SiegesLoaded { rows } => {
            // One Siege per castle (Java creates a Siege for every castle),
            // then attach the registered clans from `siege_clans`.
            use crate::model::siege::{Siege, SiegeClanType};
            let mut sieges: std::collections::HashMap<i32, Siege> = world
                .castles
                .iter()
                .map(|c| (c.id, Siege::new(c.id)))
                .collect();
            for row in &rows {
                if let (Some(siege), Some(kind)) = (
                    sieges.get_mut(&row.castle_id),
                    SiegeClanType::from_db(row.kind),
                ) {
                    siege.add_clan(row.clan_id, kind);
                }
            }
            tracing::info!(
                "GameLoop: loaded sieges for {} castles ({} registered clans).",
                sieges.len(),
                rows.len()
            );
            world.sieges = sieges;
            // The per-castle Siege records now exist — arm the weekly
            // auto-start schedule (`SiegeSchedule.xml`).
            crate::game_loop::siege::schedule_all_at_boot(world);
        }
        DbEvent::ManorLoaded {
            production,
            procure,
        } => {
            // Group the rows by castle + period, dropping ids not in the
            // seed catalogue (Java's "Don't load unknown seeds/crops").
            use crate::model::manor::{CropProcure, ManorState, SeedProduction};
            let mut manor = ManorState::default();
            let mut prod: std::collections::HashMap<(i32, bool), Vec<SeedProduction>> =
                std::collections::HashMap::new();
            let mut proc: std::collections::HashMap<(i32, bool), Vec<CropProcure>> =
                std::collections::HashMap::new();
            let mut skipped = 0;
            for r in &production {
                if world.data.manor.seed_by_id(r.seed_id).is_none() {
                    skipped += 1;
                    continue;
                }
                prod.entry((r.castle_id, r.next_period))
                    .or_default()
                    .push(SeedProduction {
                        seed_id: r.seed_id,
                        amount: r.amount,
                        price: r.price,
                        start_amount: r.start_amount,
                    });
            }
            for r in &procure {
                if world.data.manor.seed_by_crop(r.crop_id).is_none() {
                    skipped += 1;
                    continue;
                }
                proc.entry((r.castle_id, r.next_period))
                    .or_default()
                    .push(CropProcure {
                        crop_id: r.crop_id,
                        amount: r.amount,
                        price: r.price,
                        start_amount: r.start_amount,
                        reward_type: r.reward_type,
                    });
            }
            for ((castle_id, next), list) in prod {
                manor.set_seed_production(castle_id, next, list);
            }
            for ((castle_id, next), list) in proc {
                manor.set_crop_procure(castle_id, next, list);
            }
            tracing::info!(
                "GameLoop: loaded manor state ({} production, {} procure rows, {skipped} unknown skipped).",
                production.len(),
                procure.len()
            );
            world.manor = manor;
            // Set the initial period mode from the wall clock and arm the
            // first daily mode change (Java `CastleManorManager` init).
            crate::game_loop::manor::schedule_manor_at_boot(world);
        }
        DbEvent::ClanHallsLoaded { rows } => {
            // Start from the static defs, then overlay persisted ownership.
            let mut halls = world.data.clan_halls.clone();
            for row in &rows {
                if let Some(hall) = halls.get_mut(&row.id) {
                    hall.owner_id = row.owner_id;
                    hall.paid_until = row.paid_until;
                }
            }
            let owned: Vec<i32> = halls
                .values()
                .filter(|h| h.owner_id != 0)
                .map(|h| h.id)
                .collect();
            tracing::info!(
                "GameLoop: loaded {} clan halls ({} owned).",
                halls.len(),
                owned.len()
            );
            world.clan_halls = halls;
            // Java `ClanHall.setOwner` on load arms each owned hall's lease
            // check; restore those timers here.
            for hall_id in owned {
                crate::game_loop::clan_hall_auction::arm_lease_check(world, hall_id);
            }
        }
        DbEvent::ClanHallBiddersLoaded { rows } => {
            use crate::model::clan_hall::ClanHallBid;
            for row in &rows {
                world.clan_hall_bids.entry(row.hall_id).or_default().insert(
                    row.clan_id,
                    ClanHallBid {
                        amount: row.bid,
                        bid_time: row.bid_time,
                    },
                );
            }
            tracing::info!("GameLoop: loaded {} clan-hall auction bids.", rows.len());
            // Arm the weekly auction close now that the bids exist.
            crate::game_loop::clan_hall_auction::schedule_weekly_close(world);
        }
        DbEvent::ResidenceFunctionsLoaded { rows } => {
            use crate::model::clan_hall::ActiveFunction;
            for row in &rows {
                world
                    .clan_hall_functions
                    .entry(row.residence_id)
                    .or_default()
                    .insert(
                        row.func_id,
                        ActiveFunction {
                            level: row.level,
                            expiration: row.expiration,
                        },
                    );
            }
            tracing::info!("GameLoop: loaded {} clan-hall functions.", rows.len());
            // Re-arm each function's expiry (Java `ResidenceFunction.init`).
            let funcs: Vec<(i32, i32)> = world
                .clan_hall_functions
                .iter()
                .flat_map(|(&hall, fs)| fs.keys().map(move |&f| (hall, f)))
                .collect();
            for (hall_id, func_id) in funcs {
                crate::game_loop::clan_hall_function::arm_function_expiry(world, hall_id, func_id);
            }
        }
        DbEvent::CustomMailLoaded { rows } => {
            super::custom_mail::apply_loaded(world, rows);
        }
        DbEvent::OlympiadLoaded {
            current_cycle,
            period,
            olympiad_end,
            validation_end,
            next_weekly_change,
            nobles,
            eom,
        } => {
            crate::game_loop::olympiad::apply_loaded(
                world,
                current_cycle,
                period,
                olympiad_end,
                validation_end,
                next_weekly_change,
                nobles,
                eom,
            );
            // `Olympiad.init` + `scheduleWeeklyChange`: arm the window and
            // weekly-refresh schedules now the persisted state is in place.
            crate::game_loop::olympiad::schedule_at_boot(world);
        }
        DbEvent::HeroesLoaded { heroes, diary } => {
            crate::game_loop::olympiad::apply_heroes_loaded(world, heroes, diary);
        }
        DbEvent::SiegeGuardsLoaded { guards } => {
            let mut by_castle: std::collections::HashMap<
                i32,
                Vec<crate::model::siege::SiegeSpawn>,
            > = std::collections::HashMap::new();
            for (castle_id, spawn) in guards {
                by_castle.entry(castle_id).or_default().push(spawn);
            }
            let total: usize = by_castle.values().map(|v| v.len()).sum();
            tracing::info!(
                "GameLoop: loaded {total} siege guards for {} castles.",
                by_castle.len()
            );
            world.siege_guards = by_castle;
        }
        DbEvent::ClansLoaded {
            clans,
            wars,
            crests,
            recruit_clans,
            recruit_waiting,
            recruit_applicants,
            notices,
        } => {
            world.clan_notices = notices
                .into_iter()
                .map(|(id, enabled, text)| (id, (enabled, text)))
                .collect();
            tracing::info!(
                "GameLoop: loaded {} clans, {} clan wars, {} crests, {} recruiting clans, \
                 {} waiting players, {} applications.",
                clans.len(),
                wars.len(),
                crests.len(),
                recruit_clans.len(),
                recruit_waiting.len(),
                recruit_applicants.iter().len()
            );
            world.clans = clans.into_iter().map(|c| (c.id, c)).collect();
            world.clan_wars = wars;
            world.next_crest_id = crests.iter().map(|c| c.id + 1).max().unwrap_or(1);
            world.crests = crests.into_iter().map(|c| (c.id, c)).collect();
            // `ClanEntryManager.load`: drop recruiting entries for clans
            // that no longer exist.
            world.recruit_clans = recruit_clans
                .into_iter()
                .filter(|r| world.clans.contains_key(&r.clan_id))
                .map(|r| (r.clan_id, r))
                .collect();
            world.recruit_waiting = recruit_waiting
                .into_iter()
                .map(|w| (w.player_id, w))
                .collect();
            for a in recruit_applicants {
                world
                    .recruit_applicants
                    .entry(a.clan_id)
                    .or_default()
                    .insert(a.player_id, a);
            }
            super::clans::rearm_clan_wars_at_boot(world);
            // Re-arm pending dissolutions (Java `ClanTable`'s constructor:
            // past-due stamps fire immediately).
            let pending: Vec<(i32, i64)> = world
                .clans
                .values()
                .filter(|c| c.dissolving_expiry_time > 0)
                .map(|c| (c.id, c.dissolving_expiry_time))
                .collect();
            for (clan_id, due) in pending {
                super::clans::schedule_clan_dissolve(world, clan_id, due);
            }
            // Clans are the last boot-load data (static datapack already
            // loaded synchronously at startup); release the login-link task
            // to connect now that the world is fully populated.
            if let Some(ready) = world.login.ready.take() {
                let _ = ready.send(());
            }
        }
    }
}

/// A character list came back from the DB. Always cache it on the session (for
/// slot → object-id mapping); send `CharSelectionInfo` only when `send_list`
/// (login/delete/restore) — after creation Java caches without re-sending.
/// Transitions `Authenticated` → `InLobby` on the first load.
pub(crate) fn on_characters_loaded(
    world: &mut World,
    client_id: u32,
    account: String,
    chars: Vec<crate::character::CharData>,
    send_list: bool,
) {
    // Top up the `CharInfoTable` equivalent (G30). This is also the create
    // path: the client reloads its character list right after a successful
    // creation, so a brand-new character becomes mailable here without the
    // create command having to round-trip its freshly assigned id back.
    for c in &chars {
        super::mail::on_character_created(world, &c.name, c.object_id);
    }
    // Java `CharSelectionInfo.loadCharacterSelectInfo`'s
    // `OFFLINE_DISCONNECT_SAME_ACCOUNT` branch: seeing the list for an account
    // evicts that account's unattended shops. Off on this dist.
    let ids: Vec<i32> = chars.iter().map(|c| c.object_id).collect();
    super::offline_trade::on_character_list(world, &ids);
    let s = match world.clients.remove(&client_id) {
        Some(ClientSession::Authenticated(s)) => s.into_lobby(chars),
        Some(ClientSession::InLobby(mut s)) => {
            s.set_chars(chars);
            s
        }
        other => {
            // Client vanished mid-load; put back whatever was there.
            if let Some(cs) = other {
                world.clients.insert(client_id, cs);
            }
            return;
        }
    };
    if send_list {
        let body = server_packets::char_selection_info(
            &account,
            s.play_ok1(),
            &s.state.chars,
            -1,
            world.max_characters_per_account,
            &world.data.experience,
            &world.cursed_weapons,
        );
        s.send(body);
    }
    world.clients.insert(client_id, ClientSession::InLobby(s));
}

/// Port of `doKickPlayer`: disconnect the account's client and notify login.
pub(crate) fn handle_kick(world: &mut World, account: String) {
    if let Some(&client_id) = world.login.accounts_in_gameserver.get(&account) {
        world.clients.remove(&client_id); // disconnect
    }
    world.login.accounts_in_gameserver.remove(&account);
    world.login.waiting.remove(&account);
    let _ = world
        .login
        .link
        .send(LoginLinkCommand::PlayerLogout { account });
}
