# G16 (slice) — Henna / dye symbols

Vertical slice of G16 (Character variables, premium & vitality): the henna
subsystem — the "henna changes stats" clause of the milestone gate. Ground
truth: `HennaData`, `model/item/Henna`, `Player.addHenna`/`removeHenna`/
`getHennaEmptySlots`/`recalcHennaStats`, the `RequestHenna*` packet family, and
`ai/others/SymbolMaker`.

## Scope decision
Interlude hennas are **permanent** (`duration = -1`) and carry six base-stat
bonuses (STR/CON/DEX/INT/MEN/WIT). So the timed-henna scheduler, the
`HennaDuration*` `character_variables`, LUC/CHA stats, and dye `<skill>` grants
are all out of scope (none present on this dist).

## The stat-folding approach
A worn dye is a permanent base-stat modifier, exactly like the class template.
So its bonus is **folded straight into the `BaseStats` component**
(`base = template + Σ worn-dye stats`), recomputed on draw/remove. The only two
`BaseStats` readers — `recalculate_stats` (finalizers) and `user_info` (the
STR/… panel) — then pick henna up with no special-casing.

## Pieces
- `data/henna_data.rs` — `HennaData` (372 dyes from `hennaList.xml`: 6 stats,
  wear/cancel count+fee, allowed class ids) + `HennaStatSums::stat_sums`.
- `HennaSlots([Option<i32>;3])` component in `PlayerData`; `from_char` restores
  it and folds the sums into `BaseStats`. `character_hennas` load + persist
  (delete+reinsert in the store transaction).
- Packets: server `HennaInfo` (0xE5), `HennaEquipList` (0xEE), `HennaRemoveList`
  (0xE6), `HennaItemDrawInfo` (0xE4), `HennaItemRemoveInfo` (0xE7); client
  opcodes 0x6F/0x70/0x71/0x72/0xC3/0xC4.
- `game_loop/henna.rs` — draw/remove windows, per-dye previews, draw
  (class/count/adena/empty-slot gates → consume → recompute `BaseStats` +
  finalizers + UserInfo/HennaInfo), remove (adena-fee gate → refund cancel
  count). Empty-slot count uses class level (`*_CLASS_GROUP` categories → 0/2/3
  slots). SymbolMaker `Draw`/`Remove` bypass verbs; `HennaInfo` in the
  enter-world burst.

## Gate
Draw a dye at a Symbol Maker → STR/CON change and survive relog; remove it →
stats revert, dyes refunded.

## Remaining G16 (follow-on milestones)
`character_variables` key/value store; full vitality (points↔level, peace-zone
regen, consumption); premium gameplay effects (the admin flag already persists).
