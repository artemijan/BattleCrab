# G19 — SilentMove + FakeDeath

## Why this slice

The unconsumed-stat sweep (added after the `StatByMoveType` slice) came back
**clean** this time — all 44 `Stat` variants now have real consumers — so this
went back to the name ranking, which left a two-way tie at 4 learnable:
`MagicalAttackMp` and `SilentMove`.

`SilentMove` won, and then pulled a second effect in with it:

- Its four skills (Silent Move 221, Stealth 411, Dance of Shadows 366, Fake
  Death 60) all *land* today — they carry other, ported effects — but their
  **headline mechanic** does nothing. Silent Move and Stealth exist purely to
  walk past aggressive monsters; that failed 100% of the time. The aggro scan
  even carried a literal comment admitting it:
  `// invisibility/silent-move/GM states don't exist`.
- **Fake Death 60 carries only `FakeDeath` + `SilentMove`**, both unported, so
  it parsed to an empty effect list and was **dropped whole** — the skill cast
  and did nothing at all.

`FakeDeath` is 1 more learnable skill, and Java reads the two flags on
**adjacent lines of the same method** (`AttackableAI.isAggressiveTowards`,
lines 128 and 144), so splitting them would have meant touching that function
twice. 5 learnable skills total.

## What Java does

```java
// isAggressiveTowards
if (target.isAlikeDead()) return false;                       // Player overrides to include isFakeDeath()
if (target.isPlayable() && !me.isRaid() && !me.canSeeThroughSilentMove()
    && ((Playable) target).isSilentMovingAffected()) return false;
...
if (player.isRecentFakeDeath()) return false;                 // grace after standing up
```

`SilentMove` is a pure state flag — empty constructor, nothing but
`getEffectFlags`. `FakeDeath` is a flag plus an MP upkeep with the same
`power * getTicksMultiplier()` shape as `ManaDamOverTime`, plus
`startFakeDeath()`/`stopFakeDeath()` for the client-side pose.

Three findings that narrowed the port, each checked rather than assumed:

- **`canSeeThroughSilentMove()` is always false** — `setSeeThroughSilentMove`
  has no callers anywhere in the Java tree. So only the raid exemption is real.
- **`PlayerFakeDeathUpProtection = 0`** on this dist's `Character.ini`, so
  `isRecentFakeDeath()` can never read true. Not ported.
- **`FakeDeathUntarget = False`**, so the "clear the fake-dead player off
  everyone's target" block never runs. Not ported.

`FakeDeathDamageStand = True` is the one that *is* live: taking damage while
playing dead ends the act.

## What landed

- **`effect_flag::SILENT_MOVE` / `FAKE_DEATH`** + the two `SkillEffect`
  variants and their parse arms. `FakeDeath { power, ticks }` joins the
  existing DoT tick chain rather than growing its own scheduler — Fake Death 60
  is a toggle, so it also inherits the out-of-MP self-deactivate.
- **`npc_ai::notices_target`** — the ported gate, applied at all three
  player-scan sites (generic monster scan, guard PK scan, siege-guard scan).
  Applied as a `retain` *after* the sweep, because the sweep closure holds
  `objects` mutably while the flag lookup needs it shared — the same shape the
  siege branch already used.
- **`server_packets::change_wait_type`** (0x29, new) and the
  `broadcast_change_wait_type` / `stop_fake_death` helpers. Standing up sends
  `ChangeWaitType(WT_STOP_FAKEDEATH)` **and** `Revive`, as Java does.
- **`break_fake_death_on_damage`** at `apply_physical_damage`'s player branch —
  the one damage choke point this port already funnels everything through.

## Tests

`game_loop::tests::stealth_tests` (10). Worth naming:

- `an_unhidden_player_is_noticed` — the **baseline**. It failed on first run,
  which revealed that two of the "stealth works" tests had been passing
  vacuously: `NpcAi.global_aggro` starts at −10 and creeps up 1 per think tick,
  so a monster needs ~100 game ticks before its scan runs at all. Now a named
  `AGGRO_WARMUP` constant with the explanation attached.
- `a_raid_boss_sees_through_silent_move` — the `!me.isRaid()` exemption.
- `a_fake_dead_player_is_ignored_even_by_a_raid_boss` — fake death goes through
  `isAlikeDead()`, which has **no** raid exemption, unlike stealth. The
  asymmetry is easy to get wrong.
- `a_stealthed_pk_slips_past_a_guard` — guards run the same method, so the
  guard-specific scan needed the gate too.
- `damage_breaks_fake_death` / `a_zero_damage_hit_does_not_break_fake_death` —
  Java gates on `amount > 0`, so a missed swing must not stand you up.
- `real_dist_fake_death_parses_both_halves` — the skill that was dropped whole.

Implementation note: `handle_buff_expire` reads the `FAKE_DEATH` flag off the
**expiring buff**, not the skill template. That is both more robust (works for
a buff whose skill row isn't loadable) and consistent with how `Fear`'s
`onExit` and `break_fake_death_on_damage` already source it.

## Deferred (not this slice)

- **`ChameleonRest`** (ORs `SILENT_MOVE | RELAXING`) — its only skill, Sneak
  39102, is non-learnable and needs the unmodeled sitting state.
- **`Hide`** (skills 922/14488) — a separate effect, non-learnable here.
- **`isRecentFakeDeath`**, **`FAKE_DEATH_UNTARGET`** — config-disabled on this
  dist, per above.
- The `RequestRestartPoint` / `RequestActionUse` fake-death gates — small, but
  they belong with those packets rather than here.
- **`MagicalAttackMp`**, the other half of the tie.
