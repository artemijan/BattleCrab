# G16 — Character variables, vitality & premium effects

The closing slice of **G16**. The milestone's earlier halves landed already:
the admin points menu ([PLAN_G16_ADMIN_POINTS.md](PLAN_G16_ADMIN_POINTS.md))
and henna/dye symbols ([PLAN_G16_HENNA.md](PLAN_G16_HENNA.md)). What remained
was the milestone's namesake trio — the `character_variables` store, the
vitality system, and what a premium account actually *does*.

Java sources: `PlayerStat` (the vitality pool + the exp/sp bonus multipliers),
`Attackable.onKill`/`getVitalityPoints` (what a kill costs and the premium
rates), `PlayerVariables`/`AbstractVariables` (the key/value store),
`EnterWorld` (the `ExVitalityEffectInfo` block), `DailyTaskManager` (the
refills), `AdminVitality`.

Dist config (the spec): `Character.ini` `EnableVitality = True`,
`StartingVitalityPoints = 0`, `RaidbossUseVitality = False`; `Rates.ini`
`RateVitalityExpMultiplier = 2`, `RateVitalityGain/Lost = 1`,
`VitalityMaxItemsAllowed = 999`; `NPC.ini` `VitalityConsumeByMob = 2250`,
`VitalityConsumeByBoss = 1125`; `Custom/PremiumSystem.ini`
`EnablePremiumSystem = True`, `PremiumRateXp/Sp = 2`.

---

## 1. `character_variables` — the per-character key/value store

`PlayerVariables` is Java's general-purpose character scratchpad. Ported as a
plain `PlayerVariables(HashMap<String, String>)` component
(`model/components.rs`) with Java's `getInt`/`set` accessors; values stay
strings and parse on read, like Java's `StatSet`.

- **Load:** `db::load_variables` (`SELECT var, val FROM character_variables
  WHERE charId = ?`) → `CharData.variables` → the component in
  `PlayerData::from_char`.
- **Persist:** `PlayerSaveData.variables`, flushed delete-then-reinsert inside
  the existing store transaction. Java's `AbstractVariables` dirty flag is not
  ported — the memory-first autosave already batches the write, so there is
  nothing to gate.
- **Keys used today:** only `VITALITY_ITEMS_USED`. Java's remaining key set
  (instance origin/restore, UI key mapping, ability points, auto-use settings)
  belongs to unported subsystems and will arrive with them.

## 2. Vitality (`game_loop/vitality.rs`)

The pool is `characters.vitality_points`, clamped to `0..=140_000`
(`MAX_VITALITY_POINTS`/`MIN_VITALITY_POINTS`, now shared constants in
`model/mod.rs` so the config loader and the stat code agree).

- `set_vitality_points(value, quiet)` — clamp, store, and notify: the
  increased/decreased line, the at-maximum / fully-exhausted edge line (both
  suppressed when `quiet`), then — *regardless* of `quiet`, as in Java —
  `ExVitalityPointInfo`, `broadcastUserInfo`, and the party window's
  `VITALITY_POINTS` field.
- `update_vitality_points(delta, use_rates, quiet)` — the signed-delta entry
  point, through `RateVitalityGain`/`RateVitalityLost`, with Java's
  `isLucky()` exemption (level ≤ 9 + the Lucky skill 194) and the
  `EnableVitality` master gate.
- `vitality_exp_bonus` / `exp_bonus_multiplier` — the reward multiplier.
  Deliberately all-or-nothing: Java tests `getVitalityPoints() > 0`, so one
  remaining point buys the full ×2 and an empty pool buys nothing.
- `kill_vitality_delta` — `Attackable.getVitalityPoints`. Below level 85 the
  divisor is a hard-coded 1000, which is *every* character on an Interlude
  server, so `VitalityConsumeByMob`/`ByBoss` are ported but dormant. The int
  cast truncates before `max(…, 1)`, so any positive-exp kill costs ≥ 1 point.

