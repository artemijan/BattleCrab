# G13.B-login — GM login state, hero aura & admin menu

Status: **done**. Closing sub-milestone of [G13](PLAN_G13_ADMIN.md): the
GM framework (G13.A) and most command handlers (G13.B) already shipped; this
gate ports the three remaining login-time / menu pieces so a GM's *entry into
the world* matches Java.

Reference: `interlude_classic` (Java, ground truth).

## 0. Scope

Three faithful ports, all triggered at login / from the `//admin` menu — none
of them a new subsystem:

1. **GM startup state** — Java `EnterWorld.runImpl` applies a `GMStartup*`
   block to GMs (builder-hide, invulnerable, invisible, silence, diet, GM-list
   registration), all config-driven and each gated by the matching
   `admin_*` access right.
2. **Hero aura** — the hero glow byte in CharInfo / UserInfo is
   `isHero() || (isGM() && GMHeroAura)`, not a hardcoded `0`. (CharSelectionInfo
   is deliberately excluded: Java writes real-hero status there,
   `Hero.isHero(objectId) ? 2 : 0`, **not** the GM aura.)
3. **`//admin` menu** — `AdminAdmin.showMainPage`: `admin_admin`/`admin_admin1..7`
   open the `*_menu.htm` admin panels, whose buttons route back through the
   already-wired `admin_` bypass into the existing handlers.

**Not in scope** (kept faithful to the deferral rules in
[PLAN_G13_ADMIN.md](PLAN_G13_ADMIN.md) §5/§7):
- `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills` — special-skill trees not
  ported; loaded as `false`-stubs with a TODO.
- `isHero()` returning `true` — needs the Olympiad/hero system; stays `false`.
- Menu buttons whose target handler is a deferred G13.C subsystem stay
  gated-but-bodiless, as designed.

## 1. Java reference

| Piece | Java location |
|---|---|
| Config fields | `Config.java` `GM_HERO_AURA`, `GM_STARTUP_{BUILDER_HIDE,INVULNERABLE,INVISIBLE,SILENCE,AUTO_LIST,DIET_MODE}` ← `General.ini` |
| GM startup block | `network/clientpackets/EnterWorld.java` (the `gmStartupProcess:` label, ~L193–230) |
| Char-select invisible | `network/clientpackets/CharacterSelect.java` (~L170) |
| Hero byte | `CharInfo.java` L198, `UserInfo.java` L291 (CharSelectionInfo uses real-hero only) |
| Admin menu | `data/scripts/handlers/admincommandhandlers/AdminAdmin.java` `showMainPage` |

The startup block, verbatim in effect:
- If `GMStartupBuilderHide` **and** `hasAccess("admin_hide")` → hide, print the
  three "…is default for builder" lines, and **break** (skip everything below).
- Else, each independently: `GMStartupInvulnerable`+`hasAccess("admin_invul")`
  → `setInvul(true)`; `GMStartupInvisible`+`hasAccess("admin_invisible")` →
  `setInvisible(true)` + STEALTH abnormal visual; `GMStartupSilence`+
  `hasAccess("admin_silence")` → silence; `GMStartupDietMode`+
  `hasAccess("admin_diet")` → diet + `refreshOverloaded`.
- Always: `addGm(player, hidden = !GMStartupAutoList || !hasAccess("admin_gmliston"))`.

`showMainPage`: `mode = parseInt(command.substring(11))` (i.e. the digit after
`admin_admin`), switch 1..7 → `{main,game,effects,server,mods,char,gm}`, default
`main`, then `AdminHtml.showAdminHtml(player, filename + "_menu.htm")`.

## 2. Rust starting point

- **Framework done:** `AdminData` (access levels + rights), `use_admin_command`
  dispatch, confirm round-trip, `AdminFlags{invul, undying, hidden}` with
  consumers for `invul` (death/damage) and `hidden` (visibility).
- **Player** resolves `access_level`, `is_gm(data)`, `name_color`/`title_color`
  from the `AccessLevel` table at `from_char`.
- **Gaps:** no `General.ini` config section loaded; no GM branch in the
  enter-world flow; hero byte hardcoded `0` in `char_info.rs:129,151`,
  `user_info.rs:181`, `lobby.rs:208`; no `admin_admin*` handler.

## 3. Implementation

### 3.1 GeneralConfig (`config/general.rs`)
Mirror `config/character.rs` (`commons::config::PropertiesParser`, `get_bool`).
Load from `config/General.ini`:
`GMHeroAura`, `GMStartupBuilderHide`, `GMStartupInvulnerable`,
`GMStartupInvisible`, `GMStartupSilence`, `GMStartupAutoList`,
`GMStartupDietMode`; `gm_give_special_skills`/`gm_give_special_aura_skills`
loaded but `false`-stubbed (TODO: special-skill trees). Add `general:
GeneralConfig` to `Config`; surface the flags the runtime needs where the
consumers can reach them (CharInfo only receives `GameData`, so the hero-aura
flag must be reachable from `GameData` — see 3.3).

