# G29 slice 26 — a pet that was out at logout comes back

Slice 7 wrote the `pets.restore` column as a hard-coded `'false'` with a
`TODO(G29)`. This honours it.

## Live content, not an opt-in

`Character.ini` on this dist:

```
RestoreServitorOnReconnect = True
RestorePetOnReconnect = True
```

Both **True**, so a summoner logging back in expects their pet standing next to
them. Checking the config first is what made this the pick over the other
remaining items — and the same check struck pet evolution last slice.

## The flag is set where the truth is known

`restore` means *"the pet was out when the owner left"*. The natural place to
decide that is `sync_pet_row`, which `on_owner_leave_world` already calls
**before** the unsummon — precisely so it observes a live pet. Setting it there
means no separate logout hook, and no way for the two to disagree.

A pet deliberately put away mid-session has its row synced while it is still
out, then cleared on the explicit unsummon — so it stays in its collar, which is
what a player who dismissed their pet expects.

## Restoring reuses the normal summon path

`restore_pet_on_login` sets `pending_pet_collar` and calls `summon_pet` — the
same path a mid-session re-summon takes. A restored pet is therefore identical
to a freshly summoned one: same stats, same feed clock, same packets, same
state read from the saved row. A parallel "restore" path would be a second
place to keep in sync.

Guarded on the collar still being in the inventory: it can be traded or
destroyed between sessions, and setting the holder for a collar that is gone
would leave it dangling into an unrelated cast.

## Tests

`servitor_tests` 114 → 118, `char_persistence` extended.

- A pet out at logout comes back, **in the state it left in** (the food bar is
  checked, not just its existence).
- A pet put away first stays away.
- A missing collar restores nothing *and* leaves no dangling holder.
- The config flag is honoured rather than assumed.
- The real-schema test now round-trips `restore` both ways — it is a
  string column (`'true'`/`'false'`) in Java, which a bool binding would
  quietly get wrong.

## Still open in G29

**Servitor** reconnect (`character_summons` table — schema is present, and it
needs the summoning skill id rather than a collar), pet spiritshots (needs pets
to cast), servitor master-buff inheritance, `ServitorSkillUse`.