**Where it plugs in.** `death::add_exp_and_sp` gained Java's third argument,
`use_bonuses`: the kill path passes true (the reward is multiplied and the
acquisition SystemMessage reports the surplus in its bonus slots — that's the
client's floating "+N XP bonus"), while quest rewards and `//add_exp_sp` use
the two-argument overload's `false`. Amounts stay `f64` until the final round,
as in Java, so the bonus never compounds a rounding error.
`death::consume_kill_vitality` then charges the killer, in both the solo and
the party branch (Java charges each rewarded member on their post-cutoff xp).

## 3. Premium effects (`config/premium.rs`)

`Custom/PremiumSystem.ini` now has a real loader, replacing the inlined
`PREMIUM_SYSTEM_ENABLED = true` constant the community-board slice left behind.
`has_premium_status` ports `Player.hasPremiumStatus()`; because this port keeps
the whole `account_premium` table in `World.premium` rather than caching a flag
on the player at login, a `//premium_add`/`remove` on an online account takes
effect immediately instead of at next login.

`PremiumRateXp`/`PremiumRateSp` are applied on the kill reward path *before*
the vitality/skill bonus multiplier, matching `Attackable.onKill` and
`Party.calculateExpSpPartyCutoff`.

> **Behaviour note:** `PremiumConfig::default()` is Java's default —
> `enabled = false`. Worlds built from `CombatConfig::default()` (tests) now
> have premium off unless they opt in; the real server reads `True` from the
> dist ini.

## 4. Enter-world & creation

- `ExVitalityEffectInfo` carries real values now (it was three hard-coded
  zeroes): the pool, the bonus, and the remaining/allowed vitality-item counts
  from `VITALITY_ITEMS_USED` + `VitalityMaxItemsAllowed`. The bonus field
  reproduces Java's `(int) getVitalityExpBonus() * 100` precedence quirk — the
  *truncated* multiplier is scaled, so ×2.0 → 200 and a hypothetical ×2.5 would
  also send 200. Gated on `EnableVitality`, like Java's.
- `ExVitalityPointInfo` (0xA1) is new — the running-pool push.
- Character creation seeds `min(StartingVitalityPoints, MAX)` when the system
  is on (0 on this dist: a fresh character starts drained).
- `//set_vitality`/`//full_vitality`/`//empty_vitality` now run through the
  module (Java's `AdminVitality` passes `quiet = true`), replacing the
  handler's private copy of the setter and its duplicated constants.

## 5. Fixed along the way (pre-existing)

- **`HennaInfo` was sent twice, and late.** The G16 henna slice added a real
  `send_henna_info` *after* the welcome message while the pre-henna empty stub
  stayed in the burst. Java sends it once, inside the burst, ahead of the
  welcome. The payload build is now split into `henna::henna_info_packet` so
  the burst can send the real panel from the `Entering` bundle, and the
  post-welcome duplicate is gone. This was breaking `e2e_create` on `main`.
- **`char_persistence` did not compile on `main`** (the henna/crafting merges
  added `PlayerSaveData` fields without updating the fixture), and four of its
  cases then failed because the tests' hand-rolled schema omitted
  `character_hennas`/`character_recipebook`/`character_variables` — the store
  transaction wrote to them, errored, and rolled the whole flush back.

## 6. Deferred

- **`TODO(G33)` — the daily/weekly refills.** `DailyTaskManager` tops vitality
  up by 25 % daily and refills it fully weekly, at 06:30. Both need the
  wall-clock daily-task scheduler G33 brings; `reco.rs`'s
  `schedule_initial_daily_reset` is the pattern to reuse. **Until then vitality
  only ever drains** — a character who burns through it gets it back only from
  `//set_vitality` or a fresh character.
- **`TODO(G19)` — the unmodelled stats.** `VITALITY_CONSUME_RATE` (per-player
  scaling on the consumed amount) and `BONUS_EXP`/`BONUS_SP` (skill/item exp
  bonuses) read as their identities; the sites are marked.
- **`TODO(G16)` — vitality items.** The `VITALITY_ITEMS_USED` counter is
  stored, persisted and reported, but nothing increments it yet: the
  vitality-restoring item handlers are not ported.
- **`TODO(G16)` — PC-café.** `PcCafePointsManager.givePcCafePoint` on kill
  (`PC_CAFE_RETAIL_LIKE`) is still unported; the points store itself exists.
  The per-item-id premium drop tables
  (`PremiumRateDropChanceByItemId`/`…AmountByItemId`) are likewise unread — the
  flat rates are ported.
- **Fishing-rod exp bonus** (`addExpAndSp`'s `FANCY_FISHING_ROD_SKILL` ×1.5
  branch) → G32.

## 7. Gate

The milestone gate — *"a premium flag and vitality level survive relog; henna
changes stats"* — is met: henna landed earlier, premium persists in
`account_premium` (and now has gameplay effect), and vitality persists in
`characters.vitality_points` across the autosave/logout flush.

**Tests** (`game_loop/tests/vitality_tests.rs`, 12 cases): the clamp + the four
notification lines, the quiet path (messages suppressed, gauge still pushed),
the no-op set, the delta floor, the `EnableVitality` gate, the gain/lost rates,
the all-or-nothing bonus, the ×2 reward with its bonus SystemMessage, the
quest-reward opt-out, the sub-85 kill-cost formula with its level-gap floor and
1-point minimum, and an end-to-end monster kill draining the killer's pool.
