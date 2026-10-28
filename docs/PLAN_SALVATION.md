# Plan — Classic Interlude → Salvation

`main` moves from L2J Mobius **Classic Interlude** (protocol 110) to L2J Mobius
**Salvation** (`L2J_Mobius_05.0_Salvation`, protocol 140). The Interlude server
is frozen on the `interlude` branch; tag `interlude-final` marks the fork point
(`cd760097`).

Detailed evidence lives in three appendices. Read them before starting a
milestone; this file only orders the work.

| Appendix | Covers |
|---|---|
| [salvation/protocol.md](salvation/protocol.md) | Opcode tables, every packet whose bytes change, handshake/crypto/login, Rust file map |
| [salvation/engine.md](salvation/engine.md) | Model/engine delta, upstream drift vs chronicle change, formulas, skills, config, DB layer |
| [salvation/datapack.md](salvation/datapack.md) | XML format breaks, path moves, scripts, schema, `.ini` keys, client & geodata, deviations |

**References.** The new spec is
`../gitlab/L2J_Mobius/L2J_Mobius_05.0_Salvation` (`java/` + `dist/`). The old
spec, `../interlude_classic`, stays useful for one thing: diffing *why* the port
does something before changing it.

---

## The shape of the job

The two chronicles are closer than the version numbers suggest. Mobius Classic
Interlude was built on the same GoD-era engine that Salvation runs:

- **Same engine.** All 153 classes, including Ertheia and awakened ones, already
  exist in the Interlude enum. `Stat`, `EffectFlag`, `TraitType`, `TargetType`
  and `AffectObject` are the same.
- **About 80% of the Java diff is upstream drift.** Package moves, the `Config`
  split and renames come from 2.7 years of Mobius refactoring. Compared with
  *current* upstream Classic, Salvation differs by only about 3.7k lines in
  about 130 files.
- **The protocol is nearly the same.** Opcode tables, the game cipher,
  `KeyPacket`, `ProtocolVersion` and the **entire login server** are
  byte-identical. Only 43 server packets and about 10 client packets change on
  the wire.
- **The DB schema is nearly the same.** 93 shared tables, with 3 small column
  changes, 6 new tables and 3 dropped.

The real cost is **content**:
- Only 32 quest directories overlap.
- 167 of the port's 195 quest modules have no Salvation counterpart.
- About 530 Salvation quests, about 260 new AI scripts and 50 instance scripts
  are unported.
- Every item, skill and NPC file changed values, so every data-driven test needs
  a new baseline.

Rough effort, from the appendices:

| Area | Effort |
|---|---|
| Infrastructure + protocol to a playable login | ~2 weeks |
| Engine core | 10–15 dev-weeks |
| Content | Open-ended; dominates everything else |

---

## Decisions (settle before S1)

| # | Decision | Recommendation |
|---|---|---|
| D1 | **Production deploy.** CI deploys every push to `main` to battlecrab.com. | Retarget `deploy.yml` so prod deploys from `interlude`, **before** the first Salvation commit lands. See S0. |
| D2 | **What counts as spec.** Salvation HEAD Java includes 2.7 years of behavioural drift: crit on a 1000 scale, additive shot bonus, minimum damage 1, recipe model. | Salvation HEAD is the spec, drift included. "Dist is the specification" keeps meaning one thing. |
| D3 | **Interlude-only features**: lottery, soul-crystal absorb, mob groups, race village masters, class-change/Saga quests, Classic clan reward and hunting points, `global_tasks`. | Delete them from `main`; they live on in `interlude`. Do the deletion in one milestone (S9) so the census tests move once. |
| D4 | **Config layout.** Salvation renames `Character.ini` to `Player.ini`, adds `Database.ini`/`Threads.ini`/`IdManager.ini`, and renames `AltOly*` to `Olympiad*`. | Adopt Salvation's file and key names, and carry the Rust-only files over (`Dashboard`, `Logging`, `Monitor`, `Security`). |
| D5 | **Geodata.** Salvation ships **none**, and the Interlude tiles don't match the reworked Salvation world. | Source p140 geodata early. Until then, run with geodata off. A live server is blocked on this. |
| D6 | **Client.** The tools default to `*_Classic-eu.dat`, and the launcher bundles the Classic client. | Get a protocol-140 Salvation client, switch the tool defaults, and build a new launcher bundle. |
| D7 | **Optional systems**: Ceremony of Chaos, Underground Coliseum, Krateis Cube, Block Checker, Monster Book, Alchemy, Wedding, Captcha, fortresses. | Out of scope until the core is playable. Record each one in SCOPE.md as it is deferred. |

---

## Milestones

Each milestone ends with `cargo nextest run` and `cargo clippy` green. Tests are
re-baselined deliberately, never by blanket `--accept`.

