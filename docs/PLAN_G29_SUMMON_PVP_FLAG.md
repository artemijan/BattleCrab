# G29 slice 21 — a summon's attack flags its owner

Continuing the `Creature`-vs-`Player` sweep. This probe found the one with
teeth.

## The gap

Java `Creature.doAttack`:

```java
final Player player = getActingPlayer();
if (player != null && !player.isInsideZone(PVP) && player != target) {
    AttackStanceTaskManager.getInstance().addAttackStanceTask(player);
    player.updatePvPStatus(target);
}
```

`getActingPlayer()` on a `Summon` returns its **owner**. The port had no
equivalent, and its flag/stance block sat inside a player-only `else` branch —
so **a summon attacking a player flagged nobody**.

That is exploit-shaped rather than cosmetic: a player could set their pet on
someone and never go purple, leaving the victim unable to retaliate without
taking the karma themselves.

## Two fixes, and why the first wasn't enough

1. Added `pvp::acting_player` — a player is their own, a summon's is its owner —
   and resolved inside `update_pvp_status_target`, so every flagging path gets
   the summon case for free rather than each call site remembering.
2. **That alone did not work.** The end-to-end test still failed: the whole
   flag/stance block was in the `else` of `if is_npc_oid(attacker_oid)`, so a
   summon's swing never reached it. The block now runs for both branches, gated
   on the *resolved* actor being a player.

The unit test (calling the helper directly) passed after fix 1. Only the
end-to-end test — driving a real `do_auto_attack` — caught that the attack path
never called it. **A helper being correct proves nothing about whether the code
under test reaches it**, the same lesson as the pet-exp reward path.

## The guard that makes it safe

Moving the block out of the player-only branch is only safe because
`acting_player` resolves a plain monster to *itself*, and a monster is not a
player. A test pins that a mob attacking a player still flags nobody — without
it, the obvious regression from this change would be every mob flagging its
victim.

## Tests

`servitor_tests` 99 → 103, and the pvp/duel/combat/social groups re-run clean
(12/11/39/23).

- The helper resolves a summon to its owner (written **failing first**, then
  fixed).
- A real summon swing flags the owner end-to-end.
- A summon swing puts the *owner* in combat stance — it is the owner's stance
  that gates their sit/logout, not the summon's.
- A plain monster still flags nobody.

## Still open

Sweep remainder: `Reuses` and `TargetRef` against summons. Then `PET_EQUIP`
paperdoll, pet spiritshots, evolution, reconnect resummon, servitor master-buff
inheritance, `ServitorSkillUse`.
