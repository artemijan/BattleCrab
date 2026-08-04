# `tools` — offline datapack and client tools

Everything in this crate operates on data at rest: the server's datapack under
`dist/game/data`, and the game client's `system` directory under `dist/client`.
Nothing here talks to a running server.

Two rules shape the crate:

- **Questions about the datapack are answered with the server's own code.**
  `spawn-pockets` links `gameserver` and calls `GeoEngine` directly rather than
  reimplementing height snapping, so a verdict here is a verdict in game.
- **The library never prints and never reads the environment.** Each module
  exposes a plain `fn(&Config) -> Report`; argument parsing and terminal output
  live in the binary's `src/cli/` tree. That is what lets the same logic back a
  GUI later.

## Build and run

```sh
cargo build --release -p tools          # -> target/release/l2r-tools
cargo run -p tools -- <command> ...     # or straight from source
```

Every path default is **relative to the workspace root**, so run the binary from
there (or pass `--game-dir` / `--client-dir` explicitly).

| Option | Default | Notes |
| --- | --- | --- |
| `--game-dir` | `dist/game` | Global, but only `spawn-pockets` and `sync-npc` read it. |
| `--client-dir` | `dist/client` | Per-command; the base for `system`, `system_decrypted`, … |
| `--system-dir` | `<client-dir>/system` | The client's real, enciphered files. |
| `--structure-dir` | `dist/client/structure` | The vendored `.dat` schema set. |

`--help` on any subcommand prints the authoritative flag list; this file
explains *when* to reach for each one and how to read what comes back.