### S0 — Fork hygiene (½ day)
- [x] `interlude` branch and `interlude-final` tag at `cd760097`.
- [x] Push both (`interlude` + `interlude-final`).
- [x] `deploy.yml`: production deploys from `interlude`, and `main` runs build
  and test only (D1). Add a staging environment for `main` later if wanted.
- [x] README, SCOPE and `docs/README.md`: state that `main` tracks Salvation and
  link this plan. Keep the "port is done" wording on `interlude`.

### S1 — Datapack swap and config layout (2–3 days)
- Replace `dist/game` and `dist/login` with Salvation's dist. Keep the
  Rust-owned `dist/client`, `ipconfig.xml` and the extra ini files.
- Apply D4: move the loaders to `Player.ini`, `Database.ini`, `Threads.ini` and
  `IdManager.ini`, and rename `AltOly*` to `Olympiad*`. Every new key gets a
  consumer or an explicit "not consumed" note (PROJECT_LAYOUT §2).
- Re-apply the operator's values, which are the bulk of the ~173 changed
  defaults: rates, champions, autoloot, premium, banking and so on. Diff
  `interlude:dist/game/config` to build the list.
- Set `AllowedProtocolRevisions = 140` and `ServerListType = Normal`, and keep
  `PacketEncryption` at the operator's choice.
- **Gate:** both servers parse all config with no unknown-key warnings.

### S2 — Loader format fixes (≈1 week)
Gate: **the gameserver boots on the Salvation dist with zero loader warnings.**
- **Path moves to `data/stats/players/`:** skill trees, templates (`StartingClass`, `1stClass`…`4rthClass`, `Ertheia`, `BalthusKnights`), `classList`, `experience` (max level 110), `initialEquipment`/`initialShortcuts`, `karmaLoss`, `experienceLoss`.
- **The `<cond>`→`<conditions>` rename.** In skills, tell the 180 legacy `msgId` blocks apart from the 3,676 skill-condition blocks by the `msgId` attribute.
- **`Recipes.xml`:** rewrite `data/recipe_data.rs` for `<recipe><materials><product><npcFee><stat>`.
- **NPC data:** `<fakePlayer>` inside NPC XML replaces `FakePlayerVisualData.xml`; add the `<lucky>` element.
- **Henna:** `duration`, `skill`, `isPremium`.
- **Teleporters:** `feeId`, `name`, `questZoneId`.
- **Skill trees:** the new `skillTree`/`skill` attributes, and the ability, alchemy, dualClass, revelation and awakening trees. Load them all; consumers come later.
- **Skills XML:** `irreplaceableBuff` (spelling fixed), `replace`, `replacementSkillId`, and the level/sub-level expression evaluator. `EnchantSkillGroups` is now keyed by sub-level.
- **Spawns:** `locations`, `minions`, `banned_territory`, `spawnByDefault`. `spawn_data.rs` currently skips these.
- **Removals:** `LevelUpCrystalData.xml`, `ClanReward.xml`, `initialEquipmentEvent.xml`. Delete the loaders together with their consumers in S9, or stub them out here.

### S3 — Database (1–2 days)
- Add one migration:
  - 6 new tables: `character_offline_play`, `character_offline_play_group`, `character_transmogs`, `item_special_abilities`, `mods_wedding`, `underground_coliseum`.
  - 3 column changes, done as SQLite table rebuilds following the `grandboss_real_hp` pattern: `buffer_schemes.skills` becomes varchar(512); `character_shortcuts.level` becomes not-null with default 0; `olympiad_fights.classed`.
- Regenerate entities (see DATABASE.md) and update `schema_parity`.
- **Paperdoll remap migration:** `items.loc_data` +5 for equipped slots ≥ 17 (see S5). Write it once the new layout lands; it is irreversible on a live DB, so test it against a copy of prod.
- `lottery` and `global_tasks` go in S9, together with their code.
- Seed data: grand bosses (Lindvior, Trasken, Beleth, Anakim, Lilith, Kelbim and Helios added; Dr. Chaos removed). Add fort rows only if fortresses come into scope.

