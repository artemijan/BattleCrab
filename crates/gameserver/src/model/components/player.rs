//! Per-player state with no system sweep of its own: the persisted variable
//! bag, client settings, UI layout, and the GM/admin flags.

use bevy_ecs::component::Component;
use std::collections::HashMap;

/// The per-character key/value store (Java `PlayerVariables`, table
/// `character_variables`). Java's `AbstractVariables` is a `StatSet` with typed
/// getters and a dirty flag that `storeMe` consults; here the map is plain and
/// the memory-first autosave flushes it wholesale with the rest of the
/// character, so no dirty tracking is needed.
///
/// Only the keys a ported subsystem reads live here today —
/// [`VITALITY_ITEMS_USED`]. The rest of Java's key set (instance origin/restore,
/// UI key mapping, ability points, auto-use settings, …) belongs to subsystems
/// that are not ported; they will land as their milestones do. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerVariables(pub HashMap<String, String>);

impl PlayerVariables {
    /// Java `AbstractVariables.getInt(key, default)` — a non-numeric or absent
    /// value yields the default.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.0
            .get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    /// Java `AbstractVariables.set(key, value)`.
    pub fn set_int(&mut self, key: &str, value: i32) {
        self.0.insert(key.to_string(), value.to_string());
    }
}

/// `PlayerVariables.VITALITY_ITEMS_USED_VARIABLE_NAME` — how many
/// vitality-restoring items the character has consumed this week, capped by
/// `Config.VITALITY_MAX_ITEMS_ALLOWED` and reported by `ExVitalityEffectInfo`.
pub const VITALITY_ITEMS_USED: &str = "VITALITY_ITEMS_USED";

/// `PlayerVariables.UI_KEY_MAPPING` — the client's saved key layout, stored as
/// Java stores it: the raw bytes joined by tabs (`RequestSaveKeyMapping`'s
/// `SPLIT_VAR`), replayed verbatim by `ExUISetting`.
pub const UI_KEY_MAPPING: &str = "UI_KEY_MAPPING";

/// `PlayerVariables.WORLD_CHAT_VARIABLE_NAME` — how many world-chat lines the
/// character has spent today (Java `Player.getWorldChatUsed`/`setWorldChatUsed`).
///
/// Counts **up** toward the quota rather than down from it, which is why the
/// daily reset writes `0` and not the per-day allowance: the allowance is
/// config (`WorldChatPointsPerDay`) and can change under a stored value.
pub const WORLD_CHAT_USED: &str = "WORLD_CHAT_USED";

/// Java `AutoPlaySettings` + the auto-attack half of `AutoUseSettings` — the
/// `.play` panel's state. Persisted through `PlayerVariables` at logout
/// (`AUTO_USE_SETTINGS`), so the panel survives a relog; whether the *loop*
/// restarts is `ResumeAutoPlay`'s call.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPlaySettings {
    /// The loop is running.
    pub active: bool,
    /// `AutoUseSettings.getAutoActions().contains(2)` — the auto-attack box.
    /// Java calls the inverse `isMageCaster`: with it off the loop acquires a
    /// target but never swings.
    pub auto_attack: bool,
    /// `doPickup()` — walk to and take nearby loot.
    pub pickup: bool,
    /// `isRespectfulHunting()` — skip a mob already fighting somebody else.
    pub respectful_hunting: bool,
    /// `isShortRange()` — 600 units instead of 1400.
    pub short_range: bool,
    /// 0 any / 1 monster / 2 characters / 3 npc.
    pub next_target_mode: i32,
    /// The HP percentage the auto-potion half drinks at (slice 2).
    pub potion_percent: i32,
}

impl Default for AutoPlaySettings {
    fn default() -> Self {
        Self {
            active: false,
            auto_attack: true,
            pickup: false,
            respectful_hunting: false,
            short_range: false,
            next_target_mode: 0,
            potion_percent: 0,
        }
    }
}

/// Java `AutoUseSettings` — what the three sub-pages choose: buffs to keep up,
/// attack skills to fire, supply items to use, and the one healing potion.
/// Persisted alongside [`AutoPlaySettings`] so the panel survives a relog.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoUseSettings {
    /// Self-target skills, cast **even in town**.
    pub buffs: Vec<i32>,
    /// Offensive skills, cast at the current target outside a peace zone.
    pub skills: Vec<i32>,
    /// Shots, scrolls and the like, used outside a peace zone.
    pub supply_items: Vec<i32>,
    /// The single healing potion slot (`0` = none).
    pub potion_item: i32,
}

/// Panel shortcuts (Java `Player._shortCuts`), keyed by
/// `slot + page * 12` — a `BTreeMap` so `ShortCutInit` order is stable.
/// Player-only; registry logic in `model/shortcut.rs`.
#[derive(Component, Debug, Clone, Default)]
pub struct Shortcuts(pub std::collections::BTreeMap<i32, crate::model::shortcut::Shortcut>);

/// Server-stored macros (Java `Player._macros`), insertion-ordered like
/// Java's `LinkedHashMap`. `next_id` is `MacroList._macroId` (starts at
/// 1000). Player-only; registry logic in `model/shortcut.rs`.
#[derive(Component, Debug, Clone)]
pub struct Macros {
    pub next_id: i32,
    pub entries: Vec<crate::model::shortcut::Macro>,
}

