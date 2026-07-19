# Buff persistence across relog (`character_skills_save`, `restore_type = 0`)

Buffs now survive a logout and come back on the next login. This closes the
half of Java `Player.storeEffect`/`restoreEffects` that G13.9 and G17 left
deferred (they shipped the `restore_type = 1` skill-reuse half).

## The rule that drives the design

**A buff's countdown is frozen while the character is offline; a cooldown's is
not.** Both live in `character_skills_save`, but they store time in opposite
ways, and that asymmetry is the whole feature:

| | reuse row (`restore_type=1`) | buff row (`restore_type=0`) |
|---|---|---|
| stores | absolute `systime` (wall-clock end) | relative `remaining_time` (seconds) |
| offline gap | decays the cooldown | consumes none of the buff |
| restored by | `PlayerData::restore_reuses` | `effects::restore_persisted_buffs` |

This is Java-faithful, not a shortcut: `restoreEffects` hands the stored
`remaining_time` straight to `skill.applyEffects(this, this, false,
remainingTime)` as a custom `abnormalTime`, never comparing it against the
clock. Log out with 20 minutes of Wind Walk and you log back in with 20
minutes, next day or not.

## Store path

`net::buffs_to_save` (called from `build_save_data`, so it covers all four
flush triggers: autosave, logout, class transfer, shutdown save-all) turns the
live `Buffs` component into `db::SkillBuffRow`s, gated by `StoreSkillCooltime`
like the reuse half. Java's skip list is reproduced:

- dances/songs unless `AltStoreDances` (new config; retail drops them);
- toggles — modelled here as the `u64::MAX` no-expiry sentinel, which is also
  what a 0-`abnormalTime` skill looks like;
- `LIFE_FORCE_OTHERS` (heal-over-time herbs);
- one row per skill id, first wins (Java dedupes on `getReuseHashCode()`);
- plus, Rust-specific: `passive` stand-in entries (the grade-penalty stat
  pumps), which enter-world re-derives via `refresh_expertise_penalty` — storing
  them would double-apply the pump.

Buff and reuse rows share one `buff_index` sequence, like Java's `++buffIndex`.

## Restore path

Split across the two places a character exists:

1. **char-select** (`lobby.rs`): `PlayerData::restore_buffs` parks the loaded
   rows on the bundle as `pending_buffs`. No time arithmetic — the value is
   used verbatim.
2. **enter-world**: the rows are taken off the bundle before `spawn_into`
   consumes it, then applied by `effects::restore_persisted_buffs` once the
   entity exists (a buff drives stats, the expiry scheduler and client packets,
   so it needs a spawned character). It runs after the passive pumps and before
   the spawn broadcast, so nearby players get a `CharInfo` already carrying the
   buffed speed and visuals.

To apply a buff without re-firing the skill's damage or heal, the continuous
half of `apply_skill_effects` was split out as
`effects::apply_continuous_effects(.., abnormal_time_override)`. That mirrors
Java's `instant = false` argument, which is exactly why the two halves are
separable there too. Restores are self-cast (`effector == effected`), so the
debuff resist roll is skipped — a debuff that was up at logout comes back
rather than getting a second chance to resist.

## Edge case worth knowing

A client that disconnects while still `Entering` never spawned, so its buffs
exist only as `pending_buffs`. That save path writes them back verbatim;
running them through `buffs_to_save` (which reads the empty live `Buffs`
component) would silently wipe the buffs of anyone who dropped between
char-select and enter-world.

## Known gap

Java also skips `isDeleteAbnormalOnLeave()` skills in `storeEffect`. That flag
isn't parsed into `Skill` yet, so such a buff currently survives a relog it
shouldn't — marked `TODO(G22)` at the filter.

## Tests

3 added:

- `char_persistence::active_buffs_persist_with_frozen_countdown` — DB
  round-trip; `remaining_time` returns verbatim, dead rows filtered,
  `buff_index` order preserved, buff and reuse rows don't bleed into each
  other's load.
- `skills_tests::buff_survives_relog_without_offline_countdown` — a buff with
  30 s burned saves as 70 s remaining and restores to exactly 70 s off the new
  tick after a 10 000 s "offline" gap.
- `skills_tests::dances_and_toggles_are_not_stored_by_default` — the
  `AltStoreDances` gate and the toggle skip.
