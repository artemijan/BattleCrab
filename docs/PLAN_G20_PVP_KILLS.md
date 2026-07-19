# G20 — PvP kill consequences: counters, karma and zone exemptions

The third **G20** slice, and the one that closes the milestone's gate:
*"a bow attack consumes an arrow, a polearm hits a line, **PvP flagging drives
auto-attack**, a physical skill lands."*

Java sources: `Player.doDie`'s reputation block, `Playable.checkIfPvP`,
`Formulas.calculateKarmaGain`, `Config.ReputationIncrease`.

---

## 1. What was actually missing

The gate clause reads "PvP auto-attack", and the *targeting* half was already
done — `is_player_auto_attackable` handles peace zones, arena/PVP zones, active
sieges, PK status and the flag, and the attack request path consults it. The
roadmap's fuller wording is what pointed at the gap: "PvP auto-attack **+ the
karma/PK/flag consumers**".

Those consumers were absent entirely: `grep` found no `pvp_kills`, `pk_kills` or
`reputation` handling anywhere in the death path, and `player_do_die` carried a
literal `let _ = killer_oid;` with a comment noting "the only killers are plain
monsters". Killing a player did nothing at all.

## 2. The three outcomes

`pvp::on_kill_update_pvp_reputation` ports Java's branch order exactly:

1. **A lawful PvP kill** — the victim was flagged or already a PK
   (`checkIfPvP`). The killer's `pvp_kills` rises and no karma is taken. Killing
   a PK within ±10 levels additionally returns `Config.ReputationIncrease`
   reputation — **0 on this dist**, so that branch is inert here, but it is
   ported so an operator who raises it gets retail behaviour.
2. **First offence** — a killer with *positive* reputation and no prior PKs has
   it reset to 0 rather than driven negative, and takes a `pk_kills`.
3. **A PK** — reputation drops by `calculateKarmaGain(pk_kills)` and the counter
   rises. Karma scales with the body count: 720 for the first, rising through
   two brackets to a flat 43 200 past 180 kills.

Nothing happens at all when either party is inside a **PVP zone** — Java's "Do
nothing when in PVP zone" short-circuit, so arena kills are free.

`checkIfPvP` was already present in a reduced form (used to shorten the
attacker's own flag) and has been extended rather than duplicated. Its remaining
Java legs need clan wars — a *mutual* war makes kills lawful — which is
`TODO(G18)`.

## 3. Death penalty in PvP and siege zones

Found while wiring the above: `player_do_die` applied the death XP penalty
unconditionally. Java skips it when `isLucky() || insidePvpZone || isOnEvent()`,
where `insidePvpZone` is `ZoneId.PVP` **or** `ZoneId.SIEGE`. Arena and siege
deaths are now free of XP loss, which is what makes those zones usable.

## 4. Tests

`game_loop/tests/pvp_kill_tests.rs`, 8 cases: the karma curve at all three
brackets and its monotonicity; each of the three kill outcomes, including a
second PK costing more than the first; killing a PK being lawful; the PVP-zone
exemption counting nothing; a monster killer moving no counters; and
`check_if_pvp`'s own classification of clean / flagged / PK / self targets.

## 5. G20's gate

All four clauses are now met — bow ammunition (slice 1), the polearm sweep
(slice 2), a physical skill landing (the earlier instant-damage work), and PvP
flagging driving auto-attack with real consequences (this slice).

**Still open in G20** (breadth, not gate): **overhit** XP, the `SHOTS_BONUS`
dynamic value, and **duels** (`DuelManager` — G25's olympiad reuses its shape).
Smaller leftovers: karma *decay* while hunting (`Formulas.calculateKarmaLost`,
which needs the per-level `KarmaData` multiplier — absent from this dist's
`data/`), PK item drops at high karma (`Config.KARMA_DROP*`), and the ranged
leftovers from slice 1 (bow peace-zone check, `CHEAPSHOT`, NPC-archer reuse).
