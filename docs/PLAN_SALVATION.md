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
| D5 | **Geodata.** Salvation ships **none**, and the Interlude tiles don't match the reworked Salvation world. | Source p140 geodata early. Until then, the 228 Interlude `.l2j` tiles stay in `dist/game/data/geodata` as a stand-in: they are right for the unchanged regions and wrong for the reworked ones. A live server is blocked on this. |
| D6 | **Client.** The tools default to `*_Classic-eu.dat`, and the launcher bundles the Classic client. | Get a protocol-140 Salvation client, switch the tool defaults, and build a new launcher bundle. |
| D7 | **Optional systems**: Ceremony of Chaos, Underground Coliseum, Krateis Cube, Block Checker, Monster Book, Alchemy, Wedding, Captcha, fortresses. | Out of scope until the core is playable. Record each one in SCOPE.md as it is deferred. |
| D8 | **Business-logic deviations** (user decision, 2026-10-28). | **None.** `main` is strict Salvation Java parity. The CUSTOM_DIST_DEVIATIONS behaviours are not re-applied (S11 removes them), and Salvation's config defaults ship untouched. The infrastructure keys the port owns stay Rust-side: SQLite URL, `Dashboard`/`Logging`/`Monitor`/`Security.ini`, `ipconfig.xml`, `hexid.txt`, internal status, and `LoginHostname = *`. |

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
- [x] Replace `dist/game` and `dist/login` with Salvation's dist. Keep the
  Rust-owned `dist/client`, `ipconfig.xml` and the extra ini files.
- [x] Apply D4: move the loaders to `Player.ini`, `Database.ini`, `Threads.ini`,
  `IdManager.ini` and `Development.ini`, and rename `AltOly*` to `Olympiad*`.
  Every new key without a consumer is listed in
  [salvation/config-keys.md](salvation/config-keys.md) with an owning milestone.
- [x] ~~Re-apply the operator's values~~. Dropped per D8: Salvation's defaults
  ship as they are. That includes `AllowedProtocolRevisions = 140`,
  `ServerListType = Normal` and `PacketEncryption = False`.
- [x] Code paths that Salvation's config switches on were ported or recorded:
  - Olympiad item rewards and the `MONTH` period.
  - Quest-item drop scaling (`QuestItemDropAmountMultiplier`).
  - Karma decay capped at 0 regardless of `MaxReputation`.
- [x] Env overrides follow the new paths: `DIST_LOGIN_CONFIG_DATABASE_URL` and
  `CONFIG_DATABASE_URL`. The deploy scripts read `Database.ini` and login
  `Server.ini`.
- [x] **Gate:** `config_boot_warnings` passes, so all config parses with zero
  missing-key warnings. The gameserver binary gets through config and stops at
  the first data loader (`experience.xml`), which is S2.
- **Left red for later milestones:** the test suites that read the datapack.
  They fail until S2 (paths and formats) and S12 (HTML, quests, clan halls,
  community board) land.
- **Post-S2-merge cleanups:** done. The `strict` delevel parameter, the
  grade-filter field and parser, and `initial_equipment_event` are gone, and
  the `Character.ini` comments under `data/` are renamed.

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
- **Done (2026-10-28).** The server boots with no loader errors. Two SkillData
  warnings move to S7: 94 renamed effect names with no handler (for example
  `MagicalAttack`→`MagicalDamage`) and the `OpNotAffectedBySkill` condition.
  The S7 work also covers five ignored skill tests and the missing
  AbnormalVisualEffect ids 247–250. Salvation data files with no consumer yet
  (agathions, alchemy, clan mastery, prime shop, daily missions, ensoul and
  others) get loaders together with their systems.

### S3 — Database (1–2 days)
**Done (2026-10-28)** in `m20261028_000001_salvation`:
- The 6 tables are created.
- `skills` and `level` are rebuilt on SQLite and altered on PostgreSQL.
- The 13 grand-boss rows are seeded with `ON CONFLICT DO NOTHING`.
- Entities are regenerated and `schema_parity` is extended.
- It is verified on SQLite and on PostgreSQL 17.

Items with nothing to do:
- `olympiad_fights.classed` already matched Salvation.
- The Zaken and Dr. Chaos rows are left in place. They are inert, because no
  `GrandBoss.ini` window names them.
- The paperdoll remap landed with the S5 layout, in `m20261029_000001_paperdoll_37`.

- Add one migration:
  - 6 new tables: `character_offline_play`, `character_offline_play_group`, `character_transmogs`, `item_special_abilities`, `mods_wedding`, `underground_coliseum`.
  - 3 column changes, done as SQLite table rebuilds following the `grandboss_real_hp` pattern: `buffer_schemes.skills` becomes varchar(512); `character_shortcuts.level` becomes not-null with default 0; `olympiad_fights.classed`.
- Regenerate entities (see DATABASE.md) and update `schema_parity`.
- **Paperdoll remap migration:** `items.loc_data` +5 for equipped slots ≥ 17 (see S5). Done in `m20261029_000001_paperdoll_37`. Still to do: test it against a copy of prod before the switch, since it cannot be undone cleanly on a live DB (`down` sends worn agathions back to the bag).
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

**Status (2026-10-29).** Steps 1–7 and 9–12 are done; step 7 landed with
the S5 paperdoll. The step 8 gate still needs a real p140 client.

Notes on what was done:
- **System-message table (step 12):** regenerated from Salvation's
  `SystemMessageId.java`. The colour column still comes from the Classic dat,
  so re-run `gen-messages` against the client's `SystemMsg-eu.dat` once one is
  available.
  - The expected compile-error sweep did not happen, because gameserver uses
    its own hand-kept `sm_ids`.
  - Checking those constants against both Java tables found 12 whose ids were
    off by one or more, in Interlude as well. They are fixed here.
  - No Rust NpcString table exists yet.
- **Unchanged packets:**
  - `MultiSellList` and `GMHennaInfo` already had the p140 bytes, because
    null ensoul is the same two zero bytes.
  - `ExChangeAttributeItemList` and commission are not ported, so there was
    nothing to re-layout.