### 3.2 GM startup on login
- `AdminFlags` (`model/components.rs`): add `silence: bool`, `diet: bool`.
- Port the startup block into the enter-world flow as a GM-only branch,
  faithful to the `gmStartupProcess` short-circuit and each `has_access` gate.
  `invul`/`hidden` reuse existing consumers; `silence`/`diet` set the flag with
  a TODO where no consumer exists yet (whisper block / overload calc).
- Hidden invisible: reuse `admin_hide`'s hide path (broadcast `DeleteObject`),
  matching `setInvisible` + the visibility system's "don't describe a hidden
  GM" rule already in `send_char_info`.
- GM-list registration: add the player to the live GM set with
  `hidden = !GMStartupAutoList || !has_access("admin_gmliston")`.
- **Deviation — CharacterSelect invisible skipped.** Java also sets
  `GMStartupInvisible` at `CharacterSelect.java:170` to avoid a visibility flash
  before the world loads. Here the player is not added to the visible world
  until enter-world's `visibility::on_enter_world`, and the startup block runs
  immediately before it, so there is no window in which an invisible GM is ever
  broadcast. The char-select application is redundant and is not ported.

### 3.3 Hero aura byte
Resolve `hero_aura = is_hero || (is_gm && GMHeroAura)` once, on `Player` at
`from_char` (mirroring how `name_color` is pre-resolved), reading `gm_hero_aura`
from `GameData`. `is_hero` is `false` for now (TODO: Olympiad). Both in-world
builders then read `p.hero_aura`: `char_info.rs:129` and `user_info.rs:181`.
`lobby.rs` (CharSelectionInfo) is **not** touched — its hero glow is real-hero
status, unaffected by the GM aura.

### 3.4 `//admin` menu (`game_loop/admin/menu.rs`)
- Factor the `Link` bypass's file-read + `%…%` substitution into a shared
  helper `serve_admin_html(world, client_id, object_id, "<name>_menu.htm")`
  (reads `data/html/admin/`, sends via `npc_html_message`).
- Port `showMainPage`: `admin_admin` / `admin_admin1..7` → mode → filename.
- Register `admin_admin` + `admin_admin1..7` in the dispatch `match`.
- Menu buttons already fire `bypass admin_…`, which the `admin_` bypass path
  routes into `use_admin_command` — existing handlers light up for free.

## 4. Testing

- **Unit:** `GeneralConfig::load` reads the real `General.ini` with the dist
  values (`GMHeroAura=true`, `GMStartupInvulnerable=true`,
  `GMStartupInvisible=true`, `GMStartupBuilderHide=true`); hero-aura byte flips
  for a GM when `GMHeroAura` is on and stays `0` for a plain player.
- **Integration (synthetic world):** a level-100 char entering world gets
  `invul` + `hidden` set per config and lands in the GM list; `builder_hide`
  short-circuits so invul/invis are **not** re-applied past the break;
  `//admin` returns the `main_menu.htm` payload; a menu-button `admin_` bypass
  reaches its handler.
- **Live gate** (G13 doc gate #4/#6): `//admin` opens in-client; GM name renders
  in the access-level color with the hero aura; a GM logs in already
  invulnerable/invisible.

## 5. Deliverables — shipped

- `config/general.rs` — `GeneralConfig` loads the `GM*` keys from `General.ini`;
  wired into `Config` and folded onto `GameData.gm` (`GmSettings`) at boot.
- `Player.hero_aura` resolved at `from_char` (`isGM() && GMHeroAura`); read by
  `char_info.rs` and `user_info.rs`.
- `AdminFlags` gains `silence`/`diet`; `admin::apply_gm_startup` ports the
  `EnterWorld` GM branch (builder-hide short-circuit, invul, invisible, silence,
  diet) with per-command access gating, called from `lobby::handle_enter_world`
  before the spawn broadcast.
- `admin/menu.rs` — `//admin` (`admin_admin`/`admin_admin1..7`) serves the
  `*_menu.htm` panels; registered in the dispatch table.
- Tests: `config::general` (dist values), `hero_aura_resolves_from_gm_config`,
  `gm_startup_applies_invul_and_invisible`, `gm_startup_builder_hide_short_circuits`,
  `admin_menu_serves_main_page` — all green.

Recorded on the G13 line in [PROGRESS.md](PROGRESS.md).

## 6. Deferred (TODO, noted in code)

- GM-list registration (`AdminData.addGm` hidden flag) — no `//gmlist` consumer
  yet; `register_gm` computes the flag but stores nothing.
- `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills` — special-skill trees
  unported.
- `silence` / `diet` consumers (whisper delivery / overload calc) — flags set,
  not yet honored.
- `isHero()` real status — needs Olympiad.
