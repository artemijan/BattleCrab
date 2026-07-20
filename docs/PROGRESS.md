# l2r_interlude — Milestone Progress & State

Living status tracker for the Java→Rust rewrite. Plans:
[PLAN_LOGIN_SERVER.md](PLAN_LOGIN_SERVER.md) (login, M0–M5) and
[PLAN_GAME_SERVER.md](PLAN_GAME_SERVER.md) (game, G0–G14). Architecture:
[CONCURRENCY_MODEL.md](CONCURRENCY_MODEL.md),
[JAVA_TO_RUST_CHALLENGES.md](JAVA_TO_RUST_CHALLENGES.md).

**Legend:** ✅ done · 🚧 in progress · ⏳ not started.

**Porting convention — scoped-out behavior gets a TODO at the site.** When a
port intentionally skips part of the Java behavior (side effect deferred to a
later milestone, branch needing state we don't have yet), leave a
`TODO(G<N>): …` comment at the exact spot in the Rust code, naming what the
Java source does (e.g. "Java also fires `EVT_FORGET_OBJECT` at the AI here").
Never silently drop a Java side effect — that's how parity bugs like the
missing `TargetUnselected`-on-visibility-drop happen. The G13.9-style TODO
sweeps rely on these markers being greppable. Also: Java packet side effects
often hide in overrides (`Player.setTarget(null)` broadcasts `TargetUnselected`
includeSelf) — check the `Player`/`Creature` override chain, not just the
method named at the call site.

**The Java repo's `dist/` data and config are the source of truth — assume they
are 100% correct.** The XML/SQL/`.ini` datapack is retail-faithful; when Rust
behavior diverges from what that data implies, the bug is in the port, not the
data. Read the dist data as the spec and fix the Rust side — never edit the
datapack to match the port, and never write off a datapack value as "wrong"
(e.g. the Elven Ruins "to village" → Giran Harbour bug was a missing RespawnZone
port, not a bad `respawn.xml`).

---

## Snapshot

| Phase | Milestone                                                   | Status |
|-------|-------------------------------------------------------------|---|
| Login | M0–M5                                                       | ✅ feature-complete, interop-verified with Java GS |
| Game  | G0 Scaffold & boot                                          | ✅ |
| Game  | G1 Client link & cipher parity                              | ✅ |
| Game  | G2 Login-link + auth                                        | ✅ |
| Game  | G3 Character selection & persistence                        | ✅ |
| Game  | G4 Enter world (Player, HP/MP, UserInfo, enter-world burst) | ✅ (incl. paperdoll/mask enums) |
| Game  | G5 Items & inventory                                        | ✅ vertical slice (items, equip/unequip, initial gear) |
| Game  | G6 Stats, skills & effects                                  | ✅ vertical slice (stat engine, skill learn/cast, buffs) |
| Game  | G7 Movement & targeting (no geodata)                        | ✅ |
| Game  | G7.5 Full single-target skill casting                       | ✅ (real cast timing/formulas, reuse, abort, nukes/heals/buffs on others) |
| Game  | G7.8 Geodata & position validation                          | ✅ (`.l2j` loading, LOS, move clamping, ValidatePosition — zones still ⏳) |
| Game  | G7.85 Pathfinding (path-worker service)                     | ✅ (`CellPathFinding` port, dedicated worker thread + channels, multi-segment route following for player moves — NPC moves still straight-line) |
| Game  | G7.9 Region-grid visibility & scoped broadcasting           | ✅ (CharInfo/DeleteObject, 3×3 region knownlist, region-scoped broadcasts) |
| Game  | G8 Static world content (NPCs/spawns)                       | ✅ vertical slice (34.9k NPCs spawned, visible, targetable, talkable — zones/doors still ⏳) |
| Game  | G9 Combat & AI                                              | ✅ vertical slice (auto-attack, monster AI, death/decay/respawn, XP/SP/level-ups, auto-loot drops, die→revive) |
| Game  | G9.5 ECS stage 2 — split components, one world              | ✅ (plan: [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md)) |
| Game  | G9.6 Macros & panel shortcuts                               | ✅ (plan: [PLAN_MACROS_SHORTCUTS.md](PLAN_MACROS_SHORTCUTS.md)) |
| Game  | G10 Social systems                                          | ✅ vertical slice (chat, party, friends — clans/mail/BBS deferred) |
| Game  | G11 Scripting engine + quests (+ clans via bypass)          | ✅ vertical slice (bypass routing, quest engine, Q00258/Q00320, clan creation — plan: [PLAN_G11_QUESTS_CLANS.md](PLAN_G11_QUESTS_CLANS.md)) |
| Game  | G12 Static world + script/content breadth                   | ✅ vertical slice (zones peace/water/no-restart, all 1180 doors + geo collision, static objects, Link/Buy bypasses, +10 quests with on_attack/on_spawn hooks, OrcChange1, TeleportWithCharm — plan: [PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md](PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md)) |
| Game  | G13 Admin / GM command system                               | 🚧 G13.A framework done; **G13.B portable handlers landed** (B1–B7 + mounts + transform runtime: character/skill/item/spawn/movement/GM-util/world/vitality/ride/transform + geo queries + `//admin` menu); remaining: only subsystem-blocked C-group (sieges/olympiad/instances/…) + a few field-less/serializer stubs — plans: [PLAN_G13_ADMIN.md](PLAN_G13_ADMIN.md), [PLAN_G13_B_LOGIN.md](PLAN_G13_B_LOGIN.md) |
| Game  | G13.9 TODO parity sweep                                     | ✅ UserInfo weapon-enchant + party/clan relation; skill-acquire SMs; restoration enchant roll; stat-cap/run-speed config plumbing; skill-cooldown persistence (`character_skills_save`) — plan: [PLAN_G13_9_TODO_SWEEP.md](PLAN_G13_9_TODO_SWEEP.md) |

**Remaining subsystem breadth — [ROADMAP.md](ROADMAP.md) (G14→G33).** The old
single "G14 Long tail" is broken into per-subsystem milestones; each unblocks a
gated-but-bodiless admin handler, so admin parity == Java parity. A 2026-07
audit of the Java surface added six milestones the breakdown missed (G15.5
teleporters/user-commands, G15.7 crafting, G20.5 recommendations, G24.5 boats,
G26.5 lottery/monster-race, G30.5 item auction), per-milestone audit
additions, and a Classic/custom scope gate — see ROADMAP.md.

| Game  | G14 Item stats & equipment combat accuracy                  | ✅ item `<stats>`/weapon+armor bonuses (earlier) + **shields (`calcShldUse`)** + **`//setparam`/`//unsetparam`** (fixed-stat override); armor sets → G19; `SHOTS_BONUS` stat a noted micro-gap (only `reducedSoulshot` weapons) |
| Game  | G15 Economy & item actions                                  | 🚧 destroy + **ground items** (drop/pickup/visibility/auto-loot=false/decay) + **personal warehouse** (deposit/withdraw+persist) + **crystallization** + **merchant sell** + **private sell store** + **player trade** landed; **enchant** (chance engine `EnchantData` + full Ex-packet scroll flow: use→add→put-target→enchant, success +1 / safe / blessed / destroy+crystallize; item `etcitem_type`/`enchant_enabled` parse) + **clan warehouse** (shared container on `Clan`, `depositc`/`withdrawc` bypass + `ActiveWarehouse` routing + `CL_VIEW_WAREHOUSE` gate, persisted via `StoreClanWarehouse`) + **freight withdraw** (`Freight` container, `package_withdraw`, `loc="FREIGHT"` persist; unified 3-way `ActiveWarehouse` routing) + **augmentation** (`VariationData` roll engine + refine flow: confirm→refine→cancel, life stone rolls two options, consumes gemstones, stamps `ItemInstance` augment, adena cancel fee; shown via `paperdoll_augmentation`, persisted via `item_variations`) + **enchant support items** (`EnchantSupport` load + validate, put/remove 0x4A/0xE4, bonus-rate + random-step on the roll) + **item-skill cast branch** (`ItemSkillsTemplate`'s instant `triggerCast` vs `useMagic`: `withoutAction` + `immediate_effect`/`ex_immediate_effect` parsed, so scrolls now cast for their real `hitTime` — SoE 20 s, Scroll: Might 4 s — instead of firing on double-click; `checkConsume` ported via `default_action`/`itemConsumeId`) landed — augment option effects, freight send half pending; `SKILL_REDUCE_ON_SKILL_SUCCESS` still consumed by the handler rather than `finishSkill` (TODO(G15), needs the item threaded through `Casting`) |
| Game  | G15.5 Teleporters & user commands                           | 🚧 **gatekeepers live** (`TeleporterData` — all dist lists; `showTeleports`/`showTeleportsHunting`/`teleport` bypasses gated on the Teleporter class; fee suffix + adena charge, free ≤ `MaxFreeTeleportLevel` (40), karma gate) + **`/unstuck`** (`BypassUserCmd` 0xB3 → 30 s escape cast of 2099 via forced hit-time, GM 2100; new `Escape TOWN` skill effect → map-region town respawn; `teleport_player` now runs `teleToLocation`'s full prologue — `ActionFailed` + `abortCast()` (`MagicSkillCanceled`, or the escape FX kept playing at the destination for the client's own 5-minute skill duration) + `setTarget(null)` — before `decayMe`) + **`/loc`** (region `locId` SM + coords) + **newbie support magic** (`bypasshandlers/SupportMagic` + `SupportBlessing`: `SupportMagic`/`SupportMagicServitor`/`GiveBlessing` verbs on the Newbie Helper/Guide/Gatekeeper htms → fighter/mage buff sets + Blessing of Protection 5182, gated on level/class-tier via `CategoryData`; NPC cast animation; `ProtectionBlessing` lands icon-only). Pending: teleport bookmarks, remaining user commands (`/time` needs game clock), Mon/Tue fee discount (wall clock), nobles lists (G17), siege gates (G24), servitor buffs + Vampiric/Concentration/Cubic effects + PK-damage immunity (TODO(G-pvp)) |
| Game  | G15.7 Crafting & recipes                                    | ✅ vertical slice — recipe book (learn via recipe item / destroy / open — including the "Common Craft" 1322 / "Dwarven Craft" 1321 skills, whose `OpenCommonRecipeBook`/`OpenDwarfRecipeBook` effects open the window), synchronous self-craft (material+MP/HP consume, success roll, masterwork rare), and manufacture stores (set list / click→sell list / buy-a-craft with adena fee). `AltGameCreation=False` so no staged craft/XP; `StoreRecipeShopList=False` so stores are transient. Plan: [PLAN_G15_7_CRAFTING.md](PLAN_G15_7_CRAFTING.md) |
| Game  | G16 Character variables, premium & vitality                 | ✅ **admin main-menu slice landed** (`//admin` Item/Teleport/Spawn/ListPos/ListSpwn/goPosition/goSpawn/PC-Points/NCoins/Premium/Open/Close/Heal/Full-Food — plan: [PLAN_G16_ADMIN_POINTS.md](PLAN_G16_ADMIN_POINTS.md)): character-scoped `pccafe_points`, account-scoped `account_gsdata` "PRIME_POINTS" store (`//primepoints`), boot-loaded `account_premium` cache + write-through (`//premium_*`), `ExPCCafePointInfo`, spawn-line `tele_index`; Full-Food a pet-blocked `TODO(G29)` stub. **Henna slice landed** (plan: [PLAN_G16_HENNA.md](PLAN_G16_HENNA.md)): `HennaData` (372 dyes) + `HennaSlots` component, dye stat bonus folded into `BaseStats` (= template + Σ dyes, recomputed on draw/remove), `character_hennas` load/persist, the full `RequestHenna*` packet family + `HennaInfo`/`HennaEquipList`/`HennaRemoveList`/`HennaItemDrawInfo`/`HennaItemRemoveInfo`, SymbolMaker `Draw`/`Remove` bypass; permanent dyes only (`duration=-1` on this dist). **Vitality + variables + premium effects slice landed** (plan: [PLAN_G16_VITALITY.md](PLAN_G16_VITALITY.md)): `character_variables` key/value store (`PlayerVariables` component, load + transactional persist), the vitality pool (`game_loop/vitality.rs` — clamped 0..=140k, set/update with the 4 notices, `ExVitalityPointInfo`, party-window field), the ×2 exp/sp bonus folded into `add_exp_and_sp`'s new `use_bonuses` arg (quest/admin rewards opt out, like Java's 2-arg overload), per-kill consumption (`Attackable.getVitalityPoints`, solo + party branches), a real `Custom/PremiumSystem.ini` loader + `hasPremiumStatus` + PremiumRateXp/Sp on the reward path, real `ExVitalityEffectInfo` fields, and `StartingVitalityPoints` at creation. Remaining: the daily/weekly refills (`TODO(G33)` — needs the wall-clock daily-task scheduler, so **vitality only drains** today), vitality *items* (counter stored but nothing increments it), PC_CAFE_RETAIL_LIKE, and the `VITALITY_CONSUME_RATE`/`BONUS_EXP` stats (`TODO(G19)`) |
| Game  | G17 Sub-classes, class change & nobless                     | ✅ **nobless landed** (plan: [PLAN_G17_NOBLESS.md](PLAN_G17_NOBLESS.md)) — `characters.nobless` was **read at login and dropped on the floor**: it never reached `Player`, nothing consumed it, and it wasn't in the save UPDATE, so it couldn't be set either. Now `Player.is_noble` + `nobleSkillTree.xml` (**8** skills — the tree loader skipped every non-`classSkillTree` block, so this file had never been parsed) + `//setnoble` mirroring `//sethero` + persistence. Noblesse teleport lists now check nobless instead of refusing everyone. **One rule differs from hero deliberately**: `setHero` only grants while on the base class, `setNoble` has no such gate — nobless belongs to the character, so a subclass keeps it (tested, and it matters once subclasses land). **Subclasses landed** (plan: [PLAN_G17_SUBCLASSES.md](PLAN_G17_SUBCLASSES.md)) — **G17's gate headline**. Nothing existed: `class_index = 0` was hard-coded in six places in `db.rs` (each commented "no subclasses on this dist") and `character_subclasses` shipped in the schema but was never read or written. Now `Player.class_index` + `subclasses`, `add_subclass`/`set_active_class`, `StoreSubClass`, and `//setsubclass`/`//changesubclass`. **The banking is the whole mechanic**: class/level/exp/sp belong to the *active* slot, so a switch must write the current slot back before loading the target's (Java calls `store()` before touching `_classIndex`). The base class needed the same treatment and had nowhere to go — its `characters` row holds whatever class is active, so a level-7 base who switched to a level-40 subclass would return as level 40; `Player` now stashes `base_level`/`base_exp`/`base_sp` for that round trip, pinned by a test. Narrowed: **per-subclass skills aren't persisted yet** — a switch re-derives the auto-granted tree via the same `set_level` path `//setclass` uses, so a *manually learned* skill is lost on the round trip; `character_skills` needs a real `class_index` key, which is the next slice. Hennas/shortcuts still load at index 0. **Per-subclass skill books landed** (plan: [PLAN_G17_SUBCLASS_SKILLS.md](PLAN_G17_SUBCLASS_SKILLS.md)) — closing that gap: `character_skills` is now read and written per `class_index`, a switch banks the outgoing book and restores the incoming one (Java's `removeSkill`-all → `restoreSkills` → `rewardSkills`), and **a character who logs out on a subclass logs back in on it** (the active index is whichever subclass row carries `characters.classid`). The regression test — a hand-learned skill surviving a switch away and back — fails against the previous slice. **Per-subclass hennas + shortcuts landed** (plan: [PLAN_G17_SUBCLASS_HENNA_SHORTCUTS.md](PLAN_G17_SUBCLASS_HENNA_SHORTCUTS.md)) — same `class_index` treatment; dyes re-fold into `BaseStats` on the swap via `apply_henna_change`, which also pushes `HennaInfo` exactly as Java's `setActiveClass` does. **Village-master subclass flow landed** (plan: [PLAN_G17_VILLAGE_MASTER_SUBCLASS.md](PLAN_G17_VILLAGE_MASTER_SUBCLASS.md)) — the mechanic was GM-command-only; now the `Subclass` bypass on the dist's **46** VillageMasters drives it (menu/add-list/change-list/add/change). **Level 75 + free slot are enforced on the action, not just the list**, so a stale link can't slip past. `available_subclasses` ports `getAvailableSubClasses`: every `THIRD_CLASS_GROUP` entry minus the base **lineage** (Java's "similar class" rule), minus held classes and their children, minus Overlord/Warsmith, minus the Elf↔Dark-Elf cross. **Class race needed a lineage walk** — `PlayerTemplate::race()` only answers for *creatable* classes, so an advanced class returns `None` and the Elf rule would have silently disabled itself. Tested against the **real datapack** so the hierarchy/category groups are the shipped ones. Survey note: **certification skills are absent from this dist** (later-chronicle) — struck rather than stubbed. **Occupation change landed** (plan: [PLAN_G17_OCCUPATION_CHANGE.md](PLAN_G17_OCCUPATION_CHANGE.md)) — `Player.setClassId` as a shared mechanic, and it **fixes a hazard the subclass slices created**: `//setclass` set `base_class_id` unconditionally, which was harmless with one slot but would **rewrite the character's base class while standing on a subclass**. Java updates only the active slot (`getSubClasses().get(_classIndex).setClassId(id)`), touching `_baseClass` solely on the base slot. Now: base slot moves both, a subclass moves only its own stored class (re-persisted so it survives a restart), plus `rewardSkills`, the stat/UserInfo refresh, and Java's class-change flash (`MagicSkillUse` 5103). `//setclass` is rewired onto it. The key regression test was **verified to fail against the old behaviour** before being kept. Pattern worth remembering: when a new axis appears (here "which slot am I on"), every existing writer of the affected field becomes suspect, not just the new code. **Skill cooldowns per class index landed** (plan: [PLAN_G17_SKILL_COOLDOWNS.md](PLAN_G17_SKILL_COOLDOWNS.md)) — **G17 complete**. Expected to bank cooldowns per slot like skills/hennas/shortcuts; reading `setActiveClass` first showed Java calls **`resetTimeStamps()`**, i.e. a switch **wipes** them — which also closes the exploit of parking a long reuse on one class and sitting it out on another. Also fixed the IO: reuse rows now load and save under the *active* class index, where before a character on a subclass saved its cooldowns onto the base slot. Buff restore (`restore_type = 0`) remains unported and was never G17's. **Certification skills struck** — no data on this dist |
| Game  | G18 Clans — full                                            | ✅ **all 8 slices landed — G18 COMPLETE** (plan: [PLAN_G18_CLANS.md](PLAN_G18_CLANS.md)) — **slice 1** membership lifecycle: invite flow (`RequestJoinPledge` 0x26 → `AskJoinPledge` via the shared `PendingRequest` transaction slot → `RequestAnswerJoinPledge` 0x27 with Java's re-checked `checkClanJoinCondition` guard chain: CL_JOIN_CLAN priv, self/wrong-target, clan oust-penalty SM 231, already-clanned SM 10, rejoin-penalty SM 760, academy-eligibility SMs, per-type member caps — the full `getMaxNrOfMembers` table ported), accept burst (`JoinPledge`/`PledgeShowMemberListAdd`/`PledgeShowInfoUpdate`/`ExPledgeCount` 0xFE:0x13D + clan skills + Clan Advent on join), **leave** (`RequestWithdrawalPledge` 0x28: leader/combat gates, 1-day rejoin penalty), **oust** (`RequestOustPledgeMember` 0x29: CL_DISMISS, target-combat gate, dual penalty — oustee rejoin + clan-side `char_penalty_expiry_time`), **dissolve/recover** (village-master bypasses: guard chain incl. castle/siege-registration/siege-zone, 7-day `dissolving_expiry_time` + leader death-XP penalty, `ScheduledTask::ClanDissolve` re-armed at boot, recover cancels), shared `removeClanMember` teardown (title/skills/advent/window/UserInfo + `RemoveClanMember` column reset incl. offline members). New persistence: `characters.clan_join_expiry_time`, `clan_data.char_penalty_expiry_time`/`dissolving_expiry_time`. **Slice 2: level-up + rep-gated skill learning landed** — village-master `increase_clan_level` (`Clan.levelUpClan` Classic cost ladder: 1k SP+150k adena → 15k SP+300k adena → 100k SP+100 Blood Mark → 1M SP+5k → 5M SP+10k, dissolution gate SM 551, not-met SM 1790, consumption SMs 672/301/302/538, level-up FX `MagicSkillUse` 5103) via the existing `set_clan_level` (now + SM 1771 to the leader crossing level 5); `learn_clan_skills` → `showPledgeSkillList` (`ExAcquirableSkillListByClass` 0xFE:0xFA type PLEDGE, SM 607 / NoMoreSkills.htm / NotClanLeader.htm branches); `RequestAcquireSkill` PLEDGE branch + `RequestAcquireSkillInfo` 0x73 (rep cost via `AcquireSkillInfo` 0x91) — prev-level + clan-level hack checks, rep spend through `add_clan_reputation` (SM 1787, insufficient SM 1852), grant through `add_clan_skill`; `levelUpSp` now parsed from `pledgeSkillTree.xml` (`available_pledge_skills`/`pledge_skill` lookups). **Slice 3: ranks & power grades landed** — `RequestPledgePower` 0xCC (`ManagePledgePower` 0x2A answer; leader action-2 edit → `Clan.setRankPrivs`: store + `clan_privs` upsert + live mask/UserInfo refresh on online holders + `broadcastClanStatus`, rank 9 clamped to the academy-bestowable subset), `RequestPledgePowerGradeList`/`MemberPowerInfo`/`SetMemberPowerGrade`/`MemberInfo` ex 0x13/0x14/0x15/0x16 (`PledgePowerGradeList` 0x3D / `PledgeReceivePowerInfo` 0x3E / `PledgeReceiveMemberInfo` 0x3F; re-rank: CL_MANAGE_RANKS gate, leader untouchable, SM 1761 + roster broadcast + `characters.power_grade` persist), `RequestPledgeReorganizeMember` ex 0x2C parsed as Java's own same-type early-out (TODO(G18.6)); **enter-world now derives privileges from the rank table** (Java `Player.restore`: leader → all-bits + grade 1, member → `getRankPrivs(powerGrade)` with grade defaulting to 5 — the stored `clan_privs` column never wins), join sets grade 5 + `getRankPrivs(5)`; **delegated leader transfer** (`change_clan_leader`/`cancel_clan_leader_change` bypasses: stamp + persist `clan_data.new_leader_id`, 9000-07-* confirmation htmls; application at daily reset = TODO(G33) `DailyTaskManager.onClanLeaderChange`); members carry `power_grade`/`title` (loaded with the roster + `clan_privs` rows). **Slice 4: clan wars landed** — `ClanWar` model (`clan_wars` table, boot-restored + re-armed): `RequestStartPledgeWar` 0x03 (full guard chain: level 3/15 members, CL_PLEDGE_WAR, 30-war cap, dissolving target, 21-day post-defeat gate; counter-declaration → MUTUAL via `mutualClanWarAccepted`), `RequestStopPledgeWar` 0x05 (500-rep cease-fire, member-in-combat gate), `RequestSurrenderPledgeWar` 0x07 (`ClanWar.cancel`: winner set, `SurrenderPledgeWar` 0x67, torn down seconds later — Java's live path, the 5/21-day retention constants are dead code there), `RequestPledgeWarList` ex 0x17 → `PledgeReceiveWarList` 0xFE:0x40; **7-day BLOOD_DECLARATION timeout** (`ScheduledTask::ClanWarTimeout` → TIE, state-checked so MUTUAL no-ops it); **kill pipeline** (`ClanWar.onKill` from the death path outside PVP/siege zones: 5 attacked-side kills force MUTUAL with SM 3815 progress; mutual kills move `ReputationScorePerKill`=1 between clans with SMs 3811/3812, victim clan ≤0 rep exempt); **war PvP rules**: `checkIfPvP` + `isAutoAttackable` mutual-war legs, death-XP penalty ÷4 vs a war enemy (`apply_death_exp_penalty_ex`), war swords on `RelationChanged` (0x4000 declared / +0x8000 mutual per Java `getRelation`), dissolve now really rejects at-war clans (SM 264). **Slice 5: alliances landed — the ROADMAP gate (form clan/invite/level/learn skill/declare war/form ally) is now fully met.** `create_ally`/`dissolve_ally` village-master bypasses (`Clan.createAlly` guard chain SMs 504/502/549/505/550/506/507/508; dissolution broadcasts SM 523 ally-wide, clears every member clan, stamps penalty type 4), `RequestJoinAlly` 0x8C → `checkAllyJoinCondition` (ally-leader-only, penalty types 1–3 gates, target-leader checks, both-in-siege-zone SM 723, at-war SM 469, `AltMaxNumOfClansInAlly`=3 cap) → `AskJoinAlly` 0xBB via the `PendingRequest` slot → `RequestAnswerJoinAlly` 0x8D (re-checked; target clan folded in, Java's wrong friend-added SM 525 kept), `AllyLeave` 0x8E / `AllyDismiss` 0x8F (penalty types 1 / 2+3), `RequestDismissAlly` 0x90, `RequestAllyInfo` 0x2E (`AllianceInfo` 0xB5 + the SM 491–500 cascade); ally id/name persisted (`clan_data` ally columns) and **now shown everywhere** — `Player.ally_id` denormalized (synced at enter-world/join/leave/destroy) into UserInfo/CharInfo, ally name/id in `PledgeInfo`/`PledgeShowInfoUpdate`/`PledgeShowMemberListAll`; war-declare same-ally gate SM 1569 and dissolve-clan ally gate SM 554 now real (ally crests TODO(G18.7)). **Slice 6: sub-pledges & academy landed.** `create_academy`/`create_royal`/`create_knight` village-master bypasses share one `createSubPledge` port (level gates 5/6/7, name validation/clash-across-all-clans, leader-eligibility + "clan leader can't captain a sub-unit" reject, `getAvailablePledgeTypes` family-slot resolution — 1 academy / 2 royal / 4 knight — reputation cost `CreateRoyalGuardCost`=5000/`CreateKnightUnitCost`=10000, `PledgeReceiveSubPledgeCreated` 0xFE:0x41); `rename_pledge`/`assign_subpl_leader` bypasses; **academy/royal/knight invites now accept** (`RequestJoinPledge`/`AnswerJoinPledge` no longer refuse pledge type ≠ 0 — validated against the clan's founded sub-pledges, academy grants power grade 9 vs the usual 5); `RequestPledgeReorganizeMember` ex 0x2C does a real two-member pledge-type swap (CL_MANAGE_RANKS-gated); a departing sub-unit captain's slot goes vacant (`removeClanMember`'s "leadssubpledge" branch) — persisted via new `clan_subpledges` rows + `characters.subpledge`. **`Clan::pledge_class_of` fully widened** to `ClanMember.calculatePledgeClass`'s per-level academy/royal/knight-member/captain tiers (levels 6–11), verified against a hand-transcribed reference table in a dedicated unit test; `PledgeShowMemberListAll` now correctly filters to main-pledge members only (Java's per-tab window; sub-unit tabs themselves are TODO(G18.6c), cosmetic-only). Apprentice/sponsor academy-graduation rewards deferred (TODO(G18.6b) at the join site — no consumer yet, needs G22 class-change wiring). **Slice 7: crests landed; notices audited and found unreachable on this dist.** `CrestTable` port (`World.crests` + `next_crest_id`, never-reuse-the-last-id semantics) backs three crest kinds: small pledge crest (`RequestSetPledgeCrest`/`RequestPledgeCrest` 0x09/0x67, ≤256 bytes, level-3 + CL_REGISTER_CREST + dissolving gates), large pledge crest (`RequestExSetPledgeCrestLarge`/`RequestExPledgeCrestLarge` ex 0x11/0x10, ≤2176 bytes, chunked `ExPledgeEmblem` 0xFE:0x1B answer), and ally crest (`RequestSetAllyCrest`/`RequestAllyCrest` 0x91/0x92, ≤192 bytes, alliance-leader-only, pushed to every member clan). Crest ids now render for real in `PledgeShowInfoUpdate`/`PledgeShowMemberListAll`/`GmViewPledgeInfo` (read straight off `Clan`) and in `UserInfo`/`CharInfo` (denormalized `Player.clan_crest_id`/`ally_crest_id`, synced at enter-world, clan join, crest set/delete, and every ally membership change — closes the last four TODO(G18.7) markers from slice 5); a test caught a real bug where the ally-crest setter broadcast stale UserInfo without updating members' denormalized field first, now fixed. **Clan notices were audited, not ported**: `Clan._notice`/`isNoticeEnabled`/the `EnterWorld` login-popup display path exist in Java, but grepping the full gameserver tree found no caller of `setNotice`/`setNoticeEnabled` anywhere — this Interlude Classic build ships the read/restore/display plumbing with **no in-game way to ever set a notice**, so there is nothing reachable to port faithfully; documented here rather than silently dropped. **Slice 8: recruitment registry landed — G18 is now COMPLETE.** `ClanEntryManager` port: `World.recruit_waiting`/`recruit_clans`/`recruit_applicants` + 5-minute re-registration locks (`recruit_player_lock`/`recruit_clan_lock`, tick-based), persisted via `pledge_recruit`/`pledge_waiting_list`/`pledge_applicant` (boot-loaded with Java's orphan-clan cleanup). The board (`RequestPledgeRecruitBoardAccess`/`Detail`/`Search` ex 0xD5/0xD6/0xD4 — register/update/remove a clan's listing, real unsorted/sorted-by-name/level/karma/by-name search with 12-per-page paging replacing the slice-1 empty stub), the applicant queue (`RequestPledgeWaitingApply`/`Applied`/`List`/`User`/`UserAccept` ex 0xD7-0xDB — apply → leader alarm ping → view queue/one applicant → accept, reusing `add_clan_member`, or reject), the global waiting list (`RequestPledgeDraftListSearch`/`Apply` ex 0xDC/0xDD — clanless players register themselves, leaders search by level/name/sort), and open-joining instant self-join (`RequestPledgeSignInForOpenJoiningMethod` ex 0x111, gated on the same char-penalty/join-expiry/member-cap checks as a normal invite). `RequestPledgeRecruitApplyInfo` now answers real ORDERED/WAITING/DEFAULT status. **Pledge bonus (`ClanRewardData`, members-online/hunting clan-wide skill rewards) deferred with TODO(G33)** — it needs a daily-reset scheduler that doesn't exist yet (same gap noted for G16 vitality and the delegated-leader-transfer application). |
| Game  | G19 Skills & effects breadth                                | 🚧 **affect scopes + toggles landed** (plan: [PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md)): `AffectScope` SINGLE/RANGE/POINT_BLANK/PARTY/PLEDGE + `AffectObject` ALL/NOT_FRIEND/FRIEND/CLAN in `skills/affect.rs` (affectLimit cap with Java's `min + Rnd.get(max)` quirk, dead-skip, caster-skip, peace-zone leg, LOS from the target), the cast pipeline fanned out over the affected list (`apply_cast_consequences` per target — effects + PvP flag + hate), and **toggles** (recast = off, `toggleGroupId` exclusion, instant cast per `SkillCaster`'s short circuit, new `targetType NONE`). **Abnormal-state flags + crowd control landed** (plan: [PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md)): Java's `EffectFlag` mask ported as per-`ActiveBuff` flags folded on read (`game_loop/abnormal.rs` — no cached mask to invalidate), `BlockActions` (540 uses — stun/sleep/paralyze) and `Root` (79) effects, and the gates that read them (no attack/cast/move while stunned, no move while rooted, NPC AI silent while stunned, rooted mobs stay put), plus the mid-action interrupt (abort cast *then* freeze movement — the other order lets `stop_casting` resume the walk). Before this a stun landed, showed its icon and changed nothing. **Abnormal resistance/blocking + probabilistic dispel landed** (plan: [PLAN_G19_ABNORMAL_RESIST.md](PLAN_G19_ABNORMAL_RESIST.md)): `ResistAbnormalByCategory`→`Stat::ResistAbnormalDebuff` folded into `calc_effect_land_rate` as Java's `buffDebuffMod` (multiply then clamp, so Guts halves incoming debuff chance), `ResistDispelByCategory`→`ResistDispelBuff` (pumped but consumer-less until `Cancel` lands — Java reads it only in `calcCancelSuccess`), `BlockAbnormalSlot` (Prophecy mutual exclusion, stamp-and-fold like the CC flags) and `DispelBySlotProbability` (the Bane family's per-buff rate roll). **Ranking note:** unported effects must be ranked by *learnable-skill* usage, not raw instance count — `StatUp` looks like the biggest gap at 887 instances but is only 9 learnable skills (the rest are talisman/Freya/agathion content). **Periodic HP/MP + healing/CP breadth landed** (plan: [PLAN_G19_PERIODIC_EFFECTS.md](PLAN_G19_PERIODIC_EFFECTS.md)): `HealOverTime` (negative power = the upkeep toggles' HP cost, floored at 1) and `ManaDamOverTime` joined the existing DoT tick chain, with an out-of-MP tick switching a **toggle** off + SM 140 (Java's `false` return, honoured only for toggles); `HealEffect` (HEAL_EFFECT mul / _ADD diff, read off the *recipient*) folded into the Heal path; `Cp` instant restore/drain with DIFF/PER. Closes a loop: the toggles ported in the first G19 slice now actually cost HP/MP. The empty-effects guard's third exemption was generalised into `has_periodic` — any effect with no stat modifier must join *periodic*, *icon-only* or *state flag* or it is silently dropped. **CC breadth landed** (plan: [PLAN_G19_CC_BREADTH.md](PLAN_G19_CC_BREADTH.md)): `Mute`/`PhysicalMute` (magic vs non-magic cast gate in `checkDoCastConditions`, static skills exempt, mutually exclusive), `DebuffBlock` (incoming debuffs bail outright ahead of the resist roll; buffs unaffected), `BlockControl` (item-use gate — Java's wider summon/mob-control meaning is G29) and `TargetCancel` (chance-rolled instant: drops the target via `set_target(None)` so `TargetUnselected` broadcasts, and aborts attack+cast). Landing a mute also aborts the victim's in-flight cast, with **raid bosses immune** to that interrupt. `Fear` is the CC hold-out — it needs forced flee movement, so it belongs with G21's AI breadth. **Abnormal visual effects landed** (plan: [PLAN_G19_ABNORMAL_VISUALS.md](PLAN_G19_ABNORMAL_VISUALS.md)): the cosmetic half of all the CC above — `AbnormalVisualEffect` id map + `<abnormalVisualEffect>` parsed, stamped on `ActiveBuff` and folded on read; `CharInfo` (which hard-coded a count of **0**, so nobody ever saw an effect on anyone) and `ExUserInfoAbnormalVisualEffect` now carry the real set; pushed **only when the set changes**, as Java does. Plus `//ave_abnormal` toggling a GM-pinned visual via a new `AdminVisuals` component folded alongside the buff-derived ones. Remaining AdminEffects AVE handlers (`//setteam`, `//settargetable`, `//set_displayeffect`, `//playmovie`) are unblocked but need their own per-creature state + packet fields. Before this, only SINGLE resolved — every one of the datapack's 1900+ area skills hit exactly one target. **Transformation landed** (plan: [PLAN_G19_TRANSFORMATION.md](PLAN_G19_TRANSFORMATION.md)): the "Transform <Monster>" scroll family (32 learnable skills — Grail Apostle, Unicorn, Doom Wraith, Zaken, …), wired into the existing G13.B `//transform` admin runtime (`Player.transform_id`/`TransformData`) via the skill-cast path — `admin::transforms` split into state-only and state+broadcast halves so the buff-landing path can fold the transform-specific extras onto the `UserInfo` it already sends rather than double-broadcasting; reverts on `BuffExpire`, which (since death already routes stripped buffs through the same removal fn) covers death for free. Cast-time gate ports `ConditionPlayerCanTransform`'s already-transformed/in-water/cursed-weapon-equipped legs (`DefenceAttribute`, the next effect on the raw-count list at 33 learnable skills, is Kamael-era elemental attributes and out of scope). **MpConsumePerLevel landed** (plan: [PLAN_G19_MP_CONSUME_PER_LEVEL.md](PLAN_G19_MP_CONSUME_PER_LEVEL.md)): the MP-upkeep half of the core fighter toggles (Accuracy 256, Guard Stance 288, Vicious Stance 312, War Frenzy 424, Super Haste 7029, …) — each already lands a real `StatModifier`, but this *other* effect on the same skill was silently dropped, so every one of these toggles was a free, uncosted buff. Every instance in the datapack is a toggle with no `abnormalTime`, collapsing Java's formula to `ManaDamOverTime`'s `power * getTicksMultiplier()`, so it shares that effect's tick-chain arm rather than duplicating it (periodic drain, self-deactivate + SM 140 on insufficient MP); the level-scaled `abnormalTime > 0` branch is unexercised by this datapack and left a TODO. Also fixed `admin_superhaste_applies_and_persists`, whose zero-MP test setup broke once Super Haste's own drain (Java's `AdminSuperHaste` casts through the real `applyEffects` path) started applying. **ShieldDefence/ShieldDefenceRate landed** (plan: [PLAN_G19_SHIELD_DEFENCE.md](PLAN_G19_SHIELD_DEFENCE.md)): Shield Mastery (153), a passive every shield-using class can learn, pumps both stats — `ShieldDefenceRate` was already parsed (`EFFECT_REGISTRY`) but never actually read (`game_loop::combat::shield_stats` used the equipped shield's raw `rShld` directly, bypassing `StatModifiers`); `ShieldDefence` wasn't parsed at all. Both now fold through `model::finalize` (bumped `pub(crate)`) over the shield's own `sDef`/`rShld`, gated behind the existing no-shield-equipped early return so a flat buff still contributes nothing without a shield, matching `Formulas.calcShldUse`'s short-circuit. `EnergyAttack` (9 learnable) set aside — needs the unmodeled Dwarf Force/Charges resource first. **HealPercent landed** (plan: [PLAN_G19_HEAL_PERCENT.md](PLAN_G19_HEAL_PERCENT.md)): all 5 learnable instances are core priest kit — Miracle (1426), Benediction (1271), Restore Life (1258), Revival (181), Touch of Life (341) — every one of which parsed to an empty effect list and healed nothing. New match arm mirrors `Heal`'s NPC-silent/player-with-SM split and overheal clamp, computing the amount as a max-HP percentage rather than the magic-formula power, and skipping `Heal`'s recipient `HealEffect`/`HealEffectAdd` scaling (Java's real asymmetry). Surfaced `TargetType::EnemyNot` as unmodeled (falls through to `Other`, silently no-op'd by `use_magic_on`) while testing Restore Life. **`TargetType::EnemyNot` landed** (plan: [PLAN_G19_ENEMY_NOT_TARGET.md](PLAN_G19_ENEMY_NOT_TARGET.md)): "any friendly selected target" — the precise inverse of `Enemy`/`EnemyOnly`'s `is_auto_attackable` gate, no force-use override, self always allowed, exempt from the general dead-target rejection ("works on dead targets or doors as well"). Small (34 instances) but it was quietly capping the two `HealPercent` skills that heal someone other than the caster (Restore Life, Touch of Life). `AttackTrait` (7 learnable) set aside — needs a `TraitType` attacker-bonus system unmodeled on this port. **Force/charges landed** (plan: [PLAN_G19_FORCE_CHARGES.md](PLAN_G19_FORCE_CHARGES.md)): unblocks `EnergyAttack`, set aside twice before. New `Player.charges` resource (transient, never persisted) backs Sonic Focus → Sonic Blaster/Buster and the Orc/Dark Elf Force Burst/Storm/Blaster family — 9 `EnergyAttack` + 6 `FocusMomentum` learnable skills all parsed to empty effect lists before this. `FocusMomentum` gains charges capped at `max_charges.min(8)` (Java's `MAX_MOMENTUM` stat is never set anywhere in this datapack, so `8` is the real cap, not a simplification); `EnergyAttack` shares `PhysicalAttack`'s damage core times a new `1 + charge×0.1` boost, reading `chargeConsume` off a skill-level tag rather than the effect's own params. `EtcStatusUpdate` (0xF9) now carries the real charge count. Deferred: Java's 10-minute charge-decay task, `GetMomentum` (dead code — nothing sets `MAX_MOMENTUM`), and wiring the charge bonus into `PhysicalSoulAttack`/`MagicalSoulAttack`/`SoulBlow`'s existing `×1` stand-ins. **Lethal landed** (plan: [PLAN_G19_LETHAL.md](PLAN_G19_LETHAL.md)): `AttackTrait` set aside a third time — needs the cross-cutting `TraitType` system, not a slice. `Lethal` (9 learnable) was already flagged as a TODO on `SkillEffect::Blow`'s own doc comment — every learnable instance pairs it with an already-ported damage effect (Backstab 30, Lethal Blow 344, Deadly Blow 263, Critical Blow 409, Lethal Shot 343, Turn/Banish Undead/Seraph), so those skills' damage landed but the bonus instant-kill/half-kill chance never rolled. Level gate + raid-boss immunity (reusing `Mute`'s own `is_raid()` check) ported; full/half-lethal rolls set a player's CP (and HP, on a full lethal) to 1 or halve a monster's HP, with `chanceMultiplier` at 1.0 (no trait/attribute math anywhere on this port). `INSTANT_KILL_RESIST` isn't rolled — like `MAX_MOMENTUM`, nothing in this datapack ever sets it. **AttackTrait landed** (plan: [PLAN_G19_ATTACK_TRAIT.md](PLAN_G19_ATTACK_TRAIT.md)): the last item on the learnable-skill ranking, investigated properly instead of deferred a fourth time. All 7 learnable instances (Detect Insect/Beast/Animal/Dragon/Plant Weakness, Eye of Hunter/Slayer) use only the `*_WEAKNESS` category of `TraitType` — and the consuming formula turns out inert on the real Java server too (`calcWeaknessBonus` needs a matching NPC-side `DefenceTrait`, and nothing in this datapack ever sets one — grepped the whole Java tree, one call site, its own definition). Lands as an icon-only buff, closing a real regression (the effect wasn't recognized at all, so it didn't even land) without inventing damage-formula wiring for a bonus that's provably inert either way. Collateral: `NpcTemplate.race`/`Race` extended from 6 playable races to Java's full 26-member shared enum (players + creature categories) — costs nothing today, ready for when NPC-side trait data lands. **DamageBlock landed** (plan: [PLAN_G19_DAMAGE_BLOCK.md](PLAN_G19_DAMAGE_BLOCK.md)): the highest raw instance count left (5 learnable, 84 skills, 162 instances — a skill carries two `<effect>` elements, one `BLOCK_HP` one `BLOCK_MP`), already flagged by two existing TODOs on `HealPercent` and `Lethal`. The five learnable instances (Celestial Shield 1418, Flames of Invincibility 1427, Dance of Medusa 367, Sonic/Force Barrier 442/443) are short full-invulnerability shields. `HP_BLOCK` has a real single choke-point consumer in Java (`CreatureStatus.reduceHp`), matched by threading a new `is_dot: bool` parameter through `game_loop::combat::apply_physical_damage` — already the one function every damage path on this port funnels through — with an early return, exempting only DoT ticks (damage zones are *not* exempt, matching Java's `DamageZone`). `MP_BLOCK`/`isMpBlocked()` is the same "genuinely dead code in Java too" pattern as `MAX_MOMENTUM`/`INSTANT_KILL_RESIST`: zero callers anywhere in the Java tree, folded for completeness but wired to nothing. Both existing TODOs closed. **EnlargeSlot landed** (plan: [PLAN_G19_ENLARGE_SLOT.md](PLAN_G19_ENLARGE_SLOT.md)): a re-run of the ranking sweep with `EFFECT_REGISTRY`'s generic stat-modifier table correctly excluded (it had been quietly absorbing dozens of effect names and inflating earlier raw counts) surfaced this on top — Expand Inventory/Warehouse/Trade/Common Craft/Dwarven Craft (5 learnable, 162 raw instances). A `type`-selected `Stat` passive (6 new variants: `InventoryNormal`, `StoragePrivate`, `TradeSell`, `TradeBuy`, `RecipeDwarven`, `RecipeCommon`), folded through `model::finalize` into `UserInfo`'s INVENTORY_LIMIT block, `ExStorageMaxCount` (previously all six capacity fields were Java's static placeholder defaults, one literally commented "`Stat.INVENTORY_NORMAL` not wired"), and `crafting::learn_recipe`'s recipe-book cap, the one consumer with real enforcement behind it — warehouse deposit and private-store listing still aren't capacity-checked anywhere on this port (`TODO(G29+)`), so only the *number reported* changed for those. Surfaced and fixed a wider pre-existing gap along the way: a newly learned passive skill only took effect at the next login; `RequestAcquireSkill` now also calls `recompute_conditioned_passives` (already generic under its armor-swap framing), so any stat-modifier passive applies the moment it's learned. **Hate-manipulation effects landed** (plan: [PLAN_G19_HATE_EFFECTS.md](PLAN_G19_HATE_EFFECTS.md)): a tied cluster of six related effect names sharing one already-ported primitive (`AggroList`) — rather than take the top name alone and defer the rest a fifth time (the `AttackTrait` pattern), bundled the four cheap ones: `GetAgro` (Aggression, Aggression Aura, Judgment, Tribunal), `AddHate` (Charm, Lure), `DeleteHate` (Eva's Serenade, Peace, Repose), `DeleteHateOfMe` (Bluff, Forget, Trick) — 12 learnable-skill instances. `GetAgro` needed the most care: the ported AI derives its attack target fresh from `AggroList::most_hated` every think tick rather than caching a "current target," so "force intend-attack the caster" became "make the caster's hate dominant" (current max + 1) rather than a direct intention override. `DeleteHate`/`DeleteHateOfMe` both disengage via a newly `pub(crate)` `npc_ai::set_active`, shared with `think_attack`'s own timeout/leash disengage rather than duplicated. Deferred: `TargetMe` (paired with `GetAgro` on the same 2 skills) needs a locked-target UI concept nothing on this port has; `RandomizeHate` (Confusion, Switch) needs a general nearby-visible-creatures query `faction_call`'s NPC-only neighbour scan doesn't provide; `GetAgro`'s clan-mate pre-seed is left to `faction_call`'s own reactive recruit, at most one think-tick later. **DispelByCategory landed** (plan: [PLAN_G19_DISPEL_CATEGORY.md](PLAN_G19_DISPEL_CATEGORY.md)): the "Cancel" family (Cancellation, Cleanse, Purification Field, Touch of Death), another tied cluster at 4 learnable skills — picked over the cheaper `PhysicalAttackRange` (a same-shape repeat of the already-solved `ShieldDefenceRate` pattern, no new value) because it closes a real gap flagged two slices ago: `Stat::ResistDispelBuff` was pumped but "consumer-less until `Cancel` lands." Unlike `DispelBySlot`/`DispelBySlotProbability` (a fixed abnormal-type list), this steals *whatever* is up — `BUFF` slot walks dances then buffs in reverse cast order, each gated by a ported `calcCancelSuccess` (`clamp(rate + (casterMagicLvl - buffMagicLvl)*2 + (buffAbnormalTime/120)*ResistDispelBuff, 25, 75)`, skipped as automatic when `rate>=100`); `DEBUFF` slot uses a flatter `roll<=rate` (Java's exact operator, not this codebase's usual `<`). The dances-before-buffs split and most of `canBeStolen()`'s exclusions came free from the already-ported `BuffSlot` classification. Java's `ALL` slot is dead code too, and stays a no-op here. Deferred: `isIrreplacableBuff()`/hero/GM/static-skill exclusions (unmodeled fields, matching `DispelBySlotProbability`'s own precedent). **PhysicalAttackRange landed** (plan: [PLAN_G19_PHYSICAL_ATTACK_RANGE.md](PLAN_G19_PHYSICAL_ATTACK_RANGE.md)): Archery/Long Shot/Rapid Fire/Snipe, the cheapest of the tied-at-4 cluster `DispelByCategory` was picked from — a same-shape repeat of the already-solved `ShieldDefenceRate`/`AttackCancel` pattern, needing only an `EFFECT_REGISTRY` entry and wrapping `recalculate_stats`' bare `combat.atk_range` line in `finalize()` (the same gap `ShieldDefenceRate` itself had before an earlier slice). All four learnable instances are `<weaponType>BOW</weaponType>`-conditioned; the condition mask is already generic across every registry entry, so nothing extra was needed to gate correctly — proven by a test showing the bonus is inert while unarmed. **FatalBlowRate landed** (plan: [PLAN_G19_FATAL_BLOW_RATE.md](PLAN_G19_FATAL_BLOW_RATE.md)): Assassination/Critical Blow/Focus Death/Mortal Strike, another tied-at-4 pick — directly tied to the already-ported `Blow`/`Lethal`/`FatalBlow` mechanics, since `formulas::calc_blow_success`'s own doc comment flagged `Stat.BLOW_RATE`/`BLOW_RATE_DEFENCE` as hardcoded identity. Same `EFFECT_REGISTRY` wiring as `PhysicalAttackRange`; the formula gained one `blow_rate_mod` parameter multiplied into the existing rate expression, threaded from the caster's finalized `StatModifiers`. `Stat.BLOW_RATE_DEFENCE`/`FatalBlowRateDefence` is genuinely dead in Java too — a registered handler no shipped skill grants — matching the recurring `MAX_MOMENTUM`/`INSTANT_KILL_RESIST` pattern. **Fear landed** (plan: [PLAN_G19_FEAR.md](PLAN_G19_FEAR.md)): the CC hold-out the CC-breadth slice deferred to "G21's AI breadth" — **G21 is complete**, so the forced-flee movement it needed now exists. Top of the in-scope ranking at 8 learnable skills (Horror 65, Banish Undead 405, Banish Seraph 450, Fear 1092, Curse Fear 1169, Word of Fear 1272, Mass Curse Fear 1381, Turn Undead 1400); everything above it is out of scope (`DefenceAttribute` 31 — Kamael elemental attributes) or G29 (`Summon`/`SummonCubic`/`SummonNpc`, 24/12/9). Reading the Java shrank the port twice: **`EffectFlag.FEAR` has no reader** (no `isAfraid()`, nothing `isAffected(FEAR)` — a feared creature is *not* gated out of attacking, casting or walking) and **`EVT_AFRAID` has no handler**, both the recurring `MP_BLOCK`/`MAX_MOMENTUM` "dead in Java too" pattern, so the entire mechanic is `fearAction`'s repositioning: 500 units away from the caster on `onStart`, then along the victim's *own heading* every 5-tick beat (Java passes `null` for the effector on repeats, so they keep running the way the first shove threw them rather than being re-aimed at a caster who may be dead by then). Shares the existing DoT tick chain rather than growing a scheduler; `canStart` ports the raid and `Defender`/`FortCommander`/`SiegeFlag`/`SIEGE_WEAPON` carve-outs. The load-bearing piece is **`NpcIntention::MoveTo`**: `AttackableAI.onEvtThink`'s switch has **no `AI_INTENTION_MOVE_TO` case**, so a fleeing mob thinks about nothing until it arrives — without it the next think tick re-issues the chase and drags the mob straight back, making the flee invisible (`onEvtArrived`'s `MOVE_TO`→`ACTIVE` reset ported alongside, off a new `TickOutcome.arrived`). **This was a quiet gap, not a loud one:** every Fear skill also carries the already-ported `BlockControl`, so the buff always landed — icon, duration, `BLOCK_CONTROL` flag — and the debuff looked like it worked; it just never moved anyone. Deferred: `canStart`'s `isSummon()` leg (`TODO(G29)`). **StatByMoveType + the player regen stat pipeline landed** (plan: [PLAN_G19_STAT_BY_MOVE_TYPE.md](PLAN_G19_STAT_BY_MOVE_TYPE.md)): picked from a three-way tie at 4 learnable (`StatByMoveType`/`MagicalAttackMp`/`SilentMove`) because two of its four skills — Vital Force 148 and Clear Mind 1297 — carry *only* this effect and so parsed to an empty effect list and were **dropped whole**, passives that did precisely nothing. Behind it sat a much bigger gap the ranking is structurally blind to: the sweep counts *unported effect names*, and `HpRegen`/`MpRegen`/`CpRegen` are in `EFFECT_REGISTRY` — but **`regen_player` never read `StatModifiers` at all**, so all 21 learnable regen skills (Focus Mind 191, Mana Recovery 214, Regeneration 1044, Song of Life 265, Victories of Pa'agrio 1414, …) pumped a stat nobody consumed, the same "parsed but unconsumed" shape as `ShieldDefenceRate`/`PhysicalAttackRange`. Real scope: **25 learnable skills, not 4**. `regen_player` now ends in Java's `Stat.defaultValue` (`mul*base + add + getMoveTypeValue(stat, getMoveType())`) for all three of HP/MP/CP, and the hard-coded standing multiplier became the real `Creature.getMoveType`-driven block (sitting 1.5 / standing 1.1 / running 0.7 — and **walking falls through every branch for no multiplier at all**, so walking regen is *worse* than standing still; Java as written, now pinned by a test), retiring a stale `TODO(G7)`. `StatByMoveType` itself rides on a new `StatModifierEffect.move_type` qualifier, so the entire buff pipeline (landing, stacking, removal, passive folding) needed no changes; `apply_modifier` routes it to a separate `StatModifiers::by_move_type` map — Java's own `_moveTypeStats`, deliberately *not* folded into `add`, which would apply the bonus in every locomotion state instead of the one it names — read live against the current move type, so the value swings as the player stands/walks/runs with no stat recompute. Acrobatic Move 225's evasion (the one non-regen use) folds in at `combat::combatant()`'s per-attack snapshot rather than the cached `CombatStats`, matching Java's on-demand finalizer. Deferred: `MoveType::Sitting` (no source — sitting isn't modeled, `TODO(G29)`; parsed and stored so it starts applying for free once it lands), the zone/residence regen multipliers, and the tie's other two effects. **Critical-damage stats landed** (plan: [PLAN_G19_CRITICAL_DAMAGE.md](PLAN_G19_CRITICAL_DAMAGE.md)): found by running the *previous* slice's post-mortem check first — the name-based ranking is structurally blind to "parsed but unconsumed" stats, so this time every `Stat` variant was swept for consumers outside `stats.rs`/`skill_data.rs`. Exactly two came back with **zero readers**: `CriticalDamage` and `CriticalDamageAdd`. All three damage formulas hard-coded `if crit { 2.0 }`, so **18 learnable skills were completely inert** — including Death Whisper 1242, Focus Attack 317, Vicious Stance 312, Frenzy 176, Dance of Fire 274, Zealot 420, Dead Eye 414, Chant of Victory 1363. Pulling the thread gathered the family: `CriticalDamagePosition` (3, also on the ranking), `MagicCriticalDamage` (2), `DefenceCriticalDamage` (1) — **24 learnable skills**. `formulas::CritDamage { mul, add }` carries Java's `calcCritDamage`/`calcCritDamageAdd` results, with `Default` = the stat-free `2.0`/`0.0` so the refactor is provably behaviour-preserving for an unbuffed actor (pinned by a test, which is what the pre-existing damage tests rest on). `calc_auto_attack_damage` now follows Java's two-section expression `(((attack·cAtk·ss) + cAtkAdd)·critMod)·77 + (attack·(1−critMod)·ss·77)` — the bracketing is load-bearing: `cAtkAdd` lands *after* the soulshot multiply but *inside* the ×77/÷pDef, so a flat +32 is worth far more than face value. **`StatQualifier`**: last slice's `StatModifierEffect.move_type` field generalised to an enum rather than growing a second parallel `Option` that would rot — `MoveType` merges additively from 0.0 into `_moveTypeStats`, `Position` multiplicatively from 1.0 into `_positionTypeStats`, two maps because Java's merges and identities genuinely differ. The data corrected two wrong assumptions along the way, both now pinned: Focus Death 355 carries **two** position entries with opposite signs (front −30% → ×0.7, back +90% → ×1.9 — the asymmetry only survives because that map multiplies), and skill 193 "Critical Damage" is `mode=DIFF`, a flat +32 `cAtkAdd`, not a percentage. Deferred: `PHYSICAL_SKILL_CRITICAL_DAMAGE` (no learnable grantor on this dist → that branch stays 2.0, the `BLOW_RATE_DEFENCE`/`MP_BLOCK` precedent), `MAGIC_CRITICAL_DAMAGE_ADD` (computed but never applied in Java either), and `calcBlowDamage`'s own crit shape. **SilentMove + FakeDeath landed** (plan: [PLAN_G19_SILENT_MOVE_FAKE_DEATH.md](PLAN_G19_SILENT_MOVE_FAKE_DEATH.md)): the unconsumed-stat sweep came back **clean** this time (all 44 `Stat` variants now have real consumers), so back to the name ranking and its two-way tie at 4 learnable. `SilentMove` won because its four skills (Silent Move 221, Stealth 411, Dance of Shadows 366, Fake Death 60) all *land* but their **headline mechanic** did nothing — the aggro scan carried a literal `// invisibility/silent-move/GM states don't exist` comment, so stealth failed 100% of the time — and it pulled `FakeDeath` in with it: **Fake Death 60 carries only these two effects**, so with both unported it parsed to an empty effect list and was **dropped whole**. Java reads the two flags on *adjacent lines of the same method* (`AttackableAI.isAggressiveTowards`), so splitting them would have meant touching that function twice. New `npc_ai::notices_target` applies the gate at all three player-scan sites (monster, guard PK, siege guard), as a post-sweep `retain` because the sweep closure holds `objects` mutably. **Raid bosses see through stealth** (`!me.isRaid()`) but are **not** exempt from fake death, which goes through `isAlikeDead()` — an asymmetry that's easy to get wrong, now pinned. `FakeDeath` shares the existing DoT tick chain for its MP upkeep (and, being a toggle, inherits the out-of-MP self-deactivate); new `ChangeWaitType` packet (0x29) plus `Revive` on standing up; `break_fake_death_on_damage` hooks the single `apply_physical_damage` choke point (`FakeDeathDamageStand = True`), gated on `amount > 0` so a missed swing doesn't stand you up. Three Java behaviours were checked and found **inert on this dist** rather than assumed: `canSeeThroughSilentMove` (no callers anywhere in the Java tree), `PlayerFakeDeathUpProtection = 0` (the stand-up grace window), and `FakeDeathUntarget = False`. Testing note: the baseline test failed first run and revealed two stealth tests passing **vacuously** — `NpcAi.global_aggro` starts at −10 and creeps 1 per think tick, so a monster needs ~100 game ticks before its scan runs at all (guards are exempt, which is why the older guard tests get away with 20). Deferred: `ChameleonRest`/`Hide` (non-learnable, need sitting), the `RequestRestartPoint`/`RequestActionUse` gates, and `MagicalAttackMp`. **MagicalAttackMp landed** (plan: [PLAN_G19_MAGICAL_ATTACK_MP.md](PLAN_G19_MAGICAL_ATTACK_MP.md)): the MP-drain family — **Mana Burn 1398 and Mana Storm 1399 carry only this effect**, so both parsed to an empty effect list and were dropped whole (the nukes cast, animated and drained nothing); Aura Sink 1102 / Seal of Gloom 1210 pair it with a ported `ManaDamOverTime` so they landed but did none of the up-front damage. Its own formula, sharing nothing with the HP path: `(sqrt(mAtk) * power * (targetMaxMp / 97)) / mDef` — the target's **max MP is a direct multiplier** (the same nuke hurts a mage far more than a fighter), spiritshots scale `mAtk` **before** the square root (so the gain is `sqrt(bonus)`, not `bonus`), and a crit triples then **clamps to a per-skill `criticalLimit`** (1600 on the debuffs, 7000 on the nukes) with no HP-side equivalent; there is also no `damage = 1` floor on a full resist, only the halving. Plus its own landing gate `calcMagicAffected` — a *noisy* mAtk-vs-mDef comparison needing a real `Rnd.nextGaussian()`, ported as `World::roll_gaussian` (Box–Muller over two `roll_f64` draws so tests can still force it through `forced_rolls`). **Correction to the `DamageBlock` slice:** `MP_BLOCK` was documented there as having no callers anywhere in Java — that grep covered `java/` only, and every effect handler lives under `dist/game/data/scripts/handlers/effecthandlers/`, where **five** read `isMpBlocked()` (`MagicalAttackMp`, `Mp`, `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`). The flag is live; `abnormal::is_mp_blocked` now exists and gates this effect, with a `TODO(G19)` for the MP-restore family. *Lesson: grep both trees.* One wrong turn, caught by a failing test and fully backed out: `<magicType>` doesn't exist in this dist's schema — the field is `<isMagic>`, all four skills are magic, and `calcCrit`'s magic branch **discards the `magicCriticalRate` it is passed** in favour of the caster's stat, so the drain's crit is just the existing per-cast `mcrit` and the speculative `Skill.magic_critical_rate` field (which had rippled into 15 test files) was removed. Still ⏳ (the milestone's continuous half): `EFFECT_REGISTRY` breadth toward Java's 369 effect classes, the geometric FAN/SQUARE/RING_RANGE scopes, GROUND casts, AVE runtime + AdminEffects, skill enchanting (`calcMagicSuccess` is now done) |
| Game  | G20 Combat breadth                                          | ✅ **ranged attacks landed** (plan: [PLAN_G20_RANGED.md](PLAN_G20_RANGED.md)): bows/crossbows now need **ammunition** (arrow/bolt matched by crystal grade, auto-equipped to LHand via a dedicated `equip_ammunition` — the ordinary equip path refuses `Etc` items *and* would displace the two-handed bow), spend **MP** per shot, consume one arrow, and arm a **reload delay** (`900000/pAtkSpd`) shown as a red `SetupGauge`; out-of-arrows / not-enough-MP refuse the swing. Bow *range* already worked (pAtkRange 500 via G14). Survey note: `PhysicalAttack` skills and root/immobilize were already done (earlier slices + G19). **Multi-hit melee landed** (plan: [PLAN_G20_MELEE_VARIANTS.md](PLAN_G20_MELEE_VARIANTS.md)): the `Attack` packet now carries several hits (it hard-coded "0 additional"), **dual** weapons strike twice at half damage, and the **polearm sweep** hits extra targets in the weapon's radius (66 vs a sword's 40) and 120° arc — gated on `ATTACK_COUNT_MAX`, a *stat* set by Polearm Mastery 216 (`HitNumber` 5), not on the weapon type. **PvP kill consequences landed** (plan: [PLAN_G20_PVP_KILLS.md](PLAN_G20_PVP_KILLS.md)) — **G20's gate is now met**: killing a player moved nothing before (`player_do_die` had a literal `let _ = killer_oid`). Now Java's three branches — lawful PvP kill → `pvp_kills++`; positive-reputation first offence → reset to 0; otherwise karma (`calculateKarmaGain`, 720 rising to a flat 43200 past 180 PKs) + `pk_kills++` — with the PVP-zone "do nothing" short-circuit. Also found & fixed: the death XP penalty applied unconditionally, where Java skips it inside PVP/siege zones. **Over-hit landed** (plan: [PLAN_G20_OVERHIT.md](PLAN_G20_OVERHIT.md)): a killing blow from an `<overHit>` skill (59 learnable — Triple Slash, Sonic Storm…) banks its excess damage and pays it as bonus XP, capped at 25% of the share, with the "Over-hit!" notice. Note `<overHit>` is an **effect** param, not a skill field — the first read had it at skill level and only the real-datapack parse assertion caught it. **Duels (1v1) landed** (plan: [PLAN_G20_DUELS.md](PLAN_G20_DUELS.md)) — the last feature G20 names: challenge → ask → accept/decline → 5 s countdown → fight → end on death/surrender/timeout/separation, with the `canDuel` gates and the five `ExDuel*` packets. **A duel never kills** — the losing blow is capped at 1 HP and ends the duel, so no death penalty, karma or PvP counters move. Party duels need an arena instance (`TODO(G27)`) and are refused. **Death item drops landed** (plan: [PLAN_G20_DEATH_DROPS.md](PLAN_G20_DEATH_DROPS.md)) — a PK past `MinimumPKRequiredToDrop` killed by a player scatters inventory (the karma penalty, *not* general looting: a clean victim keeps everything), while a **monster** kill uses the gentler `Player*` rates. Adena/quest items never drop; equipped items unequip first and use the equip/weapon percentages; arena deaths and GMs are exempt. **G20 is complete** — `SHOTS_BONUS` is provably dead on this dist (zero items declare `reducedSoulshot`), karma decay is blocked on an absent `KarmaData` table, and party duels need G27's instances |
| Game  | G20.5 Recommendations                                       | ⏳ rec counters + daily reset (`TaskRecom`, `RequestVoteNew`) |
| Game  | G21 NPC AI & world-content breadth                          | ✅ **NPC skill casting landed** (plan: [PLAN_G21_NPC_CASTING.md](PLAN_G21_NPC_CASTING.md)) — the first of G21's four gate clauses. Mobs could only swing before: **4831 NPC templates carry a castable skill and none ever used it** (73% of those attachments are fully covered by ported effects, 9% partially). `AISkillScope` bucketing from the tail of `NpcData.parse` is now built once at load into `NpcAiSkillIndex` — **the `else if` ladder's order is load-bearing** (a *continuous* skill takes the first arm and never reaches ATTACK even when it also carries a damage effect). Needed a real `Skill.is_continuous`: the Rust `OperateType` collapses `A1`/`A2` into `Active`, so continuity is now read from the raw `operateType` (`A2..A6`/`DA2..DA5`) rather than proxied off `abnormal_time`. `<ai type>` is parsed (`AiType`; this dist has 402 **MAGE**, 220 ARCHER, 3163 BALANCED — a mage casts every think, skipping the `hasSkillChance()` roll and the stand-still requirement). `npc_cast.rs` runs Java's ladder — heal → self-buff → immobilize a moving target → mute a casting one → short/long range → general — hooked into `think_attack` *before* the chase/swing tail. The cast rides the **existing shared** launch/finish path, which needed two player assumptions fixed: `mp_consume` would have been **billed twice** (start now charges only `mp_initial_consume`), and `effects.rs` hard-`expect`ed a `Player` on the caster in 5 places, so **any NPC cast panicked the server** — only the one test that ran a cast end-to-end through the real tick loop caught it. Narrowed with `TODO(G21)`s: `skillTargetReconsider` (no faction plumbing → heal/buff target the caster), the ARCHER kite, and the SUICIDE/RES buckets (nothing declares `isSuicideAttack`; no resurrect effect ported). **Guard PK aggro + faction calls landed** (plan: [PLAN_G21_GUARD_AGGRO.md](PLAN_G21_GUARD_AGGRO.md)) — the second gate clause. `<clanList>` was **dropped entirely** by the NPC parser, so every mob fought alone: now 3760 templates carry factions (4569 `<clan>` entries; `ALL` on either side matches everything) and 82 carry `<ignoreNpcId>` lists. **Town guards** (186 `Guard` templates) seed hate on any player with `reputation < 0` inside a **hardcoded 500** — Java's bare literal, *not* the template `aggroRange` — and **regardless of `isAggressive`**; a lawful player is ignored at any distance. **Corrected 2026-07-19:** this slice originally recorded that guards are flagged *passive* in the datapack — they are not (all 186 carry `isAggressive="true" aggroRange="450"`), and because the test fixture hardcoded the same wrong value, nothing caught that the *generic* aggro scan (gated only on `is_aggressive`) was seeding hate on every lawful player within 450 units and **guards were killing them on sight**. Java reaches that scan for guards too, but every candidate must clear `isAggressiveTowards` → `isAutoAttackable`, which for an NPC attacker is true only via `attacker.isMonster()` — a `Guard` is an `Attackable`, not a `Monster`. The generic scan is now `is_monster()`-gated, leaving the reputation rule as the only way a guard aggros a player. **Faction calls** drag idle clan-mates within `clanHelpRange + collision` into the fight, with three separately-tested gates: only if the target **actually attacked this NPC** (Java's `getAttackByList`; proxied by a non-zero aggro `damage` — without it merely being *noticed* pulls the whole camp), only **idle/active** mates answer, and `ignoreNpcId` beats a shared clan. Also had to let `Guard` into the AI at all: `think()` gated on `is_monster()` and `Guard` isn't in that subtree, though Java's `Guard extends Attackable` runs the same `AttackableAI`. **Raid-boss persistence landed** (plan: [PLAN_G21_BOSS_PERSISTENCE.md](PLAN_G21_BOSS_PERSISTENCE.md)) — **G21's gate is now met**. `dbSave` was parsed by nobody, so all **225** raid-boss spawns (`RaidbossSpawns.xml`) were placed like static ones: every restart handed players a fresh full-HP boss and wiped any pending respawn timer. Ported `DBSpawnManager`/`npc_respawns` — a boss now keeps its **live HP/MP** and its **absolute respawn due time** across a restart. **The ownership split matters**: Java's `spawnNpc` hands a `dbSave` spawn to `DBSpawnManager` instead of spawning it (and only if `!isDefined(id)`), so the static pass now defers them into `pending_boss_spawns` and `resolve_boot` settles them when the DB rows arrive — keeping boot asynchronous while preserving "DB wins" (a test pins that the static pass places *no* dbSave boss, or the restore would double-spawn). Three cases: still-on-timer → scheduled not spawned; elapsed/alive → spawned with stored vitals; no row → full + insert. Guards: a dead row's `currentHp = 0` is **not** restored (it would spawn a corpse) and an over-max stored value clamps. Writes on spawn, at corpse decay (banking the absolute due time so a restart mid-window resumes the wait) and on shutdown. SQL verified against the shipped SQLite schema via `PRAGMA table_info` + a round-trip, not just test doubles. Note **any new unprompted `DbEvent` has two boot-event skip-lists to update** (lib + `char_persistence`) — missing them failed 8 tests. **Minions landed** (plan: [PLAN_G21_MINIONS.md](PLAN_G21_MINIONS.md)) — `MinionList`. The parser deliberately *skipped* minion refs (they'd be mistaken for template starts), so all **460** leaders stood alone; a full world spawn now places **3289** escorts from 962 `<minions><npc>` entries. Rules that invert easily, each tested: a **non-raid** leader's minions never respawn, and a `CustomMinionsRespawnTime` of **0 beats the raid default** (4 ids use exactly that); only a **raid** leader's death clears its escort, so killing the big mob in an ordinary camp doesn't evaporate the camp; pack aggro is asymmetric (leader struck = 10, minion = 1, ×10 for a raid). **A real perf bug surfaced only in e2e**: counting a leader's live minions via a full `world.objects` scan per spawn (~3289 × ~39k) made boot so slow the game server missed its login-server registration and the e2e failed at *login* — replaced with the per-master roster Java keeps (`_spawnedMinions`). Two test-only hazards recorded: `add_test_npc`'s `NPC_OID` **is** `FIRST_NPC_OBJECT_ID`, so a runtime-spawned minion overwrote the hand-placed leader; and ambient NPC idle `SocialAction` (0x27) wasn't in `e2e_create`'s skip-list — **the likely cause of that test's long-noted intermittent failures**, now fixed (4/4 consecutive passes). **EffectZones landed** (plan: [PLAN_G21_EFFECT_ZONES.md](PLAN_G21_EFFECT_ZONES.md)) — zones that periodically cast on players inside them (Blazing Swamp fire, Sea of Spores poison, Hot Springs Haste/Focus/Might). **Picked by behaviour, not count**: `ConditionZone` leads the census at 1080 but **1073 are `NoBookmark=true`** — a later-chronicle feature absent from Interlude — so it's ~99% inert, while the 218 `EffectZone`s (204 with skill lists) are live. Their skills were already-ported effects (`DamOverTime`, stat mods). Required **per-zone `type=` parsing**, which the loader had explicitly deferred (it mapped filename→kind and couldn't read the mixed files); a zone whose type isn't ported is now skipped outright rather than mis-filed. **Bonus: that recovered 20 zones missing from the world entirely** (+7 Peace, +7 NoRestart, +6 Pvp in the previously-unloadable mixed files) — total zones 605 → 843. **27 zones declare `targetClass="Npc"` and cast on nobody** (Java tracks only NPCs as inside, then the tick requires `isPlayer()`) — modelled explicitly so they stay inert; I had the default inverted at first and the dist parse test caught it. Runtime differs from Java by design: instead of per-zone tasks needing a live characters-inside set, one 1 s sweep groups players by occupied zone and fires each on its own `reuse` — chance rolled once per creature (not per skill), `initialDelay` honoured, and Java's affected-level guard means a buff zone grants its buff **once** rather than re-casting forever. **NPC regeneration landed** (plan: [PLAN_G21_NPC_REGEN.md](PLAN_G21_NPC_REGEN.md)) — `doRegeneration` ran for **players only**, so every NPC was frozen at whatever HP it was left on: `base_hp_reg`/`base_mp_reg` were parsed and read by nothing, and a raid boss whittled down across sessions never recovered a point. **14855** templates declare an `hpRegen` (only 58 zero; 8.5 is the commonest). Chosen over the remaining zones after checking `default_enabled`: `DamageZone` is 13 live of 35 and `SwampZone` 2 of 20 — the rest are siege-gated castle traps, so 15 zones total vs 14855 templates. **The NPC formula is much shorter than the player one and that's Java, not a narrowing** — levelMod, CON/MEN and the sitting/standing/running multipliers all sit *inside* `isPlayer()`, so an NPC regenerates its raw template value × the raid-or-normal config multiplier (both 100% here; the raid branch is tested by overriding it). **Regen runs during combat** — Java's task never checks an in-combat flag, which is what makes a long fight vs a high-regen boss a DPS race; there's a test named for it so it isn't "fixed" later. The HP-bar broadcast fires only on an actual change, else every full-HP NPC would emit a packet every 3 s. **NPC pathfinding landed** (plan: [PLAN_G21_NPC_PATHFINDING.md](PLAN_G21_NPC_PATHFINDING.md)) — `Creature.moveToLocation` is shared between players and NPCs in Java, but only the player half was ported (G7.85): `move_npc_to` built a straight-line move with **no geodata consultation at all**, so every chase, drift-return and random walk went through walls. The path worker was already built for this — `PathRequest.playable` is documented "one pass for AI" and had never been called with `false`. Now: destination clamp via `get_valid_location` (with Java's >3000 and intentional-fall skips), the **NPC takes the geodata-corrected z** (`if (!isPlayer()) z = destiny.getZ()` — a player keeps its client's z, a mob doesn't), and a clamp shortfall >30 hands off to the worker against the *original* destination. The reply path was player-only and looked up `clients[client_id]`; client-facing sends are now gated on `has_component::<Player>` rather than a sentinel id that could collide with a real client. Two hazards handled: the AI re-issues a chase every 1 s so there's **one outstanding request per mob**, and that guard is only safe because the worker replies to every request and `PathWait` clears **before** the no-route branch returns — otherwise one unroutable target would freeze a mob permanently (tested). Tests run against **real dist geodata**, with blocked/clear lines probed from Giran square first. **`skillTargetReconsider` landed** (plan: [PLAN_G21_TARGET_RECONSIDER.md](PLAN_G21_TARGET_RECONSIDER.md)) — slice 1 shipped NPC casting with heal and buff hard-wired to the caster for lack of faction data; slice 2 added it. **1040 NPCs carry a buff-bucket skill and 305 a heal-bucket one**, so a pack's healer now tops up whoever is worst off and a buffer buffs its mates. Bad skills draw from the aggro list; good skills from faction-mates + self, with heals sorted by lowest HP%; the heal chance now rolls against the *chosen target's* HP. **Deliberate deviation**: Java's good-skill candidate set is every visible creature and its auto-attackable filter sits *inside* the `isContinuous()` branch — a heal isn't continuous, so as written a mob would heal the **player** fighting it; scoped to the caster's faction instead (does less than Java, never more), with a test pinning it. **This surfaced a latent slice-1 bug**: `check_skill_target` encoded Java's `isAutoAttackable(caster)` test as `target_oid != npc_oid`, which was indistinguishable while buffs were self-only and silently blocked *every* faction buff once reconsider landed — a narrowing that is currently indistinguishable from the real rule becomes a bug the moment the thing it was narrowed around arrives. Survey note: **`FenceData` is a single fence named "demo"** and not worth porting on this dist. **`DamageZone` + `SwampZone` landed** (plan: [PLAN_G21_DAMAGE_SWAMP_ZONES.md](PLAN_G21_DAMAGE_SWAMP_ZONES.md)) — the last zone types with live content; both reuse slice 5's parser and sweep. Zone census now **898** (Damage 35, Swamp 20). **No DamageZone in this dist declares `damageHPPerSec`**, so all use Java's field default of **200**/tick — a number that appears nowhere in the datapack, so reading only the XML would suggest they do nothing. `DamageZone`'s default reuse is 5000 ms (not `EffectZone`'s 30000); the parser corrects for it. `SwampZone` multiplies move speed (0.2 here): Java re-reads the zone inside `SpeedFinalizer`, the port caches it on `Speeds` and refreshes on the enter/exit edges, then recomputes + rebroadcasts `UserInfo` like Java's `broadcastUserInfo()`. **Castle traps are gated twice** — only while that castle's siege runs, and players *defending that castle* are skipped; without the second rule a garrison would cook itself on its own defences during the siege it's fighting (both tested). **Walker routes landed** (plan: [PLAN_G21_WALKER_ROUTES.md](PLAN_G21_WALKER_ROUTES.md)) — **G21 is complete**. 13 routes drive Giran's porters, scribes and the running boy, plus Gordon on a 67-node patrol; only `cycle` and `back` styles occur here. Java hangs a `ScheduledFuture` off each arrival; the port keeps `WalkState` on the NPC and drives a 1 s sweep with two phases — travelling (a `Movement` in flight) and waiting (serving the node's `delay`). **Splitting them matters**: banking the delay before the leg starts would let travel time eat the pause. Java's `back` arithmetic steps back **two** on overrun (the index was already past the end), landing on the second-to-last node; the test pins `0→1→2→1→0→1→2` because an off-by-one makes a walker bounce on the spot. **Verification gap closed**: `tests/user_info_packet.rs` had stopped compiling after the previous slice added a `Speeds` field — I'd only been running `--lib`/`char_persistence`/`e2e_create`. Fixed, and this slice was verified with a plain `cargo test -p gameserver` across **all 8 targets (749 tests)**. G21's remaining items are all blocked or empty on this dist: `HtmCache` (caching only), `CreatureSeeTaskManager` (needs a script engine), `FenceData` (one fence named "demo") |
| Game  | G22 Quest & script breadth                                  | 🔨 **Dwarf first-class transfers landed** (plan: [PLAN_G22_DWARF_CLASS_TRANSFER.md](PLAN_G22_DWARF_CLASS_TRANSFER.md)) — G22 depended on G17, and the class-transfer quests are what G17's `setClassId` unblocked. `DwarfBlacksmithChange1` (→ Artisan 56) and `DwarfWarehouseChange1` (→ Scavenger 54) share one implementation, since the two Java scripts differ only in NPC list / target / proof / talk-category; both call the G17 mechanic, so village-master transfers and `//setclass` now share code. **A Java quirk kept deliberately**: the fourth-class refusal hard-codes the *first* NPC's page id regardless of who you're talking to — that looks like a bug, but only the first NPC of each set ships a `-12` page, so "fixing" it would produce a blank window. A dist-page-existence test **failed on its first run** (the pages live under `data/scripts/village_master/`, not `data/html/`), which would have meant a blank window at the exact moment of a class change. **Elf/Human first-class transfers landed** (plan: [PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md](PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md)) — unlike the Dwarf pair these serve **two races from the same NPCs** (Human Fighter 0 / Elven Fighter 18; Mage 10 / Elven Mage 25). **The `from_class` half of each match is load-bearing**: Java matches `(classId == TARGET) && (getClassId() == SOURCE)`, and dropping the source check would let a Human take Elven Knight from the same NPC — there's a test asserting exactly that is refused with nothing consumed. Java's nine near-identical `else if` blocks compress to a `(to, from, proof, first_page)` table because each target owns **four consecutive pages** in a fixed order; the page-existence test then sweeps every target's block across every NPC (9×9 + fixed pages), which is what makes the compression safe. **DarkElfChange1 landed** (plan: [PLAN_G22_DARK_ELF_CLASS_TRANSFER.md](PLAN_G22_DARK_ELF_CLASS_TRANSFER.md)), completing the racial first-occupation set — **and fixing a second class-corruption bug**: `QuestCtx::set_class_id` still had the unconditional `base_class_id = class_id` that G17 slice 6 fixed in `//setclass`, so a *quest-driven* transfer while on a subclass would rewrite the base class. All three paths (GM command, village-master script, quest) now share `subclass::set_class_id`. I'd recorded the "every existing writer becomes suspect" lesson last milestone, fixed one writer, and moved on — finding the second by accident is the cost of not enumerating them. Three ways DarkElfChange1 differs from its siblings, each silent if mis-ported: Java already writes it as a **table** and the event is the **row index** not a class id; the page order is `lowNoProof, low, noProof, done` (opposite pairing to ElfHuman's); and the pages are **`.html`** not `.htm`. Also honours `isSubClassActive()` → refuse, newly expressible after G17. **FirstClassTransferTalk landed** (plan: [PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md](PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md)) — the seven newbie-village headmasters, who (per Java's own header) *only talk about* transfers. Two conventions differ from every other village-master script: pages use an **underscore** (`30026_fighter.html`) and `.html`. **The page availability is asymmetric and IS the logic**: the Human fighter-guild master ships no `mystic` page and the temple master no `fighter`, so a mage at the fighter guild gets `no.html` rather than a constructed filename that would 404 — a test asserts those three absences so the branching can't drift to a "sensible" symmetric version. Also strengthened the main test: it first only checked the reply was non-empty (which would pass while serving the *wrong* page), now it compares byte-for-byte against the dist file through `strip_htm` + `%objectId%`. **The entire first-occupation group is done — 8 of 16 village-master scripts.** **Dwarf second-class transfers landed** (plan: [PLAN_G22_DWARF_SECOND_CLASS.md](PLAN_G22_DWARF_SECOND_CLASS.md)), opening the `*Change2` group: Artisan→Warsmith and Scavenger→Bounty Hunter. **Three differences from `*Change1`**: level **40** not 20; **three** proof items required and all consumed — Java's `hasQuestItems(a, b, c)` is an **AND**, and reading it as "any" would let a player transfer on one mark (tested with two of three); and a **C**-grade coupon reward. Structural quirk: **every** page is hard-coded to the *first* NPC's id whichever of the eight masters you talk to (the `*Change1` scripts did this only for the fourth-class refusal) — the dist ships one 12-page set per script, and the test asserts the other masters ship nothing, so it can't be tidied into per-NPC pages that would 404. **Orc + Dark Elf second-class transfers landed** (plan: [PLAN_G22_ORC_DARKELF_SECOND_CLASS.md](PLAN_G22_ORC_DARKELF_SECOND_CLASS.md)) — they look like siblings and differ in **four** ways, each silent if one is ported by copying the other: the bypass event is the **class id** (Orc) vs the **row index** (Dark Elf); `.htm` vs `.html`; page order `low, lowNoProof, done, noProof` vs **`lowNoProof, low`, noProof, done**; and — the real trap — Orc pays 15 C-grade coupons while **DarkElfChange2 pays nothing at all** (verified by counting: `grep -c giveItems` → Orc 4, Dark Elf 0; copying the Orc branch would have handed out 15 free coupons per transfer). The page owner also isn't the first NPC for Dark Elf — it's **30474, the third**. Process fix: the transfer test failed on first run for the **fourth consecutive slice**, always the same fixture gap, so the quest fixture now registers the whole class range `0..=57` instead of an enumerated list. **Elf/Human second-class transfers landed** (plan: [PLAN_G22_ELF_HUMAN_SECOND_CLASS.md](PLAN_G22_ELF_HUMAN_SECOND_CLASS.md)), closing the `*Change2` group with its three widest scripts — Fighter (10 targets, 477 Java lines), Wizard (5), Cleric (3). **The finding is that they are uniform**: after a slice spent on the four silent ways Orc and Dark Elf differ, I went looking for the same axes here and there are none — same level 40, three-proof `AND`, 15 C-grade coupons, `.htm`, class-id bypass event, and `low, lowNoProof, done, noProof` page order — so the port has **no per-branch code path**, just one `Spec` table. Worth stating, because the previous slice is exactly the prior that would push you to invent per-branch handling that isn't there. What *does* differ is the greeting gate: each script serves a Human and an Elven line from one NPC set through a **different pair of race categories** — `HUMAN_FALL`/`ELF_FALL` (fighter), `HUMAN_MALL`/`ELF_MALL` (mystic), `HUMAN_CALL`/`ELF_CALL` (cleric). Three near-identical names; the wrong one greets the right player with the class-mismatch page. The **`from_class` half of each row is load-bearing and worse here than in Change1**: all ten Fighter targets hang off one NPC, so matching on the target alone would let a Human Knight take **Temple Knight**, an Elven Knight's class, from the same master — tested by handing a Human Knight exactly those marks and asserting nothing happens and nothing is consumed. Two Java behaviours preserved that read as bugs: every page is hard-coded to the *first* NPC's id (the dist ships one page set per script; a test asserts the other masters ship nothing), and `THIRD_CLASS_GROUP` is checked *before* the source-class match. All 5 tests passed on **first run** — the first slice in five to do so, which is the payoff from slice 6 replacing the quest fixture's enumerated class ids with the full `0..=57` range; fixing the pattern rather than the instance held. **AllianceMaster landed** (plan: [PLAN_G22_ALLIANCE_MASTER.md](PLAN_G22_ALLIANCE_MASTER.md)) — 67 Java lines, the smallest of the 16, and **the village-master group is now complete at 16 of 16**. The whole script is one guard: `onTalk` always opens `9001-01.htm`, and `onEvent` echoes the requested page back unless the player has no clan (`9001-04.htm`). **The asymmetry is the script and is easy to "fix" away**: the menu is explicitly excluded from the gate, so a clanless player *does* see both buttons and only learns they can't use them after clicking — gating `onTalk` too, which reads tidier, would change what retail shows; a 6-case test pins both halves. Pages are numbered against a **virtual NPC id** (`9001-NN.htm`, as `ClanMaster` uses `9000`) and no real master ships one, asserted so it can't be "corrected" into a per-NPC name that would 404. **Stated plainly because it would otherwise be rediscovered as a bug: this makes the dialog work, not alliances.** Both buttons post `create_ally`/`dissolve_ally`, `VillageMaster.onBypassFeedback` verbs that are **not routed here** — the alliance system is G18, where `ally_id`/`ally_name` currently exist only as a DB column list and a "when the alliance system lands" comment. I checked the failure mode instead of assuming: unrouted `npc_` verbs hit the router's fallback `warn!` and drop, so the buttons are inert but greppable at runtime, and a `TODO(G18)` names both verbs. This matches how `ClanMaster` already ships with `learn_clan_skills`/`multisell` unrouted, but it is the same shape as the dead-button bugs this port keeps hitting (`Chat <page>`, the race-track gatekeeper), so it is recorded as a known gap. Added `QuestCtx::has_clan` alongside `is_clan_leader`. **The Elven first-occupation quests landed** (plan: [PLAN_G22_ELVEN_PATH_QUESTS.md](PLAN_G22_ELVEN_PATH_QUESTS.md)), opening G22's quest body — and the slice was **chosen by a gap the previous eight created**: all 16 village-master scripts were done, and every one consumes a proof item **no quest in the port produced**, so the transfers were reachable only via `//setclass`. `Q00406_PathOfTheElvenKnight` and `Q00407_PathOfTheElvenScout` award the Elven Knight Brooch (1204) and Reisa's Recommendation (1217), making the elven half of `ElfHumanFighterChange1` reachable in normal play. **The finding: Q00406 deliberately ignores `RateQuestDrop`.** It hand-rolls `getRandom(100) < chance` + plain `giveItems` instead of calling `giveItemRandomly`, which multiplies *both chance and amount* by the rate — so reaching for the port's faithful `give_item_randomly` helper would have silently scaled a drop Java leaves alone. Caught only by diffing against `Q00303_CollectArrowheads`, which *does* call the helper; the two look identical in shape and differ in exactly this. Test pins it with `RateQuestDrop = 3.0` → still one topaz per kill. **Generalised: check whether the Java quest calls the helper or rolls its own before picking the Rust primitive — they are not interchangeable.** Q00407's tag mechanic needs **both** hooks: `onAttack` stamps the mob's script value with the attacker's object id and `onKill` pays only on a match — porting one alone fails silently in opposite directions (kill-only never matches; attack-only leaks the tag). Tested both ways. Page conventions: extensions are **mixed inside one quest** (`.htm` pre-accept, `.html` after), and Prias ships `-01`/`-02`/`-04` but **no `-03`**, which Java never names — asserted so the gap isn't helpfully filled in, the same shape as `FirstClassTransferTalk`. Also collapsed a Java three-way level branch awarding identical exp/sp in all three arms (commented, so it doesn't read as a dropped case). The chain test failed once on first run: I asserted the quest record was gone after `exitQuest(false, …)`, but a one-time exit keeps it **COMPLETED** — deleting it would let the quest repeat. Assertion corrected, not the code. Added `QuestCtx::social_action`. 14 quests ported. **Path of the Warrior + Path of the Rogue landed** (plan: [PLAN_G22_PATH_WARRIOR_ROGUE.md](PLAN_G22_PATH_WARRIOR_ROGUE.md)), awarding the Medallion of Warrior (1145) and Beziques' Recommendation (1190) — `ElfHumanFighterChange1` now has four of its five proofs. **The finding: the same `ItemChanceHolder` type, two different denominators.** Q00406 rolls `getRandom(100) < chance`; Q00403 rolls `getRandom(REQUIRED_ITEM_COUNT)` — i.e. **`getRandom(10)`** — so a "chance" of 2 means 2% there and **20%** here. Reading Q00403's table as percentages (the obvious assumption, same type used that way one quest earlier) would have made every Spartoi bone **10× too rare**, turning a ~13-kill stage into ~125. **The denominator is a property of the call, not of the table** — the same shape as the previous slice's `giveItemRandomly` finding: a quest's drop maths is not inferable from the types it uses, so read the roll. Q00401's spider stage meanwhile has **no chance roll at all** — the weapon gate, not a rate, is what makes it slow. Quests 401/403 share a byte-identical `onAttack` state machine — the "kill it solo with the quest weapon" tag (0 → 1 on the right weapon, → 2 terminal on a weapon change *or* a second attacker; `onKill` pays only on 1) — now factored into `scripts/quest_common.rs` since 402/415 use it too. Both hooks load-bearing in opposite directions, as in 407. Two framework pieces: **`Npc.vars`** (Java's `getVariables()`, needed for `lastAttacker`) — shape chosen after checking breadth, 11 quests use it under 6 keys, so a generic map beats six `spoiler_object_id`-style named fields, and an empty `HashMap` doesn't allocate; and **`QuestCtx::npc_say_to_player`**, because the Cat's Eye Bandit taunts its attacker with `sendPacket` but broadcasts its death line — using the existing broadcasting `npc_say` would have leaked the taunt to bystanders. 5 tests, first-run green. Q00401's `/10` roll is pinned deterministically (force the roll to 4 → no drop; `getRandom(100) < 40` would drop), but **Q00403's is deliberately statistical**: a forced roll ignores the bound, so no forced test can tell `/10` from `/100` — it asserts the rate instead (chance 8 = 80% caps 10 bones within 40 kills; 8% essentially never), re-run 10× to confirm it isn't flaky. 16 quests ported. **Path of the Human Knight landed** (plan: [PLAN_G22_PATH_HUMAN_KNIGHT.md](PLAN_G22_PATH_HUMAN_KNIGHT.md)) — 629 Java lines, the widest of the Path family, taken alone because it **completes the proof set for `ElfHumanFighterChange1`**: all five targets are now reachable in normal play, closing (on the fighter side) the gap opened three slices ago. Structurally unlike its siblings: **six independent sub-quests of which you need three** — six officers each trade a badge for N trophies for a Coin of Lords — so most of those 629 lines is one block six times, ported as a `BRANCHES` + `DROPS` table. **The completion path forks on the coin count and the 6-coin case is the odd one:** 3 coins and 4–5 coins each open a prompt whose confirm button (`30417-13`/`-14`) does the awarding, but **6 coins completes immediately inside `onTalk`** with no confirmation. It reads like an oversight; the dist backs it up (`-12` is a completion page, not a prompt), so it's kept and tested both ways — tidying the asymmetry would either add a prompt nobody can answer or silently drop the 6-coin completion, and the player who did all six sub-quests is exactly the one who'd hit it. The confirm handlers also sweep **all** leftover badges/trophies (a player may have part-finished other sub-quests); the 6-coin path takes only coins and the mark, correct there since every badge was already spent. Quirks verified rather than assumed: the quest **never calls `setCond`** — not once in 629 lines, so the quest window shows one step throughout (confirmed by grep, not inferred from the sections I read); Vasper's extensions **alternate** (`-01..05`/`-07`/`-08` are `.htm`, `-06` and `-09..15` are `.html`) rather than splitting on a prefix, so the test asserts `30417-07.html` and `30417-06.htm` are *absent* to stop it being regularised; and Raymond alone ships six pages (an extra intermediate page shifts his later ones up by one), encoded per branch with a test that no other officer has a `-06`. **Two of the six trophies have no chance roll at all** — easy to miss across six near-identical blocks, so the table stores `Option<i32>` and ten unforced kills are asserted to yield exactly ten necklaces. 6 tests, first-run green. 17 quests ported. **Path of the Human Wizard + Path of the Cleric landed** (plan: [PLAN_G22_PATH_WIZARD_CLERIC.md](PLAN_G22_PATH_WIZARD_CLERIC.md)) — the Bead of Season (1292) and Mark of Faith (1201), so `ElfHumanWizardChange1` now has **2 of its 4** proofs. **Q00404 is four identical elemental branches with one exception.** Fire → Wind → Water → Earth each run the same token → collect → trinket bargain, repeating right down to the page numbering (`{npc}-01..04.html` for all four), so it ports as an `ELEMENTS` table. **The exception is Wind: its collectable is not a drop** — the feather comes from a dialog bypass on the Wasteland Lizardman, who sits outside the four-page scheme. A table-driven port assuming "collect ⇒ kill" would leave that branch permanently stuck; tested specifically. **Chance denominator is `/100` here** (`getRandom(100) < 20|80`) where 401/403 use `getRandom(10)` — the **third** distinct denominator convention in the Path family, checked per call site rather than carried over. **A test I deliberately did not write, and why:** no honest deterministic test for that denominator exists — `forced_rolls` ignores the bound, so `forced < chance` is literally the same predicate under either reading. Q00403's statistical trick doesn't transfer either: there the misreading made drops *rarer* (8% vs 80%), which 40 kills detect; here it would make them *more common*, and with a cap of 1 you get one Bernoulli per quest instance, so detection needs many worlds for little value. Pinned by a call-site comment instead. Better no test than one that appears to prove something it can't. **Q00405 has two things that break if normalised:** Simplon hands over a **stack of three** books where the other two givers give one each (and completion takes `-1`/all of his but `1` of theirs — treating them uniformly strands two or makes the check unsatisfiable; tested); and the cond-2 checks contain a **no-op `>= 0` term** — each giver re-checks all three counts but writes its own slot as `>= 0`, a placeholder for "the one I just handed over", so all three sites reduce to one predicate. Read literally it looks like a bug; it's only redundant, and collapsing is safe because the giver's own count is non-zero at that point. Praga's pendant drops with no roll at all. 5 tests, first-run green. 19 quests ported. **Path of the Elven Oracle landed** (plan: [PLAN_G22_PATH_ELVEN_ORACLE.md](PLAN_G22_PATH_ELVEN_ORACLE.md)) — the Leaf of Oracle (1235), `ElfHumanWizardChange1`'s **3rd of 4** proofs. **Taken alone rather than paired with 408 as planned**: I checked both quests' framework needs first — 408 uses none of `addSpawn`/`addAttackPlayerDesire`/`setMemoState`, 409 uses all three (23 call sites) — and carrying three new primitives plus a second 446-line quest is how sloppiness gets in. **The first quest in the port that spawns its own monsters:** Allana's re-enactment and Perrin's Tamil are ambushes conjured beside the NPC you're talking to and set on you. New framework: `QuestCtx::memo_state`/`set_memo_state` (Java stores it as the quest var `memoState` — confirmed in `QuestState.MEMO_VAR`, not guessed), `QuestCtx::spawn_attacker` (`addSpawn` + `addAttackPlayerDesire`, reproducing `Rnd.get(50,100)` per axis with independent sign), and `npc_ai::seed_attack` promoted to `pub(crate)`. **`memoState` is a second progress axis, not `cond`:** `cond` drives the client window, `memoState` is script bookkeeping, and they move in *opposite* directions — talking to Manuel empty-handed at `memoState == 2` rewinds it to 1 while pushing `cond` to 8. Collapsing them breaks the re-enactment restart path. The ambush tag is also **not** `quest_common`'s: it gates on one attacker with **no weapon check** and keys `firstAttacker`, so routing it through the shared helper would have silently added a weapon requirement — the test kills bare-handed to pin that. **The bug that cost the time was in the test fixture.** The memo test failed with a no-quest reply; instrumenting showed the talk arriving at **npc 27032, a lizardman**, instead of Priest Manuel — because `NPC_OID` and `world.next_npc_object_id` **both start at `FIRST_NPC_OBJECT_ID`**, so the first runtime spawn lands on a fixture NPC's object id and silently replaces it. No test had ever spawned at runtime before. Fixed in the shared `add_test_npc` (it now reserves each id against the allocator) rather than by shuffling my own ids — every future spawning quest would have hit it. All seven major modules re-run green after (quests 76, combat 33, npc 71, guard_aggro 13, admin 89, items 37, clans 16). 4 tests. 20 quests ported. **Path of the Elven Wizard landed** (plan: [PLAN_G22_PATH_ELVEN_WIZARD.md](PLAN_G22_PATH_ELVEN_WIZARD.md)) — the Eternity Diamond (1230), the last of `ElfHumanWizardChange1`'s four proofs. **The whole Elf/Human first-occupation tier is now self-sufficient**: both Change1 scripts (5 proofs + 4) are satisfiable entirely in normal play, which took nine quests. Three parallel errands, all required in any order, each the same four beats (introduction → charm → gated drop → gem), so it ports as one table. **The third errand is missing a step, and the dist proves it isn't a bug:** errands 1 and 2 swap introduction→charm in a **dialog event**, errand 3 does it inline in `onTalk`. Exactly the asymmetry one would "regularise" — until you count pages: Greenis and Thalia ship four each, **Northwind ships three**. There is no fourth page for an event to return, so adding one would 404 the moment a player takes that errand. Kept as `swap_event: Option<&str>` and the page test asserts `30423-04.html` does *not* exist. Same shape as `FirstClassTransferTalk`'s asymmetric pages and Q00407's missing `30426-03` — **when a script looks inconsistent, check whether the dist's page set explains it before normalising.** Like 402, `setCond` appears **zero** times in 446 lines (grepped, not inferred) — progress lives entirely in which items you hold. Denominator `/100`, as in 404/406. 3 tests, first-run green. 21 quests ported. **Path of the Palus Knight + Path of the Assassin landed** (plan: [PLAN_G22_PATH_DARKELF_1.md](PLAN_G22_PATH_DARKELF_1.md)), opening the **Dark Elf** tier — the Gaze of Abyss (1244) and Iron Heart (1252), so `DarkElfChange1` has **2 of 4** proofs. **Every drop in both is unrolled** — no `getRandom` in either `onKill`, so 13 kills is 13 skulls and 10 is 10 molars. Stated because 412/413 *do* roll and porting by analogy would add a chance that isn't here; the tests use no forced rolls at all, which is only a valid way to assert exact counts *because* the drops are unrolled. **Q00411 is one token walking a chain.** Java writes every branch as "hold this and **none** of the others" — seventeen times across three NPCs — which encodes one fact: exactly one token is in the bag at a time, since each hand-over takes before it gives. The port asks *which* token is held and matches once. The invariant is the quest's own design (checked transition by transition), not an assumption; the molars are the deliberate exception (they coexist with Leikan's note), pinned by a test that his page tracks the molar count while the token stays put. Two redundant Java terms collapsed with the reasoning recorded: a `silk >= 4` re-test inside `== 5`, and a genuinely **dead** Kalinta branch (`!has(SILK) && has(CARAPACE)` sits under `!(both)`, which already catches it) — the reachable state→page table is documented at the site so the equivalence stays checkable. **The page test earned its keep, failing on first run:** I'd asserted the `.htm`/`.html` split identically for both quests, but **410's accept page `30329-06` is `.htm` while 411's `30416-06` is `.html`** — the split point differs per quest even inside one race tier. Now asserted separately, with an explicit check that `30416-06.htm` does *not* exist. 5 tests. 23 quests ported. **Path of the Dark Wizard + Path of the Shillien Oracle landed** (plan: [PLAN_G22_PATH_DARKELF_2.md](PLAN_G22_PATH_DARKELF_2.md)) — the Jewel of Darkness (1261) and Orb of Abyss (1270). **The Dark Elf first-occupation tier is COMPLETE**: `DarkElfChange1` has all four proofs. Two races done, two to go. **Q00412 repeats quest 408's third-errand asymmetry — and twice makes it a convention.** Charkeren and Annika hand their tool over via a **dialog event**; Arkenia does it **inline in `onTalk`**, exactly as Northwind does in 408 where Greenis/Thalia use events. One occurrence looked like an oversight worth documenting; two independent quests in different race tiers makes it a datapack convention, so it's modelled (`tool_event: Option<&str>`) without further hedging, both branches exercised in one test loop. Arkenia also omits the `SEEDS_OF_DESPAIR` guard her siblings carry — kept, since adding it for symmetry would change who can start her errand. **Q00412's chance is an equality, not a threshold:** `getRandom(2) == 0` where every other Path quest uses `<`. Same 50% here, but not interchangeable — read as `getRandom(2) < 2` every kill pays. Unlike the `/10` vs `/100` cases this one **is** deterministically testable (a forced roll of 1 separates the readings), so there's a test. That's **four** distinct chance conventions in this family now: `/100`, `/10`, `== 0`, and no roll at all. **Q00413's succubus kill is a swap, not a drop** — it *consumes* a Blank Sheet to make a Bloody Rune, so the counts move in opposite directions and the cond tests **both** (sheets exhausted AND five runes). Modelling it as a capped drop would strand five sheets and never fire the cond; tested per-kill in both directions plus a sixth succubus proving no sheet means no rune. Talbot hands over **five** sheets in one `giveItems(..., 5)`, the same stack shape as Simplon in 405; and neither of 413's drops rolls while 412 rolls all three — conventions differ quest by quest even inside one tier. 4 tests, first-run green. 25 quests ported. **Path of the Orc Raider landed** (plan: [PLAN_G22_PATH_ORC_RAIDER.md](PLAN_G22_PATH_ORC_RAIDER.md)), opening the Orc tier with the Mark of Raider (1592). **Scoped down mid-slice** — planned as 414+416, but 414 carried two things worth doing carefully, so 416 follows rather than being rushed to hit the announced pairing. **Green blood is a rising summon meter, not a collection.** Java races the *held count* against the RNG: `blood <= getRandom(20)` gains one, otherwise it **wipes the stack and summons Kuruka onto the player**. At 0 blood the gain is certain, at 19 it's 5%, at 20 the summon is guaranteed. The blood is never handed in — and the tooth the quest wants drops from **Kuruka**, not the goblins, so porting the blood as an ordinary capped collection would make the quest **unfinishable**. Two tests pin the fork and the tooth source. Reuses `spawn_attacker` from slice 13; fidelity gap recorded (Java's `isSummonSpawn` animation + `addDamageHate` 999 vs our dominant-hate seed). **A branch dead at both ends — and the order I checked mattered.** Karukia's `07b` route sets `memoState=2`/`cond=5` and leads to events on NPC **31978**, who ships five pages here but is **registered nowhere** (`grep -rln 31978 data/scripts/` finds only this quest's file and its own orphaned pages). Separately, `30570-07.htm` offers **only** the `07a` button. Had *only* the serving end been missing, `07b` would be a trap — it consumes the map and all ten teeth but hands out no reports, the sole path to the reward, stranding the player permanently. Because the button doesn't exist either, there's no trap and the route ports verbatim at zero risk. Kept with a `TODO(dead)` and a test asserting **both** halves so nobody restores one end without the other. 5 tests, first-run green. 26 quests ported. **Path of the Orc Monk landed** (plan: [PLAN_G22_PATH_ORC_MONK.md](PLAN_G22_PATH_ORC_MONK.md)) — 652 Java lines, **the widest quest in the Path family**, awarding the Khavatari Totem (1615). **The weapon gate is the INVERSE of quests 401/403.** Those demand a specific quest weapon; this one demands `weapon == null || FIST || DUALFIST` — an Orc Monk fights unarmed, so **"no weapon" is the pass case**. Routing it through the shared `quest_common` tag would have flipped the entire quest: every bare-handed kill paying nothing and every sword kill paying. Needs the weapon's **type**, not id, so `QuestCtx::is_bare_or_fist_handed` was added; tested bare / sword / fist. Its tag variable is `Q00415_last_attacker` — a **third** name after `lastAttacker` (401/403) and `firstAttacker` (409). **The pouch stages take five kills, not four:** Java gives a trophy per kill and converts when the count is *already* 4, so the fifth kill fills the pouch. Reading it as "collect 4" leaves the pouch permanently unfillable — the conversion branch is never entered. The fourth pouch is the same shape over four mobs at three each, converting on the twelfth kill. Both tested per-kill. **Half the quest is unreachable — the same two-sided orphaning as 414.** `09c` opens an entire alternate ending through NPCs 31979/32056, with its own stages, a raid mob and its own reward hand-out — but `30587-09a.html` offers only the `09b` button and neither NPC is registered anywhere, leaving **13 orphaned pages**. Checked both directions again: had only the serving end been missing, `09c` would strand the player (it takes Rosheek's letter and gives no recommendation). Ported verbatim with `TODO(dead)` on the events, both dead kill handlers and the `memoState == 2` talk branch. **Two of two Orc quests now carry a fully orphaned alternate route — expect it in 416.** 5 tests, first-run green. 27 quests ported. **Path of the Orc Shaman landed** (plan: [PLAN_G22_PATH_ORC_SHAMAN.md](PLAN_G22_PATH_ORC_SHAMAN.md)) — the Mask of Medium (1631). **The Orc tier is COMPLETE**; three of four races done. Ported off groundwork from an aborted previous attempt, where I stopped rather than rush a 525-line quest needing unchecked framework — and two of the three gaps that analysis flagged turned out not to exist. **`ItemChanceHolder.count` is a cond SELECTOR here, not a quantity:** `if (item.getCount() == qs.getCond())`, with `chance` as a 0..1 probability for `giveItemRandomly`. Read `count` normally — as quests 403/406 use it — and grizzly bears drop **six** bloods a kill while the cond gate silently vanishes. Tested both sides (nothing at cond 1, exactly one at cond 6). **Fourth** distinct reading of this type in the family after `/100`, `/10` and `== 0`. **Two summon meters differing in the one way that matters:** the Durka parasites escalate exactly like 414's green blood (5 → 1-in-10, 6–7 → 2-in-10, 8 certain, success wipes the stack and conjures a spirit) — but **Java does not set this one on the player**, where 414 does. Needed `QuestCtx::spawn_near_npc` (with `spawn_attacker` refactored onto it); reusing `spawn_attacker` was the natural move and would have invented aggro the datapack never asks for. The test asserts the spirit is *not* in the aggro list. **What the groundwork got wrong, usefully:** `NpcSay` string parameters aren't needed (both such lines live inside the dead branch, so the live path never reaches them) and `getRandomPartyMemberState` reduces to the killer exactly as `q00303` already documents — a `TODO(G13+)` deviation, not new machinery. The `memoState` 100–110 branch is again **dead at both ends** (third Orc quest running: sole entry `30585-14.html` is offered by nothing, and 31979/32057/32090 are registered nowhere) — here **omitted rather than stubbed**, since half-porting it would carry dead memoState handling and a packet feature we lack. Also: the accept event is **`START`**, not `ACCEPT`; and `cond 10` is never assigned (9 → 11). 6 tests, first-run green. 28 quests ported. **Path of the Artisan landed** (plan: [PLAN_G22_PATH_ARTISAN.md](PLAN_G22_PATH_ARTISAN.md)) — the Final Pass Certificate (1635), opening the Dwarf tier. **The leader-tooth roll has a hole in it:** below 5 the kill pays *only* if one tooth is already held, so the first drops at 50% and the second at 100% — a flat "50% per tooth" reading is wrong in both directions (three forced-roll cases pin it). Consequence kept, not fixed: the `else` branch pays the second tooth **without** the `cond 2` check the other branch performs, so finishing that way leaves the quest window stale. Every downstream branch tests item counts rather than the cond, so the quest still completes — a cosmetic Java bug, ported verbatim. Also two routes to Kluto's letter differing only in whether `setCond(4)` chimes. **Dead at both ends for the fourth quest running** (`30527-08c` + NPCs 31956/31963/32052); omitted rather than stubbed, as in 416. **The dead-branch test caught my own error rather than the port's**: the first version scanned every file in the quest directory including the `.java` source, which of course names `08c` as a case label — the very handler being proven unreachable — so it fired on the evidence. Restricted to `.htm`/`.html`. 4 tests. 29 quests ported. **Path of the Scavenger landed** (plan: [PLAN_G22_PATH_SCAVENGER.md](PLAN_G22_PATH_SCAVENGER.md)) — 690 Java lines, the largest in the family. **ALL EIGHTEEN `Path of the *` quests (401–418) are now ported**, so every race's first-occupation script is proof-complete and reachable in normal play. **`dropChance` is documented as a 0..1 fraction and this quest passes `50`** — not 50%, but fifty times certainty, so **every qualifying kill drops** (`q00303` passes `0.4` for a real 40%, so the convention isn't in doubt). A datapack bug with a live effect; the dist is authoritative, so the port passes `50.0` and matches the shipped server. Writing the "obviously intended" `0.5` would halve the rate against retail — a silent divergence in the direction that looks like a fix. The test kills six tarantulas unforced and asserts six beads (at a real 0.5 it'd fail ~98% of the time). **Spoil-gated payouts** — the Scavenger's own mechanic: jars and beads pay only off a corpse that `isSpoiled()`, and `onAttack` separately disqualifies a mob whose spoiler *is* the attacker. Added `npc_is_spoiled`/`npc_spoiler_object_id`. Its npc var is `FIRST_ATTACKER`, a **fourth** spelling. **Two counters packed into one integer:** `memoStateEx(1)` is radix-packed — +10 per delivery (tens), +1 per Mion dialogue step (units), read back via `% 10` and `< 20`/`< 50`. Treating it as one counter breaks both halves; added `memo_state_ex`/`set_memo_state_ex` (a second memo axis). `FLAG` is a **third** summon-meter shape (`20 * flag` percent, reset on success) after 414's and 416's. `npc.deleteMe()` needed `delete_npc`. Dead at both ends for the **fifth** quest running (NPC 31958). 5 tests, first-run green. **30 quests ported; the Path family is complete.** Next: ~161 more quests, ~81 `ai/` scripts, daily quests, the tutorial, `//reload` |
| Game  | G23 Grand bosses & raid bosses                              | ⏳ boss zones/respawn/AI/persistence — `//grandboss` |
| Game  | G24 Castles, sieges, clan halls & territory war             | ⏳ AdminFortSiege/`//castle`/`//clanhall`/territory war |
| Game  | G24.5 Boats                                                 | ⏳ `BoatManager` + 4 ferry routes (`AllowBoat = True`) |
| Game  | G25 Olympiad & hero                                         | ⏳ AdminOlympiad/`//sethero`/`//saveolymp`/`//endolympiad` |
| Game  | G26 Seven Signs, Manor & Mammon                             | ⏳ `//manor`/`//mammon_*` |
| Game  | G26.5 Lottery & Monster Race                                | ⏳ `games/` managers (Lottery, Race Track betting) |
| Game  | G27 Instances                                              | ⏳ AdminInstance/AdminInstanceZone |
| Game  | G28 Events engine & cursed weapons                          | ⏳ AdminEvents/`//tvt_*`/AdminCursedWeapons |
| Game  | G29 Summons, pets, servitors, cubics, agathions             | ⏳ editchar summon/pet subcommands |
| Game  | G30 Mail, community board & party matching                  | 🚧 **community board: home + buffer + gatekeeper + premium + scheme buffer landed** (`ShowBoard` window + chunked `sendCBHtml`; `RequestShowBoard`/`_bbs*` bypass routing; custom `HomeBoard` render with navigation; `_bbsheal`/`_bbsteleport`/`_bbsbuff` actions + karma/combat gates; `_bbspremium` account-premium buy; `_bbs_buff_scheme_create`/`_delete`/`_execute` backed by the `buffer_schemes` table + `SchemeBufferSkills.xml` levels; `FavoriteBoard` `_bbsgetfav`/`bbs_add_fav`/`_bbsdelfav_` backed by the `bbs_favorites` table + `HomepageBoard` `_bbslink` + `DropSearchBoard` `_bbs_search_item`/`_bbs_search_drop`/`_bbs_npc_trace` — drop index, server-rate drop list, item-icon side-map, new `RadarControl` 0xF1 packet; **merchant multisell** `MultisellData` + `MultiSellList` 0xD0 + `MultiSellChoose` 0xB0 exchange behind `_bbsmultisell`/`_bbsexcmultisell`). Mail, party matching, `_bbssell` (needs buylist 423, absent) and `_bbsdelevel` (config-off) board actions and the retail forum boards still ⏳ (`TODO(G30)`). AdminBBS pending |
| Game  | G30.5 Item auction                                          | ⏳ `ItemAuctionManager` + bid packets |
| Game  | G31 Moderation, accounts, petitions & HWID                  | ⏳ AdminPunishment/AdminLogin/AdminHwid/AdminPetition |
| Game  | G32 Fishing                                                 | ⏳ |
| Game  | G33 Misc parity & finishing sweep                           | ⏳ game-clock/autosave/geosave/fightcalc/repairchar + parity checklist |
| Game  | (out of scope) Gracia/Hellbound/elemental, sayune/shuttle/airship, `tools/`, MariaDB/Postgres, Swing UI, Mobius `Custom/*` | ⛔ non-Interlude / per PLAN §11 + ROADMAP scope gate |

**Verified end-to-end:** a scripted client does the real login crypto → server
select → game `AuthLogin` → char list → **create** (with initial skills) →
reconnect → **CharacterSelect → CharSelected → EnterWorld → UserInfo + full
enter-world burst** with correct computed HP/MP, then manor / key-mapping /
skill-cooltime requests. See `crates/gameserver/tests/e2e_create.rs`.

---

## Login server (M0–M5) — ✅

Drop-in replacement for the Java login server; the unmodified Java game server
registers and interoperates. Crates: `commons` (framing, L2 crypto, config,
SQLite), `loginserver`. All crypto golden-vector tested. Parity checklist:
[LOGIN_SERVER_PARITY.md](LOGIN_SERVER_PARITY.md).

Post-M5 fixes:
- **Account case-insensitivity** (`4f29af4`): the login server now lowercases
  accounts everywhere (Java `AccountInfo._login = login.toLowerCase()`), so the
  game's lowercase `PlayerAuthRequest` matches `authed_clients`. Without it,
  mixed-case logins reached the server list but never the lobby.

---

## Game server

### G0 — Scaffold & boot ✅ (`5a8f681`)
`gameserver` crate; `Config` reads `dist/game/config/*.ini` verbatim; runs with
`dist/game` as cwd (auto-chdir); SQLite pool on the real DB; 100 ms game-thread
tick loop with id-capturing scheduler + tick-overrun metric; ctrl-c graceful
shutdown.

### G1 — Client link & cipher parity ✅ (`80d4c4d`)
Game XOR `Encryption` cipher (golden-vector verified byte-for-byte);
tokio per-connection tasks (`commons` framing); `GameClient` + `ConnectionState`;
`ProtocolVersion → KeyPacket` handshake with cipher enablement; decrypted packets
forwarded to the game thread over `NetEvent`.

### G2 — Login-link + auth ✅ (`3896fc1`)
`LoginServerThread` port (`loginlink/`): GS-link handshake (InitLS → BlowFishKey
RSA → AuthRequest → AuthResponse), relays commands/packets. Shared GS-link crypto
lifted into `commons`. Session type-state (`session.rs`, plan §3.1):
`Connecting → Authenticated`. `AuthLogin` handled on the game thread. Loads
`hexid.txt`. Real network config via **`IPConfigData` port** (`7366365`) —
`ipconfig.xml` + subnet auto-detection, so the login ServerList hands each client
the right game address.

### G3 — Character selection & persistence ✅ (`d596924`, `5fb30b1`, `98a988b`, `44fb451`)
- **DB thread** (`db.rs`): dedicated OS thread owns the SQLite pool; game thread
  sends `DbCommand`s, drains `DbEvent`s. Minimal `IdManager`.
- **Data loaders**: `ExperienceData`, `PlayerTemplateData`.
- `CharSelectionInfo` (real rows), `NewCharacter`/`CharacterCreate` (validate +
  insert with base stats/spawn), `CharacterDelete`/`Restore` (deletion timer).
  Session `InLobby`.
- **Create fixes**: match Java (no re-send of `CharSelectionInfo` after
  `CharCreateOk` — `send_list` flag); Unicode name validation;
  `RequestCharacterNameCreatable` → `ExIsCharNameCreatable`.
- **Initial skills**: `SkillTreeData` reads the class-tier + common trees; new
  characters take their starting class's level-1 auto-get skills →
  `character_skills` (Mystic 5, Orc Fighter 1, …).

### G4 — Enter world ✅ core (`82c86a0`, `0121575`, `ee682cc`, `0761efe`, `a6aea48`)
- **Player model** (`model/`): composed struct built from a stored character +
  template. **Proper max HP/MP/CP = base level-table value × CON/MEN stat bonus**
  (`MaxHp/Mp/CpFinalizer`), via new `StatBonus` (`statBonus.xml`) and per-level
  HP/MP/CP tables. Verified vs. L2 (Human Fighter L1 = 126, Mystic = 98/59).
- **Packets**: `CharSelected`; full masked **`UserInfo`** (23 blocks, mask
  `[0xFF,0xFF,0xFE]`) — byte-verified against a real client capture in a unit
  test (`a6aea48`).
- **Flow**: `CharacterSelect` → `Entering` (sends `CharSelected`); `EnterWorld`
  → moves Player into `World.players`, sends the **full enter-world packet
  burst** (`enter_world.rs`) → `InGame`. `ActionData` loader (242 ids) for
  `ExBasicActionList`.
- **In-game requests handled**: `RequestManorList`→`ExSendManorList`,
  `RequestKeyMapping`→`ExUISetting`, `RequestSkillCoolTime`→`SkillCoolTime`,
  `RequestUserBanInfo` (consumed, no reply — matches Mobius null handler).

### ✅ Paperdoll & inventory bitmasks (part of G4, items landed in G5)
Replaced hardcoded paperdoll/mask values with Java-faithful enums/bitmasks:
- **`model/inventory.rs`**: `PaperdollSlot` (32 `Inventory.PAPERDOLL_*` ids) +
  `Inventory` with paperdoll getters (`object_id`/`item_id`/`visual_id`/
  `augmentation`, zero-for-empty like Java); `Player.inventory` field. Items
  themselves landed in G5.
- **`network/masks.rs`**: `AbstractMaskPacket` port — reversed
  `DEFAULT_FLAG_ARRAY = [0x80,0x40,…,0x01]` (mask 0 → 0x80), `add_mask` /
  `contains_mask` / `build_mask`, unit-tested against the known-good UserInfo
  mask bytes.
- **`enums.rs`**: `InventorySlot` (33 wire-order components incl. `LRHand`,
  mask = ordinal, `slot()` → `PaperdollSlot`) and `UserInfoType` (23 blocks,
  mask = ordinal + `block_length()`).
- **Packets driven through the enums**: `UserInfo` (mask bytes, block count,
  `init_size`, per-block lengths all derived from `UserInfoType`; byte test
  unchanged), `ExUserInfoEquipSlot` (mask built from `InventorySlot::VALUES`,
  paperdoll values read via `Player.inventory`), `CharSelectionInfo`
  (`ServerPacket.PAPERDOLL_ORDER` + its own visual/enchant slot orders).
- **Bug fixed**: `ex_user_info_equip_slot` mask byte 5 was `0x01`; slot 32 in
  reversed flag order is `0x80` — now produced by `build_mask`.

### G5 — Items & inventory ✅ vertical slice
Full itemcontainer parity (warehouse/trade/pickup/enchant/crystallization/
augmentation) is deferred; this milestone gets items flowing end-to-end the
same way G0–G4 got a vertical slice through "enter world":
- **`data/item_data.rs`**: generic StatSet-style parse of all 441
  `dist/game/data/stats/items/*.xml` files → `ItemTemplate` (id, name,
  kind, body part, weight, stackable, `type1`/`type2` computed the same way as
  the Java `Weapon`/`Armor`/`EtcItem` constructors). Combat-stat bonuses under
  `<stats>` stay unparsed (later milestone).
- **`data/initial_equipment.rs`**: `initialEquipment.xml` → starting gear per
  class.
- **`model/inventory.rs`** rewritten: real `ItemInstance`s + a paperdoll that
  stores `object_id`s into that list (mirrors Java's `PlayerInventory`
  referencing the same `Item` objects). `equip_item`/`unequip_slot` port
  `PlayerInventory.equipItem`'s slot-conflict resolution for the cases
  ordinary gear hits (two-handed weapons, full-armor vs chest+legs, dual ear/
  finger/bracelet slots) — formalwear, pet items, and arrow/bolt auto-swap are
  explicitly out of scope.
- **DB**: `items` rows load alongside every character (not just the one
  entered — `CharSelectionInfo` needs paperdoll icons for the whole select
  list too); `CreateCharacter` persists resolved starting gear; new
  fire-and-forget `DbCommand::UpdateItemLocation` for runtime equip/unequip.
- **Character creation**: replays `initialEquipment.xml` through a scratch
  `Inventory` (`add_item`/`equip_item` in XML order, exactly like Java's
  `initNewChar` loop) so slot-conflict resolution matches Java by
  construction; starting adena from `Character.ini` `StartingAdena`.
- **Packets**: `ItemList`, `InventoryUpdate`, `ExAdenaInvenCount`,
  `ExUserInfoInvenWeight` now carry real data; `ExUserInfoEquipSlot` and
  `CharSelectionInfo`'s paperdoll block needed no format changes, just real
  data behind them.
- **Runtime**: `UseItem` (0x19, gear only — potions/shots stay a no-op) and
  `RequestUnEquipItem` (0x16) toggle equip state, send `InventoryUpdate` +
  `UserInfo`, persist via `UpdateItemLocation`.
- **Bug fixed**: `IdManager`'s next-id counter only checked
  `MAX(characters.charId)`, not `MAX(items.object_id)` — on the real dev DB
  (which has items with higher object ids than any character), freshly
  allocated item ids collided with existing rows and silently failed to
  insert (only some starting items would show up). Fixed to take the max of
  both tables, matching Java's single shared `IdManager` pool.

### G6 — Stats, skills & effects ✅ vertical slice
Real combat-stat calc, persisted/learnable skills, and a working buff cast
pipeline — scoped to self-targeted skills (see below); damage-dealing effects
and combat proper wait for G9, which is where there's finally something to
hit. Full writeup + scope rationale in the design research behind this
milestone; summary:

- **`model/stats.rs`** (new): `Stat` enum (scoped subset: p/m atk+def,
  atk/cast speed, crit, evasion, accuracy, regen rates, speed — grows as later
  milestones need more, same pattern as `UserInfoType`/`InventorySlot`) and
  `BaseStat` (STR/DEX/CON/INT/WIT/MEN). `data/stat_bonus.rs` extended from
  CON/MEN-only to all six, still one `statBonus.xml` table.
- **`Player::recalculate_stats`**: real `p_atk`/`p_def`/`m_atk`/`m_def`/
  `p_atk_spd`/`m_atk_spd`/`crit_hit`/`m_crit_hit`/`evasion`/`accuracy`/
  `magic_evasion`/`magic_accuracy`/speed, ported from the Java `Stat`
  finalizers (`PAttackFinalizer`, `PDefenseFinalizer`, …): template base ×
  `BaseStat` bonus × level mod (`(level+89)/100`), then `Player.stats_add`/
  `stats_mul` (Java `CreatureStat`'s two modifier maps) folded in — this is
  what buffs push into. Replaces the G4-era placeholder (template value or 0).
  TODO(G8+): weapon/armor `<stats>` contributions — item stat bonuses aren't
  parsed yet, so this is the unarmed/naked value (same simplification G5 made
  for item stats generally).
- **Passive regen**: a 3 s fixed-rate tick (`REGEN_TICK_PERIOD`, Java
  `Formulas.getRegeneratePeriod`) over in-game players, porting
  `RegenHPFinalizer`/`MPFinalizer`/`CPFinalizer` (× a flat "standing still"
  1.1 multiplier — TODO: sit/run states, out of G7's move-only scope). New
  `StatusUpdate` server packet.
- **Skills**: `character_skills` now loads on select/enter-world and persists
  via a new fire-and-forget `DbCommand::UpsertSkill`; `Player.skills` (skill_id
  → level); real `SkillList`. `data/skill_tree.rs` extended from "level-1
  autoGet only" to the full base-class progression (`SkillLearn`:
  `get_level`/`level_up_sp`), driving a real `AcquireSkillList` and
  `RequestAcquireSkill` (`AcquireSkillType::CLASS` only — confirmed Java skips
  the trainer-NPC check for `CLASS`, so learning needs no village-master NPC).
- **Effects**: `model/skill.rs`'s `StatModifierEffect{stat, mode, amount}` is
  the Rust counterpart of Java's `AbstractStatAddEffect`/
  `AbstractStatPercentEffect` — one generic type instead of the 63 one-line
  subclasses Java has. `data/skill_data.rs`: a generic per-level-value XML
  loader for `data/stats/skills/*.xml`, with a curated `<effect name>` → `Stat`
  registry (18 names — `PAtk`, `PhysicalDefence`, `HpRegen`, …; unregistered
  names, e.g. the damage effects, are dropped and the skill still loads).
  Buffs live in `Player.buffs`, expire via a new `ScheduledTask::BuffExpire`.
  Real `AbnormalStatusUpdate` (self-only — no known-list yet for
  `ExAbnormalStatusUpdateFromTarget`). `apply_buff` ports Java
  `EffectList.addActive` stacking: a buff of the same abnormal type (or same
  skill id when the type is `NONE`) never stacks — the higher/equal abnormal
  level replaces in place, a lower one is refused; good buffs are capped at
  `MaxBuffAmount` (24) and dances/songs at `MaxDanceAmount` (12) in separate
  pools, dropping the oldest when exceeded. The scheduled `BuffExpire` only
  fires once the current buff has truly elapsed, so a re-cast/refresh isn't
  dropped early by a stale task. Buff/debuff **duration** honors `Character.ini`
  `EnableModifySkillDuration`/`SkillDurationList` (**True** on this dist —
  stretches most songs/dances/buffs to 2h): the `skillId,seconds` list overrides
  each skill's `abnormalTime` at boot (`SkillData::apply_skill_duration_list`,
  called from `main.rs` like `combat_caps`), matching Java's `Skill` constructor
  — toggles are exempt, enchanted levels (100–139) add rather than replace. Every
  downstream reader of `abnormal_time` (buff expiry ticks, DoT scheduling) then
  sees the config value transparently.
  dropped early by a stale task. **`RequestDispel`** (alt+click a buff icon,
  ex `0xD0:0x0048`) ports the Java gate — `canBeDispelled` && !`isDebuff`, not a
  TRANSFORM abnormal, dances only under `DanceCancelBuff` (new Character.ini
  config, True on this dist) — then force-removes the self-buff via the shared
  `handle_buff_expire` path (reverting stats + `AbnormalStatusUpdate`). Skill
  parsing gained `can_be_dispelled`/`is_debuff` flags. Pet/servitor dispel is
  `TODO(G29)`.
- **Cast pipeline** *(superseded by G7.5 below — real 3-phase timing,
  targeting, reuse, abort)*: `RequestMagicSkillUse` → a 2-phase scheduled flow
  (`ScheduledTask::SkillLaunch` at `hit_time`, then `finishSkill` inline — no
  separate cancel-time wait, since G6 only handles instant `SELF`-targeting)
  porting `SkillCaster`: MP/HP checks at both start and landing,
  `MagicSkillUse`/`SetupGauge` → `MagicSkillLaunched` → `StatusUpdate` +
  `AbnormalStatusUpdate`. Scoped to `TargetType::SELF`, `OperateType::Active`
  known skills — other targeting, passive/toggle skills, and damage effects
  are out of scope (no NPCs/combat/visibility to aim at yet; see G9).
- **Tests**: `data::skill_tree::tests` (learn-list gating by level/known-skill);
  a synthetic-`World` test (`game_loop::tests::
  learn_and_cast_buff_skill_applies_and_expires`, no sockets, per the tick-
  system testing strategy) drives the real handlers end-to-end — learn
  "Defense Aura" (SP spend + level gate) → cast it → land (P.Def +8%, right
  packet sequence) → fast-forward `world.tick` past `abnormalTime` → expire
  (P.Def back to naked) — since real-time-waiting out a 20+ minute retail buff
  isn't a reasonable thing for a unit test to do.
- **`e2e_create.rs` fix**: the new regen tick can push an unsolicited
  `StatusUpdate` mid-test once a character is in-game (e.g. CP regenerating
  from its post-creation 0); added `GameClient::recv_skip_status_update` so
  reply-then-assert exchanges after enter-world aren't thrown off by it.

### G7 — Movement & targeting (no geodata) ✅
Scoped-down slice of the vertical-slice gate's original "movement +
known-list" gap: player-to-player targeting and click-to-move, both trusting
the client outright (no geodata/pathfinding validation yet — see the
deferred-TODO note below).

- **`Player` fields**: `target: Option<i32>` (targeted object id — Player-only,
  no NPCs/items exist as `WorldObject`s yet) and `move_data: Option<MoveData>`
  (`model/movement.rs`, a geodata-free port of Java's nullable `Creature._move`
  — start/dest x/y/z, `start_tick`, `total_ticks`).
- **Targeting**: `Action` (0x1F) resolves a click to another in-world player
  and calls `set_target`, a narrowed port of `Player.setTarget` (skips the
  party/vehicle/GM checks — neither exist yet): same-target re-click is a
  no-op; a real change sends `MyTargetSelected` + a `StatusUpdate`(HP) to the
  selector and broadcasts `TargetSelected` to everyone else; clearing
  broadcasts `TargetUnselected`. `RequestTargetCanceld` (0x48) reads the
  `targetLost` flag and clears the target the same way. Every `Action` ends
  with the `ActionFailed` terminator, matching `WorldObject.onAction`'s
  convention (**`ActionFailed`/opcode `0x1F` server packet added** — didn't
  exist before this milestone).
- **Movement**: `MoveBackwardToLocation` (0x0F) ports the
  `Creature.moveToLocation` math minus the entire geodata/pathfinding block
  (`Creature.java` ~3651-3816) — same-origin/target → `StopMove`; max
  click-distance (9900²) and `player.casting` are the only guards kept (the
  rest of Java's `isMovementDisabled()` — rooted/overloaded/immobilized/dead/
  teleporting — has no state to check yet); otherwise computes heading
  (`Util.calculateHeadingFrom` port) and `total_ticks` from distance/speed,
  sets `move_data`, and broadcasts one `MoveToLocation` to other players (the
  mover self-predicts, per Java — no packet sent back to itself). A new
  per-tick system (`movement::tick`, called unconditionally every 100 ms
  iteration, unlike the gated `REGEN_TICK_PERIOD` systems) interpolates
  position each tick and snaps to the destination on arrival — no `StopMove`
  broadcast needed then, since the client already predicted it.
- **Broadcast stopgap**: `broadcast_to_others` (`game_loop.rs`) sends to every
  connected in-game player except the actor — a flat pass, not a real
  known-list/region-grid (superseded by G7.9's region-scoped visibility).
- **Tests**: synthetic-`World` unit tests (`game_loop::tests`) —
  `action_selects_switches_and_cancels_target` (select/re-click no-op/cancel,
  checking both the selector's and the target's packet streams) and
  `move_backward_to_location_interpolates_and_arrives` (mid-flight
  interpolation + exact arrival snap, verifying the bystander gets
  `MoveToLocation` but the mover doesn't) plus the same-origin `StopMove` case.

### G7.5 — Full single-target skill casting ✅
Supersedes G6's self-only 2-phase cast slice with a faithful port of the
`RequestMagicSkillUse` → `Player.useMagic` → `SkillCaster` pipeline: casting
on the current target (players only — still no NPCs), Java's real timing and
damage math, server-side reuse enforcement, and cast interruption.

- **`model/formulas.rs`** (new): ports of `Formulas.calcMagicDam`
  (`77·power·√mAtk/mDef`, ×2 on magic crit), `calcCrit`'s magic branch
  (per-mille rate, 320/200 caps), `calcSkillTimeFactor`/`calcSkillCancelTime`/
  `calcAtkSpd` (casting-speed-scaled `hitTime`, 500 ms launch floor, cool
  phase), `Heal.java`'s `power + √(2·mAtk)` (×3 crit), and `calcAtkBreak`
  (cast break on hit). Each fn doc-comments the dropped terms — all identity
  for unarmed/shotless players (shots, traits, attribute, pvp/pve config
  multipliers). The `ALT_GAME_MAGICFAILURES` resist branch is **ported** — see
  the magic-failure entry below.
- **Magic failure vs. higher-level targets** (`calcMagicSuccess` +
  `calcMagicDam`'s `ALT_GAME_MAGICFAILURES` block): magic damage against an
  out-of-level target is now resisted the way Java resists it. `calcMagicSuccess`
  is ported in full — the PvE branch's `rate = 100 - round(1.3^(targetLevel -
  effectiveLevel))` (effectiveLevel = the skill's `magicLevel`, since dist sets
  `CalculateMagicSuccessBySkillMagicLevel = True`), the level-78+
  `SkillChancePenaltyForLvLDifferences` multiplier (raid-exempt, player-caster
  only), and the PvP `magicAccuracy - magicEvasion` step table. `MagicalAttack`
  and `HpDrain` roll it at `calcMagicDam`'s point in the formula: a first failed
  roll triggers a *second* roll that picks between half damage ("Your attack has
  failed" / "Drain was only 50% successful") and a flat 1 ("$c1 has resisted your
  $s2"); the target, if a player, always gets its "You resisted $c1's magic/drain"
  line. Two Java quirks are preserved deliberately: the reduction is applied
  **before** the crit multiplier (so a resisted magic crit deals 2, not 1), and
  an **NPC caster that fails the roll still deals full damage** — Java only
  reduces damage inside its `attacker.isPlayer()` branch. `resModifier`
  (`MAGIC_SUCCESS_RES`) stays 1.0: the only two dist items touching
  `magicSuccRes` declare it in a `<stats>` block, which Java parses as an
  additive func that `getMul` never sees. Before this, a level-5 character's
  Wind Strike killed a level-60 mob at full damage. New config: `MagicFailures`
  (Character.ini), `MinNPCLevelForMagicPenalty` +
  `SkillChancePenaltyForLvLDifferences` (NPC.ini).
- **3-phase cast state machine**: `Player.cast: Option<CastState>` (replaces
  `casting: bool`; snapshots skill/target/timings) + `cast_seq` generation
  counter. `startCasting` (reuse registration, stop-move, `ExRotation` target
  facing, initial MP, broadcast `MagicSkillUse`, SM 46 + `SetupGauge`) →
  `SkillLaunch` at `hit` (effect-range re-check → SM 748 quiet stop;
  broadcast `MagicSkillLaunched`; marks the cast unabortable) → `SkillFinish`
  at `+cancel` (MP/HP consume with SM 23/24 on shortfall, effect application)
  → `CastEnd` at `+cool`. Scheduled tasks carry `cast_seq` and no-op on
  mismatch — aborting is just clearing `Player.cast`, no heap surgery.
- **Abort/interrupt**: `abort_cast` (port of `Creature.abortCast` →
  `stopCasting(true)`, pre-launch only): broadcast `MagicSkillCanceled`
  (new packet, 0x49) + `ActionFailed`. Wired to Esc
  (`RequestTargetCanceld`, which Java aborts on regardless of the
  `targetLost` flag) and to incoming magic damage via `calcAtkBreak`
  (SM 27). Movement during a cast stays blocked with `ActionFailed`
  (`PlayerAI.onIntentionMoveTo` semantics — it does *not* abort).
- **Reuse**: `Player.reuses` (`Skill::reuse_key()` — the shared
  `reuseDelayGroup` when set, else skill id — → `SkillReuse`, one map for
  Java's `_reuseTimeStampsSkills`/`_disabledSkills` split), registered at
  cast start, checked lazily in the `useMagic` gate — SM 48 for short
  reuses, SM 2303/2304/2305 with the h/m/s breakdown for >3 s ones. Real
  `SkillCoolTime` packet (enter-world + `RequestSkillCoolTime`).
  Persistence across relog still deferred.
- **Targeting**: `resolve_cast_target` — static match port of the
  `Self`/`Target`/`Enemy`/`EnemyOnly` target-handler scripts (players only,
  no geodata LOS/peace zones; with no PvP flags an `ENEMY` cast always needs
  ctrl/force-use). Cast-range gate ports `Util.checkIfInRange` with collision
  radii (out-of-range = `ActionFailed`; Java's walk-into-range AI was not
  ported at the time — done post-G9.5 via `PlayerIntent::Cast`).
- **Effects**: `SkillEffect` enum (`StatModifier` | `MagicalAttack` |
  `Heal`) replaces the stat-modifier-only effect list; buffs now land on the
  *resolved target* (buff-a-friend works). Magic damage drains **CP first**
  then HP (`PlayerStatus.reduceHp`), clamped at 1.0 HP — no death system
  yet (TODO G9 `doDie`) — with SM 2261/2262 damage messages + `M_CRITICAL`.
  Heals overheal-clamp and send SM 1066/1067.
- **Packets**: parameterized `SystemMessage` builder
  (`system_message_with` + `SmParam` Text/Int/SkillName/PlayerName, `sm_ids`
  constants), `MagicSkillUse` with real target fields, multi-target
  `MagicSkillLaunched`, `MagicSkillCanceled`, real `SkillCoolTime`;
  `RequestMagicSkillUse` now reads `shiftPressed`. `World.rng` + `roll()`
  (test hook: `forced_rolls`) for the crit/break dice.
- **Skill-XML loader fix**: the `<list>` document root was being pushed onto
  the parser's tag stack, shifting every depth check by one — **the loader
  parsed 0 skills from the real dist XMLs** (G6's tests bypassed it with
  `insert_for_test`, hiding it). Now guarded + regression-tested against the
  real files (`loads_real_dist_files`, >10 000 skill levels). Parser also
  reads per-level `targetType`, `isMagic`, `effectPoint`, `hitCancelTime`,
  and `<power>` effect params.
- **Tests**: `formulas` unit tests with exact Java values; parser tests
  (Wind-Strike/Heal-shaped XML); synthetic-`World` integration tests for the
  full nuke-on-player flow (exact damage, CP-first, both packet streams,
  reuse gate), no-ctrl/out-of-range rejections, HP clamp, Esc abort +
  stale-task no-op + reuse surviving the abort, effect-range re-check,
  heal-with-formula + overheal clamp, buff-on-other + expiry, quiet
  finish-phase MP failure, `SkillCoolTime` contents, and damage breaking a
  victim's pre-launch cast.

### G7.8 — Geodata & position validation ✅
Closes G7's "trust the client outright" gap: the stock `.l2j` geodata files
now load and back server-side LOS + walkability checks.

- **`geo/` module** (`mod.rs`, `region.rs`, `line.rs`): port of
  `geoengine/GeoEngine` + `geodata/GeoData`/`regions/Region`/`blocks/*` and
  the `LinePointIterator`/`3D` cell walkers. Unlike Java's eager
  multi-GB block-object parse, each region file is **mmap'd read-only**
  (`memmap2`) and queried in place; the only parsed state is a 64K-entry
  block-offset index built in one validation pass (plan §risks: "mmap +
  read-only shared geodata"). Flat/complex/multilayer blocks, NSWE checks
  (incl. `checkNearestNsweAntiCornerCut`, Java's NW quirk kept for parity),
  `getNearestZ`/`getNextLowerZ`/`getNextHigherZ`, `getSpawnHeight`,
  `canSeeTarget` (48-unit see-over, elevated-origin allowance),
  `canMoveToTarget`, `getValidLocation`. Not ported: door/fence LOS
  carve-outs (no doors/fences yet), runtime NSWE editing.
  (`CellPathFinding` landed later as G7.85 — see below.)
- **Boot**: new `config/geoengine.rs` reads `GeoEngine.ini` (`GeoDataPath`,
  `PathFinding`); `main.rs` prints the Geodata section and loads all 227
  dist regions (~2.5 s, debug) into `World.geo` (`GeoEngine::empty()` =
  Java `NullRegion` everywhere for tests).
- **Movement** (`handle_move_backward_to_location`): ports
  `Creature.moveToLocation`'s geodata block — destination clamped via
  `getValidLocation` (players keep client z, far-click > 3000 and
  fall-intent guards honored), fully-clamped moves canceled with
  `ActionFailed`. (The pathfinding fallback — Java walks around an
  obstacle when the clamp shortened the path > 30 — landed as G7.85.)
- **`ValidatePosition` (0x59)** — previously unhandled: full
  `runImpl` reconciliation (trust-the-climb z adoption, moderate-drift
  `ValidateLocation` correction (new packet, 0x79), out-of-sync snap with
  geodata z pull-down), storing `Player.client_x/y/z/heading`. Vehicle/
  falling/flying/water/observer/Blink branches skipped (states don't exist).
- **Casting LOS**: `resolve_cast_target` now returns `Result` and ends with
  the target handlers' "Geodata check when character is within range" —
  `canSeeTarget` failure → SM 181 (`CANNOT_SEE_TARGET`) + `ActionFailed`
  (self-target bypasses, per `Target.java`).
- **Tests**: region cell-encoding/block-type/corruption units; line-walker
  units; synthetic-region wall & low-fence LOS/movement/`getValidLocation`
  behavior; real-dist load smoke test (Giran ground z, open-square LOS,
  spawn snap); game-loop tests for move clamping, blocked-move cancel,
  SM 181 on cast through a wall, and the three `ValidatePosition` branches.
  Also fixed a test-suite race: dist-loading tests now use absolute
  `CARGO_MANIFEST_DIR` paths (the ipconfig test chdirs the process
  mid-run and could starve relative-path loaders).

### G7.85 — Pathfinding (path-worker service) ✅
Closes G7.8's "walks up to the obstacle and stops" gap: blocked player
moves now route around obstacles via the `CellPathFinding` port, running
on a dedicated worker thread per CONCURRENCY_MODEL §2.4 (the game thread
never blocks on a path search).

- **`geo/path.rs`**: pure-function port of `CellNodeBuffer` (best-first
  search with the cost-sorted-chain open list, arena-allocated nodes
  instead of Java's object graph, all weights/`MAX_ITERATIONS`/z-keying
  quirks kept) + `CellPathFinding.findPath` (buffer sizing from
  `PathFindBuffers`, `constructPath` direction-change compression, the
  `canMoveToTarget` postfilter with its playable/AI pass asymmetry).
  Java's cross-thread buffer pool is collapsed to "smallest configured
  size that fits, allocated fresh" — single worker, so pooling buys
  nothing; the size ceiling (too-far request ⇒ no path) is preserved.
- **`geo/worker.rs`**: the path-worker thread. `PathRequest` in via
  `std::sync::mpsc`, `PathEvent` back to the game loop, drained per tick
  (`drain_path`, same shape as `drain_db`). `World.geo` became
  `Arc<GeoEngine>` so the worker shares the mmap'd geodata read-only.
- **Async move flow** (`position.rs`): when the `getValidLocation` clamp
  shortens a click by > 30 units, the handler stores a `PathWait { seq }`
  component and sends the *original* destination to the worker instead of
  starting the move; the reply (`handle_path_result`) either starts a
  route move or answers `ActionFailed` (no path — Java's player branch).
  Stale replies (player re-clicked → newer seq, or left) are dropped;
  re-clicks onto the geo cell already being pathed to are ignored and
  clicks elsewhere abandon route following, both per Java
  `isOnGeodataPath()`. The one-tick (~100 ms) confirmation delay replaces
  Java's synchronous in-handler search.
- **Route following** (`model/movement.rs`): `MoveData.geo_path`
  (`points`/`index`/`accurateTx/Ty`/`gtx/gty` as one `Option<GeoPath>`);
  segment completion in the movement tick runs `moveToNextRoutePoint`
  (next dest — accurate destination on the final segment — ticks
  recomputed at current speed, heading updated) and the caller broadcasts
  `MoveToLocation` per segment.
- **Config/boot**: `config/geoengine.rs` now reads the full tuning block
  (`PathFindBuffers`, `Low/Medium/High/DiagonalWeight`,
  `AdvancedDiagonalStrategy`, `MaxPostfilterPasses`) into a `PathConfig`;
  `main.rs` spawns the worker with a clone of the geodata `Arc` and joins
  it at shutdown (channel close stops it).
- **Not ported yet**: NPC moves stay straight-line (Java also paths
  chase/return-home moves and has the Attackable closest-reachable-point
  grid scan); `GeoPathFinding` (`PathFinding = 1` node files — Java's own
  default is 2, cell pathfinding); debug-item drops and `getStat()`
  counters.
- **Tests**: algorithm units on synthetic regions (walk-around through a
  wall gap with every postfiltered leg verified walkable, sealed wall ⇒
  `None`, no-geodata ⇒ `None`, over-buffer distance ⇒ `None`) + a
  real-dist Giran route; game-loop tests for the deferral (PathWait, no
  packet until reply) and a full round-trip against a live worker thread
  (click across a wall → route move with several segments →
  `MoveToLocation` per advance → arrival at the exact requested
  destination).

### Post-G7.8 — Restart/Logout + player persistence ✅
Fixed "relogin ignored": the client's `RequestRestart` (0x57) and `Logout`
(0x00) opcodes were unhandled, so leaving the world was impossible without
killing the client.

- **`RequestRestart`**: Java `storeMe().deleteMe()` + `RestartResponse.TRUE`,
  session `InGame → Authenticated` (new type-state transition; `InGame` now
  carries the `SessionKey` for it), then the character list reloads through
  the normal `Authenticated → InLobby` path. `canLogout` guards (attack
  stance, NO_RESTART zones) are TODO with combat (G9).
- **`Logout`**: store + remove player, send `LeaveWorld` (0x84), drop the
  session (socket closes after the flush; `on_disconnect` does the login-
  server notify). From the lobby it just disconnects, like Java.
- **Persistence** (`DbCommand::StorePlayer` + `PlayerSnapshot`): port of
  `Player.storeCharBase` narrowed to tracked columns (level, HP/MP/CP,
  position/heading, exp/sp, reputation, PvP/PK, class ids, vitality) +
  `updateOnlineStatus` (`online=0`, `lastAccess=now`) in one UPDATE. Runs on
  restart, logout, **and unexpected disconnect** (incl. the `Entering`
  state, where the `Player` still lives on the session). `storeCharSub` and
  `storeEffect` have since landed (G17 subclasses; cooldowns in G13.9/G17 and
  buffs in "Buff persistence" below); item-reuse persistence is still deferred.
- **Tests**: restart store+lobby round trip, restart → re-enter world (the
  original bug), logout store+`LeaveWorld`, disconnect store.

### Post-G7.8 — Skill reuse groups ✅
Fixed "every skill icon refreshes on any cast": `MagicSkillUse` (and
`SkillList`) hardcoded the reuse-group field to 0, which the client treats as
a shared everything-group; Java sends `Skill.reuseDelayGroup` (default **-1**
= ungrouped).

- **`Skill.reuse_delay_group`**: parsed from `<reuseDelayGroup>` (default -1)
  and written raw into `MagicSkillUse` and `SkillList`.
- **Shared cooldowns**: `Player.reuses` is now keyed by `Skill::reuse_key()`
  (group id when positive, else skill id — Java's `_reuseHashCode` minus the
  per-level dimension), value is a `SkillReuse` carrying the cast level so
  `SkillCoolTime` can report `group-or-id + level` like Java.
- **Tests**: ungrouped casts assert the -1 group byte in `MagicSkillUse`;
  grouped siblings share one cooldown (gate + `SkillCoolTime` group id);
  `loads_real_dist_files` probes a real grouped skill (10248 → group 10008).

### G7.9 — Region-grid visibility & scoped broadcasting ✅

Port of Java's world-region knownlist for player↔player visibility — the
first time two clients actually see each other's characters.

- **Region math** (`world.rs`): `REGION_SHIFT` (Java `World.SHIFT_BY` = 11 ⇒
  2048-unit cells), `region_of(x, y)`, and `regions_adjacent` (the 3×3
  surrounding-region rule, Java `WorldRegion.isSurroundingRegion`). Java's
  per-region object lists are *not* materialized: with players as the only
  world objects, each `Player` carries its current region cell
  (`Player.region`, kept in sync by `game_loop/visibility.rs`) and every
  query is an adjacency compare — identical semantics, no grid to keep
  consistent. The real grid collections can arrive with G8 NPC counts.
- **`CharInfo` (0x31) + `DeleteObject` (0x08)** (`server_packets.rs`): the
  full Interlude-Classic `CharInfo` layout (paperdoll/augment/visual orders
  included; clan/mount/store/cubic/fishing fields as empty Java defaults).
- **Scoped broadcasting** (`game_loop/helpers.rs`): `broadcast_to_others` /
  `broadcast_including_self` now send only to players whose region is
  adjacent to the broadcaster's (Java `broadcastPacket` via
  `World.forEachVisibleObject`), replacing the flat all-clients pass.
- **Visibility lifecycle** (`game_loop/visibility.rs`): `on_enter_world`
  (Java `spawnMe` → `addVisibleObject`: mutual `CharInfo`), `update_region`
  (Java `updateWorldRegion` → `switchRegion`: `DeleteObject`/`CharInfo`
  deltas both ways, dangling-target clearing, and
  `describeStateToPlayer`-style `MoveToLocation` for movers entering view),
  `on_leave_world` (Java `removeVisibleObject`: `DeleteObject` to watchers on
  logout/restart/disconnect). Hooked into the movement tick
  (`visibility::movement_tick` wraps `movement::tick`), the
  `ValidatePosition` out-of-sync snap, `handle_enter_world`, and
  `store_and_remove_player`.
- **Tests** (`game_loop::tests`): enter-world CharInfo exchange scoped by
  region, broadcast scoping (near vs far bystander), region-crossing
  `DeleteObject`/`CharInfo` + mid-move introduction, and leave-world
  `DeleteObject` + target drop.

### G8 — Static world content (NPCs/spawns) ✅ vertical slice
The world is no longer empty: every static spawn line places a live NPC that
players can see, target, and talk to. Scoped to what makes NPCs *exist* —
zones, doors, static objects, respawn, and any NPC behaviour (AI, random walk,
combat) are deferred (respawn is unreachable anyway until G9's `doDie` gives
NPCs a way to die).

- **`data/npc_data.rs`**: port of `NpcData` — all 191 `data/stats/npcs/*.xml`
  files → 14 407 `NpcTemplate`s (identity/display fields, base stats/vitals/
  speeds, collision, equipment rhand/lhand, status flags, aggro ranges;
  skill/drop/attribute lists wait for G9). Type classification
  (`is_monster`/`is_attackable_class`) mirrors Java's `instanceof
  Monster`/`Attackable` subtree checks — there's no class hierarchy to lean
  on, so the `type` attribute is matched against the instance-class sets.
- **`data/spawn_data.rs`**: port of `SpawnData`/`model/spawns/*` — all 154
  `data/spawns/**` files → 27 154 spawn lines (fixed locations, `count`,
  `respawnTime`/`respawnRandom` durations, spawn- and group-level
  `<territories>` with the NPoly/Cuboid/Cylinder `ZoneForm`s). Features with
  zero usages in this dist are not ported (`zone=`, `banned_territory`,
  `<locations>`, `<minions>`, `respawnPattern`); `dbSave` raid persistence
  (`DBSpawnManager`, 225 lines) spawns statically for now.
- **`model/npc.rs`**: the composed `Npc` world object (position/region/
  HP/MP; everything else reads through the template) + `spawn_all`, the
  `Spawn.doSpawn`/`initializeNpc` port: territory spawns get a random point
  (bounding-box rejection sampling, Java's 1000-try cap) at
  `GeoEngine.getHeight`, monsters snap to the geodata surface (<300 units),
  `heading == -1` randomizes with Java's odd `Rnd.get(61794)` bound.
  Boot places **34 869 NPCs** in ~1 s (891 lines skipped: Servitor/Pet/
  Defender/Decoy/Trap plus types with no instance class — those fail
  reflection on the Java server too). NPC object ids come from a dedicated
  transient base (`0x4000_0000`) instead of Java's shared `IdManager` pool
  (the pool lives on the DB thread; NPCs never persist).
- **`World`**: `npcs` registry + `npc_regions` — the first materialized
  region-grid collection (players still use the per-player adjacency compare;
  NPCs are static and 34.9k strong, so the index is built once at spawn).
- **`NpcInfo` (0x0C)** (`server_packets.rs`): the masked packet (5 mask
  bytes, "mask_bits_37", pre-set gap components) via the shared `masks.rs`
  helpers + a new `NpcInfoType` enum (explicit non-contiguous discriminants).
  Component selection ports the Java constructor with absent systems at their
  defaults. Unit-tested against hand-computed bytes (no NPC client capture
  yet — the mask math is shared with the byte-verified `UserInfo` path).
  `write_f32` added to `commons::PacketWriter` for the speed multipliers.
- **Visibility** (`visibility.rs`): enter-world sends `NpcInfo` for the 3×3
  region block; region crossings send `NpcInfo`/`DeleteObject` deltas both
  ways and drop dangling NPC targets (players get nothing new from NPCs —
  aggro/AI eyes are G9).
- **Targeting/interaction** (`target.rs`): `Action` resolves NPCs —
  `Player.setTarget` generalized over players and NPCs (`ValidateLocation` +
  `MyTargetSelected` with the level-diff color for auto-attackable targets +
  HP `StatusUpdate` + `TargetSelected` broadcast; z-diff and `targetable`
  guards). Second click = the `NpcAction` interact branch: monsters no-op
  (attack intent is G9), others within `INTERACTION_DISTANCE` (250) get
  `Npc.showChatWindow` — `NpcHtmlMessage` (0x19) from
  `data/html/<type-dir>/{id}.htm` with the Folk `npcdefault.htm` fallback and
  `%objectId%`/`%npcname%` replacement (read per interaction; no `HtmCache`).
  Out-of-range clicks walk in first (`PlayerIntent::Interact`, `combat.rs`'s
  `start_interact_intent`/`player_interact_think` — same chase-then-act shape
  as the cast/attack intents) and re-run the interact click on arrival, same
  as Java's `doInteract` re-dispatching `onAction`.
- **Tests**: loader tests against the real dist (counts + hand-checked
  templates/spawn lines, elemental `<attribute>` vs base `<defence>`
  disambiguation, duration parsing, NPoly containment); `spawn_all` smoke
  test over the real datapack (placement count, retail coordinates, region-
  index consistency); `NpcInfo` byte test; synthetic-world tests for
  enter-world NPC burst scoping, region-cross deltas + NPC-target drop, and
  the two-click select→chat-window / monster-no-chat flows. `e2e_create`'s
  skip-unsolicited helper now also skips `NpcInfo` (the starting village's
  NPCs arrive in the enter-world burst).

### G9 — Combat & AI ✅ vertical slice
The G9 gate end-to-end: kill a monster (melee and skill), take damage back,
receive XP/SP/loot, level up, die, and revive in town. Scoped to melee
single-hit combat and plain monsters — see the deferred list for what
consciously stayed out.

- **Config** (`config/rates.rs`, `config/npc.rs`, `character.rs` grown):
  `Rates.ini` (XP/SP ×50 on this dist!, drop chance/amount multipliers incl.
  the per-item `57,50;…` lists, `DropMaxOccurrences*`, the drop level-gap
  window keys), `NPC.ini` (`DefaultCorpseTime`, `MaxDriftRange`),
  `Character.ini` (`AutoLoot` — **True** on this dist, `RespawnRestoreHP` 65,
  `AltPartyRange`, `Delevel`/`DelevelMinimum`, `RandomRespawnInTownEnabled`).
  Bundled as `CombatConfig` on `World.cfg` (tests get Java defaults, ×1
  rates).
- **Data loaders**: `hit_condition_bonus.rs` (front/side/back/high/low —
  night/rain need a game clock/weather), `xp_lost.rs`
  (`playerXpPercentLost.xml`), `map_region.rs` (`data/mapregion/*` tiles +
  town respawn points, `talking_island_town` fallback); `npc_data.rs` grown:
  `<attack random critical>`, `<corpseTime>`, `<dropLists>` (`<drop>` lines
  + the `<group chance>` shape the Primeval Isle file uses; spoil dropped).
- **Physical formulas** (`model/formulas.rs`): `calculateTimeBetweenAttacks`
  (`500000/atkSpd`, 50 ms floor), melee `calculateTimeToHit` (0.644/0.735),
  `calcHitMiss` (`(80+2(acc−evasion))·10` × HitConditionBonus, clamp
  [200,980]), auto-attack `calcCrit` (position 1.1/1.3 + height bonus, clamp
  [3,97]), `calcAutoAttackDamage` (`(pAtk·rnd + proxBonus)·77/pDef`, crit ×2
  — soulshot/shield/ranged/trait terms identity and documented), the
  level-gap XP table, `Util.map` for the drop level gates. `Position`
  (front/side/back from headings) in `movement.rs`.
- **Auto-attack pipeline** (`game_loop/combat.rs`): `AttackRequest` (0x32) /
  second `Action` click on a monster → `PlayerIntent::Attack` — a per-tick
  think (`PlayerAI.thinkAttack` + the 500 ms follow cadence) that chases via
  `MoveToPawn` and swings with `Creature.doAutoAttack`'s shape: hit rolled at
  swing start (`generateHit`), `Attack` (0x33) broadcast, damage landing on a
  scheduled `AttackHit` at `timeToHit` (in-flight swings die with either
  side). Shared `Combatant` view derives NPC stats from templates through
  the same finalizer math (STR/DEX bonuses × level mod). Combat stance
  tracker (`AutoAttackStart/Stop` 0x25/0x26, 15 s), damage messages
  (SM 2261/2262/2264/2265/2266 + miss/crit), CP soak only from playable
  attackers, cast-break on hit. Magic damage now routes through the same
  receivers — the G7.5 "clamp at 1.0 HP" is gone.
- **Monster AI** (`game_loop/npc_ai.rs`): 1 s think over monsters in active
  regions (player-adjacent cells only, Java's region-activation gate).
  `thinkActive`: `_globalAggro` −10→0 spawn calm, aggro-range scan (alive +
  region-adjacent + LOS) seeding 1 hate, most-hated pick → run mode
  (`ChangeMoveType` 0x28) + Attack intention; drift-home walk past
  `MaxDriftRange`. `thinkAttack`: 120 s attack timeout (walks home — Java
  teleports), hate pruning on dead targets, chase (`MoveToPawn` re-pathed per
  think) and swing through the shared pipeline. NPC movement rides the
  interpolation tick with `npc_regions` re-indexing + `NpcInfo`/
  `DeleteObject` visibility deltas on cell crossings.
- **Death/decay/respawn** (`game_loop/death.rs`): `doDie` both kinds (`Die`
  0x00 broadcast; players get the to-village flag + XP penalty via
  `playerXpPercentLost` with the `Delevel` clamp; dead players are barred
  from move/cast/attack and regen). NPC corpse decays after
  `<corpseTime>`/`DefaultCorpseTime` (`DeleteObject`, dangling targets
  dropped), `Spawn.decreaseCount` schedules the respawn (min/max random
  spread) and the spawn line re-runs — fresh transient object id, a
  documented deviation from Java's id-reusing `respawnNpc`.
- **Buffs and death** (`death.rs::stop_effects_on_death`): `Playable.doDie`'s
  effect block, which the port had been missing entirely — **a dead player kept
  every buff through death and revive**. Now death runs
  `stopAllEffectsExceptThoseThatLastThroughDeath` (everything but
  `<stayAfterDeath>`, newly parsed onto `Skill` — case-insensitively, since the
  dist writes both `true` and `True`), unless **Noblesse Blessing** is up: then
  the blessing is stopped and the rest of the buff list survives. That blessing
  had no effect at all before — `NoblesseBless` wasn't in the parse table, so
  1323 cast and landed *nothing* (the whole-buff drop G19 describes for
  modifier-less effects); it is now a state-flag effect carrying
  `effect_flag::NOBLESS_BLESSING`, read at death off the same fold-on-read mask
  the CC gates use. Java's sibling exemption `RESURRECTION_SPECIAL` is a
  `TODO(G22)` — the self-resurrect effect isn't ported, so the flag has no
  source yet. Passive entries in `Buffs` (the grade-penalty stat pumps) are
  skipped: Java sweeps `EffectList._actives` only, and dropping those would
  silently unwind a passive on death.
- **Rewards**: `calculateRewards` from the aggro damage shares (solo-only —
  parties don't exist), `ALT_PARTY_RANGE`/surrounding-region gates,
  level-gap multiplier, ×`RateXp/RateSp`; `addExpAndSp` (SM 3259) with the
  `PlayableStat.addExp` level scan → `addLevel`: vitals re-derived, CP
  refill, autoGet skill grants (`rewardSkills`), `SocialAction` 2122 + SM 96
  + StatusUpdate/UserInfo/SkillList. Drops: `calculateDrops` port (level-gap
  gates, per-item chance/amount multipliers, occurrence cap — the cap's
  mid-list reshuffle simplified to a hard stop) **auto-looted** into the
  killer's inventory (SM 28/29/30 + InventoryUpdate) — the dist runs
  `AutoLoot = True`; ground drops wait for item-on-ground world objects.
  Runtime item ids come from DB-thread-reserved blocks
  (`DbEvent::IdBlock`/`DbCommand::ReserveIds` — Java `IdManager` semantics
  without a per-item round trip); new `InsertItem`/`UpdateItemCount`
  persistence.
- **Die → revive loop**: `RequestRestartPoint` (0x7D, TO_VILLAGE) → map
  region town respawn (`RespawnZone` override from `zones/respawn.xml` first —
  per-race target region, the layer that keeps Elven Ruins on Talking Island
  despite sharing Giran Harbour's coarse map tile — then the map-tile
  fallback) → `teleport_player` (`TeleportToLocation` 0x22 +
  `decayMe`-style DeleteObject) → client `Appearing` (0x3A) → `doRevive`
  (65% HP restore, `Revive` 0x01) + `spawnMe` visibility exchange + fresh
  UserInfo. Dead-on-login characters get their death dialog back
  (`EnterWorld` → `Die`).
- **Casting on NPCs**: `resolve_cast_target` resolves both registries
  (monsters are valid `Enemy` targets without ctrl), `MagicSkillUse` carries
  NPC target coords, NPC `mDef` through the `MDefenseFinalizer` shape; buffs
  on NPC targets are dropped (no NPC effect list — nothing casts on them
  yet).
- **Tests**: formula units with exact Java values; loader tests against the
  real dist (Gremlin `random`/`critical`, Goblin's 9 drop lines + 450 aggro
  range, Santa's `<corpseTime>3`, grouped drops, xp-lost + hit-condition
  tables, Giran map-region respawn); synthetic-world integration tests
  driving the real tick systems — the full melee kill
  (Attack/stance/Die/XP/level-up/adena auto-loot + DB insert/decay),
  out-of-reach chase + monster retaliation (run mode, `MoveToPawn`, HP bite
  with no CP soak), unprovoked aggro on an idle player, kill-by-nuke through
  the same death path, player death (penalty + to-village `Die`) →
  restart-point teleport → `Appearing` revive at 65%, and decay → respawn
  with a fresh id announced by `NpcInfo`.

### Post-G9 — ECS object storage (`bevy_ecs`) ✅
The world's object registries were refactored onto an **ECS
(Entity–Component–System)** backbone using the standalone `bevy_ecs` crate —
see [CONCURRENCY_MODEL.md §2.8](CONCURRENCY_MODEL.md) for the pattern
rationale (dense archetype-table iteration for the per-tick sweeps instead of
HashMap bucket walks).

- **`store.rs`** (new): `EntityStore<T>` — a `bevy_ecs::World` whose entities
  carry the game object as a component, an `object_id → Entity` index for
  O(1) id lookups, and a cached `QueryState` so `values_mut()` (the
  regen/movement/AI tick sweeps) is dense table iteration. Exposes the
  HashMap-shaped API the handlers were written against (`get`/`get_mut`/
  `insert`/`remove`/`values`/`values_mut`/`Index`/…), so call sites and the
  single-owner model are unchanged.
- **`World.players` / `World.npcs`**: `HashMap<i32, T>` → `EntityStore<T>`;
  `Player` and `Npc` derive `Component` (one fat component per entity —
  stage 1; component splitting + one merged world + `Schedule`-driven systems
  are the documented stage 2).
- **Tests**: `store::tests` (roundtrip + iteration); the whole existing suite
  runs against the ECS-backed stores unchanged.

### G9.5 — ECS stage 2: split components, one world ✅
Plan: [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md); executed in the planned
split-first/merge-second phases, each gated on the full (behavior-level)
test suite — no gameplay change.

- **Components** (`model/components.rs`), split along system access seams:
  shared `Position`, `RegionCell`, `Vitals` (HP/MP + `dead`), `Speeds`,
  `Collision`, `CombatStats`, `AttackState`; presence-based `Movement`/
  `Casting`/`Intent` (insert = state starts, remove = it ends — the
  movement tick sweeps only entities carrying `Movement` instead of
  scanning 34.9k static NPCs' `None`s, and the player combat tick sweeps
  only intent-holders); player-only `PlayerVitals` (CP), `BaseStats`,
  `StatModifiers`, `Buffs`, `Inventory`, `SkillBook`, `Reuses`, `TargetRef`,
  `ClientPos`; NPC-only `NpcAi`, `AggroList`.
- **One world** (`store.rs`): `World.players`/`World.npcs` →
  `World.objects: EntityStore` (non-generic) — one `bevy_ecs::World`, one
  id → `Entity` index (`npc_regions` unchanged). API:
  `spawn`/`despawn`/`get_component(_mut)`/`get_many_mut`/`has_component`/
  `add_components`/`remove_component`/`for_each_mut`/`count`. Object ids
  stay the only foreign key; `Entity` never leaves `store.rs`.
- **Residual cores as markers:** `Player`/`Npc` shrank to identity +
  bookkeeping nothing sweeps and double as the kind markers (the plan's
  separate `PlayerTag`/`NpcTag` were redundant). `combat::combatant()` is
  one component fetch for both kinds — NPC stats are memoized into
  `CombatStats` at spawn (`npc_combat_stats`, same finalizer math as the
  deleted per-call template derivation, m_def included for the magic path).
- **Movement unification:** one sweep advances every mover (player or NPC),
  returning moved-NPC ids for region re-indexing — the duplicated
  `tick`/`tick_npcs` pair is gone.
- **Boundary DTO:** `PlayerData` (né `PlayerBundle`) carries the full
  component set outside the ECS (from_char → `Entering` session →
  `spawn_into` at EnterWorld); `PlayerView` is its borrowed read-side for
  packet builders (UserInfo/CharInfo/CharSelected take one view arg, not
  eight components). Persistence (`PlayerSnapshot`) and NPC decay gather
  state from components *before* `despawn` — the old `remove() → whole
  struct` shape is gone.
- **Plan deviations:** kind markers folded into the residual cores (no
  zero-sized tags); `pair_mut` never materialized (no call site holds two
  entities' components mutably at once — the sequential re-fetch shape the
  handlers already had survived the merge); `SparseSet` storage fallback
  not needed. Known bevy quirk documented on `get_many_mut`: `Option<&C>`
  errors for never-registered `C` (probe with `has_component` instead).
- **Verified:** full suite green (147 tests incl. the real-socket
  `e2e_create` login→create→enter-world flow and the 34.9k-NPC dist spawn
  smoke test) after every phase; stage-3 (`Schedule` + ECS resources)
  logged in CONCURRENCY_MODEL §2.8 as an open question, default **no**.

### G9.6 — Macros & panel shortcuts ✅
Plan: [PLAN_MACROS_SHORTCUTS.md](PLAN_MACROS_SHORTCUTS.md). The shortcut bar
and server-stored macros, persisted per character. Macro *execution* is
client-side in the Java reference too — the server only stores and echoes.

- **Model** (`model/shortcut.rs` + `Shortcuts`/`Macros` components):
  `Shortcut`/`Macro`/`MacroCmd` + the `ShortcutType`/`MacroType`/
  `MacroUpdateType` enums (wire value = Java ordinal); registry logic as
  component methods (slot key `slot + page*12`, macro ids allocated from
  1000 skipping taken ones, insertion-ordered entries like Java's
  `LinkedHashMap`); the `type,d1,d2[,cmd];` DB `commands` codec with Java's
  tokenizer semantics (4th comma-token only, 255-char truncation) kept for
  round-trip parity.
- **DB** (`db.rs`): `character_shortcuts`/`character_macroses` load with the
  per-character select (like items/skills; `class_index` always 0); new
  fire-and-forget `UpsertShortcut`/`DeleteShortcut`/`UpsertMacro`/
  `DeleteMacro`; creation inserts the initial panel + macro presets,
  resolving ITEM entries item id → created object id on the DB thread.
- **Packets**: `ShortCutInit` (0x45, real per-type layouts — replaces the
  empty G4 stub), `ShortCutRegister` (0x44), `SendMacroList` (0xE8, one
  packet per macro with total count on enter world; ADD=1/MODIFY=2/DELETE=0
  echoes) — hand-computed byte tests (no client capture yet).
- **Handlers**: `RequestShortCutReg` 0x3D (page 0-19 gate, ITEM verified
  against the inventory + template shared-reuse-group; the
  `ShortCutRegister` echo and `SkillList` re-send are unconditional, a Java
  quirk kept), `RequestShortCutDel` 0x3F (deletion re-sends the whole
  `ShortCutInit` — there's no per-slot delete packet), `RequestMakeMacro`
  0xCD (Java's validation order: >255 command chars → SM 810, >48 macros →
  SM 797, empty name → SM 838, >32-char descr → SM 837),
  `RequestDeleteMacro` 0xCE (panel-slot cascade + DELETE echo).
- **Deliberate deviation — no recurring macros:** `RequestMakeMacro`
  rejects any macro containing a `SHORTCUT`-type command (SM 810 "Invalid
  macro"). That command ("press panel slot X") is the only way a macro can
  invoke another macro — the classic looping AFK macro, which Java happily
  registers. Blocking the command type outright is the airtight rule: slot
  contents can be rebound after registration, so checking what the slot
  holds is bypassable.
- **Hooks**: enter world sends the macro LIST burst before `ItemList` and
  the real `ShortCutInit` after it (Java's order); relog restore prunes
  ITEM shortcuts whose object id left the inventory (component + DB row);
  skill learn and level-up auto-grants rewrite matching SKILL slots
  (`updateShortCuts`: level bump + `ShortCutRegister` + row upsert).
- **New characters** (`data/initial_shortcut.rs`): `initialShortcuts.xml`
  port — global + per-class pages + macro presets (`enabled="false"`
  presets skipped, and MACRO slots referencing them dropped, so the stock
  example macro never lands). Mystic-class quirk: the class page's Self
  Heal shares slot 10 with the global Sit/Stand and overwrites it (Java
  map-put order) — a fresh Human Mystic panel is 5 slots, asserted in
  `e2e_create`.
- **Deferred**: pet/summon panels (`character_type` 2 is stored, nothing
  consumes it), RECIPE/BOOKMARK behavior (packet arms exist, nothing
  produces them), auto-soulshot deactivation on shortcut delete, the
  item-removal prune hook (no drop/trade/destroy exists yet — the
  restore-time prune covers stale rows meanwhile).
- **Tests**: codec/registry units; `initialShortcuts.xml` loader vs the
  real dist; packet byte tests; synthetic-world tests (register/delete
  round trip incl. DB commands, ITEM-verify reject, every
  `RequestMakeMacro` rejection incl. the SHORTCUT-command rule, delete
  cascade, skill-upgrade slot rewrite, `from_char` restore + stale-ITEM
  prune, enter-world packet order); `char_persistence::
  shortcuts_and_macros_persist` (real DB thread: creation panel + ITEM
  resolution, upserts/deletes, commands round-trip); `e2e_create` asserts
  the macro LIST packet + the 5-slot Mystic panel in the burst.

### G10 — Social systems ✅ vertical slice (chat + party + friends)
Plan: [PLAN_G10_SOCIAL.md](PLAN_G10_SOCIAL.md). Scoped to what two live
clients can exercise: chat, party, friends. **Clans deferred** (creation
only exists through village-master bypass dialogs — the G11 gate), with
mail/community board/matching rooms/command channels.

- **Chat** (`game_loop/chat.rs`): `Say2` (0x49) → `CreatureSay` (0x4A) with
  the `ChatType` enum. GENERAL = 1250-unit radius (region prefilter),
  SHOUT/TRADE = same map-region tile bucket (`GlobalChat/TradeChat = ON`
  semantics), WHISPER by name with the relation-mask tail (friend bit 0x01
  live, other bits await clans), PARTY via the party broadcast, CLAN/
  ALLIANCE answer SM 4202/4203. Guards: 105-char cap (SM 1078); malformed
  type/empty text **log-and-drop instead of Java's force disconnect**
  (deliberate deviation). Chat bans/jail/olympiad/block-list/say-filter/
  voiced commands/item links skipped with their systems.
- **Party** (`model/party.rs` + `game_loop/party.rs`): `World.parties`
  id-keyed map + `PartyRef` component back-pointer; one `PendingRequest`
  component slot covers Java's request map + `_activeRequester` for party
  *and* friend invites (30 s / 15 s seq-guarded `RequestTimeout` tasks).
  Full invite flow (`RequestJoinParty` 0x42 with the embryo-party shape —
  the Party exists from first invite, the leader binds on accept —
  `AskJoinParty`/`JoinParty`, busy/full/leader/pending guards),
  `PartySmallWindowAll/Add/Delete/DeleteAll` (0x4E–0x51), leave/oust with
  Java's disband rules (2 members left; leader-quit honors
  `AltLeavePartyLeader = True` on this dist; disconnect always transfers
  lead — SM 1384 + full window rebuild), `RequestChangePartyLeader`
  (D0:0x0C) slot swap, loot-rule voting (D0:0x75/0x76 →
  `ExAskModifyPartyLooting`/`ExSetPartyLooting` FE:C0/C1, unanimous-yes,
  15 s timeout), 12 s `PartyMemberPosition` (0xBA) self-rescheduling task
  (dies with the party via a seq bump), and `PartySmallWindowUpdate` (0x52
  — plain-short mask, **not** the reversed `masks.rs` scheme) piggybacked
  on every member vitals `StatusUpdate` (regen/damage/heal/MP consume;
  level-ups send the all-flags variant). Java's needCp/Hp/MpUpdate
  hysteresis dropped.
- **Party rewards** (`death.rs::calculate_rewards` party branch +
  `party::distribute_xp_and_sp`/`distribute_item`): members pool damage
  shares (alive + `AltPartyRange` of the corpse), level-gap multiplier at
  the top rewarded level, Java's fraction-squared `partyMul` quirk kept,
  `BONUS_EXP_SP` ladder × `RatePartyXp/Sp` (**70** on this dist) for 2+,
  level²-weighted split, all four `PartyXpCutoffMethod`s ported (dist runs
  `highfive`: gaps 0–9 → 100 %, 10–14 → 30 %, 15+ → 0). Auto-loot routes
  through `Party.distributeItem`: adena splits evenly in range; items go
  FINDERS_KEEPERS/RANDOM/BY_TURN (spoil variants inert — no spoil), with
  SM 299/300 "C1 has obtained" to the rest.
- **Friends** (`game_loop/friends.rs`): `character_friends` loads with the
  character (joined name/level/class snapshot → `Friends` component; new
  `InsertFriendPair`/`DeleteFriendPair` both-direction DB commands).
  Invite/answer (`FriendAddRequest` 0x83 → `FriendAddRequestResult` 0x55 +
  both lists/rows), delete by name from the snapshot (no global name cache
  needed — you can only delete someone on your list), SM-based
  `RequestFriendList`, `RequestSendFriendMsg` → `L2FriendSay` (0x78,
  receiver must have the *sender* friended). Enter world sends the real
  `L2FriendList` (0x75, replacing the G4-era empty 0x58 stub) + SM 503 and
  `FriendStatus(ONLINE)` (0x59) to online friends; leave world pings
  `FriendStatus(OFFLINE)`.
- **Config**: `AltPartyMaxMembers`/`AltLeavePartyLeader`/`PartyXpCutoff*`
  (Character.ini), `RatePartyXp/Sp` (Rates.ini). `GlobalChat`/`TradeChat`
  read as always-ON (dist value; OFF/GM variants unported).
- **Deferred**: clans/alliances (all clan chat answers "not in a clan"),
  mail, community board, party matching rooms & waiting list, command
  channels, tactical signs, block list, friend memos, `RelationChanged`
  packets (UserInfo/CharInfo re-broadcast stands in), pets in party
  windows, hero/petition chats.
- **Tests**: `model/party` units (bonus ladder, highfive gaps, cutoff
  methods); synthetic-world tests for chat scoping (1250 range, region
  bucket, whisper echo + offline SM 145, party-only chat), the invite/
  accept/decline/guards/timeout flows (packet shapes both sides), disband
  rules + leadership transfer on disconnect + oust + leader change, loot
  votes (accept + timeout), the 12 s position task lifecycle, vitals
  piggyback, party kill XP split with exact Java values, adena split +
  BY_TURN rotation skipping out-of-range members, friend invite/accept/
  delete/message round trips + login/logout notifications;
  `char_persistence::friendships_persist` (real DB thread); `e2e_create`
  now asserts the real `L2FriendList` in the burst.

### G11 — Scripting engine + quests + clans via bypass ✅ vertical slice
Plan: [PLAN_G11_QUESTS_CLANS.md](PLAN_G11_QUESTS_CLANS.md). The engine
slice of the script-breadth gate: bypass routing, a native quest framework
(compiled-in trait-object scripts), two completable quests, and clan
creation through the ClanMaster dialog. Script breadth is G12.

- **Bypass** (`game_loop/bypass.rs`): `RequestBypassToServer` 0x23 —
  `npc_<oid>_<cmd>` (existence + `INTERACTION_DISTANCE` + `ActionFailed`
  terminator) routed by first token (`Quest`, `create_clan` on
  `VillageMaster*` templates; rest log-drop); bare `Quest …` resolves its
  NPC via the new `LastFolkNpc` component (set on every NPC click —
  `validateHtmlAction` is deliberately unported, distance re-checks stand
  in). Empty bypass logs instead of Java's disconnect.
- **Quest framework**: `model/quest.rs` (`QuestState`, the
  `__compltdStateFlags` skipped-step math as a pure function, legacy
  bit-31 `condBitSet` unpack) + `Quests`/`QuestTimerSeqs` components;
  `game_loop/quests.rs` — `QuestScript` trait + `QuestRegistry` (per-npc
  start/talk/kill indexes) behind `World.quests: Arc<…>` (the `geo`
  borrow pattern), `QuestCtx` porting the `QuestState`/`AbstractScript`
  primitives (start/cond/exit, give/reward/take items, `giveItemRandomly`
  with ×`RateQuestDrop`, rated adena/XP/SP), QuestLink's chooser/talk/
  event split, `showResult`'s `.htm`-quest-window vs `.html`-plain split
  (`ExNpcQuestHtmlMessage` FE:0x8E vs `NpcHtmlMessage`), `onKill` fired
  from `npc_do_die` after combat rewards (killer-only — party sharing
  deferred), `RequestQuestAbort` 0x63, and seq-guarded
  `ScheduledTask::QuestTimer`.
- **Persistence**: `character_quests` row-per-var, Java-schema-compatible
  (`<state>` as `Start/Started/Completed`); `load_quests` (orphan vars
  dropped) + fire-and-forget `UpsertQuestVar`/`DeleteQuestVar`/
  `DeleteQuest{keep_state}`.
- **Packets/items**: real `QuestList` (one-time mask incl. Java's
  id-range exclusions) and `ExQuestItemList` replace the G4 stubs;
  `ExShowQuestMark`, `PlaySound`; **first item-removal path** —
  `Inventory::remove_item` → `ItemChange`s → removed-type
  `InventoryUpdate` + `DbCommand::DeleteItem`; `Player.addItem`'s
  stack-or-create core extracted to `items::add_inventory_item` (shared
  with G9 loot). SM 52/53/54 "earned" trio for quest gives.
- **Scripts** (`src/scripts/`, `build_registry()` = the boot-time script
  pass): `Q00258_BringWolfPelts` (deterministic drop, reward table),
  `Q00320_BonesTellTheFuture` (0.18-chance drop ×`RateQuestDrop`, rated
  adena), `ClanMaster` (60 NPC ids, `LEADER_REQUIRED` → `-no.htm` remap;
  Clan Advent buff unported). Quest htmls read from the dist tree with
  the `quests/<Name>/` fallback and `noquest.htm` default.
- **Clans** (`model/clan.rs` + `game_loop/clans.rs`): `World.clans`
  loaded at boot (unprompted `DbEvent::ClansLoaded`, `IdBlock` pattern);
  `create_clan` with Java's guard order (SM 229/190/230/261/262/5), clan
  id from the shared `IdManager` pool, `InsertClan` + `UpdateCharClan`
  persistence, `PledgeShowInfoUpdate`/`PledgeShowMemberListAll`/
  `PledgeShowMemberListUpdate` + SM 189 + UserInfo/CharInfo re-broadcast.
  `Player` grew `clan_id`/`clan_privs`/`clan_leader` (fixed up at
  enter-world)/`clan_create_expiry_time`; clan id real in UserInfo CLAN
  block, CharInfo, CharSelectionInfo, CharSelected; clan chat now
  broadcasts to online members; enter/leave world send the roster window
  and online/offline pings. The clan-window clan-entry queries are
  answered (ex 0xD3 `RequestPledgeRecruitInfo` → `ExPledgeRecruitInfo`
  with an empty sub-pledge list, ex 0xDE `RequestPledgeRecruitApplyInfo`
  → always-DEFAULT `ExPledgeRecruitApplyInfo`, ex 0xD8
  `RequestPledgeWaitingApplied` consumed silently, ex 0xD4
  `RequestPledgeRecruitBoardSearch` → empty-board
  `ExPledgeRecruitBoardSearch` page, ex 0xDC
  `RequestPledgeDraftListSearch` → empty-list
  `ExPledgeDraftListSearch`) — the registration side
  (`ClanEntryManager`, board search/apply/waiting/draft lists) is
  G18's recruitment audit addition.
- **Tests**: cond-flags/bit-unpack units; `char_persistence::
  quest_states_persist`; synthetic-world tests for bypass routing, the
  full Q00258 loop (accept → drops → cond mark → turn-in → repeatable
  re-offer, packet+DB assertions), Q00320's forced-roll chance path and
  rated adena, abort, a synthetic-script quest timer (fire/cancel), the
  clan guard matrix + creation packet trio + persistence, ClanMaster
  leader gating against the real dist htmls, and roster/chat scoping.

### G12 — Static world + script/content breadth ✅ vertical slice
Plan: [PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md](PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md).
Both plan areas landed as vertical slices; the long tail (33 more zone
types, multisell/sell/warehouse, ~188 more quests, ~81 `ai/` scripts) stays
G14; admin commands are carved out as their own G13
([PLAN_G13_ADMIN.md](PLAN_G13_ADMIN.md)).

**Zones** (`data/zone_data.rs`, `game_loop/zones.rs`):
- `ZoneManager` port narrowed to the three files with live consumers —
  `peace.xml`/`water.xml`/`no_restart.xml` (590 zones), reusing the spawn
  territories' `ZoneForm` geometry, indexed into Java's `SHIFT_BY = 15`
  zone-grid cells (bounding-box overlap registration, point query walks
  the cell's zones).
- `ZoneFlags` component (mask + `_lastZoneValidateLocation` 100-unit filter
  + `_lastCompassZone`), revalidated from the movement tick, enter world,
  teleports (`Appearing`) and the `ValidatePosition` snap — Java's
  `revalidateZone` call graph. `ExSetCompassZoneCode` (FE:0x33) pushes the
  peace icon on change (deviation: the initial no-op GENERAL push is
  suppressed — a fresh client already displays general).
- **Peace gate** where Java actually has it (playable-vs-playable only):
  `resolve_cast_target`'s `Enemy`/`EnemyOnly` arm → SM 2167 after the LOS
  check, and `Self.java`'s bad-self-skill branch. Auto-attack needs no gate
  (player targets aren't attackable until PvP exists).
- **Water**: `Speeds.swimming` flips on enter/exit (`getMoveSpeed`'s swim
  branch) + `broadcastUserInfo`; breath/drowning deferred. NO_RESTART only
  tracks membership — nothing reads the flag in this Mobius version.

**Doors** (`data/door_data.rs`, `geo/doors.rs`, `model/door.rs`,
`game_loop/doors.rs`):
- All **1180** `DoorData.xml` doors parse (Java's flattened child-attribute
  StatSet) and spawn as ECS entities; `masterClose`/`isWall` and the unused
  group/child/emitter machinery are not carried.
- Collision is Java's real shape — **doors don't carve geodata**: a
  `DoorGrid` inside `GeoEngine` (registered before the `Arc` is shared, so
  the path worker sees it; open flags are atomics) runs the
  `checkIfDoorsBetween` segment-vs-polygon test at the head of
  `can_see_target` (double-face), `get_valid_location` and
  `can_move_to_target` — closed doors block LOS, movement and pathfinding.
- `StaticObjectInfo` (0x9F) + `DoorStatusUpdate` (0x4D) render doors on
  enter world/region cross; `open_door`/`close_door` broadcast state flips,
  with the auto-close task (seq-guarded) and the BY_TIME cycle
  (`startTimerOpen`/`TimerOpen` verbatim, 111 doors self-toggling). BY_CLICK
  is intentionally inert — `isOpenableByClick` has no consumer in this
  Mobius version either (clan-hall dialogs are its only route).
- **Static objects**: 86 of the 159 `StaticObjects.xml` entries (73 are
  commented out) spawn and render via `StaticObjectInfo`; click behavior
  (town map, thrones) is gated on community board/castles.

**Bypasses/shop** (`game_loop/bypass.rs`, `game_loop/shop.rs`,
`data/buy_list_data.rs`, `network/trade.rs`):
- `Link <file>`: `Link.java`'s whitelist (23 pages) served from
  `data/html/` as plain `NpcHtmlMessage`; `..`-escapes dropped.
- `Chat <page>` (`ChatLink.java` → `Npc.showChatWindow(player, value)`): the
  follow-up dialog pages (`<npcId>-<page>.htm` in the instance class's html
  dir). Without it every "next page" button on a folk html was a log-drop —
  notably the merchant landing pages, which reach `Buy` only through
  `Chat 1`, so no shop behind a Lector-style two-step menu was openable. The
  `showPkDenyChatWindow` reputation gate and the `ON_NPC_FIRST_TALK` redirect
  on page 0 are still `TODO(G23)`.
- `Buy <listId>` on `Merchant`/`Fisherman` templates →
  `Merchant.showBuyWindow`: all **338** buylists load (file name = list id,
  `CorrectPrices = True` floors prices to sell value at load; limited stock
  treated as unlimited — 3 lists), `BuyList` + `ExBuySellList` (FE:0xB8 both)
  with the shared `AbstractItemPacket` item block, and `RequestBuyItem`
  (0x40) with Java's validation ladder (off-list/unstackable-quantity/
  MAX_ADENA/adena shortfall) → charge, deliver, `ExUserInfoInvenWeight` +
  sell-refresh + SM 4358. Weight/slot capacity gates wait for encumbrance;
  Sell/multisell deferred. `ItemTemplate` grew the reference `price`.

**Quest/script breadth** (`game_loop/quests.rs`, `src/scripts/`):
- `QuestScript` grew `on_attack`/`attack_npcs` (fired from
  `npc_receive_damage`, killing blow included) and `on_spawn`/`spawn_npcs`
  (fired from `spawn_one` — boot pass and respawns; no player in the ctx),
  plus `Npc.script_value` (Java's per-instance scratch, reset by respawn),
  `NpcSay` (0x30), and ctx primitives: category checks
  (`data/category_data.rs` — full `CategoryData.xml`), `set_class_id`
  (immediate `StorePlayer` + `broadcastUserInfo`), `teleport_to`,
  `already_completed_html`.
- **+10 quests** picked for shape variety: Q00303/Q00313 (single-kill
  collect), Q00260/Q00263/Q00265/Q00273 (multi-kill-target with per-monster
  drop tables), Q00317 (uncapped drops, pay-out-and-continue turn-in),
  Q00324 (10th-item cond bump), **Q00316** (the `on_attack` consumer —
  Varool Foulclaw's one-shot NpcSay via script value + his one-only fang),
  **Q00109** (multi-step cond 1→2→3 across three NPCs, **one-time** —
  first COMPLETED-state quest, already-completed page included).
- **OrcChange1** (village master #2): the full first-transfer matrix
  (category gates, proof marks, level 20, 15 shadow coupons, class change
  persisted immediately) through the dist htmls' `Quest OrcChange1 <event>`
  bypasses.
- **TeleportWithCharm** (first `ai/others` script): token-consuming
  teleport, registered through the same `QuestRegistry` — resolved plan
  question #1: utility scripts fit the existing registry; a new opt-in
  `bare_talk()` routes their `on_talk` from the bare `Quest` bypass
  (deviation: this Mobius build's chooser short-circuit leaves such
  scripts unreachable even though the dist htmls point at that button).
- Resolved plan question #4: ClanMaster keeps its ad hoc page loading —
  retrofitting onto `Link` risked the working G11 gate for no visible gain.
- **Tests**: zone loader/grid units + peace/water/filter world tests; door
  grid + engine-level geo units, enter-world door burst, LOS-until-opened,
  auto-close staleness, BY_TIME cycling; static-object loader/burst; Link
  whitelist round trip; buylist loader vs dist (CorrectPrices floor
  verified globally), Buy window + purchase/guards; per-shape quest loops
  (Q00303, Q00316 incl. the shout + fang cap, Q00109 incl. the completed
  mask), OrcChange1 transfer + category refusal, TeleportWithCharm, and a
  synthetic `on_spawn` script. `e2e_create` runs against the full boot
  (zones + doors + statics + 15 scripts); its skip-unsolicited helper now
  also skips the compass code (the mage-start spawn lies in a peace zone).

**Post-G12 fixes:**
- **`AutoLearnSkills` config now honored** (`config/character.rs`,
  `data/skill_tree.rs`, `game_loop/death.rs`, `game_loop/lobby.rs`): the port
  ignored `Character.ini`'s `AutoLearnSkills = True`, so players only ever got
  autoGet skills. `Player.rewardSkills` now branches on the flag — with it on,
  `SkillTreeData.all_available_skills` (highest reachable level per class skill)
  grants every reachable class skill on both enter-world and level-up, with the
  `ShortCutInit` + "learned N skills" (`SystemMessageId.S1_2`) notice.
  `SkillTreeData` now loads all four class-tier directories (`StartingClass` /
  `1st` / `2nd` / `3rdClass`) plus the common `Commons.xml` tree, and
  `complete_entries` walks the `parentClassId` chain (Java
  `getCompleteClassSkillTree`) so advanced classes reach their ancestor + common
  skills — `//setclass` to a 2nd/3rd class now recalculates the skill set. The
  auto-learn path honors `AutoLearnSkillsWithoutItems` and
  `AutoLearnDivineInspiration` (`requires_item` flag from the `<item>` child).
  FS / removeSkills paths stay out of scope (absent from the trees); parsing the
  `<item>` id/count for the manual-learn cost display + consumption is
  TODO(G6). Unit + level-up/enter-world/setclass grant tests.

### G13 — Admin / GM command system 🚧 (framework landed)
Plan: [PLAN_G13_ADMIN.md](PLAN_G13_ADMIN.md). **G13.A (the framework) is done**;
command bodies (G13.B) are next.

- **Access data** (`data/admin_data.rs`): ports `AccessLevel` +
  `AdminCommandAccessRight` + `AdminData`, loading `config/AccessLevels.xml`
  (10 tiers, Banned −1 … Master 100) and `config/AdminCommands.xml` (458
  rights) into `GameData.admin`. Faithful `has_access` (exact match or the
  `childAccess` chain walk), `require_confirm`, and the undefined-command
  master auto-grant. Negatives collapse to Banned; a miss returns the level-0
  User fallback.
- **Player state**: `Player.access_level` (from `characters.accesslevel` via
  `from_char`), `Player::is_gm` / `access_level_def`, and name/title colors
  resolved from the tier (Java `setAccessLevel` → `_appearance`). A level-0
  player keeps the client-default colors so the real UserInfo capture still
  matches — the datapack `User` row's `ECF9A2` title is a Mobius quirk the
  retail client doesn't send.
- **Dispatch** (`game_loop/admin.rs`): `SendBypassBuildCmd` (0x74, the
  `//command` bar) and the `admin_` `RequestBypassToServer` branch both reach
  `use_admin_command` → `isGM` gate → known-command check → `has_access` →
  optional confirm → run. A gated-but-unported command (G13.C) answers a
  not-implemented line instead of crashing. GMAudit is a log line.
- **Confirm round-trip**: `ConfirmDlg` (0xF3, distinct wire format) + a pending
  command on the `InGame` session + `DlgAnswer` (0xC6); `confirmDlg="true"`
  commands prompt and only run on "yes".
- **Commands (G13.B, ~220 portable handlers landed)** — each drives live game
  state through the existing systems, no new bypasses. Grouped by the handler
  family they port (`game_loop/admin/*`):
  - **B1 character/skill** (`character`/`editchar`/`skills`/`vitals`):
    `//heal`, `//res`(+`//res_monster`, name/radius forms), `//kill`
    (+`//kill_monster`, name/radius forms), `//add_exp_sp`/`//remove_exp_sp`/
    `//add_exp_sp_to_character`, `//add_level`/`//set_level`, the 8 `//set*`
    field setters + `//settitle`/`//setcolor`/`//setsex`/`//setclass`,
    `//set_hp`/`//set_mp`/`//set_cp`, the 15 per-slot enchant `//set*`,
    `//add_skill`/`//remove_skill`/`//setskill`/`//give_all_skills`(`_fs`)/
    `//remove_all_skills`/`//reset_skills`/`//get_skills`/`//cast`(`now`)/skill
    HTML menus, `//buff`/`//getbuffs`(`_ps`)/`//stopbuff`/`//stopallbuffs`/
    `//areacancel`/`//removereuse`, `//invul`/`//undying`/`//hide`.
  - **EditChar breadth**: `//current_player`/`//character_info`/
    `//character_list`/`//show_characters`/`//find_character`/`//find_account`/
    `//edit_character`/`//changename`/`//set_pvp_flag`/`//partyinfo`/
    `//remove_clan_penalty`.
  - **B2 items** (`items`): `//create_item`/`//give_item_target`/
    `//give_item_to_all`/`//create_coin`/`//itemcreate`/`//enchant` menus,
    `//destroy_items`/`//destroy_all_items` (+`destroyitems`/`destroyallitems`).
  - **B3 spawns** (`spawn`): `//spawn`/`//spawn_monster`/`//spawn_once`/
    `//spawnat`, spawn+npc HTML menus, `//list_spawns`/`//list_positions`/
    `//top_spawn_count`/`//spawn_debug_print`/`//scan`, `//summon`, `//delete`.
  - **B4 movement** (`teleport`): `//teleport`/`//recall`/`//teleto`,
    directional `//go*`, `//walk`/`//sendhome`/`//teleport_character`/
    `//recall_npc`, teleport HTML menus, `//gmspeed`/`//superhaste`/`//speed`.
  - **B5 GM utility & comms** (`gm_util`/`moderation`/`menu`): `//serverinfo`,
    `//gmchat`/`//announce`/`//announce_crit`/`//announce_screen`/`//worldchat`,
    `//target`/`//changelvl`/`//gm`/`//gmliston`/`//gmlistoff`/`//diet`/
    `//online`/`//targetsay`/`//msg`/`//kick`/`//kick_non_gm`/
    `//character_disconnect`, `//html`/`//loadhtml`/`//showdoors`/`//debug`/
    `//stats`, the `//admin` menu + AdminMenu action buttons (goto/recall
    char/party/clan, kick/kill menu).
  - **B6 world** (`world_cmds`): `//open`/`//close`/`//openall`/`//closeall`,
    `//zones`/`//zone_check`, `//buy`/`//gmshop`, `//clan_info`, and the
    read-only geo queries `//geo_pos`/`//geo_spawn_pos`/`//geo_can_move`/
    `//geo_can_see`.
  - **B7 player-vars** (`character`): `//set_vitality`/`//full_vitality`/
    `//empty_vitality`/`//get_vitality`.
  - **AdminEffects (broadcast subset)**: `//social`, `//effect`/
    `//npc_use_skill`, `//earthquake`, `//atmosphere`, `//play_sound`.
  - New infra: `remove_exp_and_sp`, an NPC-decay `!dead` revive guard
    (`//res_monster`), `creatures_in_range` (radius commands),
    `SkillData::max_level`, plus the earlier `spawn_npc_at`, `SetAccessLevel`
    DB command, and `AdminFlags`.
- **Mounts** (`admin/mounts.rs`): `//ride_strider`/`//ride_wolf`/`//ride_wyvern`
  + `//unride*`. `Player.mount_type`/`mount_npc_id` are durable state serialized
  into UserInfo/CharInfo (mount byte identical to the old hardcoded 0 when
  unmounted — the real-capture byte test still passes) plus a `Ride` (0x8C)
  broadcast. Mount speed/collision swap is a documented TODO (needs mount stat
  data); the visual mount is complete.
- **Transforms** (`data/transform_data.rs` + `admin/transforms.rs`): a
  `TransformData` loader (174 `data/stats/transformations/*.xml`) →
  `Player.transform_id`/`transform_display_id`, serialized into CharInfo
  (transform display id, identical to the old hardcoded 0 when untransformed —
  byte test green) and the self-view abnormal-visual packet. `recalculate_stats`
  overrides run/walk from the template's `<moving>`; collision + the template's
  transform skills are applied/reverted. Commands: `//transform`/`//untransform`
  + `AdminRide`'s transform-based `//ride_horse` (106) / `//ride_bike` (20001),
  with `//unride*` routing to dismount-or-untransform. Base-stat/action-list/
  additional-item overrides are a documented TODO (model + speed + collision +
  skills are complete).
- **Mob groups** (`model/mob_group.rs` + `admin/mobgroup.rs`): the full
  `AdminMobGroup` set (17 cmds) — a `MobGroupTable` (`World.mob_groups`) of
  groups whose members are runtime-spawned NPCs tagged with a `Controllable`
  component and steered by the group's `MobGroupState`
  (idle/no-move/random/attack/attack-group/follow/return/cast). The
  `controllable_think` branch in `npc_ai` reuses the wild AI's scan/attack/chase
  and a plain walk for follow/return rather than a parallel AI tree. Lifecycle
  (create/spawn/unspawn/kill/remove/teleport/list/menu) + invul + the state
  setters all land; the deeper `ControllableMobAI` nuances (formation offsets,
  skill selection for cast) are simplified.
- **Geodata editor** (`admin/world_cmds.rs` + `geo`): `//geomap`/`//geocell`
  (tile + cell/Z report), and runtime NSWE editing — `//geoenable*`/
  `//geodisable*` set/clear a passability bit on the GM's nearest cell through a
  `GeoEngine` override map (`RwLock<HashMap>` gated by an `AtomicBool` so the
  pathfinding hot path is one relaxed load when nothing is edited); edits apply
  immediately to movement/pathfinding. `//geosave*` reports the pending edit
  count (the L2 binary region serializer isn't ported — edits are in-memory);
  `//geoedit`/`//geogrid` client-viz stays a stub (no `ExServerPrimitive`
  overlay).
- Tests: 5 `admin_data` units + 74 synthetic-world dispatch/handler tests
  (gating, confirm round-trip, colors, one+ per handler group, mount +
  transform round-trips, mob-group lifecycle) + a geo NSWE-override unit test.
- **Deferred**: only the `//geosave` binary-region serializer + the geo
  client-viz overlay remain simplified. Still blocked: clan-skill grants (no
  clan-skill system), `AdminFence` (no spawnable fence), the AdminEffects
  **abnormal-visual-effect / team / targetable** subset, `//setnoble`/`//rec`/
  premium/prime/pc-cafe (fields not modelled), and the IP/dualbox tools (no
  per-client IP). **G13.C** (sieges/olympiad/instances/events/petitions/
  punishment/…) stays gated-but-bodiless.

---

## Deferred TODOs (by system)

Empty/placeholder now, to be filled in the owning milestone:

- **Inventory/items (post-G5):** warehouse/clan warehouse/freight/mail,
  trade, pickup/drop, item actions (`RequestActionUse` beyond equip),
  crystallization, enchanting, augmentation, elemental attributes,
  `ExQuestItemList` (no quest items exist yet), real `maxLoad` calc +
  encumbrance enforcement, `ItemList`/`ExUserInfoEquipSlot` visual-id block.
  Also blocks full P.Def/P.Atk/M.Def/M.Atk accuracy (see G6: naked-value only
  until item `<stats>` are parsed). `UseItem`'s `EtcItem` branch dispatches
  through a typed `ItemHandler` (`data/item_data.rs`); `ExtractableItems`
  (pack/box unpacking, e.g. "Mage Class Equipment Set") and `ItemSkills`/
  `ItemSkillsTemplate` (potions/buff scrolls — casts the item's `<skills>`
  list immediately via the existing skill-effect pipeline, `Heal`/
  `MagicalAttack`/`StatModifier` only since that's all `EFFECT_REGISTRY`
  covers so far; reuse shared with `game_loop::skills::cast::{check,set}
  _skill_reuse`, also extracted for `use_magic_on`) are ported — the
  `SoulShots`/`SpiritShot`/`BlessedSpiritShot` handlers are ported too (charge
  on manual use + auto-use toggle via `RequestAutoSoulShot`/`ExAutoSoulShot`,
  grade check, `rechargeShots` before attack/cast, melee ×2 / magic ×2/×4 /
  heal static bonus, consume-on-hit/cast). Dyes/enchant scrolls and the rest
  of Java's `handlers/itemhandlers/*` are still no-ops
  (`game_loop/items.rs::use_etc_item`'s `ItemHandler::None` arm), as is
  `<cond>`-gating and the `itemConsumeId`/`SKILL_REDUCE_ON_SKILL_SUCCESS`
  non-consume case (every `ItemSkills` use is treated as consume-on-success).
  Not ported: NPC/summon soulshots, the `reducedSoulshot` weapon perk, and the
  ruby/sapphire brooch visual swap (no jewels).
- **Skills/combat (post-G9):** `PhysicalAttack`-type *skills* (auto-attack
  damage is done; skill-based physical hits reuse `apply_physical_damage`);
  bows/crossbows (reuse gauge, arrows), dual-weapon split hits, polearm
  sweeps, the `SHOTS_BONUS` stat itself (soulshots/spiritshots are ported —
  see the items note above — but that dynamic-bonus stat stays 1.0), shield
  defence (`calcShldUse` — needs item `<stats>` parsing), PvP auto-attack
  (needs PvP flags/karma); AoE
  affect scopes (only `SINGLE` resolves); ~~`ALT_GAME_MAGICFAILURES`
  magic-resist rolls (`calcMagicSuccess`)~~ (done — see the magic-failure entry
  above); ~~queued skills +
  walk-into-cast-range AI~~ (both done: `QueuedAction` slot + `PlayerIntent::Cast`
  chase — an out-of-range cast walks into cast range then casts at the
  snapshotted target, shift-click = `dontMove` → SM 748; ground-target
  `maybeMoveToPosition` still waits on GROUND targeting);
  the other 8 `AcquireSkillType`s (PLEDGE,
  TRANSFORM, TRANSFER, SUBCLASS, …); toggle-type skills; skill mastery +
  `MAGIC_REUSE_RATE`; skill reuse-delay persistence across relog;
  `ExAbnormalStatusUpdateFromTarget` (broadcast to other players); most of
  the 230-entry `Stat` enum and 369 effect classes (grow `EFFECT_REGISTRY`/
  `SkillEffect` as needed); overhit XP bonus; buffs/effects on NPC targets
  (no NPC effect list). ~~offensive-skill aggro on NPCs~~ (✅ — `callSkill`'s
  post-`activateSkill` loop now runs `addDamageHate(caster, 0, -effectPoint)` +
  `notifyEvent(EVT_ATTACKED)` for any bad skill on an attackable, in
  `handle_skill_finish`'s `is_bad` block — **independent of whether the effects
  landed**, so a *resisted* or pure debuff still wakes the mob and makes it
  retaliate; the wake previously only fired from the damage/spoil effect
  handlers, so a non-landing debuff drew no aggro. Java skips this when the
  skill `hasEffectType(HATE)` — no HATE effect is modeled yet, tracked by a
  `TODO(G16)` at the site).
- **Movement/targeting (post-G7.8):** NPC pathfinding (player moves path
  via the G7.85 worker; NPC chase/return-home moves are still straight-line,
  and the Attackable closest-reachable-point grid scan is unported);
  ~~zones~~/~~door LOS+movement checks~~ (✅ G12 — peace/water/no-restart
  zones and all 1180 doors; the other 33 zone types, fence checks, and
  `ValidatePosition`'s door-exploit tail remain); the rest of
  `isMovementDisabled()`
  (rooted/overloaded/immobilized/dead/teleporting); cursor-key movement
  (`_cursorKeyMovement` path incl. `canMoveToTarget` front-cell check and
  `getLastServerPosition` stop); falling damage/state (`isFalling`).
- **NPCs/world content (post-G9):** guard aggro (needs karma), clan/faction
  help calls (`<clanList>` unparsed),
  minions, raid/grand-boss behaviours (chaos target swaps, raid curse,
  raid points); NPC skill casting (`AISkillScope` lists unparsed) + NPC
  buffs/effect list; NPC regen; ground drops + pickup (`AutoLoot = False`
  path — needs item world objects; herbs likewise), spoil/sweep; party XP
  split + overhit; Java's teleport-home on attack timeout (we walk);
  elemental attributes (template parse skips them); `dbSave` raid
  persistence (`DBSpawnManager` — spawned statically at full HP);
  `HtmCache` *caching* (dialog `.htm`s are still read per interaction, but
  every read now goes through `data::htm_cache::read_htm`, which applies
  `HtmCache.loadFile`'s comment/tab/newline stripping — without it the client
  rendered a literal `-->` for each commented-out block, e.g. the Newbie
  Guide at `html/default/31076.htm`; 187 dist htmls ship comments);
  ~~zones/doors/`StaticObjectData`~~ (✅ G12 vertical slice);
  `NpcNameLocalisationData`/multilang; the death
  dialog's non-village restart points (clan hall/castle/fixed-feather).
- **Quests/scripts (post-G11/G12):** party quest sharing
  (`getRandomPartyMemberState` — kill credit is killer-only); daily quests
  (`restartTime`/reset hour); ~~`onFirstTalk` hook~~ (✅ — see below;
  ~~onAttack/onSpawn~~ ✅ G12); tutorial (Q00255);
  `ExQuestNpcLogList`; the quest-window weight/inventory-90%/40-quest
  guards; the chooser's simulated-`onTalk` pre-filter; `validateHtmlAction`
  (bare bypasses resolve via `LastFolkNpc` + distance); the remaining ~188
  quests, ~14 village-master scripts and ~81 `ai/` scripts; other bypass
  families (~~`Link`~~/~~`Buy`~~ ✅ G12; `multisell`, sell,
  `learn_clan_skills`, `item_`, `admin_`, `_bbs`, menu/manor selects).
- **Social (post-G10/G11):** clans past creation (invite/leave/dissolve/
  level-up/wars/ally/academy/sub-pledges, clan skills +
  `PledgeSkillList`, crests, notices, warehouse, `PledgeInfo`/
  `PledgeStatusChanged` beyond the creation trio, the Clan Advent buff,
  RELATION bits / `RelationChanged` — the full UserInfo/CharInfo re-send
  stands in); ally chat; mail; community board; party matching rooms;
  command channels (MPCC); tactical signs; block list (`BlockList` checks
  skipped everywhere); friend memos + `RequestExFriendListExtended`;
  pet/servitor party-window packets; chat bans/say filter/voiced
  commands/item links in chat; `GlobalChat`/`TradeChat` OFF/GM modes;
  skill/reuse persistence for party-relevant buffs unchanged (see skills
  section).
- **Misc:** ~~macros~~ (✅ G9.6), `HennaInfo` empty, `ExUserBanInfo`, `ExVitalityEffectInfo`
  bonuses, real castle list for manor, game-time clock (CharSelected/UserInfo
  use 0), periodic auto-save while in game (`AutoSaveManager`; persistence on
  restart/logout/disconnect is done).

---

## Tests / verification

- **Crypto:** golden vectors (`commons/tests`, `gameserver` cipher).
- **Protocol parity:** GS↔LS packet cross-checks (loginserver as gameserver
  dev-dep), `AuthRequest`/`BlowFishKey`/`PlayerAuthRequest` layouts.
- **DB:** `char_persistence.rs` — create/load/delete/restore against the stock
  schema.
- **Full E2E:** `e2e_create.rs` — real two-server login→create→enter-world with a
  scripted client; drains the enter-world burst; checks computed HP/MP and
  (G5) that the Human Mystic's starting wand shows up equipped in `ItemList`/
  `ExUserInfoEquipSlot`.
- **UserInfo bytes:** unit test against a real client capture.
- **Inventory:** `model::inventory::tests` — item/equipment loaders load real
  `dist/game` data; `equip_item` slot-conflict cases (full armor vs
  chest+legs, two-handed vs dual single-hand, ear/finger fill order).
- **NPCs (G8):** loader counts + hand-checked templates against the real
  dist; `spawn_all` placement/coordinate/region-index smoke test; `NpcInfo`
  hand-computed byte test; synthetic-world visibility & two-click
  interaction tests.
- **Social (G10):** chat/party/friend synthetic-world tests (see the G10
  section), party-math units with exact Java values, friendship DB
  round-trip.
- **Quests/clans (G11):** cond-flags math units vs hand-traced Java
  values; `character_quests` DB round-trip; synthetic-world tests for the
  full quest loops (Q00258/Q00320 with forced rolls), bypass routing,
  abort, quest timers, the clan guard matrix/creation flow, ClanMaster
  dialog gating vs the real dist htmls, and clan roster/chat scoping.
- **Combat (G9):** physical-formula units with exact Java values; drop/
  corpse/aggro template assertions against the real dist; synthetic-world
  integration tests over the real tick systems — melee kill (rewards,
  level-up, auto-loot, decay), chase + retaliation, unprovoked aggro,
  kill-by-nuke, player death → to-village revive, decay → respawn.
- **Community board (G30):** config load vs the dist `General.ini`/
  `Custom/CommunityBoard.ini` + the gatekeeper-html teleport-whitelist scan;
  `ShowBoard` chunker units (101/102/103 split, the empty-chunk `null`
  sentinel); `SchemeBufferSkills.xml` available-buff loader; synthetic-world
  tests over the real dist htmls — the board button opens the custom home with
  the navigation injected, the offline gate sends the SystemMessage, `_bbsheal`
  restores vitals (and is refused when the player can't pay), `_bbsteleport`
  moves to a whitelisted destination and hides the board while an unlisted
  destination is refused. **Premium buy** (`_bbspremium`) grants account
  premium (reusing the `//premium_*` store), refuses out-of-range days /
  insufficient currency, and serves the thank-you page. **Scheme buffer**
  (`_bbs_buff_scheme_*`) snapshots the player's active whitelisted buffs into a
  named scheme (max 5, alphanumeric ≤14), write-throughs to `buffer_schemes`,
  renders the execute/pet/delete rows, deletes, and reports the no-pet /
  no-buffs / cap errors.
    **Merchant multisell** (`_bbsmultisell` / `_bbsexcmultisell`) opens the
    exchange window and the `MultiSellChoose` click swaps adena/items for the
    product — see the multisell subsystem below.
  - **Deferred (`TODO(G30)`):** `_bbssell` (the sell window needs buylist 423,
    absent on this dist — the command is also unreachable from the shipped
    htmls); `_bbsdelevel` (config-off in the dist); the retail forum boards
    (unreachable under the custom nav). Scheme execute onto pets/servitors is
    `TODO(G29)` (no summons yet).
- **Multisell (G30):** `MultisellData` loads every `data/multisell/*` list
  (plus the `custom/` overlay — the `6000xx` CB shop lists) keyed by file name;
  `separateAndSend` (the npc-less community-board path) pages the `MultiSellList`
  (0xD0) window and records the open list on the player (`ActiveMultisell`
  component); `MultiSellChoose` (0xB0) validates the open list / entry / amount,
  checks and takes the (summed) ingredients, grants the products with the
  acquisition SystemMessage + `ExMultiSellResult`, and sends one batched
  `InventoryUpdate`. Synthetic-world tests over the real dist lists cover the
  window open, a successful adena→item exchange, the ingredient-shortfall
  refusal, and the stale-list drop. **Not ported (`TODO(G30)`, none reached by
  the CB lists):** inventory-only exchange (`_bbsexcmultisell` opens the full
  list), chance multisells, `maintainEnchantment`/enchanted ingredients,
  `SpecialItemType` (clan reputation / fame / raid / PC café) ingredients &
  products, castle tax, and the weight/slot capacity gates (the same G5
  encumbrance deferral as the buy shop).
  - **Buffer buffs land icon-only when their combat math is unported:** a buff
    whose effects all fall through the `EFFECT_REGISTRY`/match arms produces an
    empty effect list and gets dropped whole at `apply_skill_effects`' guard (so
    the buff never appears). Effects mapping to a modeled `Stat` (`ReduceCancel`,
    `ShieldDefenceRate`, `CriticalDamage`, …) both land and work; the dance/song
    buffs whose stat isn't modeled — Dance of Light (277, `AttackAttribute`
    element power), Song of Champion/Renewal (8547/349, `MagicMpCost`/`Reuse`
    per-magic-type rate), Gift of Seraphim (4703, `Reuse`), Song of Vengeance
    (305, `DamageShield` reflect) — now carry an icon-only marker so the buff
    shows and expires, with the real effect deferred (`TODO(G16/G20)`: attack
    element, per-type MP-consume/reuse rate stats, damage reflect).

Run: `cargo test` (all green). Boot a pair on alt ports:
`cargo run -p loginserver` + `CONFIG_SERVER_GAMESERVERPORT=… cargo run -p gameserver`.

### Newbie Guide — the `onFirstTalk` hook

Java registers NPC chat windows two ways: the `data/html/**` file the NPC id
resolves to, and `addFirstTalkId`, where a script **replaces** the window
outright. Only the first was ported, so all five Newbie Guides (30598–30602)
fell through to `npcdefault.htm` and showed a single "Quest" button instead
of their four-entry menu.

- `QuestScript::first_talk_npcs`/`on_first_talk` + a one-owner-per-NPC
  `QuestRegistry` index; `quests::notify_first_talk` runs from
  `target::interact_with_npc` **before** `showChatWindow`, matching
  `NpcAction`'s ordering (so it fires even for a non-talkable NPC).
- `NpcTemplate.race` — `<race>` was parsed by nobody; the guides' own-race
  gate (`npc.getRace() != player.getRace()` → `-no.htm`) needs it. Stored as
  the `Race` ordinal so it compares to `Player.race` directly; non-player
  races (`UNDEAD`, `BEAST`, …) are `None`.
- `scripts/newbie_guide.rs` — menu + the `-<n><m|f>.htm` advice pages
  (`MAGE_GROUP` stands in for `isMageClass()`). The Q00255 tutorial reward
  branch is a `TODO(G33)`: the tutorial quest is unported.
- `scripts/npc_location_info.rs` — the "NPC Location Information" submenu,
  `custom/NpcLocationInfo`: 161 whitelisted town NPCs, radar marker on the
  chosen one's spawn (`QuestCtx::any_spawn_location`/`add_radar`).

Deviation: `getAnySpawn` reads Java's spawn *table*; the Rust port scans live
spawned NPCs instead. Identical for the always-spawned town NPCs on the
whitelist.

- `scripts/teleport_to_race_track.rs` — the Monster Derby Track round trip
  (`ai/others/TeleportToRaceTrack`). Twelve gatekeepers carry the free
  "Teleport to the Monster Arena and the Monster Race Track" button; the
  Race Track Manager (30995) reads the origin back and returns the player.
  Previously unported, so every one of those buttons was silently dead —
  the bypass resolved to no script and the window just closed.

  The return point lives in the *character* variable store (`MONSTER_RETURN`
  → npc id), so this script added `QuestCtx::{player_var_int,
  set_player_var_int, unset_player_var}` over the existing
  `PlayerVariables` component — the first script to reach for
  `character_variables` rather than per-quest `QuestState` vars.

  `bare_talk()` stays false, matching Java: all fourteen htmls point at the
  *named* `Quest TeleportToRaceTrack` bypass, which reaches `on_talk`
  regardless of `id()`, so the quest-window chooser is never involved.

  Deviation: Stanislava (31699) carries the button in her html but is absent
  from Java's `TELEPORTER_LOCATIONS`, so the Java return trip NPEs on
  `teleToLocation(null)`. The port falls back to the Dion default instead of
  dropping the teleport.

  Not ported: the `RaceManager` betting UI (`MonsterRace`, ticket
  purchase/payout). Only the exit/entry teleports work; Java's
  `RaceManager` overrides `onBypassFeedback` for betting but not
  `showChatWindow`, so `html/default/30995.htm` — the page holding the exit
  button — renders correctly either way.

### Buff persistence across relog ✅

Buffs now survive logout — the `restore_type = 0` half of Java
`storeEffect`/`restoreEffects` that G13.9 and G17 deferred. The rule: **a
buff's countdown is frozen while offline** (rows store relative
`remaining_time`), whereas a cooldown's keeps running (rows store an absolute
`systime`). Store filter reproduces Java's skip list (dances unless the new
`AltStoreDances` config, toggles, `LIFE_FORCE_OTHERS`, dedupe); restore applies
each row at enter-world through a new `apply_continuous_effects` split out of
`apply_skill_effects`, so a restored buff doesn't re-fire the skill's damage or
heal (Java's `instant = false`). Details + known gap
(`isDeleteAbnormalOnLeave` isn't parsed yet) in
[PLAN_BUFF_PERSISTENCE.md](PLAN_BUFF_PERSISTENCE.md).

### Monster level/aggro in NPC titles (`ShowNpcLevel`/`ShowNpcAggression`) ✅

Port of `Creature.getTitle()`'s custom-title branch, which `NpcInfo` reads
through `calcBlockSize`/`writeImpl`: with `NPC.ini`'s `ShowNpcLevel` /
`ShowNpcAggression` (both True on this dist), a monster's title becomes
`Lv <level>` + `[A]` (template `isAggressive`) + `[G]` (has `<clanList>` and a
`clanHelpRange`), with the template title appended. New `npc_title` helper in
`server_packets/npc.rs`; `npc_info` now takes `&NpcConfig` and includes the
TITLE component for any monster when either flag is set (the Java mask
condition), so mobs that previously sent no title now do. Champion and trap
title branches skipped (neither modeled). Quirk kept for byte parity: Java
appends the `[A]`/`[G]` separator space before checking the flags, so a calm,
clanless mob titles as `"Lv 20 "`.

---

## Cross-cutting notes

- Game server runs from `dist/game`; all ini/data paths resolve unedited.
  `GameData::load_from(path)` lets tests point at the datapack from any cwd.
- Session lifecycle is a **type-state** machine (plan §3.1):
  `Connecting → Authenticated → InLobby → Entering → InGame`; the `Player` lives
  in `World.players` keyed by object id, `InGame` links by id.
- The object registry (`World.objects`) is **one `bevy_ecs` world** holding
  players and NPCs as entities decomposed into per-concern components
  (CONCURRENCY_MODEL §2.8; G9.5 / [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md)).
  The game thread remains the sole owner; no parallel scheduling; object
  ids are the only foreign key (`Entity` never leaves `store.rs`).
- Masked packets use the reversed `DEFAULT_FLAG_ARRAY` bit order — get this right
  or the client desyncs (root cause of the earlier UserInfo mask fix).
