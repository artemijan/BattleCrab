# G29 slice 20 — summon buffs reach the client

## Running the sweep I said I'd run

Slice 19 closed admitting the "Java keeps it on `Creature`, the port put it on
`Player`" sweep had **not** actually been run — it found the missing `SUMMON`
target type by accident on the first probe.

Running it properly: the first two probes came up **clean**, and that is worth
recording rather than manufacturing a finding.

- `Buffs` *is* attached to NPCs (`npc.rs` spawn), so summons can hold buffs.
- `apply_buff_to_npc` exists and calls `recompute_npc_buffed_stats`, so a buff
  landing on a summon really does move its server-side stats.

A test now pins the second one end-to-end, because slice 19 only proved a
*heal* lands on a servitor — never that a **stat buff** changes its numbers.

## The real gap: the client is never told

The NPC buff path carries its own admission:

> "no `NpcInfo` re-broadcast, so a speed change isn't reflected client-side
> until respawn; the combat math uses it now."

For a random mob that is a tolerable narrowing. For a **servitor** it is a bug:
Servitor Haste (attack speed) and Servitor Wind Walk (movement speed) both land
in fields `PetInfo`/`SummonInfo` already carry, and both are cast by the owner
*expecting* to see the difference. The buff worked and looked broken — the worst
combination, because nothing is wrong to find.

So a summon (and only a summon) now re-sends `PetInfo` to its owner and
`SummonInfo` to everyone else when a buff lands **or expires**. The expiry half
matters just as much: without it the summon keeps displaying the buffed speed
after the buff is gone.

## The test was verified to fail without the fix

`buffing_a_servitor_refreshes_its_client_info` asserts the owner receives a
`PetInfo` (0xB2). I temporarily disabled `refresh_summon_info` and confirmed the
test fails, then restored it — otherwise a packet-presence assertion of this
shape can pass on some unrelated packet and prove nothing. Worth doing for any
test whose subject is "a packet was sent".

## Tests

`servitor_tests` 97 → 99: a stat buff moves a servitor's speed server-side, and
the owner is told about it.

## Still open

- The `Creature`-vs-`Player` sweep is now *started* but not exhausted: `Reuses`,
  `TargetRef`, `AttackState` and `PvpState` have not been checked against
  summons.
- `PET_EQUIP` paperdoll, pet spiritshots, evolution, reconnect resummon,
  servitor master-buff inheritance, `ServitorSkillUse`.
