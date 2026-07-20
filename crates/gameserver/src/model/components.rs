//! ECS components shared by players and NPCs — stage 2 of the `bevy_ecs`
//! adoption (`docs/PLAN_ECS_STAGE2.md` §2). Data only: components are split
//! along *system access seams* (what a per-tick sweep reads/writes without
//! the rest of the object), not per field, and carry no game logic beyond
//! trivial accessors. Player-only / NPC-only state stays in the (shrinking)
//! fat structs in `model/mod.rs` / `model/npc.rs` until its own phase.

use std::collections::HashMap;

use bevy_ecs::component::Component;

/// World position + facing (from Java `WorldObject`'s x/y/z +
/// `Creature._heading`). On both players and NPCs.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

impl Position {
    /// 2D center-to-center distance (the shape every range/reach check uses).
    pub fn distance_2d(&self, other: &Position) -> f64 {
        (((other.x - self.x) as f64).powi(2) + ((other.y - self.y) as f64).powi(2)).sqrt()
    }
}

/// An item lying on the ground (Java `Item` in `ItemLocation.VOID`, tracked by
/// `ItemsOnGroundManager`). A world entity with [`Position`]/[`RegionCell`];
/// indexed in `World::ground_item_regions`. Dropped by players (`//` drop) or
/// monster death (auto-loot off), picked up via a click (`Action`).
#[derive(Component, Debug, Clone)]
pub struct GroundItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub enchant: i32,
}

/// One line in a player's private sell store (Java `TradeItem`): the inventory
/// instance offered, how many, and the asking price per unit.
#[derive(Debug, Clone, Copy)]
pub struct StoreItem {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub price: i64,
    pub enchant: i32,
}

/// A player's active private sell store (Java `Player._sellList` + store title).
/// Present only while the store is open; the store *type* (the CharInfo byte)
/// lives on [`Player::store_type`](crate::model::Player::store_type).
#[derive(Component, Debug, Clone, Default)]
pub struct PrivateStore {
    pub items: Vec<StoreItem>,
    pub title: String,
}

/// An in-progress player-to-player trade (Java `Player._activeTradeList`).
/// Present on both partners while the trade window is open; `items` are this
/// player's offered lines (`price` unused), `confirmed` is their "OK" press.
#[derive(Component, Debug, Clone, Default)]
pub struct Trade {
    pub partner: i32,
    pub items: Vec<StoreItem>,
    pub confirmed: bool,
}

/// A pending trade *request* on the target (Java `Player._activeRequester`):
/// `from` asked to trade; cleared on answer/timeout.
#[derive(Component, Debug, Clone, Copy)]
pub struct PendingTrade {
    pub from: i32,
}

/// Which warehouse the player currently has open (Java
/// `Player._activeWarehouse`), set by the warehouse-keeper bypass. The
/// deposit/withdraw client packets carry no warehouse type, so the handlers
/// read this to route items to the right container.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActiveWarehouse {
    #[default]
    Private,
    Clan,
    Freight,
}

/// An open enchant window (Java `EnchantItemRequest`, held as a `Player`
/// request). Present from the `EnchantScrolls` handler's `ChooseInventoryItem`
/// until the enchant completes or is cancelled. Object ids are `0` (none) until
/// the client fills them via the Ex-packet handshake.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct EnchantRequest {
    /// The scroll's inventory object id (set when the window opens).
    pub scroll_oid: i32,
    /// The item being enchanted (set by `RequestExTryToPutEnchantTargetItem`).
    pub item_oid: i32,
    /// The support item, if any (`0` = none). Support items are not yet wired.
    pub support_oid: i32,
    /// `_isProcessing` — set once `RequestEnchantItem` starts, to reject
    /// re-entrant packets mid-roll.
    pub processing: bool,
}

/// The world-region cell this object is registered in (Java
/// `WorldObject._worldRegion`). Kept in sync with `Position` by the
/// visibility/movement systems (Java `updateWorldRegion`/`switchRegion`) —
/// visibility deltas and broadcast scoping compare this, never raw
/// coordinates. Separate from `Position` because it changes on a different
/// cadence (cell crossings) and has different readers (visibility, not math).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCell(pub (i32, i32));

