# G23 slice 22 — Dr. Chaos (the paranoia transformation)

## Why

The last unported `ai/bosses` script. The Gigantic Chaos Golem 25512 has a
`grandboss_data` row but no AI at all: nobody can trigger the fight, and a
stored CRAZY/DEAD golem never resolves at boot. The golem has **no config
respawn window**, so the shared grand-boss lifecycle skips it entirely
(`on_grand_boss_killed`/`resolve_at_boot` both early-return for 25512) — Dr.
Chaos owns its own status, kill and boot.

## The mechanic

Dr. Chaos (32033) is a small paranoid NPC. A **pissed-off timer** starts at
30 on spawn; every second, for each living player within 500 units, it drops
by 1 (talking to him drops it 1–5 more). At 15 he barks a warning; at ≤0 he
**becomes the Gigantic Chaos Golem** through a short cinematic. So *lingering
near Dr. Chaos is what spawns the boss* — that is the encounter.

The golem, once up, **despawns after 30 minutes with no attack** (reverting to
Dr. Chaos), or on death sets a `(36 ± 24)h` respawn that brings Dr. Chaos
back. The status field (on the golem's grand-boss record) is DrChaos's own
three-state ladder: `NORMAL 0` (Dr. Chaos NPC up), `CRAZY 1` (golem up),
`DEAD 2` (killed, awaiting reset).

## Port

- **`game_loop/dr_chaos.rs`** — the whole state machine.
  - `resolve_at_boot`: NORMAL/absent → spawn Dr. Chaos; CRAZY → respawn the
    golem with stored HP + arm the idle-despawn; DEAD → arm the reset at the
    remaining window, or respawn Dr. Chaos now if it elapsed while down. (The
    third case is the boss-lifecycle "elapsed during downtime" trap again.)
  - `DrChaosParanoia` (1 s, self-rescheduling while NORMAL): per-nearby-living-
    player decrement, the =15 warning bark, the ≤0 transform.
  - `become_angry` → `DrChaosTransform` beats at 2/4/6.5/12.5/17 s
    (SocialAction + SpecialCamera, transcribed), the 17 s beat deleting Dr.
    Chaos and spawning the golem with its camera/social/`Rm03_A` sound, then
    arming the idle-despawn. Dr. Chaos's cosmetic "walk to the grotto" is a
    teleport (the port teleports scripted bosses elsewhere too).
  - `DrChaosGolemDespawn` (60 s): if idle > 30 min, delete golem → spawn Dr.
    Chaos → NORMAL; else reschedule. Attacks refresh `last_attack_tick`.
  - `on_golem_killed` (from `death.rs`): bark, DEAD, persist a `(36±24)h`
    window, arm `DrChaosReset` (→ spawn Dr. Chaos, NORMAL).
  - `on_golem_attacked` (from `combat.rs`): refresh the idle clock; a
    `rnd(300) < 3` taunt bark.
- **`scripts/dr_chaos_talk.rs`** — first-talk on 32033: the timer decrement +
  the three paranoia htmls (inline, as Java writes them), or the transform.
- **`npc_say_text`** — a literal-string `NpcSay` (`npcString = -1` + the text),
  the variant `broadcastSay(type, String)` uses. The existing `npc_say` only
  does client-localized string ids; DrChaos's lines are literal English.
- New components `DrChaosState { pissed_off }` (on Dr. Chaos) and
  `DrChaosGolem { last_attack_tick }` (on the golem); 4 scheduler tasks.

## Tests (`dr_chaos_tests`)

1. Boot: NORMAL → Dr. Chaos stands; CRAZY → golem stands + idle clock; DEAD
   with elapsed window → Dr. Chaos respawns.
2. Paranoia: a nearby player drains the timer; at ≤0 Dr. Chaos is gone and the
   golem stands (status CRAZY). No nearby player → no drain. A dead player
   doesn't drain.
3. First-talk drains and, at ≤0, transforms; the htmls track the bands.
4. Idle despawn: 30 min without an attack reverts to Dr. Chaos; an attack
   inside the window keeps the golem. The attack refreshes the clock.
5. Kill: DEAD + a reset armed; the reset respawns Dr. Chaos at NORMAL. The
   count of the transform's cinematic beats is armed once.
6. `npc_say_text` wire: `-1` then the UTF-16 text.