- **Client reads (step 11):** the multisell and recipe fields that p140 adds
  all trail the packet, so the existing readers already accept them.
  `SetPrivateStoreListBuy` now reads its ensoul arrays.
- **Pledge windows:** they now carry the castle, clan hall and at-war fields.
  - The roster is now a single `PledgeShowMemberListAll`, followed by an
    update for each sub-unit member.
  - Forts and the clan rank (`ClanTable.updateClanRanks`, every 20 min) are
    still 0.
- **Kept Rust's corrected version of two Java display bugs:**
  - Java's `HennaEquipList` counts dyes the player does not hold.
  - Java's `HennaItemDrawInfo` preview sums the worn hennas instead of the
    candidate dye.

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
- **Carried over from S4:**
  - [x] `ExUserInfoEquipSlot` (38 slots).
  - [x] The agathion slots in `GMViewCharacterInfo` and UserInfo SLOTS, plus the
    talisman and brooch slot stats.
  - [x] LUC/CHA in UserInfo and the henna packets.
  - [x] The multisell product body-part mask (`BodyPart.getMask()`).
  - [x] `CharSelectionInfo` keeps its own 33-slot `PAPERDOLL_ORDER` when the shared
    order changes.
- **Gate:** a level-1 character of each starting race can be created and logs in. A migrated Interlude character keeps its equipped items in the right slots.

**Paperdoll done (2026-10-29):**
- 37 storage slots (agathions 17–21) and 38 wire slots. `BodyPart` masks
  widened to `i64` for `AGATHION`. The `bodypart` names follow
  `BodyPart.fromName`: `agathion`, `rear`, `lear`, `rfinger` and `lfinger`
  are added; `chest,legs` is dropped.
- `equipItem` is ported case by case: the formal-wear block, the hair/hair2
  rules, the ear/finger fallback, and the talisman, brooch-jewel and agathion
  runs sized by `PlayerStat`. The sizes are the maximum (6/6/5) until
  `EnterWorld` finishes, then the counter and stats from `TalismanSlot`,
  `JewelSlot` and `AgathionSlot`.
- `RequestUnEquipItem` reads a paperdoll index (`BodyPart.fromPaperdollSlot`).
  Two consequences come from Java: any slot of a run unequips the run's first
  slot, and the cursed-weapon guard can no longer fire.
- The try-on preview uses `Inventory.getPaperdollIndex`, which is the raw slot:
  two-handed and paired items are skipped, a slotless item costs a slot. The
  Kamael filters are ported.
- `ExUserInfoEquipSlot`, `GMViewCharacterInfo`, UserInfo SLOTS, the multisell
  mask and `ShopPreviewInfo` follow. Character select keeps its 33-slot order.
- The `items.loc_data` migration is `m20261029_000001_paperdoll_37`.

**Classes, levels and IdManager done (2026-10-29):**
- Classes need no code table: the 155 templates (with 190/191 and the
  Balthus 139–146 under `BalthusKnights/`) and the `CategoryType` groups are
  data. `BaseStat` gains `CHA`/`LUC`; templates read `baseLUC`, `baseCHA` and
  `basePSkillCritRate` (absent everywhere, so Java's default 5, for S6).
- LUC/CHA are summed like the other six: template + dyes (81 dyes carry
  `luc`/`cha`) + armor sets. UserInfo, `GMViewCharacterInfo`, `HennaInfo`,
  `GMHennaInfo` and the two dye previews write them.
- Level cap 110 was already in place from S2. Pets now follow
  `maxPetLevel`: exp stops one short of the level-111 row, so pets stop at 110.
  A species without that row earns nothing, as Java's exception does.
- `IdManager.ini`: `FirstObjectId`/`LastObjectId` bound the id counter; an
  exhausted range logs "Ran out of valid ids" and refuses creation.
- The dye preview windows write Java's columns, `getX() ± getHennaValue(X)`
  (the worn dyes' total); the port used to add the previewed dye instead.
- `CharacterCreate` checks the client's race (0–6, `AllowedPlayerRaces` with
  the new `AllowKamael`/`AllowErtheia`, Ertheia female-only).
- Gate: `e2e_create::every_starting_class_creates_and_enters_the_world`
  creates all 13 starting classes and enters the world with each. The
  migrated-character half of the gate is `paperdoll_37`'s test; a run on a
  copy of prod is still to do.

**Open:**
- The pet window's exp bar (`PetInfo` exp fields) is not ported; it writes 0.

**Open dependency:** equipped items' own skills (`ItemSkillsListener`,
`ItemSkillType.NORMAL`) are not ported. They are what grant the slot effects
in Salvation (bracelets open talisman slots, the agathion bracelets open
agathion slots), so in the world no talisman, brooch jewel or agathion can be
equipped until they land. Place this in S7 (skills) or S10 (items).

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

**Done (2026-10-29):**
- **Stats.**
  - CHA bonus in MaxHp/MaxCp/MaxMp, P./M.Atk, P./M.Def and both attack speeds (players only).
  - `SHOTS_BONUS` is `1 + enchant·0.7/100`, read through `asPlayer()`, so a servitor's shots stay at 1.
  - New `Stat` keys:
    - `CriticalRateSkill` (the `PhysicalSkillCriticalRate` effect);
    - the physical-skill and magic crit-damage `_ADD` pairs;
    - `DefenceMagicCriticalRate`/`_ADD`;
    - `DragonWeaponDefence`.
  - The `PSkillCritRate` finalizer: `basePSkillCritRate` 5 × STR bonus, clamped to `MaxPSkillCritRate` (2000, now read).
  - Magic crit rate is `calcWeaponPlusBaseValue`: base plus every piece's `mCritRate`.
  - `calcEnchantBodyPart`: R-grade +4 and up adds to:
    - P./M.Atk (chest, counted twice);
    - crit, magic crit and skill crit (legs);
    - accuracy (gloves);
    - evasion (head);
    - speed (feet).
  - `blessed` items take 1.5× in the R arms of every enchant table.
  - `SpeedFinalizer` runs in Java's order: a non-stance transform's `<moving>`, then the mount, `+ RunSpeedBoost`, × swamp, + feet. The GM speed cap is 10000.