/// Abnormal visual effects a GM pinned on this creature with `//ave_abnormal`,
/// independent of any buff. Java has no such component — it calls
/// `startAbnormalVisualEffect` directly, which mutates the same
/// `EffectList._abnormalVisualEffects` set the buffs feed. This port keeps the
/// buff-derived set computed (a fold, never stored), so the manual ones need
/// somewhere of their own to live.
#[derive(Component, Debug, Clone, Default)]
pub struct AdminVisuals(pub Vec<i16>);

/// GM-toggled state on a player (Java `Creature._isInvul`, `_isUndying`,
/// `Player.setInvisible`/`setSilenceMode`/`setDietMode`). Presence-based:
/// absent = every flag `false`, added on the first toggle or by the GM-startup
/// block at enter-world.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AdminFlags {
    /// `isInvul` — incoming damage is ignored entirely.
    pub invul: bool,
    /// `isUndying` — damage applies but HP never drops below 1 (no death).
    pub undying: bool,
    /// `isInvisible` — hidden from other players (`//hide`).
    pub hidden: bool,
    /// `isSilenceMode` — GM refuses incoming whispers/PMs. Set by
    /// `GMStartupSilence` and `//silence`; honored in `chat.rs`'s `Whisper` arm,
    /// which answers the sender `THAT_PERSON_IS_IN_MESSAGE_REFUSAL_MODE` and
    /// delivers nothing.
    pub silence: bool,
    /// `isInDietMode` — weight overload is ignored. Set by `GMStartupDietMode`
    /// and `//diet`; read by [`crate::game_loop::stats::weight`], which reports penalty
    /// level 0 and "not overloaded" for a dieting GM no matter what they carry.
    pub diet: bool,
    /// `//para`'s `setBlockActions(true)` + `startParalyze()` — ORed into
    /// `abnormal::is_action_blocked`/`is_movement_disabled` beside the buff
    /// flags. Attachable to NPCs too (Java paralyzes any creature target).
    pub paralyzed: bool,
    /// `//settargetable`'s `setTargetable(false)` — `handle_action` refuses to
    /// select this creature.
    pub untargetable: bool,
}

/// The player's in-progress Lucky Lottery number picks (Java `Player._loto[5]`,
/// G26.5) — the five 1–20 numbers chosen through the Loto NPC dialog before a
/// ticket is bought. Transient; presence-based (added on first pick, reset to
/// zeros each time the buy window is (re)opened). `0` = an empty slot.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LotoPicks(pub [i32; 5]);

/// The player's in-progress Monster Race bet (Java `Player._raceTickets[2]`,
/// G26.5): slot 0 = the chosen lane (1–8), slot 1 = the price tier (1–8) picked
/// through the RaceManager dialog before the ticket is bought. `0` = unset.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RaceTicket(pub [i32; 2]);

/// Object id of the last NPC this player clicked/talked to (Java
/// `Player._lastFolkNpc`, set by `NpcAction.action`). Bare (non-`npc_`-
/// prefixed) HTML bypasses like `Quest ClanMaster 9000-02.htm` resolve their
/// NPC through this — Java uses the `validateHtmlAction` origin id there,
/// which we don't port (see `game_loop/client/bypass.rs`); the distance re-check at
/// use time is the guard either way. Player-only.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastFolkNpc(pub i32);

/// Java `Player._fishing` (G32): the active fishing session. `cast_seq`
/// invalidates stale scheduled reel/cast tasks — a fresh cast (or a stop) bumps
/// it, so an in-flight `FishingReel`/`FishingCast` from a superseded cast
/// no-ops. The bait location is where the bob landed, echoed in the fishing
/// packets.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct FishingSession {
    pub is_fishing: bool,
    pub cast_seq: u64,
    pub bait_x: i32,
    pub bait_y: i32,
    pub bait_z: i32,
}

/// Java `Player._observerMode` for the **plain** flavour — the Broadcasting
/// Tower's spectator seats (`bypasshandlers/Observation`). Present only while
/// observing; `return_pos` is Java's `_lastLoc`, where `leaveObserverMode` puts
/// the viewer back.
///
/// The Olympiad's spectator mode is [`OlympiadObserver`] and is deliberately a
/// different component: Java shares one flag but two enter/leave pairs and two
/// client packets, and answering the wrong one would strand a viewer.
#[derive(Component, Debug, Clone, Copy)]
pub struct Observing {
    pub return_pos: (i32, i32, i32),
}

/// Java `Player._observerMode` + `_lastLoc` — present while a player is watching
/// an Olympiad match. Holds the location to teleport back to on exit and the
/// arena being watched (`_olympiadGameId`).
#[derive(Component, Debug, Clone, Copy)]
pub struct OlympiadObserver {
    pub return_pos: (i32, i32, i32),
    pub arena: i32,
}

/// Marks an HQ flag planted by **skill 326 "Build Advanced Headquarters"**
/// (Java `SiegeFlag._isAdvanced`). Same NPC as the basic camp (35062); the
/// flag only changes how much damage the thing takes.
///
/// **Deliberate deviation from Java** — see `docs/CUSTOM_DIST_DEVIATIONS.md`.
/// `SiegeFlagStatus.reduceHp` reads:
///
/// ```text
/// if (isAdvancedHeadquarter()) super.reduceHp(value / 2, …);
/// super.reduceHp(value, …);
/// ```
///
/// with no `else` and no `return`, so upstream an advanced HQ takes
/// `value/2 + value` — **1.5× damage**, making the noble-only skill strictly
/// worse than the basic one. This port halves, which is what the skill's name,
/// its `autoGet` place in the noble tree, and the obvious intent of that `if`
/// all say it should do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvancedHeadquarter;