/// HP/MP + liveness (Java `CreatureStatus` + `Creature._isDead`). On both
/// players and NPCs; CP is player-only and lives in [`PlayerVitals`]. `dead`
/// rides here (not a marker component): every writer flips it in the same
/// breath as HP, and death is a branch inside systems rather than a sweep
/// filter — a field avoids an archetype move per death/revive.
#[derive(Component, Debug, Clone, Copy)]
pub struct Vitals {
    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    /// Java `Creature._isDead` — for NPCs: corpse until decay removes it.
    pub dead: bool,
}

impl Vitals {
    pub fn hp_full(max_hp: i32, max_mp: i32) -> Self {
        Self { max_hp, cur_hp: max_hp as f64, max_mp, cur_mp: max_mp as f64, dead: false }
    }
}

/// CP (`PcStatus`) — the player-only vitals extension, so NPC damage code
/// never sees a CP field it must ignore.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerVitals {
    pub max_cp: i32,
    pub cur_cp: f64,
}

/// Movement speeds + run/walk mode. For players these are stat-finalizer
/// *outputs* (`recalculate_stats` writes them: template base × buff
/// modifiers); for NPCs they're memoized from the template at spawn (the
/// template never changes, so this is the same value the old code re-read
/// per use). f64 keeps NPC fractional speeds exact; player values are the
/// same rounded numbers as before, just stored as f64.
#[derive(Component, Debug, Clone, Copy)]
pub struct Speeds {
    /// `SwampZone.getMoveBonus()` for the zone the creature is standing in,
    /// or 1.0. Java re-reads the zone inside `SpeedFinalizer`; the port caches
    /// it here and refreshes it on zone enter/exit, so the stat recompute stays
    /// free of world lookups.
    pub swamp_multiplier: f64,
    pub run_spd: f64,
    pub walk_spd: f64,
    pub swim_run_spd: f64,
    pub swim_walk_spd: f64,
    pub move_multiplier: f64,
    /// Raw template base run speed (Java `getTemplate().getBaseValue(RUN_SPEED,
    /// 0)`) — the *unboosted, unbuffed* class/NPC template value. Constant for
    /// the object's lifetime; used only as the denominator of
    /// [`Speeds::client_move_multiplier`], never in the movement math.
    pub base_run_spd: f64,
    /// `Creature._isRunning` — players spawn running; NPCs walk until AI
    /// flips to run on aggro.
    pub running: bool,
    /// In a `WaterZone` (`isInsideZone(ZoneId.WATER)`) — flipped by zone
    /// revalidation; `move_speed` switches to the swim speeds while set.
    pub swimming: bool,
}

impl Speeds {
    /// The ground speed movement math uses (`Creature.getMoveSpeed`, incl.
    /// its "in water → swim speeds" branch).
    pub fn move_speed(&self) -> f64 {
        let (run, walk) =
            if self.swimming { (self.swim_run_spd, self.swim_walk_spd) } else { (self.run_spd, self.walk_spd) };
        (if self.running { run } else { walk }) * self.move_multiplier
    }

    /// Java `CreatureStat.getMovementSpeedMultiplier`: current move speed ÷ the
    /// raw template base speed. This is the value the client uses to set the
    /// **leg-animation playback rate**, so it must be *derived* from the
    /// finalized speed — not a standalone field. Stat-based speed buffs (Super
    /// Haste, Wind Walk, …) raise `run_spd` without touching `move_multiplier`;
    /// sending a bare `move_multiplier` there made the character glide at the
    /// buffed speed while its legs animated at the base cadence. Falls back to
    /// `1.0` if the base is unknown (0), so a zero-template object is unchanged.
    pub fn client_move_multiplier(&self) -> f64 {
        if self.base_run_spd <= 0.0 {
            return 1.0;
        }
        self.move_speed() *(1.0 / self.base_run_spd)
    }

