# G21 slice 2 — town-guard PK aggro + faction help calls

Second G21 slice. Covers the **"a guard aggros a PK"** clause of G21's gate,
and adds the faction/clan-help calls that make a mob camp fight as a group.

## Data survey

| Fact | Number |
|---|---|
| `Guard` templates (town guards) | **186** |
| NPC templates carrying a `<clanList>` faction | **3760** |
| Total `<clan>` entries | 4569 |
| Templates with an `<ignoreNpcId>` list | 82 |
| Most common factions | `DOOR` 801, `FORTRESS` 387, `ALL` 238, `C_DUNGEON` 122, `ORC` 104 |

None of this was parsed before — `<clanList>` was dropped entirely, so every
mob fought alone.

## What landed

**Parsing** — `<ai><clanList><clan>` and `<ignoreNpcId>` into
`NpcTemplate.clans` / `ignore_clan_npc_ids`, plus `is_guard()` and
`shares_clan_with()` (`ALL` on *either* side matches everything — that's how
the 238 `ALL` NPCs pull a whole neighbourhood in). Verified against the real
datapack, not a fixture: the test asserts 3760 clan templates / 186 guards /
82 ignore lists, and the guard count matches an independent grep of the XML
exactly.

**Guard PK aggro** — `guard_aggro_scan` in `npc_ai.rs`, ported from the
`me instanceof Guard` branch of `isAggressiveTowards`. A guard seeds hate on
any player with **`reputation < 0`** within a **hardcoded 500 units**.

Two details worth stating because both look like bugs otherwise:
- The range is Java's bare literal, **not** the template's `aggroRange`
  (the Java source has a "TODO Make sure how guards behave towards players"
  note right beside it).
- It runs **regardless of `isAggressive`** — Java's guard branch never consults
  the flag.

A lawful player is ignored no matter how close, which is what makes this a
PK-hunting rule rather than general aggression. Both directions are tested.

**Correction (2026-07-19).** This slice originally claimed guards are flagged
*passive* in the datapack. **They are not** — all 186 stock `Guard` templates
carry `isAggressive="true" aggroRange="450"`. The claim was wrong in the plan,
in the code comment, and in the test fixture (which hardcoded
`is_aggressive = false`), so the whole slice's test suite agreed with an
assumption that the shipped data contradicts and guards were killing lawful
players on sight.

The generic aggro scan in `think_active` was gated only on `is_aggressive`, so
every guard seeded hate on every player inside 450 units. Java runs that scan
for `isAggressive() || instanceof Guard` too, but each candidate then has to
clear `isAggressiveTowards` → `isAutoAttackable`, and `Player.isAutoAttackable`
only returns true for an NPC attacker via **`attacker.isMonster()`**. A `Guard`
is an `Attackable`, not a `Monster`, so it falls through every playable branch
and returns false. The generic scan is therefore **monster-only**, and
`guard_aggro_scan`'s reputation rule is the *only* path by which a guard aggros
a player. The fix adds `is_monster()` to that gate; the fixture now mirrors the
real datapack so the regression is actually reachable.

**Lesson:** a fixture that encodes the same wrong assumption as the code under
test cannot catch that assumption being wrong. When a comment asserts a fact
about the datapack, check it against `dist/`.

**Faction help calls** — `faction_call`, from the faction block of
`thinkAttack`. An engaged NPC drags idle clan-mates within
`clanHelpRange + collision` into its fight. Three gates, each individually
tested because dropping any one of them silently over-aggros:

1. **Only if the target actually attacked *this* NPC** (Java's
   `getAttackByList`; the port's proxy is a non-zero `damage` entry in the
   aggro list). Without it, being merely *noticed* by one mob would pull the
   entire camp.
2. **Only idle/active clan-mates answer** — one already fighting is left alone.
3. **`ignoreNpcId`** beats a shared clan (82 templates use this).

Java also splits the hate: a *playable* target gets `EVT_AGGRESSION … 1` (a
nudge; the recruit picks its own target), anything else inherits the caller's
full hate. Both are ported.

## `Guard` had to be let into the AI at all

`think()` gated on `is_monster()`, and `Guard` isn't in the monster subtree —
so the new scan never ran and the first test failed. In Java `Guard extends
Attackable` and therefore runs the same `AttackableAI`; the gate now admits
guards alongside monsters and stationed siege `Defender`s. Worth noting the
port's type predicates don't map 1:1 onto Java's class hierarchy, so "which
subtree is this?" is worth re-deriving from the Java `extends` chain rather
than assuming an existing helper means the same thing.

## A test that was wrong rather than a bug

`a_clan_mate_already_fighting_is_left_alone` initially failed. The fixture set
the mate's intention to `Attack` with **no target**, which the AI resolves
straight back to `Active` on the next think — after which the mate legitimately
accepts the call. "Busy" has to be a real state, so the mate now has its own
live fight with the same player and the assertion is that the call adds no
further hate on top. The implementation was correct; the fixture described a
state the game can't be in.

## Deliberate narrowings (`TODO(G21)` at the site)

- Java fires `EVT_AGGRESSION` (a `Summon`-aware event) and an
  `OnAttackableFactionCall` script hook. The port seeds hate directly — there
  are no script listeners yet.
- `skillTargetReconsider` (from slice 1) is still narrowed: faction data now
  exists, so heal/buff-a-faction-mate is unblocked, but it isn't wired yet.

## Tests

12 new in `game_loop/tests/guard_aggro_tests.rs` — four on guard aggro (PK in
range; lawful player at the *same* distance; PK beyond 500; a non-Guard monster
doesn't inherit the rule) and eight on faction calls (mate pulled in; different
faction isn't; no call without a real attack; out of range; `ignoreNpcId`;
`ALL` matches anything; a busy mate is left alone), plus the dist-backed parse
test in `npc_data.rs`.

**647 lib tests green**, `char_persistence` 7/7, `e2e_create` 1/1.

## Next in G21

- **Minions** — templates parse them; nothing spawns them.
- **`DBSpawnManager`** — raid-boss HP across restart (the last gate clause).
- **NPC pathfinding** (the G7.85 worker for NPCs) and NPC regen.
- Wire `skillTargetReconsider` now that factions exist.
- The other ~33 zone types, fences (`FenceData`), `HtmCache`, walker routes,
  `CreatureSeeTaskManager`.
