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
| Game  | G19 Skills & effects breadth                                | ✅ **affect scopes + toggles landed** (plan: [PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md)): `AffectScope` SINGLE/RANGE/POINT_BLANK/PARTY/PLEDGE + `AffectObject` ALL/NOT_FRIEND/FRIEND/CLAN in `skills/affect.rs` (affectLimit cap with Java's `min + Rnd.get(max)` quirk, dead-skip, caster-skip, peace-zone leg, LOS from the target), the cast pipeline fanned out over the affected list (`apply_cast_consequences` per target — effects + PvP flag + hate), and **toggles** (recast = off, `toggleGroupId` exclusion, instant cast per `SkillCaster`'s short circuit, new `targetType NONE`). **Abnormal-state flags + crowd control landed** (plan: [PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md)): Java's `EffectFlag` mask ported as per-`ActiveBuff` flags folded on read (`game_loop/abnormal.rs` — no cached mask to invalidate), `BlockActions` (540 uses — stun/sleep/paralyze) and `Root` (79) effects, and the gates that read them (no attack/cast/move while stunned, no move while rooted, NPC AI silent while stunned, rooted mobs stay put), plus the mid-action interrupt (abort cast *then* freeze movement — the other order lets `stop_casting` resume the walk). Before this a stun landed, showed its icon and changed nothing. **Abnormal resistance/blocking + probabilistic dispel landed** (plan: [PLAN_G19_ABNORMAL_RESIST.md](PLAN_G19_ABNORMAL_RESIST.md)): `ResistAbnormalByCategory`→`Stat::ResistAbnormalDebuff` folded into `calc_effect_land_rate` as Java's `buffDebuffMod` (multiply then clamp, so Guts halves incoming debuff chance), `ResistDispelByCategory`→`ResistDispelBuff` (pumped but consumer-less until `Cancel` lands — Java reads it only in `calcCancelSuccess`), `BlockAbnormalSlot` (Prophecy mutual exclusion, stamp-and-fold like the CC flags) and `DispelBySlotProbability` (the Bane family's per-buff rate roll). **Ranking note:** unported effects must be ranked by *learnable-skill* usage, not raw instance count — `StatUp` looks like the biggest gap at 887 instances but is only 9 learnable skills (the rest are talisman/Freya/agathion content). **Periodic HP/MP + healing/CP breadth landed** (plan: [PLAN_G19_PERIODIC_EFFECTS.md](PLAN_G19_PERIODIC_EFFECTS.md)): `HealOverTime` (negative power = the upkeep toggles' HP cost, floored at 1) and `ManaDamOverTime` joined the existing DoT tick chain, with an out-of-MP tick switching a **toggle** off + SM 140 (Java's `false` return, honoured only for toggles); `HealEffect` (HEAL_EFFECT mul / _ADD diff, read off the *recipient*) folded into the Heal path; `Cp` instant restore/drain with DIFF/PER. Closes a loop: the toggles ported in the first G19 slice now actually cost HP/MP. The empty-effects guard's third exemption was generalised into `has_periodic` — any effect with no stat modifier must join *periodic*, *icon-only* or *state flag* or it is silently dropped. **CC breadth landed** (plan: [PLAN_G19_CC_BREADTH.md](PLAN_G19_CC_BREADTH.md)): `Mute`/`PhysicalMute` (magic vs non-magic cast gate in `checkDoCastConditions`, static skills exempt, mutually exclusive), `DebuffBlock` (incoming debuffs bail outright ahead of the resist roll; buffs unaffected), `BlockControl` (item-use gate — Java's wider summon/mob-control meaning is G29) and `TargetCancel` (chance-rolled instant: drops the target via `set_target(None)` so `TargetUnselected` broadcasts, and aborts attack+cast). Landing a mute also aborts the victim's in-flight cast, with **raid bosses immune** to that interrupt. `Fear` is the CC hold-out — it needs forced flee movement, so it belongs with G21's AI breadth. **Abnormal visual effects landed** (plan: [PLAN_G19_ABNORMAL_VISUALS.md](PLAN_G19_ABNORMAL_VISUALS.md)): the cosmetic half of all the CC above — `AbnormalVisualEffect` id map + `<abnormalVisualEffect>` parsed, stamped on `ActiveBuff` and folded on read; `CharInfo` (which hard-coded a count of **0**, so nobody ever saw an effect on anyone) and `ExUserInfoAbnormalVisualEffect` now carry the real set; pushed **only when the set changes**, as Java does. Plus `//ave_abnormal` toggling a GM-pinned visual via a new `AdminVisuals` component folded alongside the buff-derived ones. Remaining AdminEffects AVE handlers (`//setteam`, `//settargetable`, `//set_displayeffect`, `//playmovie`) are unblocked but need their own per-creature state + packet fields. Before this, only SINGLE resolved — every one of the datapack's 1900+ area skills hit exactly one target. **Transformation landed** (plan: [PLAN_G19_TRANSFORMATION.md](PLAN_G19_TRANSFORMATION.md)): the "Transform <Monster>" scroll family (32 learnable skills — Grail Apostle, Unicorn, Doom Wraith, Zaken, …), wired into the existing G13.B `//transform` admin runtime (`Player.transform_id`/`TransformData`) via the skill-cast path — `admin::transforms` split into state-only and state+broadcast halves so the buff-landing path can fold the transform-specific extras onto the `UserInfo` it already sends rather than double-broadcasting; reverts on `BuffExpire`, which (since death already routes stripped buffs through the same removal fn) covers death for free. Cast-time gate ports `ConditionPlayerCanTransform`'s already-transformed/in-water/cursed-weapon-equipped legs (`DefenceAttribute`, the next effect on the raw-count list at 33 learnable skills, is Kamael-era elemental attributes and out of scope). **MpConsumePerLevel landed** (plan: [PLAN_G19_MP_CONSUME_PER_LEVEL.md](PLAN_G19_MP_CONSUME_PER_LEVEL.md)): the MP-upkeep half of the core fighter toggles (Accuracy 256, Guard Stance 288, Vicious Stance 312, War Frenzy 424, Super Haste 7029, …) — each already lands a real `StatModifier`, but this *other* effect on the same skill was silently dropped, so every one of these toggles was a free, uncosted buff. Every instance in the datapack is a toggle with no `abnormalTime`, collapsing Java's formula to `ManaDamOverTime`'s `power * getTicksMultiplier()`, so it shares that effect's tick-chain arm rather than duplicating it (periodic drain, self-deactivate + SM 140 on insufficient MP); the level-scaled `abnormalTime > 0` branch is unexercised by this datapack and left a TODO. Also fixed `admin_superhaste_applies_and_persists`, whose zero-MP test setup broke once Super Haste's own drain (Java's `AdminSuperHaste` casts through the real `applyEffects` path) started applying. **ShieldDefence/ShieldDefenceRate landed** (plan: [PLAN_G19_SHIELD_DEFENCE.md](PLAN_G19_SHIELD_DEFENCE.md)): Shield Mastery (153), a passive every shield-using class can learn, pumps both stats — `ShieldDefenceRate` was already parsed (`EFFECT_REGISTRY`) but never actually read (`game_loop::combat::shield_stats` used the equipped shield's raw `rShld` directly, bypassing `StatModifiers`); `ShieldDefence` wasn't parsed at all. Both now fold through `model::finalize` (bumped `pub(crate)`) over the shield's own `sDef`/`rShld`, gated behind the existing no-shield-equipped early return so a flat buff still contributes nothing without a shield, matching `Formulas.calcShldUse`'s short-circuit. `EnergyAttack` (9 learnable) set aside — needs the unmodeled Dwarf Force/Charges resource first. **HealPercent landed** (plan: [PLAN_G19_HEAL_PERCENT.md](PLAN_G19_HEAL_PERCENT.md)): all 5 learnable instances are core priest kit — Miracle (1426), Benediction (1271), Restore Life (1258), Revival (181), Touch of Life (341) — every one of which parsed to an empty effect list and healed nothing. New match arm mirrors `Heal`'s NPC-silent/player-with-SM split and overheal clamp, computing the amount as a max-HP percentage rather than the magic-formula power, and skipping `Heal`'s recipient `HealEffect`/`HealEffectAdd` scaling (Java's real asymmetry). Surfaced `TargetType::EnemyNot` as unmodeled (falls through to `Other`, silently no-op'd by `use_magic_on`) while testing Restore Life. **`TargetType::EnemyNot` landed** (plan: [PLAN_G19_ENEMY_NOT_TARGET.md](PLAN_G19_ENEMY_NOT_TARGET.md)): "any friendly selected target" — the precise inverse of `Enemy`/`EnemyOnly`'s `is_auto_attackable` gate, no force-use override, self always allowed, exempt from the general dead-target rejection ("works on dead targets or doors as well"). Small (34 instances) but it was quietly capping the two `HealPercent` skills that heal someone other than the caster (Restore Life, Touch of Life). `AttackTrait` (7 learnable) set aside — needs a `TraitType` attacker-bonus system unmodeled on this port. **Force/charges landed** (plan: [PLAN_G19_FORCE_CHARGES.md](PLAN_G19_FORCE_CHARGES.md)): unblocks `EnergyAttack`, set aside twice before. New `Player.charges` resource (transient, never persisted) backs Sonic Focus → Sonic Blaster/Buster and the Orc/Dark Elf Force Burst/Storm/Blaster family — 9 `EnergyAttack` + 6 `FocusMomentum` learnable skills all parsed to empty effect lists before this. `FocusMomentum` gains charges capped at `max_charges.min(8)` (Java's `MAX_MOMENTUM` stat is never set anywhere in this datapack, so `8` is the real cap, not a simplification); `EnergyAttack` shares `PhysicalAttack`'s damage core times a new `1 + charge×0.1` boost, reading `chargeConsume` off a skill-level tag rather than the effect's own params. `EtcStatusUpdate` (0xF9) now carries the real charge count. Deferred: Java's 10-minute charge-decay task, `GetMomentum` (dead code — nothing sets `MAX_MOMENTUM`), and wiring the charge bonus into `PhysicalSoulAttack`/`MagicalSoulAttack`/`SoulBlow`'s existing `×1` stand-ins. **Lethal landed** (plan: [PLAN_G19_LETHAL.md](PLAN_G19_LETHAL.md)): `AttackTrait` set aside a third time — needs the cross-cutting `TraitType` system, not a slice. `Lethal` (9 learnable) was already flagged as a TODO on `SkillEffect::Blow`'s own doc comment — every learnable instance pairs it with an already-ported damage effect (Backstab 30, Lethal Blow 344, Deadly Blow 263, Critical Blow 409, Lethal Shot 343, Turn/Banish Undead/Seraph), so those skills' damage landed but the bonus instant-kill/half-kill chance never rolled. Level gate + raid-boss immunity (reusing `Mute`'s own `is_raid()` check) ported; full/half-lethal rolls set a player's CP (and HP, on a full lethal) to 1 or halve a monster's HP, with `chanceMultiplier` at 1.0 (no trait/attribute math anywhere on this port). `INSTANT_KILL_RESIST` isn't rolled — like `MAX_MOMENTUM`, nothing in this datapack ever sets it. **AttackTrait landed** (plan: [PLAN_G19_ATTACK_TRAIT.md](PLAN_G19_ATTACK_TRAIT.md)): the last item on the learnable-skill ranking, investigated properly instead of deferred a fourth time. All 7 learnable instances (Detect Insect/Beast/Animal/Dragon/Plant Weakness, Eye of Hunter/Slayer) use only the `*_WEAKNESS` category of `TraitType` — and the consuming formula turns out inert on the real Java server too (`calcWeaknessBonus` needs a matching NPC-side `DefenceTrait`, and nothing in this datapack ever sets one — grepped the whole Java tree, one call site, its own definition). Lands as an icon-only buff, closing a real regression (the effect wasn't recognized at all, so it didn't even land) without inventing damage-formula wiring for a bonus that's provably inert either way. Collateral: `NpcTemplate.race`/`Race` extended from 6 playable races to Java's full 26-member shared enum (players + creature categories) — costs nothing today, ready for when NPC-side trait data lands. **DamageBlock landed** (plan: [PLAN_G19_DAMAGE_BLOCK.md](PLAN_G19_DAMAGE_BLOCK.md)): the highest raw instance count left (5 learnable, 84 skills, 162 instances — a skill carries two `<effect>` elements, one `BLOCK_HP` one `BLOCK_MP`), already flagged by two existing TODOs on `HealPercent` and `Lethal`. The five learnable instances (Celestial Shield 1418, Flames of Invincibility 1427, Dance of Medusa 367, Sonic/Force Barrier 442/443) are short full-invulnerability shields. `HP_BLOCK` has a real single choke-point consumer in Java (`CreatureStatus.reduceHp`), matched by threading a new `is_dot: bool` parameter through `game_loop::combat::apply_physical_damage` — already the one function every damage path on this port funnels through — with an early return, exempting only DoT ticks (damage zones are *not* exempt, matching Java's `DamageZone`). `MP_BLOCK`/`isMpBlocked()` is the same "genuinely dead code in Java too" pattern as `MAX_MOMENTUM`/`INSTANT_KILL_RESIST`: zero callers anywhere in the Java tree, folded for completeness but wired to nothing. Both existing TODOs closed. **EnlargeSlot landed** (plan: [PLAN_G19_ENLARGE_SLOT.md](PLAN_G19_ENLARGE_SLOT.md)): a re-run of the ranking sweep with `EFFECT_REGISTRY`'s generic stat-modifier table correctly excluded (it had been quietly absorbing dozens of effect names and inflating earlier raw counts) surfaced this on top — Expand Inventory/Warehouse/Trade/Common Craft/Dwarven Craft (5 learnable, 162 raw instances). A `type`-selected `Stat` passive (6 new variants: `InventoryNormal`, `StoragePrivate`, `TradeSell`, `TradeBuy`, `RecipeDwarven`, `RecipeCommon`), folded through `model::finalize` into `UserInfo`'s INVENTORY_LIMIT block, `ExStorageMaxCount` (previously all six capacity fields were Java's static placeholder defaults, one literally commented "`Stat.INVENTORY_NORMAL` not wired"), and `crafting::learn_recipe`'s recipe-book cap, the one consumer with real enforcement behind it — warehouse deposit and private-store listing still aren't capacity-checked anywhere on this port (`TODO(G29+)`), so only the *number reported* changed for those. Surfaced and fixed a wider pre-existing gap along the way: a newly learned passive skill only took effect at the next login; `RequestAcquireSkill` now also calls `recompute_conditioned_passives` (already generic under its armor-swap framing), so any stat-modifier passive applies the moment it's learned. **Hate-manipulation effects landed** (plan: [PLAN_G19_HATE_EFFECTS.md](PLAN_G19_HATE_EFFECTS.md)): a tied cluster of six related effect names sharing one already-ported primitive (`AggroList`) — rather than take the top name alone and defer the rest a fifth time (the `AttackTrait` pattern), bundled the four cheap ones: `GetAgro` (Aggression, Aggression Aura, Judgment, Tribunal), `AddHate` (Charm, Lure), `DeleteHate` (Eva's Serenade, Peace, Repose), `DeleteHateOfMe` (Bluff, Forget, Trick) — 12 learnable-skill instances. `GetAgro` needed the most care: the ported AI derives its attack target fresh from `AggroList::most_hated` every think tick rather than caching a "current target," so "force intend-attack the caster" became "make the caster's hate dominant" (current max + 1) rather than a direct intention override. `DeleteHate`/`DeleteHateOfMe` both disengage via a newly `pub(crate)` `npc_ai::set_active`, shared with `think_attack`'s own timeout/leash disengage rather than duplicated. Deferred: `TargetMe` (paired with `GetAgro` on the same 2 skills) needs a locked-target UI concept nothing on this port has; `RandomizeHate` (Confusion, Switch) needs a general nearby-visible-creatures query `faction_call`'s NPC-only neighbour scan doesn't provide; `GetAgro`'s clan-mate pre-seed is left to `faction_call`'s own reactive recruit, at most one think-tick later. **DispelByCategory landed** (plan: [PLAN_G19_DISPEL_CATEGORY.md](PLAN_G19_DISPEL_CATEGORY.md)): the "Cancel" family (Cancellation, Cleanse, Purification Field, Touch of Death), another tied cluster at 4 learnable skills — picked over the cheaper `PhysicalAttackRange` (a same-shape repeat of the already-solved `ShieldDefenceRate` pattern, no new value) because it closes a real gap flagged two slices ago: `Stat::ResistDispelBuff` was pumped but "consumer-less until `Cancel` lands." Unlike `DispelBySlot`/`DispelBySlotProbability` (a fixed abnormal-type list), this steals *whatever* is up — `BUFF` slot walks dances then buffs in reverse cast order, each gated by a ported `calcCancelSuccess` (`clamp(rate + (casterMagicLvl - buffMagicLvl)*2 + (buffAbnormalTime/120)*ResistDispelBuff, 25, 75)`, skipped as automatic when `rate>=100`); `DEBUFF` slot uses a flatter `roll<=rate` (Java's exact operator, not this codebase's usual `<`). The dances-before-buffs split and most of `canBeStolen()`'s exclusions came free from the already-ported `BuffSlot` classification. Java's `ALL` slot is dead code too, and stays a no-op here. Deferred: `isIrreplacableBuff()`/hero/GM/static-skill exclusions (unmodeled fields, matching `DispelBySlotProbability`'s own precedent). **PhysicalAttackRange landed** (plan: [PLAN_G19_PHYSICAL_ATTACK_RANGE.md](PLAN_G19_PHYSICAL_ATTACK_RANGE.md)): Archery/Long Shot/Rapid Fire/Snipe, the cheapest of the tied-at-4 cluster `DispelByCategory` was picked from — a same-shape repeat of the already-solved `ShieldDefenceRate`/`AttackCancel` pattern, needing only an `EFFECT_REGISTRY` entry and wrapping `recalculate_stats`' bare `combat.atk_range` line in `finalize()` (the same gap `ShieldDefenceRate` itself had before an earlier slice). All four learnable instances are `<weaponType>BOW</weaponType>`-conditioned; the condition mask is already generic across every registry entry, so nothing extra was needed to gate correctly — proven by a test showing the bonus is inert while unarmed. **FatalBlowRate landed** (plan: [PLAN_G19_FATAL_BLOW_RATE.md](PLAN_G19_FATAL_BLOW_RATE.md)): Assassination/Critical Blow/Focus Death/Mortal Strike, another tied-at-4 pick — directly tied to the already-ported `Blow`/`Lethal`/`FatalBlow` mechanics, since `formulas::calc_blow_success`'s own doc comment flagged `Stat.BLOW_RATE`/`BLOW_RATE_DEFENCE` as hardcoded identity. Same `EFFECT_REGISTRY` wiring as `PhysicalAttackRange`; the formula gained one `blow_rate_mod` parameter multiplied into the existing rate expression, threaded from the caster's finalized `StatModifiers`. `Stat.BLOW_RATE_DEFENCE`/`FatalBlowRateDefence` is genuinely dead in Java too — a registered handler no shipped skill grants — matching the recurring `MAX_MOMENTUM`/`INSTANT_KILL_RESIST` pattern. **Fear landed** (plan: [PLAN_G19_FEAR.md](PLAN_G19_FEAR.md)): the CC hold-out the CC-breadth slice deferred to "G21's AI breadth" — **G21 is complete**, so the forced-flee movement it needed now exists. Top of the in-scope ranking at 8 learnable skills (Horror 65, Banish Undead 405, Banish Seraph 450, Fear 1092, Curse Fear 1169, Word of Fear 1272, Mass Curse Fear 1381, Turn Undead 1400); everything above it is out of scope (`DefenceAttribute` 31 — Kamael elemental attributes) or G29 (`Summon`/`SummonCubic`/`SummonNpc`, 24/12/9). Reading the Java shrank the port twice: **`EffectFlag.FEAR` has no reader** (no `isAfraid()`, nothing `isAffected(FEAR)` — a feared creature is *not* gated out of attacking, casting or walking) and **`EVT_AFRAID` has no handler**, both the recurring `MP_BLOCK`/`MAX_MOMENTUM` "dead in Java too" pattern, so the entire mechanic is `fearAction`'s repositioning: 500 units away from the caster on `onStart`, then along the victim's *own heading* every 5-tick beat (Java passes `null` for the effector on repeats, so they keep running the way the first shove threw them rather than being re-aimed at a caster who may be dead by then). Shares the existing DoT tick chain rather than growing a scheduler; `canStart` ports the raid and `Defender`/`FortCommander`/`SiegeFlag`/`SIEGE_WEAPON` carve-outs. The load-bearing piece is **`NpcIntention::MoveTo`**: `AttackableAI.onEvtThink`'s switch has **no `AI_INTENTION_MOVE_TO` case**, so a fleeing mob thinks about nothing until it arrives — without it the next think tick re-issues the chase and drags the mob straight back, making the flee invisible (`onEvtArrived`'s `MOVE_TO`→`ACTIVE` reset ported alongside, off a new `TickOutcome.arrived`). **This was a quiet gap, not a loud one:** every Fear skill also carries the already-ported `BlockControl`, so the buff always landed — icon, duration, `BLOCK_CONTROL` flag — and the debuff looked like it worked; it just never moved anyone. Deferred: `canStart`'s `isSummon()` leg (`TODO(G29)`). **StatByMoveType + the player regen stat pipeline landed** (plan: [PLAN_G19_STAT_BY_MOVE_TYPE.md](PLAN_G19_STAT_BY_MOVE_TYPE.md)): picked from a three-way tie at 4 learnable (`StatByMoveType`/`MagicalAttackMp`/`SilentMove`) because two of its four skills — Vital Force 148 and Clear Mind 1297 — carry *only* this effect and so parsed to an empty effect list and were **dropped whole**, passives that did precisely nothing. Behind it sat a much bigger gap the ranking is structurally blind to: the sweep counts *unported effect names*, and `HpRegen`/`MpRegen`/`CpRegen` are in `EFFECT_REGISTRY` — but **`regen_player` never read `StatModifiers` at all**, so all 21 learnable regen skills (Focus Mind 191, Mana Recovery 214, Regeneration 1044, Song of Life 265, Victories of Pa'agrio 1414, …) pumped a stat nobody consumed, the same "parsed but unconsumed" shape as `ShieldDefenceRate`/`PhysicalAttackRange`. Real scope: **25 learnable skills, not 4**. `regen_player` now ends in Java's `Stat.defaultValue` (`mul*base + add + getMoveTypeValue(stat, getMoveType())`) for all three of HP/MP/CP, and the hard-coded standing multiplier became the real `Creature.getMoveType`-driven block (sitting 1.5 / standing 1.1 / running 0.7 — and **walking falls through every branch for no multiplier at all**, so walking regen is *worse* than standing still; Java as written, now pinned by a test), retiring a stale `TODO(G7)`. `StatByMoveType` itself rides on a new `StatModifierEffect.move_type` qualifier, so the entire buff pipeline (landing, stacking, removal, passive folding) needed no changes; `apply_modifier` routes it to a separate `StatModifiers::by_move_type` map — Java's own `_moveTypeStats`, deliberately *not* folded into `add`, which would apply the bonus in every locomotion state instead of the one it names — read live against the current move type, so the value swings as the player stands/walks/runs with no stat recompute. Acrobatic Move 225's evasion (the one non-regen use) folds in at `combat::combatant()`'s per-attack snapshot rather than the cached `CombatStats`, matching Java's on-demand finalizer. Deferred: `MoveType::Sitting` (no source — sitting isn't modeled, `TODO(G29)`; parsed and stored so it starts applying for free once it lands), the zone/residence regen multipliers, and the tie's other two effects. **Critical-damage stats landed** (plan: [PLAN_G19_CRITICAL_DAMAGE.md](PLAN_G19_CRITICAL_DAMAGE.md)): found by running the *previous* slice's post-mortem check first — the name-based ranking is structurally blind to "parsed but unconsumed" stats, so this time every `Stat` variant was swept for consumers outside `stats.rs`/`skill_data.rs`. Exactly two came back with **zero readers**: `CriticalDamage` and `CriticalDamageAdd`. All three damage formulas hard-coded `if crit { 2.0 }`, so **18 learnable skills were completely inert** — including Death Whisper 1242, Focus Attack 317, Vicious Stance 312, Frenzy 176, Dance of Fire 274, Zealot 420, Dead Eye 414, Chant of Victory 1363. Pulling the thread gathered the family: `CriticalDamagePosition` (3, also on the ranking), `MagicCriticalDamage` (2), `DefenceCriticalDamage` (1) — **24 learnable skills**. `formulas::CritDamage { mul, add }` carries Java's `calcCritDamage`/`calcCritDamageAdd` results, with `Default` = the stat-free `2.0`/`0.0` so the refactor is provably behaviour-preserving for an unbuffed actor (pinned by a test, which is what the pre-existing damage tests rest on). `calc_auto_attack_damage` now follows Java's two-section expression `(((attack·cAtk·ss) + cAtkAdd)·critMod)·77 + (attack·(1−critMod)·ss·77)` — the bracketing is load-bearing: `cAtkAdd` lands *after* the soulshot multiply but *inside* the ×77/÷pDef, so a flat +32 is worth far more than face value. **`StatQualifier`**: last slice's `StatModifierEffect.move_type` field generalised to an enum rather than growing a second parallel `Option` that would rot — `MoveType` merges additively from 0.0 into `_moveTypeStats`, `Position` multiplicatively from 1.0 into `_positionTypeStats`, two maps because Java's merges and identities genuinely differ. The data corrected two wrong assumptions along the way, both now pinned: Focus Death 355 carries **two** position entries with opposite signs (front −30% → ×0.7, back +90% → ×1.9 — the asymmetry only survives because that map multiplies), and skill 193 "Critical Damage" is `mode=DIFF`, a flat +32 `cAtkAdd`, not a percentage. Deferred: `PHYSICAL_SKILL_CRITICAL_DAMAGE` (no learnable grantor on this dist → that branch stays 2.0, the `BLOW_RATE_DEFENCE`/`MP_BLOCK` precedent), `MAGIC_CRITICAL_DAMAGE_ADD` (computed but never applied in Java either), and `calcBlowDamage`'s own crit shape. **SilentMove + FakeDeath landed** (plan: [PLAN_G19_SILENT_MOVE_FAKE_DEATH.md](PLAN_G19_SILENT_MOVE_FAKE_DEATH.md)): the unconsumed-stat sweep came back **clean** this time (all 44 `Stat` variants now have real consumers), so back to the name ranking and its two-way tie at 4 learnable. `SilentMove` won because its four skills (Silent Move 221, Stealth 411, Dance of Shadows 366, Fake Death 60) all *land* but their **headline mechanic** did nothing — the aggro scan carried a literal `// invisibility/silent-move/GM states don't exist` comment, so stealth failed 100% of the time — and it pulled `FakeDeath` in with it: **Fake Death 60 carries only these two effects**, so with both unported it parsed to an empty effect list and was **dropped whole**. Java reads the two flags on *adjacent lines of the same method* (`AttackableAI.isAggressiveTowards`), so splitting them would have meant touching that function twice. New `npc_ai::notices_target` applies the gate at all three player-scan sites (monster, guard PK, siege guard), as a post-sweep `retain` because the sweep closure holds `objects` mutably. **Raid bosses see through stealth** (`!me.isRaid()`) but are **not** exempt from fake death, which goes through `isAlikeDead()` — an asymmetry that's easy to get wrong, now pinned. `FakeDeath` shares the existing DoT tick chain for its MP upkeep (and, being a toggle, inherits the out-of-MP self-deactivate); new `ChangeWaitType` packet (0x29) plus `Revive` on standing up; `break_fake_death_on_damage` hooks the single `apply_physical_damage` choke point (`FakeDeathDamageStand = True`), gated on `amount > 0` so a missed swing doesn't stand you up. Three Java behaviours were checked and found **inert on this dist** rather than assumed: `canSeeThroughSilentMove` (no callers anywhere in the Java tree), `PlayerFakeDeathUpProtection = 0` (the stand-up grace window), and `FakeDeathUntarget = False`. Testing note: the baseline test failed first run and revealed two stealth tests passing **vacuously** — `NpcAi.global_aggro` starts at −10 and creeps 1 per think tick, so a monster needs ~100 game ticks before its scan runs at all (guards are exempt, which is why the older guard tests get away with 20). Deferred: `ChameleonRest`/`Hide` (non-learnable, need sitting), the `RequestRestartPoint`/`RequestActionUse` gates, and `MagicalAttackMp`. **MagicalAttackMp landed** (plan: [PLAN_G19_MAGICAL_ATTACK_MP.md](PLAN_G19_MAGICAL_ATTACK_MP.md)): the MP-drain family — **Mana Burn 1398 and Mana Storm 1399 carry only this effect**, so both parsed to an empty effect list and were dropped whole (the nukes cast, animated and drained nothing); Aura Sink 1102 / Seal of Gloom 1210 pair it with a ported `ManaDamOverTime` so they landed but did none of the up-front damage. Its own formula, sharing nothing with the HP path: `(sqrt(mAtk) * power * (targetMaxMp / 97)) / mDef` — the target's **max MP is a direct multiplier** (the same nuke hurts a mage far more than a fighter), spiritshots scale `mAtk` **before** the square root (so the gain is `sqrt(bonus)`, not `bonus`), and a crit triples then **clamps to a per-skill `criticalLimit`** (1600 on the debuffs, 7000 on the nukes) with no HP-side equivalent; there is also no `damage = 1` floor on a full resist, only the halving. Plus its own landing gate `calcMagicAffected` — a *noisy* mAtk-vs-mDef comparison needing a real `Rnd.nextGaussian()`, ported as `World::roll_gaussian` (Box–Muller over two `roll_f64` draws so tests can still force it through `forced_rolls`). **Correction to the `DamageBlock` slice:** `MP_BLOCK` was documented there as having no callers anywhere in Java — that grep covered `java/` only, and every effect handler lives under `dist/game/data/scripts/handlers/effecthandlers/`, where **five** read `isMpBlocked()` (`MagicalAttackMp`, `Mp`, `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`). The flag is live; `abnormal::is_mp_blocked` now exists and gates this effect, with a `TODO(G19)` for the MP-restore family. *Lesson: grep both trees.* One wrong turn, caught by a failing test and fully backed out: `<magicType>` doesn't exist in this dist's schema — the field is `<isMagic>`, all four skills are magic, and `calcCrit`'s magic branch **discards the `magicCriticalRate` it is passed** in favour of the caster's stat, so the drain's crit is just the existing per-cast `mcrit` and the speculative `Skill.magic_critical_rate` field (which had rippled into 15 test files) was removed. **MP-restore family landed** (plan: [PLAN_G19_MANA_RESTORE.md](PLAN_G19_MANA_RESTORE.md)): the name ranking hit a **five-way tie at 3 learnable**, so rather than pick one arbitrarily this took the entry that anchors a *cluster* — four Java handlers sharing one gate, one clamp and one message pair, differing only in the amount: `ManaHealByLevel` (3 — Recharge 1013, Servitor Recharge 1126, Mass Recharge 1428), `Mp` (2 — Pain of Sagittarius 417, Body To Mind 1157), `ManaHeal` (**0** reachable — Mortal Strike 410's instance turned out to be enchant-only, see the effect-level-gating slice), `ManaHealPercent` (0 learnable, 46 item skills), plus `ManaCharge` (1 — Higher Mana Gain 285, the stat the others read). **6 learnable skills**, and it closes the `TODO(G19)` the `MagicalAttackMp` slice left on `isMpBlocked`. **All three `ManaHealByLevel` skills carry only that effect**, so the core mage-support skill in the game parsed to an empty effect list and restored nothing. `ManaCharge` was found by applying the previous slice's both-tree grep lesson — `Stat.MANA_CHARGE` looks unused from `java/` alone, but a handler under `dist/.../effecthandlers/` grants it and a learnable skill uses it; without it the recharge skills would read a stat with no source. `ManaHealByLevel`'s penalty ladder (unpenalised to a 5-level gap, then ×0.9 down to ×0.1, **0 from 15 up**) collapses to `1 - (diff - 5)/10` and replaces Java's nine `else if` branches, with every branch pinned by test. Checked rather than assumed: `MAX_RECOVERABLE_MP` has **no grantor on this dist** (the `LimitMp` handler exists but no skill uses it), so the overheal ceiling is plain `maxMp` — documented at the clamp instead of plumbed. Testing note: the end-to-end penalty test failed first run because the level-5 fixture's ~50 max MP let the **overheal clamp cap both halves of the comparison**, making them read equal — a clamp downstream of what you're measuring will hide it. Deferred: `FACEOFF` (unmodeled), `ADDITIONAL_POTION_MP` (needs item context threaded into effect application), and the rest of the tied cluster (`TriggerSkillByAttack`, `ReflectSkill`, `BlockMove`, `TwoHandedBluntBonus`, `Confuse`). **Confuse + RandomizeHate landed** (plan: [PLAN_G19_CONFUSE.md](PLAN_G19_CONFUSE.md)): the same five-way tie at 3 learnable, resolved by grouping unported effects by prefix — three clusters tie at 5 (`Trigger*` 362 skills, `TwoHanded*` 22, `Confuse`+`RandomizeHate` 7). This pair won because they share **one blocker, already documented**: the hate-effects slice deferred `RandomizeHate` for want of "a general nearby-visible-creatures query `faction_call`'s NPC-only neighbour scan doesn't provide". New `helpers::visible_creatures` (every living player *or* NPC in an adjacent region cell — Java's `forEachVisibleObject` has no LOS or radius term, so neither does this) unblocks both. **Four of the five skills carry only the unported effect** — Madness 1105, Curse Discord 1163, Seal of Mirage 1213, Confusion 2 — so all four were dropped whole; Switch 12 landed but never switched anyone's hate. The two effects look interchangeable and are not: `Confuse` **adds** a target, `RandomizeHate` **moves** the hate and excludes same-faction mobs ("aggro cannot be transfered to a mob of the same faction") — pinned by a test pair. `calcProbability` reduces to `roll(100) < magicLevel + chance - targetLevel`, unclamped, so a high-level target pushes the threshold to zero and simply shrugs it off. `retarget_onto` reuses the `GetAgro` precedent (hate-dominance instead of a cached AI target). **A datapack trap worth recording:** three of these skills read `<effect name="Confuse" abnormalTime="20">` — but that is an *attribute*, and Java's `parseNamedParamInfo` reads only `name`/`level`/`from|toLevel`/`sub*Level` off an effect element, so it is silently ignored (7 instances datapack-wide, on `Fear` and `Confuse`, meaningless in both). With no real `<abnormalTime>` child there is no buff for an instant effect's flag to live in, so `effect_flag::CONFUSED` is unreachable and both its Java readers are dead — folded inert per the `FEAR`/`MP_BLOCK` precedent, with a test pinning `abnormal_time == 0`. Real chances are 20/20/60 and 80/80 — **none** defaults to 100. **Noted, not fixed:** the Rust skill parser ignores `fromLevel`/`toLevel` attributes on `<effect>` elements (775 instances each), which Java uses to gate an effect to a skill-level range — a real parity gap deserving its own slice. **Per-effect level gating landed** (plan: [PLAN_G19_EFFECT_LEVEL_GATING.md](PLAN_G19_EFFECT_LEVEL_GATING.md)) — **not** from the effect ranking: the `Confuse` slice noticed the parser read only the `name` attribute off an `<effect>` element and ignored `fromLevel`/`toLevel`/`fromSubLevel`/`toSubLevel`, **775 instances each**. Java uses them to attach an effect only to the skill levels its range covers, so every one was live at *every* level: **329 skills affected, 14 learnable**. That outranked the remaining tied-at-3 entries because it is *already-ported effects behaving wrongly* rather than a missing feature — Frenzy 176's two extra `PAtk` and two extra `CriticalRate` (`fromLevel="6"`) were boosting every level-1 Frenzy. Ported `forEachNamedParamInfoParam`'s gate verbatim (both bounds inclusive; `level`/`subLevel` supply the defaults for their pair) behind a new `ParsedEffect` struct, replacing the six-wide tuple the parser had been threading. **Sub-levels are the skill-enchant routes** (1001+/2001+), and this port has no enchanted skills, so sub-level reads 0 and every enchant-route effect is now correctly excluded — the gate is already written to take a real sub-level once enchanting lands. **The sweep caught a regression in the *previous* slice's tests, and it was the fix working:** that slice called Mortal Strike 410 "the one learnable `ManaHeal`", but its instance is `fromSubLevel="2001"` — enchant-only — so `ManaHeal` has **zero** reachable learnable skills here and that cluster's real reach was 6, not 7. `PLAN_G19_MANA_RESTORE.md` and this row are corrected. Checking for the same error elsewhere found it touches only already-ported effects (`PhysicalDefence` 63→59, `Speed` 55→52, `Heal` 18→16, `MagicalDefence` 36→34, `PhysicalAttackSpeed` 43→42) — **no slice-selection decision in this milestone would have changed.** **TriggerSkillByAttack landed** (plan: [PLAN_G19_TRIGGER_SKILL_BY_ATTACK.md](PLAN_G19_TRIGGER_SKILL_BY_ATTACK.md)): a four-way tie at 3 learnable, broken by the prefix-cluster heuristic — `Trigger*` and `TwoHanded*` both total 5 learnable, but `Trigger*` spans **362 skills** to `TwoHanded*`'s 22 and is a capability nothing on this port could express: landing a hit can cast another skill. Carriers are Sword/Blunt Weapon Mastery 205, Dagger Mastery 209 and Dance of Shadows 366 — each a passive/dance whose *on-hit half* did nothing. **Scope decision:** Java's handler takes 15 params, but all three reachable carriers set the same 8, so the port implements that subset and keeps Java's defaults rather than building machinery for content this dist doesn't have (`triggerSkills` ladders, `skillLevelScaleTo`, attacker-level bounds and `attackerType` are all unset here). Hooked at `combat::handle_attack_hit`, the normal-attack choke point that already carries `damage` and `crit`. **The subtle bit: `isCritical` is an *equality* test, not a minimum** — an `isCritical=false` trigger fires only on non-crits, and Dance of Shadows ships one of each, so reading it as "crits also count" would silently double it; both directions are pinned. Java's refresh guard is ported too (don't re-cast while the same buff is up at that level), without which a fast weapon would re-apply every swing. **Implementation note:** Java subscribes a listener when the carrying skill starts, but these carriers are passives whose effects this port folds into `StatModifiers` rather than keeping as a live list — so the attacker's skill book is scanned at hit time instead (a few `HashMap` lookups per swing; cache it like `NpcAiSkillIndex` if it ever profiles, it is not a behavioural difference). The triggered skills land real ported effects (5603 grants a 5-second `FatalBlowRate`), and the dist test doubles as a second check on the previous slice's `fromLevel="9"` gating. Deferred: the sibling triggers (`TriggerSkillByMagicType`/`ByDamage`/`BySkill`/…), which share this shape and should reuse its structure. **ReflectSkill + BlockMove landed** (plan: [PLAN_G19_REFLECT_BLOCKMOVE.md](PLAN_G19_REFLECT_BLOCKMOVE.md)): the previous slice's "5 learnable" for `TwoHanded*` was an artifact — skills 94/176 carry *both* TwoHanded effects, so counting per-effect double-counted them. By **distinct learnable skills** `TwoHanded*`, `Reflect*` and `Block*` all tie at 3, so this slice took two of them: both defensive-stance effects, both closing something already documented. **Physical Mirror 350 and Magical Mirror 351 carry nothing but `ReflectSkill`** (dropped whole), and **`BlockMove` is the `_isImmobilized` source** `game_loop::abnormal`'s module docs listed as having "no ported source" — now ORed into `is_movement_disabled` beside `ROOTED`, so these stances pin you without stunning you. Despite the name `ReflectSkill` is **not damage reflection**: its only Java consumer is `calcBuffDebuffReflection`, which on a successful roll **swaps the roles** (`applyEffects(target, caster, …)`) so an incoming *debuff* lands on its own caster — gated on the skill being a debuff *and* declaring an `activateRate` (the default -1 is never reflected). Ported at the per-target apply loop, with hate/PvP consequences left unconditional (the caster still cast a bad skill at that target). The data corrected three things: **`type` is `MAGIC`, not `MAGICAL`** — a **real bug** I introduced that would have routed every magic reflect into the physical stat, caught by a failing assertion and now pinned; both Mirrors carry **two** `ReflectSkill` effects each (30/10 and 10/30, differing by emphasis not kind); and their `<armorTYpe>SHIELD</armorTYpe>` gate is a **datapack typo** (10 occurrences vs 220 correct `<armorType>`) that Java's exact element matching ignores too, so it is inert on both sides. **Noted, not fixed:** the parser reads only the default `<effects>` block — Vengeance 368 puts its `BlockMove` in `<selfEffects>`, so the immobilise silently doesn't load. Datapack-wide the unread scopes are `selfEffects` (91 skills, 7 learnable), `endEffects` (58/1), `pvpEffects` (38/1), `pveEffects` (33/1), `channelingEffects` (24/4), `startEffects` (3/0) — ~14 learnable skills, comparable in reach to the `fromLevel` gap, and a strong candidate for its own slice; a test documents it and will fail when it lands. **Effect scopes landed** (plan: [PLAN_G19_EFFECT_SCOPES.md](PLAN_G19_EFFECT_SCOPES.md)): the gap the `BlockMove` slice found — the parser read **only** the default `<effects>` block, so every effect declared in another scope silently never loaded (~14 learnable skills across `selfEffects` 91/7, `endEffects` 58/1, `pvpEffects` 38/1, `pveEffects` 33/1, `channelingEffects` 24/4). More reach than any remaining effect entry (3), and silent breakage rather than a missing feature. **`SELF` + `PVE`/`PVP` ported**: SELF applies to the *caster* after the target loop (so a skill can buff its caster while debuffing its target), PVE/PVP append to the same target by Java's matchup selector. Every one of the seven `<selfEffects>` carriers holds an already-ported effect (`Speed`, `FocusMomentum`, `BlockMove`, `PhysicalEvasion`, `FatalBlowRate`), so this was pure plumbing with immediate payoff — six skills gained a real self-buff, including Vengeance 368's immobilise. Unsupported scopes parse as `Other` and are **dropped rather than merged**: merging would apply them at the wrong time, which is worse than not having them. Also lands **`impl Default for Skill`** — adding `Skill` fields had broken every exhaustive literal twice (`magic_critical_rate` churned 15 test files and was backed out partly for that), and this slice needed three more; `activate_rate: -1` and `reuse_delay_group: -1` are load-bearing "absent" sentinels that gates test for explicitly, pinned by a test. *Honest note:* the literal conversion took several passes, automated brace-matching mangled four files (reverted from git), and two were finished with explicit fields instead — so the style is mixed (20 files on `..Default::default()`, two explicit) as a deliberate stopping point. **A latent flake surfaced and was fixed:** `confuse_tests::a_confused_mob_turns_on_a_bystander` failed in the sweep, but not from this slice — `apply_skill_effects` charges an unconditional per-cast magic-crit `roll(1000)` *before* any effect runs, so the `Confuse` slice's `forced_rolls.extend([0, 1])` never pinned the candidate index and the assertion had been a coin flip for two slices. Now forces all three rolls with the ordering documented, verified over five runs. *Lesson: when forcing rolls, account for rolls charged by the surrounding machinery, not just the code under test.* Deferred: `startEffects`/`endEffects`/`channelingEffects` (6 learnable between them; they need cast-start, buff-end and channelling hooks the port lacks). **TwoHandedBluntBonus/SwordBonus landed** (plan: [PLAN_G19_TWO_HANDED_BONUS.md](PLAN_G19_TWO_HANDED_BONUS.md)): the top remaining in-scope entry at 3 distinct learnable skills (Rage 94, Frenzy 176, Two-handed Weapon Mastery 293 — Rage and Frenzy carry *both* variants, which is why the naive per-effect count read 5). Java's handler declares **eleven** stat/mode pairs but the reachable content sets only **pAtk and pAccuracy**, so those are read and the rest keep their zero default — the same scope-to-what-the-dist-reaches call `TriggerSkillByAttack` made. The gating is **two independent axes**: the existing `weapon_condition` mask (BLUNT/SWORD) *plus* a new `two_handed` flag for Java's `ConditionUsingSlotType(SLOT_LR_HAND)` — "a blunt" and "a two-handed weapon" are separate tests that both have to pass, so a one-handed mace fails. `two_handed_weapon_equipped` reads the weapon template's `bodypart` rather than inferring two-handedness from an empty off-hand, which would wrongly match an unarmed or shield-less one-hander. Also lands `impl Default for StatModifierEffect` (the same investment `Skill` got last slice; this conversion went cleanly in one pass off the single-line `qualifier:` anchor). Data correction: Rage declares `pAtkAmount = 0` at level 1 and only starts granting at level 2 — my first test asserted on level 1 and failed; a zero-amount modifier is dropped rather than stored, behaviourally identical to Java's `mergeAdd(stat, 0)`. **Resurrection landed** (plan: [PLAN_G19_RESURRECTION.md](PLAN_G19_RESURRECTION.md)): with the in-scope ranking down to a 2-learnable tail, this was picked on **player-visible value** rather than count — Resurrection 1016 / Mass Resurrection 1254 are only 2 learnable skills but a headline mechanic; without them nobody can be raised and every death is a walk back from town. The effect does **not** revive: it *proposes*, the corpse answers a `ConfirmDlg`, and only then do they come back. Two prerequisites came with it: **`TargetType::PcBody`** (a dead *player* corpse — the port had `NpcBody` for Sweeper but no player equivalent, so the skill couldn't even resolve a target) and **pre-death XP tracking** (Java keeps `_expBeforeDeath` and subtracts; the port now records the *difference* in `apply_death_exp_penalty_ex`, which already computes it — the only quantity a resurrection reads, and it can't drift from the penalty that produced it). `calculateSkillResurrectRestorePercent` scales the declared power by the reviver's WIT with a **quirk worth pinning: once the bonus has already added more than 20 it adds a further flat +20**, so high-WIT revivers jump rather than scale smoothly (clamped `[base, 90]`, short-circuited at 0 and 100). The skill's own HP/MP/CP percentages override the config respawn defaults, a zero meaning "leave what the config gave". `DlgAnswer` is now shared: the revive flow gets first refusal and reports whether the reply was its own, so the admin-confirm flow keeps working — pinned by a test. Also ported the re-check that the corpse is *still* dead when they accept (they may have used "to village" while the dialog sat on screen), without which the XP could be taken back twice. Deferred: pets (`TODO(G29)`), Charm of Courage, `BLOCK_RESURRECTION` (ported gate, no learnable grantor), and Mass Resurrection's party fan-out. **DefenceCriticalRate landed** (plan: [PLAN_G19_DEFENCE_CRITICAL_RATE.md](PLAN_G19_DEFENCE_CRITICAL_RATE.md)): the direct mirror of the crit-*damage* slice and the largest remaining in-scope entry (2 learnable, 50 skills) — Light Armor Mastery 233 (`-15% PER`) and Pa'agrio's Eye 1364 (`-30%`) make their holder harder to crit, but the port computed the autoattack crit chance as a bare `crit_stat / 10.0`, so the defender's side of the roll did not exist and both were inert. The load-bearing detail is that Java's two-arg `getValue(DEFENCE_CRITICAL_RATE, rate)` is `mul * rate + add`, so the **defender's multiplier scales the attacker's rate** — reading it the other way round would turn the stat into a flat chance instead of a reduction; pinned by a test. `calc_auto_attack_crit` gained `defence_mul`/`defence_add` at Java's identity defaults, reproducing the old expression exactly so existing combat tests keep meaning what they meant. Two corrections the tests forced, both mine rather than the code's: **`calc_critical_height_bonus(0, 0)` is 1.1, not 1.0** (Java's `+10` before the `/100`), so even the plainest case carries a multiplier and every expected value had to be recomputed; and **Light Armor Mastery is armor-conditioned** — I expected it to fold onto a naked character, but it correctly contributes nothing without light armour, so the test now asserts at the parsed-effect level with Pa'agrio's Eye as the unconditioned contrast. **ResistDDMagic landed** (plan: [PLAN_G19_RESIST_DD_MAGIC.md](PLAN_G19_RESIST_DD_MAGIC.md)): Anti Magic 146 / M. Def. 147 (2 learnable, 38 skills), the mage-defence passives that make incoming spells more likely to be resisted. **It also corrects a wrong claim the port already carried** — `calc_magic_success_rate`'s doc comment said Java's `resModifier` was "fixed at 1.0 on this dist" because the only two *items* touching `magicSuccRes` declare it additively, where `getMul` can't see it. True of the items, wrong as a conclusion: it never considered **skills**, and `ResistDDMagic` is an `AbstractStatPercentEffect` that merges *multiplicatively* — exactly what `getMul` reads. Same failure mode as the `MP_BLOCK` correction: a "provably inert" note only as good as the search behind it. The stat scales the **failure** term (`rate = 100 - (mAcc · lvl · target · res)`), so a value above 1 *lowers* the attacker's success — inverting it would turn a defensive passive into an offensive one, pinned by a test in both directions. `MagicSuccess.res_modifier` defaults to 1.0, reproducing the old expression exactly. Test-fixture correction: the step table bands on `magic_accuracy - magic_evasion` as `> -20 → 2, > -25 → 30, > -30 → 60, > -35 → 90`; my first fixture used a −31 deficit thinking it sat in the 60 band when it lands in 90, so the table is now written out beside it. **Geometric affect scopes landed** (plan: [PLAN_G19_GEOMETRIC_SCOPES.md](PLAN_G19_GEOMETRIC_SCOPES.md)): `FAN`/`FAN_PB` (163+16 skills, **5 learnable** — Sonic Buster, Force Burst, Wild Sweep, Wrath, Frost Wall), `SQUARE`/`SQUARE_PB` (35+17) and `RING_RANGE` (18) now sweep instead of falling back to single-target — chosen over the 2-learnable effect-registry tail on reach, since every dragon breath / tail sweep / quake the G21 mobs and G23 bosses cast is one of these. `<fanRange>` (`unk;startDegree;radius;angle`) parses **level-valued** (Frintezza Charge 5015 declares six tuples) into `Skill.fan_range`. The behavioural break worth knowing: **the geometry applies to the primary target too** — a FAN cast at someone behind the caster misses them, and RING_RANGE *never* hits its epicenter target (the sweep skips its origin and the 2D inner-radius test would drop it anyway — that is the donut hole), so `targets_affected` can now return a set without the named target, or empty; the consumer loop treats entries uniformly, so only the docstring's "always included" claim had to be corrected. Two Java quirks ported as written and pinned by tests: the fan's angle test has **no wrap-around normalization** (a caster whose heading maps to 350° misses a target at bearing 10° — |10−350| = 340 > half-angle — while the same 20° separation away from the seam hits), and `fanHalfAngle = fanAngle / 2` is **integer division** widened to double (a 35° fan tests against 17.0). SQUARE keeps Java's exact rotate-then-compare expression, `(int)` truncations, strict `>` and all — which makes Java's origin self-test provably dead code (the caster lands exactly on the excluded corner), reproduced by running the same filter rather than special-casing. LOS runs from the **caster** for FAN/SQUARE but from the **target** for RING_RANGE, matching each handler. Two parity corrections folded in: `corpse_skill` predated the resurrection slice's `PcBody` and exempted only `NpcBody` — Java's Range/Fan/Square all exempt `PC_BODY` too, so a dead player inside a sweep was wrongly dropped; and the dist's `Range.java` carries a **deliberate local fix** (82a54bbc "Fix minion buffs are given to players") the port had silently skipped — a monster's *good* RANGE skill never sweeps players in (dist is the spec; the branch is dated after the upstream import, found by reading the handler against the ported filter). Tests were verified by disabling each arm and confirming failures — the first pass showed three fan tests passing *vacuously* under a single-target fallback (their positive assertion was on the primary, which the fallback still returns); strengthened to assert on swept **bystanders**, after which all 10 fail when the code is stubbed. **GROUND casts + channeling landed** (plan: [PLAN_G19_GROUND_CHANNELING.md](PLAN_G19_GROUND_CHANNELING.md)): `targetType GROUND` (22 skills, **7 learnable**) split into the **channeled ground AoEs** (this slice — Volcano 1419 / Cyclone 1420 / Raging Waves 1421 / Gehenna 1423, `operateType CA1`) and the `SummonNpc` symbol family (Symbol of Noise 455 / Day of Doom 1422 / Anti-summoning Field 1424 — a totem-NPC subsystem, `TODO(G19)`). The flow: `RequestExMagicSkillUseGround` (**ex 0x41**) stores the aimed point (Java `_currentSkillWorldPosition` — **never cleared, only overwritten**), turns the caster (`ValidateLocation` to bystanders — "normally magicskilluse packet turns char client side but for these skills, it doesn't"), and enters the normal `useMagic`; the `Ground.java` target leg validates (shift/dontMove range vs castRange+collision, LOS to the point, and for bad skills Java's **five-point peace-zone effect-circle sample**) and returns **the caster as sentinel**; `PointBlank.java`'s GROUND fork sweeps around the **stored point** and — unlike the port's normal PB seeding — **never includes the caster** (Java's world sweep skips its origin), so a Volcano can't burn its own caster even under `affectObject ALL` (pinned). The **channeling runtime** is the `SkillChannelizer` as a self-rescheduling `ChannelingTick` (first fire `channelingStart`, period `channelingTickInterval`, staleness = the `Casting` seq, so every finish/abort path is `stopChanneling` for free): per tick, `mpPerChanneling` upkeep (**default = `mpConsume`**, not 0; starvation → SM 140 + abort), a full **re-sweep** (a mob that walks into the fire mid-channel burns — pinned), and the new `<channelingEffects>` scope (was parsed-and-dropped since the effect-scopes slice) applied through the normal pipeline — **without** per-tick `callSkill` consequences (Java's simple path adds no flat hate/PvP flag per tick; the damage itself wakes the mob). **Channeling cast time is static**: `_hitTime = max(hitTime − cancelTime, 0)`, `_cancelTime = 2866` — no casting-speed scaling, pinned by a doubled-mAtkSpd test. Also folded in, because Volcano needs it: the **skill reagent path** (`SkillCaster.checkUseConditions` gate SM 2156 + `startCasting` consume for bad-skill/`ActionType NONE` reagents — Volcano's Magic Symbol 8876; usable items keep paying in their own handler, so no scroll double-consume). The `channelingSkillId > 0` "channelized" branch (hero stances 426/427) is `TODO(G19)`. Test-fixture lesson re-learned: a realistic effect power one-shots the near-zero-m.def default template — probe the `dead` flag, not just despawn, before chasing ghosts. **SummonNpc symbols landed** (plan: [PLAN_G19_SYMBOLS.md](PLAN_G19_SYMBOLS.md)): the GROUND family's other half — Symbol of Noise 455 / Day of Doom 1422 / Anti-summoning Field 1424 (**3 learnable**) now drop a real seal. `SummonNpc` (EffectPoint branch; Decoy/default-spawn `TODO(G19)`) spawns the totem at the stored ground point; the `EffectPoint` runtime is a self-rescheduling `EffectPointCast` (first fire `cast_time` 0.1 s, period `skill_delay` 2 s) that `doCast`s the template's `union_skill` at itself through the shared NPC cast path, plus an `EffectPointDespawn` at `despawn_time` (15 s; effect `despawnDelay` as fallback). NPC templates now parse generic `<parameters>` (`ai_params` + `ai_skill_params` — the dist declares 5145 in BOTH the parameter holder and `<skillList>`, pinned so neither parse eats the other). **`OpExistNpc` is the first parsed skill condition**: ids/range/isAround, swept around the **caster** (not the aimed point) at `useMagic`; the dist quirk that Day of Doom's own totem 13028 is missing from the gate's id list (only Interlude-era 13018–13024) is data, ported as written. The behavioural keystone: **the seal acts as its owner** — `SummonerRef` + an `acting_player` hop in `is_friend`, so the SELF+POINT_BLANK+NOT_FRIEND auras curse bystanders but never the owner or their party/clan (Java `EffectPoint.getActingPlayer()`; same lesson as [[l2r-acting-player]]). Aura payload audit: 5145's percent debuffs + `MagicMpCost` land, 5124/5134's `DispelBySlotProbability` lands; `BuffBlock`/`Unsummon`/`DefenceAttribute` drop at parse (registry tail). Verify-by-disabling caught the owner-exemption test passing **vacuously** (the owner stood outside the aura radius — the assertion held with the friendship hop stubbed out); the owner now stands inside the blast. Deferred: per-pulse PvP flagging of the owner, `setTitle(owner)` cosmetics (per-instance NPC titles need NpcInfo plumbing). **Elemental attributes landed** (plan: [PLAN_G19_ATTRIBUTES.md](PLAN_G19_ATTRIBUTES.md)): the "2-learnable tail" claim was **stale** — a fresh census put `DefenceAttribute` at **33 learnable skills** (the whole Resist Fire/Water/Wind/Earth + Divine/Dark Protection + elemental Surrender family) and `AttackAttribute` at **7** (Holy Weapon 1043, Holy Blade 196, Dance of Light 277, Dark Form 423, the Seeds 1285–1287); it had been mentally filed under the ROADMAP's "elemental attributes are Kamael-era, out of scope" note, which actually covers item attribute *enchanting* — `Formulas.calcAttributeBonus` is live in this dist's Java. Ported: 12 element stats (`FirePower`…`DarkRes`) + an `Element` enum, skill `<attributeType>`/`<attributeValue>` (Volcano's FIRE 20 finally counts), both effects as real per-element `StatModifier`s (comma-list `attribute` params handled; `AttackAttribute`'s icon-only marker variant retired — the census test flipped from "is dropped" to "grants HolyPower +20"), NPC template `<attribute><defence …/>`/`<attack type value/>` bases, and the multiplier folded in at Java's exact spots: `calcMagicDam` (both magic sites incl. the drain family), `PhysicalAttack`, `EnergyAttack`, `calcBlowDamage`, `Lethal`'s chance, and a new `element_mod` factor in `calc_effect_land_rate`. Two read paths: players get it free through the generic buff→`StatModifiers` rebuild; NPCs keep no modifier maps, so element stats fold on read over active buffs (template base + Σ debuffs) — which is what lets Day of Doom's −50s and Surrender to X bite mobs. The no-skill-element case ports Java's `getAttackElement` "temp fix": **the attacker's strongest POWER stat elects the element**, so Holy Weapon colors an attribute-less skill holy (pinned). Auto-attacks stay attribute-less — Java itself never calls the bonus on that path. Deferred: item attribute holders (none on this dist), `calcCounterAttack`'s term, the trait half of Lethal's multiplier. **Skill enchanting slice 1 — the sub-level data foundation landed** (plan: [PLAN_G19_SKILL_ENCHANT.md](PLAN_G19_SKILL_ENCHANT.md); slice 2 = the packet flow/transaction/persistence): 413 dist skills declare enchant routes (**20 learnable** — Sonic Storm, Force/Thunder Storm, Rage, Curse Gloom, Dance of Medusa…), all previously invisible. The parser now collects ranged `<value fromLevel toLevel [fromSubLevel toSubLevel]>` rows raw and resolves them per (level, sub) at finalize — **fixing a latent bug**: those rows used to fall into the level-0 slot, where the last row's `{…}` text clobbered the field's scalar fallback (and plain `{N+index}` magic-level tables never parsed at all). A tiny recursive-descent evaluator (`data/skill_expr.rs`) covers the dist's entire 85-expression grammar (`+ − * / ( )`, `base`/`index`/`subIndex`; the one truncated expression the dist ships evaluates to None and drops, like Java's exception path). `SkillData` pre-builds every enchanted variant like Java — keyed `(id, level, subLevel)`, `get_enchanted`/`enchant_routes` accessors, `Skill.sub_level` stamped — and `EnchantSkillGroups.xml` (30 cost rows: SP/chance/Giant's-Codex-items by NORMAL/BLESSED/CHANGE/IMMORTAL type) loads into `GameData.enchant_skill_groups`. Pinned by dist census: Sonic Storm 40's three routes ((1001,1020)/(2001,2020)/(3001,3020)), `+1` power = base+1%, `+10` = base+10%, route 2/3 leaving the other params at base; Curse Gloom's **field-row** duration route with Java's `StatSet.getInt` truncation (+1 = 10.5 → **10**) — `get_i` gained the f64-truncate fallback for exactly this; fragmented route rows (1001–1005, 1006–1006, …) bucket-merge into one registry entry. Verify-by-disabling initially passed **vacuously** again (the first census case exercised only effect-param rows, not field rows — the Curse Gloom case now pins both passes). **Skill enchanting slice 2 — the flow landed; players can enchant** (plan: [PLAN_G19_SKILL_ENCHANT.md](PLAN_G19_SKILL_ENCHANT.md)): the ex-packet family (`RequestExEnchantSkillInfo` 0x0E → `ExEnchantSkillInfo` routes, `RequestExEnchantSkillInfoDetail` 0x43 → the SP/chance/codex cost preview, `RequestExEnchantSkill` 0x0F → the transaction, `ExEnchantSkillResult` 0xFE:0xA8) in `game_loop/skill_enchant.rs`; a new **`SkillEnchants` component** (id → sub-level) parallel to the (id → level) book, persisted in `character_skills.skill_sub_level` (the column was already there, written 0) as **(id, level, sub) triples through the whole persistence chain** — Char, StoredPlayer, per-index subclass banking, the lobby delevel filter (a downgraded skill drops its enchant), the load path, and `SkillList`'s previously-hardcoded sub-level short. The **cast pipeline resolves the enchanted variant end to end**: `use_magic_on` reads the component, and `CastState` carries `skill_sub_level` so the launch/finish/channeling re-lookups keep it — a sabotage run pinned exactly this (zeroing the CastState sub made the +1 cast silently deal base damage while every other assertion held). Java quirks ported as written: the `Rnd.get(100) <= chance` roll (90% rows succeed 91/100), **items consumed before the SP check** (a broke enchanter loses the codex — pinned), the **+2-onward adena-flavored consume** (`destroyItemByItemId(57, holder.getCount())` per holder — the codex is only ever spent on +1, later steps charge its count as adena — pinned), NORMAL failure → route base + `enchantFailLevel`, BLESSED failure keeps the step, CHANGE failure → the raw fail level, and the class gate on `CategoryType.FOURTH_CLASS_GROUP`. Deferred `TODO(G19)`: `UNTRAIN` (no client button), olympiad/sell-buff gates (unmodeled systems). **AdminEffects GM sweep landed — G19 ✅ COMPLETE**: `//setteam`/`//setteam_close`/`//clearteams` (a real `Player.team` behind UserInfo's SLOTS byte 3 and CharInfo's team byte — two more stubbed-zero fields made real; NPC team display TODO(G19), the port's NpcInfo lacks the block), `//settargetable` (`AdminFlags.untargetable`, gated in `handle_action` — Java toggles the GM themselves), `//para`/`//unpara`/`//para_all`/`//unpara_all` (`AdminFlags.paralyzed` ORed into `is_blocked_from_actions`/`is_movement_disabled` beside the buff flags, PARALYZE/FLESH_STONE visuals via the `//ave_abnormal` pin store), `//bighead`/`//shrinkhead`, `//playmovie` (`ExStartScenePlayer` preview; MovieHolder freeze bookkeeping TODO(G19)), `//event_trigger` (`OnEventTrigger` 0xCF fan-out), `//set_displayeffect` (`ExChangeNpcState` broadcast; state not stored — TODO(G19)), and the `//invis`/`//vis` alias family onto `//hide`. One Java-fidelity fix along the way: `AdminData.hasAccess` **auto-grants unlisted commands to the highest access level** (the dist xml genuinely lacks `admin_settargetable`), so the port's existence gate now falls back to that instead of "does not exist". Remaining out-of-milestone tail: `StatUp` (all G24 Territory-War Benefactions), `WeightLimit` (needs a weight model) |
| Game  | G20 Combat breadth                                          | ✅ **ranged attacks landed** (plan: [PLAN_G20_RANGED.md](PLAN_G20_RANGED.md)): bows/crossbows now need **ammunition** (arrow/bolt matched by crystal grade, auto-equipped to LHand via a dedicated `equip_ammunition` — the ordinary equip path refuses `Etc` items *and* would displace the two-handed bow), spend **MP** per shot, consume one arrow, and arm a **reload delay** (`900000/pAtkSpd`) shown as a red `SetupGauge`; out-of-arrows / not-enough-MP refuse the swing. Bow *range* already worked (pAtkRange 500 via G14). Survey note: `PhysicalAttack` skills and root/immobilize were already done (earlier slices + G19). **Multi-hit melee landed** (plan: [PLAN_G20_MELEE_VARIANTS.md](PLAN_G20_MELEE_VARIANTS.md)): the `Attack` packet now carries several hits (it hard-coded "0 additional"), **dual** weapons strike twice at half damage, and the **polearm sweep** hits extra targets in the weapon's radius (66 vs a sword's 40) and 120° arc — gated on `ATTACK_COUNT_MAX`, a *stat* set by Polearm Mastery 216 (`HitNumber` 5), not on the weapon type. **PvP kill consequences landed** (plan: [PLAN_G20_PVP_KILLS.md](PLAN_G20_PVP_KILLS.md)) — **G20's gate is now met**: killing a player moved nothing before (`player_do_die` had a literal `let _ = killer_oid`). Now Java's three branches — lawful PvP kill → `pvp_kills++`; positive-reputation first offence → reset to 0; otherwise karma (`calculateKarmaGain`, 720 rising to a flat 43200 past 180 PKs) + `pk_kills++` — with the PVP-zone "do nothing" short-circuit. Also found & fixed: the death XP penalty applied unconditionally, where Java skips it inside PVP/siege zones. **Over-hit landed** (plan: [PLAN_G20_OVERHIT.md](PLAN_G20_OVERHIT.md)): a killing blow from an `<overHit>` skill (59 learnable — Triple Slash, Sonic Storm…) banks its excess damage and pays it as bonus XP, capped at 25% of the share, with the "Over-hit!" notice. Note `<overHit>` is an **effect** param, not a skill field — the first read had it at skill level and only the real-datapack parse assertion caught it. **Duels (1v1) landed** (plan: [PLAN_G20_DUELS.md](PLAN_G20_DUELS.md)) — the last feature G20 names: challenge → ask → accept/decline → 5 s countdown → fight → end on death/surrender/timeout/separation, with the `canDuel` gates and the five `ExDuel*` packets. **A duel never kills** — the losing blow is capped at 1 HP and ends the duel, so no death penalty, karma or PvP counters move. Party duels need an arena instance (`TODO(G27)`) and are refused. **Death item drops landed** (plan: [PLAN_G20_DEATH_DROPS.md](PLAN_G20_DEATH_DROPS.md)) — a PK past `MinimumPKRequiredToDrop` killed by a player scatters inventory (the karma penalty, *not* general looting: a clean victim keeps everything), while a **monster** kill uses the gentler `Player*` rates. Adena/quest items never drop; equipped items unequip first and use the equip/weapon percentages; arena deaths and GMs are exempt. **G20 is complete** — `SHOTS_BONUS` is provably dead on this dist (zero items declare `reducedSoulshot`), karma decay is blocked on an absent `KarmaData` table, and party duels need G27's instances |
| Game  | G20.5 Recommendations                                       | ✅ **complete** — the row had been left at ⏳ while the work was already done, found when picking the next milestone after G29. Verified against the gate rather than the code: *"a given rec survives relog"* — `character_reco_bonus` load/store, pinned end-to-end by `char_persistence::recommendations_persist` against the real schema; *"the counters reset daily"* — `handle_daily_reco_reset` zeroes `rec_left` and decays `rec_have` for online players and issues `DbCommand::ResetRecommends` for offline ones, scheduled at boot by `reco::schedule_initial_daily_reset` and self-rescheduling 24 h out. `RequestVoteNew` (0x7B) is dispatched to `reco::handle_request_vote_new` with Java's guards (no target / out of recommendations / invalid target), and `RecoGiveTask` grants recommendations-to-give over time. 15 tests in `reco`. |
| Game  | G21 NPC AI & world-content breadth                          | ✅ **NPC skill casting landed** (plan: [PLAN_G21_NPC_CASTING.md](PLAN_G21_NPC_CASTING.md)) — the first of G21's four gate clauses. Mobs could only swing before: **4831 NPC templates carry a castable skill and none ever used it** (73% of those attachments are fully covered by ported effects, 9% partially). `AISkillScope` bucketing from the tail of `NpcData.parse` is now built once at load into `NpcAiSkillIndex` — **the `else if` ladder's order is load-bearing** (a *continuous* skill takes the first arm and never reaches ATTACK even when it also carries a damage effect). Needed a real `Skill.is_continuous`: the Rust `OperateType` collapses `A1`/`A2` into `Active`, so continuity is now read from the raw `operateType` (`A2..A6`/`DA2..DA5`) rather than proxied off `abnormal_time`. `<ai type>` is parsed (`AiType`; this dist has 402 **MAGE**, 220 ARCHER, 3163 BALANCED — a mage casts every think, skipping the `hasSkillChance()` roll and the stand-still requirement). `npc_cast.rs` runs Java's ladder — heal → self-buff → immobilize a moving target → mute a casting one → short/long range → general — hooked into `think_attack` *before* the chase/swing tail. The cast rides the **existing shared** launch/finish path, which needed two player assumptions fixed: `mp_consume` would have been **billed twice** (start now charges only `mp_initial_consume`), and `effects.rs` hard-`expect`ed a `Player` on the caster in 5 places, so **any NPC cast panicked the server** — only the one test that ran a cast end-to-end through the real tick loop caught it. Narrowed with `TODO(G21)`s: `skillTargetReconsider` (no faction plumbing → heal/buff target the caster), the ARCHER kite, and the SUICIDE/RES buckets (nothing declares `isSuicideAttack`; no resurrect effect ported). **Guard PK aggro + faction calls landed** (plan: [PLAN_G21_GUARD_AGGRO.md](PLAN_G21_GUARD_AGGRO.md)) — the second gate clause. `<clanList>` was **dropped entirely** by the NPC parser, so every mob fought alone: now 3760 templates carry factions (4569 `<clan>` entries; `ALL` on either side matches everything) and 82 carry `<ignoreNpcId>` lists. **Town guards** (186 `Guard` templates) seed hate on any player with `reputation < 0` inside a **hardcoded 500** — Java's bare literal, *not* the template `aggroRange` — and **regardless of `isAggressive`**; a lawful player is ignored at any distance. **Corrected 2026-07-19:** this slice originally recorded that guards are flagged *passive* in the datapack — they are not (all 186 carry `isAggressive="true" aggroRange="450"`), and because the test fixture hardcoded the same wrong value, nothing caught that the *generic* aggro scan (gated only on `is_aggressive`) was seeding hate on every lawful player within 450 units and **guards were killing them on sight**. Java reaches that scan for guards too, but every candidate must clear `isAggressiveTowards` → `isAutoAttackable`, which for an NPC attacker is true only via `attacker.isMonster()` — a `Guard` is an `Attackable`, not a `Monster`. The generic scan is now `is_monster()`-gated, leaving the reputation rule as the only way a guard aggros a player. **Faction calls** drag idle clan-mates within `clanHelpRange + collision` into the fight, with three separately-tested gates: only if the target **actually attacked this NPC** (Java's `getAttackByList`; proxied by a non-zero aggro `damage` — without it merely being *noticed* pulls the whole camp), only **idle/active** mates answer, and `ignoreNpcId` beats a shared clan. Also had to let `Guard` into the AI at all: `think()` gated on `is_monster()` and `Guard` isn't in that subtree, though Java's `Guard extends Attackable` runs the same `AttackableAI`. **Raid-boss persistence landed** (plan: [PLAN_G21_BOSS_PERSISTENCE.md](PLAN_G21_BOSS_PERSISTENCE.md)) — **G21's gate is now met**. `dbSave` was parsed by nobody, so all **225** raid-boss spawns (`RaidbossSpawns.xml`) were placed like static ones: every restart handed players a fresh full-HP boss and wiped any pending respawn timer. Ported `DBSpawnManager`/`npc_respawns` — a boss now keeps its **live HP/MP** and its **absolute respawn due time** across a restart. **The ownership split matters**: Java's `spawnNpc` hands a `dbSave` spawn to `DBSpawnManager` instead of spawning it (and only if `!isDefined(id)`), so the static pass now defers them into `pending_boss_spawns` and `resolve_boot` settles them when the DB rows arrive — keeping boot asynchronous while preserving "DB wins" (a test pins that the static pass places *no* dbSave boss, or the restore would double-spawn). Three cases: still-on-timer → scheduled not spawned; elapsed/alive → spawned with stored vitals; no row → full + insert. Guards: a dead row's `currentHp = 0` is **not** restored (it would spawn a corpse) and an over-max stored value clamps. Writes on spawn, at corpse decay (banking the absolute due time so a restart mid-window resumes the wait) and on shutdown. SQL verified against the shipped SQLite schema via `PRAGMA table_info` + a round-trip, not just test doubles. Note **any new unprompted `DbEvent` has two boot-event skip-lists to update** (lib + `char_persistence`) — missing them failed 8 tests. **Minions landed** (plan: [PLAN_G21_MINIONS.md](PLAN_G21_MINIONS.md)) — `MinionList`. The parser deliberately *skipped* minion refs (they'd be mistaken for template starts), so all **460** leaders stood alone; a full world spawn now places **3289** escorts from 962 `<minions><npc>` entries. Rules that invert easily, each tested: a **non-raid** leader's minions never respawn, and a `CustomMinionsRespawnTime` of **0 beats the raid default** (4 ids use exactly that); only a **raid** leader's death clears its escort, so killing the big mob in an ordinary camp doesn't evaporate the camp; pack aggro is asymmetric (leader struck = 10, minion = 1, ×10 for a raid). **A real perf bug surfaced only in e2e**: counting a leader's live minions via a full `world.objects` scan per spawn (~3289 × ~39k) made boot so slow the game server missed its login-server registration and the e2e failed at *login* — replaced with the per-master roster Java keeps (`_spawnedMinions`). Two test-only hazards recorded: `add_test_npc`'s `NPC_OID` **is** `FIRST_NPC_OBJECT_ID`, so a runtime-spawned minion overwrote the hand-placed leader; and ambient NPC idle `SocialAction` (0x27) wasn't in `e2e_create`'s skip-list — **the likely cause of that test's long-noted intermittent failures**, now fixed (4/4 consecutive passes). **EffectZones landed** (plan: [PLAN_G21_EFFECT_ZONES.md](PLAN_G21_EFFECT_ZONES.md)) — zones that periodically cast on players inside them (Blazing Swamp fire, Sea of Spores poison, Hot Springs Haste/Focus/Might). **Picked by behaviour, not count**: `ConditionZone` leads the census at 1080 but **1073 are `NoBookmark=true`** — a later-chronicle feature absent from Interlude — so it's ~99% inert, while the 218 `EffectZone`s (204 with skill lists) are live. Their skills were already-ported effects (`DamOverTime`, stat mods). Required **per-zone `type=` parsing**, which the loader had explicitly deferred (it mapped filename→kind and couldn't read the mixed files); a zone whose type isn't ported is now skipped outright rather than mis-filed. **Bonus: that recovered 20 zones missing from the world entirely** (+7 Peace, +7 NoRestart, +6 Pvp in the previously-unloadable mixed files) — total zones 605 → 843. **27 zones declare `targetClass="Npc"` and cast on nobody** (Java tracks only NPCs as inside, then the tick requires `isPlayer()`) — modelled explicitly so they stay inert; I had the default inverted at first and the dist parse test caught it. Runtime differs from Java by design: instead of per-zone tasks needing a live characters-inside set, one 1 s sweep groups players by occupied zone and fires each on its own `reuse` — chance rolled once per creature (not per skill), `initialDelay` honoured, and Java's affected-level guard means a buff zone grants its buff **once** rather than re-casting forever. **NPC regeneration landed** (plan: [PLAN_G21_NPC_REGEN.md](PLAN_G21_NPC_REGEN.md)) — `doRegeneration` ran for **players only**, so every NPC was frozen at whatever HP it was left on: `base_hp_reg`/`base_mp_reg` were parsed and read by nothing, and a raid boss whittled down across sessions never recovered a point. **14855** templates declare an `hpRegen` (only 58 zero; 8.5 is the commonest). Chosen over the remaining zones after checking `default_enabled`: `DamageZone` is 13 live of 35 and `SwampZone` 2 of 20 — the rest are siege-gated castle traps, so 15 zones total vs 14855 templates. **The NPC formula is much shorter than the player one and that's Java, not a narrowing** — levelMod, CON/MEN and the sitting/standing/running multipliers all sit *inside* `isPlayer()`, so an NPC regenerates its raw template value × the raid-or-normal config multiplier (both 100% here; the raid branch is tested by overriding it). **Regen runs during combat** — Java's task never checks an in-combat flag, which is what makes a long fight vs a high-regen boss a DPS race; there's a test named for it so it isn't "fixed" later. The HP-bar broadcast fires only on an actual change, else every full-HP NPC would emit a packet every 3 s. **NPC pathfinding landed** (plan: [PLAN_G21_NPC_PATHFINDING.md](PLAN_G21_NPC_PATHFINDING.md)) — `Creature.moveToLocation` is shared between players and NPCs in Java, but only the player half was ported (G7.85): `move_npc_to` built a straight-line move with **no geodata consultation at all**, so every chase, drift-return and random walk went through walls. The path worker was already built for this — `PathRequest.playable` is documented "one pass for AI" and had never been called with `false`. Now: destination clamp via `get_valid_location` (with Java's >3000 and intentional-fall skips), the **NPC takes the geodata-corrected z** (`if (!isPlayer()) z = destiny.getZ()` — a player keeps its client's z, a mob doesn't), and a clamp shortfall >30 hands off to the worker against the *original* destination. The reply path was player-only and looked up `clients[client_id]`; client-facing sends are now gated on `has_component::<Player>` rather than a sentinel id that could collide with a real client. Two hazards handled: the AI re-issues a chase every 1 s so there's **one outstanding request per mob**, and that guard is only safe because the worker replies to every request and `PathWait` clears **before** the no-route branch returns — otherwise one unroutable target would freeze a mob permanently (tested). Tests run against **real dist geodata**, with blocked/clear lines probed from Giran square first. **`skillTargetReconsider` landed** (plan: [PLAN_G21_TARGET_RECONSIDER.md](PLAN_G21_TARGET_RECONSIDER.md)) — slice 1 shipped NPC casting with heal and buff hard-wired to the caster for lack of faction data; slice 2 added it. **1040 NPCs carry a buff-bucket skill and 305 a heal-bucket one**, so a pack's healer now tops up whoever is worst off and a buffer buffs its mates. Bad skills draw from the aggro list; good skills from faction-mates + self, with heals sorted by lowest HP%; the heal chance now rolls against the *chosen target's* HP. **Deliberate deviation**: Java's good-skill candidate set is every visible creature and its auto-attackable filter sits *inside* the `isContinuous()` branch — a heal isn't continuous, so as written a mob would heal the **player** fighting it; scoped to the caster's faction instead (does less than Java, never more), with a test pinning it. **This surfaced a latent slice-1 bug**: `check_skill_target` encoded Java's `isAutoAttackable(caster)` test as `target_oid != npc_oid`, which was indistinguishable while buffs were self-only and silently blocked *every* faction buff once reconsider landed — a narrowing that is currently indistinguishable from the real rule becomes a bug the moment the thing it was narrowed around arrives. Survey note: **`FenceData` is a single fence named "demo"** and not worth porting on this dist. **`DamageZone` + `SwampZone` landed** (plan: [PLAN_G21_DAMAGE_SWAMP_ZONES.md](PLAN_G21_DAMAGE_SWAMP_ZONES.md)) — the last zone types with live content; both reuse slice 5's parser and sweep. Zone census now **898** (Damage 35, Swamp 20). **No DamageZone in this dist declares `damageHPPerSec`**, so all use Java's field default of **200**/tick — a number that appears nowhere in the datapack, so reading only the XML would suggest they do nothing. `DamageZone`'s default reuse is 5000 ms (not `EffectZone`'s 30000); the parser corrects for it. `SwampZone` multiplies move speed (0.2 here): Java re-reads the zone inside `SpeedFinalizer`, the port caches it on `Speeds` and refreshes on the enter/exit edges, then recomputes + rebroadcasts `UserInfo` like Java's `broadcastUserInfo()`. **Castle traps are gated twice** — only while that castle's siege runs, and players *defending that castle* are skipped; without the second rule a garrison would cook itself on its own defences during the siege it's fighting (both tested). **Walker routes landed** (plan: [PLAN_G21_WALKER_ROUTES.md](PLAN_G21_WALKER_ROUTES.md)) — **G21 is complete**. 13 routes drive Giran's porters, scribes and the running boy, plus Gordon on a 67-node patrol; only `cycle` and `back` styles occur here. Java hangs a `ScheduledFuture` off each arrival; the port keeps `WalkState` on the NPC and drives a 1 s sweep with two phases — travelling (a `Movement` in flight) and waiting (serving the node's `delay`). **Splitting them matters**: banking the delay before the leg starts would let travel time eat the pause. Java's `back` arithmetic steps back **two** on overrun (the index was already past the end), landing on the second-to-last node; the test pins `0→1→2→1→0→1→2` because an off-by-one makes a walker bounce on the spot. **Verification gap closed**: `tests/user_info_packet.rs` had stopped compiling after the previous slice added a `Speeds` field — I'd only been running `--lib`/`char_persistence`/`e2e_create`. Fixed, and this slice was verified with a plain `cargo test -p gameserver` across **all 8 targets (749 tests)**. G21's remaining items are all blocked or empty on this dist: `HtmCache` (caching only), `CreatureSeeTaskManager` (needs a script engine), `FenceData` (one fence named "demo") **NPC skill cooldowns never applied** (fix, plan: [PLAN_G21_NPC_SKILL_REUSE.md](PLAN_G21_NPC_SKILL_REUSE.md)) — found by the G29 `Creature`-vs-`Player` sweep's last probe. `set_skill_reuse` writes through `if let Some(Reuses)` and players get that component at load, but **NPCs never did**, so the write was a silent no-op; `npc_cast::check_use_conditions` reads the same component and treats an absent one as *ready*. Both halves fail open, so a mob could re-cast a 10 s skill as fast as its AI ticked — the reuse plumbing was written and called correctly from the start, it just wrote into a component nobody had attached. Fixed by attaching `Reuses` on **first use** (only NPCs that cast pay for the map; the world holds ~34.9k). Two tests, because recording and enforcing are separate failures that were broken by the same cause. servitor_tests 109 → 111; npc_cast/raid/boss/combat/skills re-run clean. |
| Game  | G22 Quest & script breadth                                  | 🔨 **Dwarf first-class transfers landed** (plan: [PLAN_G22_DWARF_CLASS_TRANSFER.md](PLAN_G22_DWARF_CLASS_TRANSFER.md)) — G22 depended on G17, and the class-transfer quests are what G17's `setClassId` unblocked. `DwarfBlacksmithChange1` (→ Artisan 56) and `DwarfWarehouseChange1` (→ Scavenger 54) share one implementation, since the two Java scripts differ only in NPC list / target / proof / talk-category; both call the G17 mechanic, so village-master transfers and `//setclass` now share code. **A Java quirk kept deliberately**: the fourth-class refusal hard-codes the *first* NPC's page id regardless of who you're talking to — that looks like a bug, but only the first NPC of each set ships a `-12` page, so "fixing" it would produce a blank window. A dist-page-existence test **failed on its first run** (the pages live under `data/scripts/village_master/`, not `data/html/`), which would have meant a blank window at the exact moment of a class change. **Elf/Human first-class transfers landed** (plan: [PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md](PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md)) — unlike the Dwarf pair these serve **two races from the same NPCs** (Human Fighter 0 / Elven Fighter 18; Mage 10 / Elven Mage 25). **The `from_class` half of each match is load-bearing**: Java matches `(classId == TARGET) && (getClassId() == SOURCE)`, and dropping the source check would let a Human take Elven Knight from the same NPC — there's a test asserting exactly that is refused with nothing consumed. Java's nine near-identical `else if` blocks compress to a `(to, from, proof, first_page)` table because each target owns **four consecutive pages** in a fixed order; the page-existence test then sweeps every target's block across every NPC (9×9 + fixed pages), which is what makes the compression safe. **DarkElfChange1 landed** (plan: [PLAN_G22_DARK_ELF_CLASS_TRANSFER.md](PLAN_G22_DARK_ELF_CLASS_TRANSFER.md)), completing the racial first-occupation set — **and fixing a second class-corruption bug**: `QuestCtx::set_class_id` still had the unconditional `base_class_id = class_id` that G17 slice 6 fixed in `//setclass`, so a *quest-driven* transfer while on a subclass would rewrite the base class. All three paths (GM command, village-master script, quest) now share `subclass::set_class_id`. I'd recorded the "every existing writer becomes suspect" lesson last milestone, fixed one writer, and moved on — finding the second by accident is the cost of not enumerating them. Three ways DarkElfChange1 differs from its siblings, each silent if mis-ported: Java already writes it as a **table** and the event is the **row index** not a class id; the page order is `lowNoProof, low, noProof, done` (opposite pairing to ElfHuman's); and the pages are **`.html`** not `.htm`. Also honours `isSubClassActive()` → refuse, newly expressible after G17. **FirstClassTransferTalk landed** (plan: [PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md](PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md)) — the seven newbie-village headmasters, who (per Java's own header) *only talk about* transfers. Two conventions differ from every other village-master script: pages use an **underscore** (`30026_fighter.html`) and `.html`. **The page availability is asymmetric and IS the logic**: the Human fighter-guild master ships no `mystic` page and the temple master no `fighter`, so a mage at the fighter guild gets `no.html` rather than a constructed filename that would 404 — a test asserts those three absences so the branching can't drift to a "sensible" symmetric version. Also strengthened the main test: it first only checked the reply was non-empty (which would pass while serving the *wrong* page), now it compares byte-for-byte against the dist file through `strip_htm` + `%objectId%`. **The entire first-occupation group is done — 8 of 16 village-master scripts.** **Dwarf second-class transfers landed** (plan: [PLAN_G22_DWARF_SECOND_CLASS.md](PLAN_G22_DWARF_SECOND_CLASS.md)), opening the `*Change2` group: Artisan→Warsmith and Scavenger→Bounty Hunter. **Three differences from `*Change1`**: level **40** not 20; **three** proof items required and all consumed — Java's `hasQuestItems(a, b, c)` is an **AND**, and reading it as "any" would let a player transfer on one mark (tested with two of three); and a **C**-grade coupon reward. Structural quirk: **every** page is hard-coded to the *first* NPC's id whichever of the eight masters you talk to (the `*Change1` scripts did this only for the fourth-class refusal) — the dist ships one 12-page set per script, and the test asserts the other masters ship nothing, so it can't be tidied into per-NPC pages that would 404. **Orc + Dark Elf second-class transfers landed** (plan: [PLAN_G22_ORC_DARKELF_SECOND_CLASS.md](PLAN_G22_ORC_DARKELF_SECOND_CLASS.md)) — they look like siblings and differ in **four** ways, each silent if one is ported by copying the other: the bypass event is the **class id** (Orc) vs the **row index** (Dark Elf); `.htm` vs `.html`; page order `low, lowNoProof, done, noProof` vs **`lowNoProof, low`, noProof, done**; and — the real trap — Orc pays 15 C-grade coupons while **DarkElfChange2 pays nothing at all** (verified by counting: `grep -c giveItems` → Orc 4, Dark Elf 0; copying the Orc branch would have handed out 15 free coupons per transfer). The page owner also isn't the first NPC for Dark Elf — it's **30474, the third**. Process fix: the transfer test failed on first run for the **fourth consecutive slice**, always the same fixture gap, so the quest fixture now registers the whole class range `0..=57` instead of an enumerated list. **Elf/Human second-class transfers landed** (plan: [PLAN_G22_ELF_HUMAN_SECOND_CLASS.md](PLAN_G22_ELF_HUMAN_SECOND_CLASS.md)), closing the `*Change2` group with its three widest scripts — Fighter (10 targets, 477 Java lines), Wizard (5), Cleric (3). **The finding is that they are uniform**: after a slice spent on the four silent ways Orc and Dark Elf differ, I went looking for the same axes here and there are none — same level 40, three-proof `AND`, 15 C-grade coupons, `.htm`, class-id bypass event, and `low, lowNoProof, done, noProof` page order — so the port has **no per-branch code path**, just one `Spec` table. Worth stating, because the previous slice is exactly the prior that would push you to invent per-branch handling that isn't there. What *does* differ is the greeting gate: each script serves a Human and an Elven line from one NPC set through a **different pair of race categories** — `HUMAN_FALL`/`ELF_FALL` (fighter), `HUMAN_MALL`/`ELF_MALL` (mystic), `HUMAN_CALL`/`ELF_CALL` (cleric). Three near-identical names; the wrong one greets the right player with the class-mismatch page. The **`from_class` half of each row is load-bearing and worse here than in Change1**: all ten Fighter targets hang off one NPC, so matching on the target alone would let a Human Knight take **Temple Knight**, an Elven Knight's class, from the same master — tested by handing a Human Knight exactly those marks and asserting nothing happens and nothing is consumed. Two Java behaviours preserved that read as bugs: every page is hard-coded to the *first* NPC's id (the dist ships one page set per script; a test asserts the other masters ship nothing), and `THIRD_CLASS_GROUP` is checked *before* the source-class match. All 5 tests passed on **first run** — the first slice in five to do so, which is the payoff from slice 6 replacing the quest fixture's enumerated class ids with the full `0..=57` range; fixing the pattern rather than the instance held. **AllianceMaster landed** (plan: [PLAN_G22_ALLIANCE_MASTER.md](PLAN_G22_ALLIANCE_MASTER.md)) — 67 Java lines, the smallest of the 16, and **the village-master group is now complete at 16 of 16**. The whole script is one guard: `onTalk` always opens `9001-01.htm`, and `onEvent` echoes the requested page back unless the player has no clan (`9001-04.htm`). **The asymmetry is the script and is easy to "fix" away**: the menu is explicitly excluded from the gate, so a clanless player *does* see both buttons and only learns they can't use them after clicking — gating `onTalk` too, which reads tidier, would change what retail shows; a 6-case test pins both halves. Pages are numbered against a **virtual NPC id** (`9001-NN.htm`, as `ClanMaster` uses `9000`) and no real master ships one, asserted so it can't be "corrected" into a per-NPC name that would 404. **Stated plainly because it would otherwise be rediscovered as a bug: this makes the dialog work, not alliances.** Both buttons post `create_ally`/`dissolve_ally`, `VillageMaster.onBypassFeedback` verbs that are **not routed here** — the alliance system is G18, where `ally_id`/`ally_name` currently exist only as a DB column list and a "when the alliance system lands" comment. I checked the failure mode instead of assuming: unrouted `npc_` verbs hit the router's fallback `warn!` and drop, so the buttons are inert but greppable at runtime, and a `TODO(G18)` names both verbs. This matches how `ClanMaster` already ships with `learn_clan_skills`/`multisell` unrouted, but it is the same shape as the dead-button bugs this port keeps hitting (`Chat <page>`, the race-track gatekeeper), so it is recorded as a known gap. Added `QuestCtx::has_clan` alongside `is_clan_leader`. **The Elven first-occupation quests landed** (plan: [PLAN_G22_ELVEN_PATH_QUESTS.md](PLAN_G22_ELVEN_PATH_QUESTS.md)), opening G22's quest body — and the slice was **chosen by a gap the previous eight created**: all 16 village-master scripts were done, and every one consumes a proof item **no quest in the port produced**, so the transfers were reachable only via `//setclass`. `Q00406_PathOfTheElvenKnight` and `Q00407_PathOfTheElvenScout` award the Elven Knight Brooch (1204) and Reisa's Recommendation (1217), making the elven half of `ElfHumanFighterChange1` reachable in normal play. **The finding: Q00406 deliberately ignores `RateQuestDrop`.** It hand-rolls `getRandom(100) < chance` + plain `giveItems` instead of calling `giveItemRandomly`, which multiplies *both chance and amount* by the rate — so reaching for the port's faithful `give_item_randomly` helper would have silently scaled a drop Java leaves alone. Caught only by diffing against `Q00303_CollectArrowheads`, which *does* call the helper; the two look identical in shape and differ in exactly this. Test pins it with `RateQuestDrop = 3.0` → still one topaz per kill. **Generalised: check whether the Java quest calls the helper or rolls its own before picking the Rust primitive — they are not interchangeable.** Q00407's tag mechanic needs **both** hooks: `onAttack` stamps the mob's script value with the attacker's object id and `onKill` pays only on a match — porting one alone fails silently in opposite directions (kill-only never matches; attack-only leaks the tag). Tested both ways. Page conventions: extensions are **mixed inside one quest** (`.htm` pre-accept, `.html` after), and Prias ships `-01`/`-02`/`-04` but **no `-03`**, which Java never names — asserted so the gap isn't helpfully filled in, the same shape as `FirstClassTransferTalk`. Also collapsed a Java three-way level branch awarding identical exp/sp in all three arms (commented, so it doesn't read as a dropped case). The chain test failed once on first run: I asserted the quest record was gone after `exitQuest(false, …)`, but a one-time exit keeps it **COMPLETED** — deleting it would let the quest repeat. Assertion corrected, not the code. Added `QuestCtx::social_action`. 14 quests ported. **Path of the Warrior + Path of the Rogue landed** (plan: [PLAN_G22_PATH_WARRIOR_ROGUE.md](PLAN_G22_PATH_WARRIOR_ROGUE.md)), awarding the Medallion of Warrior (1145) and Beziques' Recommendation (1190) — `ElfHumanFighterChange1` now has four of its five proofs. **The finding: the same `ItemChanceHolder` type, two different denominators.** Q00406 rolls `getRandom(100) < chance`; Q00403 rolls `getRandom(REQUIRED_ITEM_COUNT)` — i.e. **`getRandom(10)`** — so a "chance" of 2 means 2% there and **20%** here. Reading Q00403's table as percentages (the obvious assumption, same type used that way one quest earlier) would have made every Spartoi bone **10× too rare**, turning a ~13-kill stage into ~125. **The denominator is a property of the call, not of the table** — the same shape as the previous slice's `giveItemRandomly` finding: a quest's drop maths is not inferable from the types it uses, so read the roll. Q00401's spider stage meanwhile has **no chance roll at all** — the weapon gate, not a rate, is what makes it slow. Quests 401/403 share a byte-identical `onAttack` state machine — the "kill it solo with the quest weapon" tag (0 → 1 on the right weapon, → 2 terminal on a weapon change *or* a second attacker; `onKill` pays only on 1) — now factored into `scripts/quest_common.rs` since 402/415 use it too. Both hooks load-bearing in opposite directions, as in 407. Two framework pieces: **`Npc.vars`** (Java's `getVariables()`, needed for `lastAttacker`) — shape chosen after checking breadth, 11 quests use it under 6 keys, so a generic map beats six `spoiler_object_id`-style named fields, and an empty `HashMap` doesn't allocate; and **`QuestCtx::npc_say_to_player`**, because the Cat's Eye Bandit taunts its attacker with `sendPacket` but broadcasts its death line — using the existing broadcasting `npc_say` would have leaked the taunt to bystanders. 5 tests, first-run green. Q00401's `/10` roll is pinned deterministically (force the roll to 4 → no drop; `getRandom(100) < 40` would drop), but **Q00403's is deliberately statistical**: a forced roll ignores the bound, so no forced test can tell `/10` from `/100` — it asserts the rate instead (chance 8 = 80% caps 10 bones within 40 kills; 8% essentially never), re-run 10× to confirm it isn't flaky. 16 quests ported. **Path of the Human Knight landed** (plan: [PLAN_G22_PATH_HUMAN_KNIGHT.md](PLAN_G22_PATH_HUMAN_KNIGHT.md)) — 629 Java lines, the widest of the Path family, taken alone because it **completes the proof set for `ElfHumanFighterChange1`**: all five targets are now reachable in normal play, closing (on the fighter side) the gap opened three slices ago. Structurally unlike its siblings: **six independent sub-quests of which you need three** — six officers each trade a badge for N trophies for a Coin of Lords — so most of those 629 lines is one block six times, ported as a `BRANCHES` + `DROPS` table. **The completion path forks on the coin count and the 6-coin case is the odd one:** 3 coins and 4–5 coins each open a prompt whose confirm button (`30417-13`/`-14`) does the awarding, but **6 coins completes immediately inside `onTalk`** with no confirmation. It reads like an oversight; the dist backs it up (`-12` is a completion page, not a prompt), so it's kept and tested both ways — tidying the asymmetry would either add a prompt nobody can answer or silently drop the 6-coin completion, and the player who did all six sub-quests is exactly the one who'd hit it. The confirm handlers also sweep **all** leftover badges/trophies (a player may have part-finished other sub-quests); the 6-coin path takes only coins and the mark, correct there since every badge was already spent. Quirks verified rather than assumed: the quest **never calls `setCond`** — not once in 629 lines, so the quest window shows one step throughout (confirmed by grep, not inferred from the sections I read); Vasper's extensions **alternate** (`-01..05`/`-07`/`-08` are `.htm`, `-06` and `-09..15` are `.html`) rather than splitting on a prefix, so the test asserts `30417-07.html` and `30417-06.htm` are *absent* to stop it being regularised; and Raymond alone ships six pages (an extra intermediate page shifts his later ones up by one), encoded per branch with a test that no other officer has a `-06`. **Two of the six trophies have no chance roll at all** — easy to miss across six near-identical blocks, so the table stores `Option<i32>` and ten unforced kills are asserted to yield exactly ten necklaces. 6 tests, first-run green. 17 quests ported. **Path of the Human Wizard + Path of the Cleric landed** (plan: [PLAN_G22_PATH_WIZARD_CLERIC.md](PLAN_G22_PATH_WIZARD_CLERIC.md)) — the Bead of Season (1292) and Mark of Faith (1201), so `ElfHumanWizardChange1` now has **2 of its 4** proofs. **Q00404 is four identical elemental branches with one exception.** Fire → Wind → Water → Earth each run the same token → collect → trinket bargain, repeating right down to the page numbering (`{npc}-01..04.html` for all four), so it ports as an `ELEMENTS` table. **The exception is Wind: its collectable is not a drop** — the feather comes from a dialog bypass on the Wasteland Lizardman, who sits outside the four-page scheme. A table-driven port assuming "collect ⇒ kill" would leave that branch permanently stuck; tested specifically. **Chance denominator is `/100` here** (`getRandom(100) < 20|80`) where 401/403 use `getRandom(10)` — the **third** distinct denominator convention in the Path family, checked per call site rather than carried over. **A test I deliberately did not write, and why:** no honest deterministic test for that denominator exists — `forced_rolls` ignores the bound, so `forced < chance` is literally the same predicate under either reading. Q00403's statistical trick doesn't transfer either: there the misreading made drops *rarer* (8% vs 80%), which 40 kills detect; here it would make them *more common*, and with a cap of 1 you get one Bernoulli per quest instance, so detection needs many worlds for little value. Pinned by a call-site comment instead. Better no test than one that appears to prove something it can't. **Q00405 has two things that break if normalised:** Simplon hands over a **stack of three** books where the other two givers give one each (and completion takes `-1`/all of his but `1` of theirs — treating them uniformly strands two or makes the check unsatisfiable; tested); and the cond-2 checks contain a **no-op `>= 0` term** — each giver re-checks all three counts but writes its own slot as `>= 0`, a placeholder for "the one I just handed over", so all three sites reduce to one predicate. Read literally it looks like a bug; it's only redundant, and collapsing is safe because the giver's own count is non-zero at that point. Praga's pendant drops with no roll at all. 5 tests, first-run green. 19 quests ported. **Path of the Elven Oracle landed** (plan: [PLAN_G22_PATH_ELVEN_ORACLE.md](PLAN_G22_PATH_ELVEN_ORACLE.md)) — the Leaf of Oracle (1235), `ElfHumanWizardChange1`'s **3rd of 4** proofs. **Taken alone rather than paired with 408 as planned**: I checked both quests' framework needs first — 408 uses none of `addSpawn`/`addAttackPlayerDesire`/`setMemoState`, 409 uses all three (23 call sites) — and carrying three new primitives plus a second 446-line quest is how sloppiness gets in. **The first quest in the port that spawns its own monsters:** Allana's re-enactment and Perrin's Tamil are ambushes conjured beside the NPC you're talking to and set on you. New framework: `QuestCtx::memo_state`/`set_memo_state` (Java stores it as the quest var `memoState` — confirmed in `QuestState.MEMO_VAR`, not guessed), `QuestCtx::spawn_attacker` (`addSpawn` + `addAttackPlayerDesire`, reproducing `Rnd.get(50,100)` per axis with independent sign), and `npc_ai::seed_attack` promoted to `pub(crate)`. **`memoState` is a second progress axis, not `cond`:** `cond` drives the client window, `memoState` is script bookkeeping, and they move in *opposite* directions — talking to Manuel empty-handed at `memoState == 2` rewinds it to 1 while pushing `cond` to 8. Collapsing them breaks the re-enactment restart path. The ambush tag is also **not** `quest_common`'s: it gates on one attacker with **no weapon check** and keys `firstAttacker`, so routing it through the shared helper would have silently added a weapon requirement — the test kills bare-handed to pin that. **The bug that cost the time was in the test fixture.** The memo test failed with a no-quest reply; instrumenting showed the talk arriving at **npc 27032, a lizardman**, instead of Priest Manuel — because `NPC_OID` and `world.next_npc_object_id` **both start at `FIRST_NPC_OBJECT_ID`**, so the first runtime spawn lands on a fixture NPC's object id and silently replaces it. No test had ever spawned at runtime before. Fixed in the shared `add_test_npc` (it now reserves each id against the allocator) rather than by shuffling my own ids — every future spawning quest would have hit it. All seven major modules re-run green after (quests 76, combat 33, npc 71, guard_aggro 13, admin 89, items 37, clans 16). 4 tests. 20 quests ported. **Path of the Elven Wizard landed** (plan: [PLAN_G22_PATH_ELVEN_WIZARD.md](PLAN_G22_PATH_ELVEN_WIZARD.md)) — the Eternity Diamond (1230), the last of `ElfHumanWizardChange1`'s four proofs. **The whole Elf/Human first-occupation tier is now self-sufficient**: both Change1 scripts (5 proofs + 4) are satisfiable entirely in normal play, which took nine quests. Three parallel errands, all required in any order, each the same four beats (introduction → charm → gated drop → gem), so it ports as one table. **The third errand is missing a step, and the dist proves it isn't a bug:** errands 1 and 2 swap introduction→charm in a **dialog event**, errand 3 does it inline in `onTalk`. Exactly the asymmetry one would "regularise" — until you count pages: Greenis and Thalia ship four each, **Northwind ships three**. There is no fourth page for an event to return, so adding one would 404 the moment a player takes that errand. Kept as `swap_event: Option<&str>` and the page test asserts `30423-04.html` does *not* exist. Same shape as `FirstClassTransferTalk`'s asymmetric pages and Q00407's missing `30426-03` — **when a script looks inconsistent, check whether the dist's page set explains it before normalising.** Like 402, `setCond` appears **zero** times in 446 lines (grepped, not inferred) — progress lives entirely in which items you hold. Denominator `/100`, as in 404/406. 3 tests, first-run green. 21 quests ported. **Path of the Palus Knight + Path of the Assassin landed** (plan: [PLAN_G22_PATH_DARKELF_1.md](PLAN_G22_PATH_DARKELF_1.md)), opening the **Dark Elf** tier — the Gaze of Abyss (1244) and Iron Heart (1252), so `DarkElfChange1` has **2 of 4** proofs. **Every drop in both is unrolled** — no `getRandom` in either `onKill`, so 13 kills is 13 skulls and 10 is 10 molars. Stated because 412/413 *do* roll and porting by analogy would add a chance that isn't here; the tests use no forced rolls at all, which is only a valid way to assert exact counts *because* the drops are unrolled. **Q00411 is one token walking a chain.** Java writes every branch as "hold this and **none** of the others" — seventeen times across three NPCs — which encodes one fact: exactly one token is in the bag at a time, since each hand-over takes before it gives. The port asks *which* token is held and matches once. The invariant is the quest's own design (checked transition by transition), not an assumption; the molars are the deliberate exception (they coexist with Leikan's note), pinned by a test that his page tracks the molar count while the token stays put. Two redundant Java terms collapsed with the reasoning recorded: a `silk >= 4` re-test inside `== 5`, and a genuinely **dead** Kalinta branch (`!has(SILK) && has(CARAPACE)` sits under `!(both)`, which already catches it) — the reachable state→page table is documented at the site so the equivalence stays checkable. **The page test earned its keep, failing on first run:** I'd asserted the `.htm`/`.html` split identically for both quests, but **410's accept page `30329-06` is `.htm` while 411's `30416-06` is `.html`** — the split point differs per quest even inside one race tier. Now asserted separately, with an explicit check that `30416-06.htm` does *not* exist. 5 tests. 23 quests ported. **Path of the Dark Wizard + Path of the Shillien Oracle landed** (plan: [PLAN_G22_PATH_DARKELF_2.md](PLAN_G22_PATH_DARKELF_2.md)) — the Jewel of Darkness (1261) and Orb of Abyss (1270). **The Dark Elf first-occupation tier is COMPLETE**: `DarkElfChange1` has all four proofs. Two races done, two to go. **Q00412 repeats quest 408's third-errand asymmetry — and twice makes it a convention.** Charkeren and Annika hand their tool over via a **dialog event**; Arkenia does it **inline in `onTalk`**, exactly as Northwind does in 408 where Greenis/Thalia use events. One occurrence looked like an oversight worth documenting; two independent quests in different race tiers makes it a datapack convention, so it's modelled (`tool_event: Option<&str>`) without further hedging, both branches exercised in one test loop. Arkenia also omits the `SEEDS_OF_DESPAIR` guard her siblings carry — kept, since adding it for symmetry would change who can start her errand. **Q00412's chance is an equality, not a threshold:** `getRandom(2) == 0` where every other Path quest uses `<`. Same 50% here, but not interchangeable — read as `getRandom(2) < 2` every kill pays. Unlike the `/10` vs `/100` cases this one **is** deterministically testable (a forced roll of 1 separates the readings), so there's a test. That's **four** distinct chance conventions in this family now: `/100`, `/10`, `== 0`, and no roll at all. **Q00413's succubus kill is a swap, not a drop** — it *consumes* a Blank Sheet to make a Bloody Rune, so the counts move in opposite directions and the cond tests **both** (sheets exhausted AND five runes). Modelling it as a capped drop would strand five sheets and never fire the cond; tested per-kill in both directions plus a sixth succubus proving no sheet means no rune. Talbot hands over **five** sheets in one `giveItems(..., 5)`, the same stack shape as Simplon in 405; and neither of 413's drops rolls while 412 rolls all three — conventions differ quest by quest even inside one tier. 4 tests, first-run green. 25 quests ported. **Path of the Orc Raider landed** (plan: [PLAN_G22_PATH_ORC_RAIDER.md](PLAN_G22_PATH_ORC_RAIDER.md)), opening the Orc tier with the Mark of Raider (1592). **Scoped down mid-slice** — planned as 414+416, but 414 carried two things worth doing carefully, so 416 follows rather than being rushed to hit the announced pairing. **Green blood is a rising summon meter, not a collection.** Java races the *held count* against the RNG: `blood <= getRandom(20)` gains one, otherwise it **wipes the stack and summons Kuruka onto the player**. At 0 blood the gain is certain, at 19 it's 5%, at 20 the summon is guaranteed. The blood is never handed in — and the tooth the quest wants drops from **Kuruka**, not the goblins, so porting the blood as an ordinary capped collection would make the quest **unfinishable**. Two tests pin the fork and the tooth source. Reuses `spawn_attacker` from slice 13; fidelity gap recorded (Java's `isSummonSpawn` animation + `addDamageHate` 999 vs our dominant-hate seed). **A branch dead at both ends — and the order I checked mattered.** Karukia's `07b` route sets `memoState=2`/`cond=5` and leads to events on NPC **31978**, who ships five pages here but is **registered nowhere** (`grep -rln 31978 data/scripts/` finds only this quest's file and its own orphaned pages). Separately, `30570-07.htm` offers **only** the `07a` button. Had *only* the serving end been missing, `07b` would be a trap — it consumes the map and all ten teeth but hands out no reports, the sole path to the reward, stranding the player permanently. Because the button doesn't exist either, there's no trap and the route ports verbatim at zero risk. Kept with a `TODO(dead)` and a test asserting **both** halves so nobody restores one end without the other. 5 tests, first-run green. 26 quests ported. **Path of the Orc Monk landed** (plan: [PLAN_G22_PATH_ORC_MONK.md](PLAN_G22_PATH_ORC_MONK.md)) — 652 Java lines, **the widest quest in the Path family**, awarding the Khavatari Totem (1615). **The weapon gate is the INVERSE of quests 401/403.** Those demand a specific quest weapon; this one demands `weapon == null || FIST || DUALFIST` — an Orc Monk fights unarmed, so **"no weapon" is the pass case**. Routing it through the shared `quest_common` tag would have flipped the entire quest: every bare-handed kill paying nothing and every sword kill paying. Needs the weapon's **type**, not id, so `QuestCtx::is_bare_or_fist_handed` was added; tested bare / sword / fist. Its tag variable is `Q00415_last_attacker` — a **third** name after `lastAttacker` (401/403) and `firstAttacker` (409). **The pouch stages take five kills, not four:** Java gives a trophy per kill and converts when the count is *already* 4, so the fifth kill fills the pouch. Reading it as "collect 4" leaves the pouch permanently unfillable — the conversion branch is never entered. The fourth pouch is the same shape over four mobs at three each, converting on the twelfth kill. Both tested per-kill. **Half the quest is unreachable — the same two-sided orphaning as 414.** `09c` opens an entire alternate ending through NPCs 31979/32056, with its own stages, a raid mob and its own reward hand-out — but `30587-09a.html` offers only the `09b` button and neither NPC is registered anywhere, leaving **13 orphaned pages**. Checked both directions again: had only the serving end been missing, `09c` would strand the player (it takes Rosheek's letter and gives no recommendation). Ported verbatim with `TODO(dead)` on the events, both dead kill handlers and the `memoState == 2` talk branch. **Two of two Orc quests now carry a fully orphaned alternate route — expect it in 416.** 5 tests, first-run green. 27 quests ported. **Path of the Orc Shaman landed** (plan: [PLAN_G22_PATH_ORC_SHAMAN.md](PLAN_G22_PATH_ORC_SHAMAN.md)) — the Mask of Medium (1631). **The Orc tier is COMPLETE**; three of four races done. Ported off groundwork from an aborted previous attempt, where I stopped rather than rush a 525-line quest needing unchecked framework — and two of the three gaps that analysis flagged turned out not to exist. **`ItemChanceHolder.count` is a cond SELECTOR here, not a quantity:** `if (item.getCount() == qs.getCond())`, with `chance` as a 0..1 probability for `giveItemRandomly`. Read `count` normally — as quests 403/406 use it — and grizzly bears drop **six** bloods a kill while the cond gate silently vanishes. Tested both sides (nothing at cond 1, exactly one at cond 6). **Fourth** distinct reading of this type in the family after `/100`, `/10` and `== 0`. **Two summon meters differing in the one way that matters:** the Durka parasites escalate exactly like 414's green blood (5 → 1-in-10, 6–7 → 2-in-10, 8 certain, success wipes the stack and conjures a spirit) — but **Java does not set this one on the player**, where 414 does. Needed `QuestCtx::spawn_near_npc` (with `spawn_attacker` refactored onto it); reusing `spawn_attacker` was the natural move and would have invented aggro the datapack never asks for. The test asserts the spirit is *not* in the aggro list. **What the groundwork got wrong, usefully:** `NpcSay` string parameters aren't needed (both such lines live inside the dead branch, so the live path never reaches them) and `getRandomPartyMemberState` reduces to the killer exactly as `q00303` already documents — a `TODO(G13+)` deviation, not new machinery. The `memoState` 100–110 branch is again **dead at both ends** (third Orc quest running: sole entry `30585-14.html` is offered by nothing, and 31979/32057/32090 are registered nowhere) — here **omitted rather than stubbed**, since half-porting it would carry dead memoState handling and a packet feature we lack. Also: the accept event is **`START`**, not `ACCEPT`; and `cond 10` is never assigned (9 → 11). 6 tests, first-run green. 28 quests ported. **Path of the Artisan landed** (plan: [PLAN_G22_PATH_ARTISAN.md](PLAN_G22_PATH_ARTISAN.md)) — the Final Pass Certificate (1635), opening the Dwarf tier. **The leader-tooth roll has a hole in it:** below 5 the kill pays *only* if one tooth is already held, so the first drops at 50% and the second at 100% — a flat "50% per tooth" reading is wrong in both directions (three forced-roll cases pin it). Consequence kept, not fixed: the `else` branch pays the second tooth **without** the `cond 2` check the other branch performs, so finishing that way leaves the quest window stale. Every downstream branch tests item counts rather than the cond, so the quest still completes — a cosmetic Java bug, ported verbatim. Also two routes to Kluto's letter differing only in whether `setCond(4)` chimes. **Dead at both ends for the fourth quest running** (`30527-08c` + NPCs 31956/31963/32052); omitted rather than stubbed, as in 416. **The dead-branch test caught my own error rather than the port's**: the first version scanned every file in the quest directory including the `.java` source, which of course names `08c` as a case label — the very handler being proven unreachable — so it fired on the evidence. Restricted to `.htm`/`.html`. 4 tests. 29 quests ported. **Path of the Scavenger landed** (plan: [PLAN_G22_PATH_SCAVENGER.md](PLAN_G22_PATH_SCAVENGER.md)) — 690 Java lines, the largest in the family. **ALL EIGHTEEN `Path of the *` quests (401–418) are now ported**, so every race's first-occupation script is proof-complete and reachable in normal play. **`dropChance` is documented as a 0..1 fraction and this quest passes `50`** — not 50%, but fifty times certainty, so **every qualifying kill drops** (`q00303` passes `0.4` for a real 40%, so the convention isn't in doubt). A datapack bug with a live effect; the dist is authoritative, so the port passes `50.0` and matches the shipped server. Writing the "obviously intended" `0.5` would halve the rate against retail — a silent divergence in the direction that looks like a fix. The test kills six tarantulas unforced and asserts six beads (at a real 0.5 it'd fail ~98% of the time). **Spoil-gated payouts** — the Scavenger's own mechanic: jars and beads pay only off a corpse that `isSpoiled()`, and `onAttack` separately disqualifies a mob whose spoiler *is* the attacker. Added `npc_is_spoiled`/`npc_spoiler_object_id`. Its npc var is `FIRST_ATTACKER`, a **fourth** spelling. **Two counters packed into one integer:** `memoStateEx(1)` is radix-packed — +10 per delivery (tens), +1 per Mion dialogue step (units), read back via `% 10` and `< 20`/`< 50`. Treating it as one counter breaks both halves; added `memo_state_ex`/`set_memo_state_ex` (a second memo axis). `FLAG` is a **third** summon-meter shape (`20 * flag` percent, reset on success) after 414's and 416's. `npc.deleteMe()` needed `delete_npc`. Dead at both ends for the **fifth** quest running (NPC 31958). 5 tests, first-run green. **30 quests ported; the Path family is complete.** **Q00210 Obtain a Wolf Pet landed** (plan: [PLAN_G22_WOLF_PET.md](PLAN_G22_WOLF_PET.md)) — a standalone four-NPC dialog chain (Lundy 30827 → Bella 30256 → Bynn 30335 → Sydnia 30321 → back to Lundy) handing over the **Wolf Collar 2375**; chosen because it closes a dangling-value link — it's *how you obtain the starter wolf pet*, and the G29 pet system that consumes the reward is already built. Pure talk chain (no kills), min level 15 via `addCondMinLevel` handled in `on_talk`'s CREATED branch. **A test caught a packet split**: `no_level.htm` is a `.htm` file, so it ships as `ExNpcQuestHtmlMessage` (the quest-window packet, 0xFE/0x8E) *not* a plain `NpcHtmlMessage`, so the level-gate assertion had to decode the extended packet. Java's dead `30827-04.htm` case (no button links it — grepped the html set) transcribed anyway. quests_tests 113 → 115 (full chain through the real bypass router incl. an out-of-order-cond guard; the level gate); registration + cond-guard sabotage-verified. **31 quests ported.** **Q00261 Collector's Dream landed** — a clean newbie hunting loop (Alshupes 30222; kill Hook/Crimson/Pincer spiders for 8 legs → 700 adena, repeatable, level 15–21), a near-clone of `Q00303` reusing its `start_condition_html` (`addCondMaxLevel(21)`) + `give_item_randomly` + `on_kill` shape. **Finding — `giveNewbieReward` is dead almost everywhere:** it's commented out (`// Q00281_HeadForTheHills.giveNewbieReward`) in every newbie quest *except* Q00261 and Q00276, and `GUIDE_MISSION` (the player variable it sets) has **no reader** anywhere in the port or the dist scripts — so it's inert bookkeeping, and its `ExShowScreenMessage` is unported. Deferred with a `TODO(newbie-guide)` at the completion site (belongs with the newbie-guide mission system) rather than porting a 347-line packet + a dead variable for one hunting quest. quests_tests 115 → 117 (the kill→turn-in loop through the real router; the max-level refusal); max-level-gate + registration sabotage-verified. **32 quests ported.** **Q00257 The Guard is Busy landed** — the canonical first-hour Gludio quest (Gilbert 30039, level 6–16): kill orcs/werewolves for trophies, adena by type (5/8/10a + 1000 for 10+). **The mechanic is a per-mob hand-rolled drop table** — nine monsters, each `getRandom(random) < chance` with denominators of 10 or 100 (not `giveItemRandomly`, so un-multiplied by `RateQuestDrop`, exactly as Java writes it — the [[l2r-quest-drop-helper-vs-handroll]] call), and the Orc Archer carries a **two-entry table where the first hit wins** (`roll(10)<2` → 2 amulets, else `roll(10)<10` → 1) — pinned by a test forcing the first entry. `getRandom(0)` (Werewolf Chieftain) is an always-drop (`roll(0)` clamps to 0 < 1). Its `giveNewbieReward` is commented out in the dist (dead — see the newbie-reward note above), so omitted. quests_tests 117 → 119 (start→drops→adena-by-type→repeatable exit through the real router; max-level refusal); registration + max-level-gate sabotage-verified. **33 quests ported.** **Q00259 Request from the Farm Owner landed** — a Gludin spider hunt (level 15–21) with **two reward paths**: Edmond (30497) pays 25a per skin (+250 for 10+), or **Marius (30405) trades 10 skins for a batch of consumables** (Greater Healing Potions / arrows / soulshots / spiritshots — the player's pick). The skin drops **unrolled** (one per kill). Both paths tested through the real router (adena turn-in + repeatable exit; the Marius trade consuming 10 skins for 2 potions); registration sabotage-verified. **34 quests ported.** Next: ~157 more quests, ~81 `ai/` scripts, daily quests, the tutorial, `//reload` |
| Game  | G23 Grand bosses & raid bosses                              | 🚧 boss zones/respawn/AI/persistence — `//grandboss` **Raid curse landed** (plan: [PLAN_G23_RAID_CURSE.md](PLAN_G23_RAID_CURSE.md)), G23's first slice. Checked before planning (the G20.5 lesson): **two of the gate's three clauses were already met** — `boss_respawn`, built during G21, covers scheduled respawn and `npc_respawns` persistence for all 225 `dbSave` spawns. Raid curse had **zero references**. An anti-farming rule: a player **more than 8 levels above** a raid boss is punished for interfering, which is why it fires on *helping* and not only on attacking. Skill 4215 (`Mute`+`PhysicalMute`, 3600 s) for a **good** skill cast nearby, 4515 (`BlockActions`, 120 s) for attacking or a **bad** skill — both already in the datapack with ported effects. Two sites: the damage hook sits **after** the damage block because Java's comment says *"in retail you deal damage to raid before curse"*, and the post-cast scan covers a high-level player buffing a low-level party from outside the fight, which the damage path never sees (the boss must be **in combat**, so travelling past an idle spawn is free). Raid **minions inherit** `giveRaidCurse` from their master. Boundary kept as Java's `> level + 8` with a test pinning that exactly 8 is not cursed. 7 tests incl. an end-to-end hit asserting the curse lands *and* the damage is dealt. **Raid points landed** (plan: [PLAN_G23_RAID_POINTS.md](PLAN_G23_RAID_POINTS.md)). Candidates measured before picking: **chaos target swaps struck** — `isChaos` has **zero occurrences** in this dist's NPC data, so the mechanic exists in Java and no NPC enables it here (same call as agathions/pet evolution); **minion waves already done** in G21; **raid points** real and unimplemented — 409 `<acquire raidPoints>` attributes, 374 of them non-zero. The distribution differs from the exp split in ways worth stating: points go to the **top damage dealer** (not proportionally), and if they are in a party the award splits among members **within `ALT_PARTY_RANGE` of the corpse** — including members who dealt no damage, excluding ones who hung back — with `max(points/size, 1)` so nobody rounds to zero. Raid **minions award nothing**. `CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL` is broadcast, not sent to the earner. `raidbossPoints` is an existing `characters` column, so persistence is one field and one bind — no new table and none of the shared-flush hazard from G29 slice 27. raid_curse_tests 7 → 13 + a datapack-backed parse check. **Gate met; scope audited.** All three gate clauses hold — *spawns on schedule* and *state persists* via `boss_respawn` (G21, 225 `dbSave` spawns), *applies raid curse* via slice 1. **`BossZone` does not exist on this chronicle**: no such class in the Java tree, no `type="BossZone"` in any zone file, no script reference — the roadmap's "boss zones + entry conditions" describes a generic L2J feature set, and on this dist entry gating lives **inside the grand-boss AI scripts** instead. So the honest remaining scope is those scripts: **10 under `ai/bosses/`** (Antharas 1056 lines, Baium 787, Valakas 581, QueenAnt 408, Orfen 384, Sailren 326, DrChaos 321, Core 232, Zaken 109; Frintezza absent) against **32 `GrandBoss` NPCs**. The port has `GrandBossManager`'s state (`grand_bosses` loaded at boot from `grandboss_data`) but it backs only the read-only `//grandboss` panel — no boss AI is ported. That is milestone-sized, not slice-sized, and is left explicit rather than tracked as a vague remainder; **QueenAnt is the natural first** (mid-size, already referenced by the raid-curse code, and the most commonly-run Interlude raid). **`ScriptZone` support landed** (plan: [PLAN_G23_SCRIPT_ZONES.md](PLAN_G23_SCRIPT_ZONES.md)) — groundwork every `ai/bosses` script needs, since each opens with `ZoneManager.getZoneById(…)` and none of that existed. A `ScriptZone` is behaviourally **nothing**: no `ZoneId` in Java, so no membership bit and no flag — it exists to be *addressed by id* (Queen Ant's lair is `getZoneById(12012)`). `ZoneKind::Script.bit()` is therefore 0, asserted, since giving it a bit would put everyone standing in one into a zone state nothing intends. Added the kind + `type="ScriptZone"` mapping (133 zones), kept `Zone.id` (previously discarded), and `ZoneData::by_id` + `Zone::contains`. **Adding a file for one kind must not change another**: `custom_script.xml` also ships a stray `SiegeZone` ("GainakSiege", later-chronicle, no `castleId`), and letting it through would set the Siege bit that `death.rs` reads as a **free-death zone** — dying there would silently skip the exp penalty — so the two script files are filtered to script zones only. Caught by the existing zone-census test: it failed on the count, and following *why* found the siege zone instead of just bumping the number. boss_zone_tests 4, all against real dist data; census updated to 1031 with the reason in the assertion. **Grand-boss respawn lifecycle landed** (plan: [PLAN_G23_GRANDBOSS_LIFECYCLE.md](PLAN_G23_GRANDBOSS_LIFECYCLE.md)) — ported **once** rather than ten times: every `ai/bosses` `onKill` marks the boss dead, rolls a respawn window, persists it and arms a timer, so that block lives in `game_loop/grand_boss.rs` driven by `GrandBoss.ini`, leaving each script only its interesting half. **The boot branch is the one that matters**: alive → spawn with stored HP; dead with a running timer → arm the remainder; dead with a window that **elapsed while the server was down** → spawn *now*. Miss that third case and the boss stays dead **forever**, since only a kill schedules a respawn and it can't be killed — its own test. Window is `(interval ± random)` hours, tested as a range **and** for actual variation (a single-value assertion passes on a broken fixed window); **Baium ships no `RandomOfBaiumSpawn`**, so its spread defaults to 0 rather than being assumed symmetric — pinned, since a copied default would give it a spread retail doesn't have. Respawning an already-alive boss is a no-op (a duplicate timer would stand up a second copy); stored HP of 0 means "never wounded", so a boss wounded before a restart comes back wounded. `StoreGrandBoss` is fire-and-forget rather than folded into the character flush — it has nothing to do with a character and would inherit that transaction's failure mode. grand_boss_tests 8, incl. the real `GrandBoss.ini`. **Queen Ant landed** (plan: [PLAN_G23_QUEEN_ANT.md](PLAN_G23_QUEEN_ANT.md)) — the first grand-boss script. **The fight is a priority rule**: six nurses heal, and they heal the **larva first**, so a party that leaves the larva alive fights a Queen whose healers are permanently busy — that ordering *is* the encounter. Larva gets `HEAL1` or `HEAL2` at random, the Queen only `HEAL1`. Java's "skip a nurse whose leader is the larva" branch **cannot fire here** — the larva declares no minions, only the Queen has nurses — so it is left out rather than written as dead code (same call as `EffectFlag.FEAR`/`MP_BLOCK`). Heals route through `npc_cast::start_cast` behind `check_use_conditions`, so a nurse pays the same MP and cooldown as any NPC rather than being a privileged script effect. The larva is script-spawned (not in the minion table). **A fixture made three tests vacuous**: the first draft wounded the Queen with an absolute `cur_hp = 10_000`, but `add_test_npc` gives every NPC 100 HP regardless of template, so that set HP *above* max and read as un-wounded — no heal was ever attempted. Found by instrumenting the cast; tests now wound by **fraction of max**. queen_ant_tests 6. **Core landed** (plan: [PLAN_G23_CORE.md](PLAN_G23_CORE.md)) — second boss, and with the shared lifecycle in place the whole slice was Core's own mechanics. **The finding: Core spawns 3 minions, not 19.** Java's `MINNION_SPAWNS` is a `Map<Integer, Location>` with **19 `put`s** (10 Death Knights, 5 Doom Wraiths, 4 Susceptors) keyed by **npc id**, so each type keeps only its *last* location and three entries survive. Plainly not what the author meant — the 19 coordinates are laid out around the lair — but it is what the server does, and porting the list faithfully would have given Core **six times the adds**. Ported as it behaves, with a test named for it: *port what the script does, not what it looks like it means*, the same principle as the dist data being authoritative. Minions respawn 60 s after dying **only while Core is alive** (a cleared lair stays cleared), and Core's death clears them after **20 s** rather than immediately — tested as *still standing right after the kill*, which an immediate despawn would fail. Barks deferred: `npc_say` lives on the quest context and isn't reachable from a boss script yet. core_boss_tests 6. **Orfen landed, and Zaken came free** (plan: [PLAN_G23_ORFEN.md](PLAN_G23_ORFEN.md)). **Zaken needed no script at all** — its 109 Java lines are entirely the spawn/respawn boilerplate slice 4 ported once (verified by grep: no `onAttack`/`onSpawn`/minions), so it is already driven by the shared lifecycle. One of the ten scripts turned out to be zero work. **Orfen's drag**: an attacker between **300 and 1000** units has a 1-in-10 chance per hit of being teleported *onto* Orfen and paralysed — the band is the mechanic, punishing ranged damage while melee is never dragged; both edges tested with the roll forced. **The half-HP relocation** fires **once per life**, not once per hit below the threshold (tested by moving Orfen and hitting it again), and Java's `if/else if` means it wins over the drag — a boss that just relocated shouldn't drag someone to where it no longer is. **Riba Iren heals on *its own* wounds**, not its master's — the opposite of every other healer minion (Queen Ant's nurses watch their target), so exactly what a port gets backwards by pattern-matching; both directions tested. **A vacuous assertion was hiding a broken fixture**: the first Riba Iren test asserted `Vitals.is_some()` (always true); replacing it with a real measurement made it fail and revealed the fixture had given `ORFEN_HEAL` the *paralysis* effect list. orfen_tests 8. **Boss-id audit** (plan: [PLAN_G23_BOSS_IDS.md](PLAN_G23_BOSS_IDS.md)) — fixes a defect introduced in slice 4 and found by running the reachability check before picking the next boss, not by anything failing. Slice 4 mapped **Antharas to 29019**; the script uses `ANTHARAS = 29068` (the "strong" variant) and `grandboss_data` has a row for 29068 and **none** for 29019 — so Antharas's respawn window never resolved and **it would have died and never come back**. Silent, because 29019 is a valid NPC template: the id looks right in isolation and is only wrong against the boss table. The table ships 8 rows here (Queen Ant, Core, Orfen, Baium, Zaken, Valakas, Antharas **29068**, and 25512 Gigantic Chaos Golem = DrChaos's second form); **Sailren (29065) has no row**, so it isn't a tracked grand boss on this dist. New test cross-checks the config against the real table **in both directions** and pins that 29019 must *not* resolve — a one-sided check wouldn't have caught it. Lesson: run the reachability check even when you already know what you're building next. **Boss barks landed** (plan: [PLAN_G23_BOSS_BARKS.md](PLAN_G23_BOSS_BARKS.md)) — the blocker was **one function**: `npc_say` was a `QuestCtx` method, but its body only ever needed the world and the speaker, so the quest coupling was incidental. Lifted to `helpers::npc_say` with `QuestCtx` delegating; all 113 quest tests pass unchanged. *A helper that lives in one subsystem because that's where it was first needed is not the same as one that depends on it — check the body before assuming a port is blocked.* Core now speaks: the two intro lines on the **first hit of a life**, a 1-in-100 "Removing intruders" taunt after, and two death lines. **The intro resets on death** (`_firstAttacked = false` in `onKill`) — without it a Core killed once stays silent for the lifetime of the process, invisible in testing and obvious to players. core_boss_tests 6 → 10, counting `NpcSay` (0x30) on a real client channel so the assertions measure packets sent rather than a flag. **Valakas attack rules landed** (plan: [PLAN_G23_VALAKAS.md](PLAN_G23_VALAKAS.md)) — the first boss with the **four-state ladder** (DORMANT/WAITING/FIGHTING/DEAD) rather than the ALIVE/DEAD pair; only the `onAttack` half is ported, with the lair entry flow and 30-minute window stated as their own slice rather than left implied. **Attacking from outside the lair kills you** — Java's `attacker.doDie(attacker)`, a hard anti-exploit against plinking from safety, self-inflicted so it carries no PvP or karma consequence. **The order is the mechanic**: the zone check precedes the status check, so an out-of-zone attacker dies *whatever* the boss's status — including while Valakas is dead, when the status branch would merely have teleported them; its own test, since that is the half a reordering silently loses. Strider riders are debuffed **once**, not every swing. Zone 12010 is a `ScriptZone` — the first script to consume slice 3's loader work. Also added a **fixture guard** asserting the tests' lair coordinate really is inside the zone: without it every "inside" test would silently exercise the outside path and still pass. valakas_tests 5. **Baium landed** (plan: [PLAN_G23_BAIUM.md](PLAN_G23_BAIUM.md)) — **chosen by counting cinematics**. Valakas's entry flow was next on the list, but it is **19 `SpecialCamera` calls** and the camera packet isn't ported, so most of that slice would be stubs; Antharas has 7 and **Baium has 0**, making it the only one of the three great bosses portable now. One grep changed which slice was worth doing. Landed: **five archangels** at fixed points (not in a minion table — the script places them) and the **anti-strider debuff** cast *once* (`!isAffectedBySkill(4258)`), tested by draining the client channel and asserting a second hit starts no new cast. **Deliberately not ported**: Baium's targeting is a **top-3 threat table** on NPC variables fed by a weighting that shifts as he is worn down — melee is `damage × 1000` while a caster at full health is `(damage/3) × 20`, so **melee threat is worth fifty times a caster's**, and the caster weighting swings tenfold across the HP bands. Folding that into the ordinary aggro list would look like it worked and would not be Baium, so it is its own slice with the table written down. baium_tests 4. **Baium's threat table landed** (plan: [PLAN_G23_BAIUM_THREAT.md](PLAN_G23_BAIUM_THREAT.md)) — the piece slice 11 deliberately left out, ported rather than approximated onto the aggro list. Baium keeps a **top-3 table** fed by an HP-banded weighting: a 300-damage hit scores **300 000** in melee but **2 000** from a caster at full health (**150×**), and the caster figure climbs to 10 000 below 25% — so Baium fixates on melee, and a caster beneath notice early becomes a real target as he weakens. Both asserted as **relationships** (a ratio, an ordered progression across the four bands) rather than four magic numbers, so a mis-ported band shows as the wrong shape. Two behaviours easy to flatten into "set the value": an existing entry is raised **only** when below `aggro + 1000` (so small hits don't ratchet a threat upward), and a newcomer displaces the **weakest** slot — not the oldest, and not nobody. Jitter forced to 0 so the ladder alone decides. baium_tests 4 → 8. **Baium's skill selection landed — Baium is complete** (plan: [PLAN_G23_BAIUM_SKILLS.md](PLAN_G23_BAIUM_SKILLS.md)). Two mechanics beyond "pick a skill": **the rotation** — after acting, the top threat is knocked down to **500** seventy percent of the time, which is what stops Baium tunnelling the biggest damage dealer all fight and lets the next player take a turn; and **the widening pool** — two options above 75% HP, three above 50%, four below 25%, each an independent 10% roll taken *in order* with the basic attack as fallback, so his repertoire opens up as the fight goes on (the same shape as his threat weighting). **Pruning is targeting, not tidiness**: a threat whose attacker died or fled beyond 9000 units is zeroed, which can change who he attacks — a test puts the top two threats on a corpse and a runaway and asserts he turns on the third, lowest attacker. baium_tests 8 → 14, rolls forced so each test isolates one decision; the band test asserts the **first option of each band** rather than "some skill", which is what distinguishes an ordered ladder from four skills in a bag. **`SpecialCamera` (0xD6) landed** (plan: [PLAN_G23_SPECIAL_CAMERA.md](PLAN_G23_SPECIAL_CAMERA.md)) — the blocker named at slice 11, unblocking Valakas's entry flow (19 uses) and Antharas (7). **`range` is accepted and never written**: Java's canonical constructor takes twelve parameters, assigns eleven, and drops `range`, so the wire carries eleven ints. The port keeps the parameter as `_range` so call sites transcribe Java's argument list literally — removing it would shift every following argument at 26 call sites of unlabelled integers, the worst place for a silent off-by-one. The test asserts the field after `time` is **duration, not range**, which is exactly the corruption a "helpful" serialisation would cause. Java's 11-arg overload additionally forwards `duration` and `range` into each other's slots (a caller's range is written as the duration) — **not reproduced, because no boss script uses it**; all 26 call sites take the 12-arg form, checked rather than assumed. special_camera_tests 2, including Valakas's opening shot transcribed from the script and checked field by field. **Valakas's entry cinematic landed** (plan: [PLAN_G23_VALAKAS_CINEMATIC.md](PLAN_G23_VALAKAS_CINEMATIC.md)), the first thing `SpecialCamera` unblocked. Ten beats **scheduled up front from the start of the sequence**, as Java does, rather than each chaining the next — deliberate, because **the beats are unevenly spaced** (330 ms between steps 5 and 6, 6.7 s between 8 and 9) and a relative chain would be easy to get subtly wrong in a way visible only as a cinematic that felt off; a test pins the 26-second span and that the beats occupy distinct ticks. The tenth beat carries no camera — it flips the status to `FIGHTING`, which starts the fight and locks entry. The camera table is transcribed in Java's argument order, `range` included even though the wire drops it, which is exactly why slice 14 kept that parameter — the two tables diff by eye. **It plays for the lair, not the neighbourhood**: tested with one player inside and one outside, since the ordinary region broadcast would pass a weaker test and show the cinematic to bystanders. valakas_tests 5 → 10, plus a `pending_ticks_for_test` scheduler hook for asserting a sequence's *shape*. **Antharas's minion waves landed** (plan: [PLAN_G23_ANTHARAS.md](PLAN_G23_ANTHARAS.md)) — the last boss script opened, with the mechanic that defines the fight. Adds arrive every five minutes in **growing waves**: the multiplier starts at 1 and climbs on ~89% of waves to a ceiling of 4 (one pair → four pairs), with the lair capped near 100. The spawn ladder is cap-aware and its steps are **not** interchangeable — **step 3 is the one worth having**: at 98 minions Antharas adds a *single, randomly chosen* dragon rather than skipping the wave, so the lair fills to exactly 99; collapsing it to "a pair if there's room for two" reads equivalent, caps the fight two adds early and would never be noticed. **A deliberate divergence, documented**: Java keeps `_minionCount`/`minionMultipler` as script *statics*, the port puts them on the boss as a component — two Antharas instances sharing one counter is a bug waiting to happen and nothing in the Java relies on the sharing. antharas_tests 6, incl. a full lair spawning nothing while still rearming so the fight recovers as adds die. Still open for Antharas: the entry cinematic (7 shots), the Heart of Warding entry gate, the 200-player cap, `manageSkills`. **Antharas's entry cinematic landed** (plan: [PLAN_G23_ANTHARAS_CINEMATIC.md](PLAN_G23_ANTHARAS_CINEMATIC.md)). **Antharas chains; Valakas batches** — the obvious move was to reuse slice 15's table, and it would have been wrong: Valakas arms all ten beats up front, Antharas has each beat schedule the next with a *relative* delay. Reshaping one into the other silently changes the timing model, so a test asserts exactly **one** cinematic timer is pending after the start, which is what distinguishes a chain from a batch. **`CAMERA_3` forks** — it roars, arms the next beat *and* a second social 5.2 s later, the only beat arming two timers, which a uniform "each beat arms the next" port drops entirely. The tail (`START_MOVE`) now **starts the minion waves**, moved off spawn from slice 16, so an un-engaged boss isn't already producing adds. **A vacuous assertion was caught on review**: the fork test first read `assert!(drain(...) > 0 || true)` — passing unconditionally, and against the wrong opcode; replaced with an exact count against `SocialAction` (0x27). A passing suite says nothing about assertions that cannot fail. antharas_tests 6 → 12. **Antharas's entry gate landed** (plan: [PLAN_G23_ANTHARAS_GATE.md](PLAN_G23_ANTHARAS_GATE.md)) — the Heart of Warding's ladder. **Order is the user experience**: the boss's state is checked *before* the ticket, so a player without a Portal Stone arriving at a dead Antharas is told "Antharas is dead", not "you need a stone" — tested with an empty inventory so a reordering shows as the wrong message. Two rungs easy to lose: **only the leader may bring a group in** (and for a command channel it's the *channel* leader, so a party leader inside a CC is refused), and **the whole group must fit** — `members > MAX_PEOPLE - inside` refuses outright rather than admitting as many as will fit, so a raid isn't split in half by the doorway. Only members gathered within 1000 units come along. **A branch no test could reach**: the first overfill test admitted in its own comment that it asserted something else, since filling a 200-player lair in a unit test is impractical — so the ladder was split to take occupancy as a parameter, and the rung is now tested from both sides (199 inside refuses a party of two, 198 admits). *A test named after an unreachable branch is worse than none, because it reads as coverage.* antharas_tests 12 → 19. **Antharas's skill ladder + the caller neither boss had** (plan: [PLAN_G23_ANTHARAS_SKILLS.md](PLAN_G23_ANTHARAS_SKILLS.md)). **`baium::manage_skills` had no caller anywhere in the crate** — written, documented and tested in slice 12, and Baium chose skills into the void and only ever swung, for seven slices, while the plan doc said "Baium is complete". The [[regen-stat-pipeline]] shape one level up: not a stat pumped but never read, a whole *procedure* that is correct, covered and unreachable — and **being well-tested is what hid it**, since a unit test calls the function directly and passes exactly as it would if wired. What finds this is `cargo build`'s dead-code warning and reading the entry point, not the unit. Both bosses now run from `onAttack`. **The threat table was duplicated** — Antharas's `refreshAiParams` is identical to Baium's line for line, six slices apart; extracted to `boss_threat.rs`. **The tail sweep's angle is absolute**: Java gates it on `calculateDirectionTo` with no heading term, so "within 8° of 180°" means the target is due *west*, not behind him — every other cone check in the codebase subtracts the heading first. Ported as written and pinned by a test (target west, Antharas facing east), because "correcting" it is a behaviour change dressed as a fix. The ladder is a **chain of else-if**, so its percentages are conditional; four bands widen as he weakens, and the Breath Attack is the only skill that *opens* a band (rolled first, 30%, below 25% HP). `castOnTarget == false` = cast on **himself** — the areas are centred on the boss. Two Baium tests changed because choosing decays the threat it just read, immediately, which is Java's real order. antharas_tests 19 → 27, baium_tests 14 → 15; both hook tests verified failing on the previous commit. **The entry flow is wired — slice 18's defect closed** (plan: [PLAN_G23_ANTHARAS_ENTRY.md](PLAN_G23_ANTHARAS_ENTRY.md)): `scripts::antharas_heart` registers the **Antharas** quest script (the name is load-bearing — the dist htmls already say `Quest Antharas enter`/`teleportOut`), so the Heart of Warding 13001 serves `13001.html`, the ladder's five verdicts map to the five refusal pages, an admitted group teleports to `(179700+rnd, 113800+rnd, −7709)` and the **first** admission flips WAITING and arms `SPAWN_ANTHARAS` at `AntharasWaitTime` minutes (20 on this dist; a second party mid-window must not restart the clock — tested by entering at half-window and asserting the boss arrives on the FIRST deadline). `SPAWN_ANTHARAS` relocates the boss to the platform via a new **`relocate_npc`** (Orfen's in-place `Position` mutation is region-local; this re-indexes `npc_regions`, `DeleteObject`s the old cell and re-introduces — the cross-region move is asserted from both cells), flips IN_FIGHT, plays `BS02_A` to the lair and starts slice 17's camera chain (whose tail starts the waves) — the spawn-time cinematic stand-in is removed (Valakas's stays unwired until its entry slice, TODO(G23) at the site). **The wiring exposed a status collision**: `on_grand_boss_killed` wrote the two-state `DEAD = 1` for every boss, which the four-state ladder reads as *WAITING* — `try_enter` would have admitted raids into a dead Antharas's lair; `dead_status(boss_id)` (3 for Antharas/Valakas) now feeds the kill and both boot branches, pinned from both sides (a killed Antharas reads 3 and refuses entry; an elapsed window still respawns him; Core keeps 1). And the test that would have caught slices 12 and 18: **the `enter` bypass runs through the real router** (`handle_request_bypass_to_server` → registered script → ladder), where a direct `heart_enter` call would pass with the script unregistered — it also caught the bypass distance guard correctly refusing `teleportOut` aimed at the 180k-units-away Heart (the cubic is its own NPC inside the nest). antharas_tests 27 → 32. Antharas still open: the five invisible clear-NPCs + `CUBE` spawn on kill, the 5-minute `MANAGE_SKILL` cadence vs the port's on-attack hook. **Valakas's entry flow wired** (plan: [PLAN_G23_VALAKAS_ENTRY.md](PLAN_G23_VALAKAS_ENTRY.md)) — slice 15's 10-beat cinematic (`begin_cinematic`) was another **complete, tested, uncalled** `"beginning"` handler; `scripts::valakas_teleporters` (the `ai/others/ValakasTeleporters` chain) is the caller. Six NPCs route through the bare `Quest ValakasTeleporters` bypass (→ `on_talk`): **Klein 31540** shows a crowding html by lifetime count and, on its `31540` sub-event, does the **Vacualite-gated** (7267) Hall of Flames teleport + `allowEnter` grant; **Heart of Volcano 31385** is the lair door — refuses while fighting/dead/full/flagless, else consumes the flag, teleports in and, **on the first (DORMANT) entry only**, arms `"beginning"` at `ValakasWaitTime` (30 min) and flips WAITING; **cubic 31759** exits; the **gatekeepers 31384/31686/31687** open doors 24210004-6 (new `doors::open_door_by_id`, since door oids are dynamic). `"beginning"` (new `ValakasBeginning` timer, guarded on still-WAITING) runs `begin_cinematic`, whose final beat flips FIGHTING. **The count quirk ported faithfully**: Java's `playerCount` is a `static int` that **only increments — never resets** on spawn/death/window (after 200 lifetime entries the lair locks until restart); stored as `World.valakas_entry_count`, pinned by a test that a kill+respawn keeps counting. Both subtle wires sabotage-verified (arm-on-every-entry fails the once-only test; unregistering the script fails the router e2e). valakas_tests 10 → 16. **Dr. Chaos landed — the last `ai/bosses` script** (plan: [PLAN_G23_DR_CHAOS.md](PLAN_G23_DR_CHAOS.md)). The Gigantic Chaos Golem 25512 had a `grandboss_data` row but zero AI. **The encounter is the paranoia**: Dr. Chaos (32033) is a small NPC whose `pissed_off` timer starts at 30 and drains 1 per nearby living player per second (1–5 more per talk); at 15 he barks a warning, at ≤0 he **becomes** the golem through a 5-beat cinematic. So lingering near him *is* what spawns the boss. The golem carries **no config respawn window**, so the shared lifecycle skips 25512 entirely — this module owns its status (a third ladder: `NORMAL 0`/`CRAZY 1`/`DEAD 2`), boot (CRAZY restores the golem with stored HP; DEAD arms the reset or, if elapsed while down, respawns Dr. Chaos now — the downtime trap again), the **30-idle-minute despawn** (revert to Dr. Chaos; an attack refreshes the clock), and the `(36 ± 24)h` kill window. Barks needed a **literal-text `NpcSay`** (`npcString = -1` + the string) — the existing builder only did client-localized string ids, but Dr. Chaos's lines are literal English. **The slice-20 lesson applied preemptively**: the kill test drives `npc_do_die` end to end (a direct `on_golem_killed` call passes even with the `death.rs` hook unwired — sabotage-verified), and the transform is verified through `handle_paranoia`. Faithful timing detail pinned: Dr. Chaos **lingers through the 17 s cinematic** and is deleted only on beat 5 (the golem replaces him then, not on trigger). dr_chaos_tests 9. **Every `ai/bosses` script is now ported.** **Antharas death tail landed** (plan: [PLAN_G23_ANTHARAS_DEATH_TAIL.md](PLAN_G23_ANTHARAS_DEATH_TAIL.md)) — the missing `onKill` half left players **stranded in the lair** after the kill (no exit). Now `death::npc_do_die` runs `antharas::on_antharas_killed`: `DESPAWN_MINIONS` (every Behemoth/Terasque in the nest), the death `SpecialCamera`+`PlaySound("BS01_D")`, spawn the exit cube 31859 at `(177615,114941,-7709)`, and arm `AntharasClearZone` at +15 min — which teleports lingering players to the Giran-side exit and despawns every NPC left in the nest (cube included). The cube's `teleportOut` talk was already wired (`AntharasHeart` lists 31859), so the loop closes. **Bug found + fixed**: `LAIR_ZONE_ID` was `12016` (a Talking Island `ScriptZone`), not the Antharas Nest — Java's `getZoneById(70050, NoRestartZone.class)` (`antaras_no_restart`); the wrong id was latent because its only reader (`players_in_lair` occupancy) **fails open** (empty zone = nobody inside), so the `MAX_PEOPLE` gate silently never tripped. antharas_tests 32 → 35 (kill→cube+minion-clear via the real death path, cube `teleportOut` through the router, `CLEAR_ZONE` oust+despawn through the loop dispatch); both new wires sabotage-verified. **Valakas death tail landed** (plan: [PLAN_G23_VALAKAS_DEATH_TAIL.md](PLAN_G23_VALAKAS_DEATH_TAIL.md)) — the symmetric counterpart. `death::npc_do_die` runs `valakas::on_valakas_killed`: the death sound + opening `SpecialCamera`, then the **eight-beat `die_1..die_8` death cinematic** scheduled up front from the kill (the entry cinematic's batch model), whose eighth beat drops the **fifteen** exit cubes 31759 at `TELEPORT_CUBE_LOCATIONS` and arms `ValakasRemovePlayers` at +15 min → `oustAllPlayers` teleports lingering players to `LAIR_EXIT`. The cube's `teleportOut` was already routed by `scripts::valakas_teleporters`, so the loop closes. `BOSS_ZONE_ID = 12010` was already correct (fixture-guarded — no repeat of the Antharas zone-id bug). New `ScheduledTask::ValakasDeathCinematic`/`ValakasRemovePlayers`. valakas_tests 15 → 18 (kill arms the cinematic via the real death path, `die_8` spawns all 15 cubes + arms remove_players through the loop, remove_players ousts through the loop); all three wires sabotage-verified. **Both grand-boss death tails now done.** Remaining G23 tail: the 5-min `MANAGE_SKILL` cadence vs the on-attack hook — cosmetic/secondary. |
| Game  | G24 Castles, sieges, clan halls & territory war             | 🚧 **The automatic siege schedule landed** (plan: [PLAN_G24_SIEGE_SCHEDULE.md](PLAN_G24_SIEGE_SCHEDULE.md)), G24's first slice. Checked before planning (the G20.5 lesson): the siege *combat* is already extensive (towers/guards/flags/doors, the throne-room artifact **capture**, zones, PvP relations, `start_siege`/`end_siege`) — but **sieges only ever fired from a GM command**: `SiegeSchedule.xml` (the weekly per-castle calendar) was never loaded, so on a real server no siege ever happened. Now each **enabled** castle's siege starts itself on its scheduled day/hour and **re-arms next week** — a self-perpetuating timer that needs no persisted `siegeDate` (the calendar is fixed, so the next occurrence is a pure function of the clock). New `SiegeScheduleEntry` loader (all 9 castles: Sunday 16:00/20:00), a pure `next_siege_millis(now, weekday, hour)` (1970-01-01 = Thursday anchor; computed in **UTC** — Rust std has no timezone, a documented divergence from Java's server-local time, the weekly cadence exact either way), boot-arming from the `SiegesLoaded` handler where the per-castle `Siege`s exist, and a `SiegeStart` task that begins the siege + re-arms. **Also confirmed a stale marker**: `capture`/`try_capture_artifact` is **already production-reachable** (the Holy Artifact 35063 etc. is a permanent castle spawn, the interaction siege-gated), so its `#[allow(dead_code)]`/"nothing reaches capture" comments were removed — a castle can be won by seizing the artifact. Both subtle wires sabotage-verified (drop the re-arm → the weekly timer dies; drop the enabled filter → disabled castles get armed). siege_schedule_tests 4. Still ⏳: clan halls (`//clanhall` — a greenfield residence subsystem), the player-facing siege registration window, and the end-siege polish (blood-alliance count, ticket count, residential skills).  |
| Game  | G24.5 Boats                                                 | ⏳ `BoatManager` + 4 ferry routes (`AllowBoat = True`) |
| Game  | G25 Olympiad & hero                                         | ⏳ AdminOlympiad/`//sethero`/`//saveolymp`/`//endolympiad` |
| Game  | G26 Seven Signs, Manor & Mammon                             | ⏳ `//manor`/`//mammon_*` |
| Game  | G26.5 Lottery & Monster Race                                | ⏳ `games/` managers (Lottery, Race Track betting) |
| Game  | G27 Instances                                              | ⏳ AdminInstance/AdminInstanceZone |
| Game  | G28 Events engine & cursed weapons                          | ⏳ **cursed-weapon gate MET** (plan: [PLAN_G28_CURSED_WEAPONS.md](PLAN_G28_CURSED_WEAPONS.md)): the autonomous cursed-weapon loop landed (`game_loop/cursed_weapon.rs`) — a slain ordinary monster drops one via `CursedWeaponsManager.checkDrop` (killer's acting player must be an un-cursed real player; `Rnd.get(100000) < dropRate`; new `DropSource::CursedWeapon` exempts it from auto-destroy per Java's `setDropTime(0)`; `RedSky`+`Earthquake`+`S2_WAS_DROPPED_IN_THE_S1_REGION`, life task armed at `now + duration`), picking it up curses the finder (`activate` reuse, intercepted in `pickup_ground_item`; already-cursed picker consumes the duplicate), and `ScheduledTask::CursedWeaponExpiry` end-of-lifes it on the deadline (`RemoveTask`, stale-timer guard). New `CursedWeapon.dropped_item_oid` + SM id 1817. 10 tests, all three wires sabotage-verified. **Deferred (TODO(G28)):** kill-count level-up/stage bonus, "hungry" HP decay, drop-on-PK-death, login restore, region-name SysString, pickup end_time preservation. **Still pending for G28:** the events engine (TvT / `AbstractEvent` / `EventManager`), `AdminEvents`/`//tvt_*`. Activation engine + `//cw_*` GM commands already landed (G21). |
| Game  | G29 Summons, pets, servitors, cubics, agathions             | ✅ **Servitor summoning landed** (plan: [PLAN_G29_SERVITOR_SUMMON.md](PLAN_G29_SERVITOR_SUMMON.md)): the first G29 slice, and it closes the **single biggest unported effect on the whole ranking** — `Summon`, 24 learnable skills (Dark Panther 283, Kat the Cat 1111, Shadow 1128, the golems, …), every one of which cast and produced nothing. **Design decision:** a servitor is an ordinary NPC entity marked with a `ServitorOf` component rather than its own `Creature` subclass as in Java — it already *is* a template + stats + position + AI, so the only genuinely new state is the owner link, the summoning skill and the lifetime; that keeps servitors inside the existing spawn/region/visibility/combat machinery. `Player.getServitors()` is a scan, not a cached index (at most one servitor per player here). Re-casting **swaps** rather than stacking (Java unsummons first); `lifeTime <= 0` is Java's no-expiry case (`Integer.MAX_VALUE`, "Classic hack. Resummon upon entering game."); `npcId` is declared **per skill level**, so each level summons a stronger template. Ported `PetSummonInfo` (`PET_INFO` 0xB2), the ~50-field flat packet the **owner** sees, with the servitor's remaining lifetime in the fed/max-fed pair that draws its time bar. **Servitor follow & attack landed** (plan: [PLAN_G29_SERVITOR_AI.md](PLAN_G29_SERVITOR_AI.md)) — **the first gate clause is met**: a summoned servitor now follows its owner and attacks on command. Java's `SummonAI` is a `PlayableAI`, **not** an `AttackableAI`, and that distinction is the design: a servitor trails its owner when idle and **never scans for prey** — it fights only what the owner points it at, through the action bar (`ServitorAttack` 22 / `ServitorStop` 23 / `ServitorHold` 21, delivered by the new `RequestActionUse` 0x56). The NPC think dispatch branches on `ServitorOf` before the `AttackableAI` state machine, which is what stops a servitor hunting on its own (pinned by standing a monster next to one for 200 ticks). An ordered attack seeds hate and flips the intention — the same primitive `GetAgro`/`Confuse` use, since this port's NPC AI re-derives its target from the aggro list each think — and clears the follow flag, or the servitor would drift home between swings. Java's 3000-unit bail (a target further than that from the owner falls back to following, so a stray click can't send the summon across the map) is ported. Following reuses `npc_ai::move_npc_to`, inheriting G21's geodata/pathfinding. **Test trap recorded:** three tests failed at first because the sparring dummy sat at `NPC_OID` — a servitor is spawned through the *runtime* allocator, which starts at `FIRST_NPC_OBJECT_ID`, so the fixture NPC silently replaced it; the same collision `add_test_npc` already warns about, but in a new guise (it bites whenever a test spawns at runtime *before* placing a fixture). **`SummonInfo` landed** (plan: [PLAN_G29_SUMMON_INFO.md](PLAN_G29_SUMMON_INFO.md)) — other players can now see a servitor, closing what was the most glaring gap left by slice 1 (a summon visible only to its owner). **It was far cheaper than sized:** the 338-line Java class uses the **same `NpcInfoType` 37-bit mask format the port already implements for `npc_info`**, helpers and two-block size accounting included, so the real work was the summon-specific component set rather than the mask machinery — a calibration note worth keeping: check whether a big Java packet shares a format the port already has before pricing it. Differences from `NpcInfo`: opcode 0x8B, `TITLE` always present and carrying the **owner's name** (what draws the "of X" label — its own test searches the encoded packet for it), `PVP_FLAG` always present, `NAME` when `displayId != id`, and `SUMMONED` for the spawn animation. Wired at **both** introduction points (enter-world and the region-delta path) so a servitor walking into view is introduced the same way as one already there, with the **owner excluded everywhere** since they hold the `PetInfo` view — Java splits the two the same way. Left at Java's defaults: `relation` (the per-viewer PvP relation isn't resolved at this call site), clan crests, team, reputation, water/fly, enchant, transformation. **Servitor lifecycle landed** (plan: [PLAN_G29_SERVITOR_LIFECYCLE.md](PLAN_G29_SERVITOR_LIFECYCLE.md)): with slices 1-3 a servitor existed, followed, attacked and was visible — what it could not do was **end**. The lifetime was recorded but never enforced, the upkeep item parsed but never charged, and logging out left an ownerless NPC in the world. Ported Java's fixed **5-second** `Servitor.run()` as a self-rescheduling `ServitorLifeTick` (same "dead or gone → stop" contract as the DoT chain): lifetime countdown → "Your servitor passed away" + unsummon; the periodic upkeep item (default **240 s**, 60 for siege weapons) → "a summoned monster uses X" on payment or "not enough items to maintain the servitor's stay" + unsummon on failure; `SetSummonRemainTime` (0xD1, new) for the time bar; and the **2000-unit leash** — which matters more than it looks, because an ordered attack clears the follow flag, so without it a servitor sent at a distant target would simply be abandoned there. Unsummon-on-leave is wired into `net::store_and_remove_player`, covering logout *and* disconnect. **Honest narrowing:** Java stores a servitor in `CharSummonTable` and restores it on reconnect; persistence is a later slice, so for now it goes away with its owner — a behaviour difference, not a bug, and strictly better than the ownerless NPC it replaces. **PetData loader landed** (plan: [PLAN_G29_PET_DATA.md](PLAN_G29_PET_DATA.md)) — a **foundation slice**, stated plainly: it loads the 56 pet templates from `data/stats/pets/*.xml` but does **not** summon a pet. A pet's stats, food item, hunger limit and food capacity come from `PetData` rather than its NPC template, and the summon is keyed by the **collar item** (`itemId` → `npcId`), so the table has to exist before anything can be summoned from it. Two parsing details worth naming: species-wide and per-level `<set>` elements **share a tag name**, separated only by being inside `<stats>` (a test asserts they don't bleed into each other — reading `food` into a level row would be silent and wrong), and `max_meal(level)` clamps to the table's top row like Java. Per-level combat stats are parsed but not yet consumed; the NPC template's stats stand in until pet levelling lands. **Deliberately deferred to the summon slice:** the collar→cast binding — Java's `SummonPet` effect never receives the item, the `SummonItems` handler stashes a `PetItemHolder` on the player and the effect pulls it back out, and this port's `use_item_skills` has no equivalent "this cast came from item X" channel, so that is genuinely new plumbing rather than something to bolt onto a loader. Persistence is also its own slice (a pet's identity is the collar's **object id**, which is how two collars of the same kind stay two different pets; the `pets` table already ships in the dist schema, so it is query work, not migration work). **Pet summoning landed** (plan: [PLAN_G29_PET_SUMMON.md](PLAN_G29_PET_SUMMON.md)): a collar now summons its pet, which follows and is visible to everyone. **The collar→cast channel** was the piece the data slice stopped short of — Java's `SummonPet` effect never receives the item, so `SummonItems` attaches a `PetItemHolder` to the player and the effect pulls it back out; ported as `Player.pending_pet_collar`, set in `use_item_skills` and **taken** (not copied) by the effect so an unused one can't linger into an unrelated cast. **A pet is a servitor plus a collar:** the owner link, follow state and AI all come from `ServitorOf`, which a pet also carries — "owned summon" is the same relationship whether it came from a skill or a collar, so pets inherit follow, attack, stop/hold and the leash for free; `PetOf` holds only the collar object id and the food bar. A pet sets life-time/upkeep to "none" (it is fed instead), so the lifecycle tick leaves it alone. **The collar's object id is the pet's identity** (Java's `pets.item_obj_id`), not the item type — that is how two Wolf Collars stay two different wolves. `summonType` is load-bearing: `PetInfo`'s second byte is 1 for a pet and 2 for a servitor and the client uses it to decide whether to offer the pet inventory and food bar, so one test summons each and reads the byte; the same field pair carries a pet's **food bar** and a servitor's **remaining lifetime**, which is Java's own reuse. **Still open for pets:** persistence (the `pets` table, already in the dist schema, keyed by the collar object id — the gate's "and it persists"), feeding (`PetOf.fed` is tracked and displayed but nothing drains or refills it), pet inventory, exp/level and evolution. **Still open:** see the servitor (needs `SummonInfo` 0x8B, a 338-line *masked* packet — its own slice), it does not follow or attack (the other half of G29's gate; the NPC AI has chase/attack primitives but a servitor takes orders from its owner, not an aggro list), the lifetime deadline is recorded and displayed but not enforced, and there is no unsummon-on-logout, item consumption, master-buff inheritance or persistence. | ⏳ editchar summon/pet subcommands **Pet persistence landed** (plan: [PLAN_G29_PET_PERSISTENCE.md](PLAN_G29_PET_PERSISTENCE.md)): the `pets` table (already in `dist/db_installer/sql/*/game/pets.sql`, though absent from the consolidated dump) now loads with the character into a `PlayerPets` component keyed by collar object id, and writes back through `servitor::sync_pet_row` on every flush and on owner-leave; level/exp/sp/fed/vitals all round-trip. Upsert-per-row rather than the usual delete-then-reinsert reconcile, because a row is keyed by a collar the character can trade away; rows are deleted only when the collar is destroyed (Java `RequestDestroyItem`), which also unsummons the bound pet — object ids are recycled, so an orphan row would eventually bind a stale pet to an unrelated item. Java's exp floor ("avoiding pet delevels"), the Sin Eater's summon-at-owner-level rule and `getPetMinLevel` clamp are ported; the food bar deliberately does **not** refill on summon. `restore` is always written false (auto-resummon needs `CharSummonTable`, `TODO(G29)`). Caught two latent bugs: `PlayerPets` was declared on `PlayerData` but never added to the component insert bundle (silent no-op in production), and `tests/user_info_packet.rs` had been failing to compile on `main` since G19 resurrection + slice 6 added `Player` fields — `--lib` filters never build the `tests/` directory. Next: **feeding** (`PetFood` handler + consumption tick) closes the gate. **Pet feeding landed — G29 gate clause "summon a pet, feed it, and it persists" now met** (plan: [PLAN_G29_PET_FEEDING.md](PLAN_G29_PET_FEEDING.md)): feeding runs through the item's `NORMAL` item-skills, not a flat value — item 2515 → skill 2048 → `<effect name="Feed"><normal>100</normal>`, so a new `SkillEffect::Feed` variant + parse arm was required (7 Feed instances, 9 `PetFood` items; without it food was consumed for nothing). `PetFoodRate` is now a real `Rates.ini` key. Because Java's `PetFood` refuses an unmounted *player*, food can only reach a pet through its own bag, so this slice also ports `PetInventory` (`ItemLocation.PET`, keyed by the **owner's** object id like Java, so it persists through the existing item reconcile), `RequestGiveItemToPet` (0x95), `RequestGetItemFromPet` (0x2C), `RequestPetUseItem` (0x94) and `PetItemList` (0xB3). The 10 s `PetFeedTick` burns the normal/battle rate, floors at zero, auto-eats when below `hungryLimit`%, and nags/starves per Java; `setCurrentFed` clamps at `maxMeal`. Kept Java's quirk that two collars share one pet inventory (no per-pet discriminator on the rows). Added one datapack-backed test asserting the real skill 2048 parses `normal == 100`, since the feeding fixtures hand-build their own skill and would pass through a broken parse arm. `ItemTemplate` gained `Default`. servitor_tests 41 → 51. **Cubics landed** (plan: [PLAN_G29_CUBICS.md](PLAN_G29_CUBICS.md)): chosen over agathions by the learnable-skill ranking — `SummonCubic` has 28 skills / **12 learnable**, `SummonAgathion` 166 / **0** (all off every skill tree), so raw counts would have pointed at 6x the work for unreachable content. `CubicData` loader (207 templates), `SummonCubic` effect, a `Cubics` component (a cubic is **not** a world object — no template/position/AI, so it can't be targeted), and the `CubicAction` tick: cumulative `triggerRate` skill choice, `successRate` rolled after the choice, owner `<hp>` gate, `<range>`, target `<healthPercent>` band, and TARGET/HEAL/MASTER/BY_SKILL target types. `maxCount` counts *actions* not attempts (no charge spent on a failed roll, missing target, dead target or out-of-range). Java's `scheduleAtFixedRate(..,0,delay)` fires immediately on summon. **Fixed a second hard-coded-zero count in `CharInfo`** (`cubic count`, the same shape as the G19 abnormal-visual bug) — cubics were invisible to other players; added `visibility::refresh_char_info`. `MAX_CUBIC` is always 1 on this dist (nothing sets `cubicCount`). cubic_tests 13 + 2 datapack-backed parser tests. **Client-visibility sweep** (plan: [PLAN_G29_CLIENT_GAPS.md](PLAN_G29_CLIENT_GAPS.md)): after the cubics slice found the *second* hard-coded-zero count in a packet builder, ran the check deliberately across all of them. Two live regressions — features that landed in an earlier milestone but never reached the client because the packet was stubbed before them: **`PartySmallWindowAll` summon count** (pets/servitors exist since slices 1-8; now writes Java's per-summon block — object id, `npcId+1000000`, the 1=pet/2=servitor discriminator, name, HP/MP, level) and **`ExSubjobInfo` subclass count** (subclasses landed in G17; Java puts the **base class first** so the count is never 0 even with no subclasses — the client's class list was empty for everyone). Three other zero counts verified as genuinely-absent features; the dead `enter_world::henna_info` stub (superseded by the real `HennaInfo`) deleted. Also replaced the `pet_of`/`servitor_of` store sweeps with a `SummonRef` link on the owner — **closer to Java** (`getPet()` is a field read, not a scan), O(1), and readable from `&World`, which is what the packet builders have; ids are validated on read so a missed clear yields `None`, not a dangling id. servitor_tests 51 → 54 (all 51 pre-existing passed unchanged through the refactor). **Cubic `power` fix** (addendum in the cubics plan): the previous slice flagged template `power` as "parsed but unconsumed" — checking the Java showed the port was consuming the **wrong** thing. `Cubic extends Creature` with `getBasePAtk()/getBaseMAtk()` = `power / 10`, and casts via `skill.activateSkill(this, target)`: **the cubic is the caster, not the owner**. The port passed the owner, so cubic damage scaled off the player's m.atk (Storm Cubic lvl 1 is power=282 → m.atk 28.2, vs a levelled mage's several hundred). Fixed with a stats-only caster entity — `CombatStats`/`Vitals`/`Position` but no `Npc`/`Player`/`RegionCell`/`Movement`, which every store sweep is anchored on, verified by enumerating the `for_each_mut` call sites — despawned with the cubic. Found two more bugs while fixing it: `Cubic.getLevel()` delegates to the **owner's** level (without it every cast resisted and cubics did zero damage — new `CubicOf` component), and `add_components` silently no-ops on an unspawned id (`spawn` first). cubic_tests 13 → 16, incl. one asserting a 500x swing in owner m.atk leaves cubic damage identical. **Pet exp + levelling landed** (plan: [PLAN_G29_PET_EXP.md](PLAN_G29_PET_EXP.md)): slice 7 made level/exp/sp round-trip but nothing awarded them, so every pet stayed at its summon level. A nearby pet's cut comes **out of** the owner's award, not on top — `get_exp_type` (73) is the share the *owner keeps*, the pet takes the remainder, split after the vitality/premium bonuses so it shares them. **A starving pet earns nothing** (`isUncontrollable()` guards `PetStat.addExp`) — a real link between the feeding loop and progression. Levelling advances through every earned level at once, caps at the species table's top level, moves `max_meal` with it, sends no system message (just `SocialAction(LEVEL_UP)`), and stamps the pet's level onto the **collar's enchant level** (`getControlItem().setEnchantLevel`) — which was a separate remaining-work item and turned out to be three lines here. servitor_tests 54 → 62, incl. an end-to-end test through the real reward path (owner keeps 1000 alone vs 730 with a pet in range). Next: per-level pet **stats** are still parsed-but-unread, so a levelled pet's level moves but it doesn't get stronger. **Per-level pet stats landed** (plan: [PLAN_G29_PET_STATS.md](PLAN_G29_PET_STATS.md)): slice 12 levelled pets but combat still read the NPC template, so a levelled pet's number moved while it stayed as strong as at level 1. Following the cubic-`power` lesson, checked *who consumes* the columns first: Java overrides at the **finalizer** level (`MaxHp`/`MaxMp`/`PDefense`/`MDefence`/`calcWeaponBaseValue`/`Regen*`), uniformly substituting the per-level pet row wherever an NPC would use its template base. Ported as `pet_template_at_level` — clone the template with the pet row's stats **and the pet's own level** (which drives `levelMod`) patched in, then reuse the existing `npc_finalized_stats` pipeline, rather than growing a parallel pet stat path that would drift. Levelling preserves the HP/MP **fraction** (a refill would be a free heal; an absolute carry would wound the pet as max HP rose). A row missing a stat falls back to the NPC template — not speculative: without it the shared fixture produced pets at **0 max HP**. `org_hp_regen`/`org_mp_regen` parsed but still unread (`NpcTemplate` has no regen fields — its own slice). servitor_tests 62 → 66, pet_data 2 → 3 incl. datapack-backed assertions on the shipped Wolf's exact stat values and that its top level is strictly stronger than level 1. **Pet death landed** (plan: [PLAN_G29_PET_DEATH.md](PLAN_G29_PET_DEATH.md)), closing the `TODO(G29)` slice 7 left at the restore site: `deathPenalty` (`-0.07×level + 6.5` percent of the **current level's band**, so it shrinks as the pet levels; skipped for duel/arena deaths), `_expBeforeDeath` captured pre-penalty and **not persisted** (Java holds it on the live instance), `restoreExp(percent)` handing back a share and zeroing the record so a second revive restores nothing, a floor so the penalty can't de-level, and a zero-penalty no-op at the species cap where there is no next-level band. A pet stored with `curHp < 1` now restores as a corpse. **Fixture bug found:** the first draft reported "exp lost (6000 → 6000)" — the shared fixture had only two levels, so a level-2 pet was already capped and every death test measured the empty-band case; fixed with a third level and the cap case pinned separately. **Bug found incidentally:** `YOUR_SERVITOR_PASSED_AWAY` was 1519 (written in slice 1) but is **1520** — 1519 is "The pet has been killed…", so expiring servitors told owners their pet had died. servitor_tests 66 → 73; the duel test also puts `is_in_duel` to use, clearing a long-standing dead-code warning. **Pet resurrection landed** (plan: [PLAN_G29_PET_REVIVE.md](PLAN_G29_PET_REVIVE.md)), closing slice 14's dangling `pet_restore_exp` (wired and tested but called by nothing). Java's `Resurrection` calls `effected.getActingPlayer().reviveRequest(…, effected.isPet(), …)` — `getActingPlayer()` on a pet returns its **owner**, so the `ConfirmDlg` goes to the owner, who answers for it; one `_reviveRequested` block on the player carries both cases via `_revivePet`. Ported by turning the five-element proposal tuple into a named `ReviveRequest` struct with the flag. **`PcBody` was rejecting pets** (`targethandlers/PcBody.java` is `!isPlayer() && !isPet()`; the port had only the player half), so a dead pet could not be targeted at all. A pet's restorable exp is the gap the death penalty opened, not `lost_exp_on_death`, so the dialog's number branches on the flag. Reviving restarts the food clock and syncs the pet row. servitor_tests 73 → 78, incl. one pinning that a pet revival does **not** revive a dead owner; all 10 player-resurrection tests passed unchanged through the struct conversion. **Pet corpse decay landed** (plan: [PLAN_G29_PET_DECAY.md](PLAN_G29_PET_DECAY.md)). Slice 15 closed by claiming the corpse "persists indefinitely" and needed Java's 24-hour timer — **both halves were wrong**, and the datapack caught it: `npc_do_die` already schedules decay, `DecayTaskManager.add` has **no pet branch**, no pet NPC template overrides `corpseTime`, and `DefaultCorpseTime = 7`, so Java also decays a pet corpse after **7 seconds**. The "24 hours" in the death message is flavour text that contradicts the mechanic; trusting it would have replaced faithful behaviour with a divergence. The real gap was what happens *at* decay: `Summon.onDecay` → `Pet.deleteMe` transfers the pet's inventory to the owner, then **`destroyControlItem`** — letting a dead pet rot **destroys it permanently** (collar consumed, row deleted). Previously a decayed corpse just despawned, so death cost only the exp penalty and the pet could be re-summoned free. servitor_tests 78 → 82, incl. the slice-15 interaction (resurrecting before decay saves the pet; the decay task fires anyway and must no-op) and a guard that servitors don't take the pet path. **Pet regen landed** (plan: [PLAN_G29_PET_REGEN.md](PLAN_G29_PET_REGEN.md)). The carried-forward claim that `NpcTemplate` "has no regen fields at all" — repeated across three plan docs and three PROGRESS rows — was **false**: the fields are `base_hp_reg`/`base_mp_reg` and `run_npc_regen_tick` already read them; the original grep said `hp_regen`. Second wrong carried-forward TODO in three slices (after the corpse "24 hours"). The real change is ten lines: Java's `RegenHPFinalizer` pet branch substitutes the per-level pet row's regen under `PetHpRegenMultiplier`/`PetMpRegenMultiplier` (now real config keys, 100/×1.0 here — inlining 1.0 would be invisible today and wrong for a retuned server, and a monster-regen retune must not retune pets). Lives in the regen tick rather than `pet_template_at_level` because regen re-reads the template each tick instead of caching. servitor_tests 82 → 86 (incl. a test that sets the monster multiplier to 100× to prove it does *not* apply to pets) + datapack assertions that the shipped Wolf's regen is 2.0 at level 1 and grows. **Summon shots landed** (plan: [PLAN_G29_PET_SHOTS.md](PLAN_G29_PET_SHOTS.md)): the autoshot handler carried an explicit "summon shots aren't in scope" narrowing and `soulshot_count` was unparsed, so pets could not use shots. Java `Summon.rechargeShots` reads the **owner's** auto-shot list, spends from the **owner's** inventory and charges the **summon** — three actors in one flow. Cost is the pet's **per-level** `soulshot_count`, so a levelled pet is more expensive to keep shotted. Java's `isSummonShot` branch checks `hasSummon()` and **never looks at the player's weapon**, so reusing the player grade check would have rejected every Beast Soulshot; it also charges the summon immediately on toggle. `_chargedShots` lives on **Creature** in Java, so NPC attackers were skipping charge/spend entirely — added a `ChargedShots` component for summons (`TODO(G29+)`: fold the player's bits in). A partial stack buys nothing rather than a partial charge. servitor_tests 86 → 92 + datapack assertions that the shipped Wolf's shot cost is 1 at level 1 and grows. Spiritshots parse but stay unwired until pets cast. **`SUMMON` target type landed** (plan: [PLAN_G29_SUMMON_TARGET.md](PLAN_G29_SUMMON_TARGET.md)) — found while sweeping the "Java-on-Creature vs port-on-Player" bug class from slice 18, which led somewhere else entirely: `TargetType::SUMMON` was **never implemented**, so all **18 learnable** summon-targeted skills fell through to `INVALID_TARGET`. Ranked by learnable skills it outranks `NpcBody` (5), `EnemyNot` (4) and `PcBody` (2) **combined**, all of which the port already handled. What was dead: the **entire Summoner support kit** — Servitor Heal/Recharge/Magic Shield/Physical Shield/Haste/Wind Walk/Magic Boost/Empowerment/Cure/Blessing, Mighty Servitor, the four class servitor buffs (Warrior/Wizard/Assassin/Final) and Mass Surrender ×3. A Summoner could summon a servitor and then do nothing for it. Java's quirk kept as written: `getAnyServitor()` is null for a **pet**-only owner (and `hasSummon()` is true for a pet, so the `getPet()` fallback is unreachable), so "Servitor Heal" does nothing for a Wolf owner — thematically right, and pinned by a test so a later "fix" must be deliberate. servitor_tests 92 → 97 incl. a datapack-backed parse check on the real kit. **Summon buff visibility landed** (plan: [PLAN_G29_SUMMON_BUFF_INFO.md](PLAN_G29_SUMMON_BUFF_INFO.md)), running the `Creature`-vs-`Player` sweep slice 19 admitted it had skipped. First two probes came up **clean** (NPCs do get `Buffs`; `apply_buff_to_npc` does recompute stats) — recorded as such rather than manufacturing a finding, and now pinned end-to-end since slice 19 only proved a *heal* lands on a servitor, never that a **stat buff** moves its numbers. The real gap was the NPC buff path's own admission — *"no `NpcInfo` re-broadcast, so a speed change isn't reflected client-side until respawn"* — tolerable for a mob, a bug for a servitor: Servitor Haste and Wind Walk both land in fields `PetInfo`/`SummonInfo` carry and are cast by an owner expecting to see the difference, so the buff worked and looked broken. Summons (only) now re-send `PetInfo`/`SummonInfo` on buff land **and expiry** (without the expiry half the summon keeps showing the buffed speed). The new packet-presence test was **verified to fail with the fix disabled** before being kept. servitor_tests 97 → 99. **Summon PvP flagging fixed** (plan: [PLAN_G29_SUMMON_PVP_FLAG.md](PLAN_G29_SUMMON_PVP_FLAG.md)) — the `Creature`-vs-`Player` sweep's probe with teeth. Java flags `getActingPlayer()`, which for a `Summon` is its **owner**; the port had no equivalent and kept its flag/stance block inside a player-only `else`, so **a summon attacking a player flagged nobody** — exploit-shaped, since a player could set their pet on someone and never go purple while the victim couldn't retaliate without taking the karma. Added `pvp::acting_player` and resolved inside `update_pvp_status_target` so every flagging path gets summons for free. **That alone did not work**: the block never ran for NPC attackers, and only the end-to-end test (a real `do_auto_attack`) caught it — the unit test calling the helper directly passed. The block now runs for both branches gated on the *resolved* actor being a player, which is safe precisely because `acting_player` maps a mob to itself; a test pins that a monster still flags nobody. servitor_tests 99 → 103, pvp/duel/combat/social re-run clean. **Summon kill credit fixed** (plan: [PLAN_G29_SUMMON_KILL_CREDIT.md](PLAN_G29_SUMMON_KILL_CREDIT.md)) — the `getActingPlayer()` audit's biggest find. Java's `calculateRewards` resolves every damage dealer with `info.getAttacker().getActingPlayer()`; the port keyed the aggro list by the dealer's own id and never resolved it, so **a summoner whose pet did the fighting earned nothing** — no exp, no drops, no quest kill credit. The core summoner loop was completely broken. Resolved in the damage-share loop, the looter fallback and `notify_kill`, with range measured from the **earner** as Java does. Resolution creates a new hazard the fix has to handle: an owner fighting *alongside* their summon now appears twice in the aggro list, so shares **merge per resolved player** — a test pins that owner 100 + summon 100 earns the same as a rival's 200. The probe test needed three corrections before it measured anything (no damage history; a real swing lands on a *scheduled* tick; `default_template` awards 0 exp), then was confirmed to fail with the fix disabled. servitor_tests 103 → 105; drop/quest/party/social/combat groups re-run clean. **`getActingPlayer()` audit part 2** (plan: [PLAN_G29_ACTING_PLAYER_AUDIT.md](PLAN_G29_ACTING_PLAYER_AUDIT.md)): two more live bugs from the same root. **PK/karma** — `Player.doDie`'s reputation block reads `killer.getActingPlayer()`, but the port gated on "is the killer a player", so **a summon killing a player produced no PK counter and no karma**: set your pet on someone and walk away clean. **Duels** — `duel_lethal_guard` exists to hold *a duel never kills*, and began with `are_dueling(attacker, …)`; a summon carries no `DuelRef`, so its blow wasn't recognised as duel damage and slipped past the cap, really killing the opponent. A guard whose whole purpose is an invariant was violable by an actor it never considered. Also corrected a test that asserted an *intermediate* (1 HP) rather than the observable outcome — capping ends the duel and `restorePlayerConditions` heals both sides. **Audit is four for four**: every `getActingPlayer()` site probed (flagging, rewards, PK/karma, duels) was a live bug, so the remaining sites (clan-war kill counting, `OnAttackableKill`'s `isSummon` flag) deserve the same treatment. servitor_tests 105 → 107; duel/pvp/death/combat/quest groups re-run clean. **`getActingPlayer()` audit closed** (plan: [PLAN_G29_ACTING_PLAYER_AUDIT_3.md](PLAN_G29_ACTING_PLAYER_AUDIT_3.md)). The last two flagged sites — clan-war kill counting and the clan-war death-exp relief — turned out to be **already covered, by accident**: slice 23's resolution was a `let` shadow part-way down `player_do_die`, and nothing between it and them used the raw id. Coverage by luck, so it is hoisted to the **top of the function** where insertion order can't defeat it, and both behaviours are now pinned by tests (unresolved, a summon killer has no clan, so a victim paid **four times** the exp they should for dying to an enemy's pet). Final tally: **four genuine bugs from four probes** (flagging, reward attribution, PK/karma, duel lethal guard) plus two sites made robust; the only remaining Java call sites are event dispatch this port has no equivalent of. Generalisable finding: **when the reference implementation routes through a resolver, port the resolver, not the common case** — expressing `getActingPlayer()` as "is this a player" compiles, runs, and is wrong only for summons, which no existing test exercised. servitor_tests 107 → 109; clan/death/pvp/duel re-run clean. **Pet equipment landed** (plan: [PLAN_G29_PET_EQUIP.md](PLAN_G29_PET_EQUIP.md)), closing the `TODO(G29)` slice 8 left in `PetInventory::to_rows`. 96 equippable pet-armour items ship on this dist; pet **evolution** has no item handler at all here and is struck rather than scheduled. `PetInventory` already wraps `Inventory`, which owns the paperdoll and every slot rule, so pet armour reuses the player equip path wholesale — as Java does (`PetInventory extends Inventory`) — with the click-to-remove toggle. Two halves had to be added: **stats** (the NPC pipeline has no inventory step, so `recalculate_pet_stats` now sums the pet's own paperdoll via `item_stats`; defensive stats only) and **persistence** (`to_rows` emits `PET_EQUIP` for worn rows, `PET` for carried; the slot already rides in `loc_data`, so renaming the location preserves it and `from_rows` renames back — a pet's armour comes back **on**, not loose in its bag). servitor_tests 111 → 114; inventory/items/char_persistence re-run clean. **Pet reconnect resummon landed** (plan: [PLAN_G29_RECONNECT_RESUMMON.md](PLAN_G29_RECONNECT_RESUMMON.md)), honouring the `pets.restore` column slice 7 hard-coded to `'false'`. `RestorePetOnReconnect`/`RestoreServitorOnReconnect` are **both True** on this dist, so this is the normal path — checking the config first is what made it the pick. The flag is set in `sync_pet_row`, which `on_owner_leave_world` already calls **before** the unsummon precisely so it observes a live pet: no separate logout hook and no way for the two to disagree. Restoring reuses `summon_pet` via `pending_pet_collar` rather than a parallel path, so a restored pet is identical to a freshly summoned one; guarded on the collar still being in the inventory, since it can be traded away between sessions and a dangling holder would leak into an unrelated cast. servitor_tests 114 → 118 (incl. the pet coming back *in the state it left in*, and a missing collar leaving no dangling holder) + `char_persistence` round-tripping `restore` both ways — it is a **string** column in Java, which a bool binding would quietly get wrong. Servitor reconnect (`character_summons`) still open. **Servitor reconnect landed** (plan: [PLAN_G29_SERVITOR_RECONNECT.md](PLAN_G29_SERVITOR_RECONNECT.md)) — a different shape from the pet case: a servitor has no collar, so Java rebuilds it by **re-casting the summoning skill** and stamping the saved vitals/lifetime onto the result. `character_summons` therefore stores a *skill id*, and a restored servitor comes back at the player's **current** level of it. Remaining lifetime is preserved (relogging is not a free duration reset); the row is consumed *before* the re-cast so an unlearned skill isn't retried every login; an empty row is written when nothing is out. **The write nearly cost characters their data**: `DELETE FROM character_summons` with `?` aborts the entire save transaction on any schema lacking the table — six unrelated persistence tests failed on it, which is how it was caught. Now best-effort, same rationale as `load_account_var` but applied to a write, since a failing write inside the transaction takes every other write down with it. servitor_tests 118 → 121 + a real-schema round trip asserting the lifetime survives a relog. **Servitor buff persistence landed** (plan: [PLAN_G29_SUMMON_BUFF_PERSIST.md](PLAN_G29_SUMMON_BUFF_PERSIST.md)), completing slice 27 — the servitor came back but stripped of everything cast on it, arguably worse than not restoring it since slice 19 had just turned on the Summoner support kit. **The remaining-work note was mislabelled**: `SummonEffectTable` is not "master-buff inheritance" (a Freya-era mechanic, **struck** — not on this chronicle) but persistence of the summon's *own* buffs via `character_summon_skills_save`. Third mislabelled carried-forward note this milestone. Reuses `SkillBuffRow` verbatim and restores through `restore_persisted_buffs`, the player's own login path, so a servitor's buffs can't drift from a player's; `ORDER BY buff_index` preserves application order for the slot cap, and expired buffs are filtered at capture. Writes are best-effort, applying slice 27's lesson without relearning it. servitor_tests 121 → 123, asserted on the buff's actual effect (run speed) rather than row presence. **`ServitorSkillUse` landed** (plan: [PLAN_G29_SERVITOR_SKILL_USE.md](PLAN_G29_SERVITOR_SKILL_USE.md)) — the summon's action-bar buttons. `ActionData.xml` ships **105** bindings; the port matched three hard-coded ids (hold/attack/stop) and returned early on the rest, so every one was dead. **13** name a skill the six summonable servitors here actually have (measured before building — the rest bind later-chronicle summons). The `action_data` loader already existed but kept only the id list for `ExBasicActionList`, discarding `handler`/`option`; widened so this is a lookup rather than 105 match arms. Guard that matters: the skill must be in the servitor's **own** `skill_list`, since the table binds every summon in the game and casting blind would let one summon borrow another's abilities. Ordered casts go through `npc_cast::start_cast` behind the same `check_use_conditions` gate as AI casts, so they pay the same MP, mutes and cooldown. servitor_tests 123 → 126 incl. a datapack-backed binding check; the cast test was confirmed to fail with `start_cast` disabled. **G29's summon subsystem is complete for this chronicle**; only pet spiritshots remain (they need pets to cast first). **Summon spiritshots landed — G29 COMPLETE** (plan: [PLAN_G29_SUMMON_SPIRITSHOTS.md](PLAN_G29_SUMMON_SPIRITSHOTS.md)). The "blocked on pets casting" note was **wrong**: `npc_ai_tick`'s summon branch already runs `think_attack` → `try_cast`, so summons have cast since G21 (53 active skills across 56 pet species). Fourth mislabelled carried-forward note this milestone; the check cost one grep. Mirror of the soulshot slice with one real difference — a magic shot is charged before the **cast** and spent by the cast itself, so the charge sits in `start_cast` and the spend in the effect path (Java splits them the same way). Cost is the level's `spiritshot_count`, parsed in slice 18 and unread until now. `apply_skill_effects` read the shot flags off `Player`, so an NPC caster silently got no bonus — **third instance** of that gate shape in this subsystem. Blessed Beast Spiritshots don't exist here, so only the ×2 tier is reachable. servitor_tests 126 → 130, with the bonus measured by running the same cast charged and uncharged rather than asserting a flag. |
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
    `//gmchat`/`//announce`/`//announce_crit`/`//announce_screen`/`//worldchat`
    (`//announce_screen` now sends a real **`ExShowScreenMessage`** top-centre
    banner — new reusable packet `server_packets::ex_show_screen_message`, text
    variant, `MULTILANG` branch skipped; the NpcString/parameterised variants
    and its boss/quest consumers (Antharas taunt, Q261 newbie reward) are a
    later add),
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