    /// The four speed shorts `UserInfo`/`CharInfo` carry, in wire order. Java
    /// sends `Math.round(speed / moveMultiplier)` and the client multiplies
    /// [`Speeds::client_move_multiplier`] back in for display and movement —
    /// so the finalized speeds must be sent *divided*, or the buff scale is
    /// counted twice (Super Haste 4 showed ~3100 on the client while the
    /// server moved at ~630).
    pub fn client_speed_fields(&self) -> [i16; 4] {
        let mult = self.client_move_multiplier();
        let div = |v: f64| if mult > 0.0 { (v / mult).round() as i16 } else { v as i16 };
        [div(self.run_spd), div(self.walk_spd), div(self.swim_run_spd), div(self.swim_walk_spd)]
    }
}

/// Collision cylinder (template `collision_radius`/`collision_height`) —
/// reach/range gates and packet fields.
#[derive(Component, Debug, Clone, Copy)]
pub struct Collision {
    pub radius: f64,
    pub height: f64,
}

/// Combat-stat finalizer outputs (Java `CreatureStat`'s computed values).
/// Players: written by `recalculate_stats` (base × stat bonus × level mod ×
/// buff modifiers), same rounded values as before stored as f64. NPCs:
/// memoized once at spawn from the (immutable) template through the same
/// finalizer math the old `combatant()` ran per call — values identical.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CombatStats {
    pub p_atk: f64,
    pub m_atk: f64,
    pub p_def: f64,
    pub m_def: f64,
    pub p_atk_spd: i32,
    pub m_atk_spd: i32,
    /// Per-mille (×10), like Java's `PCriticalRateFinalizer` output.
    pub crit_hit: f64,
    pub m_crit_hit: f64,
    pub evasion: i32,
    pub accuracy: i32,
    pub magic_evasion: i32,
    pub magic_accuracy: i32,
    pub atk_range: i32,
    /// Weapon `randomDamage` (class templates all declare `baseRndDam = 10`;
    /// NPC templates carry their own).
    pub random_dmg: i32,
}

impl CombatStats {
    /// Java `CreatureStat.getAttackSpeedMultiplier` (`Formulas.calcAtkSpdMultiplier`):
    /// the client uses this to set the **attack-animation playback rate**, the
    /// haste counterpart of [`Speeds::client_move_multiplier`]. Java's formula
    /// `dexBonus × (weaponBaseAtkSpd / 333) × mul + add / 333` reduces exactly to
    /// `pAtkSpd / 333` (the finalized `p_atk_spd` is `weaponBase × dexBonus × mul
    /// + add`) whenever `mul ≥ 0.7` and there is no move-type term — the case for
    /// every player here. Sending a bare `1.0` (the old value) left the swing
    /// animation at base cadence while Super Haste quadrupled the actual p_atk_spd.
    pub fn client_atk_speed_multiplier(&self) -> f64 {
        self.p_atk_spd as f64 / 333.0
    }
}

/// Swing/stance timing (Java `_attackEndTime` + `AttackStanceTaskManager`
/// membership). `stance_until_tick` is player-only in practice (the client
/// sword-drawn state); it stays 0 on NPCs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AttackState {
    /// Busy-swinging until this tick; the next swing may start once past.
    pub attack_end_tick: u64,
    /// In combat stance until this tick — 15 s past the last swing/hit;
    /// 0 = not in stance.
    pub stance_until_tick: u64,
}

/// PvP flag state (Java `Player._pvpFlag` + `_pvpFlagLasts`), runtime-only —
/// never persisted, so it lives in its own component rather than on the stored
/// `Player`. `flag` is the broadcast value: 0 = clean, 1 = solid purple,
/// 2 = blinking (the last 20 s before it clears). `expires_tick` is when the
/// flag drops back to 0; `PvpFlagTaskManager`'s 1 s sweep drives the 1→2→0
/// transitions.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PvpState {
    pub flag: u8,
    pub expires_tick: u64,
}

/// An in-flight move — **present only while moving** (the stage-2 shape of
/// Java's nullable `Creature._move`). Presence is the movement tick's sweep
/// filter: the interpolation query visits only entities that carry this,
/// instead of scanning 34.9k static NPCs' `None`s every 100 ms. Insert =
/// `moveToLocation`, remove = arrival/stop/teleport/death.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Movement(pub crate::model::movement::MoveData);

