# Shadow Weapon Exchange Coupons + shadow-item mana ✅ (2026-08-01)

**Symptom that started it:** a character finishes a class transfer, receives
15 Shadow Item Exchange Coupons — and has nowhere to spend them.

Two independent gaps sat behind that, one restored and one ported.

---

## 1. The exchange desk — restored, not ported

`dist` ships the coupons (8869 D-grade, 8870 C-grade) and the 238 shadow
weapons (8821+), but **not the desk that joins them**:

- `data/scripts/custom/ShadowWeapons/` does not exist in this dist,
- the three exchange multisells (306893001/2/3) are absent from the 82 that do
  ship,
- and the `<Button … _Quest ShadowWeapons>` line is **commented out** in all 81
  `data/html/villagemaster/*.htm`.

So the Java reference server has the same dead end. The pieces were restored
from the authentic Interlude datapack (`l2j_mobius/L2J_Mobius_CT_0_Interlude`),
byte-for-byte where possible:

| restored | from |
|---|---|
| `dist/game/data/multisell/306893001.xml` (9 D-grade weapons) | CT_0 datapack |
| `dist/game/data/multisell/306893002.xml` (10 C-grade) | CT_0 datapack |
| `dist/game/data/multisell/306893003.xml` (all 19) | CT_0 datapack |
| `dist/game/data/scripts/custom/ShadowWeapons/exchange_{d,c,both,no}.html` | CT_0 datapack |
| `crates/gameserver/src/scripts/shadow_weapons.rs` | `custom/ShadowWeapons/ShadowWeapons.java` |

Every id was validated against *this* dist before use: all 19 products, both
ingredients and all 80 `<npc>` entries resolve to real templates here.

The script is Java's whole `onTalk` — which coupons are in the bag picks one of
four pages, and the page carries the multisell link. The exchange rate is
1 coupon → 1 weapon.

**The button:** uncommented for the 78 masters present in *both* the script's
NPC list and this dist's htmls. Three htmls (30508, 30594, 31279) name a master
that appears in no multisell's `<npcs>` allow-list — upstream never wired those,
and uncommenting them would open a page whose exchange link then refuses, so
they stay commented. (30847 is the mirror case: on the script's list, but this
dist has no button in its html. Left alone — the html is the dist's word.)

---

## 2. Shadow-item mana — ported

Without this the exchange would hand out *permanent* free D-grade weapons.
`ItemTemplate` never parsed `<set name="duration">` and `ItemInstance
.mana_left` was hard-coded `-1` at every creation site — a persisted column
that nothing ever wrote, the same "stubbed field" shape as the old `curCp`.

- **`ItemTemplate.duration`** parsed (`-1` = not a shadow item). 1353 items
  declare it on this dist; within the Interlude id range they are the 238
  shadow weapons (90 or 300 minutes) plus the talismans.
- **`Inventory::add_item`** stamps `mana_left` from it, mirroring Java's `Item`
  constructors (`_mana = _itemTemplate.getDuration()`). `Item.isShadowItem()`
  is then simply `mana >= 0`, exactly as in Java.
- **`Inventory::insert_instance` gained a `mana` parameter.** It rebuilds the
  instance, so a transfer had to carry mana explicitly; re-deriving it from the
  template would have **refilled** a worn shadow weapon on every private-
  warehouse round trip. The warehouse/freight path threads the real value; the
  trade/store/mail paths pass `-1` under a comment, since all of them demand
  tradability and no shadow item is tradable.
- **`game_loop/item_mana.rs`** — `Item.decreaseMana` + `scheduleConsumeManaTask`
  + `ItemManaTaskManager`, as a `ScheduledTask::ItemManaTick` per item and a
  `World::item_mana_consuming` set standing in for `_consumingMana`. One point
  per minute **while worn**; warnings at 10/5/1; at 0 the item unequips itself,
  is destroyed, and the player is told.
- Consumption points ported at all three Java sites: equip
  (`Player.useEquipableItem` → `finish_equip_change`), the 60 s beat, and the
  `EnterWorld` sweep over worn shadow items.
- **Two exploit guards that only became reachable now that mana is real:**
  `RequestCrystallizeItem`'s `isShadowItem()` refusal (a shadow weapon is not a
  crystal printer) and `AbstractRefinePacket.isValid`'s (no augmenting one).

### The upstream quirk, kept

`ItemManaTaskManager.run` calls `decreaseMana(item.isEquipped())` — the *reset*
argument. Worn → the flag clears and the item re-arms. **Taken off before the
beat lands → the point is still spent, the flag stays set, and nothing ever
clears it again**, so that item stops draining for the rest of its life
(re-equipping burns one more point and goes quiet). Reproduced deliberately,
documented at the site: a shadow weapon should last exactly as long here as it
does on the Java server. See [[l2r-port-behaviour-not-intent]].

---

## Tests

`game_loop/tests/shadow_weapons_tests.rs`, 8 tests against the **real** dist
catalog and multisell loader (so they also prove the three restored lists
resolve):

1. all four coupon-holding branches open the right page/multisell,
2. a coupon buys a weapon charged with its template `duration`,
3. the C-grade list refuses a D-grade coupon,
4. a worn weapon burns 1 mana/minute and re-arms,
5. an unworn one burns none,
6. at zero it unequips, is destroyed and announces itself,
7. the 10/5/1 warnings fire exactly once each,
8. a shadow item cannot be crystallized.

Sabotage-verified: removing the `duration` stamp fails (2), (4) and (6);
removing the crystallize guard fails (8). Test (8) was **rewritten after a
first version passed vacuously** — the shadow *weapons* declare no
`crystal_count`, so they are already refused by the "cannot be crystallized"
branch; the subject is now a Bastard Sword with a `duration` stamped onto its
template, where the shadow guard is the only thing in the way. See
[[l2r-fixture-hides-testcase]].
