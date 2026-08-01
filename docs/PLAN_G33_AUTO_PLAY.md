# PLAN — auto play (`Custom/AutoPlay.ini`)

The last feature of the G33 `Custom/*.ini` audit
([PLAN_G33_CUSTOM_INI_AUDIT.md](PLAN_G33_CUSTOM_INI_AUDIT.md)) and the largest:
~1670 lines of Java across `AutoPlayTaskManager` (406),
`AutoUseTaskManager` (546) and the `AutoPlay` voiced-command handler (716),
plus four html pages under `data/html/mods/AutoPlay/`.

## What it is on **this** dist

Worth stating up front, because the name suggests something else: this build has
**no Classic auto-hunt packet family**. `ExClientPackets` registers no
`ExAutoPlay*` opcode, and nothing in `java/` reads one. The whole feature is
driven by a **voiced command** (`.play`, `.playskills`, `.playitems`,
`.playpotion`) that opens an html panel. That makes it a much smaller port than
the packet-driven Classic version — no new opcodes at all.

## Shape

| piece | Java | what it does |
|---|---|---|
| `AutoPlaySettings` | `Player.getAutoPlaySettings()` | pickup, next-target mode, short range, respectful hunting, potion percent |
| `AutoUseSettings` | `Player.getAutoUseSettings()` | the auto-attack action flag, buff/skill/supply-item lists, the potion item |
| `AutoPlayTaskManager` | 300 ms pool | target acquisition, chase, attack intention, loot pickup, party assist |
| `AutoUseTaskManager` | 300 ms pool | casting the chosen buffs/skills, using supply items and the potion |
| voiced handler | `.play` + 3 sub-pages | the panel, the toggles, and start/stop |

Settings persist in **player variables** (`AUTO_USE_SETTINGS`,
`AUTO_USE_ACTIONS`, `AUTO_USE_BUFFS`, `AUTO_USE_SKILLS`, `AUTO_USE_ITEMS`,
`AUTO_USE_POTION`), written at logout and read at login — so the panel survives
a relog, and `ResumeAutoPlay` (**False** here) decides whether the loop itself
restarts.

## Slices

### Slice 1 — settings, the panel, and the play loop *(this slice)*
- `config/auto_play.rs` for the whole ini.
- `AutoPlaySettings` / `AutoUseSettings` on `Player`, persisted through the
  existing `PlayerVariables` component so a relog keeps the panel.
- The `.play` panel (`Main.htm`) with its toggles: auto-attack, loot, respect,
  range, the four next-target modes, and the potion percent.
- `AutoPlayTaskManager`'s loop: validate the current target, acquire the
  nearest valid one (respecting mode / short range / respectful hunting / a
  geodata reachability check), attack it, and pick up loot within 200 units.
- Party assist (`AssistLeader`, **False** here) ported for shape.

### Slice 2 — auto use
`AutoUseTaskManager`: the buff, skill, supply-item and potion loops, plus the
`.playskills` / `.playitems` / `.playpotion` pages that choose them.

## Notes for the port

- Java's pool is 300 ms; the port ticks at 100 ms, so the loop runs every
  **3 ticks**.
- `isMageCaster` is a misnomer: it means "auto-attack is **off**", so a player
  who unticked the attack box acquires a target but never swings. Kept.
- The idle-count nudge (after 10 idle passes, step to a computed spot so a
  stuck melee unsticks) is a real behaviour, not a heuristic — ported.
- Spoil/Sweeper has a dedicated branch *before* the target is cleared, so an
  auto-player sweeps a corpse it spoiled. Needs auto-skill state, so it lands
  with slice 2.