/// A move deferred on the path worker — **present only while waiting** for
/// the `PathEvent` reply. `seq` is the request's sequence number: a reply
/// with an older one is stale (superseded by a newer click) and is dropped.
/// Java has no equivalent state — `CellPathFinding.findPath` runs
/// synchronously inside `moveToLocation`.
#[derive(Component, Debug, Clone, Copy)]
pub struct PathWait {
    pub seq: u64,
}

/// An in-flight cast — **present only mid-cast** (Java's single NORMAL
/// `SkillCaster` slot, `Player.cast` before stage 2). The generation counter
/// (`Player.cast_seq`) stays on the player: it must survive across casts for
/// the scheduler's stale-task no-op contract.
#[derive(Component, Debug, Clone)]
pub struct Casting(pub crate::model::CastState);

/// A persistent AI intention (the attack loop) — **present only while set**,
/// so the player combat tick sweeps intent-holders only.
#[derive(Component, Debug, Clone, Copy)]
pub struct Intent(pub crate::model::PlayerIntent);

/// The one action a busy actor holds back — Java's three queue slots
/// (`PlayerAI._nextIntention` MOVE_TO, `Player._queuedSkill`,
/// `AbstractAI._nextAction` equip) folded into a single presence-based slot.
/// Written while a cast or attack swing is in flight, consumed when it stops
/// (`stop_casting` / the `AttackFinish` task), dropped on death/teleport.
/// The slot is last-click-wins, matching Java's observable outcomes: a move
/// packet wipes `_queuedSkill` (`MoveBackwardToLocation.runImpl`), and a
/// queued skill's `stopCasting` launch supersedes the saved move. Known
/// narrowing: Java keeps the equip in its own slot and can fire it
/// *alongside* a queued move; this single slot keeps only the last click.
#[derive(Component, Debug, Clone, Copy)]
pub enum QueuedAction {
    /// A move click swallowed while busy (`onIntentionMoveTo` →
    /// `saveNextIntention`), or the move a good-skill cast interrupted
    /// (`PlayerAI.changeIntention`).
    Move { x: i32, y: i32, z: i32 },
    /// `Player._queuedSkill` (`SkillUseHolder`): skill + click modifiers, no
    /// target — the target is re-resolved from the player's *current* target
    /// at replay, which is what lets a mid-cast re-target redirect the
    /// queued skill.
    Skill { skill_id: i32, ctrl: bool, shift: bool },
    /// An equipable `UseItem` click (Java defers it via
    /// `NextAction(EVT_FINISH_CASTING, …)` / a swing-end schedule).
    UseItem { item_object_id: i32 },
}

/// Known skills (skill_id → level), loaded from `character_skills` (or the
/// class's autoGet initial set at creation). Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct SkillBook(pub HashMap<i32, i32>);

/// The player's three worn henna dyes (Java `Player._henna[3]`), by slot →
/// dye id. Loaded from `character_hennas`, persisted in the store transaction.
/// The dyes' base-stat bonuses are folded into [`BaseStats`] (recomputed on
/// add/remove); this component holds only the slot assignments. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct HennaSlots(pub [Option<i32>; 3]);

impl HennaSlots {
    /// Number of filled slots (Java `3 - getHennaEmptySlots()` counts these).
    pub fn worn(&self) -> usize {
        self.0.iter().filter(|s| s.is_some()).count()
    }

    /// The worn dye ids in slot order.
    pub fn dye_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.0.iter().filter_map(|s| *s)
    }
}

/// Clan skills currently granted to this member (skill_id → level), Java's
/// `Player.addSkill(clanSkill, false)` set. **Transient** — re-derived from the
/// clan on every login (see `game_loop::clans::apply_clan_skills`) and never
/// written to `character_skills` (Java passes `store=false`). Kept separate from
/// [`SkillBook`] both to preserve that no-persist contract and so leaving/
/// dispersing the clan strips exactly these. Folded into the `SkillList` packet
/// alongside the skill book. Player-only.
#[derive(Component, Debug, Clone, Default)]
pub struct ClanSkills(pub HashMap<i32, i32>);

