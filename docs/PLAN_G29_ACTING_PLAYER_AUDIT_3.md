# G29 slice 24 — closing the `getActingPlayer()` audit

The last two flagged sites, and a robustness fix on the pattern itself.

## The remaining sites were already fixed — by accident

Clan-war kill counting and the clan-war exp-penalty relief both read
`killer_oid` in `player_do_die`. Slice 23 introduced a **shadowing**
`let killer_oid = acting_player(...)` part-way down that function, which
happened to cover both, because nothing between them used the raw id.

That is coverage by luck. Any code added *above* the shadow would silently have
used the unresolved id, with no signal — the same class of latent breakage the
audit has been finding, now built into the fix.

The resolution is hoisted to the **top of the function**, where insertion order
cannot defeat it, with a comment saying why it lives there.

## What was at stake

- **Clan-war kill counting** — a kill by the enemy's pet still scores for the
  war.
- **Death exp penalty** — Java quarters the loss when the killer is a clan-war
  enemy. Unresolved, a summon killer has no clan, so the victim paid **four
  times** the exp they should for dying to an enemy's pet.

Both now pinned by tests, precisely because accidental coverage is invisible
when it breaks.

## Audit closed

| site | slice | was broken? |
|---|---|---|
| PvP flagging | 21 | yes |
| Reward attribution (exp/drops/quest credit) | 22 | yes |
| PK/karma | 23 | yes |
| Duel lethal guard | 23 | yes |
| Clan-war kill counting | 24 | covered accidentally, now pinned |
| Clan-war exp relief | 24 | covered accidentally, now pinned |

**Four genuine bugs from four probes**, plus two sites made robust. The
remaining Java `getActingPlayer()` call sites are event dispatch
(`OnAttackableKill`'s `killer.isSummon()` flag), which this port has no
equivalent of — noted rather than ported.

## The generalisable finding

Java's object model puts behaviour on `Creature` and resolves actors through
`getActingPlayer()`. This port grew the player paths first and expressed the
same rules as "is this a player". Every place those two differ is a silent
divergence: it compiles, it runs, and it is wrong only for summons — which no
existing test exercised.

The lesson is not "check summons" but: **when the reference implementation
routes through a resolver, port the resolver, not the common case.**

## Tests

`servitor_tests` 107 → 109; clan/death/pvp/duel groups re-run clean
(57/26/12/11).

## Still open in G29

`Reuses`/`TargetRef` summon probes, `PET_EQUIP` paperdoll, pet spiritshots,
evolution, reconnect resummon, servitor master-buff inheritance,
`ServitorSkillUse`.
