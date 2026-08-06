//! Cursed weapons — Java `model/CursedWeapon` + `CursedWeaponsManager`. On this
//! Interlude dist there are two: Demonic Sword Zariche (8190) and Blood Sword
//! Akamanah (8689). Static config comes from `CursedWeapons.xml`; the live
//! wielder state (`cursed_weapons` table) is overlaid at boot.
//!
//! Scope: the manager state + the admin surface (`//cw_*`) and its
//! activate/end-of-life lifecycle. The autonomous parts — drop-from-monster,
//! the expiry task and the login restore — live in
//! `game_loop::cursed_weapon`, including drop-on-PK-death and the per-kill
//! time decay (the `end_time -= durationLost` tail of `increase_kills`). Java
//! has no HP drain here.

/// One cursed weapon: its `CursedWeapons.xml` config plus the runtime state Java
/// keeps on the `CursedWeapon` object (persisted in the `cursed_weapons` table).
#[derive(Debug, Clone)]
pub struct CursedWeapon {
    // --- config (CursedWeapons.xml) ---
    pub item_id: i32,
    pub skill_id: i32,
    pub name: String,
    /// `disapearChance` (%) — used by the (deferred) drop logic.
    pub disappear_chance: i32,
    /// `dropRate` (100000 == 100%) — used by the (deferred) drop logic.
    pub drop_rate: i32,
    /// `duration` in minutes: how long a fresh activation lasts.
    pub duration: i32,
    /// `durationLost` in minutes: the "hungry" decay step (deferred task).
    pub duration_lost: i32,
    /// `stageKills`: kills per skill-level step.
    pub stage_kills: i32,
    /// `SkillData.getMaxLevel(skillId)` — computed at boot; clamps `level`.
    pub skill_max_level: i32,

    // --- runtime state (cursed_weapons table) ---
    pub is_activated: bool,
    pub is_dropped: bool,
    /// Object id of the wielder (Java `_playerId`), 0 when unowned.
    pub player_id: i32,
    /// The wielder's saved reputation/pk-kills, restored on end-of-life.
    pub player_reputation: i32,
    pub player_pk_kills: i32,
    pub nb_kills: i32,
    /// Epoch millis at which the weapon expires (Java `_endTime`). Also the
    /// disappear deadline while `is_dropped` (an un-grabbed drop vanishes).
    pub end_time: i64,
    /// The ground-item object id while `is_dropped` (0 otherwise) — so the
    /// pickup / disappear paths can despawn it. Runtime only, not persisted.
    pub dropped_item_oid: i32,
}

impl CursedWeapon {
    /// Java `isActive()` — activated (held) or dropped (on the ground).
    pub fn is_active(&self) -> bool {
        self.is_activated || self.is_dropped
    }

    /// Java `getLevel()`: `1 + kills/stageKills` clamped to the skill max, or 0
    /// when not activated.
    pub fn level(&self) -> i32 {
        if self.is_activated {
            (1 + self.nb_kills / self.stage_kills.max(1)).min(self.skill_max_level.max(1))
        } else {
            0
        }
    }

    /// Java `getTimeLeft()` = `endTime - now` (never negative).
    pub fn time_left(&self, now_ms: i64) -> i64 {
        (self.end_time - now_ms).max(0)
    }

    /// Reset to the "not in the world" state (Java `endOfLife`'s tail).
    pub fn reset(&mut self) {
        self.is_activated = false;
        self.is_dropped = false;
        self.player_id = 0;
        self.nb_kills = 0;
        self.end_time = 0;
        self.dropped_item_oid = 0;
    }
}