/// The player's registered crafting recipes as recipe-*list* ids, split by
/// book (Java `Player._dwarvenRecipeBook` / `_commonRecipeBook`, keyed by
/// `RecipeList.getId()`). Loaded from `character_recipebook`, persisted in the
/// store transaction (the `type` column = dwarven/common, derived from
/// `RecipeData`). Player-only. Order is kept stable (Java uses a sorted map;
/// here insertion order — the wire packet carries a running 1-based slot index
/// the client keys buttons by, so consistency across resends is what matters).
#[derive(Component, Debug, Clone, Default)]
pub struct RecipeBook {
    pub dwarven: Vec<i32>,
    pub common: Vec<i32>,
}

impl RecipeBook {
    /// Whether either book holds this recipe-list id (Java `hasRecipeList`).
    pub fn contains(&self, list_id: i32) -> bool {
        self.dwarven.contains(&list_id) || self.common.contains(&list_id)
    }
}

/// The duel this player is currently in (`Player._isInDuel` → the duel id).
/// Present from the countdown until the duel ends.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DuelRef(pub u32);

/// An outstanding duel challenge awaiting this player's answer
/// (`ExDuelAskStart` sent, `RequestDuelAnswerStart` pending).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PendingDuel {
    pub challenger: i32,
}

/// Java `Attackable`'s over-hit trio (`_overhitEnabled` / `_overhitDamage` /
/// `_overhitAttacker`): a killing blow from an `<overHit>` skill banks the
/// *excess* damage, which becomes bonus XP for whoever landed it.
///
/// Armed per damaging blow and disarmed by any blow that fails to kill, so it
/// only ever survives on a corpse — exactly Java's `setOverhitValues` contract.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Overhit {
    pub damage: f64,
    pub attacker: i32,
}

/// Java `Creature._disableRangedAttackEndTime` — the tick a bow/crossbow may
/// fire again. Present only after a shot; the reload delay is
/// `900000 / pAtkSpd` ms (`Formulas.calculateReuseTime`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RangedReload {
    pub ready_at_tick: u64,
}

/// Abnormal visual effects a GM pinned on this creature with `//ave_abnormal`,
/// independent of any buff. Java has no such component — it calls
/// `startAbnormalVisualEffect` directly, which mutates the same
/// `EffectList._abnormalVisualEffects` set the buffs feed. This port keeps the
/// buff-derived set computed (a fold, never stored), so the manual ones need
/// somewhere of their own to live.
#[derive(Component, Debug, Clone, Default)]
pub struct AdminVisuals(pub Vec<i16>);

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

/// `PlayerVariables.VITALITY_ITEMS_USED_VARIABLE_NAME` — how many
/// vitality-restoring items the character has consumed this week, capped by
/// `Config.VITALITY_MAX_ITEMS_ALLOWED` and reported by `ExVitalityEffectInfo`.
pub const VITALITY_ITEMS_USED: &str = "VITALITY_ITEMS_USED";

impl PlayerVariables {
    /// Java `AbstractVariables.getInt(key, default)` — a non-numeric or absent
    /// value yields the default.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.0.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    /// Java `AbstractVariables.set(key, value)`.
    pub fn set_int(&mut self, key: &str, value: i32) {
        self.0.insert(key.to_string(), value.to_string());
    }
}

/// A player's active private *manufacture* store (Java `Player._manufactureItems`
/// + store title): the recipes they craft-for-hire and the adena fee each.
/// Present only while the store is open; not persisted (`StoreRecipeShopList =
/// False`). The store *type* byte (MANUFACTURE) lives on
/// [`Player::store_type`](crate::model::Player::store_type). `items` are
/// `(recipe_list_id, cost)`.
#[derive(Component, Debug, Clone, Default)]
pub struct ManufactureStore {
    pub items: Vec<(i32, i64)>,
    pub title: String,
}

/// The currently-applied grade-penalty levels (Java `Player._expertiseWeaponPenalty`
/// / `_expertiseArmorPenalty`, each 0-4). Cached so `refresh_expertise_penalty`
/// can no-op when nothing changed, and read by `EtcStatusUpdate`. Player-only.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ExpertisePenalty {
    pub weapon: i32,
    pub armor: i32,
}

