# G22 slice 6 — Orc and Dark Elf second-class transfers

Sixth G22 slice. `OrcChange2` (4 targets) and `DarkElfChange2` (7 targets).

## They look like siblings and differ in four ways

Both are level-40, three-proof second occupations. Everything else about how
they're driven is different, and each difference is silent if you port one by
copying the other:

| | Orc | Dark Elf |
|---|---|---|
| bypass event | the **class id** | the **row index** |
| page extension | `.htm` | `.html` |
| page order in a row | low, lowNoProof, done, noProof | **lowNoProof, low**, noProof, done |
| coupon reward | 15 × C-grade | **none at all** |

The last one is the trap. Every other `*Change2` script pays a C-grade coupon;
`DarkElfChange2` has **zero** `giveItems` calls in the dist. I checked by
counting rather than assuming (`grep -c giveItems`: Orc 4, Dark Elf 0), and
there's a test asserting the player is paid nothing.

The page *owner* also isn't the first NPC in both cases: Orc pages belong to
30513, which is first in its list, but Dark Elf pages belong to **30474 — the
third**. One page set serves all masters either way.

## Tests

4 added: the Orc transfer with coupons; the Dark Elf transfer by row index
paying nothing; a one-of-three-marks refusal; and page sweeps over both full
matrices (4 × 4 and 7 × 4 pages plus the fixed talk/refusal pages).

## Fixture: stopped chasing class ids

The transfer test failed on first run for the fourth slice running, always the
same cause — the shared `set_class_id` refuses a class id with no template, and
the quest fixture enumerated target ids by hand. Replaced the enumeration with
the whole Interlude class range (`0..=57`). Four identical failures was three
too many before fixing the pattern rather than the instance.

## Status

Port has **12 of 16** village-master scripts. Remaining: the three
`ElfHuman*Change2` scripts (Fighter is 477 lines — the widest) and
`AllianceMaster`.
