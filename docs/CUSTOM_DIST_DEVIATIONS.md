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

## Cruma Tower — 1st basement floor from the entrance

- **Files:** `data/teleporters/others/CrumaTower.xml` (npc 30483),
  `data/html/teleporter/30483.htm`
- **Retail:** Carsus (30483), at the tower entrance, offers only the **2nd**
  and **3rd** basement floors. The **1st** floor is reachable solely from
  Ivory Tower Wizard Ian (30486), who stands at `17722,119749,-9068` — the far
  end of the 2nd floor, by the Core room. Carsus' own page says as much, and
  upstream Mobius Classic Interlude / Classic 2.9 / Classic 3.0 /
  GrandCrusade all ship the same two-entry list.
- **Here:** Carsus carries a third destination, the 1st floor
  `17616,115436,-6584`, with a matching button on his page. The line is
  **appended last** so his retail destinations keep indices 0 and 1 — the
  shipped html buttons address destinations by index, so inserting the new
  line first would have sent every existing button to the wrong floor.
  Ian's route is untouched and still works.
- **Guarded by:** `game_loop::tests::cruma_tower_tests` — the entrance list
  order, the custom button and its teleport, and Ian's original route.