/// Live cooldowns (Java `_reuseTimeStampsSkills` + `_disabledSkills`,
/// unified), keyed by `Skill::reuse_key()`. Checked lazily — no expiry tasks.
/// Persisted across relog via `character_skills_save` (Java `storeEffect`/
/// `restoreEffects`, reuse half; gated by `StoreSkillCooltime`) — see
/// `db::SkillReuseRow` + `PlayerData::restore_reuses`. Buff restore (the
/// `restore_type = 0` half) is still deferred.
#[derive(Component, Debug, Clone, Default)]
pub struct Reuses(pub HashMap<i32, crate::model::SkillReuse>);

/// Active buffs/debuffs (Java `EffectList`). Expiry is driven by the
/// `Scheduler` (`ScheduledTask::BuffExpire`), not by anything here.
#[derive(Component, Debug, Clone, Default)]
pub struct Buffs(pub Vec<crate::model::skill::ActiveBuff>);

/// Java `CreatureStat`'s two modifier maps — buffs/gear push entries here;
/// `recalculate_stats` folds them into `CombatStats`/`Speeds`.
#[derive(Component, Debug, Clone, Default)]
pub struct StatModifiers {
    pub add: HashMap<crate::model::stats::Stat, f64>,
    pub mul: HashMap<crate::model::stats::Stat, f64>,
    /// Admin fixed-value overrides (`//setparam` → Java
    /// `CreatureStat.addFixedValue`): when present, the stat's finalizer
    /// returns this value verbatim, ignoring base/buffs. Persists across buff
    /// recomputes (not cleared with `add`/`mul`); cleared by `//unsetparam`.
    pub fixed: HashMap<crate::model::stats::Stat, f64>,
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

/// Currently targeted object id (Java `Creature._target`), player-only —
/// NPC targeting goes through the aggro list.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetRef(pub Option<i32>);

/// The multisell list the player currently has open (Java
/// `Player._currentMultiSell` / `setMultiSell`), player-only. Presence-based:
/// added when a `MultiSellList` is sent, read/validated by `MultiSellChoose`,
/// removed on a stale/forged choose. Only the list id is retained — the
/// community-board path uses default (1.0) multipliers and no npc/tax, so the
/// `PreparedMultisellListHolder` collapses to a lookup by id.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveMultisell {
    pub list_id: i32,
}

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
    /// `GMStartupSilence`; TODO(G14): honored once whisper delivery exists.
    pub silence: bool,
    /// `isInDietMode` — weight overload is ignored. Set by `GMStartupDietMode`;
    /// TODO(G14): honored once the overload calc exists.
    pub diet: bool,
}

/// Object id of the last NPC this player clicked/talked to (Java
/// `Player._lastFolkNpc`, set by `NpcAction.action`). Bare (non-`npc_`-
/// prefixed) HTML bypasses like `Quest ClanMaster 9000-02.htm` resolve their
/// NPC through this — Java uses the `validateHtmlAction` origin id there,
/// which we don't port (see `game_loop/bypass.rs`); the distance re-check at
/// use time is the guard either way. Player-only.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastFolkNpc(pub i32);

/// Zone membership (Java `Creature._zones` narrowed to the loaded
/// `ZoneKind`s) + the revalidation bookkeeping `Creature`/`Player` keep
/// alongside it. Player-only for now: the peace gate only ever checks
/// playables, and NPC water/no-restart behavior has no consumer yet.
#[derive(Component, Debug, Clone, Copy)]
pub struct ZoneFlags {
    /// OR of `ZoneKind::bit()`s the player currently stands in.
    pub mask: u8,
    /// `Creature._lastZoneValidateLocation` — revalidation is skipped while
    /// within 100 units of the last validated spot (movement calls it every
    /// tick).
    pub last_validate: (i32, i32, i32),
    /// `Player._lastCompassZone` (`ExSetCompassZoneCode` value last sent).
    /// Starts at 0 like Java's field default — 0 is not a valid zone code
    /// (they run 0x08–0x0F), so the first revalidate always pushes the real
    /// code. The client needs that initial push: without a valid code it
    /// treats the zone as unknown and refuses to open the world map.
    pub last_compass: i32,
    /// Whether the player currently stands in an *active* siege zone (its
    /// castle's siege in progress). Tracked so `revalidateZone` can fire
    /// `SiegeZone.onEnter`/`onExit` (combat-zone messages + the leave-flag) on
    /// the transition — a `SiegeZone` is only a combat zone while active, which
    /// the plain zone mask (membership only) can't express.
    pub in_active_siege: bool,
}

