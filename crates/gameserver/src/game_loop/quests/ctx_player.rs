//! `QuestCtx` player-side predicates and state: clan/race/class checks,
//! player variables, memo state, radar and teleports.

use super::*;

impl<'w> QuestCtx<'w> {
    /// Java `isOwningClan` — the player's clan is `owner_id`.
    ///
    /// `owner_id == 0` means *unowned*, and nobody's clan owns an unowned
    /// residence, so that case is `false` before the player is even looked at.
    pub fn is_owning_clan(&self, owner_id: i32) -> bool {
        owner_id != 0
            && self
                .world
                .objects
                .get_component::<crate::model::Player>(&self.player)
                .is_some_and(|p| p.clan_id == owner_id)
    }

    /// Java `player.hasClanPrivilege(...)`: the leader holds every privilege,
    /// otherwise the member's rank privilege mask must carry the bit.
    ///
    /// `false` for a clanless player, and for one whose clan id points at
    /// nothing — the residence scripts gate every paid action on this, so an
    /// unresolvable clan must not read as "allowed".
    pub fn has_clan_privilege(&self, privilege: i32) -> bool {
        let Some(p) = self
            .world
            .objects
            .get_component::<crate::model::Player>(&self.player)
        else {
            return false;
        };
        self.world
            .clans
            .get(&p.clan_id)
            .is_some_and(|c| c.has_privilege(self.player, p.clan_privs, privilege))
    }

    /// `npc.getLevel()` — the in-context NPC's template level (regular mobs do
    /// not level up, so the template value is authoritative). 0 when unknown.
    pub fn npc_level(&self) -> i32 {
        self.world
            .data
            .npc_data
            .get(self.npc_id)
            .map(|t| t.level)
            .unwrap_or(0)
    }

    /// The `Race` ordinal (`characters.race` — 0 Human, 1 Elf, 2 Dark Elf,
    /// 3 Orc, 4 Dwarf).
    pub fn player_race(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .map(|p| p.race)
            .unwrap_or(0)
    }

    /// `Npc.getRace()` — the talked-to NPC's `<race>` as the same ordinal
    /// [`player_race`](QuestCtx::player_race) returns, `None` when the
    /// template declares a non-player race (or none).
    pub fn npc_race(&self) -> Option<i32> {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .and_then(|n| n.template(self.world))
            .and_then(|t| t.race)
    }

    /// `AbstractScript.addRadar` → `Radar.addMarker` — drop a radar marker on
    /// the player's map. Type 1 is the plain red flag the "find an NPC"
    /// services use; for a quest objective prefer [`add_quest_radar`], which
    /// the client draws as the quest pin.
    ///
    /// Java sends **two** packets here, not one: `RadarControl(2, 2, x, y, z)`
    /// clears any marker already standing at that spot, then
    /// `RadarControl(0, 1, x, y, z)` shows the new one. Dropping the leading
    /// clear — as this helper did until 2026-08-05 — leaves the client
    /// stacking duplicate flags when the same location is re-pinged, which is
    /// exactly what the "find an NPC" services and Q255's tutorial do on every
    /// repeat ask. `community_board::npc_trace` already sent the pair, so the
    /// two radar paths in this port disagreed with each other.
    ///
    /// [`add_quest_radar`]: QuestCtx::add_quest_radar
    pub fn add_radar(&mut self, x: i32, y: i32, z: i32) {
        let clear = server_packets::radar_control(2, 2, x, y, z);
        self.send(clear);
        let pkt = server_packets::radar_control(0, 1, x, y, z);
        self.send(pkt);
    }

    /// `player.sendPacket(new ExShowScreenMessage(npcString, position, time))`
    /// — an on-screen banner whose text is a client-side string id.
    ///
    /// Simulated probes are suppressed, as for every other send here.
    pub fn send_screen_message_npc_string(&self, npc_string_id: i32, position: i32, time: i32) {
        self.send(server_packets::ex_show_screen_message_npc_string(
            npc_string_id,
            position,
            time,
            &[],
        ));
    }

