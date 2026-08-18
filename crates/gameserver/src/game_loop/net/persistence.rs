//! Player persistence: the World → PlayerSaveData serialization, immediate
//! and shutdown saves, and the store-then-remove leave path.

use super::*;
/// Take the player out of the world and persist them — Java
/// `Disconnection.storeMe().deleteMe()`. Shared by restart, logout, and
/// unexpected disconnects. Scheduled tasks holding the dead object id no-op.
pub(crate) fn store_and_remove_player(world: &mut World, player_object_id: i32) {
    // `Instance.onPlayerLogout`: with `RestorePlayerInstance` on, remember the
    // instance in a player variable so the next login puts them back in it;
    // with it off, Java moves them to the instance's exit location instead so
    // they do not wake up inside a world that no longer exists.
    crate::game_loop::instances::on_player_logout(world, player_object_id);
    // deleteMe → leaveParty (DISCONNECTED semantics: leadership transfers)
    // + pending party/friend request cleanup on both sides.
    crate::game_loop::party::on_player_leave_world(world, player_object_id);
    crate::game_loop::party_room::on_player_leave_world(world, player_object_id);
    // deleteMe → notifyFriends(MODE_OFFLINE).
    crate::game_loop::friends::on_leave_world(world, player_object_id);
    // The `Item._published` flags of this player's items die with the `Item`
    // instances, so their chat links stop resolving (Java: the objects leave
    // the world with them).
    crate::game_loop::chat::on_player_leave_world(world, player_object_id);
    // A servitor does not outlive its owner's session. Java stores it in
    // `CharSummonTable` for `RestoreServitorOnReconnect`; persistence is a
    // later slice, so for now it simply goes away with them — which is at
    // least better than leaking an ownerless NPC into the world.
    crate::game_loop::servitor::on_owner_leave_world(world, player_object_id);
    // Cubics do not outlive their owner; nothing persists them.
    crate::game_loop::cubic::on_owner_leave_world(world, player_object_id);
    // deleteMe → clan.broadcastToOnlineMembers(PledgeShowMemberListUpdate offline).
    {
        let clan_id = clan_of_or_zero(world, player_object_id);
        crate::game_loop::clans::on_leave_world(world, player_object_id, clan_id);
    }
    // deleteMe → World.removeVisibleObject: DeleteObject to everyone watching.
    crate::game_loop::visibility::on_leave_world(world, player_object_id);
    // `.apon` does not survive a logout — Java's task manager holds `Player`
    // references and drops anyone offline on its next sweep.
    crate::game_loop::auto_potions::remove(world, player_object_id);
    crate::game_loop::auto_play::remove(world, player_object_id);
    // A buff shop dies with its seller — the flag also gates `canOpenPrivateStore`,
    // so a stale one would follow the character into their next session.
    crate::game_loop::sell_buffs::clear(world, player_object_id);
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
    crate::game_loop::item_mana::on_player_leave_world(world, player_object_id);
    // Gather everything persistence needs before despawn — components drop
    // with the entity (PLAN_ECS_STAGE2 §7 risk 3).
    if let Some(save) = build_save_data(world, player_object_id) {
        // Index entry goes just before the despawn, while the `RegionCell` is
        // still there to locate it — and only on the branch that actually
        // despawns, so a player left in the world keeps receiving broadcasts.
        world.unindex_player(player_object_id);
        world.objects.despawn(&player_object_id);
        let _ = world.db.send(db::DbCommand::StorePlayer {
            save: Box::new(save),
        });
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
                .filter(|(id, _)| {
                    !world.data.transforms.is_transform_skill(**id)
                        && !world.data.armor_sets.is_armor_set_skill(**id)
                })
                // The GM convenience kits are the same shape: granted at
                // enter-world with Java's `addSkill(skill, false)`, so they
                // must not survive as learned rows — otherwise turning
                // `GMGiveSpecialSkills` back off leaves every GM who ever
                // logged in still holding Super Haste.
                .filter(|(id, _)| !world.data.skill_trees.is_gm_skill(**id))
                // Hero and noble skills are the same shape again, and Java
                // says so in a comment at the grant site ("Don't persist hero
                // skills into database"). Both are re-derived on the way in —
                // hero from the `heroes` table at enter-world, nobless from the
                // `nobless` column in `from_char` — so a row here would
                // outlive the status that justified it.
                .filter(|(id, _)| !world.data.skill_trees.is_hero_or_noble_skill(**id))
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
        book.retain(|&(id, _, _)| {
            !world.data.transforms.is_transform_skill(id)
                && !world.data.armor_sets.is_armor_set_skill(id)
        });
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
        // `Config.UPDATE_ITEMS_ON_CHAR_STORE` — Java's `autoSave` gate on
        // `getInventory()/getWarehouse()/getFreight().updateDatabase()`.
        store_items: world.cfg.general.update_items_on_char_store,
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
pub(super) fn henna_rows(henna: &crate::model::components::HennaSlots) -> Vec<(i32, i32)> {
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
pub(super) fn reuses_to_save(
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
        let _ = world.db.send(db::DbCommand::StorePlayer {
            save: Box::new(save),
        });
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

/// Staggered periodic player flush — the port of `PlayerAutoSaveTaskManager.run`
/// and the timer half of the memory-first model. Flushes **at most one** due
/// player per sweep (Java's `break; // Prevent SQL flood`) and reschedules it
/// one `CharacterDataStoreInterval` out. Because gameplay only mutates in-memory
/// components, this — together with the logout and shutdown flushes — is the
/// sole writer of character state, so no packet flood can become a DB flood.
pub(crate) fn autosave_tick(world: &mut World) {
    let interval = world.cfg.character.character_data_store_interval_ticks;
    // The single due player this sweep (lowest object id = deterministic).
    let due = world
        .player_autosave_due
        .iter()
        .filter(|&(_, &due)| world.tick >= due)
        .map(|(&oid, _)| oid)
        .min();
    if let Some(oid) = due {
        world.player_autosave_due.insert(oid, world.tick + interval);
        store_player_now(world, oid);
    }
}