impl Default for ZoneFlags {
    fn default() -> Self {
        // A fresh player has no validated location yet — `i32::MIN` keeps the
        // first revalidate from being skipped by the distance filter.
        Self {
            mask: 0,
            last_validate: (i32::MIN, i32::MIN, i32::MIN),
            last_compass: 0,
            in_active_siege: false,
        }
    }
}

impl ZoneFlags {
    pub fn contains(&self, kind: crate::data::zone_data::ZoneKind) -> bool {
        self.mask & kind.bit() != 0
    }
}

/// Last position/heading the client reported via `ValidatePosition`
/// (Java `Player._clientX/_clientY/_clientZ/_clientHeading`).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ClientPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
}

/// STR/DEX/CON/INT/WIT/MEN (player-only for now — NPC base stats stay on the
/// template until something buffs them). Inputs to the stat finalizers and
/// the regen bonuses.
#[derive(Component, Debug, Clone, Copy)]
pub struct BaseStats {
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int_: i32,
    pub wit: i32,
    pub men: i32,
}

/// Party membership — **present only while in a party**; the value keys
/// `World.parties`. The party's member list is the authority on membership,
/// this is the O(1) back-pointer (Java `Player._party`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyRef(pub u32);

/// What a `PendingRequest` is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// An `AskJoinParty` is on the target's screen; answering joins this party.
    PartyInvite { party_id: u32 },
    /// A `FriendAddRequest` is on the target's screen.
    FriendInvite,
    /// An `AskJoinPledge` is on the target's screen; answering joins the
    /// inviter's clan. `pledge_type` rides along (Java keeps it on the stored
    /// `RequestJoinPledge` packet) — only 0 (main pledge) is accepted until
    /// sub-units land (G18 slice 6).
    ClanInvite { clan_id: i32, pledge_type: i32 },
}

/// The one outstanding transaction-request slot — **present only while a
/// request is in flight**, on *both* sides (Java splits this across
/// `Player._requests`, `_activeRequester` and `_requestExpireTime`; one slot
/// covers them because a busy player answers "C1 is on another task" either
/// way). Cleared by the answer, the `RequestTimeout` task (seq-guarded), or
/// either side leaving the world.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRequest {
    pub kind: RequestKind,
    /// The other side (target for the requestor, requestor for the target).
    pub other: i32,
    /// True on the side that must answer (got the Ask/FriendAddRequest).
    pub answerer: bool,
    pub seq: u64,
}

/// Friend-list snapshot (`Player._friendList` + the name/level/class data
/// Java pulls from `CharInfoTable`), loaded with the character. Online
/// status is always read live from `World`, never from here.
#[derive(Component, Debug, Clone, Default)]
pub struct Friends(pub Vec<crate::character::FriendInfo>);

/// Quest progress (Java `Player._quests`), keyed by quest name — the same
/// key the `character_quests` rows and the `Quest <Name> …` bypasses use.
/// Loaded with the character; mutated only through the quest engine
/// (`game_loop/quests.rs`), which mirrors every change to the DB.
#[derive(Component, Debug, Clone, Default)]
pub struct Quests(pub std::collections::HashMap<String, crate::model::quest::QuestState>);

/// Live quest-timer generations, keyed by `(quest name, timer name)` — the
/// cancellation side of `ScheduledTask::QuestTimer` (a fired task whose seq
/// no longer matches is stale). Starting a timer bumps the seq; so does
/// cancelling (Java's `QuestTimer.cancel`). Not persisted, like Java.
#[derive(Component, Debug, Clone, Default)]
pub struct QuestTimerSeqs(pub std::collections::HashMap<(&'static str, String), u64>);