    /// `player.sendPacket(SystemMessageId.X)` — a parameterless system message.
    ///
    /// Prefer this over reaching into `world.clients` from a script: it routes
    /// through [`QuestCtx::send`], which suppresses output during a simulated
    /// probe exactly as Java's `isSimulatingTalking()` guards do. A direct
    /// client send skips that and leaks packets to a player who is only being
    /// *asked* whether a dialogue would proceed.
    pub fn send_sm(&self, message_id: i16) {
        self.send(server_packets::system_message_with(message_id, &[]));
    }

    /// `RadarControl(0, 2, x, y, z)` — the *quest* marker, as Q211 sends it
    /// raw in Java. Same packet as [`add_radar`] but radar type 2, which the
    /// client renders as the quest pin rather than the red flag.
    ///
    /// [`add_radar`]: QuestCtx::add_radar
    pub fn add_quest_radar(&mut self, x: i32, y: i32, z: i32) {
        let pkt = server_packets::radar_control(0, 2, x, y, z);
        self.send(pkt);
    }

    /// `RadarControl(2, 2, 0, 0, 0)` — drop every marker on the player's map.
    /// This is how Q348 retires its marker once the objective is reached; the
    /// client has no "remove this one type-2 marker" form, so reaching an
    /// objective clears the board.
    pub fn clear_radar(&mut self) {
        let pkt = server_packets::radar_control(2, 2, 0, 0, 0);
        self.send(pkt);
    }

    /// `SpawnTable.getAnySpawn(npcId)` — the spawn point of any live instance
    /// of `npc_id` (the `spawn_loc` anchor, not its wandered-to position, so
    /// the marker matches Java's `Spawn.getX/Y/Z`). Java reads its spawn
    /// *table* — every registered point, spawned or not; the Rust world holds
    /// spawned objects, so this scans those. The two agree for the
    /// always-spawned town NPCs this serves; a despawned NPC yields `None`
    /// where Java would still answer.
    pub fn any_spawn_location(&mut self, npc_id: i32) -> Option<(i32, i32, i32)> {
        let oid = *self.world.npcs_with_id(npc_id).first()?;
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .map(|npc| npc.spawn_loc)
    }

