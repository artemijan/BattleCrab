# Custom dist deviations

Places where `dist/game/data` **intentionally differs** from retail / upstream
`L2J_Mobius_Classic_Interlude`, by operator decision rather than by porting
accident.

The dist data is otherwise treated as the specification: when the server
behaves differently from what the data implies, the bug is in the server, not
in the data. That rule only works if the handful of deliberate exceptions are
written down — otherwise a future "the data must be wrong" or a re-sync from
the Java reference repo silently reverts them.

Each entry names the files, what retail does, and the test that fails if the
change is dropped.

## Cruma Tower — the entrance serves the 3rd basement floor only

- **Files:** `data/teleporters/others/CrumaTower.xml` (npc 30483),
  `data/html/teleporter/30483.htm`
- **Retail:** Carsus (30483), at the tower entrance, offers **two**
  destinations — the 2nd basement floor `17664,108288,-9056` (index 0) and the
  3rd `17726,114838,-11696` (index 1) — with a button apiece on his page.
  Upstream Mobius Classic Interlude / Classic 2.9 / Classic 3.0 /
  GrandCrusade all ship the same two-entry list.
- **Here:** the 2nd-floor destination and its button are removed. The entrance
  drops players on the **3rd floor only**; the 2nd floor is reached onward
  from there through Ivory Tower Wizard Rombel (30487), who stands at
  `17811,114750,-11680` on the 3rd floor and whose sole destination is the 2nd
  floor. Nothing else in the chain changes: Belkadhi (30485) still runs 2nd →
  3rd/entrance, Janssen (30484) still runs 3rd → entrance, and Ian (30486) is
  still the only holder of the 1st floor, from the far end of the 2nd.
- **Index trap:** the shipped html buttons address destinations by *list
  index*, so dropping the 2nd-floor line moved the 3rd floor from index 1 to
  index 0 — `30483.htm`'s surviving button was retargeted to `OTHER 0` in the
  same change. Re-adding the 2nd-floor line without also fixing the html would
  not error; it would quietly send that button to the 2nd floor.
- **Guarded by:** `game_loop::tests::cruma_tower_tests` — the entrance list
  contents, the page's single button walked end to end onto the 3rd floor,
  Rombel's list and spawn, and Ian's original route.