| Command | What it does |
| --- | --- |
| [`spawn-pockets`](#spawn-pockets) | Find spawn rows buried under the floor by geodata layer snapping |
| [`client-dat`](#client-dat) | Client `system` ⇄ editable text, end to end, plus a round-trip check |
| [`dat-text`](#dat-text) | One stage of that pipeline alone: decrypted `.dat` ⇄ `.dat.txt` |
| [`msg-color`](#msg-color) | Recolour system messages in a terminal UI |
| [`sync-messages`](#sync-messages) | Push the server's system-message table into the client |
| [`sync-npc`](#sync-npc) | Reconcile NPC names and titles between datapack and client |

---

## `spawn-pockets`

Finds spawn rows that `getNearestZ` snapped onto a geodata layer *underneath*
the floor players walk on. The mob is invisible, unaggroable and unhittable
there until its AI happens to walk it out. The ten Cruma Tower rows it was
calibrated against were the first of 176 lifted off sub-floor slabs worldwide.

```sh
l2r-tools spawn-pockets --region 20_21     # one geo region tile (Cruma Tower)
l2r-tools spawn-pockets --all-regions      # every region that has spawns
l2r-tools spawn-pockets --bbox 16000,115000,24000,123000   # minx,miny,maxx,maxy
```

Exactly one of `--bbox`, `--region` or `--all-regions` is required — the flood
fill is area-bounded, so sweeping the world means sweeping region by region. A
single region is quick once geodata is loaded; `--all-regions` repeats that for
every tile holding spawns and takes correspondingly longer.

Give a `--bbox` room to breathe. The fill is clipped to the box, so unless a
seed falls inside it nothing gets filled and every row comes back `uncovered` —
no verdict either way, which is *not* a clean bill of health. `--region` avoids
the trap.

**How it decides.** It flood fills the walkable surface from coordinates players
demonstrably stand on: every teleport destination in `data/teleporters`, any
`--seed x,y,z` you add, and — since most dungeons are entered on foot and have
no teleport destination inside them — in-area spawn rows sitting on cells with a
single surface, where there is no ambiguity about what the ground is. Then, per
row, it asks the engine the two questions a player asks: can a walker starting
on the floor *arrive* at the mob's layer, and can the floor *see* it. Two more
obvious detectors do not work — a pocket is not always small (Cruma's slab spans
the whole tower), and reachability alone leaks onto the slab because the step
rule has no vertical limit. Read the module docs in `src/spawn_pockets.rs`
before touching the thresholds.

**Output.** One `BURIED` line per hit, carrying the file and line to edit, the
snapped z, the walkable floor above it, and a `suggest z="…"` you can paste in:

```
BURIED DionMonsterSpawns.xml:303 id=20215 at (20076,119925) z="-12131" -> snapped
  -12160, walkable floor -12080 | suggest z="-12084" | visible 0/24 before, 24/24 after
```

(A real hit from Cruma Tower, before `dba71aa1` lifted 166 such rows — the mob
was visible from none of the 24 vantage points and from all 24 once lifted.)

The closing summary counts what was judged, what was buried, and how many rows
the fill never reached.

**Calibrating.** `--csv` dumps the raw metrics behind every candidate, buried or
not; that is the data the thresholds were fitted to. `--near x,y[,radius]`
(default radius 400) narrows the dump to one mob a player reported, and implies
`--csv`. `--spawns-dir` points the sweep at a different datapack checkout.

---

## `client-dat`

The client keeps its data behind a 28-byte `Lineage2VerNNN` header naming the
cipher that follows. This command turns that whole tree into editable text and
back:

```sh
l2r-tools client-dat decrypt        # system -> system_decrypted (text)
#   ...edit files in dist/client/system_decrypted...
l2r-tools client-dat encrypt        # system_decrypted -> system
l2r-tools client-dat roundtrip      # verify: decrypt, re-encrypt, compare
```

One directory each side, original filenames, no binary halfway stage. Only
`.ini`, `.int` and `.dat` are touched — deciphering already yields text for the
first two, while a `.dat` is a binary record stream and additionally gets walked
with a schema. Executables, libraries and `.u` packages are left where they are.

**The manifest is load-bearing.** Both directions need facts the files do not
carry: which cipher a file used is not derivable from its name (`.ini` appears
under Ver111, Ver413 *and* unencrypted), and once a `.dat` has become text
nothing in it says which schema produced it. `decrypt` records all of that in
`.l2client-manifest.json` inside the output directory; `encrypt` reads it back
and refuses anything it cannot place. Do not delete it, and keep it with the
text if you move the tree.

**What can be written back.** The XOR versions (111/120/121) are symmetric and
reproduce the original byte for byte. Of the RSA versions, only **Ver413**
round-trips — NCsoft published just the public exponent for 411/412/414, so
those are decrypt-only. A file no schema fits is stored as raw deciphered bytes
and passes through untouched.

**`roundtrip`** decrypts and re-encrypts without editing anything, then checks
the client got its own files back. It never writes to `system` or
`system_decrypted` — it uses a scratch `system_roundtrip/` and rebuilds into
`system_encrypted/` — so it is safe to run over a tree with pending hand edits.
Read its verdicts carefully:

- `identical` — byte for byte.
- `equivalent` — different bytes, identical decrypted content. Expected and
  fine: NCsoft's deflate encoder is not `flate2` at any level, so the zlib
  stream is framed differently. On a pristine client this is most of the tree.
  **Byte identity against a retail client is not achievable**; plaintext
  equality is the real gate.
- `CHANGED` / `MISSING` — actual failures; the command exits non-zero.

`--verbose` lists every file rather than only failures. `--chronicle` pins the
schema set; the default `auto` tries every layout per file and keeps whichever
consumes the file *exactly*, which is what a client mixing revisions needs.

---

## `dat-text`

The schema stage of the pipeline above, on its own, for when you want to see or
edit the record text without re-encrypting anything:

```sh
l2r-tools dat-text unpack        # system_decrypted -> system_text  (*.dat.txt)
l2r-tools dat-text pack          # system_text      -> system_decrypted
```

Both take optional positional `IN_DIR` and `OUT_DIR`; the defaults swap with the
direction as shown above (the built-in help phrases them from `unpack`'s point
of view). `unpack` needs a decrypted tree, so run `client-dat decrypt` first.

A file is written only when a layout consumes it exactly — landing on the last
byte, plus the 13-byte `SafePackage` trailer. A walk that drifts is reported and
**not written**, because text from a drifting walk repacks into a corrupt
`.dat`. `--verbose` shows which layout matched each file.

`--enums` prints enum-valued integers as their labels. It is off by default and
should stay off for anything you intend to pack again: the labels do not
round-trip back to bytes yet.

---

## `msg-color`

A terminal UI for the colour system messages render in. Opens the table straight
out of the client, in memory:

```sh
l2r-tools msg-color                                  # SystemMsg_Classic-eu.dat
l2r-tools msg-color --file SystemMsg-eu.dat
```

| Key | Action |
| --- | --- |
| type / `Backspace` | *(search box)* filter by message id or text — `2810` and `dead` both work |
| `Tab` | Switch between the search box and the list; `/` also focuses search |
| `↑` `↓` or `k` `j`, `PgUp` `PgDn` | *(list)* move the selection |
| `Enter` | *(list)* open the colour picker for the selected message |
| `↑` `↓` then `Space` | *(picker)* highlight a preset and apply it to the input |
| hex digits then `Enter` | *(picker)* type `RRGGBB` or `RRGGBBAA` and commit it |
| `r` | *(list)* revert the selected message to its original colour |
| `Ctrl-S` | Save: pack and re-encrypt the file back into `system/` |
| `q` *(list)* or `Esc` | Close — asks first if there are unsaved edits |

Ten presets are offered, starting with the client's own notice colour
(`B09B79FF`). Nothing on disk changes until you save, and a save that fails
leaves the file untouched and says so in the status line.

---

## `sync-messages`

The server sends a message *id*; the client supplies the wording from its own
table. So a message this server invents displays as nothing, and a reworded one
keeps showing the old text, until this has run:

```sh
l2r-tools sync-messages --dry-run
l2r-tools sync-messages
```

Per run it overwrites the text and render class of every id the server's table
and the client's dat share, and appends every message marked `custom` that the
client has no row for. Both `SystemMsg_Classic-eu.dat` and `SystemMsg-eu.dat`
are written by default; `--message-file` (repeatable) overrides that.

**Colour is deliberately not synced.** It belongs to whoever is looking at the
client, and `msg-color` is how it is set — rewriting it here would silently undo
every such edit on the next run. Existing rows keep their colour; appended rows
start neutral.

A record has sixteen fields and the server's table knows three (id, text,
colour). Rather than invent the other thirteen, an appended row takes the
**modal** value of each column across the file — the client's own habit rather
than our guess, and derived at run time so it stays visible. Server messages the
client has no row for and that are not marked `custom` are left alone and
counted: the Java reference is simply newer than this client.

---

## `sync-npc`

The client, not the server, supplies the name over a mob's head unless the
template is flagged `usingServerSideName`. Renaming an NPC in the datapack
therefore changes nothing on screen until this has run.

```sh
l2r-tools sync-npc --dry-run                 # to-client (default)
l2r-tools sync-npc
l2r-tools sync-npc to-datapack --dry-run     # the other direction
```

**`to-client`** writes each NPC's `name=` and `title=` from
`dist/game/data/stats/npcs` into the client row keyed by `displayId`, and
appends a row for any NPC the client lacks (`--no-append` to only correct what
already exists).

**`to-datapack`** runs it backwards, for when the client's table is the retail
truth and the datapack has drifted. It is deliberately the weaker direction: it
only corrects NPCs the datapack **already declares**, and a client row naming an
NPC with no template is reported as a `warning:` and skipped — a name cannot
support inventing a template whose level, stats, drops and AI would all be
guesses. Edits are line-local, so the BOM, tab indentation and
`<!-- Confirmed CT2.5 -->` comments survive, and a new `title=` lands in the
datapack's own attribute order. Afterwards the whole datapack is reloaded
through the server's own parser and every edit re-checked, because one broken
tag takes its entire file's NPCs down with it.

**Reading a dry run.** `--limit` caps each list at 20 entries (`0` prints all);
an elided list always says how much it dropped.

| Mark | `to-client` | `to-datapack` |
| --- | --- | --- |
| `~` | row corrected | template corrected |
| `+` / `·` | missing from the client, will be appended | template the client has no row for, left alone |
| `-` / `!` | client row no template claims | client row naming an undeclared NPC, ignored |
| `=` | field only the losing side knows — kept, not blanked |  |

**Three things neither direction does.** The title's render colour is not
modelled by the datapack, so an existing row keeps its own and an appended one
takes the file's modal colour. A missing or empty name/title is one side
declining to say rather than a claim that the string is empty, so neither side
blanks the other — those are the `=` lines, and running both directions in turn
is how you resolve them. And only the **Classic** table is synced by default:
`system` also ships `NpcName-eu.dat`, but that is a *different chronicle's*
mapping (id 20138 is "Gargoyle" there and "Turek Orc Commander" in both the
Classic table and this datapack), so it takes an explicit `--npc-file`.

Nothing is written unless the packed file re-reads as what it meant to say.

---

## Recipes

**Diff a client table against the datapack.**

```sh
l2r-tools client-dat decrypt
l2r-tools dat-text unpack
$EDITOR dist/client/system_text/ItemName_Classic-eu.dat.txt   # vs data/stats/items
```

**Edit a client table by hand and put it back.**

```sh
l2r-tools client-dat decrypt      # -> dist/client/system_decrypted
$EDITOR dist/client/system_decrypted/...
l2r-tools client-dat encrypt      # back into dist/client/system
```

**Prove the pipeline is lossless after touching cipher or schema code.**

```sh
l2r-tools client-dat roundtrip --verbose
```

**Fix mobs a player reports as unhittable.**

```sh
l2r-tools spawn-pockets --region 20_21 --near 19920,108970   # metrics for one mob
l2r-tools spawn-pockets --region 20_21                       # every burial there
# apply the suggested z= values, then re-run to confirm the report is empty
```

## Tests and layout

```sh
cargo nextest run -p tools        # plain `cargo test` can hang in this workspace
```

81 tests as of writing: mostly round-trip assertions on the ciphers, the schema
walk and the two sync directions, plus keyboard-level tests of the `msg-color`
UI.

```
src/lib.rs             the library surface — fn(&Config) -> Report, no printing, no env reads
src/main.rs            the command table; a new tool is a module and one arm here
src/cli/               flags and terminal output, one module per subcommand
src/client_dat.rs      Lineage2Ver ciphers (XOR + RSA/zlib/CRC32)
src/client_files.rs    the one-directory view and its manifest
src/dat_schema.rs      the schema language, chronicles and layout selection
src/dat_text.rs        decrypted .dat -> text
src/dat_pack.rs        text -> decrypted .dat
src/dat_roundtrip.rs   the identical / equivalent / broken verdicts
src/datapack.rs        spawn rows, teleport seeds, region bboxes
src/spawn_pockets.rs   the burial detector and its calibrated thresholds
src/msg_sync.rs        system-message table -> client
src/system_msg.rs      the msg-color session model
src/npc_sync.rs        NPC names/titles both ways
src/npc_xml.rs         line-local edits to datapack XML
```