    /// `Player.getClan() != null` (AllianceMaster's clan gate). Clan id 0 is
    /// the no-clan sentinel.
    pub fn has_clan(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.clan_id != 0)
    }

    /// `Player.isClanLeader` (ClanMaster's LEADER_REQUIRED gate).
    pub fn is_clan_leader(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.clan_leader)
    }

    pub fn player_class_id(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .map(|p| p.class_id)
            .unwrap_or(-1)
    }

    /// Java `Player.isSubClassActive()` — true while a subclass slot is the
    /// active one (G17). Several village-master scripts refuse to talk at all
    /// in that state.
    pub fn is_subclass_active(&self) -> bool {
        self.world
            .objects
            .get_component::<crate::model::Player>(&self.player)
            .is_some_and(|p| p.class_index != 0)
    }

    /// `Player.isInCategory(CategoryType.X)` against `CategoryData.xml`.
    pub fn is_in_category(&self, category: &str) -> bool {
        self.world
            .data
            .categories
            .contains(category, self.player_class_id())
    }

    /// The village-master class transfer — routed through the G17 mechanic
    /// (`game_loop::subclass::set_class_id`), so it moves the *active* slot:
    /// the base class only when the player is on it. Persisted immediately
    /// through the regular `StorePlayer` snapshot.
    pub fn set_class_id(&mut self, class_id: i32) {
        if self.simulated {
            return;
        }
        // Was an unconditional `base_class_id = class_id`, which since G17
        // would rewrite the character's *base* class if a quest transfer ran
        // while a subclass was active. The shared mechanic moves the active
        // slot only, and also does `rewardSkills` + the stat/UserInfo refresh.
        crate::game_loop::subclass::set_class_id(self.world, self.player, class_id);
        crate::game_loop::net::store_player_now(self.world, self.player);
    }

    /// `player.teleToLocation(loc)` (TeleportWithCharm and friends).
    pub fn teleport_to(&mut self, x: i32, y: i32, z: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::death::teleport_player(self.world, self.player, x, y, z);
    }

    /// `player.getVariables().getInt(key, default)` — the *character*
    /// key/value store (`character_variables`), not the per-quest
    /// `QuestState` vars: it outlives the script's quest state and is what
    /// the `ai/others` behaviors use to remember something about a player
    /// (TeleportToRaceTrack's return point).
    pub fn player_var_int(&self, key: &str, default: i32) -> i32 {
        crate::game_loop::helpers::player_var_int(self.world, self.player, key, default)
    }

    /// `player.getVariables().getString(key, null)` — the raw value, so a
    /// caller can tell **absent** from a stored zero.
    ///
    /// [`player_var_int`] cannot: it folds both into its default. Java leans on
    /// the difference wherever a variable's *first* write is special —
    /// `giveNewbieReward` seeds `GUIDE_MISSION` to 100000 when unset but adds a
    /// digit when it exists, and those two branches disagree for a stored 0.
    ///
    /// [`player_var_int`]: QuestCtx::player_var_int
    pub fn player_var(&self, key: &str) -> Option<String> {
        crate::game_loop::helpers::player_var(self.world, self.player, key).map(str::to_string)
    }

    /// `player.getVariables().set(key, value)` (memory-first — flushed with
    /// the character like every other persisted field).
    pub fn set_player_var_int(&mut self, key: &str, value: i32) {
        if self.simulated {
            return;
        }
        crate::game_loop::helpers::set_player_var_int(self.world, self.player, key, value);
    }

    /// `player.getVariables().remove(key)`.
    pub fn unset_player_var(&mut self, key: &str) {
        if self.simulated {
            return;
        }
        crate::game_loop::helpers::unset_player_var(self.world, self.player, key);
    }

    /// The involved NPC's per-instance scratch value (Java
    /// `Npc.isScriptValue`/`setScriptValue` — reset on respawn because the
    /// respawned NPC is a fresh instance).
    pub fn npc_script_value(&self) -> i32 {
        self.world
            .objects
            .get_component::<crate::model::npc::Npc>(&self.npc)
            .map(|n| n.script_value)
            .unwrap_or(0)
    }

    pub fn set_npc_script_value(&mut self, value: i32) {
        if self.simulated {
            return;
        }
        if let Some(n) = self
            .world
            .objects
            .get_component_mut::<crate::model::npc::Npc>(&self.npc)
        {
            n.script_value = value;
        }
    }

    /// `QuestState.getMemoState()` — Java stores it as the quest variable
    /// `memoState` (`QuestState.MEMO_VAR`), a second progress axis alongside
    /// `cond`: `cond` drives the client's quest window, `memoState` is the
    /// script's own bookkeeping and is never shown.
    pub fn memo_state(&self) -> i32 {
        self.get_int("memoState")
    }

    /// `QuestState.setMemoState(value)`.
    pub fn set_memo_state(&mut self, value: i32) {
        self.set_var("memoState", value.to_string());
    }

    /// `QuestState.getMemoStateEx(slot)` — a *second*, slotted memo axis
    /// (`QuestState.MEMO_EX_VAR + slot`), independent of `memoState`. Quest
    /// 417 packs two counters into one slot via tens/units arithmetic.
    pub fn memo_state_ex(&self, slot: i32) -> i32 {
        self.get_int(&format!("memoStateEx{slot}"))
    }

    /// `QuestState.setMemoStateEx(slot, value)`.
    pub fn set_memo_state_ex(&mut self, slot: i32, value: i32) {
        self.set_var(&format!("memoStateEx{slot}"), value.to_string());
    }
}