### S4 — Protocol 140 (5–7 days)
Order follows [salvation/protocol.md §5](salvation/protocol.md):
1. **Opcodes:** `REQUEST_USER_BAN_INFO` moves `0x138` → `0x15E` (`client_packets/ex_opcodes.rs`). Rename `MoveBackwardToLocation` → `MoveToLocation`.
2. **`CharSelectionInfo`:** +1 header byte, +5 ints per character, −1 delete-timer for banned characters, hero as bool. **Gate: the character list shows.**
3. **Item encoding:** `write_item_entry` gains +2 zero bytes and the optional `SOUL_CRYSTAL` ensoul block.
4. **Two-phase `ItemList` / `ExQuestItemList`:** `sendType` 1 then 2, with int counts.
5. **`InventoryUpdate`:** header becomes `byte 0, int 0, int n`.
6. **`UserInfo`:** mask count hardcoded to **24** (not `VALUES.len()`); SLOTS block grows to 11 bytes.
7. **`ExUserInfoEquipSlot`:** 38 slots (needs S5).
8. **Gate: enter world** with a real p140 client.
9. **Smaller packets:** `AcquireSkillList` (i32 level, +dual-class byte, +isNew), henna packets (+duration), `GMViewPledgeInfo`, `GMViewCharacterInfo`, `ValidateLocation`.
10. **Remaining two-phase lists:** warehouse, package, trade, private-store manage, mail.
11. **Client reads:**
    - `SetPrivateStoreListBuy` ensoul arrays. These are mandatory: without them, item 2 onward desyncs.
    - Multisell and recipe trailing fields.
12. **System messages and NpcStrings** from the Salvation client: `gen-messages` with non-Classic dat defaults. Expect a compile-error sweep: 1,248 names change and 11 ids move.

Handshake, cipher and the login server need no code change; that is verified in the appendix.

### S5 — Foundations: slots, classes, levels (1–2 weeks)
- **Paperdoll 32 → 37:**
  - AGATHION1–5 sit at 17–21, and deco, belt and brooch shift by +5.
  - `BodyPart::AGATHION` mask is `0x3000000000`.
  - Touches `model/inventory.rs`, `enums::InventorySlot`, `PAPERDOLL_ORDER`, `RequestUnEquipItem` (which now takes a paperdoll slot) and the S3 remap.
- **Classes:**
  - Class ids 190/191 (Balthus) and the `CategoryType` Balthus groups.
  - Templates for 155 classes, with `baseCHA`, `baseLUC` and `basePSkillCritRate`.
- **Levels:** level cap 110 and pet level 110. Re-check the level-cap note in SCOPE.md against the new table.
- **IdManager:** read `IdManager.ini`.
- **Gate:** a level-1 character of each starting race can be created and logs in. A migrated Interlude character keeps its equipped items in the right slots.

### S6 — Stats and combat (1–2 weeks)
- **Stats:**
  - 4 new `Stat` keys.
  - A CHA bonus in the MaxHp and PAtk finalizers.
  - `ShotsBonus` 0.3 → 0.7 per enchant level.
  - A new `PSkillCritRate` finalizer.
- **Formulas:**
  - Crit rates on a 1000 scale, clamped to `[30, MAX_*]`.
  - Crit damage becomes `1 + critDmg·bal·max(0.2, 2 − defCritDmg)`.
  - Additive shot bonus.
  - `critMagicAdd`.
  - Minimum damage 1.
  - `activateRate > 99` lands for sure.
  - Dragon-weapon PvP defence.
- **Stat recalculation debounce:** decide on parity. Mark dirty and recalculate next tick; this is visible as a one-tick-later UserInfo.
- **Re-baseline `formula_parity` and the combat goldens**, each against hand-checked Java output, not against the new Rust output.

### S7 — Skills (2–3 weeks)
- **Effect renames:**
  - The `*Attack` handlers become `*Damage`.
  - `TriggerSkillByAttack/ByDamage/BySkillAttack` become `ByDamageDealt/ByDamageReceived/BySkillDamage`.
  - `PAtk`, `MAtk` and `ResistDDMagic` are removed.
- **About 18 new effects:**
  - Equipment and items: `AgathionSlot`, `CreateItem`, `CreateCommonItem`, `Crystallize`, `RemoveEquipPenalty`, `Ride`.
  - Survival and debuffs: `CheatDeath`, `Distrust`, `DispelCaster`, `FlipBlock`.
  - Skill mechanics: `LimitSkill`, `TrackLimitedSkill`, `ReplaceSkillBySkill`, `ApplySkillEffects`, `TriggerSkillWithDelay`, `MagicalSuccessResistance`.
  - Rewards: `BonusRaidPoints`, `WorldChatPoints`.
- **Conditions and targets:** `OpNotInEventSkillCondition`; `BalakasScope` renamed `ValakasScope`.
- **`AbnormalType`:** 22 added and 7 removed. `CommonSkill` changes.
- **Skill infrastructure:** the `TriggerCastInfo` queue, alter/replaced skills, and the reuse-message threshold changing 3000 → 1000 ms.
- **Census:** `coverage_census` gets a new denominator. Expect a large initial gap list, which is the point of the census.

### S8 — Class progression (2–3 weeks)
- 1st, 2nd and 3rd class change without the Interlude quests. Check how Salvation
  gates them (`ClassMaster.xml` and the remaining village_master scripts).