- **Formulas.**
  - Crit rates use the 1000 scale with `[30, MAX_*]` clamps, and the defence stats act as reductions `1 − min(0.8, v − 1)`.
  - The magic level term fires when either side is 78+.
  - Crit damage is `1 + crit·max(0.2, 2 − def)`, plus `max(0, add − defAdd)`.
  - Shots are additive `2 + SB/100` everywhere except Heal.
  - `critMagicAdd` and `critPSkillAdd` (PhysicalDamage only, not HpLink).
  - Minimum damage 1 for auto-attacks, magic and physical skills.
  - `activateRate > 99` always lands.
  - Heal's grade arm reads the weapon's `crystalPlus`: R → 4, S → 2.
  - Dragon-weapon PvP defence.
- **Data.** The whole `AbstractStatPercentEffect` family parses as `PER` whatever `<mode>` says. Two `PvpMagicalSkillDamageBonus` effects omit it and had read as `DIFF`.
- **Re-baselined.**
  - `formula_parity` is retranscribed from Salvation's source: damage, heal, land rate, enchant tables, shots and all three crit arms.
  - The unit tests use hand-derived values.
  - `full_login_to_character_create` was hand-checked for a level-1 Human Mystic. STATS (3, 52, 599) and 8 starting skills (Commons adds Change Hair Accessory). The test now reads the p140 two-part `ItemList`.

**Open / carried over:**
- **Stat recalculation debounce:** not decided. Stats still recalculate inline.
- **Effect handlers for S7:**
  - `StatBonusSpeed` and `StatBonusSkillCritical` (two carriers each) are unported. Until then speed has no DEX-style bonus and the skill-crit stat stays STR.
  - The `*_CRITICAL_RATE` max-raise effects (`AddMaxPhysicalCriticalRate` etc.) are unported. One `AddMaxMagicCriticalRate` skill carries them.
  - Soul formulas: done — PhysicalSoulDamage and MagicalSoulDamage in S7 slice A, SoulBlow's 0.048 in slice B.
  - Physical skills deal no damage until S7 parses the `PhysicalDamage` names: done in S7 slice A.
  - Stat keys with no consumer yet: `PVE_DAMAGE_TAKEN_*`, `BONUS_RAID_POINTS` and `AGATHION_SLOTS` (the last also needs item skills).
- **Transforms:** `TransformLevelData.getStats(stat, default)` makes the 60 transforms with `<levels>` ignore `<base>`/`<moving>` at levels that have data. Rust reads `<base>`/`<moving>` only. Transform `getBaseDefBySlot` is unused.
- **NPC crit cap:** `npc_stats` caps NPC crit at the player cap. In Java NPCs are uncapped and only summons are capped.
- **Brooch jewels** (`SHOTS_BONUS` ruby/sapphire) only reach the stat once item skills land.
- **Deviations to clear in S11:** Rust refills HP/MP on level-up.
- **Test environment:** `dist/game/data/geodata` was emptied in the working tree on 2026-10-29. The 8 geodata-dependent tests (`geo::tests::loads_real_dist_geodata`, `npc_path_tests`, the tower-mob test) fail until it is restored.

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

**Slice A — damage renames (done 2026-10-29):**
- **Names.** The damage handlers parse under their Salvation names (`PhysicalDamage`, `MagicalDamage`, `MagicalDamageRange`, `MagicalDamageMp`, `PhysicalDamageHpLink`, `EnergyDamage`, `PhysicalDamageMute`, `TriggerSkillByDamageDealt`, `TriggerSkillByDamageReceived`), and the Rust variants carry the same names.
  - `PhysicalAttack` and `MagicalAttack` are now the P.Atk/M.Atk stat effects. About 1,700 dist effects had been read as damage.
  - `MagicalSuccessResistance` replaces `ResistDDMagic` in the registry.
- **New handlers.**
  - `PhysicalSoulDamage` (×(1 + 0.048·souls), raw trait, no crit add or `max(1)`, player casters only).
  - `MagicalSoulDamage` (mAtk ×(1 + 0.022·souls)).
  - Both take `souls = min(soulMaxConsumeCount, charges)` from the **charges** pool, as Mobius does.
- **Params.**
  - `PhysicalDamage`: `abnormalType`/`damageModifier`/`powerModifier` and `races`/`raceModifier`.
  - `MagicalDamage`: `debuffModifier` (Java's DEBUFF buff type, so triggered, toggle and dance debuffs don't count) and `races`/`raceModifier`.
  - `FatalBlow`: `abnormalType`/`abnormalPower`.
  - `EnergyDamage`: reads its own `chargeConsume`.
  - The skill fields `soulMaxConsumeCount`, `chargeConsume` and `isTriggeredSkill` are parsed.
- **Behaviour fixed on the way.**
  - `generalTraitMod` is multiplied raw (no `== 0 → 1` guard) by `calcBlowDamage`, `EnergyDamage`, `PhysicalDamageHpLink` and `PhysicalSoulDamage`.
  - HpLink's HP multiplier no longer scales a perfect block's 1.
  - `EnergyDamage` applies `PHYSICAL_SKILL_POWER` and `max(0)`, and its charge spend restarts the decay timer (`decreaseCharges`).
  - The skill-evasion gate now also covers `MagicalDamageRange`, `DeathLink` and the soul handlers.
  - `MagicalSoulDamage` no longer arms over-hit.
  - The mute family's `onStart`: `PhysicalDamageMute` aborts the swing; `PhysicalMute` aborts nothing (it had aborted cast and swing); `Mute` aborts the cast, raids exempt.
  - A `TriggerSkillByDamageReceived` trigger skill with SELF target is cast by its bearer.

