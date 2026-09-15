# Scope — what this server deliberately does not do

The port is complete. This file records what was **decided against**, so that an
absent feature is not mistaken for unfinished work, and so a future audit does
not re-derive an answer that already exists.

The rule these decisions follow: *a shipped config file proves nothing on its
own*. Check that Java parses **and** consumes a key before calling its absence a
gap — `Custom/PcCafe.ini` looks authoritative and is dead in Java itself
(`Config.java` never opens it; the live PC-cafe keys are in `PremiumSystem.ini`).

## Off-chronicle content

This is an **Interlude** server. Content from later chronicles is not ported and
is not expected to be.

| Not ported | Why |
|---|---|
| Gracia / Hellbound content, elemental item attributes, `AdminGraciaSeeds`, `AdminElement` | Kamael-era content. |
| Sayune, shuttles, airships | Post-Interlude travel systems, inert on Interlude maps. |
| Fort sieges, territory war, siegable clan halls, fences | Off-chronicle for this build. |
| Seven Signs (`SSQZone`, 41 zones) | Same. |

## Dropped by decision

| Not ported | Why |
|---|---|
| MariaDB / PostgreSQL backends | The Java dist ships SQLite here; one backend, one dialect. See [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md) #9. |
| The Java Swing server UI | A GUI on a headless server process. See [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md) #10. |
| Java's `tools/` tree | Replaced rather than ported — [`crates/tools`](../crates/tools/README.md) answers datapack questions by calling the server's own geo engine. |
| Runtime script loading (`//script_load`, `//quest_reload`, `//script_dir`) | Architecturally N/A: scripts are compiled in, so there is no runtime loader to drive. See [JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md) #3. |

## Mobius `Custom/*.ini`

The Mobius `config/Custom/*` features are out of scope **except any this dist
enables**. The audit ran: all 17 features enabled here are ported, consumed and
tested. The three whose consumers are least obvious, recorded so the next audit
need not re-derive them: L2Walker protection in `game_loop/social/chat/`, the
private-store spacing rule in `game_loop/commerce/private_store.rs`, and the
boss spawn announcement in `model/npc.rs`. Fourteen further `Custom/*` files are
genuinely disabled on this dist and stay out — among them `FakePlayers.ini` and
`OfflinePlay.ini`.

## Measured, and correctly out of scope

Not gaps — an audit reached these and they are off-chronicle, config-disabled,
or unimplemented in Java too:

mentor, item commission, the 9 daily-mission handlers, prime shop, beauty shop,
appearance stones, 96 wired Ex opcodes, and 13 base opcodes that are `null` in
Java's own enum (`SOCIAL_ACTION`, `CHANGE_MOVE_TYPE`, `CHANGE_WAIT_TYPE`,
`REQUEST_EVALUATE`, `REQUEST_MAGIC_LIST`, `NET_PING`, `REQUEST_SSQ_STATUS`,
`REQUEST_BUY_PROCURE` among them). `PetSkillData.xml` is unread but nearly
irrelevant here: of its 1046 npc ids only 8 are reachable from an
Interlude-range summon skill. `.lang` and `.changepassword` are disabled in this
dist's config. The 17 `quests/not_done` classes load in Java and do nothing, so
their absence *is* parity.

## The level cap

`MaximumPlayerLevel` is read and immediately incremented in Java
(`PLAYER_MAXIMUM_LEVEL++`), so a shipped `80` reaches `ExperienceData` as 81 and
`MAX_LEVEL` names the row *above* the highest attainable level. The port's own
attainable cap is wider because `ExperienceData` reads `maxLevel` raw. Narrowing
it is a live-server decision, not a porting one, so it is left alone —
`data/karma_data.rs` answers from the row the file actually declares.

## How a gap is recorded

A behaviour deliberately skipped inside a shipped feature gets a
`TODO(<tag>)` comment **at the exact spot**, naming what the Java source does.
The inventory of those markers is an assertion, not prose:
`crates/tools/tests/coverage_census.rs::deferral_markers_match_the_recorded_inventory`
fails the build when a marker is added without being recorded, or closed without
being removed. It currently expects exactly one — `TODO(antharas-cc)`.

The same shape guards persistence:
`crates/tools/tests/persistence_census.rs` holds the list of tables Java writes
and this server does not, and fails the moment the db layer starts writing one
of them.