- **Awakening and dual class:**
  - The subclass/dual-class flow moves from `VillageMaster` to the Raina script.
  - Original-class tracking, dual skills and the revelation tree.
- **Other progression systems:**
  - Ability points.
  - Noble level.
  - Olympiad heroes from `SIXTH_CLASS_GROUP`.

### S9 — Removals (parallel with S6–S8, 1 week)
- **Deleted from `main`** (D3; the code stays on `interlude`):
  - Lottery
  - Mob groups (`//mobgroup`)
  - Soul-crystal absorb
  - Race village masters and their subclass menus
  - Classic clan reward and hunting points
  - `global_tasks` (fold its work into the daily reset)
  - `PlayerCondOverride`
  - The item-grade condition
- **Tables:** drop `lottery`, `global_tasks` and `custom_teleport` in a migration.
- **Census tests:** update `persistence_census` and `deferral_markers_match_the_recorded_inventory` in the same commits.
- **Scripts:** retire the ~167 Interlude quest modules with no Salvation counterpart, together with `_SagaShared` and the class-change scripts.

### S10 — Items and clans (3–4 weeks)
- **Items:**
  - Ensoul: `item_special_abilities`, the `stats/ensoul/*` data and the ensoul window bypass.
  - Agathions: `AgathionData`, equip and the slot-count stat.
  - The recipe craft model (`RecipeHolder`).
- **Clans:**
  - `ClanLevelData` exp-based levelling.
  - Mastery and development points.
  - Clan shop.
  - Contribution.
  - The pledge V2 packets that go with these: 14 client and about 11 server.

### S11 — Re-apply operator deviations (2 days)
Follow [salvation/datapack.md §6](salvation/datapack.md):
- **Re-apply:**
  - Cruma Tower: the 3rd-floor coordinate differs in Salvation.
  - Advanced HQ half damage: the bug is still upstream.
  - `StrictDelevelSkillRemoval`, now in `Player.ini`.
  - The over-enchant accessory ceiling.
  - `HtmCache=True`.
- **Drop and reclassify:**
  - Drop Q214, which is gone in Salvation.
  - Reclassify TvT thaw, which is now fixed upstream.
- **Decide anew:**
  - The Shadow Weapon coupon buttons. Salvation ships `custom/ShadowWeapons`, so check its multisell ids.
  - The Formal Wear (Q33–Q37) "authentic Interlude" levels.
- Record every surviving edit in CUSTOM_DIST_DEVIATIONS.md. The ~181 undocumented local datapack edits are the lesson here.

### S12 — Content (open-ended)
Port in the order a new player meets content:
1. **Talking Island / starter zones and the newbie flow:** Salvation's tutorial and newbie quests and the starting-area AI.
2. **Core instances:** extend `instance_data.rs` to the full Salvation instance XML (6 → 175 templates), then Kamaloka, Pailakas and the level-band instances.
3. **Quests**, by level band. Use the 28 overlapping quests as a re-port warm-up.
4. **Area and boss AI** (17 → 61 areas, 11 → 31 bosses).
5. **Events** (7 → 20).

Track coverage with a census, the way `coverage_census` does for skills, instead
of a hand-kept list.

### Cross-cutting, run in parallel
- **Client and tools:**
  - Point `gen-messages`, `sync-messages`, `msg-color`, `sync-npc` and the `dat_schema` armorgrp matcher at the non-Classic dat files.
  - Re-run `sync-npc` against the Salvation client.
  - Re-verify `utx`/asset tooling on the new client packages.
  - New launcher bundle (D6).
- **Geodata (D5):**
  - Source p140 tiles.
  - Re-run `spawn-pockets` on Salvation spawns. The ~176 Interlude z-fixes do not carry over.
  - Optionally match the Salvation single-A* post-filter.
- **Script hook signatures:** in Salvation, `onAttack`, `onKill` and `onSpawn` return void. Decide whether the Rust hook traits follow.

---

## Risks

| Risk | Mitigation |
|---|---|
| A Salvation commit auto-deploys to the live Interlude server | S0 first; no other milestone merges before it |
| No geodata for p140 | Start sourcing in S0; the server runs geo-off for development |
| Re-baselined tests quietly encode Rust's own output | Every golden-number change cites the Java computation or a client capture |
| Paperdoll remap corrupts equipped items on a migrated DB | Dry-run on a prod DB copy; verify equipped-item counts before and after |
| Message-id regeneration renames ~1,248 constants | Do it as its own commit in S4, compile-error driven, with no behaviour changes mixed in |
| Content scope never ends | S12 is explicitly open-ended; ship a playable 1–40 slice first |