**Triggers slice — done 2026-10-29.** Both `TriggerSkillByDamage*` handlers re-ported with every Salvation param:
- **Events:** `OnCreatureDamageDealt`/`Received` fire from the `reduceCurrentHp` analogs with a `DamageHit` context (DoT, skill, crit, reflect). That covers swings, skill hits, reflects, counters, zone damage, the servitor transfer and doors. On an NPC they fire after hate (`Attackable` order); the HP-block refusal moved after them, so a blocked mob still takes hate and still fires triggers.
- **Carriers:** live non-passive buffs, plus held passives whose `<passiveConditions>` pass (book, clan and augment grants; an NPC's template skills).
- **`SkillCaster.triggerCast`:** a shared port in `skills::cast::trigger`. It runs `checkCondition`, then `isSkillDisabled`, then a `disableSkill` lock (the new `DisabledSkills` component: no timestamp, not persisted). Then the affect scope, the `MagicSkillUse` broadcast (skipped while channeling) and the activation legs shared with a finished cast. A per-target queue (`World::trigger_casts`) reproduces Java's re-entrancy order.
- **Target handlers:** `resolve_npc_cast_target` became the creature-generic `resolve_handler_target`, taking `targetType` apart from the skill. `MY_PARTY` and `SUMMON` now follow Java for any caster, and `SUMMON` has no LOS gate.

**Magic type slice — done 2026-10-29.** `TriggerSkillByMagicType` re-ported with every Salvation param (`replace`, `skillLevelScaleTo`, `targetType`, `skillLevel` defaulting to 0). It fires through `trigger_cast` and the target handlers, and skips `CommonSkill` ids. The finish-cast event now fires where Java's `finishSkill` fires it: after the consumes and before `callSkill`, with the cast's own target. `triggerCast` fires it too, with the trigger's target. There is no loop guard, as in Java: a fired skill whose own magic type is in its carrier's list re-fires it. Real carriers are bounded by reuse, by chance, or by the fired skill dispelling the carrier (7097, 14839).

**`triggerCast` callers slice — done 2026-10-29.**
- **`trigger_cast`** gains Java's `ignoreTargetType = false` form (`trigger_cast_at_own_target`): the fired skill re-resolves its own `targetType` against the caster's selection after the reuse is spent. The finish-cast event still carries the original target.
- **`CallSkill`** reads `skillLevel` 0 (the effector's learned level) and `skillLevelScaleTo` (19 carriers). A called skill with a `hitTime` (20 of 103) checks `isSkillDisabled`, plays its `MagicSkillUse` and lands through a scheduled trigger cast (`CallSkillCast`).
- **Augment option triggers** use the `ignoreTargetType = false` form. The cast half now runs per affected target inside `callSkill`, before `activateSkill`, rolling each holder in turn.
- **Aura pulses** resolve through the target handlers for any effector and `trigger_cast` at each result. A synergy (`A6`) pulse lands on the holder first, and `onStart` no longer blocks its abnormal type.
- **NPC buffers** (arena, TvT, support magic) `trigger_cast`. The clan-hall manager's `castSkill` is `doCast`, so it now goes through the NPC cast (`npc::cast::cast_checked`).
- **`isNotBroadcastable()`** is Java's full set (`T`, `TG`, `AU`, `A5`, `A6`). It gates the player and NPC `MagicSkillUse` at cast start, `MagicSkillLaunched`, the toggle path and `trigger_cast`.
- **Not ported:** `skillSubLevel` on `CallSkill`. The skill model has no sub-levels, and the one carrier (Dissolve 11835's sub-levelled call) needs them.

**Sayune zones slice — done 2026-10-29.**
- **Data:** `SayuneData.xml` loads as `GameData.sayune` (the map tree of `loc`/`selector`/`choice`). `sayune_zones.xml`'s 84 `SayuneZone`s load with their `mapId`.
- **`SayuneZone`:** entering sets `ZoneId.SAYUNE` for a `SIXTH_CLASS_GROUP` player (or anyone with `FreeJumpsForAll`) who is neither transformed nor mounted, and starts `FlyMoveStartTask` (`ExNotifyFlyMoveStart` every second while listed in the zone and no flight is open). Leaving clears the flag.
- **Packets:** `RequestFlyMoveStart` (ex 0xAD) checks the summon, chaotic and other-request refusals, then opens a `SayuneRequest` on the zone's map. `RequestFlyMove` (ex 0x91) takes the next hop or the picked branch. Each hop sends `ExFlyMove`, broadcasts `ExFlyMoveBroadcast` to known players within 1000, then moves the player.
- **Elsewhere:** `checkUseConditions` refuses skills inside SAYUNE (SM 3648), and auto-use treats SAYUNE like peace.
- **Not ported:** `SayuneRequest.onLogout`, which is dead code in Java (only called from `onPlayerEnter`, where the request is always null). `hasRequests` covers the requests the port has: enchant, party invite and Sayune.
- **In practice** only Awakened classes (S8) or `FreeJumpsForAll` can jump, so the zones stay inert on a default config until S8.

**`reduceCurrentHp` remainder — done 2026-10-29.**
- **Amount after the damage events** (`combat::damage::damage_taken`): the `DAMAGE_LIMIT` cap (a GM attacker is exempt), then `PVP_DAMAGE_TAKEN` for a playable attacker or `PVE_DAMAGE_TAKEN` otherwise, then `PVE_DAMAGE_TAKEN_RAID` for a raid or raid minion or `PVE_DAMAGE_TAKEN_MONSTER` for a monster, each floored at -80. A player hit by a non-DoT skill with a cast range takes 0 when the attacker has no line of sight. The events still see the raw amount.
- **Effects:** `GetDamageLimit` and `DamageByAttack` (`PK`, `ENEMY_ALL`, `MOB`, `BOSS`) feed the new stats.
- **`reduceHp` for every creature:** a monster now gets the stun break and both creature kinds get the Real Target break (`Rnd.get(100) <= 3` on every non-DoT hit). The stun break follows `calcStunBreak`'s abnormal-time guard, so a stun doubled by Skill Mastery holds until it is back to its normal length. A GM whose access level has `giveDamage="false"` deals nothing.
- **NPC order** is `Attackable.reduceCurrentHp`'s: loot rights, `addDamage` (hate and the AI reaction, which now calls the minion pack), the raid curse, the second minion call, then the events and the HP cut. Before this, a damaging hit never called the pack.
- **Damage lines:** the attacker's `C1_HAS_INFLICTED` line reports the adjusted amount (moved into `reduceCurrentHp` by the next slice).
- **Kept:** the advanced-HQ halving deviation (S11).

**`sendDamageMessage` — done 2026-10-29.**
- **One sender** (`combat::damage_message`), called from the end of `apply_physical_damage` and door damage with the amount that landed, after the HP cut and any death. The call-site lines in the auto-attack and skill-damage paths are gone. Every `reduceCurrentHp` with an attacker now reports, so DoT ticks, reflect, counters, the servitor's Transfer Pain share, Open Chest and GM kills send the line too. Water and falling damage call it with the player as their own attacker.
- **Overrides:** `Player` (crit line, the `ExMagicAttackInfo` `CRITICAL` popup for a skill, blocked, `YOU_HIT_FOR_S1_DAMAGE` on a door or control tower, the damage line with the target as a plain string), `Summon` (to the owner: `PET_S_CRITICAL_HIT` / `SUMMONED_MONSTER_S_CRITICAL_HIT`, the line under the summon's NPC name, nothing when it hits its owner) and `Cubic` (to the owner, under the owner's name).
- **No miss lines:** Salvation never passes `miss`, so a missed swing sends no `C1_S_ATTACK_WENT_ASTRAY` / `C1_HAS_EVADED_C2_S_ATTACK`. The `DUELIST_FURY` term is always false (no skill carries the effect).
- **`Hp` / `HealPercent`:** both open with Java's dead/door/HP-blocked/raid gate. A negative amount is `reduceCurrentHp` (no counter, absorb or reflect) followed by a second `sendDamageMessage`. `Hp` now has that branch at all and truncates `PER` as Java's int arithmetic does.

**Gaps found while porting it** (each its own slice):
- **`isHpBlocked` includes `isInvul`.** The `reduceHp` gates still treat invulnerability apart: a DoT tick goes through GM invul in Java and not here, a teleporting player is not invulnerable here, and an NPC's `AdminFlags.invul` does not block its HP.
- **Olympiad damage tally.** `notifyCompetitorDamage` has nothing to feed: matches keep no per-side damage, and a timed-out match is a draw instead of going to the bigger damage dealer.
- **`Summon.reduceCurrentHp`'s owner line** (`C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2`). Java only reaches it from the three-argument overload (zone damage, Transfer Pain, counters, Open Chest, GM kills), not from `doAttack`.
- **`Hp` / `HealPercent` heal branches:** `Hp` sends no restored line, `HealPercent` skips `HEAL_EFFECT`/`HEAL_EFFECT_ADD` and caps at max HP instead of `MAX_RECOVERABLE_HP`, and neither adds `ADDITIONAL_POTION_HP`.
- **`RealDamage`** is not ported (13 skills).
- **`DamOverTime.onStart`'s magic-crit burst** goes through the skill-damage (`doAttack`) path; Java calls `reduceCurrentHp` with `isDOT` and `critical` set.
- **The other `ExMagicAttackInfo` types:** `OVERHIT` on the reward, `RESISTED` from `Formulas`, `CRITICAL_HEAL` from `Heal`/`HpCpHeal`.

**Souls and charges (slice B) — done 2026-10-29.**
- **Souls** (`skills::effects::souls`): `Player.souls` with `increaseSouls` (SM 2162), `decreaseSouls` (floors at 0, never refuses), `clearSouls` and the ten-minute `ResetSoulsTask`, staled by `souls_seq` like the charge task. `EtcStatusUpdate` now carries the count.
- **Effects:** `FocusSouls` (living player target, capped at `MAX_SOULS`, SM 2163 when full); `SoulEating`, whose pump feeds `MAX_SOULS` and whose exp listener absorbs a soul on a gain of at least `expNeeded` (SM 2186 at the cap, `ExSpawnEmitter` when the target is an NPC). The listener fires from `addExpAndSp` with the full `finalExp` before the level cap, and from the revive exp restore when there was a loss.
- **Force:** `MAX_MOMENTUM` is a real stat (base 0) fed by `FocusEnergy` (Sonic/Force Mastery and friends), replacing the hard-coded cap of 8. `FocusMomentum` caps at `min(maxCharges, MAX_MOMENTUM)`, skips the at-cap message for a triggered skill and sets charges without a client. `FocusMaxMomentum` and `GetMomentum` are ported; `GetMomentum` ticks on its own period (`GetMomentumTick`), not the skill's `DamOverTime` one. `setCharges` always restarts the decay task; `clearCharges` leaves it running, as in Java.
- **Cast finish:** `finishSkill` spends `soulMaxConsumeCount` souls, then `chargeConsume` charges (a short pool aborts the cast), before the item consume.
- **Conditions:** `SoulSaved` and `OpSoulMax` are real.
- **SoulBlow:** ×(1 + min(souls, soulMaxConsumeCount)·0.048) for a player caster, on the souls left after the spend.
- **Olympiad:** the end of a match clears souls and charges (`cleanEffects`).

**Gaps found while porting it:**
- **Olympiad `cleanEffects`** is otherwise unported: no effect strip, target drop, cast abort or HP/MP/CP refill at the end of a match.
- **Revive exp restore** writes the exp directly; Java's `addExp` path also recomputes the level and works off karma.
- **Tick chains end only with the buff.** A toggle switched off and back on within one period runs two `GetMomentum` (or `DamOverTime`) chains.
- **`OpSoulMax` on a summon caster** refuses; Java dereferences a null `asPlayer()`. No summon carries it.

**`SkillPowerAdd` — done 2026-10-29.** `SKILL_POWER_ADD` is a real stat, fed by `SkillPowerAdd` (the ten R-weapon special abilities) and by a `DIFF` `PhysicalSkillPower`/`MagicalSkillPower` (none on this dist; `PER` still multiplies). It is added to the skill's power in `calcMagicDam`, `calcBlowDamage`, `PhysicalDamage` (after the abnormal power mod), `PhysicalDamageHpLink`, `PhysicalSoulDamage` and `EnergyDamage`. `PhysicalDamageWeaponBonus` and `PhysicalDamageSaveHp` read it too, but neither handler is ported yet.

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
- **Carried over from S4:** Java's `getAvailableSkills` and
  `getNextAvailableSkills` selection for `AcquireSkillList`.

**Class change goes through Salvation's quests (user, 2026-10-29).** Quests stay deferred except these. Most class quests run through an instance script, so they come with the instance framework they need. The instance owners below are taken from each instance script's quest imports:

| Class | Quest | Instance |
|---|---|---|
| 1st | Q10331 Start of Fate | Labyrinth of Belis (178) |
| 2nd | Q10360 Certification of Fate | none: open-world Wastelands |
| 3rd | Q10341–10346 Day of Destiny (`ThirdClassTransferQuest`) | Evil Incubator (185) |
| 4th | Q10338 Seize Your Destiny, `AwakeningMaster` | Harnak Underground Ruins (195) |
| Ertheia | Q10751–10753 Winds of Fate | Fortress of the Dead (254, Q10752), Chamber of Prophecies (255, Q10753) |

Sub-slices, in order:
1. **Instance framework:** the full template (conditions, door `openStatus`, `respawnTime`, `<time duration>`); `InstanceScript.enterInstance` (enter group, conditions, world cap, already-in-another-instance and re-entry checks); `finishInstance(delay)` and the duration timer.
2. **Script hooks the instances use:** move-finished, zone enter/exit, level changed, NPC death whatever the killer, timers bound to an NPC and player (one-shot and repeating), movies, NPC follow and move-to.
3. **1st class:** Labyrinth of Belis and Q10331, including the tutorial question mark and popups.
4. **2nd class:** Q10360 in the Wastelands.
5. **3rd class:** Evil Incubator, `ThirdClassTransferQuest` and Q10341–10346.
6. **Awakening:** Harnak Underground Ruins, Q10338 and `AwakeningMaster` (`RequestChangeToAwakenedClass`).
7. **Ertheia:** Fortress of the Dead, Chamber of Prophecies and Q10751–10753.
8. **Retire the Interlude chain:** village-master change scripts, path quests, trials and sagas (with S9).

**Instance framework (sub-slice 1) — done 2026-10-29.**
- **Template:** `<time>` keeps Java's -1 defaults (never ending, never emptied out); `<enter>` has its FIXED/RANDOM/NONE type and every location; each door keeps its `<openStatus>` override; each spawn line keeps its `respawnTime`; `<conditions>` are read with their params.
- **`enterInstance`:**
  - An allowed player re-enters their own world; a different template gets SM 2105.
  - The enter group follows Java's group mask.
  - `Level` is checked (SM 2097), unless the player is a GM.
  - The world cap is checked (SM 3148), and so is "already allowed elsewhere".
- **Lifetime:**
  - `setDuration`/`cleanUp` send SM 2107 at five and one minute, then destroy.
  - `finishInstance(delay)` resets that clock.
  - A zero duration or empty time tears the world down when it empties.
- **Spawns:** an instance spawn line with `respawnTime` respawns into its own world. Instance spawns now send `NpcInfo`; before, a group spawned mid-instance (Frintezza's rooms) was never shown.
- **Scripts:** `QuestCtx` gets enter, status, params, spawn groups, single spawns, doors, finish and the NPC lookups.

**Gaps found while porting it:**
- **Entry conditions other than `Level`** refuse entry and log; none of the class-quest instances declares one.
- **`<reenter>`** has no store yet: no `character_instance_time`, no `C1_MAY_NOT_RE_ENTER_YET`.
- **`<time eject>`, `<misc>`, `<rates>`, `<removeBuffs>` and `<parameters>`** are still unread.

**Script hooks, first half (sub-slice 2) — done 2026-10-29.**
- **`getTimers()`:** a per-script `TimerExecutor` (`quests::timers`), keyed by event, NPC and player.
  - One-shot and repeating timers; adding the same key replaces the pending holder.
  - `cancelTimer` and `hasTimer` work as in Java.
  - NPC decay and player logout stop the task but keep the holder, as Java's `stopTimerHolders` does.
  - `StatSet` params and `onTimerCancel` are not ported; no class-quest script uses them.
- **`onMoveFinished`** fires for every NPC arrival, whatever its AI.
- **`onEnterZone` / `onExitZone`** fire on edges for players, tracked per zone a script listens on. Player logout exits them.
- **`ON_CREATURE_DEATH`** on an NPC fires whatever the killer, right after the dead flag is set.
- **`ON_PLAYER_LEVEL_CHANGED`** fires from the exp paths (`addExp`/`removeExp`, death exp loss, `//set_level`); a class or subclass switch does not fire it.
- **Fix:** zone `minZ`/`maxZ` are swapped into order for NPoly and Cuboid zones, as Java's forms do. Belis' damage zone (12014) is written the wrong way round and could never contain anyone.

**Script hooks, second half (sub-slice 2) — done 2026-10-29.**
- **`FriendlyNpcAI`:** a `FriendlyNpc` used to run no AI at all; it now thinks.
  - `thinkActive` attacks the most hated once the spawn calm runs out; it never scans.
  - `thinkAttack` swings at the stored target, with the crowd shuffle and the archer back-off.
  - It ignores playables' and other friendlies' blows. A monster's blow is hated but not answered at once.
  - Monsters may now target it (`FriendlyNpc.isAutoAttackable`).
- **Normal follow:** `startFollow(target)` re-closes to a fresh 50–100 every second and gives up past 3000. Any intention other than ATTACK stops it.
- **Script orders on `QuestCtx`:**
  - Follow, move-to (refused while casting, aborts the swing), run, heading and `broadcastInfo`.
  - `addDamageHate`, `reduceCurrentHp` and `doDie` between NPCs.
  - Display effect, `applyEffects` from an NPC, and `playMovie`. Movies and display effects were GM-only before.

**Gaps:**
- `disableCoreAI` is not modelled; Belis only sets it on a Folk, which has no AI.
- Java's IDLE intention maps to ACTIVE when a follow gives up.

**`Script` bypass — done 2026-10-29.** Salvation has no `Quest` bypass verb. `ScriptLink` (`Script`) replaces it, and 1,731 dist htmls use it.
- The router, the quest chooser's buttons, and the links that `CastleChamberlain`, `ClanHallManager` and `ClanHallAuctioneer` build now say `Script`.
- An event (`Script <name> <event>`, bare or `npc_<id>_`) follows `processScriptEvent`. It goes to the last folk NPC within 2D interaction distance, else to the last quest NPC (`_questNpcObject`, now its own component), else nowhere. The bypass's own NPC id plays no part.
- The tutorial window still routes `Quest <name> <event>` with no NPC. Only the Interlude Q255 uses that, and it leaves with the Interlude chain.
- `TutorialShowQuestionMark` now carries Salvation's mark type.

**First class transfer (sub-slice 3) — done 2026-10-29.** Start of Fate (10331) and the Labyrinth of Belis (instance 178) are ported.
- Q10331: Sebion starts it and opens the Labyrinth. Nemertess' death sets cond 2. Sebion then gives Saril's Necklace and the race's cond, and the race master's `classChange` changes the class and pays out. A class off the player's tree goes to `handleIllegalPlayerAction`.
- From level 18, login and the step onto 18 show question mark 10331 (unless `DisableTutorial`); its click opens the race popup.
- Belis: all four rooms, the officer's follow, move-to orders and lines, the attacker waves, the zone shock, both movies, and the instance failing when the officer dies.
- New `QuestCtx` helpers: say, delete and script value on any NPC; `with_other_quest`; `player_name`; `punish_illegal_action`.
- A deleted NPC's `npc_instance()` still names the instance it was spawned into, as Java's `_instance` does. Nemertess' 30 s timer depends on this.

**Gaps:**
- NPC spawns never play the summon animation (`isSummonSpawn`). Belis' attackers use it.
- An NPC's own target (`npc.setTarget`) is not modelled. Belis aims the officer at the generator with it, but the hate does the real work.

**Second class transfer (sub-slice 4) — done 2026-10-29.** Certification of Fate (10360) is ported. It needs no instance.
- The race masters start it once the player is 38, not Ertheia, and has completed Q10331 (else `restriction.html`). Humans and Elves have a mystic master and a fighter master each (`isRightMaster`).
- Renfad, Joel and Schuazen send the player after Regenerated Kanilov, Regenerated Poslof and Sakum, who spawn in the Wastelands. Schuazen plays `ExShowUsm` 4. Sakum drops the Shine Stone, and Selon then sets the race's cond (8–15).
- `classChange;<id>`: Java punishes only a class the current one descends from, or a cond under 8. Any other class goes through, the port included. The payout is 461,880 adena, potions, shots, Paulina's set, 2.7M exp, and multisell 718.
- Question mark 10360 shows on login from 38 and on the step onto 38, to `SECOND_CLASS_GROUP`. Unlike Q10331, Java checks neither `DisableTutorial` nor, on level-up, the quest state.
- New: `ExShowUsm`, `class_names::{class_name, is_mage}` (Java's `PlayerClass.isMage`, which is not `MAGE_GROUP`), and the `QuestCtx` helpers `is_mage_class`, `show_usm`, `show_multisell` and `show_npc_chat_window`.

**Third class transfer (sub-slice 5) — done 2026-10-29.** Day of Destiny (10341–10346) and the Evil Incubator (instance 185) are ported.
- `ThirdClassTransferQuest` and its six race subclasses are one `DayOfDestiny` with the master, race and id per quest. The master starts it from 76 for the race's `THIRD_CLASS_GROUP` (`-11`/`-12` otherwise) and stores `STARTED_CLASS`. The Quartermaster sends the player for the race tag of each of the four Vanguards, with Java's `1 << (33166 - npcId)` bits (0, 31, 30, 29). `classTransfer` moves to the class's single next class, with the 5103 cast, SM 1606, social action 3, the payout and 42M exp, and a repeatable `exitQuest`.
- Mark 101 shows from 76 on login and level-up (`DisableTutorial` respected), shared by the six quests, each opening its own race's invitation.
- Evil Incubator: the briefing, the two officers picked (the rest within 1000 of Adolph leave), waves 1–6 as each is cleared, the pause and the class pendant, waves 8–15 every 20 s, the class's Death Wound with the earthquake, and Adolph's Scroll of Escape. `managerWorldAttack` sends the friendly NPCs around the player at the monsters.
- **Deliberate difference:** Java's `SPAWN_WAVE` and `WORLD_ATTACK` timers carry no NPC or player, so all Evil Incubator worlds share one key and a second world's timer cancels the first's. The port puts the world id in the event, so each world keeps its own.
- New: `SkillTreeData::next_classes`, `ExShowScreenMessage`'s effect flag, and the `QuestCtx` helpers `with_player_quest`, `instance_first_player`, `instance_screen_message_npc_string`, `instance_earthquake`, `visible_npcs`, `npc_is_friendly`, `npc_is_attackable_class`, `npc_attack_desire`, `npc_teleport` and `broadcast_social_action`.

**Awakening (sub-slice 6) — done 2026-10-30.** Seize Your Destiny (10338), Harnak Underground Ruins (instance 195) and the `AwakeningMaster` script are ported.
- **Q10338:**
  - Cellphine starts it from 85 for `FOURTH_CLASS_GROUP`. An Ertheia gets `-08`, a Judicator or an active subclass nothing, anything else `-07`.
  - Cellphine sends her `-01.htm` as a plain chat window, as Java does.
  - Hadel sets cond 2 and opens Harnak. The Wraith's death sets cond 3.
  - Hermuncus gives the Scroll of Afterlife and 400 Steel Door Guild Coins, and exits the quest (repeatable). His `TELEPORT` goes to the Reliquary with movie 55.
- **Harnak:**
  - First room: the eight guards are called up in turn. Striking out of turn sets status 0 and turns the room on the player.
  - Second room: three waves of the player's category monster (`MOB_CATEGORY`), with the Maximum Defense source on the third.
  - Third room: the opening movie and the Wraith's room with the Light Heal source. The Wraith's on-screen lines at 80/60. At 50 the seals, their guards, `ExSendUIEvent` and a 60 s fail clock.
  - On failure: `removeNpcs`, movie 48 and `finishInstance(0)`. On success: movie 47 and Hermuncus 25 s later.
  - Java's timers key on name, NPC and player. The port's quest timers key on name per player, so the NPC id goes into the name (`event#npc`).
- **`AwakeningMaster`:**
  - `awakening` checks the scroll, level above 84 and `FOURTH_CLASS_GROUP` (`-no.htm`), then the master's category (`-no_class.htm`), then sends `ExChangeToAwakenedClass` (a Female Soul Hound gets Feoh Soul Hound). An Ertheia's first talk is `ertheia.html`; Q10472's branch is not ported.
  - The client's `RequestChangeToAwakenedClass` (ex 0xA2) fires a new global hook, `on_change_to_awakened_class`. It applies Java's gates (subclass, level, hero or unclaimed hero, inventory under 80%, mounted or transformed), spends the scroll and moves the class.
  - Payout: SM 1308, social action 20, the `SIXTH_*_GROUP` power item and two Chaos Pomanders.
  - Skills: `cleanSkillUponAwakening` keeps the class's own max-level entries, `removeSkill` and awakening-save ids, alchemy skills and any inventory item's skill, and removes the rest with their shortcuts. The race skill tree is added, and `ExShowUsm` 10 follows 10 s later.
- **Java quirks kept:**
  - The Seal Control Device is registered for first talk, but the script has no `onFirstTalk`, so a click shows nothing and `activate_seal` cannot be reached. Only the Wraith's death frees Hermuncus.
  - `seal_say` never advances its counter, so each seal repeats its 50-seconds line every 10 s.
- **New:**
  - Packets: `ExChangeToAwakenedClass`, `ExSendUIEvent` and `NpcSay` with a chat type.
  - Instances: `instances::remove_npcs`.
  - Skill trees: `SkillTreeData::{kept_upon_awakening, race_skill_tree}`.
  - `QuestCtx` helpers: `start_quest_timer_on`, `npc_say_typed`, `send_plain_html`, `send_ui_event`, `send_change_to_awakened_class`, `instance_all_alive_npcs`, `remove_instance_npcs`, the awakening gates, `clean_skills_upon_awakening`, `add_race_skills` and `send_skill_list`.

**Gaps:**
- Dual class is not modelled, so `isDualClassActive` is always false: any subclass is refused, and the dual-class pomander never drops.
- The `CharacterConfig` default `maximum_player_level` is still Interlude's 81. The dist's `Player.ini` sets 110 in production; the awakening tests set it themselves.

**`AcquireSkillList` selection — done 2026-10-29.** `available_skills` now applies all of Java's filters: no autoGet or forgotten-scroll entries, none of the class's `<removeSkill>` cache, no level above the skill's highest loaded one, and, for an awakened learner, only the class's own entries plus its awakening-save skills. `next_available_skills` lists every next level from `getMinLevelForNewSkill` up to `PLAYER_MAXIMUM_LEVEL`. Both end with the known-skill unlearn pass. The list is empty while transformed.

**Gaps found while porting it:**
- **`hasReplacedSkill`** has no counterpart; only `ReplaceSkillBySkill` feeds it, and that effect is not ported.
- **Dual class** is not modeled, so an awakened Ertheia never counts as awakened in the selection.
- **Java's list order** comes from a `HashSet`; the port keeps tree order.

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
  - The recipe craft model (`RecipeHolder`). This includes the p140 craft
    offerings and the `RequestRecipeShopMakeItem` manufacture-price check,
    whose fields are already sent by the client.
  - Carried over from S4:
    - Enchant item options (`EnchantItemOptions.xml`, item mask bit 4).
    - Ensoul and visual ids in the item block and the multisell/store reads.
    - The `MultiSellList` maintain-enchantment display: Java's fallback to
      the first unequipped ingredient, and the augment and elemental blocks.
    - The crystallize effect, which the UserInfo crystallize byte reads.
    - The premium (4th) henna slot.
- **Clans:**
  - `ClanLevelData` exp-based levelling.
  - Mastery and development points.
  - Clan shop.
  - Contribution.
  - The pledge V2 packets that go with these: 14 client and about 11 server.

### S11 — Remove the deviations (2 days)
Per D8, nothing from [CUSTOM_DIST_DEVIATIONS.md](CUSTOM_DIST_DEVIATIONS.md)
carries over. Remove each Rust-side behaviour together with its tests, and
restore Salvation's data where the dist was edited:
- Cruma Tower 3rd-floor coordinate.
- Advanced HQ half-damage fix: Salvation's behaviour, bug included.
- `StrictDelevelSkillRemoval`: the config side is done in S1, and only the
  `strict` parameter remains.
- Over-enchant accessory ceiling.
- `HtmCache` override.
- Shadow Weapon coupon buttons and the Formal Wear (Q33–Q37) levels: take
  Salvation's as shipped.
- Drop Q214 and the TvT thaw entries, and close CUSTOM_DIST_DEVIATIONS.md with a
  pointer to `interlude`.

### S12 — Content (open-ended)
**Quests are out of scope for now (2026-10-28).** Mechanics and config parity
come first; items 1 and 3 below wait until those are settled.

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
