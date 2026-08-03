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
| Game  | G7.8 Geodata & position validation                          | ✅ (`.l2j` loading, LOS, move clamping, ValidatePosition — ~~zones still ⏳~~ zones landed at G12) |
| Game  | G7.85 Pathfinding (path-worker service)                     | ✅ (`CellPathFinding` port, dedicated worker thread + channels, multi-segment route following for player moves — NPC moves still straight-line) |
| Game  | G7.9 Region-grid visibility & scoped broadcasting           | ✅ (CharInfo/DeleteObject, 3×3 region knownlist, region-scoped broadcasts) |
| Game  | G8 Static world content (NPCs/spawns)                       | ✅ vertical slice (34.9k NPCs spawned, visible, targetable, talkable — ~~zones/doors still ⏳~~ both landed at G12) |
| Game  | G9 Combat & AI                                              | ✅ vertical slice (auto-attack, monster AI, death/decay/respawn, XP/SP/level-ups, auto-loot drops, die→revive) |
| Game  | G9.5 ECS stage 2 — split components, one world              | ✅ (plan: [PLAN_ECS_STAGE2.md](PLAN_ECS_STAGE2.md)) |
| Game  | G9.6 Macros & panel shortcuts                               | ✅ (plan: [PLAN_MACROS_SHORTCUTS.md](PLAN_MACROS_SHORTCUTS.md)) |
| Game  | G10 Social systems                                          | ✅ vertical slice (chat, party, friends — clans/mail/BBS deferred) |
| Game  | G11 Scripting engine + quests (+ clans via bypass)          | ✅ vertical slice (bypass routing, quest engine, Q00258/Q00320, clan creation — plan: [PLAN_G11_QUESTS_CLANS.md](PLAN_G11_QUESTS_CLANS.md)) |
| Game  | G12 Static world + script/content breadth                   | ✅ vertical slice (zones peace/water/no-restart, all 1180 doors + geo collision, static objects, Link/Buy bypasses, +10 quests with on_attack/on_spawn hooks, OrcChange1, TeleportWithCharm — plan: [PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md](PLAN_G12_STATIC_WORLD_AND_CONTENT_BREADTH.md)) |
| Game  | G13 Admin / GM command system                               | ✅ **COMPLETE** (2026-07-31: the row's one named gap, `//manor`, is ported — `admin/castle.rs`; `//clan_show_pending`/`//clan_force_pending` landed with the clan-leader sweep) — G13.A framework done; **G13.B portable handlers landed** (B1–B7 + mounts + transform runtime: character/skill/item/spawn/movement/GM-util/world/vitality/ride/transform + geo queries + `//admin` menu); remaining: subsystem-blocked C-group (sieges/olympiad/instances/…) + a few field-less/serializer stubs, 2026-07 category-4 sweep LANDED: invis family + `//debug` panel, `//rec`, pet/summon admin (`//unsummon`/`//summon_info`/`//summon_setlvl`/`//show_pet_inv` + GMViewItemList 0x9A), `//strict_find_dualbox`, the AdminPunishment console (+`//ban_menu`/`//unban_menu`), `//force_peti`, `//respawnall`/`//unspawnall`/`//spawn_reload`, `//clan_changeleader` (forced `setNewLeader` + UpdateClanLeader row), `//add_clan_skill`, `//play_sounds`, `//effect_menu`, `//event_menu` (G28 engine list), `//bbs`, `//viewblockedeffects`. Batch 3 LANDED: server control (`//server_shutdown|restart` countdown with Java announce marks → graceful game-thread stop, `//server_abort`, `admin_server_*` runtime `ServerStatus` pushes over the login link), olympiad manual controls (`//endolympiad` → `handle_olympiad_end`, `//saveolymp`, `//settruehero`→`//sethero`), quest admin (`//charquestmenu`/`//show_quests` panel + `//setcharquest` incl. state DELETE). Tail polish LANDED: `//tradeoff` (new `Player.trade_refusal` gate in TradeRequest), `//exceptions`/`//set_exception` (PlayerCondOverride bitmask; SEE_ALL_PLAYERS consumed by the visibility describe path), `//quest_info` (registry listing/detail), `//clanhall` (list + give/take over the auction owner setters), `//reload` (config/access/npc/skill/item/multisell/buylist/teleport/fishing), `//switch_gm_buffs` (Java's nothing-to-switch outcome on this dist's config). STILL not implemented (low-value tail): `//clan_force_pending` (no ClanEntryManager), item-attribute `//setl*` (no per-item attribute storage), `//delete_group`/`//instance_spawns`/`//instancezone` (no named-group/reuse-time stores), `//setconfig`/`//config_server`, `//set_mod`, and remaining dev tools (zone_visual, fight calculator, forge, missing-htmls; geodata *editing* commands exist — `//geoenable*`/`//geodisable*`). The Debug panel is now FULLY live: `ExServerPrimitive` (FE:11) ported with the doors (12-line box, 3 s), geodata NSWE-arrow grid (41×41 cells, 1.5 s) and movement-line (100 ms) visualizers as per-GM scheduler loops (`admin/debug_draw.rs`) — plans: [PLAN_G13_ADMIN.md](PLAN_G13_ADMIN.md), [PLAN_G13_B_LOGIN.md](PLAN_G13_B_LOGIN.md). **[Verified 2026-07-31]** — G13 legitimately stays 🚧, and now with a number: a mechanical diff of `dist/game/config/AdminCommands.xml` (443 distinct commands) against the Rust dispatch gives **361 dispatched / 82 absent**. The C-group is no longer subsystem-blocked (sieges G24, olympiad G25, instances G27 all landed — `//clanhall`, `//endolympiad`, `//instance*`, `//grandboss`, `//zone_check` are live). Every one of the 82 falls into a category that is *not* ordinary parity work: **(a) off-chronicle content** — fort sieges (9), territory war (5), fences (5), Gracia/Hellbound (`gracia_seeds`, `set_sodstate`, `kill_tiat`, `hellbound*`), siegable clan halls, the elemental item-attribute setters (`//setl*`, 7 — Kamael/Gracia, and no per-item attribute storage here); **(b) dev/ops tooling** — `//fight_calculator*`, `//forge*`, the missing-html crawlers (3), `//zone_visual*`, `//path_find`, `//skill_test`, `//setconfig`/`//config_server`, `//set_mod`, `//delete_group`, `//server_login`; **(c) architecturally N/A** — `//quest_reload`, `//script_load`/`//script_unload`/`//script_dir`: the port compiles its scripts in, so there is no runtime script loader to drive them (G22's gate lists these as 'unblocked by G22', which cannot apply here); **(d) unreachable in Java too** — `//set_vitality_level`, `//tvt_add`/`_advance`/`_remove`, `//camera`, `//pointpicking` are in `AdminCommands.xml` but **no handler class registers them**, the same dist quirk as `//mammon_find` (`AdminVitality` registers `//set_vitality`/`//full_vitality`/`//empty_vitality`/`//get_vitality`, all of which *are* dispatched); **(e) deferred by an unported feature** — `//fakechat` (FakePlayers), `//clan_force_pending` (no ClanEntryManager), `//instancezone*`/`//instance_spawns` (no reuse-time store). ~~**One genuine portable gap found: `//manor`**~~ **LANDED 2026-07-31** (`admin/castle.rs::admin_manor`) — the read-only status page: current mode name (Java's bare `ManorMode.toString()`), the next mode change as `dd/MM HH:mm:ss`, and each castle's current *and* next-period seed/crop cost, castles in id order. The next-change instant is **derived from the same function that arms the timer** (`manor::next_mode_change_at`) rather than cached in a field as Java does, so the page and the scheduler cannot drift. While wiring it, the port's **third** copy of Howard Hinnant's civil-from-days was about to be written — the calendar math now lives once in `commons::util::civil_from_days`, with `format_date`, the new `format_day_month_time` and `admin/premium.rs::format_datetime` all expressed on top of it. 1 test, all five mechanisms (dispatch, next-vs-current period list, id ordering, mode name, the change stamp) sabotage-verified. **This was the last portable admin gap**; the other 81 absent commands remain off-chronicle, tooling, N/A or feature-blocked. ~~Also still stubs rather than ports: `//geosave` and `//geoedit`~~ **2026-08-01: THE WHOLE GEO SURFACE IS PORTED** — `//geogrid [off]` (`admin/debug_draw.rs::admin_geogrid`, Java's one-shot `GeoUtils.debugGrid`/`hideDebugGrid` over the renderer the Debug panel's geodata toggle already used — Java's own layering, since `AdminDebug.setGeodataDebugging` refreshes by re-issuing the command), then the editor half in the new `admin/geo_editor.rs`: the heading-rotated `//geoedit`/`//ge` community-board panels, the eight `//en`/`//dn`/… aliases with `<geoX> <geoY>` + panel re-open, and `//geosave`/`//geosaveall` on a real `.l2j` **region serializer** (`geo::region::Region::write_to`: overrides folded into the cells they edit, Java's flat→complex promotion included, untouched blocks copied byte for byte; `//geosaveall` on a worker thread because Java's inline 1.4 GB write would stall the game loop). `AdminPathNode`'s `//path_find` landed with them. 11 tests (6 admin + 5 serializer units), every mechanism sabotage-verified. The **only** geo commands still absent are ones Java itself cannot reach: `//pointpicking` (in `AdminCommands.xml`, no handler class registers it) and `//geomap_missing_htmls` (the html crawler, dev tooling), plus `GeoEngine.traceTerrainZ` — a public method with no caller anywhere in the Java tree. |
| Game  | G13.9 TODO parity sweep                                     | ✅ UserInfo weapon-enchant + party/clan relation; skill-acquire SMs; restoration enchant roll; stat-cap/run-speed config plumbing; skill-cooldown persistence (`character_skills_save`) — plan: [PLAN_G13_9_TODO_SWEEP.md](PLAN_G13_9_TODO_SWEEP.md) |

**Remaining subsystem breadth — [ROADMAP.md](ROADMAP.md) (G14→G33).** The old
single "G14 Long tail" is broken into per-subsystem milestones; each unblocks a
gated-but-bodiless admin handler, so admin parity == Java parity. A 2026-07
audit of the Java surface added six milestones the breakdown missed (G15.5
teleporters/user-commands, G15.7 crafting, G20.5 recommendations, G24.5 boats,
G26.5 lottery/monster-race, G30.5 item auction), per-milestone audit
additions, and a Classic/custom scope gate — see ROADMAP.md.

| Game  | G14 Item stats & equipment combat accuracy                  | ✅ item `<stats>`/weapon+armor bonuses (earlier) + **shields (`calcShldUse`)** + **`//setparam`/`//unsetparam`** (fixed-stat override); armor sets → G19; `SHOTS_BONUS` stat a noted micro-gap (only `reducedSoulshot` weapons) |
| Game  | G15 Economy & item actions                                  | ✅ destroy + **ground items** (drop/pickup/visibility/auto-loot=false/decay) + **personal warehouse** (deposit/withdraw+persist) + **crystallization** + **merchant sell** (2026-07-28: `is_sellable` template gate — non-sellable items like the event Agathion bracelets no longer listed/accepted — + **refund buy-back tab**: `PlayerRefund` 12-slot container, `RequestRefundItem` 0xD0:0x72, `ExBuySellList` refund section) + **private sell store** + **player trade** landed; **enchant** (chance engine `EnchantData` + full Ex-packet scroll flow: use→add→put-target→enchant, success +1 / safe / blessed / destroy+crystallize; item `etcitem_type`/`enchant_enabled` parse) + **clan warehouse** (shared container on `Clan`, `depositc`/`withdrawc` bypass + `ActiveWarehouse` routing + `CL_VIEW_WAREHOUSE` gate, persisted via `StoreClanWarehouse`) + **freight withdraw** (`Freight` container, `package_withdraw`, `loc="FREIGHT"` persist; unified 3-way `ActiveWarehouse` routing) + **augmentation** (`VariationData` roll engine + refine flow: confirm→refine→cancel, life stone rolls two options, consumes gemstones, stamps `ItemInstance` augment, adena cancel fee; shown via `paperdoll_augmentation`, persisted via `item_variations`) + **enchant support items** (`EnchantSupport` load + validate, put/remove 0x4A/0xE4, bonus-rate + random-step on the roll) + **item-skill cast branch** (`ItemSkillsTemplate`'s instant `triggerCast` vs `useMagic`: `withoutAction` + `immediate_effect`/`ex_immediate_effect` parsed, so scrolls now cast for their real `hitTime` — SoE 20 s, Scroll: Might 4 s — instead of firing on double-click; `checkConsume` ported via `default_action`/`itemConsumeId`) landed. ~~augment option effects, freight send half pending; `SKILL_REDUCE_ON_SKILL_SUCCESS` still consumed by the handler~~ — **all three landed in audit row 9** (2026-07-30): `data/option_data.rs` applies an augment's two options as passive buffs on equip, `RequestPackageSend` completes the freight half, and the trigger item now rides `CastState` and is spent in `finishSkill`.  **[Verified 2026-07-31]** — every gate clause is met and pinned: warehouse round-trip (`char_persistence` + `warehouse_tests`), private-store sale (`private_store_sell_and_buy`), trade (`player_trade_swaps_items`), enchant incl. break (21 enchant tests), ground loot (`drop_and_pickup_ground_item`). Two `TODO(G15)`s remain, both narrow and unrelated to the gate: the item-use busy check (`!isPotion && !isElixir && !isScroll`) and `checkInventorySlotsAndWeight`'s inventory-full refusal. **Fix (2026-08-02) — pickup ignored distance:** clicking a ground item called `pickup_ground_item` straight from `target::handle_action`, so loot flew into the bag from anywhere on the map (the branch even said so: *"pick it straight up (the walk-to-item approach path is a simplification)"*). Java never picks up on the click — `handlers.actionhandlers.ItemAction` only sets `AI_INTENTION_PICK_UP`, `CreatureAI.onIntentionPickUp` fires `moveToPawn(object, 20)`, and `Player.doPickupItem` runs later out of `PlayerAI.thinkPickUp`, once `maybeMoveToPawn(target, 36)` reports arrival. Ported as a fourth `PlayerIntent::PickUp` variant driven by the existing `player_combat_tick` think loop: `combat::start_pickup_intent` (the REST + `isCastingNow` refusals, both a bare `ActionFailed`) → `player_pickup_think` (36 + collision-radius reach, `checkTargetLost` when the item decays or someone else lifts it first, then `setIntention(IDLE)` + `doPickupItem`). Because a ground item carries no `Vitals`/`Collision`/`CombatStats` it is not a `Combatant`, so `chase_target` was split into a `chase_pawn` half taking an already-resolved pawn, and a new `stationary_pawn(Position)` builder feeds it a zero-extent one — Java's `maybeMoveToPawn` likewise adds only the *actor's* collision radius when the target is not a `Creature`. The `!player.isFlying()` gate from `ItemAction` came with it (a wyvern rider's click does nothing, and gets no packet back). The REST check stays duplicated inside `pickup_ground_item` for the callers that bypass the intention — auto-play looting — and for sitting down mid-walk. 2 tests; the distance one is sabotage-verified (reverting the wire makes it lift 400 adena from 500 units away). `TODO(G24)` left at the site: `ItemAction`'s mercenary-ticket refusal (`SiegeGuardManager.getSiegeGuardByItem` + `CS_MERCENARIES`) needs tickets, which aren't modelled. **Fix (2026-08-02) — drops ignored the packet's coordinates:** `RequestDropItem` carries `dqddd` (object id, count, **x/y/z**), and the port read the trailing three ints only to throw them away — every discarded stack landed on the character's own position, so dragging an item out of the bag piled loot underfoot instead of tossing it where the cursor was. The location is now honoured and validated the way Java validates it, twice: `RequestDropItem.runImpl`'s `!isInsideRadius2D(_x, _y, 0, 150) || (Math.abs(_z - player.getZ()) > 50)` box (SM 151 *You cannot discard something that far away from you*, so a client cannot post items across the map), and then `Item.dropMe`'s `GeoEngine.getValidLocation(dropper, x, y, z)`, which walks the cell line and stops the item at the last walkable cell — a drop can't be thrown through a wall or a closed door. The **where** gate came with it: `ZoneId.NO_ITEM_DROP` needed `ConditionZone`, which was not a loaded zone kind at all, so `data/zones/no_drop_item.xml` (7 zones — Steel Citadel's bascule bridge, the Underground Coliseum floors) is now parsed into `ZoneKind::Condition` + `ConditionZoneParams` (`NoItemDrop`/`NoBookmark`; the latter has no Interlude consumer) and queried by geometry via `no_item_drop_at` — the u8 membership mask is full. The rest of Java's guard chain landed in the same pass, in Java's order: `isDead`, a loaded pet collar (`PET_COLLAR` + `havePetInvItems`), `_count > item.getCount()` (Java **refuses** with SM 729 rather than clamping, so a forged count can't over-drop), `JailDisableTransaction` (new General.ini key, dist `False`), private store (SM 1065), fishing (SM 1470), `isFlying`, `hasItemRequest` (SM 4148), `TYPE2_QUEST` (SM 603) and the casting-a-known-skill refusal. The casting refusal quotes the skill by name the way Java's does (`"…while casting " + skill.getName() + "."`), which needed `SkillData` to keep `<skill name="…">` — parsed all along but thrown away. Held **per id** (the name is on the `<skill>` element, so every level and enchant sub-level shares it), which is the same data as Java's per-instance copy for a fraction of the strings. The 15 dist skills declaring `name=""` are stored as *absent* rather than empty, so the message falls back to the nameless sentence instead of Java's literal "…casting ." — a deliberate cosmetic deviation, pinned by a census test. 7 tests; four are sabotage-verified (dropping at the player's position, dropping the z test, dropping the zone test, and dropping the skill name each make one fail). |
| Game  | G15.5 Teleporters & user commands                           | ✅ **gatekeepers live** (`TeleporterData` — all dist lists; `showTeleports`/`showTeleportsHunting`/`teleport` bypasses gated on the Teleporter class; fee suffix + adena charge, free ≤ `MaxFreeTeleportLevel` (40), karma gate) + **`/unstuck`** (`BypassUserCmd` 0xB3 → 30 s escape cast of 2099 via forced hit-time, GM 2100; new `Escape TOWN` skill effect → map-region town respawn; `teleport_player` now runs `teleToLocation`'s full prologue — `ActionFailed` + `abortCast()` (`MagicSkillCanceled`, or the escape FX kept playing at the destination for the client's own 5-minute skill duration) + `setTarget(null)` — before `decayMe`) + **`/loc`** (region `locId` SM + coords) + **newbie support magic** (`bypasshandlers/SupportMagic` + `SupportBlessing`: `SupportMagic`/`SupportMagicServitor`/`GiveBlessing` verbs on the Newbie Helper/Guide/Gatekeeper htms → fighter/mage buff sets + Blessing of Protection 5182, gated on level/class-tier via `CategoryData`; NPC cast animation; `ProtectionBlessing` lands icon-only). ~~Pending: teleport bookmarks, remaining user commands (`/time` needs game clock), Mon/Tue fee discount (wall clock), nobles lists (G17), siege gates (G24)~~ — **all closed in audit row 9** (2026-07-30): the full user-command sweep, the Mon/Tue 20:00+ half price, the subclass fare, both siege gates, the combat-flag gate and the noble list page. **Teleport bookmarks are not portable** — `EX_BOOKMARK_PACKET` (0x4E) is registered with a `null` handler in this Java build, so the feature does not exist here.  **[Verified 2026-07-31]** — both gate clauses met and pinned (`a_subclass_pays_the_teleport_fee` + the fee/discount tests; `unstuck_casts_escape_and_teleports_to_town`). One `TODO(G15.5)` remains: an augment option's *active*/activation skills are parsed but not granted. **Fix 2026-07-31:** `Teleporter.showChatWindow`'s castle-ground check resolved through `findNearestCastle` (unbounded nearest) instead of `CastleManager.getCastle(x,y,z)` (strict `checkIfInZone`), so *every* town gatekeeper counted as standing on castle ground and answered `castleteleporter-no.htm` ("How dare you talk to me!") — no one could teleport anywhere. Now uses `siege_castle_at` containment (`town_gatekeeper_is_not_on_castle_ground`). **Fix 2026-08-01:** `/unstuck` emitted its two lines in the wrong order — the handler's "You use Escape: 30 seconds." chat line went out *before* the cast, so the client showed it ahead of the skill's own `YOU_USE_S1` ("You use Escape (5-minute).", named client-side after 2099). Java's `SkillCaster.castSkill` runs phase 0 synchronously, so `startCasting`'s SM + `SetupGauge` land first; the cast now goes first here too. Same fix gates the chat line on the cast actually starting — Java answers a null `SkillCaster` with `ActionFailed` + `setIntention(AI_INTENTION_ACTIVE)` and no message (`Unstuck.java:135-141`), where the port announced the escape even when it was refused (`unstuck_says_nothing_when_the_cast_is_refused`). Still open elsewhere: servitor buffs + Vampiric/Concentration/Cubic effects + PK-damage immunity (TODO(G-pvp)) |
| Game  | G15.7 Crafting & recipes                                    | ✅ vertical slice — recipe book (learn via recipe item / destroy / open — including the "Common Craft" 1322 / "Dwarven Craft" 1321 skills, whose `OpenCommonRecipeBook`/`OpenDwarfRecipeBook` effects open the window), synchronous self-craft (material+MP/HP consume, success roll, masterwork rare), and manufacture stores (set list / click→sell list / buy-a-craft with adena fee). `AltGameCreation=False` so no staged craft/XP; `StoreRecipeShopList=False` so stores are transient. Plan: [PLAN_G15_7_CRAFTING.md](PLAN_G15_7_CRAFTING.md) |
| Game  | G16 Character variables, premium & vitality                 | ✅ **admin main-menu slice landed** (`//admin` Item/Teleport/Spawn/ListPos/ListSpwn/goPosition/goSpawn/PC-Points/NCoins/Premium/Open/Close/Heal/Full-Food — plan: [PLAN_G16_ADMIN_POINTS.md](PLAN_G16_ADMIN_POINTS.md)): character-scoped `pccafe_points`, account-scoped `account_gsdata` "PRIME_POINTS" store (`//primepoints`), boot-loaded `account_premium` cache + write-through (`//premium_*`), `ExPCCafePointInfo`, spawn-line `tele_index`; Full-Food a pet-blocked `TODO(G29)` stub. **Henna slice landed** (plan: [PLAN_G16_HENNA.md](PLAN_G16_HENNA.md)): `HennaData` (372 dyes) + `HennaSlots` component, dye stat bonus folded into `BaseStats` (= template + Σ dyes, recomputed on draw/remove), `character_hennas` load/persist, the full `RequestHenna*` packet family + `HennaInfo`/`HennaEquipList`/`HennaRemoveList`/`HennaItemDrawInfo`/`HennaItemRemoveInfo`, SymbolMaker `Draw`/`Remove` bypass; permanent dyes only (`duration=-1` on this dist). **Vitality + variables + premium effects slice landed** (plan: [PLAN_G16_VITALITY.md](PLAN_G16_VITALITY.md)): `character_variables` key/value store (`PlayerVariables` component, load + transactional persist), the vitality pool (`game_loop/vitality.rs` — clamped 0..=140k, set/update with the 4 notices, `ExVitalityPointInfo`, party-window field), the ×2 exp/sp bonus folded into `add_exp_and_sp`'s new `use_bonuses` arg (quest/admin rewards opt out, like Java's 2-arg overload), per-kill consumption (`Attackable.getVitalityPoints`, solo + party branches), a real `Custom/PremiumSystem.ini` loader + `hasPremiumStatus` + PremiumRateXp/Sp on the reward path, real `ExVitalityEffectInfo` fields, and `StartingVitalityPoints` at creation. Remaining: the daily/weekly refills (`TODO(G33)` — needs the wall-clock daily-task scheduler, so **vitality only drains** today), vitality *items* (counter stored but nothing increments it), PC_CAFE_RETAIL_LIKE, and the `VITALITY_CONSUME_RATE`/`BONUS_EXP` stats (`TODO(G19)`) **Vitality decrease notice removed (2026-08-01, operator request).** Java's `PlayerStat.setVitalityPoints` sends `YOUR_VITALITY_HAS_DECREASED` (2316) on every downward move of the pool, and since every monster kill drains it the line fired on essentially every kill — chat spam, not information. `set_vitality_points` now sends only the increase line; the at-maximum / fully-exhausted edge lines are kept (they are rare), and `ExVitalityPointInfo` / `broadcastUserInfo` / the party window field are untouched, so the client still shows the drain on the gauge. The `sm_ids` constant is kept, documented as never-sent, so the deviation stays greppable. 1 new test (`ordinary_drain_sends_no_system_message`) plus the absence pinned in `set_clamps_and_announces`; both sabotage-verified against the old behaviour. **2468 gameserver tests green**, clippy clean. |
| Game  | G17 Sub-classes, class change & nobless                     | ✅ **nobless landed** (plan: [PLAN_G17_NOBLESS.md](PLAN_G17_NOBLESS.md)) — `characters.nobless` was **read at login and dropped on the floor**: it never reached `Player`, nothing consumed it, and it wasn't in the save UPDATE, so it couldn't be set either. Now `Player.is_noble` + `nobleSkillTree.xml` (**8** skills — the tree loader skipped every non-`classSkillTree` block, so this file had never been parsed) + `//setnoble` mirroring `//sethero` + persistence. Noblesse teleport lists now check nobless instead of refusing everyone. **One rule differs from hero deliberately**: `setHero` only grants while on the base class, `setNoble` has no such gate — nobless belongs to the character, so a subclass keeps it (tested, and it matters once subclasses land). **Subclasses landed** (plan: [PLAN_G17_SUBCLASSES.md](PLAN_G17_SUBCLASSES.md)) — **G17's gate headline**. Nothing existed: `class_index = 0` was hard-coded in six places in `db.rs` (each commented "no subclasses on this dist") and `character_subclasses` shipped in the schema but was never read or written. Now `Player.class_index` + `subclasses`, `add_subclass`/`set_active_class`, `StoreSubClass`, and `//setsubclass`/`//changesubclass`. **The banking is the whole mechanic**: class/level/exp/sp belong to the *active* slot, so a switch must write the current slot back before loading the target's (Java calls `store()` before touching `_classIndex`). The base class needed the same treatment and had nowhere to go — its `characters` row holds whatever class is active, so a level-7 base who switched to a level-40 subclass would return as level 40; `Player` now stashes `base_level`/`base_exp`/`base_sp` for that round trip, pinned by a test. Narrowed: **per-subclass skills aren't persisted yet** — a switch re-derives the auto-granted tree via the same `set_level` path `//setclass` uses, so a *manually learned* skill is lost on the round trip; `character_skills` needs a real `class_index` key, which is the next slice. Hennas/shortcuts still load at index 0. **Per-subclass skill books landed** (plan: [PLAN_G17_SUBCLASS_SKILLS.md](PLAN_G17_SUBCLASS_SKILLS.md)) — closing that gap: `character_skills` is now read and written per `class_index`, a switch banks the outgoing book and restores the incoming one (Java's `removeSkill`-all → `restoreSkills` → `rewardSkills`), and **a character who logs out on a subclass logs back in on it** (the active index is whichever subclass row carries `characters.classid`). The regression test — a hand-learned skill surviving a switch away and back — fails against the previous slice. **Per-subclass hennas + shortcuts landed** (plan: [PLAN_G17_SUBCLASS_HENNA_SHORTCUTS.md](PLAN_G17_SUBCLASS_HENNA_SHORTCUTS.md)) — same `class_index` treatment; dyes re-fold into `BaseStats` on the swap via `apply_henna_change`, which also pushes `HennaInfo` exactly as Java's `setActiveClass` does. **Village-master subclass flow landed** (plan: [PLAN_G17_VILLAGE_MASTER_SUBCLASS.md](PLAN_G17_VILLAGE_MASTER_SUBCLASS.md)) — the mechanic was GM-command-only; now the `Subclass` bypass on the dist's **46** VillageMasters drives it (menu/add-list/change-list/add/change). **Level 75 + free slot are enforced on the action, not just the list**, so a stale link can't slip past. `available_subclasses` ports `getAvailableSubClasses`: every `THIRD_CLASS_GROUP` entry minus the base **lineage** (Java's "similar class" rule), minus held classes and their children, minus Overlord/Warsmith, minus the Elf↔Dark-Elf cross. **Class race needed a lineage walk** — `PlayerTemplate::race()` only answers for *creatable* classes, so an advanced class returns `None` and the Elf rule would have silently disabled itself. Tested against the **real datapack** so the hierarchy/category groups are the shipped ones. Survey note: **certification skills are absent from this dist** (later-chronicle) — struck rather than stubbed. **Occupation change landed** (plan: [PLAN_G17_OCCUPATION_CHANGE.md](PLAN_G17_OCCUPATION_CHANGE.md)) — `Player.setClassId` as a shared mechanic, and it **fixes a hazard the subclass slices created**: `//setclass` set `base_class_id` unconditionally, which was harmless with one slot but would **rewrite the character's base class while standing on a subclass**. Java updates only the active slot (`getSubClasses().get(_classIndex).setClassId(id)`), touching `_baseClass` solely on the base slot. Now: base slot moves both, a subclass moves only its own stored class (re-persisted so it survives a restart), plus `rewardSkills`, the stat/UserInfo refresh, and Java's class-change flash (`MagicSkillUse` 5103). `//setclass` is rewired onto it. The key regression test was **verified to fail against the old behaviour** before being kept. Pattern worth remembering: when a new axis appears (here "which slot am I on"), every existing writer of the affected field becomes suspect, not just the new code. **Skill cooldowns per class index landed** (plan: [PLAN_G17_SKILL_COOLDOWNS.md](PLAN_G17_SKILL_COOLDOWNS.md)) — **G17 complete**. Expected to bank cooldowns per slot like skills/hennas/shortcuts; reading `setActiveClass` first showed Java calls **`resetTimeStamps()`**, i.e. a switch **wipes** them — which also closes the exploit of parking a long reuse on one class and sitting it out on another. Also fixed the IO: reuse rows now load and save under the *active* class index, where before a character on a subclass saved its cooldowns onto the base slot. Buff restore (`restore_type = 0`) remains unported and was never G17's. **Certification skills struck** — no data on this dist |
| Game  | G18 Clans — full                                            | ✅ **all 8 slices landed + G18.6 (clan academy) — G18 COMPLETE** *(the 2026-07-31 verification pass found the ROADMAP gate also names **sub-pledges + academy**; sub-pledges were already built, the academy was not. **G18.6 landed 2026-07-31**, closing all 9 `TODO(G18.6)` markers — see the academy paragraph in this row.)* **G18.6 — the clan academy** (`game_loop/academy.rs`): `lvl_joined_academy` / `apprentice` / `sponsor` now load, live on `Player` and persist through two dedicated commands (`UpdateCharAcademyLevel`, `UpdateCharApprenticeSponsor` — Java writes the pair **even for online players**, "since both must match"). **Graduation is the feature**: `Player.setClassId`'s academy block runs *before* the class changes, so completing the **2nd** class transfer (Java's `THIRD_CLASS_GROUP` — the base class is the first group) pays the clan on Java's sliding scale (`CompleteAcademyMaxPoints` 650 at joining level ≤16, `Min` 190 at ≥39, `max - (joined-16)*20` between — three arms, not a clamp), expels the graduate with **`removeClanMember(id, 0)`**, i.e. *no* rejoin penalty, and hands over the Clan Academy Circlet (8181). The reward reads the level the member **joined** at, which is why it is stamped at accept time and cleared in exactly three places (graduation, leaving the clan, dissolution). **Restrictions**: an academy member cannot be re-ranked or nominated clan leader (SM 1754), and clan-war kills are exempt on **either** side (Java `!isAcademyMember() && !pk.isAcademyMember()`). **Mentorship**: `RequestPledgeSetAcademyMaster` (ex 0x12) + the new `CL_APPRENTICE` privilege (ordinal 8) pairs/unpairs a sponsor with an academy member — the packet names both by *name* in either order and Java decides by pledge type, so the port does too; refuses when either side already has a link; login notifies the partner (SM 1756/1758). `PledgeReceiveMemberInfo` now carries the real pledge type, the **sub-unit's** name for a sub-unit member, and `getApprenticeOrSponsorName()` (including Java's literal `"Error"` when the id names nobody). **Per-tab rosters**: `PledgeShowMemberListAll` is now sent Java's way — one packet per sub-unit then the main pledge **last** (the leading `!isSubPledge` int is what closes the set), and the per-member `hasSponsor` flag is real. **Verified skip: squad skills.** `subPledgeSkillTree.xml` needs clan level 8+ and Knight's Epaulettes (9910/9911) and its own comment marks it "Confirmed CT2.5" — unreachable on Interlude, so the `RequestAcquireSkillInfo` SUBPLEDGE arm stays unanswered *by decision*, documented at the site rather than left as a TODO. Remaining TODO(G18.6): an offline member's apprentice/sponsor name (the pair lives on `Player`, not on the roster row) and sub-pledge-leader cleanup on removal. 6 tests, every mechanism sabotage-verified. | (plan: [PLAN_G18_CLANS.md](PLAN_G18_CLANS.md)) — **slice 1** membership lifecycle: invite flow (`RequestJoinPledge` 0x26 → `AskJoinPledge` via the shared `PendingRequest` transaction slot → `RequestAnswerJoinPledge` 0x27 with Java's re-checked `checkClanJoinCondition` guard chain: CL_JOIN_CLAN priv, self/wrong-target, clan oust-penalty SM 231, already-clanned SM 10, rejoin-penalty SM 760, academy-eligibility SMs, per-type member caps — the full `getMaxNrOfMembers` table ported), accept burst (`JoinPledge`/`PledgeShowMemberListAdd`/`PledgeShowInfoUpdate`/`ExPledgeCount` 0xFE:0x13D + clan skills + Clan Advent on join), **leave** (`RequestWithdrawalPledge` 0x28: leader/combat gates, 1-day rejoin penalty), **oust** (`RequestOustPledgeMember` 0x29: CL_DISMISS, target-combat gate, dual penalty — oustee rejoin + clan-side `char_penalty_expiry_time`), **dissolve/recover** (village-master bypasses: guard chain incl. castle/siege-registration/siege-zone, 7-day `dissolving_expiry_time` + leader death-XP penalty, `ScheduledTask::ClanDissolve` re-armed at boot, recover cancels), shared `removeClanMember` teardown (title/skills/advent/window/UserInfo + `RemoveClanMember` column reset incl. offline members). New persistence: `characters.clan_join_expiry_time`, `clan_data.char_penalty_expiry_time`/`dissolving_expiry_time`. **Slice 2: level-up + rep-gated skill learning landed** — village-master `increase_clan_level` (`Clan.levelUpClan` Classic cost ladder: 1k SP+150k adena → 15k SP+300k adena → 100k SP+100 Blood Mark → 1M SP+5k → 5M SP+10k, dissolution gate SM 551, not-met SM 1790, consumption SMs 672/301/302/538, level-up FX `MagicSkillUse` 5103) via the existing `set_clan_level` (now + SM 1771 to the leader crossing level 5); `learn_clan_skills` → `showPledgeSkillList` (`ExAcquirableSkillListByClass` 0xFE:0xFA type PLEDGE, SM 607 / NoMoreSkills.htm / NotClanLeader.htm branches); `RequestAcquireSkill` PLEDGE branch + `RequestAcquireSkillInfo` 0x73 (rep cost via `AcquireSkillInfo` 0x91) — prev-level + clan-level hack checks, rep spend through `add_clan_reputation` (SM 1787, insufficient SM 1852), grant through `add_clan_skill`; `levelUpSp` now parsed from `pledgeSkillTree.xml` (`available_pledge_skills`/`pledge_skill` lookups). **Slice 3: ranks & power grades landed** — `RequestPledgePower` 0xCC (`ManagePledgePower` 0x2A answer; leader action-2 edit → `Clan.setRankPrivs`: store + `clan_privs` upsert + live mask/UserInfo refresh on online holders + `broadcastClanStatus`, rank 9 clamped to the academy-bestowable subset), `RequestPledgePowerGradeList`/`MemberPowerInfo`/`SetMemberPowerGrade`/`MemberInfo` ex 0x13/0x14/0x15/0x16 (`PledgePowerGradeList` 0x3D / `PledgeReceivePowerInfo` 0x3E / `PledgeReceiveMemberInfo` 0x3F; re-rank: CL_MANAGE_RANKS gate, leader untouchable, SM 1761 + roster broadcast + `characters.power_grade` persist), `RequestPledgeReorganizeMember` ex 0x2C parsed as Java's own same-type early-out (TODO(G18.6)); **enter-world now derives privileges from the rank table** (Java `Player.restore`: leader → all-bits + grade 1, member → `getRankPrivs(powerGrade)` with grade defaulting to 5 — the stored `clan_privs` column never wins), join sets grade 5 + `getRankPrivs(5)`; **delegated leader transfer** (`change_clan_leader`/`cancel_clan_leader_change` bypasses: stamp + persist `clan_data.new_leader_id`, 9000-07-* confirmation htmls; application at daily reset = TODO(G33) `DailyTaskManager.onClanLeaderChange`); members carry `power_grade`/`title` (loaded with the roster + `clan_privs` rows). **Slice 4: clan wars landed** — `ClanWar` model (`clan_wars` table, boot-restored + re-armed): `RequestStartPledgeWar` 0x03 (full guard chain: level 3/15 members, CL_PLEDGE_WAR, 30-war cap, dissolving target, 21-day post-defeat gate; counter-declaration → MUTUAL via `mutualClanWarAccepted`), `RequestStopPledgeWar` 0x05 (500-rep cease-fire, member-in-combat gate), `RequestSurrenderPledgeWar` 0x07 (`ClanWar.cancel`: winner set, `SurrenderPledgeWar` 0x67, torn down seconds later — Java's live path, the 5/21-day retention constants are dead code there), `RequestPledgeWarList` ex 0x17 → `PledgeReceiveWarList` 0xFE:0x40; **7-day BLOOD_DECLARATION timeout** (`ScheduledTask::ClanWarTimeout` → TIE, state-checked so MUTUAL no-ops it); **kill pipeline** (`ClanWar.onKill` from the death path outside PVP/siege zones: 5 attacked-side kills force MUTUAL with SM 3815 progress; mutual kills move `ReputationScorePerKill`=1 between clans with SMs 3811/3812, victim clan ≤0 rep exempt); **war PvP rules**: `checkIfPvP` + `isAutoAttackable` mutual-war legs, death-XP penalty ÷4 vs a war enemy (`apply_death_exp_penalty_ex`), war swords on `RelationChanged` (0x4000 declared / +0x8000 mutual per Java `getRelation`), dissolve now really rejects at-war clans (SM 264). **Slice 5: alliances landed — the ROADMAP gate (form clan/invite/level/learn skill/declare war/form ally) is now fully met.** `create_ally`/`dissolve_ally` village-master bypasses (`Clan.createAlly` guard chain SMs 504/502/549/505/550/506/507/508; dissolution broadcasts SM 523 ally-wide, clears every member clan, stamps penalty type 4), `RequestJoinAlly` 0x8C → `checkAllyJoinCondition` (ally-leader-only, penalty types 1–3 gates, target-leader checks, both-in-siege-zone SM 723, at-war SM 469, `AltMaxNumOfClansInAlly`=3 cap) → `AskJoinAlly` 0xBB via the `PendingRequest` slot → `RequestAnswerJoinAlly` 0x8D (re-checked; target clan folded in, Java's wrong friend-added SM 525 kept), `AllyLeave` 0x8E / `AllyDismiss` 0x8F (penalty types 1 / 2+3), `RequestDismissAlly` 0x90, `RequestAllyInfo` 0x2E (`AllianceInfo` 0xB5 + the SM 491–500 cascade); ally id/name persisted (`clan_data` ally columns) and **now shown everywhere** — `Player.ally_id` denormalized (synced at enter-world/join/leave/destroy) into UserInfo/CharInfo, ally name/id in `PledgeInfo`/`PledgeShowInfoUpdate`/`PledgeShowMemberListAll`; war-declare same-ally gate SM 1569 and dissolve-clan ally gate SM 554 now real (ally crests TODO(G18.7)). **Slice 6: sub-pledges & academy landed.** `create_academy`/`create_royal`/`create_knight` village-master bypasses share one `createSubPledge` port (level gates 5/6/7, name validation/clash-across-all-clans, leader-eligibility + "clan leader can't captain a sub-unit" reject, `getAvailablePledgeTypes` family-slot resolution — 1 academy / 2 royal / 4 knight — reputation cost `CreateRoyalGuardCost`=5000/`CreateKnightUnitCost`=10000, `PledgeReceiveSubPledgeCreated` 0xFE:0x41); `rename_pledge`/`assign_subpl_leader` bypasses; **academy/royal/knight invites now accept** (`RequestJoinPledge`/`AnswerJoinPledge` no longer refuse pledge type ≠ 0 — validated against the clan's founded sub-pledges, academy grants power grade 9 vs the usual 5); `RequestPledgeReorganizeMember` ex 0x2C does a real two-member pledge-type swap (CL_MANAGE_RANKS-gated); a departing sub-unit captain's slot goes vacant (`removeClanMember`'s "leadssubpledge" branch) — persisted via new `clan_subpledges` rows + `characters.subpledge`. **`Clan::pledge_class_of` fully widened** to `ClanMember.calculatePledgeClass`'s per-level academy/royal/knight-member/captain tiers (levels 6–11), verified against a hand-transcribed reference table in a dedicated unit test; `PledgeShowMemberListAll` now correctly filters to main-pledge members only (Java's per-tab window; sub-unit tabs themselves are TODO(G18.6c), cosmetic-only). Apprentice/sponsor academy-graduation rewards deferred (TODO(G18.6b) at the join site — no consumer yet, needs G22 class-change wiring). **Slice 7: crests landed; notices audited and found unreachable on this dist.** `CrestTable` port (`World.crests` + `next_crest_id`, never-reuse-the-last-id semantics) backs three crest kinds: small pledge crest (`RequestSetPledgeCrest`/`RequestPledgeCrest` 0x09/0x67, ≤256 bytes, level-3 + CL_REGISTER_CREST + dissolving gates), large pledge crest (`RequestExSetPledgeCrestLarge`/`RequestExPledgeCrestLarge` ex 0x11/0x10, ≤2176 bytes, chunked `ExPledgeEmblem` 0xFE:0x1B answer), and ally crest (`RequestSetAllyCrest`/`RequestAllyCrest` 0x91/0x92, ≤192 bytes, alliance-leader-only, pushed to every member clan). Crest ids now render for real in `PledgeShowInfoUpdate`/`PledgeShowMemberListAll`/`GmViewPledgeInfo` (read straight off `Clan`) and in `UserInfo`/`CharInfo` (denormalized `Player.clan_crest_id`/`ally_crest_id`, synced at enter-world, clan join, crest set/delete, and every ally membership change — closes the last four TODO(G18.7) markers from slice 5); a test caught a real bug where the ally-crest setter broadcast stale UserInfo without updating members' denormalized field first, now fixed. **Clan notices were audited, not ported**: `Clan._notice`/`isNoticeEnabled`/the `EnterWorld` login-popup display path exist in Java, but grepping the full gameserver tree found no caller of `setNotice`/`setNoticeEnabled` anywhere — this Interlude Classic build ships the read/restore/display plumbing with **no in-game way to ever set a notice**, so there is nothing reachable to port faithfully; documented here rather than silently dropped. **Slice 8: recruitment registry landed — G18 is now COMPLETE.** `ClanEntryManager` port: `World.recruit_waiting`/`recruit_clans`/`recruit_applicants` + 5-minute re-registration locks (`recruit_player_lock`/`recruit_clan_lock`, tick-based), persisted via `pledge_recruit`/`pledge_waiting_list`/`pledge_applicant` (boot-loaded with Java's orphan-clan cleanup). The board (`RequestPledgeRecruitBoardAccess`/`Detail`/`Search` ex 0xD5/0xD6/0xD4 — register/update/remove a clan's listing, real unsorted/sorted-by-name/level/karma/by-name search with 12-per-page paging replacing the slice-1 empty stub), the applicant queue (`RequestPledgeWaitingApply`/`Applied`/`List`/`User`/`UserAccept` ex 0xD7-0xDB — apply → leader alarm ping → view queue/one applicant → accept, reusing `add_clan_member`, or reject), the global waiting list (`RequestPledgeDraftListSearch`/`Apply` ex 0xDC/0xDD — clanless players register themselves, leaders search by level/name/sort), and open-joining instant self-join (`RequestPledgeSignInForOpenJoiningMethod` ex 0x111, gated on the same char-penalty/join-expiry/member-cap checks as a normal invite). `RequestPledgeRecruitApplyInfo` now answers real ORDERED/WAITING/DEFAULT status. **Pledge bonus (`ClanRewardData`, members-online/hunting clan-wide skill rewards) deferred with TODO(G33)** — it needs a daily-reset scheduler that doesn't exist yet (same gap noted for G16 vitality and the delegated-leader-transfer application). |
| Game  | G19 Skills & effects breadth                                | ✅ **affect scopes + toggles landed** (plan: [PLAN_G19_EFFECTS.md](PLAN_G19_EFFECTS.md)): `AffectScope` SINGLE/RANGE/POINT_BLANK/PARTY/PLEDGE + `AffectObject` ALL/NOT_FRIEND/FRIEND/CLAN in `skills/affect.rs` (affectLimit cap with Java's `min + Rnd.get(max)` quirk, dead-skip, caster-skip, peace-zone leg, LOS from the target), the cast pipeline fanned out over the affected list (`apply_cast_consequences` per target — effects + PvP flag + hate), and **toggles** (recast = off, `toggleGroupId` exclusion, instant cast per `SkillCaster`'s short circuit, new `targetType NONE`). **Abnormal-state flags + crowd control landed** (plan: [PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md)): Java's `EffectFlag` mask ported as per-`ActiveBuff` flags folded on read (`game_loop/abnormal.rs` — no cached mask to invalidate), `BlockActions` (540 uses — stun/sleep/paralyze) and `Root` (79) effects, and the gates that read them (no attack/cast/move while stunned, no move while rooted, NPC AI silent while stunned, rooted mobs stay put), plus the mid-action interrupt (abort cast *then* freeze movement — the other order lets `stop_casting` resume the walk). Before this a stun landed, showed its icon and changed nothing. **Abnormal resistance/blocking + probabilistic dispel landed** (plan: [PLAN_G19_ABNORMAL_RESIST.md](PLAN_G19_ABNORMAL_RESIST.md)): `ResistAbnormalByCategory`→`Stat::ResistAbnormalDebuff` folded into `calc_effect_land_rate` as Java's `buffDebuffMod` (multiply then clamp, so Guts halves incoming debuff chance), `ResistDispelByCategory`→`ResistDispelBuff` (pumped but consumer-less until `Cancel` lands — Java reads it only in `calcCancelSuccess`), `BlockAbnormalSlot` (Prophecy mutual exclusion, stamp-and-fold like the CC flags) and `DispelBySlotProbability` (the Bane family's per-buff rate roll). **Ranking note:** unported effects must be ranked by *learnable-skill* usage, not raw instance count — `StatUp` looks like the biggest gap at 887 instances but is only 9 learnable skills (the rest are talisman/Freya/agathion content). **Periodic HP/MP + healing/CP breadth landed** (plan: [PLAN_G19_PERIODIC_EFFECTS.md](PLAN_G19_PERIODIC_EFFECTS.md)): `HealOverTime` (negative power = the upkeep toggles' HP cost, floored at 1) and `ManaDamOverTime` joined the existing DoT tick chain, with an out-of-MP tick switching a **toggle** off + SM 140 (Java's `false` return, honoured only for toggles); `HealEffect` (HEAL_EFFECT mul / _ADD diff, read off the *recipient*) folded into the Heal path; `Cp` instant restore/drain with DIFF/PER. Closes a loop: the toggles ported in the first G19 slice now actually cost HP/MP. The empty-effects guard's third exemption was generalised into `has_periodic` — any effect with no stat modifier must join *periodic*, *icon-only* or *state flag* or it is silently dropped. **CC breadth landed** (plan: [PLAN_G19_CC_BREADTH.md](PLAN_G19_CC_BREADTH.md)): `Mute`/`PhysicalMute` (magic vs non-magic cast gate in `checkDoCastConditions`, static skills exempt, mutually exclusive), `DebuffBlock` (incoming debuffs bail outright ahead of the resist roll; buffs unaffected), `BlockControl` (item-use gate — Java's wider summon/mob-control meaning is G29) and `TargetCancel` (chance-rolled instant: drops the target via `set_target(None)` so `TargetUnselected` broadcasts, and aborts attack+cast). Landing a mute also aborts the victim's in-flight cast, with **raid bosses immune** to that interrupt. `Fear` is the CC hold-out — it needs forced flee movement, so it belongs with G21's AI breadth. **Abnormal visual effects landed** (plan: [PLAN_G19_ABNORMAL_VISUALS.md](PLAN_G19_ABNORMAL_VISUALS.md)): the cosmetic half of all the CC above — `AbnormalVisualEffect` id map + `<abnormalVisualEffect>` parsed, stamped on `ActiveBuff` and folded on read; `CharInfo` (which hard-coded a count of **0**, so nobody ever saw an effect on anyone) and `ExUserInfoAbnormalVisualEffect` now carry the real set; pushed **only when the set changes**, as Java does. Plus `//ave_abnormal` toggling a GM-pinned visual via a new `AdminVisuals` component folded alongside the buff-derived ones. Remaining AdminEffects AVE handlers (`//setteam`, `//settargetable`, `//set_displayeffect`, `//playmovie`) are unblocked but need their own per-creature state + packet fields. Before this, only SINGLE resolved — every one of the datapack's 1900+ area skills hit exactly one target. **Transformation landed** (plan: [PLAN_G19_TRANSFORMATION.md](PLAN_G19_TRANSFORMATION.md)): the "Transform <Monster>" scroll family (32 learnable skills — Grail Apostle, Unicorn, Doom Wraith, Zaken, …), wired into the existing G13.B `//transform` admin runtime (`Player.transform_id`/`TransformData`) via the skill-cast path — `admin::transforms` split into state-only and state+broadcast halves so the buff-landing path can fold the transform-specific extras onto the `UserInfo` it already sends rather than double-broadcasting; reverts on `BuffExpire`, which (since death already routes stripped buffs through the same removal fn) covers death for free. Cast-time gate ports `ConditionPlayerCanTransform`'s already-transformed/in-water/cursed-weapon-equipped legs (`DefenceAttribute`, the next effect on the raw-count list at 33 learnable skills, is Kamael-era elemental attributes and out of scope). **MpConsumePerLevel landed** (plan: [PLAN_G19_MP_CONSUME_PER_LEVEL.md](PLAN_G19_MP_CONSUME_PER_LEVEL.md)): the MP-upkeep half of the core fighter toggles (Accuracy 256, Guard Stance 288, Vicious Stance 312, War Frenzy 424, Super Haste 7029, …) — each already lands a real `StatModifier`, but this *other* effect on the same skill was silently dropped, so every one of these toggles was a free, uncosted buff. Every instance in the datapack is a toggle with no `abnormalTime`, collapsing Java's formula to `ManaDamOverTime`'s `power * getTicksMultiplier()`, so it shares that effect's tick-chain arm rather than duplicating it (periodic drain, self-deactivate + SM 140 on insufficient MP); the level-scaled `abnormalTime > 0` branch is unexercised by this datapack and left a TODO. Also fixed `admin_superhaste_applies_and_persists`, whose zero-MP test setup broke once Super Haste's own drain (Java's `AdminSuperHaste` casts through the real `applyEffects` path) started applying. **ShieldDefence/ShieldDefenceRate landed** (plan: [PLAN_G19_SHIELD_DEFENCE.md](PLAN_G19_SHIELD_DEFENCE.md)): Shield Mastery (153), a passive every shield-using class can learn, pumps both stats — `ShieldDefenceRate` was already parsed (`EFFECT_REGISTRY`) but never actually read (`game_loop::combat::shield_stats` used the equipped shield's raw `rShld` directly, bypassing `StatModifiers`); `ShieldDefence` wasn't parsed at all. Both now fold through `model::finalize` (bumped `pub(crate)`) over the shield's own `sDef`/`rShld`, gated behind the existing no-shield-equipped early return so a flat buff still contributes nothing without a shield, matching `Formulas.calcShldUse`'s short-circuit. `EnergyAttack` (9 learnable) set aside — needs the unmodeled Dwarf Force/Charges resource first. **HealPercent landed** (plan: [PLAN_G19_HEAL_PERCENT.md](PLAN_G19_HEAL_PERCENT.md)): all 5 learnable instances are core priest kit — Miracle (1426), Benediction (1271), Restore Life (1258), Revival (181), Touch of Life (341) — every one of which parsed to an empty effect list and healed nothing. New match arm mirrors `Heal`'s NPC-silent/player-with-SM split and overheal clamp, computing the amount as a max-HP percentage rather than the magic-formula power, and skipping `Heal`'s recipient `HealEffect`/`HealEffectAdd` scaling (Java's real asymmetry). Surfaced `TargetType::EnemyNot` as unmodeled (falls through to `Other`, silently no-op'd by `use_magic_on`) while testing Restore Life. **`TargetType::EnemyNot` landed** (plan: [PLAN_G19_ENEMY_NOT_TARGET.md](PLAN_G19_ENEMY_NOT_TARGET.md)): "any friendly selected target" — the precise inverse of `Enemy`/`EnemyOnly`'s `is_auto_attackable` gate, no force-use override, self always allowed, exempt from the general dead-target rejection ("works on dead targets or doors as well"). Small (34 instances) but it was quietly capping the two `HealPercent` skills that heal someone other than the caster (Restore Life, Touch of Life). `AttackTrait` (7 learnable) set aside — needs a `TraitType` attacker-bonus system unmodeled on this port. **Force/charges landed** (plan: [PLAN_G19_FORCE_CHARGES.md](PLAN_G19_FORCE_CHARGES.md)): unblocks `EnergyAttack`, set aside twice before. New `Player.charges` resource (transient, never persisted) backs Sonic Focus → Sonic Blaster/Buster and the Orc/Dark Elf Force Burst/Storm/Blaster family — 9 `EnergyAttack` + 6 `FocusMomentum` learnable skills all parsed to empty effect lists before this. `FocusMomentum` gains charges capped at `max_charges.min(8)` (Java's `MAX_MOMENTUM` stat is never set anywhere in this datapack, so `8` is the real cap, not a simplification); `EnergyAttack` shares `PhysicalAttack`'s damage core times a new `1 + charge×0.1` boost, reading `chargeConsume` off a skill-level tag rather than the effect's own params. `EtcStatusUpdate` (0xF9) now carries the real charge count. Deferred: Java's 10-minute charge-decay task, `GetMomentum` (dead code — nothing sets `MAX_MOMENTUM`), and wiring the charge bonus into `PhysicalSoulAttack`/`MagicalSoulAttack`/`SoulBlow`'s existing `×1` stand-ins. **Lethal landed** (plan: [PLAN_G19_LETHAL.md](PLAN_G19_LETHAL.md)): `AttackTrait` set aside a third time — needs the cross-cutting `TraitType` system, not a slice. `Lethal` (9 learnable) was already flagged as a TODO on `SkillEffect::Blow`'s own doc comment — every learnable instance pairs it with an already-ported damage effect (Backstab 30, Lethal Blow 344, Deadly Blow 263, Critical Blow 409, Lethal Shot 343, Turn/Banish Undead/Seraph), so those skills' damage landed but the bonus instant-kill/half-kill chance never rolled. Level gate + raid-boss immunity (reusing `Mute`'s own `is_raid()` check) ported; full/half-lethal rolls set a player's CP (and HP, on a full lethal) to 1 or halve a monster's HP, with `chanceMultiplier` at 1.0 (no trait/attribute math anywhere on this port). `INSTANT_KILL_RESIST` isn't rolled — like `MAX_MOMENTUM`, nothing in this datapack ever sets it. **AttackTrait landed** (plan: [PLAN_G19_ATTACK_TRAIT.md](PLAN_G19_ATTACK_TRAIT.md)): the last item on the learnable-skill ranking, investigated properly instead of deferred a fourth time. All 7 learnable instances (Detect Insect/Beast/Animal/Dragon/Plant Weakness, Eye of Hunter/Slayer) use only the `*_WEAKNESS` category of `TraitType` — and the consuming formula turns out inert on the real Java server too (`calcWeaknessBonus` needs a matching NPC-side `DefenceTrait`, and nothing in this datapack ever sets one — grepped the whole Java tree, one call site, its own definition). Lands as an icon-only buff, closing a real regression (the effect wasn't recognized at all, so it didn't even land) without inventing damage-formula wiring for a bonus that's provably inert either way. Collateral: `NpcTemplate.race`/`Race` extended from 6 playable races to Java's full 26-member shared enum (players + creature categories) — costs nothing today, ready for when NPC-side trait data lands. **DamageBlock landed** (plan: [PLAN_G19_DAMAGE_BLOCK.md](PLAN_G19_DAMAGE_BLOCK.md)): the highest raw instance count left (5 learnable, 84 skills, 162 instances — a skill carries two `<effect>` elements, one `BLOCK_HP` one `BLOCK_MP`), already flagged by two existing TODOs on `HealPercent` and `Lethal`. The five learnable instances (Celestial Shield 1418, Flames of Invincibility 1427, Dance of Medusa 367, Sonic/Force Barrier 442/443) are short full-invulnerability shields. `HP_BLOCK` has a real single choke-point consumer in Java (`CreatureStatus.reduceHp`), matched by threading a new `is_dot: bool` parameter through `game_loop::combat::apply_physical_damage` — already the one function every damage path on this port funnels through — with an early return, exempting only DoT ticks (damage zones are *not* exempt, matching Java's `DamageZone`). `MP_BLOCK`/`isMpBlocked()` is the same "genuinely dead code in Java too" pattern as `MAX_MOMENTUM`/`INSTANT_KILL_RESIST`: zero callers anywhere in the Java tree, folded for completeness but wired to nothing. Both existing TODOs closed. **EnlargeSlot landed** (plan: [PLAN_G19_ENLARGE_SLOT.md](PLAN_G19_ENLARGE_SLOT.md)): a re-run of the ranking sweep with `EFFECT_REGISTRY`'s generic stat-modifier table correctly excluded (it had been quietly absorbing dozens of effect names and inflating earlier raw counts) surfaced this on top — Expand Inventory/Warehouse/Trade/Common Craft/Dwarven Craft (5 learnable, 162 raw instances). A `type`-selected `Stat` passive (6 new variants: `InventoryNormal`, `StoragePrivate`, `TradeSell`, `TradeBuy`, `RecipeDwarven`, `RecipeCommon`), folded through `model::finalize` into `UserInfo`'s INVENTORY_LIMIT block, `ExStorageMaxCount` (previously all six capacity fields were Java's static placeholder defaults, one literally commented "`Stat.INVENTORY_NORMAL` not wired"), and `crafting::learn_recipe`'s recipe-book cap, the one consumer with real enforcement behind it — warehouse deposit and private-store listing still aren't capacity-checked anywhere on this port (`TODO(G29+)`), so only the *number reported* changed for those. Surfaced and fixed a wider pre-existing gap along the way: a newly learned passive skill only took effect at the next login; `RequestAcquireSkill` now also calls `recompute_conditioned_passives` (already generic under its armor-swap framing), so any stat-modifier passive applies the moment it's learned. **Hate-manipulation effects landed** (plan: [PLAN_G19_HATE_EFFECTS.md](PLAN_G19_HATE_EFFECTS.md)): a tied cluster of six related effect names sharing one already-ported primitive (`AggroList`) — rather than take the top name alone and defer the rest a fifth time (the `AttackTrait` pattern), bundled the four cheap ones: `GetAgro` (Aggression, Aggression Aura, Judgment, Tribunal), `AddHate` (Charm, Lure), `DeleteHate` (Eva's Serenade, Peace, Repose), `DeleteHateOfMe` (Bluff, Forget, Trick) — 12 learnable-skill instances. `GetAgro` needed the most care: the ported AI derives its attack target fresh from `AggroList::most_hated` every think tick rather than caching a "current target," so "force intend-attack the caster" became "make the caster's hate dominant" (current max + 1) rather than a direct intention override. `DeleteHate`/`DeleteHateOfMe` both disengage via a newly `pub(crate)` `npc_ai::set_active`, shared with `think_attack`'s own timeout/leash disengage rather than duplicated. Deferred: `TargetMe` (paired with `GetAgro` on the same 2 skills) needs a locked-target UI concept nothing on this port has; `RandomizeHate` (Confusion, Switch) needs a general nearby-visible-creatures query `faction_call`'s NPC-only neighbour scan doesn't provide; `GetAgro`'s clan-mate pre-seed is left to `faction_call`'s own reactive recruit, at most one think-tick later. **DispelByCategory landed** (plan: [PLAN_G19_DISPEL_CATEGORY.md](PLAN_G19_DISPEL_CATEGORY.md)): the "Cancel" family (Cancellation, Cleanse, Purification Field, Touch of Death), another tied cluster at 4 learnable skills — picked over the cheaper `PhysicalAttackRange` (a same-shape repeat of the already-solved `ShieldDefenceRate` pattern, no new value) because it closes a real gap flagged two slices ago: `Stat::ResistDispelBuff` was pumped but "consumer-less until `Cancel` lands." Unlike `DispelBySlot`/`DispelBySlotProbability` (a fixed abnormal-type list), this steals *whatever* is up — `BUFF` slot walks dances then buffs in reverse cast order, each gated by a ported `calcCancelSuccess` (`clamp(rate + (casterMagicLvl - buffMagicLvl)*2 + (buffAbnormalTime/120)*ResistDispelBuff, 25, 75)`, skipped as automatic when `rate>=100`); `DEBUFF` slot uses a flatter `roll<=rate` (Java's exact operator, not this codebase's usual `<`). The dances-before-buffs split and most of `canBeStolen()`'s exclusions came free from the already-ported `BuffSlot` classification. Java's `ALL` slot is dead code too, and stays a no-op here. Deferred: `isIrreplacableBuff()`/hero/GM/static-skill exclusions (unmodeled fields, matching `DispelBySlotProbability`'s own precedent). **PhysicalAttackRange landed** (plan: [PLAN_G19_PHYSICAL_ATTACK_RANGE.md](PLAN_G19_PHYSICAL_ATTACK_RANGE.md)): Archery/Long Shot/Rapid Fire/Snipe, the cheapest of the tied-at-4 cluster `DispelByCategory` was picked from — a same-shape repeat of the already-solved `ShieldDefenceRate`/`AttackCancel` pattern, needing only an `EFFECT_REGISTRY` entry and wrapping `recalculate_stats`' bare `combat.atk_range` line in `finalize()` (the same gap `ShieldDefenceRate` itself had before an earlier slice). All four learnable instances are `<weaponType>BOW</weaponType>`-conditioned; the condition mask is already generic across every registry entry, so nothing extra was needed to gate correctly — proven by a test showing the bonus is inert while unarmed. **FatalBlowRate landed** (plan: [PLAN_G19_FATAL_BLOW_RATE.md](PLAN_G19_FATAL_BLOW_RATE.md)): Assassination/Critical Blow/Focus Death/Mortal Strike, another tied-at-4 pick — directly tied to the already-ported `Blow`/`Lethal`/`FatalBlow` mechanics, since `formulas::calc_blow_success`'s own doc comment flagged `Stat.BLOW_RATE`/`BLOW_RATE_DEFENCE` as hardcoded identity. Same `EFFECT_REGISTRY` wiring as `PhysicalAttackRange`; the formula gained one `blow_rate_mod` parameter multiplied into the existing rate expression, threaded from the caster's finalized `StatModifiers`. `Stat.BLOW_RATE_DEFENCE`/`FatalBlowRateDefence` is genuinely dead in Java too — a registered handler no shipped skill grants — matching the recurring `MAX_MOMENTUM`/`INSTANT_KILL_RESIST` pattern. **Fear landed** (plan: [PLAN_G19_FEAR.md](PLAN_G19_FEAR.md)): the CC hold-out the CC-breadth slice deferred to "G21's AI breadth" — **G21 is complete**, so the forced-flee movement it needed now exists. Top of the in-scope ranking at 8 learnable skills (Horror 65, Banish Undead 405, Banish Seraph 450, Fear 1092, Curse Fear 1169, Word of Fear 1272, Mass Curse Fear 1381, Turn Undead 1400); everything above it is out of scope (`DefenceAttribute` 31 — Kamael elemental attributes) or G29 (`Summon`/`SummonCubic`/`SummonNpc`, 24/12/9). Reading the Java shrank the port twice: **`EffectFlag.FEAR` has no reader** (no `isAfraid()`, nothing `isAffected(FEAR)` — a feared creature is *not* gated out of attacking, casting or walking) and **`EVT_AFRAID` has no handler**, both the recurring `MP_BLOCK`/`MAX_MOMENTUM` "dead in Java too" pattern, so the entire mechanic is `fearAction`'s repositioning: 500 units away from the caster on `onStart`, then along the victim's *own heading* every 5-tick beat (Java passes `null` for the effector on repeats, so they keep running the way the first shove threw them rather than being re-aimed at a caster who may be dead by then). Shares the existing DoT tick chain rather than growing a scheduler; `canStart` ports the raid and `Defender`/`FortCommander`/`SiegeFlag`/`SIEGE_WEAPON` carve-outs. The load-bearing piece is **`NpcIntention::MoveTo`**: `AttackableAI.onEvtThink`'s switch has **no `AI_INTENTION_MOVE_TO` case**, so a fleeing mob thinks about nothing until it arrives — without it the next think tick re-issues the chase and drags the mob straight back, making the flee invisible (`onEvtArrived`'s `MOVE_TO`→`ACTIVE` reset ported alongside, off a new `TickOutcome.arrived`). **This was a quiet gap, not a loud one:** every Fear skill also carries the already-ported `BlockControl`, so the buff always landed — icon, duration, `BLOCK_CONTROL` flag — and the debuff looked like it worked; it just never moved anyone. Deferred: `canStart`'s `isSummon()` leg (`TODO(G29)`). **StatByMoveType + the player regen stat pipeline landed** (plan: [PLAN_G19_STAT_BY_MOVE_TYPE.md](PLAN_G19_STAT_BY_MOVE_TYPE.md)): picked from a three-way tie at 4 learnable (`StatByMoveType`/`MagicalAttackMp`/`SilentMove`) because two of its four skills — Vital Force 148 and Clear Mind 1297 — carry *only* this effect and so parsed to an empty effect list and were **dropped whole**, passives that did precisely nothing. Behind it sat a much bigger gap the ranking is structurally blind to: the sweep counts *unported effect names*, and `HpRegen`/`MpRegen`/`CpRegen` are in `EFFECT_REGISTRY` — but **`regen_player` never read `StatModifiers` at all**, so all 21 learnable regen skills (Focus Mind 191, Mana Recovery 214, Regeneration 1044, Song of Life 265, Victories of Pa'agrio 1414, …) pumped a stat nobody consumed, the same "parsed but unconsumed" shape as `ShieldDefenceRate`/`PhysicalAttackRange`. Real scope: **25 learnable skills, not 4**. `regen_player` now ends in Java's `Stat.defaultValue` (`mul*base + add + getMoveTypeValue(stat, getMoveType())`) for all three of HP/MP/CP, and the hard-coded standing multiplier became the real `Creature.getMoveType`-driven block (sitting 1.5 / standing 1.1 / running 0.7 — and **walking falls through every branch for no multiplier at all**, so walking regen is *worse* than standing still; Java as written, now pinned by a test), retiring a stale `TODO(G7)`. `StatByMoveType` itself rides on a new `StatModifierEffect.move_type` qualifier, so the entire buff pipeline (landing, stacking, removal, passive folding) needed no changes; `apply_modifier` routes it to a separate `StatModifiers::by_move_type` map — Java's own `_moveTypeStats`, deliberately *not* folded into `add`, which would apply the bonus in every locomotion state instead of the one it names — read live against the current move type, so the value swings as the player stands/walks/runs with no stat recompute. Acrobatic Move 225's evasion (the one non-regen use) folds in at `combat::combatant()`'s per-attack snapshot rather than the cached `CombatStats`, matching Java's on-demand finalizer. Deferred: `MoveType::Sitting` (no source — sitting isn't modeled, `TODO(G29)`; parsed and stored so it starts applying for free once it lands), the zone/residence regen multipliers, and the tie's other two effects. **Critical-damage stats landed** (plan: [PLAN_G19_CRITICAL_DAMAGE.md](PLAN_G19_CRITICAL_DAMAGE.md)): found by running the *previous* slice's post-mortem check first — the name-based ranking is structurally blind to "parsed but unconsumed" stats, so this time every `Stat` variant was swept for consumers outside `stats.rs`/`skill_data.rs`. Exactly two came back with **zero readers**: `CriticalDamage` and `CriticalDamageAdd`. All three damage formulas hard-coded `if crit { 2.0 }`, so **18 learnable skills were completely inert** — including Death Whisper 1242, Focus Attack 317, Vicious Stance 312, Frenzy 176, Dance of Fire 274, Zealot 420, Dead Eye 414, Chant of Victory 1363. Pulling the thread gathered the family: `CriticalDamagePosition` (3, also on the ranking), `MagicCriticalDamage` (2), `DefenceCriticalDamage` (1) — **24 learnable skills**. `formulas::CritDamage { mul, add }` carries Java's `calcCritDamage`/`calcCritDamageAdd` results, with `Default` = the stat-free `2.0`/`0.0` so the refactor is provably behaviour-preserving for an unbuffed actor (pinned by a test, which is what the pre-existing damage tests rest on). `calc_auto_attack_damage` now follows Java's two-section expression `(((attack·cAtk·ss) + cAtkAdd)·critMod)·77 + (attack·(1−critMod)·ss·77)` — the bracketing is load-bearing: `cAtkAdd` lands *after* the soulshot multiply but *inside* the ×77/÷pDef, so a flat +32 is worth far more than face value. **`StatQualifier`**: last slice's `StatModifierEffect.move_type` field generalised to an enum rather than growing a second parallel `Option` that would rot — `MoveType` merges additively from 0.0 into `_moveTypeStats`, `Position` multiplicatively from 1.0 into `_positionTypeStats`, two maps because Java's merges and identities genuinely differ. The data corrected two wrong assumptions along the way, both now pinned: Focus Death 355 carries **two** position entries with opposite signs (front −30% → ×0.7, back +90% → ×1.9 — the asymmetry only survives because that map multiplies), and skill 193 "Critical Damage" is `mode=DIFF`, a flat +32 `cAtkAdd`, not a percentage. Deferred: `PHYSICAL_SKILL_CRITICAL_DAMAGE` (no learnable grantor on this dist → that branch stays 2.0, the `BLOW_RATE_DEFENCE`/`MP_BLOCK` precedent), `MAGIC_CRITICAL_DAMAGE_ADD` (computed but never applied in Java either), and `calcBlowDamage`'s own crit shape. **SilentMove + FakeDeath landed** (plan: [PLAN_G19_SILENT_MOVE_FAKE_DEATH.md](PLAN_G19_SILENT_MOVE_FAKE_DEATH.md)): the unconsumed-stat sweep came back **clean** this time (all 44 `Stat` variants now have real consumers), so back to the name ranking and its two-way tie at 4 learnable. `SilentMove` won because its four skills (Silent Move 221, Stealth 411, Dance of Shadows 366, Fake Death 60) all *land* but their **headline mechanic** did nothing — the aggro scan carried a literal `// invisibility/silent-move/GM states don't exist` comment, so stealth failed 100% of the time — and it pulled `FakeDeath` in with it: **Fake Death 60 carries only these two effects**, so with both unported it parsed to an empty effect list and was **dropped whole**. Java reads the two flags on *adjacent lines of the same method* (`AttackableAI.isAggressiveTowards`), so splitting them would have meant touching that function twice. New `npc_ai::notices_target` applies the gate at all three player-scan sites (monster, guard PK, siege guard), as a post-sweep `retain` because the sweep closure holds `objects` mutably. **Raid bosses see through stealth** (`!me.isRaid()`) but are **not** exempt from fake death, which goes through `isAlikeDead()` — an asymmetry that's easy to get wrong, now pinned. `FakeDeath` shares the existing DoT tick chain for its MP upkeep (and, being a toggle, inherits the out-of-MP self-deactivate); new `ChangeWaitType` packet (0x29) plus `Revive` on standing up; `break_fake_death_on_damage` hooks the single `apply_physical_damage` choke point (`FakeDeathDamageStand = True`), gated on `amount > 0` so a missed swing doesn't stand you up. Three Java behaviours were checked and found **inert on this dist** rather than assumed: `canSeeThroughSilentMove` (no callers anywhere in the Java tree), `PlayerFakeDeathUpProtection = 0` (the stand-up grace window), and `FakeDeathUntarget = False`. Testing note: the baseline test failed first run and revealed two stealth tests passing **vacuously** — `NpcAi.global_aggro` starts at −10 and creeps 1 per think tick, so a monster needs ~100 game ticks before its scan runs at all (guards are exempt, which is why the older guard tests get away with 20). Deferred: `ChameleonRest`/`Hide` (non-learnable, need sitting), the `RequestRestartPoint`/`RequestActionUse` gates, and `MagicalAttackMp`. **MagicalAttackMp landed** (plan: [PLAN_G19_MAGICAL_ATTACK_MP.md](PLAN_G19_MAGICAL_ATTACK_MP.md)): the MP-drain family — **Mana Burn 1398 and Mana Storm 1399 carry only this effect**, so both parsed to an empty effect list and were dropped whole (the nukes cast, animated and drained nothing); Aura Sink 1102 / Seal of Gloom 1210 pair it with a ported `ManaDamOverTime` so they landed but did none of the up-front damage. Its own formula, sharing nothing with the HP path: `(sqrt(mAtk) * power * (targetMaxMp / 97)) / mDef` — the target's **max MP is a direct multiplier** (the same nuke hurts a mage far more than a fighter), spiritshots scale `mAtk` **before** the square root (so the gain is `sqrt(bonus)`, not `bonus`), and a crit triples then **clamps to a per-skill `criticalLimit`** (1600 on the debuffs, 7000 on the nukes) with no HP-side equivalent; there is also no `damage = 1` floor on a full resist, only the halving. Plus its own landing gate `calcMagicAffected` — a *noisy* mAtk-vs-mDef comparison needing a real `Rnd.nextGaussian()`, ported as `World::roll_gaussian` (Box–Muller over two `roll_f64` draws so tests can still force it through `forced_rolls`). **Correction to the `DamageBlock` slice:** `MP_BLOCK` was documented there as having no callers anywhere in Java — that grep covered `java/` only, and every effect handler lives under `dist/game/data/scripts/handlers/effecthandlers/`, where **five** read `isMpBlocked()` (`MagicalAttackMp`, `Mp`, `ManaHeal`, `ManaHealByLevel`, `ManaHealPercent`). The flag is live; `abnormal::is_mp_blocked` now exists and gates this effect, with a `TODO(G19)` for the MP-restore family. *Lesson: grep both trees.* One wrong turn, caught by a failing test and fully backed out: `<magicType>` doesn't exist in this dist's schema — the field is `<isMagic>`, all four skills are magic, and `calcCrit`'s magic branch **discards the `magicCriticalRate` it is passed** in favour of the caster's stat, so the drain's crit is just the existing per-cast `mcrit` and the speculative `Skill.magic_critical_rate` field (which had rippled into 15 test files) was removed. **MP-restore family landed** (plan: [PLAN_G19_MANA_RESTORE.md](PLAN_G19_MANA_RESTORE.md)): the name ranking hit a **five-way tie at 3 learnable**, so rather than pick one arbitrarily this took the entry that anchors a *cluster* — four Java handlers sharing one gate, one clamp and one message pair, differing only in the amount: `ManaHealByLevel` (3 — Recharge 1013, Servitor Recharge 1126, Mass Recharge 1428), `Mp` (2 — Pain of Sagittarius 417, Body To Mind 1157), `ManaHeal` (**0** reachable — Mortal Strike 410's instance turned out to be enchant-only, see the effect-level-gating slice), `ManaHealPercent` (0 learnable, 46 item skills), plus `ManaCharge` (1 — Higher Mana Gain 285, the stat the others read). **6 learnable skills**, and it closes the `TODO(G19)` the `MagicalAttackMp` slice left on `isMpBlocked`. **All three `ManaHealByLevel` skills carry only that effect**, so the core mage-support skill in the game parsed to an empty effect list and restored nothing. `ManaCharge` was found by applying the previous slice's both-tree grep lesson — `Stat.MANA_CHARGE` looks unused from `java/` alone, but a handler under `dist/.../effecthandlers/` grants it and a learnable skill uses it; without it the recharge skills would read a stat with no source. `ManaHealByLevel`'s penalty ladder (unpenalised to a 5-level gap, then ×0.9 down to ×0.1, **0 from 15 up**) collapses to `1 - (diff - 5)/10` and replaces Java's nine `else if` branches, with every branch pinned by test. Checked rather than assumed: `MAX_RECOVERABLE_MP` has **no grantor on this dist** (the `LimitMp` handler exists but no skill uses it), so the overheal ceiling is plain `maxMp` — documented at the clamp instead of plumbed. Testing note: the end-to-end penalty test failed first run because the level-5 fixture's ~50 max MP let the **overheal clamp cap both halves of the comparison**, making them read equal — a clamp downstream of what you're measuring will hide it. Deferred: `FACEOFF` (unmodeled), `ADDITIONAL_POTION_MP` (needs item context threaded into effect application), and the rest of the tied cluster (`TriggerSkillByAttack`, `ReflectSkill`, `BlockMove`, `TwoHandedBluntBonus`, `Confuse`). **Confuse + RandomizeHate landed** (plan: [PLAN_G19_CONFUSE.md](PLAN_G19_CONFUSE.md)): the same five-way tie at 3 learnable, resolved by grouping unported effects by prefix — three clusters tie at 5 (`Trigger*` 362 skills, `TwoHanded*` 22, `Confuse`+`RandomizeHate` 7). This pair won because they share **one blocker, already documented**: the hate-effects slice deferred `RandomizeHate` for want of "a general nearby-visible-creatures query `faction_call`'s NPC-only neighbour scan doesn't provide". New `helpers::visible_creatures` (every living player *or* NPC in an adjacent region cell — Java's `forEachVisibleObject` has no LOS or radius term, so neither does this) unblocks both. **Four of the five skills carry only the unported effect** — Madness 1105, Curse Discord 1163, Seal of Mirage 1213, Confusion 2 — so all four were dropped whole; Switch 12 landed but never switched anyone's hate. The two effects look interchangeable and are not: `Confuse` **adds** a target, `RandomizeHate` **moves** the hate and excludes same-faction mobs ("aggro cannot be transfered to a mob of the same faction") — pinned by a test pair. `calcProbability` reduces to `roll(100) < magicLevel + chance - targetLevel`, unclamped, so a high-level target pushes the threshold to zero and simply shrugs it off. `retarget_onto` reuses the `GetAgro` precedent (hate-dominance instead of a cached AI target). **A datapack trap worth recording:** three of these skills read `<effect name="Confuse" abnormalTime="20">` — but that is an *attribute*, and Java's `parseNamedParamInfo` reads only `name`/`level`/`from|toLevel`/`sub*Level` off an effect element, so it is silently ignored (7 instances datapack-wide, on `Fear` and `Confuse`, meaningless in both). With no real `<abnormalTime>` child there is no buff for an instant effect's flag to live in, so `effect_flag::CONFUSED` is unreachable and both its Java readers are dead — folded inert per the `FEAR`/`MP_BLOCK` precedent, with a test pinning `abnormal_time == 0`. Real chances are 20/20/60 and 80/80 — **none** defaults to 100. **Noted, not fixed:** the Rust skill parser ignores `fromLevel`/`toLevel` attributes on `<effect>` elements (775 instances each), which Java uses to gate an effect to a skill-level range — a real parity gap deserving its own slice. **Per-effect level gating landed** (plan: [PLAN_G19_EFFECT_LEVEL_GATING.md](PLAN_G19_EFFECT_LEVEL_GATING.md)) — **not** from the effect ranking: the `Confuse` slice noticed the parser read only the `name` attribute off an `<effect>` element and ignored `fromLevel`/`toLevel`/`fromSubLevel`/`toSubLevel`, **775 instances each**. Java uses them to attach an effect only to the skill levels its range covers, so every one was live at *every* level: **329 skills affected, 14 learnable**. That outranked the remaining tied-at-3 entries because it is *already-ported effects behaving wrongly* rather than a missing feature — Frenzy 176's two extra `PAtk` and two extra `CriticalRate` (`fromLevel="6"`) were boosting every level-1 Frenzy. Ported `forEachNamedParamInfoParam`'s gate verbatim (both bounds inclusive; `level`/`subLevel` supply the defaults for their pair) behind a new `ParsedEffect` struct, replacing the six-wide tuple the parser had been threading. **Sub-levels are the skill-enchant routes** (1001+/2001+), and this port has no enchanted skills, so sub-level reads 0 and every enchant-route effect is now correctly excluded — the gate is already written to take a real sub-level once enchanting lands. **The sweep caught a regression in the *previous* slice's tests, and it was the fix working:** that slice called Mortal Strike 410 "the one learnable `ManaHeal`", but its instance is `fromSubLevel="2001"` — enchant-only — so `ManaHeal` has **zero** reachable learnable skills here and that cluster's real reach was 6, not 7. `PLAN_G19_MANA_RESTORE.md` and this row are corrected. Checking for the same error elsewhere found it touches only already-ported effects (`PhysicalDefence` 63→59, `Speed` 55→52, `Heal` 18→16, `MagicalDefence` 36→34, `PhysicalAttackSpeed` 43→42) — **no slice-selection decision in this milestone would have changed.** **TriggerSkillByAttack landed** (plan: [PLAN_G19_TRIGGER_SKILL_BY_ATTACK.md](PLAN_G19_TRIGGER_SKILL_BY_ATTACK.md)): a four-way tie at 3 learnable, broken by the prefix-cluster heuristic — `Trigger*` and `TwoHanded*` both total 5 learnable, but `Trigger*` spans **362 skills** to `TwoHanded*`'s 22 and is a capability nothing on this port could express: landing a hit can cast another skill. Carriers are Sword/Blunt Weapon Mastery 205, Dagger Mastery 209 and Dance of Shadows 366 — each a passive/dance whose *on-hit half* did nothing. **Scope decision:** Java's handler takes 15 params, but all three reachable carriers set the same 8, so the port implements that subset and keeps Java's defaults rather than building machinery for content this dist doesn't have (`triggerSkills` ladders, `skillLevelScaleTo`, attacker-level bounds and `attackerType` are all unset here). Hooked at `combat::handle_attack_hit`, the normal-attack choke point that already carries `damage` and `crit`. **The subtle bit: `isCritical` is an *equality* test, not a minimum** — an `isCritical=false` trigger fires only on non-crits, and Dance of Shadows ships one of each, so reading it as "crits also count" would silently double it; both directions are pinned. Java's refresh guard is ported too (don't re-cast while the same buff is up at that level), without which a fast weapon would re-apply every swing. **Implementation note:** Java subscribes a listener when the carrying skill starts, but these carriers are passives whose effects this port folds into `StatModifiers` rather than keeping as a live list — so the attacker's skill book is scanned at hit time instead (a few `HashMap` lookups per swing; cache it like `NpcAiSkillIndex` if it ever profiles, it is not a behavioural difference). The triggered skills land real ported effects (5603 grants a 5-second `FatalBlowRate`), and the dist test doubles as a second check on the previous slice's `fromLevel="9"` gating. Deferred: the sibling triggers (`TriggerSkillByMagicType`/`ByDamage`/`BySkill`/…), which share this shape and should reuse its structure. **ReflectSkill + BlockMove landed** (plan: [PLAN_G19_REFLECT_BLOCKMOVE.md](PLAN_G19_REFLECT_BLOCKMOVE.md)): the previous slice's "5 learnable" for `TwoHanded*` was an artifact — skills 94/176 carry *both* TwoHanded effects, so counting per-effect double-counted them. By **distinct learnable skills** `TwoHanded*`, `Reflect*` and `Block*` all tie at 3, so this slice took two of them: both defensive-stance effects, both closing something already documented. **Physical Mirror 350 and Magical Mirror 351 carry nothing but `ReflectSkill`** (dropped whole), and **`BlockMove` is the `_isImmobilized` source** `game_loop::abnormal`'s module docs listed as having "no ported source" — now ORed into `is_movement_disabled` beside `ROOTED`, so these stances pin you without stunning you. Despite the name `ReflectSkill` is **not damage reflection**: its only Java consumer is `calcBuffDebuffReflection`, which on a successful roll **swaps the roles** (`applyEffects(target, caster, …)`) so an incoming *debuff* lands on its own caster — gated on the skill being a debuff *and* declaring an `activateRate` (the default -1 is never reflected). Ported at the per-target apply loop, with hate/PvP consequences left unconditional (the caster still cast a bad skill at that target). The data corrected three things: **`type` is `MAGIC`, not `MAGICAL`** — a **real bug** I introduced that would have routed every magic reflect into the physical stat, caught by a failing assertion and now pinned; both Mirrors carry **two** `ReflectSkill` effects each (30/10 and 10/30, differing by emphasis not kind); and their `<armorTYpe>SHIELD</armorTYpe>` gate is a **datapack typo** (10 occurrences vs 220 correct `<armorType>`) that Java's exact element matching ignores too, so it is inert on both sides. **Noted, not fixed:** the parser reads only the default `<effects>` block — Vengeance 368 puts its `BlockMove` in `<selfEffects>`, so the immobilise silently doesn't load. Datapack-wide the unread scopes are `selfEffects` (91 skills, 7 learnable), `endEffects` (58/1), `pvpEffects` (38/1), `pveEffects` (33/1), `channelingEffects` (24/4), `startEffects` (3/0) — ~14 learnable skills, comparable in reach to the `fromLevel` gap, and a strong candidate for its own slice; a test documents it and will fail when it lands. **Effect scopes landed** (plan: [PLAN_G19_EFFECT_SCOPES.md](PLAN_G19_EFFECT_SCOPES.md)): the gap the `BlockMove` slice found — the parser read **only** the default `<effects>` block, so every effect declared in another scope silently never loaded (~14 learnable skills across `selfEffects` 91/7, `endEffects` 58/1, `pvpEffects` 38/1, `pveEffects` 33/1, `channelingEffects` 24/4). More reach than any remaining effect entry (3), and silent breakage rather than a missing feature. **`SELF` + `PVE`/`PVP` ported**: SELF applies to the *caster* after the target loop (so a skill can buff its caster while debuffing its target), PVE/PVP append to the same target by Java's matchup selector. Every one of the seven `<selfEffects>` carriers holds an already-ported effect (`Speed`, `FocusMomentum`, `BlockMove`, `PhysicalEvasion`, `FatalBlowRate`), so this was pure plumbing with immediate payoff — six skills gained a real self-buff, including Vengeance 368's immobilise. Unsupported scopes parse as `Other` and are **dropped rather than merged**: merging would apply them at the wrong time, which is worse than not having them. Also lands **`impl Default for Skill`** — adding `Skill` fields had broken every exhaustive literal twice (`magic_critical_rate` churned 15 test files and was backed out partly for that), and this slice needed three more; `activate_rate: -1` and `reuse_delay_group: -1` are load-bearing "absent" sentinels that gates test for explicitly, pinned by a test. *Honest note:* the literal conversion took several passes, automated brace-matching mangled four files (reverted from git), and two were finished with explicit fields instead — so the style is mixed (20 files on `..Default::default()`, two explicit) as a deliberate stopping point. **A latent flake surfaced and was fixed:** `confuse_tests::a_confused_mob_turns_on_a_bystander` failed in the sweep, but not from this slice — `apply_skill_effects` charges an unconditional per-cast magic-crit `roll(1000)` *before* any effect runs, so the `Confuse` slice's `forced_rolls.extend([0, 1])` never pinned the candidate index and the assertion had been a coin flip for two slices. Now forces all three rolls with the ordering documented, verified over five runs. *Lesson: when forcing rolls, account for rolls charged by the surrounding machinery, not just the code under test.* Deferred: `startEffects`/`endEffects`/`channelingEffects` (6 learnable between them; they need cast-start, buff-end and channelling hooks the port lacks). **TwoHandedBluntBonus/SwordBonus landed** (plan: [PLAN_G19_TWO_HANDED_BONUS.md](PLAN_G19_TWO_HANDED_BONUS.md)): the top remaining in-scope entry at 3 distinct learnable skills (Rage 94, Frenzy 176, Two-handed Weapon Mastery 293 — Rage and Frenzy carry *both* variants, which is why the naive per-effect count read 5). Java's handler declares **eleven** stat/mode pairs but the reachable content sets only **pAtk and pAccuracy**, so those are read and the rest keep their zero default — the same scope-to-what-the-dist-reaches call `TriggerSkillByAttack` made. The gating is **two independent axes**: the existing `weapon_condition` mask (BLUNT/SWORD) *plus* a new `two_handed` flag for Java's `ConditionUsingSlotType(SLOT_LR_HAND)` — "a blunt" and "a two-handed weapon" are separate tests that both have to pass, so a one-handed mace fails. `two_handed_weapon_equipped` reads the weapon template's `bodypart` rather than inferring two-handedness from an empty off-hand, which would wrongly match an unarmed or shield-less one-hander. Also lands `impl Default for StatModifierEffect` (the same investment `Skill` got last slice; this conversion went cleanly in one pass off the single-line `qualifier:` anchor). Data correction: Rage declares `pAtkAmount = 0` at level 1 and only starts granting at level 2 — my first test asserted on level 1 and failed; a zero-amount modifier is dropped rather than stored, behaviourally identical to Java's `mergeAdd(stat, 0)`. **Resurrection landed** (plan: [PLAN_G19_RESURRECTION.md](PLAN_G19_RESURRECTION.md)): with the in-scope ranking down to a 2-learnable tail, this was picked on **player-visible value** rather than count — Resurrection 1016 / Mass Resurrection 1254 are only 2 learnable skills but a headline mechanic; without them nobody can be raised and every death is a walk back from town. The effect does **not** revive: it *proposes*, the corpse answers a `ConfirmDlg`, and only then do they come back. Two prerequisites came with it: **`TargetType::PcBody`** (a dead *player* corpse — the port had `NpcBody` for Sweeper but no player equivalent, so the skill couldn't even resolve a target) and **pre-death XP tracking** (Java keeps `_expBeforeDeath` and subtracts; the port now records the *difference* in `apply_death_exp_penalty_ex`, which already computes it — the only quantity a resurrection reads, and it can't drift from the penalty that produced it). `calculateSkillResurrectRestorePercent` scales the declared power by the reviver's WIT with a **quirk worth pinning: once the bonus has already added more than 20 it adds a further flat +20**, so high-WIT revivers jump rather than scale smoothly (clamped `[base, 90]`, short-circuited at 0 and 100). The skill's own HP/MP/CP percentages override the config respawn defaults, a zero meaning "leave what the config gave". `DlgAnswer` is now shared: the revive flow gets first refusal and reports whether the reply was its own, so the admin-confirm flow keeps working — pinned by a test. Also ported the re-check that the corpse is *still* dead when they accept (they may have used "to village" while the dialog sat on screen), without which the XP could be taken back twice. Deferred: pets (`TODO(G29)`), Charm of Courage, `BLOCK_RESURRECTION` (ported gate, no learnable grantor), and Mass Resurrection's party fan-out. **DefenceCriticalRate landed** (plan: [PLAN_G19_DEFENCE_CRITICAL_RATE.md](PLAN_G19_DEFENCE_CRITICAL_RATE.md)): the direct mirror of the crit-*damage* slice and the largest remaining in-scope entry (2 learnable, 50 skills) — Light Armor Mastery 233 (`-15% PER`) and Pa'agrio's Eye 1364 (`-30%`) make their holder harder to crit, but the port computed the autoattack crit chance as a bare `crit_stat / 10.0`, so the defender's side of the roll did not exist and both were inert. The load-bearing detail is that Java's two-arg `getValue(DEFENCE_CRITICAL_RATE, rate)` is `mul * rate + add`, so the **defender's multiplier scales the attacker's rate** — reading it the other way round would turn the stat into a flat chance instead of a reduction; pinned by a test. `calc_auto_attack_crit` gained `defence_mul`/`defence_add` at Java's identity defaults, reproducing the old expression exactly so existing combat tests keep meaning what they meant. Two corrections the tests forced, both mine rather than the code's: **`calc_critical_height_bonus(0, 0)` is 1.1, not 1.0** (Java's `+10` before the `/100`), so even the plainest case carries a multiplier and every expected value had to be recomputed; and **Light Armor Mastery is armor-conditioned** — I expected it to fold onto a naked character, but it correctly contributes nothing without light armour, so the test now asserts at the parsed-effect level with Pa'agrio's Eye as the unconditioned contrast. **ResistDDMagic landed** (plan: [PLAN_G19_RESIST_DD_MAGIC.md](PLAN_G19_RESIST_DD_MAGIC.md)): Anti Magic 146 / M. Def. 147 (2 learnable, 38 skills), the mage-defence passives that make incoming spells more likely to be resisted. **It also corrects a wrong claim the port already carried** — `calc_magic_success_rate`'s doc comment said Java's `resModifier` was "fixed at 1.0 on this dist" because the only two *items* touching `magicSuccRes` declare it additively, where `getMul` can't see it. True of the items, wrong as a conclusion: it never considered **skills**, and `ResistDDMagic` is an `AbstractStatPercentEffect` that merges *multiplicatively* — exactly what `getMul` reads. Same failure mode as the `MP_BLOCK` correction: a "provably inert" note only as good as the search behind it. The stat scales the **failure** term (`rate = 100 - (mAcc · lvl · target · res)`), so a value above 1 *lowers* the attacker's success — inverting it would turn a defensive passive into an offensive one, pinned by a test in both directions. `MagicSuccess.res_modifier` defaults to 1.0, reproducing the old expression exactly. Test-fixture correction: the step table bands on `magic_accuracy - magic_evasion` as `> -20 → 2, > -25 → 30, > -30 → 60, > -35 → 90`; my first fixture used a −31 deficit thinking it sat in the 60 band when it lands in 90, so the table is now written out beside it. **Geometric affect scopes landed** (plan: [PLAN_G19_GEOMETRIC_SCOPES.md](PLAN_G19_GEOMETRIC_SCOPES.md)): `FAN`/`FAN_PB` (163+16 skills, **5 learnable** — Sonic Buster, Force Burst, Wild Sweep, Wrath, Frost Wall), `SQUARE`/`SQUARE_PB` (35+17) and `RING_RANGE` (18) now sweep instead of falling back to single-target — chosen over the 2-learnable effect-registry tail on reach, since every dragon breath / tail sweep / quake the G21 mobs and G23 bosses cast is one of these. `<fanRange>` (`unk;startDegree;radius;angle`) parses **level-valued** (Frintezza Charge 5015 declares six tuples) into `Skill.fan_range`. The behavioural break worth knowing: **the geometry applies to the primary target too** — a FAN cast at someone behind the caster misses them, and RING_RANGE *never* hits its epicenter target (the sweep skips its origin and the 2D inner-radius test would drop it anyway — that is the donut hole), so `targets_affected` can now return a set without the named target, or empty; the consumer loop treats entries uniformly, so only the docstring's "always included" claim had to be corrected. Two Java quirks ported as written and pinned by tests: the fan's angle test has **no wrap-around normalization** (a caster whose heading maps to 350° misses a target at bearing 10° — |10−350| = 340 > half-angle — while the same 20° separation away from the seam hits), and `fanHalfAngle = fanAngle / 2` is **integer division** widened to double (a 35° fan tests against 17.0). SQUARE keeps Java's exact rotate-then-compare expression, `(int)` truncations, strict `>` and all — which makes Java's origin self-test provably dead code (the caster lands exactly on the excluded corner), reproduced by running the same filter rather than special-casing. LOS runs from the **caster** for FAN/SQUARE but from the **target** for RING_RANGE, matching each handler. Two parity corrections folded in: `corpse_skill` predated the resurrection slice's `PcBody` and exempted only `NpcBody` — Java's Range/Fan/Square all exempt `PC_BODY` too, so a dead player inside a sweep was wrongly dropped; and the dist's `Range.java` carries a **deliberate local fix** (82a54bbc "Fix minion buffs are given to players") the port had silently skipped — a monster's *good* RANGE skill never sweeps players in (dist is the spec; the branch is dated after the upstream import, found by reading the handler against the ported filter). Tests were verified by disabling each arm and confirming failures — the first pass showed three fan tests passing *vacuously* under a single-target fallback (their positive assertion was on the primary, which the fallback still returns); strengthened to assert on swept **bystanders**, after which all 10 fail when the code is stubbed. **GROUND casts + channeling landed** (plan: [PLAN_G19_GROUND_CHANNELING.md](PLAN_G19_GROUND_CHANNELING.md)): `targetType GROUND` (22 skills, **7 learnable**) split into the **channeled ground AoEs** (this slice — Volcano 1419 / Cyclone 1420 / Raging Waves 1421 / Gehenna 1423, `operateType CA1`) and the `SummonNpc` symbol family (Symbol of Noise 455 / Day of Doom 1422 / Anti-summoning Field 1424 — a totem-NPC subsystem, `TODO(G19)`). The flow: `RequestExMagicSkillUseGround` (**ex 0x41**) stores the aimed point (Java `_currentSkillWorldPosition` — **never cleared, only overwritten**), turns the caster (`ValidateLocation` to bystanders — "normally magicskilluse packet turns char client side but for these skills, it doesn't"), and enters the normal `useMagic`; the `Ground.java` target leg validates (shift/dontMove range vs castRange+collision, LOS to the point, and for bad skills Java's **five-point peace-zone effect-circle sample**) and returns **the caster as sentinel**; `PointBlank.java`'s GROUND fork sweeps around the **stored point** and — unlike the port's normal PB seeding — **never includes the caster** (Java's world sweep skips its origin), so a Volcano can't burn its own caster even under `affectObject ALL` (pinned). The **channeling runtime** is the `SkillChannelizer` as a self-rescheduling `ChannelingTick` (first fire `channelingStart`, period `channelingTickInterval`, staleness = the `Casting` seq, so every finish/abort path is `stopChanneling` for free): per tick, `mpPerChanneling` upkeep (**default = `mpConsume`**, not 0; starvation → SM 140 + abort), a full **re-sweep** (a mob that walks into the fire mid-channel burns — pinned), and the new `<channelingEffects>` scope (was parsed-and-dropped since the effect-scopes slice) applied through the normal pipeline — **without** per-tick `callSkill` consequences (Java's simple path adds no flat hate/PvP flag per tick; the damage itself wakes the mob). **Channeling cast time is static**: `_hitTime = max(hitTime − cancelTime, 0)`, `_cancelTime = 2866` — no casting-speed scaling, pinned by a doubled-mAtkSpd test. Also folded in, because Volcano needs it: the **skill reagent path** (`SkillCaster.checkUseConditions` gate SM 2156 + `startCasting` consume for bad-skill/`ActionType NONE` reagents — Volcano's Magic Symbol 8876; usable items keep paying in their own handler, so no scroll double-consume). The `channelingSkillId > 0` "channelized" branch (hero stances 426/427) is `TODO(G19)`. Test-fixture lesson re-learned: a realistic effect power one-shots the near-zero-m.def default template — probe the `dead` flag, not just despawn, before chasing ghosts. **SummonNpc symbols landed** (plan: [PLAN_G19_SYMBOLS.md](PLAN_G19_SYMBOLS.md)): the GROUND family's other half — Symbol of Noise 455 / Day of Doom 1422 / Anti-summoning Field 1424 (**3 learnable**) now drop a real seal. `SummonNpc` (EffectPoint branch; Decoy/default-spawn `TODO(G19)`) spawns the totem at the stored ground point; the `EffectPoint` runtime is a self-rescheduling `EffectPointCast` (first fire `cast_time` 0.1 s, period `skill_delay` 2 s) that `doCast`s the template's `union_skill` at itself through the shared NPC cast path, plus an `EffectPointDespawn` at `despawn_time` (15 s; effect `despawnDelay` as fallback). NPC templates now parse generic `<parameters>` (`ai_params` + `ai_skill_params` — the dist declares 5145 in BOTH the parameter holder and `<skillList>`, pinned so neither parse eats the other). **`OpExistNpc` is the first parsed skill condition**: ids/range/isAround, swept around the **caster** (not the aimed point) at `useMagic`; the dist quirk that Day of Doom's own totem 13028 is missing from the gate's id list (only Interlude-era 13018–13024) is data, ported as written. The behavioural keystone: **the seal acts as its owner** — `SummonerRef` + an `acting_player` hop in `is_friend`, so the SELF+POINT_BLANK+NOT_FRIEND auras curse bystanders but never the owner or their party/clan (Java `EffectPoint.getActingPlayer()`; same lesson as [[l2r-acting-player]]). Aura payload audit: 5145's percent debuffs + `MagicMpCost` land, 5124/5134's `DispelBySlotProbability` lands; `BuffBlock`/`Unsummon`/`DefenceAttribute` drop at parse (registry tail). Verify-by-disabling caught the owner-exemption test passing **vacuously** (the owner stood outside the aura radius — the assertion held with the friendship hop stubbed out); the owner now stands inside the blast. Deferred: per-pulse PvP flagging of the owner, `setTitle(owner)` cosmetics (per-instance NPC titles need NpcInfo plumbing). **Elemental attributes landed** (plan: [PLAN_G19_ATTRIBUTES.md](PLAN_G19_ATTRIBUTES.md)): the "2-learnable tail" claim was **stale** — a fresh census put `DefenceAttribute` at **33 learnable skills** (the whole Resist Fire/Water/Wind/Earth + Divine/Dark Protection + elemental Surrender family) and `AttackAttribute` at **7** (Holy Weapon 1043, Holy Blade 196, Dance of Light 277, Dark Form 423, the Seeds 1285–1287); it had been mentally filed under the ROADMAP's "elemental attributes are Kamael-era, out of scope" note, which actually covers item attribute *enchanting* — `Formulas.calcAttributeBonus` is live in this dist's Java. Ported: 12 element stats (`FirePower`…`DarkRes`) + an `Element` enum, skill `<attributeType>`/`<attributeValue>` (Volcano's FIRE 20 finally counts), both effects as real per-element `StatModifier`s (comma-list `attribute` params handled; `AttackAttribute`'s icon-only marker variant retired — the census test flipped from "is dropped" to "grants HolyPower +20"), NPC template `<attribute><defence …/>`/`<attack type value/>` bases, and the multiplier folded in at Java's exact spots: `calcMagicDam` (both magic sites incl. the drain family), `PhysicalAttack`, `EnergyAttack`, `calcBlowDamage`, `Lethal`'s chance, and a new `element_mod` factor in `calc_effect_land_rate`. Two read paths: players get it free through the generic buff→`StatModifiers` rebuild; NPCs keep no modifier maps, so element stats fold on read over active buffs (template base + Σ debuffs) — which is what lets Day of Doom's −50s and Surrender to X bite mobs. The no-skill-element case ports Java's `getAttackElement` "temp fix": **the attacker's strongest POWER stat elects the element**, so Holy Weapon colors an attribute-less skill holy (pinned). Auto-attacks stay attribute-less — Java itself never calls the bonus on that path. Deferred: item attribute holders (none on this dist), `calcCounterAttack`'s term, the trait half of Lethal's multiplier. **Skill enchanting slice 1 — the sub-level data foundation landed** (plan: [PLAN_G19_SKILL_ENCHANT.md](PLAN_G19_SKILL_ENCHANT.md); slice 2 = the packet flow/transaction/persistence): 413 dist skills declare enchant routes (**20 learnable** — Sonic Storm, Force/Thunder Storm, Rage, Curse Gloom, Dance of Medusa…), all previously invisible. The parser now collects ranged `<value fromLevel toLevel [fromSubLevel toSubLevel]>` rows raw and resolves them per (level, sub) at finalize — **fixing a latent bug**: those rows used to fall into the level-0 slot, where the last row's `{…}` text clobbered the field's scalar fallback (and plain `{N+index}` magic-level tables never parsed at all). A tiny recursive-descent evaluator (`data/skill_expr.rs`) covers the dist's entire 85-expression grammar (`+ − * / ( )`, `base`/`index`/`subIndex`; the one truncated expression the dist ships evaluates to None and drops, like Java's exception path). `SkillData` pre-builds every enchanted variant like Java — keyed `(id, level, subLevel)`, `get_enchanted`/`enchant_routes` accessors, `Skill.sub_level` stamped — and `EnchantSkillGroups.xml` (30 cost rows: SP/chance/Giant's-Codex-items by NORMAL/BLESSED/CHANGE/IMMORTAL type) loads into `GameData.enchant_skill_groups`. Pinned by dist census: Sonic Storm 40's three routes ((1001,1020)/(2001,2020)/(3001,3020)), `+1` power = base+1%, `+10` = base+10%, route 2/3 leaving the other params at base; Curse Gloom's **field-row** duration route with Java's `StatSet.getInt` truncation (+1 = 10.5 → **10**) — `get_i` gained the f64-truncate fallback for exactly this; fragmented route rows (1001–1005, 1006–1006, …) bucket-merge into one registry entry. Verify-by-disabling initially passed **vacuously** again (the first census case exercised only effect-param rows, not field rows — the Curse Gloom case now pins both passes). **Skill enchanting slice 2 — the flow landed; players can enchant** (plan: [PLAN_G19_SKILL_ENCHANT.md](PLAN_G19_SKILL_ENCHANT.md)): the ex-packet family (`RequestExEnchantSkillInfo` 0x0E → `ExEnchantSkillInfo` routes, `RequestExEnchantSkillInfoDetail` 0x43 → the SP/chance/codex cost preview, `RequestExEnchantSkill` 0x0F → the transaction, `ExEnchantSkillResult` 0xFE:0xA8) in `game_loop/skill_enchant.rs`; a new **`SkillEnchants` component** (id → sub-level) parallel to the (id → level) book, persisted in `character_skills.skill_sub_level` (the column was already there, written 0) as **(id, level, sub) triples through the whole persistence chain** — Char, StoredPlayer, per-index subclass banking, the lobby delevel filter (a downgraded skill drops its enchant), the load path, and `SkillList`'s previously-hardcoded sub-level short. The **cast pipeline resolves the enchanted variant end to end**: `use_magic_on` reads the component, and `CastState` carries `skill_sub_level` so the launch/finish/channeling re-lookups keep it — a sabotage run pinned exactly this (zeroing the CastState sub made the +1 cast silently deal base damage while every other assertion held). Java quirks ported as written: the `Rnd.get(100) <= chance` roll (90% rows succeed 91/100), **items consumed before the SP check** (a broke enchanter loses the codex — pinned), the **+2-onward adena-flavored consume** (`destroyItemByItemId(57, holder.getCount())` per holder — the codex is only ever spent on +1, later steps charge its count as adena — pinned), NORMAL failure → route base + `enchantFailLevel`, BLESSED failure keeps the step, CHANGE failure → the raw fail level, and the class gate on `CategoryType.FOURTH_CLASS_GROUP`. Deferred `TODO(G19)`: `UNTRAIN` (no client button), olympiad/sell-buff gates (unmodeled systems). **AdminEffects GM sweep landed — G19 ✅ COMPLETE**: `//setteam`/`//setteam_close`/`//clearteams` (a real `Player.team` behind UserInfo's SLOTS byte 3 and CharInfo's team byte — two more stubbed-zero fields made real; NPC team display TODO(G19), the port's NpcInfo lacks the block), `//settargetable` (`AdminFlags.untargetable`, gated in `handle_action` — Java toggles the GM themselves), `//para`/`//unpara`/`//para_all`/`//unpara_all` (`AdminFlags.paralyzed` ORed into `is_blocked_from_actions`/`is_movement_disabled` beside the buff flags, PARALYZE/FLESH_STONE visuals via the `//ave_abnormal` pin store), `//bighead`/`//shrinkhead`, `//playmovie` (`ExStartScenePlayer` preview; MovieHolder freeze bookkeeping TODO(G19)), `//event_trigger` (`OnEventTrigger` 0xCF fan-out), `//set_displayeffect` (`ExChangeNpcState` broadcast; state not stored — TODO(G19)), and the `//invis`/`//vis` alias family onto `//hide`. One Java-fidelity fix along the way: `AdminData.hasAccess` **auto-grants unlisted commands to the highest access level** (the dist xml genuinely lacks `admin_settargetable`), so the port's existence gate now falls back to that instead of "does not exist". Remaining out-of-milestone tail: `StatUp` (all G24 Territory-War Benefactions), `WeightLimit` (needs a weight model) |
| Game  | G20 Combat breadth                                          | ✅ **ranged attacks landed** (plan: [PLAN_G20_RANGED.md](PLAN_G20_RANGED.md)): bows/crossbows now need **ammunition** (arrow/bolt matched by crystal grade, auto-equipped to LHand via a dedicated `equip_ammunition` — the ordinary equip path refuses `Etc` items *and* would displace the two-handed bow), spend **MP** per shot, consume one arrow, and arm a **reload delay** (`900000/pAtkSpd`) shown as a red `SetupGauge`; out-of-arrows / not-enough-MP refuse the swing. Bow *range* already worked (pAtkRange 500 via G14). Survey note: `PhysicalAttack` skills and root/immobilize were already done (earlier slices + G19). **Multi-hit melee landed** (plan: [PLAN_G20_MELEE_VARIANTS.md](PLAN_G20_MELEE_VARIANTS.md)): the `Attack` packet now carries several hits (it hard-coded "0 additional"), **dual** weapons strike twice at half damage, and the **polearm sweep** hits extra targets in the weapon's radius (66 vs a sword's 40) and 120° arc — gated on `ATTACK_COUNT_MAX`, a *stat* set by Polearm Mastery 216 (`HitNumber` 5), not on the weapon type. **PvP kill consequences landed** (plan: [PLAN_G20_PVP_KILLS.md](PLAN_G20_PVP_KILLS.md)) — **G20's gate is now met**: killing a player moved nothing before (`player_do_die` had a literal `let _ = killer_oid`). Now Java's three branches — lawful PvP kill → `pvp_kills++`; positive-reputation first offence → reset to 0; otherwise karma (`calculateKarmaGain`, 720 rising to a flat 43200 past 180 PKs) + `pk_kills++` — with the PVP-zone "do nothing" short-circuit. Also found & fixed: the death XP penalty applied unconditionally, where Java skips it inside PVP/siege zones. **Over-hit landed** (plan: [PLAN_G20_OVERHIT.md](PLAN_G20_OVERHIT.md)): a killing blow from an `<overHit>` skill (59 learnable — Triple Slash, Sonic Storm…) banks its excess damage and pays it as bonus XP, capped at 25% of the share, with the "Over-hit!" notice. Note `<overHit>` is an **effect** param, not a skill field — the first read had it at skill level and only the real-datapack parse assertion caught it. **Duels (1v1) landed** (plan: [PLAN_G20_DUELS.md](PLAN_G20_DUELS.md)) — the last feature G20 names: challenge → ask → accept/decline → 5 s countdown → fight → end on death/surrender/timeout/separation, with the `canDuel` gates and the five `ExDuel*` packets. **A duel never kills** — the losing blow is capped at 1 HP and ends the duel, so no death penalty, karma or PvP counters move. Party duels need an arena instance (`TODO(G27)`) and are refused. **Death item drops landed** (plan: [PLAN_G20_DEATH_DROPS.md](PLAN_G20_DEATH_DROPS.md)) — a PK past `MinimumPKRequiredToDrop` killed by a player scatters inventory (the karma penalty, *not* general looting: a clean victim keeps everything), while a **monster** kill uses the gentler `Player*` rates. Adena/quest items never drop; equipped items unequip first and use the equip/weapon percentages; arena deaths and GMs are exempt. **G20 is complete** — `SHOTS_BONUS` is provably dead on this dist (zero items declare `reducedSoulshot`), karma decay is blocked on an absent `KarmaData` table, and party duels need G27's instances |
| Game  | G20.5 Recommendations                                       | ✅ **complete** — the row had been left at ⏳ while the work was already done, found when picking the next milestone after G29. Verified against the gate rather than the code: *"a given rec survives relog"* — `character_reco_bonus` load/store, pinned end-to-end by `char_persistence::recommendations_persist` against the real schema; *"the counters reset daily"* — `handle_daily_reco_reset` zeroes `rec_left` and decays `rec_have` for online players and issues `DbCommand::ResetRecommends` for offline ones, scheduled at boot by `reco::schedule_initial_daily_reset` and self-rescheduling 24 h out. `RequestVoteNew` (0x7B) is dispatched to `reco::handle_request_vote_new` with Java's guards (no target / out of recommendations / invalid target), and `RecoGiveTask` grants recommendations-to-give over time. 15 tests in `reco`. |
| Game  | G21 NPC AI & world-content breadth                          | ✅ **NPC skill casting landed** (plan: [PLAN_G21_NPC_CASTING.md](PLAN_G21_NPC_CASTING.md)) — the first of G21's four gate clauses. Mobs could only swing before: **4831 NPC templates carry a castable skill and none ever used it** (73% of those attachments are fully covered by ported effects, 9% partially). `AISkillScope` bucketing from the tail of `NpcData.parse` is now built once at load into `NpcAiSkillIndex` — **the `else if` ladder's order is load-bearing** (a *continuous* skill takes the first arm and never reaches ATTACK even when it also carries a damage effect). Needed a real `Skill.is_continuous`: the Rust `OperateType` collapses `A1`/`A2` into `Active`, so continuity is now read from the raw `operateType` (`A2..A6`/`DA2..DA5`) rather than proxied off `abnormal_time`. `<ai type>` is parsed (`AiType`; this dist has 402 **MAGE**, 220 ARCHER, 3163 BALANCED — a mage casts every think, skipping the `hasSkillChance()` roll and the stand-still requirement). `npc_cast.rs` runs Java's ladder — heal → self-buff → immobilize a moving target → mute a casting one → short/long range → general — hooked into `think_attack` *before* the chase/swing tail. The cast rides the **existing shared** launch/finish path, which needed two player assumptions fixed: `mp_consume` would have been **billed twice** (start now charges only `mp_initial_consume`), and `effects.rs` hard-`expect`ed a `Player` on the caster in 5 places, so **any NPC cast panicked the server** — only the one test that ran a cast end-to-end through the real tick loop caught it. Narrowed with `TODO(G21)`s: `skillTargetReconsider` (no faction plumbing → heal/buff target the caster), the ARCHER kite, and the SUICIDE/RES buckets (nothing declares `isSuicideAttack`; no resurrect effect ported). **Guard PK aggro + faction calls landed** (plan: [PLAN_G21_GUARD_AGGRO.md](PLAN_G21_GUARD_AGGRO.md)) — the second gate clause. `<clanList>` was **dropped entirely** by the NPC parser, so every mob fought alone: now 3760 templates carry factions (4569 `<clan>` entries; `ALL` on either side matches everything) and 82 carry `<ignoreNpcId>` lists. **Town guards** (186 `Guard` templates) seed hate on any player with `reputation < 0` inside a **hardcoded 500** — Java's bare literal, *not* the template `aggroRange` — and **regardless of `isAggressive`**; a lawful player is ignored at any distance. **Corrected 2026-07-19:** this slice originally recorded that guards are flagged *passive* in the datapack — they are not (all 186 carry `isAggressive="true" aggroRange="450"`), and because the test fixture hardcoded the same wrong value, nothing caught that the *generic* aggro scan (gated only on `is_aggressive`) was seeding hate on every lawful player within 450 units and **guards were killing them on sight**. Java reaches that scan for guards too, but every candidate must clear `isAggressiveTowards` → `isAutoAttackable`, which for an NPC attacker is true only via `attacker.isMonster()` — a `Guard` is an `Attackable`, not a `Monster`. The generic scan is now `is_monster()`-gated, leaving the reputation rule as the only way a guard aggros a player. **Faction calls** drag idle clan-mates within `clanHelpRange + collision` into the fight, with three separately-tested gates: only if the target **actually attacked this NPC** (Java's `getAttackByList`; proxied by a non-zero aggro `damage` — without it merely being *noticed* pulls the whole camp), only **idle/active** mates answer, and `ignoreNpcId` beats a shared clan. Also had to let `Guard` into the AI at all: `think()` gated on `is_monster()` and `Guard` isn't in that subtree, though Java's `Guard extends Attackable` runs the same `AttackableAI`. **Raid-boss persistence landed** (plan: [PLAN_G21_BOSS_PERSISTENCE.md](PLAN_G21_BOSS_PERSISTENCE.md)) — **G21's gate is now met**. `dbSave` was parsed by nobody, so all **225** raid-boss spawns (`RaidbossSpawns.xml`) were placed like static ones: every restart handed players a fresh full-HP boss and wiped any pending respawn timer. Ported `DBSpawnManager`/`npc_respawns` — a boss now keeps its **live HP/MP** and its **absolute respawn due time** across a restart. **The ownership split matters**: Java's `spawnNpc` hands a `dbSave` spawn to `DBSpawnManager` instead of spawning it (and only if `!isDefined(id)`), so the static pass now defers them into `pending_boss_spawns` and `resolve_boot` settles them when the DB rows arrive — keeping boot asynchronous while preserving "DB wins" (a test pins that the static pass places *no* dbSave boss, or the restore would double-spawn). Three cases: still-on-timer → scheduled not spawned; elapsed/alive → spawned with stored vitals; no row → full + insert. Guards: a dead row's `currentHp = 0` is **not** restored (it would spawn a corpse) and an over-max stored value clamps. Writes on spawn, at corpse decay (banking the absolute due time so a restart mid-window resumes the wait) and on shutdown. SQL verified against the shipped SQLite schema via `PRAGMA table_info` + a round-trip, not just test doubles. Note **any new unprompted `DbEvent` has two boot-event skip-lists to update** (lib + `char_persistence`) — missing them failed 8 tests. **Minions landed** (plan: [PLAN_G21_MINIONS.md](PLAN_G21_MINIONS.md)) — `MinionList`. The parser deliberately *skipped* minion refs (they'd be mistaken for template starts), so all **460** leaders stood alone; a full world spawn now places **3289** escorts from 962 `<minions><npc>` entries. Rules that invert easily, each tested: a **non-raid** leader's minions never respawn, and a `CustomMinionsRespawnTime` of **0 beats the raid default** (4 ids use exactly that); only a **raid** leader's death clears its escort, so killing the big mob in an ordinary camp doesn't evaporate the camp; pack aggro is asymmetric (leader struck = 10, minion = 1, ×10 for a raid). **A real perf bug surfaced only in e2e**: counting a leader's live minions via a full `world.objects` scan per spawn (~3289 × ~39k) made boot so slow the game server missed its login-server registration and the e2e failed at *login* — replaced with the per-master roster Java keeps (`_spawnedMinions`). Two test-only hazards recorded: `add_test_npc`'s `NPC_OID` **is** `FIRST_NPC_OBJECT_ID`, so a runtime-spawned minion overwrote the hand-placed leader; and ambient NPC idle `SocialAction` (0x27) wasn't in `e2e_create`'s skip-list — **the likely cause of that test's long-noted intermittent failures**, now fixed (4/4 consecutive passes). **EffectZones landed** (plan: [PLAN_G21_EFFECT_ZONES.md](PLAN_G21_EFFECT_ZONES.md)) — zones that periodically cast on players inside them (Blazing Swamp fire, Sea of Spores poison, Hot Springs Haste/Focus/Might). **Picked by behaviour, not count**: `ConditionZone` leads the census at 1080 but **1073 are `NoBookmark=true`** — a later-chronicle feature absent from Interlude — so it's ~99% inert, while the 218 `EffectZone`s (204 with skill lists) are live. Their skills were already-ported effects (`DamOverTime`, stat mods). Required **per-zone `type=` parsing**, which the loader had explicitly deferred (it mapped filename→kind and couldn't read the mixed files); a zone whose type isn't ported is now skipped outright rather than mis-filed. **Bonus: that recovered 20 zones missing from the world entirely** (+7 Peace, +7 NoRestart, +6 Pvp in the previously-unloadable mixed files) — total zones 605 → 843. **27 zones declare `targetClass="Npc"` and cast on nobody** (Java tracks only NPCs as inside, then the tick requires `isPlayer()`) — modelled explicitly so they stay inert; I had the default inverted at first and the dist parse test caught it. Runtime differs from Java by design: instead of per-zone tasks needing a live characters-inside set, one 1 s sweep groups players by occupied zone and fires each on its own `reuse` — chance rolled once per creature (not per skill), `initialDelay` honoured, and Java's affected-level guard means a buff zone grants its buff **once** rather than re-casting forever. **NPC regeneration landed** (plan: [PLAN_G21_NPC_REGEN.md](PLAN_G21_NPC_REGEN.md)) — `doRegeneration` ran for **players only**, so every NPC was frozen at whatever HP it was left on: `base_hp_reg`/`base_mp_reg` were parsed and read by nothing, and a raid boss whittled down across sessions never recovered a point. **14855** templates declare an `hpRegen` (only 58 zero; 8.5 is the commonest). Chosen over the remaining zones after checking `default_enabled`: `DamageZone` is 13 live of 35 and `SwampZone` 2 of 20 — the rest are siege-gated castle traps, so 15 zones total vs 14855 templates. **The NPC formula is much shorter than the player one and that's Java, not a narrowing** — levelMod, CON/MEN and the sitting/standing/running multipliers all sit *inside* `isPlayer()`, so an NPC regenerates its raw template value × the raid-or-normal config multiplier (both 100% here; the raid branch is tested by overriding it). **Regen runs during combat** — Java's task never checks an in-combat flag, which is what makes a long fight vs a high-regen boss a DPS race; there's a test named for it so it isn't "fixed" later. The HP-bar broadcast fires only on an actual change, else every full-HP NPC would emit a packet every 3 s. **NPC pathfinding landed** (plan: [PLAN_G21_NPC_PATHFINDING.md](PLAN_G21_NPC_PATHFINDING.md)) — `Creature.moveToLocation` is shared between players and NPCs in Java, but only the player half was ported (G7.85): `move_npc_to` built a straight-line move with **no geodata consultation at all**, so every chase, drift-return and random walk went through walls. The path worker was already built for this — `PathRequest.playable` is documented "one pass for AI" and had never been called with `false`. Now: destination clamp via `get_valid_location` (with Java's >3000 and intentional-fall skips), the **NPC takes the geodata-corrected z** (`if (!isPlayer()) z = destiny.getZ()` — a player keeps its client's z, a mob doesn't), and a clamp shortfall >30 hands off to the worker against the *original* destination. The reply path was player-only and looked up `clients[client_id]`; client-facing sends are now gated on `has_component::<Player>` rather than a sentinel id that could collide with a real client. Two hazards handled: the AI re-issues a chase every 1 s so there's **one outstanding request per mob**, and that guard is only safe because the worker replies to every request and `PathWait` clears **before** the no-route branch returns — otherwise one unroutable target would freeze a mob permanently (tested). Tests run against **real dist geodata**, with blocked/clear lines probed from Giran square first. **`skillTargetReconsider` landed** (plan: [PLAN_G21_TARGET_RECONSIDER.md](PLAN_G21_TARGET_RECONSIDER.md)) — slice 1 shipped NPC casting with heal and buff hard-wired to the caster for lack of faction data; slice 2 added it. **1040 NPCs carry a buff-bucket skill and 305 a heal-bucket one**, so a pack's healer now tops up whoever is worst off and a buffer buffs its mates. Bad skills draw from the aggro list; good skills from faction-mates + self, with heals sorted by lowest HP%; the heal chance now rolls against the *chosen target's* HP. **Deliberate deviation**: Java's good-skill candidate set is every visible creature and its auto-attackable filter sits *inside* the `isContinuous()` branch — a heal isn't continuous, so as written a mob would heal the **player** fighting it; scoped to the caster's faction instead (does less than Java, never more), with a test pinning it. **This surfaced a latent slice-1 bug**: `check_skill_target` encoded Java's `isAutoAttackable(caster)` test as `target_oid != npc_oid`, which was indistinguishable while buffs were self-only and silently blocked *every* faction buff once reconsider landed — a narrowing that is currently indistinguishable from the real rule becomes a bug the moment the thing it was narrowed around arrives. Survey note: **`FenceData` is a single fence named "demo"** and not worth porting on this dist. **`DamageZone` + `SwampZone` landed** (plan: [PLAN_G21_DAMAGE_SWAMP_ZONES.md](PLAN_G21_DAMAGE_SWAMP_ZONES.md)) — the last zone types with live content; both reuse slice 5's parser and sweep. Zone census now **898** (Damage 35, Swamp 20). **No DamageZone in this dist declares `damageHPPerSec`**, so all use Java's field default of **200**/tick — a number that appears nowhere in the datapack, so reading only the XML would suggest they do nothing. `DamageZone`'s default reuse is 5000 ms (not `EffectZone`'s 30000); the parser corrects for it. `SwampZone` multiplies move speed (0.2 here): Java re-reads the zone inside `SpeedFinalizer`, the port caches it on `Speeds` and refreshes on the enter/exit edges, then recomputes + rebroadcasts `UserInfo` like Java's `broadcastUserInfo()`. **Castle traps are gated twice** — only while that castle's siege runs, and players *defending that castle* are skipped; without the second rule a garrison would cook itself on its own defences during the siege it's fighting (both tested). **Walker routes landed** (plan: [PLAN_G21_WALKER_ROUTES.md](PLAN_G21_WALKER_ROUTES.md)) — **G21 is complete**. 13 routes drive Giran's porters, scribes and the running boy, plus Gordon on a 67-node patrol; only `cycle` and `back` styles occur here. Java hangs a `ScheduledFuture` off each arrival; the port keeps `WalkState` on the NPC and drives a 1 s sweep with two phases — travelling (a `Movement` in flight) and waiting (serving the node's `delay`). **Splitting them matters**: banking the delay before the leg starts would let travel time eat the pause. Java's `back` arithmetic steps back **two** on overrun (the index was already past the end), landing on the second-to-last node; the test pins `0→1→2→1→0→1→2` because an off-by-one makes a walker bounce on the spot. **Verification gap closed**: `tests/user_info_packet.rs` had stopped compiling after the previous slice added a `Speeds` field — I'd only been running `--lib`/`char_persistence`/`e2e_create`. Fixed, and this slice was verified with a plain `cargo test -p gameserver` across **all 8 targets (749 tests)**. G21's remaining items are all blocked or empty on this dist: `HtmCache` (caching only), `CreatureSeeTaskManager` (needs a script engine), `FenceData` (one fence named "demo") **NPC skill cooldowns never applied** (fix, plan: [PLAN_G21_NPC_SKILL_REUSE.md](PLAN_G21_NPC_SKILL_REUSE.md)) — found by the G29 `Creature`-vs-`Player` sweep's last probe. `set_skill_reuse` writes through `if let Some(Reuses)` and players get that component at load, but **NPCs never did**, so the write was a silent no-op; `npc_cast::check_use_conditions` reads the same component and treats an absent one as *ready*. Both halves fail open, so a mob could re-cast a 10 s skill as fast as its AI ticked — the reuse plumbing was written and called correctly from the start, it just wrote into a component nobody had attached. Fixed by attaching `Reuses` on **first use** (only NPCs that cast pay for the map; the world holds ~34.9k). Two tests, because recording and enforcing are separate failures that were broken by the same cause. servitor_tests 109 → 111; npc_cast/raid/boss/combat/skills re-run clean. **Faction call on death landed** (2026-08, plan: [PLAN_G21_GUARD_AGGRO.md](PLAN_G21_GUARD_AGGRO.md)) — reported from live play: Cave Blade Spiders (npc 20462) show `[G]` but their pack never retaliated. Java calls the faction from **two** sites and only `thinkAttack`'s had been ported; the other is `Creature.doDie`'s *"Clan help range aggro on kill"*, and it is the **only one that fires when a mob is one-shot** — the AI thinks once a second, so a monster killed before its first think in `AI_INTENTION_ATTACK` never reached `faction_call` at all. That makes the hole invisible to normal play and total for an over-levelled character farming low-level `[G]` mobs, which is exactly how it was found. `faction_call_on_kill` is wired from `npc_do_die` after `calculate_rewards` (Java's position inside `Creature.doDie`), and the scan is now shared with `thinkAttack` via `faction_recruits`. **Java's two sites deliberately differ and all three differences are mirrored**: the death call requires a **non-GM playable** killer (`killer.isPlayable() && !getActingPlayer().isGM()` — a GM clearing a spawn leaves no aggroed camp), scans the **bare `clanHelpRange`** with *no* collision radius added, and does **not** consult `ignoreNpcId`. The `[G]` marker itself (`Creature.getTitle()` under `ShowNpcAggression`) is exactly "non-empty clan list **and** `clanHelpRange > 0`", so it promises precisely what these two call sites deliver. 5 new tests, each one-shotting through `npc_do_die` with no think tick so `thinkAttack`'s call provably can't be what they observe; sabotage-verified. **2325 gameserver tests green**, clippy clean. **Lesson:** a ported behaviour can have more than one trigger site — grep the *helper* (`getClanHelpRange`), not the feature. **Chase leash (`AggroDistanceCheck`) completed + snap-back (2026-08-01).** The leash body (`npc_ai::npc_leash_return_home`) had been ported as a *walk* home with three of Java's gate clauses missing; it now matches `AttackableAI.thinkAttack` in full — `!npc.isWalker()` (route NPCs exempt), the `AggroDistanceCheckInstances` gate (new config key, False on this dist, so instanced mobs never leash), `spawn.getChaseRange()` as a per-spawn-line override floored at `max(MaxDriftRange, chaseRange)` (new `chaseRange` attribute in `spawn_data`, carried onto the `Npc` instance — Silent Valley and Tower of Insolence set 5000), and "minions should return as well" (the leader's escort goes home to the *leader's* spawn point, healed, hate cleared). **Deliberate deviation:** where Java issues `AI_INTENTION_MOVE_TO` and lets the mob jog home — re-aggroable and re-pullable the whole way — this port **teleports** it (`death::relocate_npc`, the relocate the attack-timeout branch already used), at operator request. The in-flight chase is dropped first (`stop_npc`) or the movement sweep interpolates the mob straight back out. `NPC.ini` on this dist has it on: 2000 / 4000 raid range, raids included, instances excluded, `RestoreLife = True`. 10 new tests (`mob_leash_tests.rs`) covering the fire, the zero case, both `chaseRange` legs, the four exemptions and the escort; sabotage-verified against the walk-home version. **Monster combat parity sweep (2026-08-02, reported from live play).** Three reports, one shared cause and two neighbours. (1) *Stunned by a mob standing on its spawn point.* NPC casts never ran Java's target-type handlers: `AttackableAI.checkSkillTarget` opens with `skill.getTarget(npc, target, …)` and `doCast` re-resolves through the same handlers, but the port cast at whatever creature the AI happened to be thinking about. Catherok's Stun (4072) is `targetType=SELF`/`affectScope=POINT_BLANK`/`affectRange=150` — a shockwave centred on the **mob** — so aiming it at the player made the player the primary affected target and it connected at any distance. **1332 NPC skill attachments on this dist are SELF+POINT_BLANK**, so this was every point-blank mob skill, not one monster. `npc_cast::resolve_npc_cast_target` now ports `targethandlers/{Self,None,Target,Enemy,EnemyOnly,EnemyNot,NpcBody,PcBody,Summon,Ground}.java`, which also restores the `GeoEngine.canSeeTarget` every non-self handler closes with — mobs now cast under the same geodata rules as players. **Two adjacent bugs fell out of it**: `PointBlank.java` rings the *target* (`forEachVisibleObjectInRange(target, …)`, the same reference object `Range.java` uses) and, unlike `Range`, never re-adds the target — the port had it caster-centred *and* seeding the target, which is invisible for the 757 SELF point-blank skills and wrong for the 19 that aren't; and `is_auto_attackable` lacked `Monster.isAutoAttackable`'s `if (attacker.isMonster()) return attacker.isFakePlayer()`, so a mob would accept a faction-mate as an ENEMY skill target. (2) *Porta never teleports you.* Skill 4161's `CallPc` effect was parsed and dropped — the skill cast, animated for two seconds and did nothing. The **NPC half** of `CallPc.java` is now implemented (abort cast/attack/move, `FlyToLocation(DUMMY)`, `setLocation(effector)`); its player half (Summon Friend's `ConfirmDlg` recall) remains `TODO(G30)`, and the skill census comments say so explicitly so the improved coverage number isn't read as "Summon Friend works". Needed a new `FlyToLocation` packet — **Interlude writes 8 ints and stops**; Mobius's `flySpeed`/`flyDelay`/`animationSpeed` tail is a later chronicle's, verified against the C6/Interlude L2J server. (3) *Group aggro walks instead of running.* `AttackableAI.onEvtAggression` calls `setRunning()` before switching intention; the faction-call and minion-assist paths set the attack intention directly, bypassing the one place the port flipped the flag — so a recruited mob chased at ~30 speed instead of ~170. **While in there, `thinkAttack` was finished end to end**: the anti-stacking shuffle, the `AIType.ARCHER` kite *and its flat 850 bow range* (220 ARCHER templates were walking into melee before shooting), the raid/minion target-chaos block (new `RaidChaosTime`/`GrandChaosTime`/`MinionChaosTime` config keys), and the `checkTarget` → `targetReconsider` tail that lets an immobilised mob re-pick rather than stand still. 11 new tests; the two headline ones sabotage-verified. |
| Game  | G22 Quest & script breadth                                  | ✅ **Dwarf first-class transfers landed** (plan: [PLAN_G22_DWARF_CLASS_TRANSFER.md](PLAN_G22_DWARF_CLASS_TRANSFER.md)) — G22 depended on G17, and the class-transfer quests are what G17's `setClassId` unblocked. `DwarfBlacksmithChange1` (→ Artisan 56) and `DwarfWarehouseChange1` (→ Scavenger 54) share one implementation, since the two Java scripts differ only in NPC list / target / proof / talk-category; both call the G17 mechanic, so village-master transfers and `//setclass` now share code. **A Java quirk kept deliberately**: the fourth-class refusal hard-codes the *first* NPC's page id regardless of who you're talking to — that looks like a bug, but only the first NPC of each set ships a `-12` page, so "fixing" it would produce a blank window. A dist-page-existence test **failed on its first run** (the pages live under `data/scripts/village_master/`, not `data/html/`), which would have meant a blank window at the exact moment of a class change. **Elf/Human first-class transfers landed** (plan: [PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md](PLAN_G22_ELF_HUMAN_CLASS_TRANSFER.md)) — unlike the Dwarf pair these serve **two races from the same NPCs** (Human Fighter 0 / Elven Fighter 18; Mage 10 / Elven Mage 25). **The `from_class` half of each match is load-bearing**: Java matches `(classId == TARGET) && (getClassId() == SOURCE)`, and dropping the source check would let a Human take Elven Knight from the same NPC — there's a test asserting exactly that is refused with nothing consumed. Java's nine near-identical `else if` blocks compress to a `(to, from, proof, first_page)` table because each target owns **four consecutive pages** in a fixed order; the page-existence test then sweeps every target's block across every NPC (9×9 + fixed pages), which is what makes the compression safe. **DarkElfChange1 landed** (plan: [PLAN_G22_DARK_ELF_CLASS_TRANSFER.md](PLAN_G22_DARK_ELF_CLASS_TRANSFER.md)), completing the racial first-occupation set — **and fixing a second class-corruption bug**: `QuestCtx::set_class_id` still had the unconditional `base_class_id = class_id` that G17 slice 6 fixed in `//setclass`, so a *quest-driven* transfer while on a subclass would rewrite the base class. All three paths (GM command, village-master script, quest) now share `subclass::set_class_id`. I'd recorded the "every existing writer becomes suspect" lesson last milestone, fixed one writer, and moved on — finding the second by accident is the cost of not enumerating them. Three ways DarkElfChange1 differs from its siblings, each silent if mis-ported: Java already writes it as a **table** and the event is the **row index** not a class id; the page order is `lowNoProof, low, noProof, done` (opposite pairing to ElfHuman's); and the pages are **`.html`** not `.htm`. Also honours `isSubClassActive()` → refuse, newly expressible after G17. **FirstClassTransferTalk landed** (plan: [PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md](PLAN_G22_FIRST_CLASS_TRANSFER_TALK.md)) — the seven newbie-village headmasters, who (per Java's own header) *only talk about* transfers. Two conventions differ from every other village-master script: pages use an **underscore** (`30026_fighter.html`) and `.html`. **The page availability is asymmetric and IS the logic**: the Human fighter-guild master ships no `mystic` page and the temple master no `fighter`, so a mage at the fighter guild gets `no.html` rather than a constructed filename that would 404 — a test asserts those three absences so the branching can't drift to a "sensible" symmetric version. Also strengthened the main test: it first only checked the reply was non-empty (which would pass while serving the *wrong* page), now it compares byte-for-byte against the dist file through `strip_htm` + `%objectId%`. **The entire first-occupation group is done — 8 of 16 village-master scripts.** **Dwarf second-class transfers landed** (plan: [PLAN_G22_DWARF_SECOND_CLASS.md](PLAN_G22_DWARF_SECOND_CLASS.md)), opening the `*Change2` group: Artisan→Warsmith and Scavenger→Bounty Hunter. **Three differences from `*Change1`**: level **40** not 20; **three** proof items required and all consumed — Java's `hasQuestItems(a, b, c)` is an **AND**, and reading it as "any" would let a player transfer on one mark (tested with two of three); and a **C**-grade coupon reward. Structural quirk: **every** page is hard-coded to the *first* NPC's id whichever of the eight masters you talk to (the `*Change1` scripts did this only for the fourth-class refusal) — the dist ships one 12-page set per script, and the test asserts the other masters ship nothing, so it can't be tidied into per-NPC pages that would 404. **Orc + Dark Elf second-class transfers landed** (plan: [PLAN_G22_ORC_DARKELF_SECOND_CLASS.md](PLAN_G22_ORC_DARKELF_SECOND_CLASS.md)) — they look like siblings and differ in **four** ways, each silent if one is ported by copying the other: the bypass event is the **class id** (Orc) vs the **row index** (Dark Elf); `.htm` vs `.html`; page order `low, lowNoProof, done, noProof` vs **`lowNoProof, low`, noProof, done**; and — the real trap — Orc pays 15 C-grade coupons while **DarkElfChange2 pays nothing at all** (verified by counting: `grep -c giveItems` → Orc 4, Dark Elf 0; copying the Orc branch would have handed out 15 free coupons per transfer). The page owner also isn't the first NPC for Dark Elf — it's **30474, the third**. Process fix: the transfer test failed on first run for the **fourth consecutive slice**, always the same fixture gap, so the quest fixture now registers the whole class range `0..=57` instead of an enumerated list. **Elf/Human second-class transfers landed** (plan: [PLAN_G22_ELF_HUMAN_SECOND_CLASS.md](PLAN_G22_ELF_HUMAN_SECOND_CLASS.md)), closing the `*Change2` group with its three widest scripts — Fighter (10 targets, 477 Java lines), Wizard (5), Cleric (3). **The finding is that they are uniform**: after a slice spent on the four silent ways Orc and Dark Elf differ, I went looking for the same axes here and there are none — same level 40, three-proof `AND`, 15 C-grade coupons, `.htm`, class-id bypass event, and `low, lowNoProof, done, noProof` page order — so the port has **no per-branch code path**, just one `Spec` table. Worth stating, because the previous slice is exactly the prior that would push you to invent per-branch handling that isn't there. What *does* differ is the greeting gate: each script serves a Human and an Elven line from one NPC set through a **different pair of race categories** — `HUMAN_FALL`/`ELF_FALL` (fighter), `HUMAN_MALL`/`ELF_MALL` (mystic), `HUMAN_CALL`/`ELF_CALL` (cleric). Three near-identical names; the wrong one greets the right player with the class-mismatch page. The **`from_class` half of each row is load-bearing and worse here than in Change1**: all ten Fighter targets hang off one NPC, so matching on the target alone would let a Human Knight take **Temple Knight**, an Elven Knight's class, from the same master — tested by handing a Human Knight exactly those marks and asserting nothing happens and nothing is consumed. Two Java behaviours preserved that read as bugs: every page is hard-coded to the *first* NPC's id (the dist ships one page set per script; a test asserts the other masters ship nothing), and `THIRD_CLASS_GROUP` is checked *before* the source-class match. All 5 tests passed on **first run** — the first slice in five to do so, which is the payoff from slice 6 replacing the quest fixture's enumerated class ids with the full `0..=57` range; fixing the pattern rather than the instance held. **AllianceMaster landed** (plan: [PLAN_G22_ALLIANCE_MASTER.md](PLAN_G22_ALLIANCE_MASTER.md)) — 67 Java lines, the smallest of the 16, and **the village-master group is now complete at 16 of 16**. The whole script is one guard: `onTalk` always opens `9001-01.htm`, and `onEvent` echoes the requested page back unless the player has no clan (`9001-04.htm`). **The asymmetry is the script and is easy to "fix" away**: the menu is explicitly excluded from the gate, so a clanless player *does* see both buttons and only learns they can't use them after clicking — gating `onTalk` too, which reads tidier, would change what retail shows; a 6-case test pins both halves. Pages are numbered against a **virtual NPC id** (`9001-NN.htm`, as `ClanMaster` uses `9000`) and no real master ships one, asserted so it can't be "corrected" into a per-NPC name that would 404. **Stated plainly because it would otherwise be rediscovered as a bug: this makes the dialog work, not alliances.** Both buttons post `create_ally`/`dissolve_ally`, `VillageMaster.onBypassFeedback` verbs that are **not routed here** — the alliance system is G18, where `ally_id`/`ally_name` currently exist only as a DB column list and a "when the alliance system lands" comment. I checked the failure mode instead of assuming: unrouted `npc_` verbs hit the router's fallback `warn!` and drop, so the buttons are inert but greppable at runtime, and a `TODO(G18)` names both verbs. This matches how `ClanMaster` already ships with `learn_clan_skills`/`multisell` unrouted, but it is the same shape as the dead-button bugs this port keeps hitting (`Chat <page>`, the race-track gatekeeper), so it is recorded as a known gap. Added `QuestCtx::has_clan` alongside `is_clan_leader`. **The Elven first-occupation quests landed** (plan: [PLAN_G22_ELVEN_PATH_QUESTS.md](PLAN_G22_ELVEN_PATH_QUESTS.md)), opening G22's quest body — and the slice was **chosen by a gap the previous eight created**: all 16 village-master scripts were done, and every one consumes a proof item **no quest in the port produced**, so the transfers were reachable only via `//setclass`. `Q00406_PathOfTheElvenKnight` and `Q00407_PathOfTheElvenScout` award the Elven Knight Brooch (1204) and Reisa's Recommendation (1217), making the elven half of `ElfHumanFighterChange1` reachable in normal play. **The finding: Q00406 deliberately ignores `RateQuestDrop`.** It hand-rolls `getRandom(100) < chance` + plain `giveItems` instead of calling `giveItemRandomly`, which multiplies *both chance and amount* by the rate — so reaching for the port's faithful `give_item_randomly` helper would have silently scaled a drop Java leaves alone. Caught only by diffing against `Q00303_CollectArrowheads`, which *does* call the helper; the two look identical in shape and differ in exactly this. Test pins it with `RateQuestDrop = 3.0` → still one topaz per kill. **Generalised: check whether the Java quest calls the helper or rolls its own before picking the Rust primitive — they are not interchangeable.** Q00407's tag mechanic needs **both** hooks: `onAttack` stamps the mob's script value with the attacker's object id and `onKill` pays only on a match — porting one alone fails silently in opposite directions (kill-only never matches; attack-only leaks the tag). Tested both ways. Page conventions: extensions are **mixed inside one quest** (`.htm` pre-accept, `.html` after), and Prias ships `-01`/`-02`/`-04` but **no `-03`**, which Java never names — asserted so the gap isn't helpfully filled in, the same shape as `FirstClassTransferTalk`. Also collapsed a Java three-way level branch awarding identical exp/sp in all three arms (commented, so it doesn't read as a dropped case). The chain test failed once on first run: I asserted the quest record was gone after `exitQuest(false, …)`, but a one-time exit keeps it **COMPLETED** — deleting it would let the quest repeat. Assertion corrected, not the code. Added `QuestCtx::social_action`. 14 quests ported. **Path of the Warrior + Path of the Rogue landed** (plan: [PLAN_G22_PATH_WARRIOR_ROGUE.md](PLAN_G22_PATH_WARRIOR_ROGUE.md)), awarding the Medallion of Warrior (1145) and Beziques' Recommendation (1190) — `ElfHumanFighterChange1` now has four of its five proofs. **The finding: the same `ItemChanceHolder` type, two different denominators.** Q00406 rolls `getRandom(100) < chance`; Q00403 rolls `getRandom(REQUIRED_ITEM_COUNT)` — i.e. **`getRandom(10)`** — so a "chance" of 2 means 2% there and **20%** here. Reading Q00403's table as percentages (the obvious assumption, same type used that way one quest earlier) would have made every Spartoi bone **10× too rare**, turning a ~13-kill stage into ~125. **The denominator is a property of the call, not of the table** — the same shape as the previous slice's `giveItemRandomly` finding: a quest's drop maths is not inferable from the types it uses, so read the roll. Q00401's spider stage meanwhile has **no chance roll at all** — the weapon gate, not a rate, is what makes it slow. Quests 401/403 share a byte-identical `onAttack` state machine — the "kill it solo with the quest weapon" tag (0 → 1 on the right weapon, → 2 terminal on a weapon change *or* a second attacker; `onKill` pays only on 1) — now factored into `scripts/quest_common.rs` since 402/415 use it too. Both hooks load-bearing in opposite directions, as in 407. Two framework pieces: **`Npc.vars`** (Java's `getVariables()`, needed for `lastAttacker`) — shape chosen after checking breadth, 11 quests use it under 6 keys, so a generic map beats six `spoiler_object_id`-style named fields, and an empty `HashMap` doesn't allocate; and **`QuestCtx::npc_say_to_player`**, because the Cat's Eye Bandit taunts its attacker with `sendPacket` but broadcasts its death line — using the existing broadcasting `npc_say` would have leaked the taunt to bystanders. 5 tests, first-run green. Q00401's `/10` roll is pinned deterministically (force the roll to 4 → no drop; `getRandom(100) < 40` would drop), but **Q00403's is deliberately statistical**: a forced roll ignores the bound, so no forced test can tell `/10` from `/100` — it asserts the rate instead (chance 8 = 80% caps 10 bones within 40 kills; 8% essentially never), re-run 10× to confirm it isn't flaky. 16 quests ported. **Path of the Human Knight landed** (plan: [PLAN_G22_PATH_HUMAN_KNIGHT.md](PLAN_G22_PATH_HUMAN_KNIGHT.md)) — 629 Java lines, the widest of the Path family, taken alone because it **completes the proof set for `ElfHumanFighterChange1`**: all five targets are now reachable in normal play, closing (on the fighter side) the gap opened three slices ago. Structurally unlike its siblings: **six independent sub-quests of which you need three** — six officers each trade a badge for N trophies for a Coin of Lords — so most of those 629 lines is one block six times, ported as a `BRANCHES` + `DROPS` table. **The completion path forks on the coin count and the 6-coin case is the odd one:** 3 coins and 4–5 coins each open a prompt whose confirm button (`30417-13`/`-14`) does the awarding, but **6 coins completes immediately inside `onTalk`** with no confirmation. It reads like an oversight; the dist backs it up (`-12` is a completion page, not a prompt), so it's kept and tested both ways — tidying the asymmetry would either add a prompt nobody can answer or silently drop the 6-coin completion, and the player who did all six sub-quests is exactly the one who'd hit it. The confirm handlers also sweep **all** leftover badges/trophies (a player may have part-finished other sub-quests); the 6-coin path takes only coins and the mark, correct there since every badge was already spent. Quirks verified rather than assumed: the quest **never calls `setCond`** — not once in 629 lines, so the quest window shows one step throughout (confirmed by grep, not inferred from the sections I read); Vasper's extensions **alternate** (`-01..05`/`-07`/`-08` are `.htm`, `-06` and `-09..15` are `.html`) rather than splitting on a prefix, so the test asserts `30417-07.html` and `30417-06.htm` are *absent* to stop it being regularised; and Raymond alone ships six pages (an extra intermediate page shifts his later ones up by one), encoded per branch with a test that no other officer has a `-06`. **Two of the six trophies have no chance roll at all** — easy to miss across six near-identical blocks, so the table stores `Option<i32>` and ten unforced kills are asserted to yield exactly ten necklaces. 6 tests, first-run green. 17 quests ported. **Path of the Human Wizard + Path of the Cleric landed** (plan: [PLAN_G22_PATH_WIZARD_CLERIC.md](PLAN_G22_PATH_WIZARD_CLERIC.md)) — the Bead of Season (1292) and Mark of Faith (1201), so `ElfHumanWizardChange1` now has **2 of its 4** proofs. **Q00404 is four identical elemental branches with one exception.** Fire → Wind → Water → Earth each run the same token → collect → trinket bargain, repeating right down to the page numbering (`{npc}-01..04.html` for all four), so it ports as an `ELEMENTS` table. **The exception is Wind: its collectable is not a drop** — the feather comes from a dialog bypass on the Wasteland Lizardman, who sits outside the four-page scheme. A table-driven port assuming "collect ⇒ kill" would leave that branch permanently stuck; tested specifically. **Chance denominator is `/100` here** (`getRandom(100) < 20 **[Verified 2026-07-31]** — the gate is met as far as this chronicle allows. 163 quest engines; a representative **one-time**, **repeatable** (Q354/Q360/Q641/Q642) and **class-transfer** (all 16 village-master scripts) quest completes. The gate's other two kinds do not exist here: **no quest in this dist creates an instance** (`grep createInstance data/scripts/quests` → nothing; the instance engine's only content consumers are Frintezza's lair and the TvT arena), and the only **daily** quests in the tree — Q500, Q933, Q935 — are the off-chronicle skips the audit already ruled out. 30 `TODO(G22)`s remain, all per-script side effects (tamed-beast buff task, sepulcher persistence, region-scoped shouts), not missing quests.|80`) where 401/403 use `getRandom(10)` — the **third** distinct denominator convention in the Path family, checked per call site rather than carried over. **A test I deliberately did not write, and why:** no honest deterministic test for that denominator exists — `forced_rolls` ignores the bound, so `forced < chance` is literally the same predicate under either reading. Q00403's statistical trick doesn't transfer either: there the misreading made drops *rarer* (8% vs 80%), which 40 kills detect; here it would make them *more common*, and with a cap of 1 you get one Bernoulli per quest instance, so detection needs many worlds for little value. Pinned by a call-site comment instead. Better no test than one that appears to prove something it can't. **Q00405 has two things that break if normalised:** Simplon hands over a **stack of three** books where the other two givers give one each (and completion takes `-1`/all of his but `1` of theirs — treating them uniformly strands two or makes the check unsatisfiable; tested); and the cond-2 checks contain a **no-op `>= 0` term** — each giver re-checks all three counts but writes its own slot as `>= 0`, a placeholder for "the one I just handed over", so all three sites reduce to one predicate. Read literally it looks like a bug; it's only redundant, and collapsing is safe because the giver's own count is non-zero at that point. Praga's pendant drops with no roll at all. 5 tests, first-run green. 19 quests ported. **Path of the Elven Oracle landed** (plan: [PLAN_G22_PATH_ELVEN_ORACLE.md](PLAN_G22_PATH_ELVEN_ORACLE.md)) — the Leaf of Oracle (1235), `ElfHumanWizardChange1`'s **3rd of 4** proofs. **Taken alone rather than paired with 408 as planned**: I checked both quests' framework needs first — 408 uses none of `addSpawn`/`addAttackPlayerDesire`/`setMemoState`, 409 uses all three (23 call sites) — and carrying three new primitives plus a second 446-line quest is how sloppiness gets in. **The first quest in the port that spawns its own monsters:** Allana's re-enactment and Perrin's Tamil are ambushes conjured beside the NPC you're talking to and set on you. New framework: `QuestCtx::memo_state`/`set_memo_state` (Java stores it as the quest var `memoState` — confirmed in `QuestState.MEMO_VAR`, not guessed), `QuestCtx::spawn_attacker` (`addSpawn` + `addAttackPlayerDesire`, reproducing `Rnd.get(50,100)` per axis with independent sign), and `npc_ai::seed_attack` promoted to `pub(crate)`. **`memoState` is a second progress axis, not `cond`:** `cond` drives the client window, `memoState` is script bookkeeping, and they move in *opposite* directions — talking to Manuel empty-handed at `memoState == 2` rewinds it to 1 while pushing `cond` to 8. Collapsing them breaks the re-enactment restart path. The ambush tag is also **not** `quest_common`'s: it gates on one attacker with **no weapon check** and keys `firstAttacker`, so routing it through the shared helper would have silently added a weapon requirement — the test kills bare-handed to pin that. **The bug that cost the time was in the test fixture.** The memo test failed with a no-quest reply; instrumenting showed the talk arriving at **npc 27032, a lizardman**, instead of Priest Manuel — because `NPC_OID` and `world.next_npc_object_id` **both start at `FIRST_NPC_OBJECT_ID`**, so the first runtime spawn lands on a fixture NPC's object id and silently replaces it. No test had ever spawned at runtime before. Fixed in the shared `add_test_npc` (it now reserves each id against the allocator) rather than by shuffling my own ids — every future spawning quest would have hit it. All seven major modules re-run green after (quests 76, combat 33, npc 71, guard_aggro 13, admin 89, items 37, clans 16). 4 tests. 20 quests ported. **Path of the Elven Wizard landed** (plan: [PLAN_G22_PATH_ELVEN_WIZARD.md](PLAN_G22_PATH_ELVEN_WIZARD.md)) — the Eternity Diamond (1230), the last of `ElfHumanWizardChange1`'s four proofs. **The whole Elf/Human first-occupation tier is now self-sufficient**: both Change1 scripts (5 proofs + 4) are satisfiable entirely in normal play, which took nine quests. Three parallel errands, all required in any order, each the same four beats (introduction → charm → gated drop → gem), so it ports as one table. **The third errand is missing a step, and the dist proves it isn't a bug:** errands 1 and 2 swap introduction→charm in a **dialog event**, errand 3 does it inline in `onTalk`. Exactly the asymmetry one would "regularise" — until you count pages: Greenis and Thalia ship four each, **Northwind ships three**. There is no fourth page for an event to return, so adding one would 404 the moment a player takes that errand. Kept as `swap_event: Option<&str>` and the page test asserts `30423-04.html` does *not* exist. Same shape as `FirstClassTransferTalk`'s asymmetric pages and Q00407's missing `30426-03` — **when a script looks inconsistent, check whether the dist's page set explains it before normalising.** Like 402, `setCond` appears **zero** times in 446 lines (grepped, not inferred) — progress lives entirely in which items you hold. Denominator `/100`, as in 404/406. 3 tests, first-run green. 21 quests ported. **Path of the Palus Knight + Path of the Assassin landed** (plan: [PLAN_G22_PATH_DARKELF_1.md](PLAN_G22_PATH_DARKELF_1.md)), opening the **Dark Elf** tier — the Gaze of Abyss (1244) and Iron Heart (1252), so `DarkElfChange1` has **2 of 4** proofs. **Every drop in both is unrolled** — no `getRandom` in either `onKill`, so 13 kills is 13 skulls and 10 is 10 molars. Stated because 412/413 *do* roll and porting by analogy would add a chance that isn't here; the tests use no forced rolls at all, which is only a valid way to assert exact counts *because* the drops are unrolled. **Q00411 is one token walking a chain.** Java writes every branch as "hold this and **none** of the others" — seventeen times across three NPCs — which encodes one fact: exactly one token is in the bag at a time, since each hand-over takes before it gives. The port asks *which* token is held and matches once. The invariant is the quest's own design (checked transition by transition), not an assumption; the molars are the deliberate exception (they coexist with Leikan's note), pinned by a test that his page tracks the molar count while the token stays put. Two redundant Java terms collapsed with the reasoning recorded: a `silk >= 4` re-test inside `== 5`, and a genuinely **dead** Kalinta branch (`!has(SILK) && has(CARAPACE)` sits under `!(both)`, which already catches it) — the reachable state→page table is documented at the site so the equivalence stays checkable. **The page test earned its keep, failing on first run:** I'd asserted the `.htm`/`.html` split identically for both quests, but **410's accept page `30329-06` is `.htm` while 411's `30416-06` is `.html`** — the split point differs per quest even inside one race tier. Now asserted separately, with an explicit check that `30416-06.htm` does *not* exist. 5 tests. 23 quests ported. **Path of the Dark Wizard + Path of the Shillien Oracle landed** (plan: [PLAN_G22_PATH_DARKELF_2.md](PLAN_G22_PATH_DARKELF_2.md)) — the Jewel of Darkness (1261) and Orb of Abyss (1270). **The Dark Elf first-occupation tier is COMPLETE**: `DarkElfChange1` has all four proofs. Two races done, two to go. **Q00412 repeats quest 408's third-errand asymmetry — and twice makes it a convention.** Charkeren and Annika hand their tool over via a **dialog event**; Arkenia does it **inline in `onTalk`**, exactly as Northwind does in 408 where Greenis/Thalia use events. One occurrence looked like an oversight worth documenting; two independent quests in different race tiers makes it a datapack convention, so it's modelled (`tool_event: Option<&str>`) without further hedging, both branches exercised in one test loop. Arkenia also omits the `SEEDS_OF_DESPAIR` guard her siblings carry — kept, since adding it for symmetry would change who can start her errand. **Q00412's chance is an equality, not a threshold:** `getRandom(2) == 0` where every other Path quest uses `<`. Same 50% here, but not interchangeable — read as `getRandom(2) < 2` every kill pays. Unlike the `/10` vs `/100` cases this one **is** deterministically testable (a forced roll of 1 separates the readings), so there's a test. That's **four** distinct chance conventions in this family now: `/100`, `/10`, `== 0`, and no roll at all. **Q00413's succubus kill is a swap, not a drop** — it *consumes* a Blank Sheet to make a Bloody Rune, so the counts move in opposite directions and the cond tests **both** (sheets exhausted AND five runes). Modelling it as a capped drop would strand five sheets and never fire the cond; tested per-kill in both directions plus a sixth succubus proving no sheet means no rune. Talbot hands over **five** sheets in one `giveItems(..., 5)`, the same stack shape as Simplon in 405; and neither of 413's drops rolls while 412 rolls all three — conventions differ quest by quest even inside one tier. 4 tests, first-run green. 25 quests ported. **Path of the Orc Raider landed** (plan: [PLAN_G22_PATH_ORC_RAIDER.md](PLAN_G22_PATH_ORC_RAIDER.md)), opening the Orc tier with the Mark of Raider (1592). **Scoped down mid-slice** — planned as 414+416, but 414 carried two things worth doing carefully, so 416 follows rather than being rushed to hit the announced pairing. **Green blood is a rising summon meter, not a collection.** Java races the *held count* against the RNG: `blood <= getRandom(20)` gains one, otherwise it **wipes the stack and summons Kuruka onto the player**. At 0 blood the gain is certain, at 19 it's 5%, at 20 the summon is guaranteed. The blood is never handed in — and the tooth the quest wants drops from **Kuruka**, not the goblins, so porting the blood as an ordinary capped collection would make the quest **unfinishable**. Two tests pin the fork and the tooth source. Reuses `spawn_attacker` from slice 13; fidelity gap recorded (Java's `isSummonSpawn` animation + `addDamageHate` 999 vs our dominant-hate seed). **A branch dead at both ends — and the order I checked mattered.** Karukia's `07b` route sets `memoState=2`/`cond=5` and leads to events on NPC **31978**, who ships five pages here but is **registered nowhere** (`grep -rln 31978 data/scripts/` finds only this quest's file and its own orphaned pages). Separately, `30570-07.htm` offers **only** the `07a` button. Had *only* the serving end been missing, `07b` would be a trap — it consumes the map and all ten teeth but hands out no reports, the sole path to the reward, stranding the player permanently. Because the button doesn't exist either, there's no trap and the route ports verbatim at zero risk. Kept with a `TODO(dead)` and a test asserting **both** halves so nobody restores one end without the other. 5 tests, first-run green. 26 quests ported. **Path of the Orc Monk landed** (plan: [PLAN_G22_PATH_ORC_MONK.md](PLAN_G22_PATH_ORC_MONK.md)) — 652 Java lines, **the widest quest in the Path family**, awarding the Khavatari Totem (1615). **The weapon gate is the INVERSE of quests 401/403.** Those demand a specific quest weapon; this one demands `weapon == null || FIST || DUALFIST` — an Orc Monk fights unarmed, so **"no weapon" is the pass case**. Routing it through the shared `quest_common` tag would have flipped the entire quest: every bare-handed kill paying nothing and every sword kill paying. Needs the weapon's **type**, not id, so `QuestCtx::is_bare_or_fist_handed` was added; tested bare / sword / fist. Its tag variable is `Q00415_last_attacker` — a **third** name after `lastAttacker` (401/403) and `firstAttacker` (409). **The pouch stages take five kills, not four:** Java gives a trophy per kill and converts when the count is *already* 4, so the fifth kill fills the pouch. Reading it as "collect 4" leaves the pouch permanently unfillable — the conversion branch is never entered. The fourth pouch is the same shape over four mobs at three each, converting on the twelfth kill. Both tested per-kill. **Half the quest is unreachable — the same two-sided orphaning as 414.** `09c` opens an entire alternate ending through NPCs 31979/32056, with its own stages, a raid mob and its own reward hand-out — but `30587-09a.html` offers only the `09b` button and neither NPC is registered anywhere, leaving **13 orphaned pages**. Checked both directions again: had only the serving end been missing, `09c` would strand the player (it takes Rosheek's letter and gives no recommendation). Ported verbatim with `TODO(dead)` on the events, both dead kill handlers and the `memoState == 2` talk branch. **Two of two Orc quests now carry a fully orphaned alternate route — expect it in 416.** 5 tests, first-run green. 27 quests ported. **Path of the Orc Shaman landed** (plan: [PLAN_G22_PATH_ORC_SHAMAN.md](PLAN_G22_PATH_ORC_SHAMAN.md)) — the Mask of Medium (1631). **The Orc tier is COMPLETE**; three of four races done. Ported off groundwork from an aborted previous attempt, where I stopped rather than rush a 525-line quest needing unchecked framework — and two of the three gaps that analysis flagged turned out not to exist. **`ItemChanceHolder.count` is a cond SELECTOR here, not a quantity:** `if (item.getCount() == qs.getCond())`, with `chance` as a 0..1 probability for `giveItemRandomly`. Read `count` normally — as quests 403/406 use it — and grizzly bears drop **six** bloods a kill while the cond gate silently vanishes. Tested both sides (nothing at cond 1, exactly one at cond 6). **Fourth** distinct reading of this type in the family after `/100`, `/10` and `== 0`. **Two summon meters differing in the one way that matters:** the Durka parasites escalate exactly like 414's green blood (5 → 1-in-10, 6–7 → 2-in-10, 8 certain, success wipes the stack and conjures a spirit) — but **Java does not set this one on the player**, where 414 does. Needed `QuestCtx::spawn_near_npc` (with `spawn_attacker` refactored onto it); reusing `spawn_attacker` was the natural move and would have invented aggro the datapack never asks for. The test asserts the spirit is *not* in the aggro list. **What the groundwork got wrong, usefully:** `NpcSay` string parameters aren't needed (both such lines live inside the dead branch, so the live path never reaches them) and `getRandomPartyMemberState` reduces to the killer exactly as `q00303` already documents — a `TODO(G13+)` deviation, not new machinery. The `memoState` 100–110 branch is again **dead at both ends** (third Orc quest running: sole entry `30585-14.html` is offered by nothing, and 31979/32057/32090 are registered nowhere) — here **omitted rather than stubbed**, since half-porting it would carry dead memoState handling and a packet feature we lack. Also: the accept event is **`START`**, not `ACCEPT`; and `cond 10` is never assigned (9 → 11). 6 tests, first-run green. 28 quests ported. **Path of the Artisan landed** (plan: [PLAN_G22_PATH_ARTISAN.md](PLAN_G22_PATH_ARTISAN.md)) — the Final Pass Certificate (1635), opening the Dwarf tier. **The leader-tooth roll has a hole in it:** below 5 the kill pays *only* if one tooth is already held, so the first drops at 50% and the second at 100% — a flat "50% per tooth" reading is wrong in both directions (three forced-roll cases pin it). Consequence kept, not fixed: the `else` branch pays the second tooth **without** the `cond 2` check the other branch performs, so finishing that way leaves the quest window stale. Every downstream branch tests item counts rather than the cond, so the quest still completes — a cosmetic Java bug, ported verbatim. Also two routes to Kluto's letter differing only in whether `setCond(4)` chimes. **Dead at both ends for the fourth quest running** (`30527-08c` + NPCs 31956/31963/32052); omitted rather than stubbed, as in 416. **The dead-branch test caught my own error rather than the port's**: the first version scanned every file in the quest directory including the `.java` source, which of course names `08c` as a case label — the very handler being proven unreachable — so it fired on the evidence. Restricted to `.htm`/`.html`. 4 tests. 29 quests ported. **Path of the Scavenger landed** (plan: [PLAN_G22_PATH_SCAVENGER.md](PLAN_G22_PATH_SCAVENGER.md)) — 690 Java lines, the largest in the family. **ALL EIGHTEEN `Path of the *` quests (401–418) are now ported**, so every race's first-occupation script is proof-complete and reachable in normal play. **`dropChance` is documented as a 0..1 fraction and this quest passes `50`** — not 50%, but fifty times certainty, so **every qualifying kill drops** (`q00303` passes `0.4` for a real 40%, so the convention isn't in doubt). A datapack bug with a live effect; the dist is authoritative, so the port passes `50.0` and matches the shipped server. Writing the "obviously intended" `0.5` would halve the rate against retail — a silent divergence in the direction that looks like a fix. The test kills six tarantulas unforced and asserts six beads (at a real 0.5 it'd fail ~98% of the time). **Spoil-gated payouts** — the Scavenger's own mechanic: jars and beads pay only off a corpse that `isSpoiled()`, and `onAttack` separately disqualifies a mob whose spoiler *is* the attacker. Added `npc_is_spoiled`/`npc_spoiler_object_id`. Its npc var is `FIRST_ATTACKER`, a **fourth** spelling. **Two counters packed into one integer:** `memoStateEx(1)` is radix-packed — +10 per delivery (tens), +1 per Mion dialogue step (units), read back via `% 10` and `< 20`/`< 50`. Treating it as one counter breaks both halves; added `memo_state_ex`/`set_memo_state_ex` (a second memo axis). `FLAG` is a **third** summon-meter shape (`20 * flag` percent, reset on success) after 414's and 416's. `npc.deleteMe()` needed `delete_npc`. Dead at both ends for the **fifth** quest running (NPC 31958). 5 tests, first-run green. **30 quests ported; the Path family is complete.** **Q00210 Obtain a Wolf Pet landed** (plan: [PLAN_G22_WOLF_PET.md](PLAN_G22_WOLF_PET.md)) — a standalone four-NPC dialog chain (Lundy 30827 → Bella 30256 → Bynn 30335 → Sydnia 30321 → back to Lundy) handing over the **Wolf Collar 2375**; chosen because it closes a dangling-value link — it's *how you obtain the starter wolf pet*, and the G29 pet system that consumes the reward is already built. Pure talk chain (no kills), min level 15 via `addCondMinLevel` handled in `on_talk`'s CREATED branch. **A test caught a packet split**: `no_level.htm` is a `.htm` file, so it ships as `ExNpcQuestHtmlMessage` (the quest-window packet, 0xFE/0x8E) *not* a plain `NpcHtmlMessage`, so the level-gate assertion had to decode the extended packet. Java's dead `30827-04.htm` case (no button links it — grepped the html set) transcribed anyway. quests_tests 113 → 115 (full chain through the real bypass router incl. an out-of-order-cond guard; the level gate); registration + cond-guard sabotage-verified. **31 quests ported.** **Q00261 Collector's Dream landed** — a clean newbie hunting loop (Alshupes 30222; kill Hook/Crimson/Pincer spiders for 8 legs → 700 adena, repeatable, level 15–21), a near-clone of `Q00303` reusing its `start_condition_html` (`addCondMaxLevel(21)`) + `give_item_randomly` + `on_kill` shape. **Finding — `giveNewbieReward` is dead almost everywhere:** it's commented out (`// Q00281_HeadForTheHills.giveNewbieReward`) in every newbie quest *except* Q00261 and Q00276, and `GUIDE_MISSION` (the player variable it sets) has **no reader** anywhere in the port or the dist scripts — so it's inert bookkeeping, and its `ExShowScreenMessage` is unported. Deferred with a `TODO(newbie-guide)` at the completion site (belongs with the newbie-guide mission system) rather than porting a 347-line packet + a dead variable for one hunting quest. quests_tests 115 → 117 (the kill→turn-in loop through the real router; the max-level refusal); max-level-gate + registration sabotage-verified. **32 quests ported.** **Q00257 The Guard is Busy landed** — the canonical first-hour Gludio quest (Gilbert 30039, level 6–16): kill orcs/werewolves for trophies, adena by type (5/8/10a + 1000 for 10+). **The mechanic is a per-mob hand-rolled drop table** — nine monsters, each `getRandom(random) < chance` with denominators of 10 or 100 (not `giveItemRandomly`, so un-multiplied by `RateQuestDrop`, exactly as Java writes it — the [[l2r-quest-drop-helper-vs-handroll]] call), and the Orc Archer carries a **two-entry table where the first hit wins** (`roll(10)<2` → 2 amulets, else `roll(10)<10` → 1) — pinned by a test forcing the first entry. `getRandom(0)` (Werewolf Chieftain) is an always-drop (`roll(0)` clamps to 0 < 1). Its `giveNewbieReward` is commented out in the dist (dead — see the newbie-reward note above), so omitted. quests_tests 117 → 119 (start→drops→adena-by-type→repeatable exit through the real router; max-level refusal); registration + max-level-gate sabotage-verified. **33 quests ported.** **Q00259 Request from the Farm Owner landed** — a Gludin spider hunt (level 15–21) with **two reward paths**: Edmond (30497) pays 25a per skin (+250 for 10+), or **Marius (30405) trades 10 skins for a batch of consumables** (Greater Healing Potions / arrows / soulshots / spiritshots — the player's pick). The skin drops **unrolled** (one per kill). Both paths tested through the real router (adena turn-in + repeatable exit; the Marius trade consuming 10 skins for 2 potions); registration sabotage-verified. **34 quests ported.** **Q00293 The Hidden Veins landed** — a **Dwarf-only** mining quest (Filaur 30535, level 6–15) with a **crafting** twist: kills drop Chrysolite Ore (5a) on `roll(100) > 50` or, rarely, a Torn Map Fragment on `< 5` (**one roll decides both**), and Chichirin (30539) **combines 4 fragments into a Hidden Ore Map worth 150a** — the fragment→map trade is the payoff. Filaur's turn-in page varies by what you hand in (ore-only/map-only/both). Race gate handled in `on_talk`'s CREATED branch (`player_race() != DWARF → 30535-01.htm`). 3 tests through the real router (full loop drops→craft→165a turn-in; race gate via Dwarf-vs-Human pages differing; max-level refusal); registration + craft-gate sabotage-verified. **35 quests ported.** **Q00300 Hunting Leto Lizardman landed** — a higher-level camp grind (Rath 30126, level 34–39): collect **60** Bracelets of Lizardman off the five Leto variants (per-mob drop chances **out of 1000** — 360/390/410/790/890), then a `getRandom(1000)` **reward fork** (50% 5000 adena, 25% 50 Animal Skin, 25% 50 Animal Bone). Repeatable. Two mechanics pinned: the cond flips to 2 at **exactly 60** (`== REQUIRED_BRACELET_COUNT`, not `>=`), and the reward branches are driven through repeatable re-runs with the roll forced. 3 tests through the real router (drop gate + exact-60 cond + adena; skin/bone forks; max-level refusal); registration + exact-60-trigger sabotage-verified. **36 quests ported.** **Q00296 Tarantula's Spider Silk landed** — a Gludin hunt (Trader Mion 30519, level 15–21) with a **converter** twist: kills yield Tarantula Spider Silk (5a) on `roll(100) > 45` or, rarely, a **Tarantula Spinnerette** on `> 95`, and **Defender Nathan (30548) spins each spinnerette into `15 + rnd(9)` Silk** — the spinnerette is the jackpot. Both drops use `giveItemRandomly` (rate-multiplied, [[l2r-quest-drop-helper-vs-handroll]]), so the port mirrors with `give_item_randomly` (chance 1.0, limit 0 → always gives 1×rate). 2 tests through the real router (drops→Nathan-spin→Mion-turn-in incl. the empty-converter branch; max-level refusal); registration + converter-multiplier sabotage-verified. **37 quests ported.** **Q00266 Pleas of Pixies landed** — Elf-only (Pixy Murika 31852, level 3–8): collect **100 Predator's Fangs** off the Keltir/wolf packs, then a weighted reward roll. Two quirks pinned: kills give a **variable amount** on a `getRandom(10)` threshold (per-mob `(threshold,count)` tables — e.g. the Gray Wolf's two-entry 2-or-3), and the reward roll is **inverted** — the 2% bucket hands the *cheapest* prize (Glass Shard + 100a) with the **jackpot** chime while the Emerald + 5000a is the 55% common case (ported faithfully; new `quest_sounds::JACKPOT`). **Test finding:** `addCondMaxLevel` blocks by preventing the start-npc talk from ever creating the quest state (`is_created()` is true for a stateless player, so the gate fires on first talk and `on_event`'s `has_qs` guard then finds nothing to start) — so a max-level refusal must be tested with a **fresh** player at the high level, not one leveled up after starting. 3 tests through the real router (drop-gate + limit-100 cond + inverted jackpot; reward buckets; race + fresh-level gates); registration + cond-trigger sabotage-verified. **38 quests ported.** **Q00271 Proof of Valor landed** — Orc-only (Rukain 30577, level 4–8): collect **50 Kasha Wolf Fangs** for the Necklace of Valor (+ a Healing Potion on a 13% roll). Two mechanics pinned: kills give a **25% double drop capped so it can't overshoot 50** (`roll(100) < 25 && count < 49 ? 2 : 1`), and the dialog **changes once you hold the necklace** (`30577-07`/`08` instead of `03`/`04`). `addCondMaxLevel` here refuses with a **specific page** (`30577-02.htm`), not the generic no-quest. 2 tests through the real router (loop: double-drop + cap + cond + potion reward; gates: non-Orc/necklace-held pages differ + fresh-level-9 refusal); registration + double-drop-cap sabotage-verified. **39 quests ported.** **Q00277 Gatekeeper's Offering landed** — a clean Greystone Golem hunt (Tamil 30576, level 15–21): collect **20 Starstones** (unrolled, capped) for **2 Gatekeeper Charms** (the teleport tokens). The distinct bit: the **min-level check lives in the start event** (`30576-03.htm` → `30576-01.htm` if under 15), not the talk, so a low-level player sees the intro and is refused only on clicking start. 2 tests through the real router (loop: cap + cond + charm reward + exit-clears-starstones; both level gates); registration + start-event-min-level sabotage-verified. **40 quests ported.** **Q00295 Dreaming of the Skies landed** — a Magical Weaver hunt (Arin 30536, level 11–15): collect **50 Floating Stones** (variable 1-or-2 amount via `giveItemRandomly`, 75%/25%, capped at 50) for the **Ring of Firefly** — but a **repeat run pays 200 adena** instead of a second ring (the `hasQuestItems(RING)` branch). 2 tests through the real router (both reward branches via re-runs; max-level refusal); registration + already-have-ring branch sabotage-verified. **41 quests ported.** **Q00262 Trade with the Ivory Tower landed** — a Gludio fungus hunt (Vollodos 30137, level 8–16): collect **10 Spore Sacs** for 300 adena. **A third rate convention:** the drop is `getRandom(10) < base * RATE_QUEST_DROP` (rate folded into the roll *threshold*, per-mob base 3/4), then `rewardItems` gives one (its own reward-rate applies to the amount) — neither the plain hand-roll nor `giveItemRandomly`, so a new `QuestCtx::rate_quest_drop()` accessor backs it. 2 tests through the real router (per-mob threshold distinction 3-vs-4 + cond + turn-in; max-level refusal); registration + threshold sabotage-verified. **42 quests ported.** **Q00267 Wrath of Verdure landed** — Elf-only (Treant Bremec 31853, level 4–9): a flat **50% Goblin Club drop** (hand-rolled `getRandom(10) < 5`, not rate-multiplied) traded for a trickle of adena. Two quirks kept: the odd **`2 + club count`** payout formula, and the **turn-in is separate from leaving** — handing clubs in (`31853-06`) keeps the quest running; a distinct `31853-07` event is the exit. 2 tests through the real router (drop + `2+count` turn-in-without-exit + separate leave; Elf/level gates); registration + adena-formula sabotage-verified. **43 quests ported.** **Batch of 7 simple hunt/collect quests landed** (Q00272 Wrath of Ancestors, Q00274 Skirmish with the Werewolves, Q00294 Covert Business, Q00297 Gatekeeper's Favor, Q00326 Vanquish Remnants, Q00328 Sense for Business, Q00331 Arrow of Vengeance) — all following established patterns: collect-N-→-reward, per-mob hand-rolled or `giveItemRandomly` drops, adena-by-type turn-ins, race/necklace gates, the ring-or-adena repeat branch, and the Q326 Black-Lion-Mark 100-badge milestone. One loop test each through the real router (7 tests, 143 → 150), all first-run green; the 7 registrations sabotage-verified as one (removing them fails exactly 7). **50 quests ported.** **Second batch of 4 simple quests landed** (Q00264 Keen Claws, Q00319 Scent of Death, Q00329 Curiosity of a Dwarf, Q00360 Plunder Their Supplies) — per-mob variable-amount drops, hand-rolled thresholds, and adena turn-ins. Java quirks kept faithfully: Q00264's **HashMap-iteration reward** (only `roll(17)` of 0→734+jackpot and 1→35 pay; item 735 is unreachable), Q00319's **cond-2-set-below-target** oddity + rate-free 500a, and Q00329's **inverted bonus** (<700 items pays the *larger* 1000a). One loop test each through the real router (150 → 154), first-run green; 4 registrations sabotage-verified as one. **54 quests ported.** **Third batch of 3 simple hunt/collect quests landed** (Q00369 Collector of Jewels — two-stage `memoState` collect of 100 then 400 fire/water elemental shards, `cond` mirroring the memo for the UI arrow; Q00619 Relics of the Old Empire — 1000 relics → one random S-grade weapon recipe, drops via **plain non-rate-multiplied `giveItems`** on kill (1–2 per, 50/50) plus a separate 10% Entrance-Pass roll where the pass is *not* a registered quest item so it survives turn-in; Q00623 The Finest Food — 100 each of three thermal-beast ingredients → a weighted 1000-slot reward table, the 940–999 slice paying nothing but still exiting). All three reduce Java's `getRandomPartyMember*` kill-credit selection to the killer (the established G11 party deviation). quests_tests 154 → 157 (kill→turn-in loops through the real bypass router; the `give_item_randomly` drop roll is `roll_f64`, forced via the same `forced_rolls` queue — missing that force made the first draft flaky); each sabotage-verified (Q00623's `set_cond` disable confirmed a red test); registrations sabotage-verified as one. **57 quests ported.** **Fourth batch of 3 simple quests landed** (Q00292 Brigands Sweep — Dwarf level 5–18, two turn-in NPCs, a goblin-token adena formula (6/8/10 each + 1000 for 10+) *plus* a rarer memo→contract sub-chain paid 100 at Spiron and 620 at Balanki; Q00276 Totem of the Hestui — Orc level 15–21, the **first quest to use `spawn_near_npc`** for `addSpawn`: a weighted parasite-hoard ladder (checked high→low, so a bigger hoard both unlocks and steepens the odds) conjures a Kasha Bear Totem whose kill drops the finishing crystal; Q00617 Gather the Flames — level 74+, three NPCs, Torches (1–2 per kill, plain non-rate `giveItems`) spent 1000 for a *random* recipe at Vulcan or 1200 for a *chosen* one at Rooney via id-string bypass events). Q00276's `giveNewbieReward` deferred with a `TODO(newbie-guide)` (active in this dist but GUIDE_MISSION has no reader). All three reduce Java's party kill-credit helpers to the killer (G11 deviation). quests_tests 157 → 160 (kill→spawn→turn-in loops through the real router; the totem found via `npcs_of`); Q00276's crystal→cond sabotage-verified; registrations sabotage-verified as one. **A process note:** a `sed`-based sabotage/revert left the incremental-test binary stale, producing a phantom cross-run failure that vanished on a forced rebuild — sabotage-verify via `Edit`, not `sed`, and rebuild before trusting a red. **60 quests ported.** **Fifth batch of 3 simple hunt/collect quests landed** (Q00358 Illegitimate Child of the Goddess — level 63–67, collect 108 Snake Scales → one random B-grade recipe; Q00354 Conquest of Alligator Island — level 38–49, per-mob Alligator Tooth drops plus a Nos Lad 1-or-2 double, turned in 400-at-a-time for 2000 adena via the `ADENA` bypass; Q00356 Dig Up the Sea of Spores — level 43–51, two spore types each capped at 100 from two mobs, `cond` tracking 2 = one kind full / 3 = both, a weighted `FINISH` adena roll). All three use `giveItemRandomly` with per-mob `f64` chances and reduce Java's `getRandomPartyMember*` selection to the killer (G11 deviation). quests_tests 160 → 163 (kill→turn-in loops through the real router); Q00356's cond-3 ladder sabotage-verified via `Edit`; registrations sabotage-verified as one. **63 quests ported.** **Sixth batch of 3 simple quests landed** (Q00355 Family Honor — level 36–49, Timak Orc Troops drop Galfredo Romer's Busts (20a each, or a 120a collector exit) on one `roll(1000)` band and rare Sculptor Beronas on the next, which Patrin appraises into one of four ancient statues on a weighted `roll(100)`; Q00622 Specialty Liquor Delivery — level 68+, a **pure 7-NPC delivery chain** (`cond` 1 → 7, no kills): Jeremy hands over 5 drinks, five bartenders in order swap each for a payment slip via `TALKERS.indexOf + 2` cond gating and dynamic `{npcId}-NN.html` pages, then Lietta pays a weighted reward; Q00688 Defeat the Elrokian Raiders — level 75+, Elroki drop Dinosaur Fang Necklaces via a `DROP_RATE * RateQuestDrop` rate-in-threshold, sold 3000a each or `donation`ed 100-at-a-time for a 50/50 450000/150000 jackpot). All reduce Java's party helpers to the killer (G11 deviation). quests_tests 163 → 166 (delivery/turn-in loops through the real router, each event fired at the correct NPC oid); Q00622's per-bartender cond advance sabotage-verified via `Edit`; registrations sabotage-verified as one. **66 quests ported.** **Seventh batch of 3 quests landed** (Q00110 To the Primeval Isle — level 75+, a one-time Anton→Marquez book delivery for 189208 adena + XP; its `addCondMinLevel(75, "")` refuses below level with an **empty html**, which `show_result` renders as nothing — ported as `start_condition_html → Some(String::new())` and tested with a level-70 player who can't create state; Q00628 Hunt of the Golden Ram — level 66+, a mercenary **rank machine** (100 Splinter Chitin → Recruit badge/cond 2, +100 Splinter +100 Needle → Soldier badge/cond 3), where `ItemChanceHolder.count` is again a **cond selector** — Splinter (1) drops from cond 1, Needle (2) only from cond 2 — tested by confirming a needle mob drops nothing at cond 1; Q00374 Whisper of Dreams Part 1 — level 56–66 via a two-sided `addCondLevel`, collect 360+360 for a chosen B-grade scroll/enchant + 9000a at `cond` 2 or 4, with a `cond`-3-gated 20% Sealed Mysterious Stone that Galman swaps for the Mysterious Stone that **opens Q00375 Part 2**). One loop test each through the real router (quests_tests 166 → 169, each event fired at the correct NPC oid); Q00628's soldier-rank transition sabotage-verified via `Edit`; registrations sabotage-verified as one. Q00110 keeps a Java quirk verbatim — Anton's post-start page points at `32113-06.html`, which the dist doesn't ship (retail 404s to a blank window). **69 quests ported; Q00375 is now unblocked.** **Eighth batch of 3 quests landed** (Q00306 Crystals of Fire and Ice — level 17–23, salamanders/undines drop Flame/Ice Shards at a `1000.0/count` chance which for every count (900–950) is **> 1.0**, so the shard drops on *every* kill — a datapack oddity kept verbatim; turned in for 15a each + 5000 for 10+; Q00127 Fishing Specialist's Request — level 20–75, a one-time Pierre→Ferma→Baikal→Pierre courier chain (`cond` 1 → 3, no kills, plus a `teleport_to` event) ending in a Fishing Rod Chest; Q00375 Whisper of Dreams Part 2 — **Part 1's payoff, now reachable**: the Mysterious Stone from Q00374 is consumed on the first Vanutu talk to enter, then collect 325 Karik Horns + 325 Limal Bloods for a chosen B-grade weapon reward + 9000a). Q00375 shares NPC 30938 with Q00374, but each has its own `html_dir` so the same-numbered pages don't collide. One loop test each through the real router (quests_tests 169 → 172, each event fired at the correct NPC oid); Q00375's cond-2 transition sabotage-verified via `Edit`; registrations sabotage-verified as one. **72 quests ported.** **Ninth batch of 3 quests landed** (Q00606 Battle against Varka Silenos + Q00612 Battle against Ketra Orcs — the two mirror-image level-74+ faction hunts: kill the enemy camp for Manes/Molars (per-mob `/1000` chance), turn 100 in for 20 alliance tokens (Horns/Seeds); structurally identical, ported as twins; Q00634 In Search of Fragments of Dimension — level 20+, started by *every* Dimensional Gate Keeper (`31095..=31194` minus the non-existing ids, built via `OnceLock`), an 80% Dimension-Fragment drop whose amount **scales with the killed mob's level** (`(int)(npc.getLevel()*0.15 + 2.6)`), no turn-in — the fragments are Rift currency). Added `QuestCtx::npc_level()` (reads the in-context NPC's template level) for Q00634's level-scaled drop. One loop test each through the real router (quests_tests 172 → 175); Q00634's level-scaled amount sabotage-verified via `Edit` (which also exercises the new `npc_level` helper); registrations sabotage-verified as one. **75 quests ported.** **Tenth batch of 3 quests landed** (Q00325 Grim Collector — level 15+, a three-NPC grave-robbing errand: Samed's Anatomy Diagram gates a per-mob **cumulative-threshold drop ladder** for organs/bones, Varsak assembles five bones into a Complete Skeleton at 80%, and Samed buys the lot — where **`hasQuestItems(getRegisteredItemIds())` requires ALL ten registered items** (confirmed against `AbstractScript.hasQuestItems`, which returns false on the first missing id), a demanding-but-shipped sell gate kept verbatim with a comment; Q00124 Meeting the Elroki — level 75+, the Primeval-Isle follow-up to Q00110, a pure 5-NPC dialog chain `cond` 1 → 6 ending in a Mantarasa Egg and Asamah's 100013-adena reward; Q00643 Rise and Fall of the Elroki Tribe — level 75+, collect Bones of a Plains Dinosaur (per-mob rate-in-threshold, 1–2 each) to sell at 1374 apiece or exchange 300 for 5 of a random B-grade weapon piece, keeping Java's **`isFirstTalk` per-server singleton flag** faithful via an `AtomicBool` on the script instance). One loop test each through the real router (quests_tests 175 → 178); Q00325's payout formula sabotage-verified via `Edit` (947 vs 948); registrations sabotage-verified as one. **78 quests ported.** **Q00111 Elrokian Hunter's Proof landed** (dedicated slice) — the deep Primeval-Isle chain that caps the Q00110 → Q00124 arc, a **12-step `memoState` machine** across Marquez/Mushika/Asamah/Kirikachin: gather 50 Diary Fragments, learn the Elroki flute, then hunt 10 each of three dinosaur trophies into a Practice Elrokian Trap redeemed for the real Trap + 100 Trap Stones + a ~1.7M-adena / ~20M-XP reward. **`memoState` (1–12) is the real progress axis and `cond` deliberately skips values** (several steps advance only the memo, so cond ≠ memo — e.g. memo 7→8 leaves cond at 7); the two collection stages key their drop off **`ItemChanceHolder.count == memoState`** (a stage selector, not a quantity — Diary count 4 drops at memo 4, the trophies' count 11 at memo 11), the same count-as-selector shape seen in Q00416/Q00628 but against the memo axis. Added `quest_sounds::ELROKI_SONG_FULL` (the client's `EtcSound.elcroki_song_full`, keeping the "elcroki" typo). One end-to-end test walks all twelve stages through the real router (quests_tests 178 → 179); the trophy-stage cond transition sabotage-verified via `Edit` (cond 10 vs 11). **79 quests ported.** **Q00373 Supplier of Reagents landed** (dedicated slice) — a self-contained alchemy minigame: Wesley (30166, level 57+) hands over a Mixing Stone + Manual, seven monster types drop reagent pouches and raw ingredients (a `Single` /1_000_000 roll or a `Pair` /1000 roll that picks item1 below one threshold, item2 up to a second), and Urn (31149) runs a **three-step mixing UI** — pick ingredient → catalyst → temperature — matched against a 15-row `FORMULAS` table into higher reagents. **The ingredient/catalyst choices ride in quest-state vars** (`st.set`/`getInt` → `set_var`/`get_int`) between dialog pages, and **the chosen item id is embedded in the page id** (`31149-03-XXXX`, parsed from `event[9..13]`); hotter `TEMPERATURES` yield more product at a lower success chance (temp 1 = 100%/×1, temp 2 = 45%/×2, temp 3 = 15%/×3). Java echoes any unhandled event page by default (`htmltext = event`), ported as a wildcard `Some(event)` arm. One test covers both drop shapes + the full pick→pick→mix→Dracoplasm flow (quests_tests 179 → 180); the formula table sabotage-verified via `Edit` (product swap). **80 quests ported.** **Q00344 1000 Years, the End of Lamentation landed** (dedicated slice) — a genuine Interlude collect/gamble/exchange quest: Gilmore (30754, level 48–52) buys Articles of Sacrifice off the Cave of Trials servants, but the turn-in is a **`roll(1000) >= count` gamble** — usually 60 adena each, but the more you hand in the likelier you instead draw one of four ancient relics, and `memoState` (1–4) routes each relic to its own Aden scholar (Kaien / Rodemai / Garvarentz / Orven), who trades it for a weighted-random B-grade prize before resetting to cond 1. Factored the four scholars' exchanges into one `exchange` helper (a `(bound /100, item, count)` table). One test covers drop → gamble → Old-Hilt relic → Kaien's Oriharukon reward → the adena fallback (quests_tests 180 → 181); the adena formula sabotage-verified via `Edit` (180 vs 183). **Note:** several nominally-unported dist quests are **later-chronicle content, deliberately not ported** — Q00500 (High Five: agathions, `QuestType.DAILY`, `ON_ATTACKABLE_KILL` listener, 70xxx items), Q00933/935 (daily + 90xxx items), the Q32–37 craftsman chain (level-85 / 36xxx items), Q00348 (Kamael-era, item 14857) — porting them would add dead code to an Interlude server. **81 quests ported.** **Q00235 Mimir's Elixir landed** (dedicated slice) — the A-grade enchant capstone and a **cross-quest chain link**: Ladd (30721, level 75+, needs a Star of Destiny from Fate's Whisper) brews Mimir's Elixir across `cond` 1 → 8, and **its first step consumes Pure Silver, the product of the just-ported Q00373 alchemy** (silver → forge True Gold via Joan + a Sage Stone drop → gather Blood Fire → mix at the Magister's Urn → elixir → Scroll: Enchant Weapon (A-Grade)). The `hasQuestItems(a, b, c…)`-is-AND semantics gate each mixing step. The cosmetic `MagicSkillUse 4339` mixing flash is a `TODO(cinematic)` (no broadcast-skill helper yet); the `SocialAction` victory pose uses the existing self-send helper. One end-to-end test walks all eight stages to the scroll (quests_tests 181 → 182); the final mix sabotage-verified via `Edit`. **82 quests ported.** **Q00222 Test of the Duelist landed** (dedicated slice) — the first **2nd-class-transfer proof** ported: Duelist Kaien (30623, level 39+, fighter classes) awards the **Mark of Duelist (2762)** that the village-master Gladiator/Warlord transfer consumes, so the Warrior 2nd occupation is now reachable in normal play. Two collection stages — 10-each of ten regional trophies (each gated on holding that region's Order), then 3-each of five tougher ones — factored into `stage1_mob`/`stage2_mob` tables + a `total()` helper. **The load-bearing quirk is an anti-shortcut kill counter**: `memoStateEx(1)` counts qualifying kills, and each stage completes only on a kill where the counter has passed a threshold (`>= 9` / `>= 5`) *and* every trophy is capped — so stockpiling the items another way can't finish it (the counter resets to 0 the instant a stage completes, so a completion at too-low a count silently strands progress). Reuses `memo_state_ex`/`set_memo_state_ex` from Q00417. One end-to-end test drives both stages (10 + 6 real kills via the inject-to-just-below-cap trick so the counter reaches its threshold on the completing kill) → cond 3 → Final Order → cond 5 → Mark (quests_tests 182 → 183); the stage-1 counter gate sabotage-verified via `Edit` (cond 2 vs 3). **Q00231 Test of the Maestro** (Warsmith 2nd-class proof, Artisan 56 → level-39 gate) — a hub quest of three *sequential* recommendation errands run through one `memoState` axis (Balanki 2 → Arin 3 → Filaur 4, each returning to hub 1 once its recommendation is collected), and the first ported quest to exercise the `on_timer` wire end-to-end: Toma's Broken-Teleport-Device errand teleports the player to Cruma and starts a 5 s `SPAWN_KING_BUGBEAR` timer that conjures three ambushers via the new `spawn_attacker_at(npc, x, y, z)` helper (fixed-location sibling of `spawn_attacker`). The test drives all three chains — Balanki's Evil Eye Lord → Necklace of Kamutu drop, Arin's teleport-device round-trip (advancing 50 ticks to fire the timer and asserting exactly three King Bugbears spawn), Filaur's collect-10-each antidote reagents (one real Giant Mist Leech kill, the rest injected since this chain has no kill counter) — to cond 2, then Lockirin awards the Mark of Maestro 2867 + 372154 adena (quests_tests 183 → 184). A `teleport_to` mid-quest tripped the bypass `INTERACTION_DISTANCE` guard on the next turn-in, so the test walks the player back to the NPCs before continuing; the `on_timer` spawn-count was sabotage-verified via `Edit` (2 vs 3 bugbears). **Q00223 Test of the Champion** (Warlord / Orc Raider 2nd-class proof, `WARRIOR`/`ORC_RAIDER`, level-39 gate) — a long linear letter-relay between four NPCs (Veteran Ascalon hub → Mason → Trader Groot → Captain Mouen) interleaved with four hunt legs, a pure item-gated state machine where every `cond` is cosmetic and each turn-in is gated on which letter/insignia the player holds (Java's `hasQuestItems` chain, ported as `quest_items_count(x) > 0`). First quest to lean on the `on_attack` ambush wire for real content: three legs (Bloody Axe Elite, Harpy, Road Scavenger) spring a `scriptValue`-gated first-hit ambush that conjures one or two quest monsters (`HARPY_MATRIARCH`/`ROAD_COLLECTOR`, or an extra elite) via `spawn_attacker`, and those quest monsters share the leg's `onKill` drop table. The full-flow test drives all six legs to cond 14 — collapsing the collect legs by injecting to the 9→10 / 27→30 boundary and taking the completing kill for real (one Harpy kill validates the insignia-gated 2-egg drop; one Windsus kill validates the all-three-at-30 → cond 7 gate) — and probes the Bloody-Axe `on_attack` ambush directly via `npc_receive_damage` with a forced roll (asserting a second elite spawns), then Ascalon awards the Mark of the Champion 3276 + 229764 adena (quests_tests 184 → 185). The ambush spawn was sabotage-verified via `Edit` (flipping its roll test so the forced roll no longer fires). **Q00224 Test of Sagittarius** (archer 2nd-class proof — `ROGUE`/`ELVEN_SCOUT`/`ASSASSIN`, level-39 gate) — a `memoState`-driven machine (states 1..14, most `onKill` legs guarded on `isMemoState(n)` rather than pure item-gates) relaying through Bernard → Hamil → Aron → Vokian → Gauen. Two mechanics carry the port: (1) the four Crescent-Moon-Bow materials (mithril clip / stakato chitin / reinforced bowstring / manashen's horn) each drop from their own mob but only advance to state 11 when the *other three* are already held, so the set completes in whatever order it is farmed; (2) **Serpent Demon Kadesh is gated on the killing-blow weapon** — while farming Blood of Lizardman the summon chance climbs with the stack (`((count-10)*5) > rnd(100)`, consuming the whole stack on the summon), and Kadesh only yields the Talisman of Kadesh if felled with the Crescent Moon Bow, else he simply respawns. Java reads `npc.getKillingBlowWeapon()` (a value captured on the NPC at death); we approximate with the killer's currently-equipped weapon at `onKill` time (`equipped_weapon_id()`), which is the finishing weapon in the common case — noted inline. The full-flow test drives all 14 states (collapsing the rune/material/blood legs with boundary injects and forced rolls), exercises both the summon-chance branch and the material-set ordering, and covers the weapon gate from **both** sides — a bare-handed kill that yields nothing and respawns him, then an `equip_weapon_row(CRESCENT_MOON_BOW)` kill that awards the Talisman — before Hamil hands over the Mark of Sagittarius 3293 + 161806 adena (quests_tests 185 → 186). The weapon gate was sabotage-verified via `Edit` (relaxing it to always-pass, caught by the bare-handed assertion). The Java 300000 ms Kadesh despawn timer is a `TODO(G22)` (cleanup only, no progression effect). **Q00225 Test of the Searcher** (scout / scavenger 2nd-class proof — `ROGUE`/`ELVEN_SCOUT`/`ASSASSIN`/`SCAVENGER`, level-39 gate) — the longest linear chain ported so far (cond 1..19): a detective errand relaying through Luther → Alex → Leirynn (Delu totems + Chief Kalkis's fang) → wine catalog / red spore dust → four torn map pieces → buried chest of gold → Alex's recommendation → Mark of the Searcher. Pure item-gate (`memoState` is set at accept but never read). Notable bits: (1) the two treasure-map halves each assemble from four torn pieces — Road Scavenger drops (and the 3→map conversion) are deterministic, the Hangman Tree ones are 50/50 rolls — and cond 15 only fires when the second half completes with the first already held, a cross-gate covered from both directions in the test; (2) the Ancient Tree conjures a Strong Wooden Chest beside itself and hands over a Rusted Key, which the chest trades for 20 Gold Bars before `deleteMe`; (3) a **deliberate Java copy-paste quirk kept faithfully** — the Delu Shaman `onKill` checks `RED_SPORE_DUST >= 10` (not totems) for its cond 4, and since Red Spore Dust comes from a far-later leg that cond never fires, the totem leg advancing instead at Leirynn's turn-in; the test asserts the totem kill leaves cond at 3. The full-flow test drives all 19 conds (collapsing collect legs with boundary injects + forced 50/50 rolls), exercises the onAttack Neer-Bodyguard ambush, and validates the tree-conjured chest; the map-piece 3→conversion was sabotage-verified via `Edit` (widening the threshold so Solt's Map never forms). The Java `getSummonedNpcCount() < 5` chest-spam guard is a `TODO(G22)` (the talk gate already blocks re-entry). **2nd-class proof chain — batch 1 of the 19-quest group (13 quests).** The class/race prerequisite quests that gate the 2nd occupation change, ported as one batch (branch `feat/second-class-chain`), each a full happy-path e2e test through completion plus a sabotage-verified key gate. Landed: **Trials** Q211 Challenger (WARRIOR_GROUP; chest jackpot gamble), Q212 Duty (KNIGHT_GROUP; escalating flag-counter spirit spawns, Old-Knight-Sword killing-blow gate, giveItemRandomly), Q213 Seeker (scout; two order-independent 4-piece ore sets), Q215 Pilgrim (HEAL_GROUP; 5000-adena buy/refund), Q216 Guildsman (Artisan/Scavenger; two supply chains, party-kill gates folded to killer per G11, recipe-craft rings supplied in test); **all three fitting Testimony** Q217 Trust (Human; memoState 1..19 diplomatic circuit, flat-33% Actea/Luell via Java's never-incremented flag var), Q218 Life (Elf; Talin's-Spear killing-blow on the Unicorn), Q219 Fate (Dark Elf; five-poison herb hunt + Red-Fairy-Dust/Blight-Sap alchemy); **five of seven Tests** Q226 Healer (WHITE_MAGIC; Tatoma ambush + four secret letters, self-deleting Mysterious Dark Elf), Q228 Magus (wizard; four elemental charm→gather→tone loops), Q229 Witchcraft (six Gems of Aklantoth, Sword-of-Binding bind of Zeruel — attack-to-flee then killing-blow), Q232 Lord (Orc Overlord; five-clan crafting), Q233 War Spirit (Orc; four-skeleton bone hunt). Reused idioms: `equipped_weapon_id()` approximates Java `getKillingBlowWeapon()` (noted inline at each site); category gates via `is_in_category`; the `on_attack` `script_value` ambush wire ([[l2r-trigger-skills]] neighbours); spawn count / `getSummonedNpcCount` caps and radar type-2/removeMarker left as `TODO(G22)` (cosmetic). **Deferred:** Q227 Reformer needs the skill id inside `on_attack` (Disrupt-Undead / magic-only kill gates) — a framework extension, so stubbing it would be a parity bug. **Remaining in the group (5, all 950-1265 lines):** Q214 Scholar, Q220 Glory, Q221 Prosperity, Q230 Summoner, Q234 Fate's Whisper. **2nd-class chain — batch 2 (2 quests): the remaining Testimony.** Q220 Testimony of Glory (Orc `ORC_2ND_GROUP`, cond 1..11) — subjugate five rival Orc clans for their Scepters (letters from Kasman/Manakia, gloves that summon each clan's champions), then bind the Revenant of the Tantos chief; the Ragna/Revenant `onAttack` is cosmetic chat over an unread `scriptValue` so it is not ported, and two Java copy-paste `==` sound checks (`TYRANT_TALON==29`, `MANASHEN_SHARD==19`) are kept faithful. Q221 Testimony of Prosperity (Dwarf `DWARF_2ND_GROUP`, cond 1..9) — four First-Ring proofs including the Old Account Book from a **five-guild contribution subsystem** (Shari/Mion+Maryse/Torocco/Bolter/Toma feed Spiron/Balanki/Keef/Filaur/Arin, each chief consuming its Lockirin notice before its giver hands over the contribution), then the recipe-crafted Key of Titan opens the Box of Titan for Maphr's Tablet Fragment. Both full-flow e2e + sabotage-verified. **Remaining in the group (3, the largest quests in the game):** Q214 Trial of the Scholar (1068 lines, three parallel Symbol sub-quests), Q230 Test of the Summoner (1265), Q234 Fate's Whisper; plus deferred Q227 Reformer (needs skill-id in `on_attack`). **2nd-class chain — batch 3 (1 quest): Q214 Trial of the Scholar** (mage trial, Wizard/Elven/Dark Wizard, level 35+). The longest linear chain in the whole group (cond 1..31): three Symbol sub-quests gated by Mirien's three sigils — Symbol of Sylvain (a Maria/Lucas/Creta letter-and-painting relay + Leto brown-scroll scraps), Symbol of Jurek (a monster-trophy hunt), and Symbol of Cronos (the four Scripture Chapters via Dieter/Edroc/Raut/Triff/Valkon/Poitan/Casian and a Grandis kill). Pure item-gate, full-flow e2e through all 31 conds + sabotage-verified. **This closes the batch of quests that fit the current framework: 16 of the 19 are ported (Q211-219, 226, 228-229, 232-233).** The remaining **3 are deferred pending framework extensions** (stubbing any would be a parity bug): Q227 Reformer needs the cast skill-id inside `on_attack` (Disrupt-Undead / magic-only kill gates); Q230 Test of the Summoner needs `isSummon` + servitor-vs-npc dueling in `on_attack` (its six arcana battles are entirely summon-vs-summon crystal races); Q234 Fate's Whisper needs templated-HTML returns for its B→A weapon-selection UI (`%weaponname%` substitution) and is level-75 raid endgame (Baium + three epic bosses). **`on_attack` skill/summon extension + Q227 Test of the Reformer.** Extended the quest attack notification to carry Java's `onAttack(npc, player, damage, isSummon, skill)` context, which two deferred quests needed. `notify_attack` now resolves the acting player (the attacker, or a servitor's owner via `ServitorOf`) and passes `skill_id: Option<i32>` + `is_summon: bool` into a `QuestCtx` that exposes them as `attack_skill_id()` / `attack_is_summon()`. The skill id rides on a transient `World.quest_attack_skill` the skill-damage path sets around `apply_physical_damage` (`None` on the melee path), threaded through `apply_skill_damage`; `is_summon` is computed at the notify site. With it, **Q227 Test of the Reformer** (Cleric / Shillien Oracle, level 39+, memoState 1..18) is now ported: its `onAttack` gates read the striking skill — the Nameless Revenant only yields its diary pages when first struck with **Disrupt Undead** (else a spoiling scriptValue 2, and a plain melee kill drops nothing), and the Crimson Werewolf **flees a melee/non-mage blow** but is credited to the mage who engages it. Full-flow e2e drives all 20 conds and asserts both the positive skill paths and the melee-negative paths; the Disrupt-Undead gate was sabotage-verified. **This unblocks 17 of the 19 2nd-class quests.** Still deferred: Q230 Test of the Summoner needs the *servitor-vs-npc dueling AI* on top of the now-available `is_summon` flag (its six arcana crystal races), and Q234 Fate's Whisper needs templated-HTML returns for its B→A weapon-selection UI (and is level-75 raid endgame). **152 quests ported; all 31 Sagas Q70-Q100 playable in-client (shared htmls) + finale AI + choreography/chatter + FX casts + timed companion taunt cadence (Saga block COMPLETE).** Servitor-battle primitives landed (attack_is_summon, owner_servitor, make_npc_attack, is_oid_dead, servitor-kill credit) with an arcana-duel round-trip test, unblocking the deferred Q230 full port. Q230 Test of the Summoner ported (servitor arcana duels: farm lists, Beginner Arcana turn-in, foul/victory duel loop over real servitor combat, six-arcana completion). Q234 Fate's Whisper ported (lvl-75 A-grade weapon quest: boss-chest materials, Pipette-on-Baium fill, Reorin templated B→A weapon-upgrade UI awarding the Star of Destiny). Q42/Q43/Q44 (Help the Uncle/Sister/Son) pet-ticket trio ported via a shared driver test. Formal Wear chain Q33-Q37 RESTORED to authentic Interlude (level 60 + Interlude materials, replacing the datapack's level-85 Grand Crusade version sourced from L2JServer_C6_Interlude); htmls de-GoD-ed. Q641 Attack Sailren + Q642 A Powerful Primeval Creature ported (repeatable Primeval Isle hunts; Q641 gated on unported Q126 via other_quest_completed). Name of Evil chain Q125+Q126 ported (Primeval Isle story: Kaimu letter puzzle + Warrior Grave 3-melody puzzle); Q126 completion un-gates Q641 Sailren. Q420 Little Wing ported (hatchling-pet quest: fairy/deluxe stone forge + onAttack shatter, 5 drake egg-farm paths, RNG Dragonflute reward). Q421 Little Wing's Big Adventure PORTED (hatchling→strider: pet_control_object_id/attack_is_summon/item_enchant_level QuestCtx accessors + DespawnNpc scheduled task for the 20-Guardian kill ambush; 4-tree memoState bitfield drink grind → Dragon Bugle). Q620 Four Goblets ported (Imperial Tomb: relic/box/goblet farm, full Sealed Box RNG loot table, brooch turn-in, tomb teleports). Q32 An Obvious Lie ported (Maximilian/Gentler/Miki errand: alligator herb farm + Spirit Ore/Thread/Suede turn-in for animal Ears). FIXME backlog fully cleared (Q421 ported). Ketra/Varka faction area completed: Q605 Alliance with Ketra Orcs + Q611 Alliance with Varka Silenos ported via a shared AllianceQuest engine (6-rank badge-turn-in ladder + mutual-exclusion; mirror pair, 1 engine + 2 data tables) — the core alliance quests the already-ported 606/612/617/619/622/623 depend on. Q350 Enhance Your Weapon ported (Soul Crystal SA system): new SoulCrystalData loader (LevelUpCrystalData.xml) + a new onSkillSee QuestScript hook (wired into skill-finish) + Absorbers NPC component with an absorb-below-half-HP gate + kill-driven crystal leveling; 102 of 124 leveling mobs require the Soul Crystal skill (2096) absorb. Standalone-quest batch ported: Q275 Dark Winged Spies (Orc, lv11-15 fang collection), Q370 An Elder Sows Seeds (lv28-42 spellbook-page/chapter exchange), Q640 The Zero Hour (lv66+ Stakato fang -> crafting-mat exchange; gated on Q109). Q327 Recover the Farmland ported (lv25-34, 5-NPC menu-driven: Turek Orc kills drop tokens+relic fragments; Piotur adena, Asha 5-fragment gamble, Iris XP buy-back, Nestle consumable trade). Q348 An Arrogant Search ported (lv60+ Seven Signs: Shell hunt → summon+slay Stone Watchman Ezekiel → White Cloth from Platinum Tribe/Angels → Blooded Fabric; linear cond 2-11; radar pings TODO). Q662 A Game of Cards ported (lv61 card-gambling minigame: chip drops, 50-chip stake, packed-int 5-card state machine + verbatim pair-scoring + card-cell HTML templating -> Ziggo Gemstone/mats). Q333 Hunt of the Black Lion ported (the datapack largest quest, ~1190L: 6-NPC mercenary guild — Sophya orders + 20-mob kill-drop table with boss ambushes, material->Lion Claw/Eye turn-in, Reedfoot cargo->trade-goods gamble + 20-way fortune, Morgon guild-coin, Rupio statue/tablet assembly, Black Lion Mark completion). ALL genuine-Interlude quests now ported (Q255 Tutorial deferred = needs tutorial-window subsystem). SAGA 3rd-class engine built (shared SagasSuperClass port: 20-cond ladder + class transfer) with Q70 Phoenix Knight as tested proof; authentic Interlude data from C6 (dist ships broken Classic versions). 30 more Sagas = data tables on the engine; htmls (index-named 0-01.htm..) still needed for playability. **Scoped out (need dedicated slices):** the ~30 Saga (Q70–Q100, 530-line 3rd-class certification + cinematics) and ~25 Trial/Testimony/Test (Q211–Q235, 400–1265-line 2nd-class chains); the tutorial (Q255, unported tutorial packets); Q00264 (fragile HashMap-iteration reward — item 735 unreachable); Q00275 (`addCreatureSeeId` + spawn framework); the daily/instance quests (Q00933/935 need daily-reset + High-Five items); and the raid/faction-alliance quests (Q00605/611 Ketra/Varka alliance state, Sailren, Four Goblets). Next: those chains, ~40 more simple quests, `ai/` scripts, the tutorial, `//reload` |
| Game  | G23 Grand bosses & raid bosses                              | ✅ boss zones/respawn/AI/persistence — `//grandboss` **Raid curse landed** (plan: [PLAN_G23_RAID_CURSE.md](PLAN_G23_RAID_CURSE.md)), G23's first slice. Checked before planning (the G20.5 lesson): **two of the gate's three clauses were already met** — `boss_respawn`, built during G21, covers scheduled respawn and `npc_respawns` persistence for all 225 `dbSave` spawns. Raid curse had **zero references**. An anti-farming rule: a player **more than 8 levels above** a raid boss is punished for interfering, which is why it fires on *helping* and not only on attacking. Skill 4215 (`Mute`+`PhysicalMute`, 3600 s) for a **good** skill cast nearby, 4515 (`BlockActions`, 120 s) for attacking or a **bad** skill — both already in the datapack with ported effects. Two sites: the damage hook sits **after** the damage block because Java's comment says *"in retail you deal damage to raid before curse"*, and the post-cast scan covers a high-level player buffing a low-level party from outside the fight, which the damage path never sees (the boss must be **in combat**, so travelling past an idle spawn is free). Raid **minions inherit** `giveRaidCurse` from their master. Boundary kept as Java's `> level + 8` with a test pinning that exactly 8 is not cursed. 7 tests incl. an end-to-end hit asserting the curse lands *and* the damage is dealt. **Raid points landed** (plan: [PLAN_G23_RAID_POINTS.md](PLAN_G23_RAID_POINTS.md)). Candidates measured before picking: **chaos target swaps struck** — `isChaos` has **zero occurrences** in this dist's NPC data, so the mechanic exists in Java and no NPC enables it here (same call as agathions/pet evolution); **minion waves already done** in G21; **raid points** real and unimplemented — 409 `<acquire raidPoints>` attributes, 374 of them non-zero. The distribution differs from the exp split in ways worth stating: points go to the **top damage dealer** (not proportionally), and if they are in a party the award splits among members **within `ALT_PARTY_RANGE` of the corpse** — including members who dealt no damage, excluding ones who hung back — with `max(points/size, 1)` so nobody rounds to zero. Raid **minions award nothing**. `CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL` is broadcast, not sent to the earner. `raidbossPoints` is an existing `characters` column, so persistence is one field and one bind — no new table and none of the shared-flush hazard from G29 slice 27. raid_curse_tests 7 → 13 + a datapack-backed parse check. **Gate met; scope audited.** All three gate clauses hold — *spawns on schedule* and *state persists* via `boss_respawn` (G21, 225 `dbSave` spawns), *applies raid curse* via slice 1. **`BossZone` does not exist on this chronicle**: no such class in the Java tree, no `type="BossZone"` in any zone file, no script reference — the roadmap's "boss zones + entry conditions" describes a generic L2J feature set, and on this dist entry gating lives **inside the grand-boss AI scripts** instead. So the honest remaining scope is those scripts: **10 under `ai/bosses/`** (Antharas 1056 lines, Baium 787, Valakas 581, QueenAnt 408, Orfen 384, Sailren 326, DrChaos 321, Core 232, Zaken 109; Frintezza absent) against **32 `GrandBoss` NPCs**. The port has `GrandBossManager`'s state (`grand_bosses` loaded at boot from `grandboss_data`) but it backs only the read-only `//grandboss` panel — no boss AI is ported. That is milestone-sized, not slice-sized, and is left explicit rather than tracked as a vague remainder; **QueenAnt is the natural first** (mid-size, already referenced by the raid-curse code, and the most commonly-run Interlude raid). **`ScriptZone` support landed** (plan: [PLAN_G23_SCRIPT_ZONES.md](PLAN_G23_SCRIPT_ZONES.md)) — groundwork every `ai/bosses` script needs, since each opens with `ZoneManager.getZoneById(…)` and none of that existed. A `ScriptZone` is behaviourally **nothing**: no `ZoneId` in Java, so no membership bit and no flag — it exists to be *addressed by id* (Queen Ant's lair is `getZoneById(12012)`). `ZoneKind::Script.bit()` is therefore 0, asserted, since giving it a bit would put everyone standing in one into a zone state nothing intends. Added the kind + `type="ScriptZone"` mapping (133 zones), kept `Zone.id` (previously discarded), and `ZoneData::by_id` + `Zone::contains`. **Adding a file for one kind must not change another**: `custom_script.xml` also ships a stray `SiegeZone` ("GainakSiege", later-chronicle, no `castleId`), and letting it through would set the Siege bit that `death.rs` reads as a **free-death zone** — dying there would silently skip the exp penalty — so the two script files are filtered to script zones only. Caught by the existing zone-census test: it failed on the count, and following *why* found the siege zone instead of just bumping the number. boss_zone_tests 4, all against real dist data; census updated to 1031 with the reason in the assertion. **Grand-boss respawn lifecycle landed** (plan: [PLAN_G23_GRANDBOSS_LIFECYCLE.md](PLAN_G23_GRANDBOSS_LIFECYCLE.md)) — ported **once** rather than ten times: every `ai/bosses` `onKill` marks the boss dead, rolls a respawn window, persists it and arms a timer, so that block lives in `game_loop/grand_boss.rs` driven by `GrandBoss.ini`, leaving each script only its interesting half. **The boot branch is the one that matters**: alive → spawn with stored HP; dead with a running timer → arm the remainder; dead with a window that **elapsed while the server was down** → spawn *now*. Miss that third case and the boss stays dead **forever**, since only a kill schedules a respawn and it can't be killed — its own test. Window is `(interval ± random)` hours, tested as a range **and** for actual variation (a single-value assertion passes on a broken fixed window); **Baium ships no `RandomOfBaiumSpawn`**, so its spread defaults to 0 rather than being assumed symmetric — pinned, since a copied default would give it a spread retail doesn't have. Respawning an already-alive boss is a no-op (a duplicate timer would stand up a second copy); stored HP of 0 means "never wounded", so a boss wounded before a restart comes back wounded. `StoreGrandBoss` is fire-and-forget rather than folded into the character flush — it has nothing to do with a character and would inherit that transaction's failure mode. grand_boss_tests 8, incl. the real `GrandBoss.ini`. **Queen Ant landed** (plan: [PLAN_G23_QUEEN_ANT.md](PLAN_G23_QUEEN_ANT.md)) — the first grand-boss script. **The fight is a priority rule**: six nurses heal, and they heal the **larva first**, so a party that leaves the larva alive fights a Queen whose healers are permanently busy — that ordering *is* the encounter. Larva gets `HEAL1` or `HEAL2` at random, the Queen only `HEAL1`. Java's "skip a nurse whose leader is the larva" branch **cannot fire here** — the larva declares no minions, only the Queen has nurses — so it is left out rather than written as dead code (same call as `EffectFlag.FEAR`/`MP_BLOCK`). Heals route through `npc_cast::start_cast` behind `check_use_conditions`, so a nurse pays the same MP and cooldown as any NPC rather than being a privileged script effect. The larva is script-spawned (not in the minion table). **A fixture made three tests vacuous**: the first draft wounded the Queen with an absolute `cur_hp = 10_000`, but `add_test_npc` gives every NPC 100 HP regardless of template, so that set HP *above* max and read as un-wounded — no heal was ever attempted. Found by instrumenting the cast; tests now wound by **fraction of max**. queen_ant_tests 6. **Core landed** (plan: [PLAN_G23_CORE.md](PLAN_G23_CORE.md)) — second boss, and with the shared lifecycle in place the whole slice was Core's own mechanics. **The finding: Core spawns 3 minions, not 19.** Java's `MINNION_SPAWNS` is a `Map<Integer, Location>` with **19 `put`s** (10 Death Knights, 5 Doom Wraiths, 4 Susceptors) keyed by **npc id**, so each type keeps only its *last* location and three entries survive. Plainly not what the author meant — the 19 coordinates are laid out around the lair — but it is what the server does, and porting the list faithfully would have given Core **six times the adds**. Ported as it behaves, with a test named for it: *port what the script does, not what it looks like it means*, the same principle as the dist data being authoritative. Minions respawn 60 s after dying **only while Core is alive** (a cleared lair stays cleared), and Core's death clears them after **20 s** rather than immediately — tested as *still standing right after the kill*, which an immediate despawn would fail. Barks deferred: `npc_say` lives on the quest context and isn't reachable from a boss script yet. core_boss_tests 6. **Orfen landed, and Zaken came free** (plan: [PLAN_G23_ORFEN.md](PLAN_G23_ORFEN.md)). **Zaken needed no script at all** — its 109 Java lines are entirely the spawn/respawn boilerplate slice 4 ported once (verified by grep: no `onAttack`/`onSpawn`/minions), so it is already driven by the shared lifecycle. One of the ten scripts turned out to be zero work. **Orfen's drag**: an attacker between **300 and 1000** units has a 1-in-10 chance per hit of being teleported *onto* Orfen and paralysed — the band is the mechanic, punishing ranged damage while melee is never dragged; both edges tested with the roll forced. **The half-HP relocation** fires **once per life**, not once per hit below the threshold (tested by moving Orfen and hitting it again), and Java's `if/else if` means it wins over the drag — a boss that just relocated shouldn't drag someone to where it no longer is. **Riba Iren heals on *its own* wounds**, not its master's — the opposite of every other healer minion (Queen Ant's nurses watch their target), so exactly what a port gets backwards by pattern-matching; both directions tested. **A vacuous assertion was hiding a broken fixture**: the first Riba Iren test asserted `Vitals.is_some()` (always true); replacing it with a real measurement made it fail and revealed the fixture had given `ORFEN_HEAL` the *paralysis* effect list. orfen_tests 8. **Boss-id audit** (plan: [PLAN_G23_BOSS_IDS.md](PLAN_G23_BOSS_IDS.md)) — fixes a defect introduced in slice 4 and found by running the reachability check before picking the next boss, not by anything failing. Slice 4 mapped **Antharas to 29019**; the script uses `ANTHARAS = 29068` (the "strong" variant) and `grandboss_data` has a row for 29068 and **none** for 29019 — so Antharas's respawn window never resolved and **it would have died and never come back**. Silent, because 29019 is a valid NPC template: the id looks right in isolation and is only wrong against the boss table. The table ships 8 rows here (Queen Ant, Core, Orfen, Baium, Zaken, Valakas, Antharas **29068**, and 25512 Gigantic Chaos Golem = DrChaos's second form); **Sailren (29065) has no row**, so it isn't a tracked grand boss on this dist. New test cross-checks the config against the real table **in both directions** and pins that 29019 must *not* resolve — a one-sided check wouldn't have caught it. Lesson: run the reachability check even when you already know what you're building next. **Boss barks landed** (plan: [PLAN_G23_BOSS_BARKS.md](PLAN_G23_BOSS_BARKS.md)) — the blocker was **one function**: `npc_say` was a `QuestCtx` method, but its body only ever needed the world and the speaker, so the quest coupling was incidental. Lifted to `helpers::npc_say` with `QuestCtx` delegating; all 113 quest tests pass unchanged. *A helper that lives in one subsystem because that's where it was first needed is not the same as one that depends on it — check the body before assuming a port is blocked.* Core now speaks: the two intro lines on the **first hit of a life**, a 1-in-100 "Removing intruders" taunt after, and two death lines. **The intro resets on death** (`_firstAttacked = false` in `onKill`) — without it a Core killed once stays silent for the lifetime of the process, invisible in testing and obvious to players. core_boss_tests 6 → 10, counting `NpcSay` (0x30) on a real client channel so the assertions measure packets sent rather than a flag. **Valakas attack rules landed** (plan: [PLAN_G23_VALAKAS.md](PLAN_G23_VALAKAS.md)) — the first boss with the **four-state ladder** (DORMANT/WAITING/FIGHTING/DEAD) rather than the ALIVE/DEAD pair; only the `onAttack` half is ported, with the lair entry flow and 30-minute window stated as their own slice rather than left implied. **Attacking from outside the lair kills you** — Java's `attacker.doDie(attacker)`, a hard anti-exploit against plinking from safety, self-inflicted so it carries no PvP or karma consequence. **The order is the mechanic**: the zone check precedes the status check, so an out-of-zone attacker dies *whatever* the boss's status — including while Valakas is dead, when the status branch would merely have teleported them; its own test, since that is the half a reordering silently loses. Strider riders are debuffed **once**, not every swing. Zone 12010 is a `ScriptZone` — the first script to consume slice 3's loader work. Also added a **fixture guard** asserting the tests' lair coordinate really is inside the zone: without it every "inside" test would silently exercise the outside path and still pass. valakas_tests 5. **Baium landed** (plan: [PLAN_G23_BAIUM.md](PLAN_G23_BAIUM.md)) — **chosen by counting cinematics**. Valakas's entry flow was next on the list, but it is **19 `SpecialCamera` calls** and the camera packet isn't ported, so most of that slice would be stubs; Antharas has 7 and **Baium has 0**, making it the only one of the three great bosses portable now. One grep changed which slice was worth doing. Landed: **five archangels** at fixed points (not in a minion table — the script places them) and the **anti-strider debuff** cast *once* (`!isAffectedBySkill(4258)`), tested by draining the client channel and asserting a second hit starts no new cast. **Deliberately not ported**: Baium's targeting is a **top-3 threat table** on NPC variables fed by a weighting that shifts as he is worn down — melee is `damage × 1000` while a caster at full health is `(damage/3) × 20`, so **melee threat is worth fifty times a caster's**, and the caster weighting swings tenfold across the HP bands. Folding that into the ordinary aggro list would look like it worked and would not be Baium, so it is its own slice with the table written down. baium_tests 4. **Baium's threat table landed** (plan: [PLAN_G23_BAIUM_THREAT.md](PLAN_G23_BAIUM_THREAT.md)) — the piece slice 11 deliberately left out, ported rather than approximated onto the aggro list. Baium keeps a **top-3 table** fed by an HP-banded weighting: a 300-damage hit scores **300 000** in melee but **2 000** from a caster at full health (**150×**), and the caster figure climbs to 10 000 below 25% — so Baium fixates on melee, and a caster beneath notice early becomes a real target as he weakens. Both asserted as **relationships** (a ratio, an ordered progression across the four bands) rather than four magic numbers, so a mis-ported band shows as the wrong shape. Two behaviours easy to flatten into "set the value": an existing entry is raised **only** when below `aggro + 1000` (so small hits don't ratchet a threat upward), and a newcomer displaces the **weakest** slot — not the oldest, and not nobody. Jitter forced to 0 so the ladder alone decides. baium_tests 4 → 8. **Baium's skill selection landed — Baium is complete** (plan: [PLAN_G23_BAIUM_SKILLS.md](PLAN_G23_BAIUM_SKILLS.md)). Two mechanics beyond "pick a skill": **the rotation** — after acting, the top threat is knocked down to **500** seventy percent of the time, which is what stops Baium tunnelling the biggest damage dealer all fight and lets the next player take a turn; and **the widening pool** — two options above 75% HP, three above 50%, four below 25%, each an independent 10% roll taken *in order* with the basic attack as fallback, so his repertoire opens up as the fight goes on (the same shape as his threat weighting). **Pruning is targeting, not tidiness**: a threat whose attacker died or fled beyond 9000 units is zeroed, which can change who he attacks — a test puts the top two threats on a corpse and a runaway and asserts he turns on the third, lowest attacker. baium_tests 8 → 14, rolls forced so each test isolates one decision; the band test asserts the **first option of each band** rather than "some skill", which is what distinguishes an ordered ladder from four skills in a bag. **`SpecialCamera` (0xD6) landed** (plan: [PLAN_G23_SPECIAL_CAMERA.md](PLAN_G23_SPECIAL_CAMERA.md)) — the blocker named at slice 11, unblocking Valakas's entry flow (19 uses) and Antharas (7). **`range` is accepted and never written**: Java's canonical constructor takes twelve parameters, assigns eleven, and drops `range`, so the wire carries eleven ints. The port keeps the parameter as `_range` so call sites transcribe Java's argument list literally — removing it would shift every following argument at 26 call sites of unlabelled integers, the worst place for a silent off-by-one. The test asserts the field after `time` is **duration, not range**, which is exactly the corruption a "helpful" serialisation would cause. Java's 11-arg overload additionally forwards `duration` and `range` into each other's slots (a caller's range is written as the duration) — **not reproduced, because no boss script uses it**; all 26 call sites take the 12-arg form, checked rather than assumed. special_camera_tests 2, including Valakas's opening shot transcribed from the script and checked field by field. **Valakas's entry cinematic landed** (plan: [PLAN_G23_VALAKAS_CINEMATIC.md](PLAN_G23_VALAKAS_CINEMATIC.md)), the first thing `SpecialCamera` unblocked. Ten beats **scheduled up front from the start of the sequence**, as Java does, rather than each chaining the next — deliberate, because **the beats are unevenly spaced** (330 ms between steps 5 and 6, 6.7 s between 8 and 9) and a relative chain would be easy to get subtly wrong in a way visible only as a cinematic that felt off; a test pins the 26-second span and that the beats occupy distinct ticks. The tenth beat carries no camera — it flips the status to `FIGHTING`, which starts the fight and locks entry. The camera table is transcribed in Java's argument order, `range` included even though the wire drops it, which is exactly why slice 14 kept that parameter — the two tables diff by eye. **It plays for the lair, not the neighbourhood**: tested with one player inside and one outside, since the ordinary region broadcast would pass a weaker test and show the cinematic to bystanders. valakas_tests 5 → 10, plus a `pending_ticks_for_test` scheduler hook for asserting a sequence's *shape*. **Antharas's minion waves landed** (plan: [PLAN_G23_ANTHARAS.md](PLAN_G23_ANTHARAS.md)) — the last boss script opened, with the mechanic that defines the fight. Adds arrive every five minutes in **growing waves**: the multiplier starts at 1 and climbs on ~89% of waves to a ceiling of 4 (one pair → four pairs), with the lair capped near 100. The spawn ladder is cap-aware and its steps are **not** interchangeable — **step 3 is the one worth having**: at 98 minions Antharas adds a *single, randomly chosen* dragon rather than skipping the wave, so the lair fills to exactly 99; collapsing it to "a pair if there's room for two" reads equivalent, caps the fight two adds early and would never be noticed. **A deliberate divergence, documented**: Java keeps `_minionCount`/`minionMultipler` as script *statics*, the port puts them on the boss as a component — two Antharas instances sharing one counter is a bug waiting to happen and nothing in the Java relies on the sharing. antharas_tests 6, incl. a full lair spawning nothing while still rearming so the fight recovers as adds die. Still open for Antharas: the entry cinematic (7 shots), the Heart of Warding entry gate, the 200-player cap, `manageSkills`. **Antharas's entry cinematic landed** (plan: [PLAN_G23_ANTHARAS_CINEMATIC.md](PLAN_G23_ANTHARAS_CINEMATIC.md)). **Antharas chains; Valakas batches** — the obvious move was to reuse slice 15's table, and it would have been wrong: Valakas arms all ten beats up front, Antharas has each beat schedule the next with a *relative* delay. Reshaping one into the other silently changes the timing model, so a test asserts exactly **one** cinematic timer is pending after the start, which is what distinguishes a chain from a batch. **`CAMERA_3` forks** — it roars, arms the next beat *and* a second social 5.2 s later, the only beat arming two timers, which a uniform "each beat arms the next" port drops entirely. The tail (`START_MOVE`) now **starts the minion waves**, moved off spawn from slice 16, so an un-engaged boss isn't already producing adds. **A vacuous assertion was caught on review**: the fork test first read `assert!(drain(...) > 0  **[Verified 2026-07-31]** — all three gate clauses are met and pinned: a boss **spawns on schedule** (`boss_respawn_tests`: respawn-timer honoured, elapsed timer spawns immediately), **raid curse applies** (19 curse/raid-point tests), and **state persists** (`DbCommand::StoreNpcRespawn` on death and respawn, asserted in `boss_respawn_tests`). All 10 grand/raid bosses are ported. 5 `TODO(G23)`s remain, all small (Valakas music variant, cube despawn task, the `showPkDenyChatWindow` reputation gate).|| true)` — passing unconditionally, and against the wrong opcode; replaced with an exact count against `SocialAction` (0x27). A passing suite says nothing about assertions that cannot fail. antharas_tests 6 → 12. **Antharas's entry gate landed** (plan: [PLAN_G23_ANTHARAS_GATE.md](PLAN_G23_ANTHARAS_GATE.md)) — the Heart of Warding's ladder. **Order is the user experience**: the boss's state is checked *before* the ticket, so a player without a Portal Stone arriving at a dead Antharas is told "Antharas is dead", not "you need a stone" — tested with an empty inventory so a reordering shows as the wrong message. Two rungs easy to lose: **only the leader may bring a group in** (and for a command channel it's the *channel* leader, so a party leader inside a CC is refused), and **the whole group must fit** — `members > MAX_PEOPLE - inside` refuses outright rather than admitting as many as will fit, so a raid isn't split in half by the doorway. Only members gathered within 1000 units come along. **A branch no test could reach**: the first overfill test admitted in its own comment that it asserted something else, since filling a 200-player lair in a unit test is impractical — so the ladder was split to take occupancy as a parameter, and the rung is now tested from both sides (199 inside refuses a party of two, 198 admits). *A test named after an unreachable branch is worse than none, because it reads as coverage.* antharas_tests 12 → 19. **Antharas's skill ladder + the caller neither boss had** (plan: [PLAN_G23_ANTHARAS_SKILLS.md](PLAN_G23_ANTHARAS_SKILLS.md)). **`baium::manage_skills` had no caller anywhere in the crate** — written, documented and tested in slice 12, and Baium chose skills into the void and only ever swung, for seven slices, while the plan doc said "Baium is complete". The [[regen-stat-pipeline]] shape one level up: not a stat pumped but never read, a whole *procedure* that is correct, covered and unreachable — and **being well-tested is what hid it**, since a unit test calls the function directly and passes exactly as it would if wired. What finds this is `cargo build`'s dead-code warning and reading the entry point, not the unit. Both bosses now run from `onAttack`. **The threat table was duplicated** — Antharas's `refreshAiParams` is identical to Baium's line for line, six slices apart; extracted to `boss_threat.rs`. **The tail sweep's angle is absolute**: Java gates it on `calculateDirectionTo` with no heading term, so "within 8° of 180°" means the target is due *west*, not behind him — every other cone check in the codebase subtracts the heading first. Ported as written and pinned by a test (target west, Antharas facing east), because "correcting" it is a behaviour change dressed as a fix. The ladder is a **chain of else-if**, so its percentages are conditional; four bands widen as he weakens, and the Breath Attack is the only skill that *opens* a band (rolled first, 30%, below 25% HP). `castOnTarget == false` = cast on **himself** — the areas are centred on the boss. Two Baium tests changed because choosing decays the threat it just read, immediately, which is Java's real order. antharas_tests 19 → 27, baium_tests 14 → 15; both hook tests verified failing on the previous commit. **The entry flow is wired — slice 18's defect closed** (plan: [PLAN_G23_ANTHARAS_ENTRY.md](PLAN_G23_ANTHARAS_ENTRY.md)): `scripts::antharas_heart` registers the **Antharas** quest script (the name is load-bearing — the dist htmls already say `Quest Antharas enter`/`teleportOut`), so the Heart of Warding 13001 serves `13001.html`, the ladder's five verdicts map to the five refusal pages, an admitted group teleports to `(179700+rnd, 113800+rnd, −7709)` and the **first** admission flips WAITING and arms `SPAWN_ANTHARAS` at `AntharasWaitTime` minutes (20 on this dist; a second party mid-window must not restart the clock — tested by entering at half-window and asserting the boss arrives on the FIRST deadline). `SPAWN_ANTHARAS` relocates the boss to the platform via a new **`relocate_npc`** (Orfen's in-place `Position` mutation is region-local; this re-indexes `npc_regions`, `DeleteObject`s the old cell and re-introduces — the cross-region move is asserted from both cells), flips IN_FIGHT, plays `BS02_A` to the lair and starts slice 17's camera chain (whose tail starts the waves) — the spawn-time cinematic stand-in is removed (Valakas's stays unwired until its entry slice, TODO(G23) at the site). **The wiring exposed a status collision**: `on_grand_boss_killed` wrote the two-state `DEAD = 1` for every boss, which the four-state ladder reads as *WAITING* — `try_enter` would have admitted raids into a dead Antharas's lair; `dead_status(boss_id)` (3 for Antharas/Valakas) now feeds the kill and both boot branches, pinned from both sides (a killed Antharas reads 3 and refuses entry; an elapsed window still respawns him; Core keeps 1). And the test that would have caught slices 12 and 18: **the `enter` bypass runs through the real router** (`handle_request_bypass_to_server` → registered script → ladder), where a direct `heart_enter` call would pass with the script unregistered — it also caught the bypass distance guard correctly refusing `teleportOut` aimed at the 180k-units-away Heart (the cubic is its own NPC inside the nest). antharas_tests 27 → 32. Antharas still open: the five invisible clear-NPCs + `CUBE` spawn on kill, the 5-minute `MANAGE_SKILL` cadence vs the port's on-attack hook. **Valakas's entry flow wired** (plan: [PLAN_G23_VALAKAS_ENTRY.md](PLAN_G23_VALAKAS_ENTRY.md)) — slice 15's 10-beat cinematic (`begin_cinematic`) was another **complete, tested, uncalled** `"beginning"` handler; `scripts::valakas_teleporters` (the `ai/others/ValakasTeleporters` chain) is the caller. Six NPCs route through the bare `Quest ValakasTeleporters` bypass (→ `on_talk`): **Klein 31540** shows a crowding html by lifetime count and, on its `31540` sub-event, does the **Vacualite-gated** (7267) Hall of Flames teleport + `allowEnter` grant; **Heart of Volcano 31385** is the lair door — refuses while fighting/dead/full/flagless, else consumes the flag, teleports in and, **on the first (DORMANT) entry only**, arms `"beginning"` at `ValakasWaitTime` (30 min) and flips WAITING; **cubic 31759** exits; the **gatekeepers 31384/31686/31687** open doors 24210004-6 (new `doors::open_door_by_id`, since door oids are dynamic). `"beginning"` (new `ValakasBeginning` timer, guarded on still-WAITING) runs `begin_cinematic`, whose final beat flips FIGHTING. **The count quirk ported faithfully**: Java's `playerCount` is a `static int` that **only increments — never resets** on spawn/death/window (after 200 lifetime entries the lair locks until restart); stored as `World.valakas_entry_count`, pinned by a test that a kill+respawn keeps counting. Both subtle wires sabotage-verified (arm-on-every-entry fails the once-only test; unregistering the script fails the router e2e). valakas_tests 10 → 16. **Dr. Chaos landed — the last `ai/bosses` script** (plan: [PLAN_G23_DR_CHAOS.md](PLAN_G23_DR_CHAOS.md)). The Gigantic Chaos Golem 25512 had a `grandboss_data` row but zero AI. **The encounter is the paranoia**: Dr. Chaos (32033) is a small NPC whose `pissed_off` timer starts at 30 and drains 1 per nearby living player per second (1–5 more per talk); at 15 he barks a warning, at ≤0 he **becomes** the golem through a 5-beat cinematic. So lingering near him *is* what spawns the boss. The golem carries **no config respawn window**, so the shared lifecycle skips 25512 entirely — this module owns its status (a third ladder: `NORMAL 0`/`CRAZY 1`/`DEAD 2`), boot (CRAZY restores the golem with stored HP; DEAD arms the reset or, if elapsed while down, respawns Dr. Chaos now — the downtime trap again), the **30-idle-minute despawn** (revert to Dr. Chaos; an attack refreshes the clock), and the `(36 ± 24)h` kill window. Barks needed a **literal-text `NpcSay`** (`npcString = -1` + the string) — the existing builder only did client-localized string ids, but Dr. Chaos's lines are literal English. **The slice-20 lesson applied preemptively**: the kill test drives `npc_do_die` end to end (a direct `on_golem_killed` call passes even with the `death.rs` hook unwired — sabotage-verified), and the transform is verified through `handle_paranoia`. Faithful timing detail pinned: Dr. Chaos **lingers through the 17 s cinematic** and is deleted only on beat 5 (the golem replaces him then, not on trigger). dr_chaos_tests 9. **Every `ai/bosses` script is now ported.** **Antharas death tail landed** (plan: [PLAN_G23_ANTHARAS_DEATH_TAIL.md](PLAN_G23_ANTHARAS_DEATH_TAIL.md)) — the missing `onKill` half left players **stranded in the lair** after the kill (no exit). Now `death::npc_do_die` runs `antharas::on_antharas_killed`: `DESPAWN_MINIONS` (every Behemoth/Terasque in the nest), the death `SpecialCamera`+`PlaySound("BS01_D")`, spawn the exit cube 31859 at `(177615,114941,-7709)`, and arm `AntharasClearZone` at +15 min — which teleports lingering players to the Giran-side exit and despawns every NPC left in the nest (cube included). The cube's `teleportOut` talk was already wired (`AntharasHeart` lists 31859), so the loop closes. **Bug found + fixed**: `LAIR_ZONE_ID` was `12016` (a Talking Island `ScriptZone`), not the Antharas Nest — Java's `getZoneById(70050, NoRestartZone.class)` (`antaras_no_restart`); the wrong id was latent because its only reader (`players_in_lair` occupancy) **fails open** (empty zone = nobody inside), so the `MAX_PEOPLE` gate silently never tripped. antharas_tests 32 → 35 (kill→cube+minion-clear via the real death path, cube `teleportOut` through the router, `CLEAR_ZONE` oust+despawn through the loop dispatch); both new wires sabotage-verified. **Valakas death tail landed** (plan: [PLAN_G23_VALAKAS_DEATH_TAIL.md](PLAN_G23_VALAKAS_DEATH_TAIL.md)) — the symmetric counterpart. `death::npc_do_die` runs `valakas::on_valakas_killed`: the death sound + opening `SpecialCamera`, then the **eight-beat `die_1..die_8` death cinematic** scheduled up front from the kill (the entry cinematic's batch model), whose eighth beat drops the **fifteen** exit cubes 31759 at `TELEPORT_CUBE_LOCATIONS` and arms `ValakasRemovePlayers` at +15 min → `oustAllPlayers` teleports lingering players to `LAIR_EXIT`. The cube's `teleportOut` was already routed by `scripts::valakas_teleporters`, so the loop closes. `BOSS_ZONE_ID = 12010` was already correct (fixture-guarded — no repeat of the Antharas zone-id bug). New `ScheduledTask::ValakasDeathCinematic`/`ValakasRemovePlayers`. valakas_tests 15 → 18 (kill arms the cinematic via the real death path, `die_8` spawns all 15 cubes + arms remove_players through the loop, remove_players ousts through the loop); all three wires sabotage-verified. **Both grand-boss death tails now done.** **QueenAnt AI completed** — beyond the larva + nurse-heal rotation (already ported), her three missing behaviors landed: the **larva is now immobilized + undying** (`AdminFlags.paralyzed`/`undying` on spawn — you cannot kill or move the healing-sink larva, which is the fight), it is **removed when the Queen dies** (a per-boss tail in `on_grand_boss_killed`), and the **`DISTANCE_CHECK` leash** (`ScheduledTask::QueenAntDistanceCheck`, 5 s) resets her — drops hate + walks home via `move_npc_to` — when dragged >2000 from her anchor. queen_ant_tests 6 → 9 (undying/immobilized flags, larva cleanup on death, and the leash firing far / sparing near — the last two sabotage-verified). **Orfen leash landed** — her drag, half-HP relocation and Riba-Iren self-heal were already ported; the missing `DISTANCE_CHECK` anti-drag now landed (`ScriptZone` audit found the Raikel Leos minions are in Orfen's NPC minion table, so the generic system already spawns/respawns them). New `on_orfen_spawned` (wired into the grand-boss spawn dispatch) records her spawn anchor on `OrfenState.home` and arms `ScheduledTask::OrfenDistanceCheck` (5 s); dragged >10000 from her anchor she drops hate + walks back (`move_npc_to`). orfen_tests +1 (leash fires far / spares near, sabotage-verified). Deferred (need new plumbing, documented): the onSkillSee "buff near Orfen → dragged in" punish (skill-see only reaches QuestScripts, not native bosses), onFactionCall minion-assist (Raikel Leos BLOW + Riba call-heal), the `check_orfen_pos` reposition, and the drag shouts. **Core immobilization landed** — Core's minions (the "19-that-are-3"), intro/taunt/death lines and 60 s minion respawn / 20 s despawn were already ported; the one gap was `onSpawn → setImmobilized(true)`. Added a **movement-only** `Immobilized` marker component (folded into `abnormal::is_movement_disabled`, distinct from `AdminFlags.paralyzed` which also blocks *actions*): `on_core_spawned` now roots Core so it melees adjacent attackers (FIST range 40) but never chases. core_boss_tests +1 (Core is movement-disabled yet not control-blocked, sabotage-verified). **Zaken** complete (2026-07-28) — its Java script is pure spawn/respawn lifecycle (its own comments TODO the AI), covered by the generic grand-boss window (29022 in the config) + `boss_respawn`; the one ported artifact was its pair of stock roars, now broadcast from the shared lifecycle (`grand_boss::roar`): `BS01_A` on spawn, `BS02_D` on death, positioned `PlaySound` anchored to the boss, gated to the four simple scripts (Queen Ant, Core, Orfen, Zaken — the cinematic bosses voice themselves), which also un-mutes Queen Ant/Core/Orfen's spawns and deaths. **Baium archangels landed** — Baium's combat (archangel spawn, strider debuff, threat table + skill ladder) was ported; the Archangels (29021, passive `Monster`s with no aggro range) never engaged. Added the Java `SELECT_TARGET` beat (`ScheduledTask::BaiumSelectTarget`, 5 s, armed at `on_baium_spawned`): each archangel keeps a living player it already hates, else `add_hate`s the nearest player in reach (1000), else regroups on Baium (`move_npc_to`); when Baium falls they despawn and the beat stops. baium_tests +2 (an archangel engages a nearby player — sabotage-verified; archangels despawn when Baium dies). Baium's stone→live **awakening cinematic + status machine** (wakeUp → earthquake → port-and-kill the waker → status ALIVE/IN_FIGHT/DEAD, CHECK_ATTACK decay) remain a larger multi-slice effort. **Sailren wave ladder landed** (fresh port — no prior module) — the dinosaur-summon raid (`game_loop/sailren.rs`): three Velociraptors (22218) → Pterosaur (22199) → Tyrannosaurus (22217) → Sailren (29065), each rung spawning the next on death. The chain is **stateless** (counts living tagged mobs, not a kill counter) and scoped by a `SailrenWaveMob` marker component so a random dinosaur kill in the open world doesn't advance it (the death hook checks the marker before routing to `on_wave_kill`). The last Velociraptor summons the Pterosaur (`add_hate`-ing the killer); Pterosaur → Trex; Trex arms `ScheduledTask::SailrenSpawn` (3 min); Sailren enters invulnerable + immobilized (`AdminFlags.invul` + the movement-only `Immobilized`) for the intro, then `SailrenAttackEnable` (24.6 s) lifts both; felling Sailren drops the exit cube. sailren_tests +3 (the wave climbs to Sailren — the raptor-clear gate sabotage-verified; the intro invul→fight transition; the exit cube). **Sailren Statue entry landed (slice 2)** — `scripts/sailren_altar.rs` (the Statue 32109 + exit cube 32107): the leader's `Quest Sailren enter` runs `sailren::entry_refusal` (party/in-fight/leader/Gazkh gates -> the matching 32109-0x.html), then takes the Gazkh (8784) and `enter_party` teleports the leader's nearby party members to the nest (gathering them *before* teleporting the leader, so moving the reference point doesn't strand the rest) and arms the first Velociraptor wave 60 s out (`ScheduledTask::SailrenBeginFight`); the cube teleports survivors to Rune. Concurrency is gated on live `SailrenWaveMob`s -- the marker doubles as the IN_FIGHT status, so no global state is needed. sailren_tests +2 (solo refusal sabotage-verified; a party is teleported in + the wave armed). **DrChaos verified fully ported** (paranoia drain -> transformation cinematic -> golem idle-despawn -> all hooks). Deferred: the zone time-out/decay + respawn lock. Remaining G23 tail: the 5-min `MANAGE_SKILL` cadence -- cosmetic/secondary; the big cinematic entry flows (Baium awakening; Antharas/Valakas modules exist). **Valakas `regen_task` landed:** the 60 s recovery beat (`game_loop/valakas.rs`), armed at the end of the entry cinematic and re-armed each tick while FIGHTING. Two halves, Java's order: **(1) the 15-minute-idle reset** — a new `ValakasCombat{last_attack_tick}` component (Java's static `_timeTracker`) is stamped by every valid lair hit in `on_valakas_attacked`; if nobody has struck in 9000 ticks Valakas ports home (`VALAKAS_REGENERATION_LOC` -105200,-253104,-15264), reverts to `DORMANT`, heals to full, clears his aggro and empties the lair — and the beat does **not** re-arm (the reset ends the fight); **(2) the escalating self-heal** — skill 4691 (`Valakas Recovery`) recast at a level scaled by missing HP (<25%→4, <50%→3, <75%→2, else 1). Deferred: the `skill_task` breath-attack combat AI. **Baium stone→live awakening landed:** Baium at rest is now the sleeping **stone statue** (29025), not a live boss — `grand_boss::spawn_from_record` hands Baium off to `baium::spawn_from_record`, which places the statue at ALIVE/WAITING (folding WAITING→ALIVE, Java's constructor) and only spawns the **live** boss + archangels on IN_FIGHT crash-recovery. (Before this the lifecycle spawned a fully-aggressive Baium at boot.) Talking to the statue shows its default html's *Wake Baium* button (`Quest Baium wakeUp`, wired via new `scripts/baium.rs`); `baium::wake_up` flips ALIVE→IN_FIGHT (locking entry), removes the stone, spawns the live boss **pinned** (`Immobilized` = Java `disableCoreAI(true)`) and arms a six-beat cinematic (`ScheduledTask::BaiumCinematic`): WAKEUP pose → earthquake+`BS02_A` → STAND pose → port the waker to `BAIUM_GIFT_LOC` → roar + gift skill 4136 → archangels arrive, the pin lifts and Baium engages the waker. Also fixed a latent bug: `dead_status(BAIUM)` was the two-state `DEAD`(1) — which is Baium's *WAITING* — so a dead Baium would have read as enterable; now 3. Six tests (stone-at-rest, WAITING-folds, wake-raises [sabotage-verified], double-wake-refused, final-beat-engages, in-fight-recovery). Deferred (next slice): Angelic Vortex (31862) entry via Blooded Fabric + teleport cube, and CHECK_ATTACK (30-min decay + self-heal). **Baium entry/exit landed:** the Angelic Vortex (31862) now ferries raiders in and the teleport cube (31842) scatters them out — the encounter is reachable without an admin port. `scripts/baium.rs` grew the vortex (first-talk `31862.html`; `enter` → `baium::entry_outcome`, which reads the fight's state **before** the fabric — DEAD `31862-03`, IN_FIGHT `31862-02`, else no Blooded Fabric `31862-01`, else take fabric 4295 + teleport to `TELEPORT_IN_LOC`) and the cube (`teleportOut` → `random_exit`, one of three surface points jittered ±100). Baium's `onKill` tail (`on_baium_killed`, wired into `on_grand_boss_killed`) drops the exit cube at `TELEPORT_CUBIC_LOC` and roars `BS01_D`. Five tests (admit, inert-without-fabric, state-before-fabric [sabotage-verified], kill-drops-cube, scatter). Deferred (last Baium slice): CHECK_ATTACK — the 30-min inactivity decay (revert to stone + clear zone) and the <75%-HP self-heal (4135) — plus `CLEAR_ZONE` cube despawn/oust and onCreatureSee cleric threat. **Baium CHECK_ATTACK landed — the encounter is now COMPLETE.** A 60 s beat (`ScheduledTask::BaiumCheckAttack`, armed at the cinematic's end and on IN_FIGHT recovery) watches a new `BaiumCombat{last_attack_tick}` component (Java's static `_lastAttack`, stamped by every hit in `on_baium_damage`): **30 min with no hit** → `clear_zone` (despawn the boss + angels in zone 70051, scatter stragglers out via `random_exit`), put the sleeping stone back, revert to ALIVE, no re-arm; **5 min idle and `<75%` HP** → self-cast `HEAL_OF_BAIUM` (4135), then re-arm; else re-arm. Three tests (30-min-reset [sabotage-verified], recently-hit-keeps-fighting, wounded-idle-self-heals). **CLEAR_ZONE cube-despawn/oust landed** (`BaiumClearZone`, armed 900 s into `on_baium_killed` → `handle_clear_zone`: despawn the cube + any lingering NPCs in zone 70051 and scatter stragglers out; 1 test). The only remaining Baium gap is `onCreatureSee` cleric-threat weighting, which needs a native creature-see hook the port doesn't have yet — deferred, not quick. **Valakas `skill_task` breath AI landed — Valakas now fights.** A 2 s beat (`ScheduledTask::ValakasSkillTask`, armed with the regen task at the cinematic's `spawn_10` and self-re-arming while FIGHTING) ports `callSkillAI`: hold `_actualVictim` on the new `ValakasCombat.actual_victim`, re-picking a random living lair player when it dies, leaves the zone, or on a 1-in-10 whim (else roam ±1400); then `getRandomSkill` — Lava Skin (4680) priority when `<75%` HP and a 1-in-150 roll (unless already up), a mass spell from `AOE_SKILLS` when ≥20 players sit within 1200, the regular pool above 50% HP, the low-HP pool (with Meteor Storm) below — cast when the target is within `max(600, castRange)`, else give chase. Three tests (casts-at-a-lair-target [sabotage-verified], stops-when-fight-over, re-picks-a-dead-victim). This closes Valakas's combat AI; only the abbreviated cosmetic cameras remain. **Antharas gap audit + fill.** An audit of `antharas.rs` (waves/cinematic/entry/skill-AI/death already done) against the 1056-line Java found three unported lifecycle/onAttack pieces, now landed: **SET_REGEN** (`ScheduledTask::AntharasSetRegen`, 60 s, armed at `start_move`) — an escalating self-heal that casts regen skill 4125/4239/4240/4241 for his HP band, once per band; **CHECK_ATTACK** (`AntharasCheckAttack`, 60 s) — a 15-min-idle reset that parks him at his resting spot (185708,114298,-8221), reverts to ALIVE, despawns the adds and ousts stragglers; and the two missing legs of **onAttack** — a new `AntharasCombat.last_attack_tick` stamp (Java `_lastAttack`), the strider debuff (4258), and the anti-exploit teleport of an attacker striking from outside the lair or before the fight is live. Five tests (heals-for-band [sabotage-verified], regen-stops-when-dead, 15-min-reset [sabotage-verified], recently-hit-keeps-fighting, strider-hindered). Audited-and-deferred (cosmetic / needs a spell-see hook): the `TID_FEAR` sandstorm walk with its BOMBER/invisible-NPC decorations, and the `onSpellFinished` 1 s MANAGE_SKILL re-arm. |
| Game  | G24 Castles, sieges, clan halls & territory war             | ✅ **The automatic siege schedule landed** (plan: [PLAN_G24_SIEGE_SCHEDULE.md](PLAN_G24_SIEGE_SCHEDULE.md)), G24's first slice. Checked before planning (the G20.5 lesson): the siege *combat* is already extensive (towers/guards/flags/doors, the throne-room artifact **capture**, zones, PvP relations, `start_siege`/`end_siege`) — but **sieges only ever fired from a GM command**: `SiegeSchedule.xml` (the weekly per-castle calendar) was never loaded, so on a real server no siege ever happened. Now each **enabled** castle's siege starts itself on its scheduled day/hour and **re-arms next week** — a self-perpetuating timer that needs no persisted `siegeDate` (the calendar is fixed, so the next occurrence is a pure function of the clock). New `SiegeScheduleEntry` loader (all 9 castles: Sunday 16:00/20:00), a pure `next_siege_millis(now, weekday, hour)` (1970-01-01 = Thursday anchor; computed in **UTC** — Rust std has no timezone, a documented divergence from Java's server-local time, the weekly cadence exact either way), boot-arming from the `SiegesLoaded` handler where the per-castle `Siege`s exist, and a `SiegeStart` task that begins the siege + re-arms. **Also confirmed a stale marker**: `capture`/`try_capture_artifact` is **already production-reachable** (the Holy Artifact 35063 etc. is a permanent castle spawn, the interaction siege-gated), so its `#[allow(dead_code)]`/"nothing reaches capture" comments were removed — a castle can be won by seizing the artifact. Both subtle wires sabotage-verified (drop the re-arm → the weekly timer dies; drop the enabled filter → disabled castles get armed). siege_schedule_tests 4. Still ⏳: clan halls (`//clanhall` — a greenfield residence subsystem), the player-facing siege registration window, and the end-siege polish (blood-alliance count, ticket count, residential skills). **Player siege-registration logic landed** (G24 continuation): the `checkIfCanRegister` ladder and the register/approve/remove operations from Java `Siege`, as pure testable functions in `game_loop/siege.rs`. `RegisterOutcome` mirrors every Java refusal branch — registration closed (24 h before the siege), siege in progress, clan below level 3, castle owner (auto-registered), owns another castle, already registered here, already registered for another siege the same weekday, attacker/defender side full (500 each, per Siege.ini), plus the two side-specific pre-checks (an owner's ally can't attack; you can't defend an NPC-held castle). `register` adds the clan (attacker → `Attacker`, defender → `DefenderPending`) and persists; `approve_defender` promotes a pending defender; `remove_registration` cancels. 11 tests (happy path sabotage-verified). **Staged** (`#[allow(dead_code)]`, the Sailren precedent): the reachability wiring — the `RequestJoinSiege`/`SiegeInfo` packet flow that lets a clan leader register in-game — is the next slice. **Siege registration is now reachable in-game.** Wired the `RequestJoinSiege` client packet (0xAD): a `CS_MANAGE_SIEGE` clan leader registers as attacker/defender (isJoining=1) or cancels (isJoining=0) for a castle, driving the staged `siege::register`/`remove_registration` ladder (the `#[allow(dead_code)]` staging came off) and getting the refreshed `SiegeInfo` window (new 0xC9 packet, Java `listRegisterClan`) back. Refusals map to their Interlude SystemMessages (the contiguous 636–641 block, anchored by the already-ported 638 — clan-level, owner-auto, owns-castle, already-requested, attacker/defender-full; the scattered ids stay window-only). New `CS_MANAGE_SIEGE = 1<<18` (ordinal confirmed via `ALL_CLAN_PRIVILEGES = (1<<24)-1`). 3 packet tests (leader-registers [sabotage-verified], cancel, unauthorized-refused). Deferred: `RequestConfirmSiegeWaitingList` (0xAE) → `approve_defender` + `SiegeDefenderList` (still staged), the attacker/defender list packets (0xAB/0xAC), and the owner's set-siege-time hour list. **Owner defender approval landed (0xAE) — the last staged siege function is now live.** Wired `RequestConfirmSiegeWaitingList` (client 0xAE): the castle owner's clan leader approves (`approved==1`) a pending defender → `approve_defender` (un-staged; the `#[allow(dead_code)]` is gone) or rejects/removes a pending-or-confirmed defender → `remove_registration`, gated on the owner-leader check and the open registration window, then sends the refreshed `SiegeDefenderList` (new 0xCB packet: owner first, then confirmed, then pending, with Java's `SiegeClanType.ordinal()+1` type bytes — owner 1, pending 2, defender 3). 3 packet tests (approve [sabotage-verified], reject, non-owner-refused); also made the register/cancel packet tests deterministic (the handlers read real wall-clock `now`, so the tests disable the schedule to keep registration open). Deferred: the attacker/defender list *request* packets (0xAB/0xAC) and the owner set-siege-time hour list (0xAF). **Clan halls — foundation landed.** The greenfield residence subsystem's data layer: a new `ClanHall` model + `ClanHallData` loader parses all **48** `data/residences/clanHalls/**` XMLs (grade A/B/S, AUCTIONABLE type, auction terms minBid/lease/deposit, agent NPCs, doors, owner-restart + banish points) into `GameData.clan_halls`; a new `clanhall` DB load (`DbEvent::ClanHallsLoaded`, id→ownerId/paidUntil) overlays persisted **ownership** onto them at boot (mirroring the SiegesLoaded flow) into `World.clan_halls`. Reachable read: the admin `//claninfo` panel's `clan_has_clanhall` now shows the clan's owned hall by name (was hardcoded "No"). 3 tests (all-48-load + Onyx Hall fields, ownership-overlay [sabotage-verified], find-clan-by-hall). Next clan-hall slices: the auction/bidding cycle, the lease-payment/eviction loop, the Clan Hall Manager NPC + teleport agents, and function upgrades. **Clan-hall auction logic landed.** The bid/outbid/cancel/finalize core of `ClanHallAuction` in `game_loop/clan_hall_auction.rs`, over new `World.clan_hall_bids` (hall→clan→bid). `place_bid` ports Java's `processBidBypass` ladder — hall must be free & auctionable, clan level ≥2, not already owning a hall, not bidding elsewhere, ≤999.9 B, above the current highest (or the minimum) — then takes the bid from the clan **warehouse** and **refunds the previous highest bidder** (the escrow invariant: only the top bid's adena is ever held). `cancel_bid` removes the bid with no refund (Java `removeBid` — the highest forfeits by cancelling); `finalize_auction` awards the hall to the highest bidder and clears the bids. `BidOutcome` mirrors each Java refusal. 9 tests (first-bid, outbid-refund [sabotage-verified], not-above-highest, below-minimum, insufficient-adena, two-halls, owner-can't-bid, finalize-awards, cancel-no-refund). **Staged** (`#[allow(dead_code)]`, the siege-registration precedent): the Clan Hall Auctioneer NPC (bid/cancel reachability), bidder persistence + boot load, and the weekly finalize scheduler are the next slice. **Clan-hall auction is now reachable & persistent.** Un-staged the whole auction module — three callers: the **Clan Hall Auctioneer NPC** (30767, new `scripts/clan_hall_auctioneer.rs`) routes `bid id=X bid=Y` → `place_bid` (leader-gated, inline bid form with the hall baked in) and `cancel` → `cancel_bid`; the **weekly close** (`ScheduledTask::ClanHallAuctionEnd` → `handle_auction_end`, armed at boot, self-re-arming every 7 days) finalizes every hall's auction; and **persistence** — `place_bid`/`cancel_bid` write `clanhall_auctions_bidders` (SaveClanHallBid/RemoveClanHallBid) and flush the affected clan warehouses, `finalize_auction` writes `clanhall` ownership (SaveClanHall) + clears bids, and boot restores bids (`DbEvent::ClanHallBiddersLoaded`). 3 tests (weekly-close [sabotage-verified], bid-persists, bids-restored-at-boot) on top of the 9 logic tests. Deferred: full templating of the auctioneer's dynamic pages (hall list, bidder list, current-bid display), and the lease/eviction rental cycle (`paidUntil`). **Clan-hall lease/eviction cycle landed** (`ClanHall.CheckPaymentTask`). Winning a hall now starts a rental clock: `set_hall_owner` sets `paidUntil = now + 7 days` and arms a `ScheduledTask::ClanHallLeaseCheck`; `handle_lease_check` charges the weekly `lease` adena from the owner's clan warehouse and advances `paidUntil` a week, or — if the warehouse can't cover it — retries daily until the rent is **more than 8 days overdue**, at which point `revoke_hall` returns the hall to the free auction pool (owner cleared, clock reset). Ownership + `paidUntil` persist via `SaveClanHall` throughout; owned halls re-arm their lease check at boot (the `ClanHallsLoaded` handler). 4 tests (lease-clock-starts, solvent-owner-pays, delinquent-retry, week-overdue-eviction [sabotage-verified]). Deferred: the clan-member overdue/revoked SystemMessage broadcasts, and the auctioneer's dynamic-page templating. **Clan Hall Door Manager NPC landed.** The owning clan can open/close its hall's doors: new `scripts/clan_hall_door_manager.rs` (all ~45 `DOOR_MANAGERS` NPCs) routes `manageDoors 1|0` → `open_close_hall_doors` (gated on the owning clan + the CH_OPEN_DOOR privilege, else the no-authority page), and first-talk shows the owner/unowned/other page. NPC→hall resolution is `hall_by_npc_id` (the hall whose `<npcs>` names the template id, Java `Npc.getClanHall`); a new `doors::set_door_by_id(id, open)` drives each of the hall's doors through the existing door system. New privilege consts `CH_OPEN_DOOR`(1<<11)/`CH_DISMISS`(1<<14). 2 tests (npc→hall lookup, door open/close toggle [sabotage-verified]). Deferred: the ClanHallManager NPC's expel/banish (needs a ClanHallZone to find non-members inside) and the function-upgrade menu (HP/MP/XP regen, teleport, buffs). **Clan-hall function-upgrade economics landed** (`ClanHall.addFunction`/`ResidenceFunction`). New `ResidenceFunctionData` loader parses `data/ResidenceFunctions.xml` (each function id/type — HP_REGEN, TELEPORT, … — with a per-level cost/duration/value ladder) into `GameData`; active purchases live in `World.clan_hall_functions` (hall→func→level/expiry), restored at boot from `residence_functions` (`DbEvent::ResidenceFunctionsLoaded`). `buy_function` (Java `setFunction`) charges the level's cost from the **buyer's own inventory**, records the function and arms a `ScheduledTask::ClanHallFunctionExpire`; on expiry `handle_function_expiry` renews by charging the **owning clan's warehouse** (Java `reactivate`) or drops the function; `remove_function` clears it. All persisted via `SaveResidenceFunction`/`RemoveResidenceFunction`. 6 tests (catalogue-loads, buy-charges-and-activates [sabotage-verified], insufficient-refused, expired-renews-or-drops, remove). **Staged** for the Clan Hall Manager NPC's function menu; the per-type *benefits* (HP/MP regen, teleport, buffs — several need a ClanHallZone) are their own slices. **Clan Hall Manager NPC landed — un-stages the function economics.** New `scripts/clan_hall_manager.rs` (all ~48 `CLANHALL_MANAGERS` NPCs) is the owning clan's console: `manageDoors 1|0` → `open_close_hall_doors` (CH_OPEN_DOOR), `manageFunctions setFunction <id> <lv>` → `buy_function` (CH_SET_FUNCTIONS; manageFuncDone/noAdena), `manageFunctions removeFunction remove <TYPE>` → `remove_function` (via a new `id_of_type` reverse lookup), plus the static recovery/other/decor/selectFunction menu pages — the whole console owner-gated (Java's `isOwningClan`). The purchase/remove logic's `#[allow(dead_code)]` staging comes off. New privilege const `CH_SET_FUNCTIONS`(1<<15). 1 test (type→id lookup) on top of the tested function/door logic. Deferred (need infrastructure): `expel`/banishOthers and the `useFunctions` benefits (teleport/buffs/regen), several of which need a ClanHallZone. **ClanHallZone groundwork + banish landed.** The zone layer now loads `data/zones/clan_hall.xml` as a new `ZoneKind::ClanHall` (geometry-queried, no membership bit) with a `clanHallId` stat, exposing `ZoneData::clan_hall_at(x,y,z) → Option<hall_id>` — the "who's inside a hall" lookup the richer features need. First consumer wired: the Clan Hall Manager's `expel` bypass (CH_DISMISS) → `banish_others`, which ejects every player standing in the hall who isn't in the owning clan to the hall's banish point (Java `banishOthers`). 2 tests (point→hall-id resolution against real dist zones, banish-ejects-outsiders-not-members [sabotage-verified]). This unblocks the last clan-hall gap group — the remaining `useFunctions` per-type *benefits* (HP/MP-regen for members inside, teleport list, support buffs) can now scope by `clan_hall_at`. **Clan-hall HP/MP regen benefit landed** (`RegenHPFinalizer`/`RegenMPFinalizer`). A clan member standing in **their own** hall now regenerates HP/MP faster by the hall's `HP_REGEN`/`MP_REGEN` function `value` (1.2–5.0 / 1.05–1.50). `regen_player` gained the two hall multipliers; `run_regen_tick` computes them via `clan_hall_regen_mult` — `clan_hall_at(pos)` gives the hall the player stands in, and the boost applies only when that hall's `owner_id` is the player's clan (Java's `clanHallIndex == posChIndex`, derived from ownership since the port has no `hideoutId` field). New `clan_hall_function::active_function_value(hall, type)` reads the active level's value. 3 tests (value-reported, member-boosted, outsider-not-boosted [sabotage-verified]). The first of the `useFunctions` benefits — teleport list / support buffs / item creation remain. **Clan-hall teleport benefit landed.** The Clan Hall Manager's `useFunctions teleport` (CH_OTHER_RIGHTS-gated) now shows and executes the hall's teleport list: the hall's `TELEPORT` function level picks the manager NPC's `tel<level>` list (`data/teleporters/clanhall/**`), routed through the manager's own quest bypass (`Quest ClanHallManager useFunctions teleport`) so a button click returns as `useFunctions teleport tel<n> <loc>` and teleports (guarding the list token's level against the hall's, Java's `teleportLevel == funcLvl`); level 0 serves `ClanHallManager-noFunction.html`. `teleporter::show_teleport_list`/`do_teleport` are now `pub(crate)`, and `show_teleport_list` takes an explicit `bypass` button-prefix (gatekeepers still pass `npc_<oid>_teleport`). New `CH_OTHER_RIGHTS`(1<<12). 2 tests (TELEPORT function + tel1/tel2 lists load; a hall teleport moves the player). Remaining `useFunctions` benefits: support buffs and item creation. **Clan-hall support-BUFF benefit landed.** `useFunctions buffs` now serves the support-magic menu and casts it: the hall's `BUFF` function level picks `funcBuffs_<level>.html` (with `%manaLeft%` filled from the manager NPC's current MP), and a `<skillId>_<skillLevel>` button drives `cast_hall_buff` — the skill must be in `ALLOWED_BUFFS` (the 4342–4360 support line), the manager NPC must have the MP (`mpConsume + mpInitialConsume`), and the skill must be off its reuse timer (Java `castSkill` → `npc.doCast`). On success it trigger-casts the buff on the caller (reusing `support_magic::cast_from_npc`), charges the NPC's MP, and arms the reuse; the result page (Done / NoMp / NoReuse) shows the remaining MP. 4 tests (cast charges MP + lands on the player [MP-charge sabotage-verified], insufficient-MP refused with nothing spent, reuse blocks a repeat, only listed buffs castable). Remaining `useFunctions` benefit: item creation (the Merchant buy-window path). **Clan-hall ITEM benefit landed — the `useFunctions` benefits are complete.** `useFunctions items` opens the hall's item shop: the ITEM function level (1/2/3) selects the buylist `npcId*100 + (level-1)` (Java `showBuyWindow(player, npcId·"0"·(level-1))`) and opens it through the existing `shop::show_buy_window` (module made `pub(crate)`); any other level serves `noFunction.html`. 2 tests (the level→buylist formula maps to real dist buylists that allow the manager NPC — levels 1-3 exist, level 4 doesn't; a manager serves its item buylist as an ExBuySellList packet [sabotage-verified against a bad id]). All three clan-hall `useFunctions` benefits (teleport / buffs / items) are now wired. **Clan-hall EXP_RESTORE benefit landed — the last clan-hall function benefit.** Respawning "to clanhall" (`RequestRestartPoint` case 1, previously unhandled — it fell through to the town respawn) now sends the player to their clan's hall `ownerRestartPoint` and, if the hall has the EXP_RESTORE function, restores that percentage (5%-60% by level) of the exp the death penalty cost — Java `RequestRestartPoint.portPlayer` + `Player.restoreExp`. The port pre-computes the lost amount into `lost_exp_on_death` (as the resurrection path does), so the restore reads it directly and pushes a `UserInfo` like Java's `addExp`. 2 tests (a level-1 5% function restores 5% of lost exp and lands at the hall [restore sabotage-verified]; a hall without the function restores nothing while still respawning there). Clan halls are now **functionally complete** (12 slices: data/ownership, auction, auctioneer, lease/eviction, doors, function economics, manager console, ClanHallZone/banish, HP/MP regen, teleport, buffs, items, EXP_RESTORE); only the auctioneer's dynamic-page templating remains as polish. **Auctioneer dynamic-page templating landed — the last clan-hall item.** The Clan Hall Auctioneer (30767) now renders its dynamic dist htmls with real auction data instead of serving them with unfilled placeholders (Java `ClanHallAuctioneer`): `auctionList` (no id) → the free-hall list (`%agitList%` rows link to each hall's info page with the current highest bid; owned halls excluded per `getFreeAuctionableHall`); `auctionList id=X` → the hall info page (rent, grade, minimum bid, bid count, auction-end countdown); `bid id=X` (bid=0) → the templated bid form (clan warehouse adena + minimum); `listBidder id=X` → the bidder list, newest bid first, with clan names/amounts; `cancelBid` → the cancel-confirmation (own bid + non-refundable-tax note). The `%auctionEnd%`/`%hours%`/`%minutes%` countdown comes from a new `World.auction_end_tick` set when the weekly close is armed; dates use the civil-from-days formatter the community board uses. The event parser now tolerates the dist bid form's quoted, space-padded value (`bid=' $bidprice '`). New `clan_hall_auction::bid_count` helper. 4 tests (free-hall list templates rows + omits owned halls, info page fills rent/grade/minBid/bidCount, bidder list orders newest-first [sort sabotage-verified], bid form shows clan adena + minimum). **Clan halls are now fully complete** — no remaining gaps. **Siege-end blood-alliance + ticket counts landed** (the flagged `TODO(G24)` in `end_siege`). Java `endSiege`'s two owner-branch effects now resolve: defenders held (owner unchanged) → the owner clan's blood-alliance count is bumped by `SiegeManager.getBloodAllianceReward()` and persisted (the Interlude reward is 0 per Siege.ini `BloodAllianceReward = 0`, so it stays 0 unless an admin raises the `BLOOD_ALLIANCE_REWARD` knob); attacker captured (owner changed) → the castle's mercenary ticket-buy count is reset to 0 (`Castle.setTicketBuyCount(0)`) and persisted. New `Clan.blood_alliance_count` (loaded from `clan_data.blood_alliance_count`, persisted via `UpdateClanBloodAlliance`) and `Castle.ticket_buy_count` (from `castle.ticketBuyCount`, via `UpdateCastleTicketCount`) — both columns already in the schema. `Hero.setCastleTaken` for the captor's nobles is left as `TODO(G25)` (hero tracking unmodelled). 2 tests (defenders-hold awards the reward + leaves the ticket count untouched; a capture resets the ticket count + gives no reward — the untouched-vs-reset ticket count is the branch discriminator, condition sabotage-verified). The remaining G24 end-siege item is the residential (castle) skills. **Residential castle skills landed — the last end-siege item.** A castle-owning clan now grants its members the castle's residential skills (Residence Body/Health/Spirit/…, Java `AbstractResidence` + `Castle.setOwner` + `Player.enterWorld`). The pledge skill tree now parses each residential entry's `<residenceId>` list (`PledgeSkillLearn.residence_ids`), and `available_residential_skills(residenceId)` returns a residence's skills (Java `getAvailableResidentialSkills`). New `clans::give_residential_skills` (gated by the same `pledgeClass+1 >= socialClass` rule as clan skills) / `remove_residential_skills` ride the transient `ClanSkills` passive channel (never persisted), keyed by their own ids (590+). Wired at **login** (a castle-owning clan's member gets them in `apply_clan_skills_to_member`) and **capture** (`siege::capture` strips them from the former owner's online members and grants them to the captor's). Also corrected a stale comment claiming no dist entry sets `residenceSkill` — the pledge tree carries the 590-series. 3 tests (skills load per castle [castle 1 → Residence Health 593]; a member gets/loses them on login; a capture moves them old-owner→captor — `available_residential_skills` filter sabotage-verified). **G24 is now essentially complete** — the remaining items are the optional siege list-request packets 0xAB/0xAC and the owner set-siege-time hour list (0xAF). **Siege list-request packets landed (0xAB/0xAC).** Wired the two client packets that open the register window's attacker/defender tabs: `RequestSiegeAttackerList` (0xAB) → a new `SiegeAttackerList` server packet (`CASTLE_SIEGE_ATTACKER_LIST`, 0xCA) listing the castle's registered attacker clans (id/name/leader/crest/ally, ally-leader name written empty as Java does); `RequestSiegeDefenderList` (0xAC) → the existing `SiegeDefenderList` (0xCB), reusing the owner-approval roster builder (owner, then confirmed, then pending defenders). Both read just the castle id and send nothing when the castle doesn't exist (Java `getCastleById == null`); no gating — any in-game player may view. 3 tests (attacker list answers 0xAB with the one registered attacker, defender list answers 0xAC with owner + pending defender, unknown castle ignored — attacker-filter sabotage-verified). The only G24 remainder is the owner set-siege-time hour list (0xAF), plus `Hero.setCastleTaken` deferred to G25. **Owner set-siege-time (0xAF) landed — G24's last packet.** `RequestSetCastleSiegeTime` (0xAF) + the `SiegeInfo` hour-list branch: when the owner-leader may still set the time (`!isTimeRegistrationOver`), `SiegeInfo` (0xC9) now offers the selectable `SIEGE_HOUR_LIST` slots (`Feature.ini SiegeHourList = 16,20`) instead of the fixed date; the owner picks one (validated against the hour slots on the castle's scheduled day, Java `isSiegeTimeValid`), which stores it, closes the window, persists, broadcasts "S1 has announced the next castle siege time", and refreshes `SiegeInfo`. New `Castle.time_registration_over` (from `castle.regTimeOver`, default true → feature dormant until an operator opens the window) + `Castle.siege_date` (from `castle.siegeDate`), persisted via `UpdateCastleSiegeTime`. The chosen time is honored in the `SiegeInfo` display and the registration cut-off (`effective_siege_millis`); the auto-start timer still fires at the fixed `SiegeSchedule.xml` hour (`TODO(G24)`: honoring it in the timer needs scheduler task cancellation), and the choice clears at siege start. 4 tests (owner sets a valid time, invalid hour rejected [valid-time gate sabotage-verified], non-owner refused, SiegeInfo offers the hour list when open). **G24 is now complete** — the only deferred item is `Hero.setCastleTaken` on capture (G25, hero tracking unmodelled). **`Hero.setCastleTaken` landed — G24's last TODO closed.** When an attacker captures a castle, each of the capturing clan's online **noble** members now gets a `heroes_diary` "castle taken" entry (Java `endSiege` → `Hero.setCastleTaken` → `setDiaryData(charId, ACTION_CASTLE_TAKEN=3, castleId)`). New `SaveHeroDiary` DbCommand (`INSERT INTO heroes_diary`); `end_siege`'s owner-changed branch calls `record_castle_taken_for_nobles` — every online member of the new owner clan whose `Player` is a noble gets a diary row (action 3, param = castle id, keyed by char id = the player's object id). The in-memory hero-diary display (only meaningful for a currently-crowned hero, unmodelled) is skipped; only the persistent row is written. 2 tests (a capture diaries the captor's noble, a non-noble captor gets nothing — `isNoble` gate sabotage-verified). **G24 is fully complete with no deferred items.** |
| Game  | G24.5 Boats                                                 | ✅ **Slice 1 (the moving ferry) landed** — a Boat world object (model/boat.rs, not a player/NPC) cycles a route: spawns at its dock, sails waypoint-to-waypoint on a travel-time (distance/speed) schedule (ScheduledTask::BoatArrive), broadcasting VehicleInfo/VehicleDeparture (opcodes 0x60/0x6C). The Talking↔Gludin route (BoatTalkingGludin) is hardcoded. Slice 2 (boarding — THE GATE) landed: dock waypoints (dwell + BoatDepart), RequestGetOnVehicle/GetOffVehicle (0x53/0x54) → board/disembark gated on the boat anchored + within 1000 units, InVehicle component (boat + relative seat), passengers ride along (position snapped to the boat on arrival), GetOnVehicle/GetOffVehicle packets. Gate met: board → sail between harbors → disembark. Slice 3 landed: all four Interlude ferries run — BoatGiranTalking, BoatInnadrilTour (single-harbor scenic loop) and BoatRunePrimeval added alongside BoatTalkingGludin (waypoints transcribed from the Java scripts, docks marked). Slice 4 landed: staged dwell + departure announcements — each dock can carry a DockSchedule (ordered DwellStages: system-message ids + inter-stage delays), driven by ScheduledTask::BoatDwellStage; on docking the ferry shouts "arrived", then 5-min/1-min/"leaving soon" warnings, then departs (CreatureSay ChatType.BOAT=11 via the polymorphic-name creature_say_system builder, broadcast to both harbours). Slice 5 landed: all four ferries now carry their real announced cadences — Talking↔Gludin, Giran↔Talking and Innadril anchor 10 min (ten_minute_dwell! macro), Rune↔Primeval anchors 3 min (three_minute_dwell!), each with its own harbour message ids (979–1002, 1620, 1988–1992). Slice 6 landed: ticket collection (payForRide) — on departure each harbour charges its boat ticket (DockSchedule.fare): riders holding it have one consumed (Innadril is free, ticket id 0), stowaways get SM 402 + teleported ashore to the oust location. Slice 7 landed: on-deck movement (RequestMoveToLocationInVehicle 0x75) — riders walk on deck; the server updates their relative seat and broadcasts MoveToLocationInVehicle (0x7E), or StopMoveInVehicle (0x7F) on a zero-length move; they stay at the new seat as the ferry sails. Slice 8 landed: in-transit "arriving in ~N minutes" shouts (DockSchedule.voyage, scheduled at departure via BoatVoyageShout, skipped once docked). G24.5 combat/vehicle features complete for Interlude; busy-dock delay messages omitted (one boat per route = a harbor is never occupied, so Java dockBusy is unreachable). |
| Game  | G25 Olympiad & hero                                         | ✅ **COMPLETE** — **Slice 1 (noble registration) landed** (plan: [PLAN_G25_OLYMPIAD.md](PLAN_G25_OLYMPIAD.md)) — `OlympiadState` on `World` (period/cycle, `in_comp_period`, `comp_end_tick`, the noble registry + the two waiting queues); `game_loop/olympiad.rs` `register`/`unregister` with the Classic gates (competition period open, 20-min registration cutoff, 30/week cap, already-registered, eligibility = 3rd/4th class group + level 55), creating the noble record with the starting 10 points on first join. **Slice 2 (the manager NPC dialog) landed** — OlyManager (31688) ported as a QuestScript (scripts/oly_manager.rs): onFirstTalk (noble/noNoble/noCursed), the `Quest OlyManager <event>` menu bypasses (index, static info/rules/points/rewards pages, joinMatch with round/week/participant substitutions, register1v1 with the subclass/eligibility/points gates, unregister), and empty class-rank pages. NON_CLASSED 1v1 only. **Slice 3 (DB persistence) landed** — boot-loads `olympiad_data` (cycle/period/end-times) + all `olympiad_nobles` into `OlympiadState` (`DbEvent::OlympiadLoaded` → `olympiad::apply_loaded`), and writes the period row + every noble record on shutdown (`DbCommand::SaveOlympiad`, `olympiad::save_all`). **Slice 3b (period/window state machine) landed** — the competition window opens 18:00 for 6 h on the weekend competition days (`AltOlyCompetitionDays = 1,7`) and closes/clears the queues, driven by `ScheduledTask::Olympiad{CompStart,CompEnd,WeeklyChange}` armed at boot from the persisted `next_weekly_change`; the weekly refresh adds the weekly points + resets weekly matches (both skipped in the validation period). `in_comp_period` now toggles on the real schedule instead of only in tests. **Slice 4 (match-making) landed** — a game-manager sweep (`ScheduledTask::OlympiadGameManager`, every 30 s while the window is open) pairs waiting nobles into stadium matches once the non-class queue holds ≥ 20 (`AltOlyNonClassedParticipants`): each of the 4 arenas takes two random **online** players drawn from the queue (Java `createListOfParticipants`, offline entries dropped), marking them `in_competition` (which now gates re-register/unregister). `OlympiadMatch` records + `in_competition` clear at comp end. **Slice 5 (match run + scoring) landed** — a started match ports both fighters to the arena (remembering their prior spot) and polls via `ScheduledTask::OlympiadMatchTick`: a death/disconnect loses, the 5-min `AltOlyBattle` timeout is a draw. On resolve (Java `validateWinner`) the point transfer `clamp(min(pts)/5, 1, 10)` moves winner→loser (floored at 0), win/loss/draw + `competitions_done`/`_week` are recorded, both are ported back, the match frees them, and the state persists. Stadiums share one grassy-arena coord for now (true per-match instances need G27). **Slice 6 (monthly heroes + cycle) landed** — `ScheduledTask::OlympiadEnd` (period 0→1) computes the heroes (Java `sortHerosToBe`: per `FOURTH_CLASS_GROUP` class the top eligible noble — competitor on that class or its parent 3rd class, ≥ 10 matches, ≥ 1 win), uncrowns the old + crowns the new (reusing `admin::hero::set_hero` for online winners), then arms `OlympiadValidationEnd` (period 1→0) which advances the cycle and truncates the noble table. Both armed at boot from the persisted `olympiad_end`/`validation_end` (`OLYMPIAD_PERIOD_MS` defaults a fresh/past end to +30 days). **Hero persistence landed** — the crown saves to the `heroes` table (`DbCommand::SaveHeroes`, count bumped per re-crown), boot-loads (`DbEvent::HeroesLoaded`), and re-applies on login (`olympiad::on_enter_world`) so it survives relogs / reaches offline heroes. **Point→mark exchange landed** — at round end each noble's exchangeable points (rank percentile tiers 1-5 = 200/80/50/30/15 + 300 hero bonus, Java loadNoblesRank/getOlympiadTradePoint) are banked on the UNCLAIMED_OLYMPIAD_POINTS variable (online nobles); the OlyManager calculatePoints/calculatePointsDone buttons convert them to Mark of Battle (45584) at 20/point. **Match buff-strip + round-ended broadcast landed** — fighters lose all active buffs on entering the arena (AbstractOlympiadGame.removeBuffs), and the round end announces ROUND_S1..._HAS_NOW_ENDED to everyone online. TODO (follow-ups): the pre-fight countdown ceremony, exact calendar month-end, offline-noble trade-point write, isInventoryUnder80 gate. **showEquipmentReward multisell + exact period-end landed** — the reward-shop button opens EQUIPMENT_MULTISELL 3168801; the round end is now the exact retail boundary (noon, AltOlyPeriod DAY × 14 = 13 competition days + 1 validation) instead of a 30-day approximation. **Offline trade-point write landed** — round-end reward points now reach offline nobles too, via DbCommand::StoreCharVar (a targeted character_variables delete+insert) instead of only the online PlayerVariables component. **isInventoryUnder80 gate landed** — register1v1 and the point→mark exchange now refuse (with SM 1118) while the non-quest slot count exceeds 80%% of the inventory limit. **Pre-fight countdown ceremony landed** — a started match now runs the teleport countdown (AltOlyWaitTime 120s, SM 1492) then teleports+strips, then the 60s battle countdown (SM 1495) before the fight begins (ScheduledTask::OlympiadCountdown step machine). G25 COMPLETE except stadium instancing (blocked on G27). **Stadium instancing landed** (closed by G27 slice 1): each concurrent match gets its own instance (`OlympiadMatch.instance_id`, `world.instances.create`/`destroy` around the match, `InstanceId` on both fighters), so matches at the shared arena coords no longer see each other. **Monument of Heroes NPC landed** — `scripts/monument_of_heroes.rs` (31690), the hero-reward NPC: onFirstTalk 3rd/4th-class + lvl55 gate; `heroWeapon` → the Infinity-weapon list (hero + inventory-80% gated) with `give_<id>` handing over the chosen weapon (the 10 listed ids); `heroCirclet` → the Wings of Destiny Circlet (once); `receiveCloak` → the Hero Cloak (Java's rank-1 gate == a crowned class hero, so `is_hero` stands in); `heroCertification` → already/not-a-hero (this port auto-crowns at the Olympiad end). 2 tests via the Quest-bypass path (a hero claims a weapon + circlet-once, a non-hero is refused — hero gate sabotage-verified). Deferred (`TODO(G25)`): the `heroList` → `ExHeroList` packet and the observer/spectator mode (`RequestOlympiadMatchList`/`ObserverEnd`, `ObservationMode`/`Return`) — both polish beyond the G25 gate. **Observer mode landed** — a player can watch ongoing matches: the OlyManager's `watchmatch` bypass sends the `ExOlympiadMatchList` picker (`EX_RECEIVE_OLYMPIAD`, arena/playing-standby/both fighter names), and `arenachange <n>` (or the client's `_olympiad?command=move_op_field&field=N`) enters observer mode — gated on the competition period + not being registered/competing, it saves the return point, scopes the viewer into the match's instance (so they see only that fight), teleports them to the spectator stand, and sends `ExOlympiadMode(3)`; `RequestOlympiadObserverEnd` (ex 0x29) drops the state, teleports back, and sends `ExOlympiadMode(0)`; `RequestOlympiadMatchList`/refresh re-send the list. New `OlympiadObserver` component + `server_packets/olympiad.rs`; invul/invisible while observing is skipped (the port has no such flags — the instance scoping isolates the spectator). 2 tests (the enter→observe→leave round-trip, a competitor refused [gate sabotage-verified]). Remaining G25 polish: the `ExHeroList` packet + the observer invul flags. **`ExHeroList` landed** — the Monument's "Hero List" button now sends the hero roll (`EX_HERO_LIST`, 0xFE:0x7A): each crowned hero's name, class, clan name/crest, ally name/crest, and times-been-a-hero count. Hero display data is resolved even for offline heroes via a new `OlympiadState.hero_info` map (name + clan id), populated from the boot heroes load (extended to `LEFT JOIN characters` for `char_name`/`clanid`) and at crown time (name from the noble, clan from the online player); clan/ally names + crests are resolved from the live clan registry at send time. 1 test (an offline hero is listed with its name + count, row-count sabotage-verified). **G25 COMPLETE** — the last polish landed: the **observer-mode invul + invisible flags** (spectators are now untouchable/hidden via `AdminFlags`, cleared on leave) and the **hero-diary window** (`_diary?class=X&page=Y` → `show_hero_diary`: boot-loads `heroes_diary` + `heroes.message` into `OlympiadState.hero_diary`/`HeroInfo.message` via the extended `HeroesLoaded` event, renders `herodiary.htm` paginated with the 3 action formats — raid/hero/castle). 3 tests, sabotage-verified. Was: `AdminOlympiad`/`//sethero`/`//saveolymp`/`//endolympiad` |
| Game  | G26 Seven Signs, Manor & Mammon                             | ✅ **COMPLETE** — **Seven Signs is removed from this dist** (no Java class survives — the Interlude Classic build drops the whole Signs/Festival/Dawn-Dusk system), so G26 reduces to **Manor** + the two **Mammon** merchant NPCs. **Manor seed-catalogue foundation landed**: `ManorData` on `GameData` ports `CastleManorManager`'s `data/Seeds.xml` — the `Seed` model (castle/seed/crop/mature ids, reward1/2, `alternative` flag, seed/crop limits) keyed by castle, all 9 castles. Gated by a new `AllowManor` config (`General.ini`, **dist ships `False`** — but per the config-disabled-still-port rule the data + packets load regardless so an operator flipping the flag gets a working manor). `ex_send_manor_list` now sends the real manor castle ids gated on `allow_manor`; `REQUEST_MANOR_LIST` dispatch wired. **Chamberlain manor menu landed**: the Castle Chamberlain (of Light / of Darkness, 18 NPC ids) ported narrowed to the manor branch — `on_first_talk` serves the owner console (`chamberlain-01`) / non-owner page (`chamberlain-04`), and the "Manage manor" button gates on **castle ownership + the new `CS_MANOR_ADMIN` clan privilege** (ordinal 17), serving `manor.html` or the refusal page; `AllowManor=False` chats "deactivated" (Java's `sendMessage` branch). The `manor_menu_select` client bypass (`OnNpcManorBypass`) is wired — parses ask/state/time, resolves the castle through the last folk NPC, and routes **request 5 → `ExShowManorDefaultInfo`** (the seed/crop reference table, built from `ManorData.all_crops` + item reference prices resolved on the fly). **Production/procure runtime state landed**: `model/manor.rs` (`SeedProduction`/`CropProcure` with `decrease_amount` + `ManorState` holding the current/next-period per-castle lists and the `getSeedProduction`/`getCropProcure`/`getSeedProduct` getters), carried on `World.manor`, **boot-loaded** from `castle_manor_production`/`castle_manor_procure` (`DbEvent::ManorLoaded`, grouped by castle+period, unknown seeds/crops filtered out per Java's `loadDb`). **Requests 3 & 4** now serve that live state — request 3 → `ExShowSeedInfo` (new packet), request 4 → `ExShowCropInfo`, each line's level/rewards resolved via `seed_by_id`/`seed_by_crop`. **Owner setup path landed**: `ManorMode` (Disabled/Modifiable/Maintenance/Approved, default Approved) on `ManorState` with the `isManorApproved`/`isModifiablePeriod`/`isUnderMaintenance` predicates; requests **7 → `ExShowSeedSetting`** / **8 → `ExShowCropSetting`** (new packets) list every seed/crop the castle farms with its catalogue limits/prices (`seed_limit = limit × RateDropManor`, min/max = reference price × 0.6 / × 10) + the owner's current/next settings, both gated on the modifiable period (the `A_MANOR_CANNOT_BE_SET_UP` SystemMessageId is `TODO` — not in this repo's data). **`RequestSetSeed`/`RequestSetCrop`** (ex 0x03/0x04) parse the owner's submission, gate on modifiable-period + ownership + `CS_MANOR_ADMIN` + chamberlain range, filter each line to a known seed/crop within its limit/price band, and replace the castle's next-period state. New config `RateDropManor` + `AltManorSaveAllActions`. **Period scheduler landed**: `schedule_manor_at_boot` sets the initial `ManorMode` from the wall clock (Java's init guess, quirk kept) and arms a `ScheduledTask::ManorModeChange`; `advance_manor_mode` cycles APPROVED→MAINTENANCE→MODIFIABLE→APPROVED on the daily `AltManor*` cutover times (refresh 20:00, approve 04:30, 6-min maintenance) and re-arms — stale boot modes self-correct via Java's immediate-fire cascade. The APPROVED→MAINTENANCE step runs the **production rollover** (`ManorState::roll_period`): each **owned** castle's next-period seed/crop setup becomes current and next is re-seeded at full amounts (unowned castles skipped; lists kept as independent clones vs Java's shared-object aliasing). **Manor Manager buy-seed trader landed**: `RequestBuySeed` (0xC5) — a player buys seeds from a castle's current production; gates on `!isUnderMaintenance` + castle exists + last folk NPC is a Merchant in range whose `manor_id` param matches, validates price/stock/adena, takes adena, decrements stock (`ManorState::decrease_seed_amount`), hands over seeds + `S1_ADENA_DISAPPEARED`. Added `NpcTemplate::ai_param_i32`. **Note: the reference build never sends the buy/sell *display* packets** (`BuyListSeed`/`ExShowSellCropList` are dead), so the trader window is client-native. **Sell-crop trader landed**: `RequestProcureCropList` (ex 0x02) — sell crops for the crop's reward item (`getSeedByCrop(crop).getReward(type)`), paying `price / rewardReferencePrice` of the reward, with a 5 % adena fee when the crop's procurement is registered at a castle other than the manager's; decrements the procurement (`ManorState::decrease_crop_amount`), destroys the crops, adds the reward. Added `Inventory::item_by_object_id`. The `FAILED_IN_TRADING` SystemMessageId (not in this repo's data) is `TODO` — the line is still skipped as Java does. **The buy+sell trader is now functionally complete** (both handlers live; the reference never sends the buy/sell display packets, so the window is client-native). **Remaining**: the economic settlement folded into Java's rollover (crop payout to clan warehouse via `getMatureId`, treasury refund/charge, affordability gating) + the buy-seed treasury credit + the maintenance leader notification — all need the **unported castle treasury**; plus per-action DB persist (`AltManorSaveAllActions`, off on dist). **Seed sowing + harvest landed — the "sow+harvest a seed" gate is MET**: NPC seed state (Java `Attackable._seed`/`_seederObjId`/`_seeded`/`_harvestItem`, inline `Npc` fields) + the `canBeSown` template flag; `ItemHandler::Seed` (validate the targeted monster, flag it with the seed+seeder, cast the item's Sow skill); `SkillEffect::Sow` (roll `calc_sow_success` — base 90 %/20 % alternative, level-scaled, Java's discarded-`Math.max` floor quirk kept — and on success mark seeded + stash the crop: strong-type ×2..×9 + hi-level bonus × `RateDropManor`); `SkillEffect::Harvesting` (on the dead seeded corpse the caster sowed, roll `calc_harvest_success` — 100 %, floored 1 % — and hand over the crop via `takeHarvest`). The seed is consumed by the item-skill cast; the `taxCastle` sow-location gate + the sow/harvest `SystemMessageId`s are `TODO` (unported tax zones / ids not in this repo's data). **Castle treasury landed** (2026-07-30, audit row 8's blocker): `Castle.treasury` on the castle model + `game_loop/castle.rs` porting `addToTreasury`/`addToTreasuryNoTax` (owner gate, negative-withdrawal refusal, `MaxAdena` clamp, per-change `DbCommand::UpdateCastleTreasury` write) and `getTaxPercent`/`getTaxRate` off the new `Feature.ini` buy/sell tax keys; the **liege cascade** (Gludio/Dion/Giran/Oren/Innadril → Aden, Goddard/Schuttgart → Rune) verbatim, including Java's quirk that an *unowned* liege's cut still leaves the vassal. `tax.xml`'s **122 `TaxZone`s** now load (`domainId` → castle), giving `Npc.getTaxCastle`/`getCastleTaxRate`, so the three income paths are live: **merchant purchases** (`RequestBuyItem` charges `price × (1 + castleTax + baseTax)` and pays `handleTaxPayment`; the `BuyList` packet shows the taxed price, and the mercenary manager's `applyCastleTax = false` window keeps Java's display-only exemption), **`applyTaxes` multisells** (the adena ingredient is inflated at display *and* charge time — the rate is latched on `ActiveMultisell` when the window opens, as Java latches it in `PreparedMultisellListHolder` — and only the tax slice is banked), and **manor seed sales** (`addToTreasuryNoTax(totalPrice)`). The **chamberlain vault console** landed with it (`manage_vault`/`_deposit`/`_withdraw` pages behind `CS_TAXES` (ordinal 21), `deposit <n>`/`withdraw <n>`, the not-enough-balance page, `Util.formatAdena` grouping), and the sow-location gate (`seed.getCastleId() == target.getTaxCastle()`) is now honored. New `MaxAdena` character-config key (**9 999 999 999 999** on this dist, not Java's 99.9 B default). 17 tests; the liege cascade, the shop/multisell tax payments, the manor credit and the sow gate all sabotage-verified. **Manor rollover settlement landed** (2026-07-30, on top of the treasury): `advance_manor_mode` now runs Java's `changeMode` economics at each transition. **APPROVED → MAINTENANCE** settles the closing period *before* the roll — crops players actually sold (`startAmount − amount`) are paid into the owner clan's warehouse as **mature** crops at 90 % (with Java's consolation rounding: a payout that truncates to 0 becomes 1 item on `Rnd.get(99) < 90`), and the adena still reserved for unsold crops (`amount × price`) goes back to the treasury — then promotes next → current and **wipes the new next setup when the vault can't cover the just-promoted period** (`getManorCost(castleId, false)`). **MAINTENANCE → MODIFIABLE** sends each owner's online leader `THE_MANOR_INFORMATION_HAS_BEEN_UPDATED`. **MODIFIABLE → APPROVED** charges `getManorCost(castleId, true)` to the vault, or — only when the warehouse has no room **and** the vault is short, Java's `&&`, kept — clears the setup and warns the leader with `YOU_DO_NOT_HAVE_ENOUGH_FUNDS_IN_THE_CLAN_WAREHOUSE_FOR_THE_MANOR_TO_OPERATE`. The rollover is **persisted** through a new `DbCommand::StoreManor` (Java's `storeMe()`; both periods of both tables per rolled castle) — until now the manor state was never written back at all. New `MaximumWarehouseSlotsForClan` config (200 here) backs `ClanWarehouse.validateCapacity`. **Five manor SystemMessageIds were not missing after all** — they live in `SystemMessageId.java`'s `@ClientString` annotations, so the standing "id not in this repo's data" TODOs are closed: 884, 935, 872 (`THIS_SEED_MAY_NOT_BE_SOWN_HERE`, now sent by the sow gate), 1675 (the manor-setup period refusal) and 1491 (the failed crop trade line). 8 tests; the payout, the refund, the consolation roll, the next-period gate, the charge, the warn-and-clear branch, the leader notification and the store are each sabotage-verified. **Mammon economics landed — G26 COMPLETE** (2026-07-30): the Mammon guild trades only in **Ancient Adena**, and its whole economy is multisell-driven — the Priest converts seal stones (6360–6362, dropped by 27 dist NPCs) into Ancient Adena, the Merchant and Blacksmith spend it — so what was actually missing was the **inventory-only (`exc_multisell`) window**, which 9 of the Blacksmith's 13 buttons and every town blacksmith's weapon-SA exchange use. Ported `PreparedMultisellListHolder`'s match-up: a window is now built from **prepared rows** (Java `_entries` + `_itemInfos`), one per list entry normally and one per *unequipped weapon/armor the player holds that an entry names as an ingredient* for `exc` — each row carrying that instance's object id and enchant level, which the `MultiSellList` packet writes and the client echoes back. `MultiSellChoose`'s entry id now indexes those rows (Java does the same), the exchange **destroys the paired instance** rather than any stack of that item id, an item-paired row refuses `amount > 1` or a mismatched echoed enchant (Java's forged-stats guard), and **`maintainEnchantment`** carries the consumed equippable's enchant + augmentation onto the product (list 1005, every town blacksmith). `MultiSellChoose` now parses the echoed enchant level. **`//mammon_find`/`//mammon_respawn` do not exist in this Java build** — they survive only as entries in `AdminCommands.xml` with no handler class (the Seven Signs-era `AdminMammon` is gone), so there is nothing to port. 5 tests; the filter, the equipped-item skip, the paired-instance consumption, the forged-echo refusal and the enchant carry-over are each sabotage-verified. |
| Game  | G26.5 Lottery & Monster Race                                | ✅ **COMPLETE** — **Lottery round lifecycle + persistence landed** (slice 1, plan: [PLAN_G26_5_LOTTERY_RACE.md](PLAN_G26_5_LOTTERY_RACE.md)): `LotteryState` on `World` + `game_loop/lottery.rs` round engine (`on_loaded` boot restore — fresh #1 / finished-row pot carry / live resume; `open_round` next-Sunday-19:00 draw via `siege::next_siege_millis` + row insert + announce; `stop_selling`; `finish_lottery` slice-1 rollover with **no draw**, whole pot carries — number-roll/ticket-match/tiers are `TODO(G26.5)` slice 2). `lottery` table load (`db.rs::load_lottery` + `DbEvent::LotteryLoaded`) + writes (`DbCommand::{StoreLottery,FinishLottery}`); `ScheduledTask::{LotteryStart,LotteryStopSelling,LotteryFinish}`; `AltLottery*` config in `GeneralConfig` (`AllowLottery` gate, dist off). **Slice 2 landed — the whole lottery economics:** the `Loto` NPC dialog (`bypass.rs` `"Loto"` verb → number-pick toggle, buy, jackpot/instructions pages, winning-numbers claim list + direct claim), `LotoPicks` component, verbatim bitmask `encode`/`decode`/`match_count`, ticket 4442 minting (`Inventory::set_lotto_fields`) + adena charge + `increase_prize` (`DbCommand::IncreaseLotteryPrize`), and the **two-phase faithful draw** (`finish_begin` rolls 5 numbers + `LoadLotteryTickets` → `finish_complete` merges offline DB rows with online inventories deduped by object id, tallies tiers, splits the pot verbatim, persists). Claim scores against the boot-loaded `DrawnRound` cache (`LotteryState.drawn`). +2 SM ids (784/930). **A full lottery now runs: buy → draw → payout.** 10 tests, sabotage-verified. **Monster Race foundation landed (slice 3a):** `model/monster_race.rs` (`RaceState`/`HistoryInfo`/`MonsterRaceState` on `World`) + `game_loop/monster_race.rs` pure race math ported verbatim (`roll_speeds` winner-by-total-speed, `calculate_odds` pari-mutuel `max(1.25, pool·0.7/lane)`, `add_bet`) + the `MonRaceInfo` packet (`MON_RACE_INFO` 0xE3). 4 tests, sabotage-verified. **Slice 3b landed — the race runs:** `ZoneKind::DerbyTrack` (geometry-queried, `DerbyTrackZone` → the 8 `zone.xml` derby zones now load), `AllowRace` config, and the 1-second race-cycle state machine (`monster_race::{start,tick}` on a re-armed `ScheduledTask::MonsterRaceTick`): countdown 0 opens a race (8 shuffled packet-only racers 31003–31026 + `roll_speeds` + `MonRaceInfo(SETUP)`), 900 posts odds, 1080 `MonRaceInfo(OFF)`+"they're off", 1085 `MonRaceInfo(MID)`, 1115 records the winner + clears bets, 1140 `DeleteObject`s — all broadcast to Derby-zone players; started at boot. **A race now runs its full cycle and animates.** 8 tests, sabotage-verified. **Slice 4 landed — G26.5 COMPLETE:** `mdt_history`/`mdt_bets` persistence (`DbEvent::MdtLoaded` boot load → `on_mdt_loaded` seeds history/bets/race-number; `DbCommand::{SaveMdtHistory,SaveMdtBet,ClearMdtBets}`; `finish_race` persists + clears in DB) + the `RaceManager` NPC 30995 betting dialog (`race_bypass`: `BuyTicket` multi-step lane→price→confirm→buy with a `RaceTicket` buffer + ticket 4443 mint + `add_bet`; `ShowOdds`/`ShowInfo`/`ShowTickets`/`ShowTicket`; `CalculateWin` pays `bet·(lane==winner?oddRate:0.01)`; `ViewHistory`). +2 SM ids (1044/1046). **A full monster race now runs: bet → race → animate → payout.** 12 tests, sabotage-verified. **G26.5 (Lottery + Monster Race) is done** (only the cosmetic ticket-sale reminder cadence deferred). |
| Game  | G27 Instances                                              | ✅ **Instance engine complete.** **Slice 1 (the instance partition) landed** (plan: [PLAN_G27_INSTANCES.md](PLAN_G27_INSTANCES.md)) — `InstanceId(i32)` component (absent = overworld 0) + `helpers::instance_of`; player visibility (`on_enter_world`, `update_region`) and `broadcast_to_others` now gated on matching instance ids so different instances can't see each other or receive each other's broadcasts. `World.instances` (`InstanceManager`: id allocator ≥ 1 + live registry, `create`/`destroy`). **G25 Olympiad matches now run in their own instance** (per-match `create`, `InstanceId` set on both fighters at arena entry, cleared + instance destroyed on resolve), closing the stadium-overlap gap. **Slice 2 (content visibility) landed** — NPC/door/static/ground-item visibility (`on_enter_world`, `update_region`, `update_npc_region`) and `broadcast_near_region` (now overworld-only) are scoped by instance too, so instanced content is fully private. **Slice 3 (template loader) landed** — `data/instance_data.rs` recursively parses `data/instances/**/*.xml` into `InstanceTemplate` (id, maxWorlds, `<time>` duration/empty, enter/exit locations, doorlist, named spawn groups with spawnByDefault), on `GameData.instance_templates` (verified against the Grassy Arena + Frintezza's tomb). **Slice 4 (instance lifecycle) landed** — `game_loop/instances.rs`: `create_from_template` allocates an instance and spawns its `spawnByDefault` groups into it (tagging each NPC's `InstanceId` + recording it for teardown, on-demand groups stay dormant); `enter` remembers the player's spot and teleports them to the template's enter location; `exit` returns them (ORIGIN → entry spot, or a fixed exit) and arms `ScheduledTask::InstanceEmptyCheck` (`<time empty>` minutes) when the last member leaves; `handle_empty_check` tears the instance down only if still empty (a re-entry spares it); `destroy` ousts any remaining members, despawns the instance's NPCs, and drops its bookkeeping. Reachable via the `AdminInstance` GM commands `//instancecreate <templateId>`, `//instanceteleport <instanceId>`, `//instancedestroy <instanceId>`. **Slice 5 (the `AdminInstance` panel) landed** — `admin/instance.rs` ports the full GM instance UI: `//instance` shows the overview (`instances.htm`: live + template counts), `//instancelist [id=N]` serves the template list (`instances_list.htm`, `IGNORED_TEMPLATES`-filtered, most-populated first) or a template's detail (`instances_detail.htm`: stats + every live instance with Teleport/Destroy bypass buttons), and `//instancecreate <t> [Alone|Party]` moves the chosen group in (CommandChannel collapses to Party — none in Interlude). Templates gained a `name` attr; create/teleport/destroy each redraw the detail page. Faithful dist quirk: the overview's "Show me all templates" button fires `admin_listinstances`, which AdminCommands.xml never registers, so it's inert in retail too — only `//instancelist` reaches the list. Deferred: the per-player instance-reuse view (`AdminInstanceZone`), which needs reuse-time tracking. **Slice 6 (instance-scoped NPC broadcasts) landed** — `helpers::broadcast_near_region_in(region, instance, …)` is the new primitive (`broadcast_near_region` is it with instance 0); the NPC-lifecycle broadcasters that can fire inside an instance now pass the source's instance: `death.rs` (corpse StopMove/StatusUpdate/Die, DeleteObject on despawn — read *before* the despawn drops `InstanceId` — and on region change, NpcInfo on `introduce_npc`, the raid-success message), `combat.rs` (door StatusUpdate, NPC-attacker Attack, NPC HP StatusUpdate + run-toggle), `npc_cast.rs` (MagicSkillUse) and `npc_ai.rs` (all 7 social/move/stop broadcasts). So an instanced NPC's combat/death/cast reaches only same-instance players (test: an instanced NPC's despawn `DeleteObject` reaches the same-instance player but not an overworld player on the same spot). **The G27 instance engine is complete** — partition, visibility scoping, template loader, create/enter/exit/empty-destroy lifecycle, the `AdminInstance` panel, and instance-scoped NPC broadcasts. Remaining as content/feature work, not engine gaps: per-instance door state (`//` open-doors TODO), the `AdminInstanceZone` reuse-time view, and the Frintezza encounter (below). **Frintezza (Last Imperial Tomb) — slice 1 landed** (plan: [PLAN_FRINTEZZA.md](PLAN_FRINTEZZA.md)): the enabling primitives `Instance.status`/`vars` (+ `InstanceManager` get/set) and `instances::spawn_group(id, name)` (spawn a named non-default group into a live instance), plus the native `game_loop/frintezza.rs` state machine + thin `scripts/last_imperial_tomb.rs` QuestScript. GUIDE (32011) first-talk with the Magic Force Field Removal Scroll (8073) builds instance 136 + enters (spawning the default HALL_ALARM group); killing HALL_ALARM (18328) opens room 1 (`spawnGroup("room1")`, `monstersCount = size-1`), and clearing each room advances the crawl `room2_part1 → room2_part2 → status 4` (Java `onKill` 0→4); CUBE (29061) talk teleports out. Verified group names + NPC ids against the real datapack. **Slice 2 (per-instance doors) landed** — instance templates' doorlists now spawn private door copies on creation (Java the instance's own door instances): a new `InstanceDoorOpen(bool)` component gives each copy its own open state (instead of the global `geo.doors` collision atomic), so concurrent instances of a template toggle the same door id independently; `instances::open_close_door(instance_id, door_id, open)` flips the copy + broadcasts `StaticObjectInfo`/`DoorStatusUpdate` scoped to the instance, `door_open_state` reads the copy's flag (else the shared grid), the global `open_door_by_id` skips instance copies, and `destroy` despawns the copies. Frintezza now opens FIRST_ROOM_DOORS / FIRST_ROUTE_DOORS / SECOND_ROOM_DOORS / SECOND_ROUTE_DOORS as the crawl advances. Deferred: per-instance geo *collision* (the copies are visual/logical; the crawl gates on kill-count, not physical blocking). **Slice 3 (intro cinematic) landed** — clearing the arena arms `FRINTEZZA_INTRO_START` (10 min), then a `ScheduledTask::FrintezzaIntro` step machine plays the entrance: earthquake, seal the doors + spawn the teleport cube, freeze the party (`AdminFlags.paralyzed` + stop-move), stage the spawns (Frintezza invulnerable, four immobilized demons, Scarlet, four portraits — object ids stashed in instance vars for the fight), the Mournful Chorale Prelude beat (screen text + Frintezza's cast), then hand control back and set `fightActive`. New primitives: `instances::spawn_npc` (single spawn into an instance) + `instances::broadcast_to_instance` (packet to all members). The player freeze and the staged spawn/hand-back are faithful; the exhaustive ~20-shot dummy-anchored `SpecialCamera` choreography is abbreviated to the establishing beats (TODO). **Slice 4 (Scarlet morphs) landed** — `on_attack(SCARLET1)` crosses the 80% (first-morph cast, skill 5017) and 20% HP thresholds once each (gated on `Npc.script_value` 0→1→2), and the second morph runs a `ScheduledTask::FrintezzaFight` beat that despawns Scarlet1 and raises its final form **Scarlet2 (29047)** at the same spot (frozen, then woken with control returned); `onKill(SCARLET2)` ends the encounter (minimal finish — Frintezza dies + the doors reopen; the death cinematic is slice 5). Also fixed a latent `set_frozen` bug so the intro hand-back actually clears Scarlet's invulnerability (it was spawned invulnerable and would have stayed unkillable). **Slice 4b (fight loops) landed** — at the intro hand-back the encounter arms two recurring `ScheduledTask`s: `FrintezzaSong` (every 90 s Frintezza performs one of five named songs — the 5007 animation + a screen message; the 5008 debuff is a TODO) and `FrintezzaDemons` (every 20 s each still-standing portrait emits a demon, capped at 24 alive via `demonCount`, rescheduling only while a portrait survives). The Dewdrop of Destruction (skill 2276) makes a portrait suicide through `on_attack`, and the demon/portrait `on_kill` branches keep the books (a slain demon frees a cap slot; a downed portrait clears its slot and stops feeding demons). **Slice 5 (finish cinematic) landed** — `onKill(SCARLET2)` cuts Frintezza's song (`MagicSkillCanceled`) and rolls a `ScheduledTask::FrintezzaFinish` step machine: a parting shot of the fallen Scarlet, then Frintezza's death ~7.4 s in (invulnerability dropped, `Die` broadcast + despawn), then the doors reopen and `cleared` is set so the party can walk to the exit cube (spawned back in the intro). **Frintezza is now playable end-to-end** — enter → clear three rooms → intro cinematic → fight Scarlet through both morphs amid songs and portrait-spawned demons → kill the final form → finish cinematic → doors reopen → exit. **Scarlet's daemon-skill AI landed** (Java `ScarletVanHalisha`) — its combat skills (5014 attack / 5015 charge / 5016 yoke / 5018 morph / 5019 field) aren't in Scarlet's template, so a `ScheduledTask::ScarletSkill` tick (armed on the first blow for both forms, stopped on death or fight-end) picks a skill by the per-form probability table and `start_cast`s it at a random in-range player, skipping while casting/invulnerable, with the two ranged skills honouring the 1-minute cooldown. **Crawl polish landed** — the room aggro-nudge (Java `reduceCurrentHp(1)`: a freshly-spawned room `add_hate`s every guard onto the intruder so they attack at once), the 5% Dewdrop drop (`world.roll(100) < 5` drops item 8556 at the slain trash), and the song debuff (each song now applies skill 5008 at its rolled level to every player via `apply_skill_effects`, alongside the 5007 animation). **Frintezza is functionally complete** — the only remaining gap is cosmetic (the exhaustive dummy-anchored `SpecialCamera` choreography is abbreviated throughout). |
| Game  | G28 Events engine & cursed weapons                          | ✅ **cursed-weapon gate MET** (plan: [PLAN_G28_CURSED_WEAPONS.md](PLAN_G28_CURSED_WEAPONS.md)): the autonomous cursed-weapon loop landed (`game_loop/cursed_weapon.rs`) — a slain ordinary monster drops one via `CursedWeaponsManager.checkDrop` (killer's acting player must be an un-cursed real player; `Rnd.get(100000) < dropRate`; new `DropSource::CursedWeapon` exempts it from auto-destroy per Java's `setDropTime(0)`; `RedSky`+`Earthquake`+`S2_WAS_DROPPED_IN_THE_S1_REGION`, life task armed at `now + duration`), picking it up curses the finder (`activate` reuse, intercepted in `pickup_ground_item`; already-cursed picker consumes the duplicate), and `ScheduledTask::CursedWeaponExpiry` end-of-lifes it on the deadline (`RemoveTask`, stale-timer guard). New `CursedWeapon.dropped_item_oid` + SM id 1815. 10 tests, all three wires sabotage-verified. **Login restore landed (2026-08-01):** `cursed_weapon::on_enter_world` (Java `CursedWeaponsManager.checkPlayer` + `CursedWeapon.cursedOnLogin`) wired into `lobby::handle_enter_world` right after the spawn broadcast — the curse flag, `giveSkill`, the 301/302 `doTransform`, the `S2_S_OWNER_HAS_LOGGED_INTO_THE_S1_REGION` announce and the time-left notice; without it a relog silently lifted the curse (no transform, no skill, every `isCursedWeaponEquipped()` gate false). Alongside it: the `RemoveTask` is re-armed at boot (`reActivate` — a restored curse was otherwise immortal), `EnterWorld`'s "remove demonic weapon if not cursed weapon equipped" sweep, the `UseItem` (hand slots + formal wear 6408) and `RequestUnEquipItem` (`SLOT_LR_HAND`) locks that stopped a wielder swapping the sword out, and a **corrected SM id** — `S2_WAS_DROPPED_IN_THE_S1_REGION` is 1815, not 1817 (1817 is the *login* line, so the drop announce was rendering the wrong text). Also the **offline `endOfLife` branch** (`DbCommand::RestoreOfflineCursedOwner`), which the boot-armed timer makes reachable: a curse that runs out while its owner is away deletes the weapon row and puts back their reputation/pk-kills, plus — no Java counterpart, because Java's `addSkill(…, false)`/`addTransformSkill` never persist while this port stores the whole `SkillBook` — deletes the cursed and 301/302 transform skills. 8 more tests, all five gates sabotage-verified. **Java parity pass (2026-08-01):** `increaseKills` (kill tally -> PK counter, stage-boundary skill level-up, `durationLost` burned off the deadline with a re-arm, since this port uses a one-shot task rather than Java's fixed-rate poll), the `onPlayerKill` hook placed **ahead of** the olympiad/duel/siege/PVP-zone bails with Java's early return (a cursed kill awards no pvp kills and no karma), `dropIt(killer)` on wielder death driven through the real `player_do_die` wire (`Rnd.get(100) <= disapearChance` — the `<=` makes 50 a 51-in-100 chance), the "cannot own 2 cursed swords" stage bonus, the `EXPELLED` party removal, shutdown `saveData()`, and `end_time` inheritance (Java's `activate` never sets `_endTime`, so a ground pickup inherits the drop's remaining life while `//cw_add` starts a fresh one — which moved task-arming out of `activate` to its callers). Gates added: Say2 trade/shout, private store buy + sell, augment, destroy-item, olympiad registration, `PlayerAppearance.getVisible*` pledge blanking (clan id, both crests, ally id + crest report 0), mutual level-21 attack protection. Deliberately **not** ported: `MaxHpFinalizer`'s HP-limit lift and `PlayerStat`'s karma-recovery exemption — neither mechanism exists in this port (no `Config.MAX_HP` cap, no karma-on-XP-gain), so they would be dead code. 17 cursed tests, every wire sabotage-verified — including the death wire, which a direct-call test had left uncovered. Still open: ground-item persistence across restart, region-name SysString, `controlPlayers()` boot sweep. **Deferred (TODO(G28)):** kill-count level-up/stage bonus, "hungry" HP decay, drop-on-PK-death, region-name SysString, pickup end_time preservation. **Events engine — slice 1 landed** (plan: [PLAN_G28_EVENTS_ENGINE.md](PLAN_G28_EVENTS_ENGINE.md)): the event lifecycle + TvT registration phase. `EventManager`/`TvtState` (`model/event.rs`, a `World` field); `game_loop/events/{mod,tvt}.rs` (name-dispatched `start`/`stop` + TvT `event_start`/`event_stop`/`teleport_to_arena`); `scripts/tvt.rs` (manager NPC 70010 first-talk → register/cancel window, `Participate`/`CancelParticipation` bypass buttons, `canRegister` gates whose state exists — level 76–200/already-registered/max-count/cursed-weapon/reputation/olympiad/fishing, `TODO(G28)` the rest); `//event_start`/`//event_stop` GM trigger (the dist `config.xml` schedule ships commented out); `ScheduledTask::TvtTeleportToArena` registration-close timer (fully implements the too-few-participants cancel; enough-players → arena is the slice-2 `TODO(G28)` stub); `ChatType::Announcement`; `Player.on_event`/`registered_on_event`. 6 tvt tests, sabotage-verified. **Events engine — slice 2 landed** (arena stand-up): `teleport_to_arena`'s enough-players path creates the coliseum instance (template 3049), shuffle-splits BLUE/RED (strict alternation from a random side), teleports each into their team spawn (`instances::enter` + `teleport_player`), sets `team`/`on_event`, spawns the two buffers, broadcasts `ExPVPMatchCCRecord::INITIALIZE` (new packet, opcode `EX_PVP_MATCH_CCRECORD` 0x8A), and arms `StartFight`; `start_fight` opens the arena doors + "The fight has began!" + arms `EndFight`; `end_fight` (slice-2 minimal) announces the end and tears the arena down via `instances::destroy` (ousts players to ORIGIN, despawns NPCs/doors), clearing team/event flags. New `TvtPhase::{Warmup,Fighting}` + `ScheduledTask::{TvtStartFight,TvtEndFight}`; `event_stop` destroys a live arena. 10 tvt tests (a full event now runs start→finish). **TODO(G28) deferred to slice 3/4:** parties-of-7 + command channels + death listeners (need a CC primitive), the countdown screen messages, and the real `EndFight` — freeze/revive/winner firework+adena/tie + `ExPVPMatchCCRecord::FINISH` + 7s delay. **Events engine — slice 3 landed** (scoring + respawn): `on_player_death` (hooked into `death::player_do_die`, no-op off-event) scores cross-team kills — killer's side + personal tally, "Blue: X - Red: Y" screen tally + `ExPVPMatchCCRecord::UPDATE` — and queues the victim's respawn (`ScheduledTask::TvtResurrect`, 10s); `resurrect_player` revives the still-dead participant at their team spawn behind the Ghost Walking invuln (skill 100000, `DamageBlock` HP/MP, ported G19). 15 tvt tests (incl. one through the real `player_do_die` wire). **Events engine — slice 4 landed** (EndFight + forfeit + logout): real `end_fight` (`TvtPhase::Ending`) closes the doors, freezes (`AdminFlags.invul`) + revives participants, resolves BLUE vs RED — winner firework (skill 5965) + cheer + Adena 57×100000, tie shrug — then arms `ScoreBoard` (3.5s → `ExPVPMatchCCRecord::FINISH`) and `TeleportOut` (7s → unfreeze + `instances::destroy`); `on_player_logout` (hooked into `net::handle_logout` + `on_disconnect`) drops a leaver and `manage_forfeit`s (early `EndFight`, `Ending`-guard makes the original timer a no-op) when a team empties. New `ScheduledTask::{TvtScoreBoard,TvtTeleportOut}` + `TvtPhase::Ending`. **A complete TvT match now runs end-to-end with a real winner + reward.** 20 tvt tests. **Still TODO(G28) (polish):** enemy-HQ zone kicks + inactivity timers (need `on_enter_zone`/`on_exit_zone` hooks + `colosseum_peace1/2` zones), manager `BuffHeal`, parties-of-7 + command channels, countdown screen messages. **Remaining G28:** optional cron auto-schedule (slice 5) + the deferred polish. Cursed-weapon activation engine + `//cw_*` GM commands already landed (G21). **Fix (2026-08-01) — the curse's stat pumps never applied, then never left:** the weapon's own skill (Zariche 3603 / Akamanah 3629) is a *passive* and is most of what wearing the curse is (3629 L1: `MaxCp` ×11.5 +1300, `PAtk`/`MAtk`/defence PER+DIFF, `VampiricAttack`). `give_skill` only inserted it into the `SkillBook`, and `removeSkill` only deleted it — neither touched the effects, so **taking** the curse changed no stat, and **losing** it left the modifiers applied. Worse, they were invisible until the moment the curse *ended*: max HP/MP/CP are cached on `Vitals`/`PlayerVitals` rather than derived per read, so the pumps sat unnoticed in `StatModifiers` until `remove_transform`'s `recompute_max_vitals` finally folded them in — the reported "lost Akamanah, untransformed, still MAX CP 3844" (= 225.75 base CP × 0.98 CON bonus × 11.5 + 1300 for a level-21 human wizard, i.e. exactly 3629 L1 still on). Three changes: `give_skill` now runs `refresh_conditioned_passives` (Java's `addSkill` → `EffectList`); `recompute_conditioned_passives` follows its apply phase with `recompute_max_vitals`, so **any** passive carrying `MaxHp`/`MaxMp`/`MaxCp` moves the bar when it lands or drops (level-up learns, robe swaps, not just this); and both `removeSkill()` sites (`end_of_life` and the death `dropIt`) go through a new `skills::remove_player_skill` — Java's `Player.removeSkill` → `EffectList.stopSkillEffects` — because `recompute_conditioned_passives` deliberately only manages passive buffs whose skill is *still in the book* (clan skills and expertise penalties are book-less passive buffs that must not be swept), so a skill that just left it was invisible to that diff. 2 tests, both halves sabotage-verified. **Panel usability (2026-08-01, deliberate deviation from Java):** `cwinfo.htm` draws its buttons from the weapon's live state, and Java's `useAdminCommand` returns without touching the window — so after `//cw_add` the page still offered `Give to Target` and could not remove the sword it had just handed out; the GM had to leave and re-open `//cw_info_menu`. `//cw_add` and `//cw_remove` (and the already-active refusal, which is by definition a stale page) now redraw the panel from the state they just changed. `//cw_goto` is left alone — it teleports and changes no weapon state. 1 test, sabotage-verified. |
| Game  | G29 Summons, pets, servitors, cubics, agathions             | ✅ **Servitor summoning landed** (plan: [PLAN_G29_SERVITOR_SUMMON.md](PLAN_G29_SERVITOR_SUMMON.md)): the first G29 slice, and it closes the **single biggest unported effect on the whole ranking** — `Summon`, 24 learnable skills (Dark Panther 283, Kat the Cat 1111, Shadow 1128, the golems, …), every one of which cast and produced nothing. **Design decision:** a servitor is an ordinary NPC entity marked with a `ServitorOf` component rather than its own `Creature` subclass as in Java — it already *is* a template + stats + position + AI, so the only genuinely new state is the owner link, the summoning skill and the lifetime; that keeps servitors inside the existing spawn/region/visibility/combat machinery. `Player.getServitors()` is a scan, not a cached index (at most one servitor per player here). Re-casting **swaps** rather than stacking (Java unsummons first); `lifeTime <= 0` is Java's no-expiry case (`Integer.MAX_VALUE`, "Classic hack. Resummon upon entering game."); `npcId` is declared **per skill level**, so each level summons a stronger template. Ported `PetSummonInfo` (`PET_INFO` 0xB2), the ~50-field flat packet the **owner** sees, with the servitor's remaining lifetime in the fed/max-fed pair that draws its time bar. **Servitor follow & attack landed** (plan: [PLAN_G29_SERVITOR_AI.md](PLAN_G29_SERVITOR_AI.md)) — **the first gate clause is met**: a summoned servitor now follows its owner and attacks on command. Java's `SummonAI` is a `PlayableAI`, **not** an `AttackableAI`, and that distinction is the design: a servitor trails its owner when idle and **never scans for prey** — it fights only what the owner points it at, through the action bar (`ServitorAttack` 22 / `ServitorStop` 23 / `ServitorHold` 21, delivered by the new `RequestActionUse` 0x56). The NPC think dispatch branches on `ServitorOf` before the `AttackableAI` state machine, which is what stops a servitor hunting on its own (pinned by standing a monster next to one for 200 ticks). An ordered attack seeds hate and flips the intention — the same primitive `GetAgro`/`Confuse` use, since this port's NPC AI re-derives its target from the aggro list each think — and clears the follow flag, or the servitor would drift home between swings. Java's 3000-unit bail (a target further than that from the owner falls back to following, so a stray click can't send the summon across the map) is ported. Following reuses `npc_ai::move_npc_to`, inheriting G21's geodata/pathfinding. **Test trap recorded:** three tests failed at first because the sparring dummy sat at `NPC_OID` — a servitor is spawned through the *runtime* allocator, which starts at `FIRST_NPC_OBJECT_ID`, so the fixture NPC silently replaced it; the same collision `add_test_npc` already warns about, but in a new guise (it bites whenever a test spawns at runtime *before* placing a fixture). **`SummonInfo` landed** (plan: [PLAN_G29_SUMMON_INFO.md](PLAN_G29_SUMMON_INFO.md)) — other players can now see a servitor, closing what was the most glaring gap left by slice 1 (a summon visible only to its owner). **It was far cheaper than sized:** the 338-line Java class uses the **same `NpcInfoType` 37-bit mask format the port already implements for `npc_info`**, helpers and two-block size accounting included, so the real work was the summon-specific component set rather than the mask machinery — a calibration note worth keeping: check whether a big Java packet shares a format the port already has before pricing it. Differences from `NpcInfo`: opcode 0x8B, `TITLE` always present and carrying the **owner's name** (what draws the "of X" label — its own test searches the encoded packet for it), `PVP_FLAG` always present, `NAME` when `displayId != id`, and `SUMMONED` for the spawn animation. Wired at **both** introduction points (enter-world and the region-delta path) so a servitor walking into view is introduced the same way as one already there, with the **owner excluded everywhere** since they hold the `PetInfo` view — Java splits the two the same way. Left at Java's defaults: `relation` (the per-viewer PvP relation isn't resolved at this call site), clan crests, team, reputation, water/fly, enchant, transformation. **Servitor lifecycle landed** (plan: [PLAN_G29_SERVITOR_LIFECYCLE.md](PLAN_G29_SERVITOR_LIFECYCLE.md)): with slices 1-3 a servitor existed, followed, attacked and was visible — what it could not do was **end**. The lifetime was recorded but never enforced, the upkeep item parsed but never charged, and logging out left an ownerless NPC in the world. Ported Java's fixed **5-second** `Servitor.run()` as a self-rescheduling `ServitorLifeTick` (same "dead or gone → stop" contract as the DoT chain): lifetime countdown → "Your servitor passed away" + unsummon; the periodic upkeep item (default **240 s**, 60 for siege weapons) → "a summoned monster uses X" on payment or "not enough items to maintain the servitor's stay" + unsummon on failure; `SetSummonRemainTime` (0xD1, new) for the time bar; and the **2000-unit leash** — which matters more than it looks, because an ordered attack clears the follow flag, so without it a servitor sent at a distant target would simply be abandoned there. Unsummon-on-leave is wired into `net::store_and_remove_player`, covering logout *and* disconnect. **Honest narrowing:** Java stores a servitor in `CharSummonTable` and restores it on reconnect; persistence is a later slice, so for now it goes away with its owner — a behaviour difference, not a bug, and strictly better than the ownerless NPC it replaces. **PetData loader landed** (plan: [PLAN_G29_PET_DATA.md](PLAN_G29_PET_DATA.md)) — a **foundation slice**, stated plainly: it loads the 56 pet templates from `data/stats/pets/*.xml` but does **not** summon a pet. A pet's stats, food item, hunger limit and food capacity come from `PetData` rather than its NPC template, and the summon is keyed by the **collar item** (`itemId` → `npcId`), so the table has to exist before anything can be summoned from it. Two parsing details worth naming: species-wide and per-level `<set>` elements **share a tag name**, separated only by being inside `<stats>` (a test asserts they don't bleed into each other — reading `food` into a level row would be silent and wrong), and `max_meal(level)` clamps to the table's top row like Java. Per-level combat stats are parsed but not yet consumed; the NPC template's stats stand in until pet levelling lands. **Deliberately deferred to the summon slice:** the collar→cast binding — Java's `SummonPet` effect never receives the item, the `SummonItems` handler stashes a `PetItemHolder` on the player and the effect pulls it back out, and this port's `use_item_skills` has no equivalent "this cast came from item X" channel, so that is genuinely new plumbing rather than something to bolt onto a loader. Persistence is also its own slice (a pet's identity is the collar's **object id**, which is how two collars of the same kind stay two different pets; the `pets` table already ships in the dist schema, so it is query work, not migration work). **Pet summoning landed** (plan: [PLAN_G29_PET_SUMMON.md](PLAN_G29_PET_SUMMON.md)): a collar now summons its pet, which follows and is visible to everyone. **The collar→cast channel** was the piece the data slice stopped short of — Java's `SummonPet` effect never receives the item, so `SummonItems` attaches a `PetItemHolder` to the player and the effect pulls it back out; ported as `Player.pending_pet_collar`, set in `use_item_skills` and **taken** (not copied) by the effect so an unused one can't linger into an unrelated cast. **A pet is a servitor plus a collar:** the owner link, follow state and AI all come from `ServitorOf`, which a pet also carries — "owned summon" is the same relationship whether it came from a skill or a collar, so pets inherit follow, attack, stop/hold and the leash for free; `PetOf` holds only the collar object id and the food bar. A pet sets life-time/upkeep to "none" (it is fed instead), so the lifecycle tick leaves it alone. **The collar's object id is the pet's identity** (Java's `pets.item_obj_id`), not the item type — that is how two Wolf Collars stay two different wolves. `summonType` is load-bearing: `PetInfo`'s second byte is 1 for a pet and 2 for a servitor and the client uses it to decide whether to offer the pet inventory and food bar, so one test summons each and reads the byte; the same field pair carries a pet's **food bar** and a servitor's **remaining lifetime**, which is Java's own reuse. **Still open for pets:** persistence (the `pets` table, already in the dist schema, keyed by the collar object id — the gate's "and it persists"), feeding (`PetOf.fed` is tracked and displayed but nothing drains or refills it), pet inventory, exp/level and evolution. ~~**Still open:** see the servitor (needs `SummonInfo` 0x8B…), it does not follow or attack…, no unsummon-on-logout, item consumption, master-buff inheritance or persistence.~~ — **that whole list is stale**; the later G29 slices landed it. **[Verified 2026-07-31]** — both gate clauses met: a servitor **follows and attacks** (`SummonInfo` ported, owner-order AI), and a pet is **summoned, fed and persists** (`pets` table round-trip pinned by `char_persistence::pets_persist`; feeding is an item-skill effect; pet inventory and `add_pet_exp` are in). ~~**One genuine gap found by this pass: pet evolution.**~~ **LANDED 2026-07-31** (`game_loop/pet_evolve.rs`) — all three `PetManager` verbs, which the port was already serving pages for. **`exchange <n>`**: ticket → collar (3 pairs). **`evolve <n>`** (`Evolve.doEvolve`): a *summoned, living* pet of the right species and level becomes its evolved form — old collar **and its saved `pets` row** destroyed (Java's `destroyControlItem(owner, evolve = true)`, or the new pet would inherit the old one's stored state), new collar added, pet re-summoned carrying its experience and name, placed where the old one stood, and the collar stamped with the new level. **`restore <n>`** (`doRestore`): works off an **item**, not a live pet, and reads the pet's level out of the **collar's enchant** — the one place the summon path records it. **Java's exp floor is load-bearing**: the carried exp is floored at the *new* species' exp-for-`petminLevel`, which is what stops a qualifying pet landing 45 levels down when the new curve starts lower than the button's minimum. The evolved pet is built by **seeding the saved row and re-using `summon_pet`**, not by a parallel construction — so it is identical to a re-summoned pet (stats, feed clock, packets). **Dist finding: `restore` is unreachable on this dist** — its page is `36478.htm` and **npc 36478 has no spawn**; `evolve`/`exchange` hang off Lundy (30827), who is spawned. The verb is ported anyway since it lives on the shared `PetManager` class. **A real infinite loop was caught by the test run**: `PetTemplate::level_row` clamps *up* past the top row while `exp_for_level` is an exact lookup returning 0, so a `while level_row(n+1).is_some() && exp_for_level(n+1) <= exp` walk never terminates — the level is now derived by scanning the table's keys. 5 tests; the bypass wiring, the exp floor, the level/species/dead gates, the collar enchant stamp, the ticket check and the restore level-read were each sabotage-verified — **two of them initially passed under sabotage** (the floor was masked by `summon_pet`'s own re-floor, and the wrong-species arm was refusing on a missing pet-data lookup instead) and were rewritten until they failed. 18 `TODO(G29)`s remain (mount feed task, cubic count stat, servitor tails). | ⏳ editchar summon/pet subcommands **Pet persistence landed** (plan: [PLAN_G29_PET_PERSISTENCE.md](PLAN_G29_PET_PERSISTENCE.md)): the `pets` table (already in `dist/db_installer/sql/*/game/pets.sql`, though absent from the consolidated dump) now loads with the character into a `PlayerPets` component keyed by collar object id, and writes back through `servitor::sync_pet_row` on every flush and on owner-leave; level/exp/sp/fed/vitals all round-trip. Upsert-per-row rather than the usual delete-then-reinsert reconcile, because a row is keyed by a collar the character can trade away; rows are deleted only when the collar is destroyed (Java `RequestDestroyItem`), which also unsummons the bound pet — object ids are recycled, so an orphan row would eventually bind a stale pet to an unrelated item. Java's exp floor ("avoiding pet delevels"), the Sin Eater's summon-at-owner-level rule and `getPetMinLevel` clamp are ported; the food bar deliberately does **not** refill on summon. `restore` is always written false (auto-resummon needs `CharSummonTable`, `TODO(G29)`). Caught two latent bugs: `PlayerPets` was declared on `PlayerData` but never added to the component insert bundle (silent no-op in production), and `tests/user_info_packet.rs` had been failing to compile on `main` since G19 resurrection + slice 6 added `Player` fields — `--lib` filters never build the `tests/` directory. Next: **feeding** (`PetFood` handler + consumption tick) closes the gate. **Pet feeding landed — G29 gate clause "summon a pet, feed it, and it persists" now met** (plan: [PLAN_G29_PET_FEEDING.md](PLAN_G29_PET_FEEDING.md)): feeding runs through the item's `NORMAL` item-skills, not a flat value — item 2515 → skill 2048 → `<effect name="Feed"><normal>100</normal>`, so a new `SkillEffect::Feed` variant + parse arm was required (7 Feed instances, 9 `PetFood` items; without it food was consumed for nothing). `PetFoodRate` is now a real `Rates.ini` key. Because Java's `PetFood` refuses an unmounted *player*, food can only reach a pet through its own bag, so this slice also ports `PetInventory` (`ItemLocation.PET`, keyed by the **owner's** object id like Java, so it persists through the existing item reconcile), `RequestGiveItemToPet` (0x95), `RequestGetItemFromPet` (0x2C), `RequestPetUseItem` (0x94) and `PetItemList` (0xB3). The 10 s `PetFeedTick` burns the normal/battle rate, floors at zero, auto-eats when below `hungryLimit`%, and nags/starves per Java; `setCurrentFed` clamps at `maxMeal`. Kept Java's quirk that two collars share one pet inventory (no per-pet discriminator on the rows). Added one datapack-backed test asserting the real skill 2048 parses `normal == 100`, since the feeding fixtures hand-build their own skill and would pass through a broken parse arm. `ItemTemplate` gained `Default`. servitor_tests 41 → 51. **Cubics landed** (plan: [PLAN_G29_CUBICS.md](PLAN_G29_CUBICS.md)): chosen over agathions by the learnable-skill ranking — `SummonCubic` has 28 skills / **12 learnable**, `SummonAgathion` 166 / **0** (all off every skill tree), so raw counts would have pointed at 6x the work for unreachable content. `CubicData` loader (207 templates), `SummonCubic` effect, a `Cubics` component (a cubic is **not** a world object — no template/position/AI, so it can't be targeted), and the `CubicAction` tick: cumulative `triggerRate` skill choice, `successRate` rolled after the choice, owner `<hp>` gate, `<range>`, target `<healthPercent>` band, and TARGET/HEAL/MASTER/BY_SKILL target types. `maxCount` counts *actions* not attempts (no charge spent on a failed roll, missing target, dead target or out-of-range). Java's `scheduleAtFixedRate(..,0,delay)` fires immediately on summon. **Fixed a second hard-coded-zero count in `CharInfo`** (`cubic count`, the same shape as the G19 abnormal-visual bug) — cubics were invisible to other players; added `visibility::refresh_char_info`. `MAX_CUBIC` is always 1 on this dist (nothing sets `cubicCount`). cubic_tests 13 + 2 datapack-backed parser tests. **Client-visibility sweep** (plan: [PLAN_G29_CLIENT_GAPS.md](PLAN_G29_CLIENT_GAPS.md)): after the cubics slice found the *second* hard-coded-zero count in a packet builder, ran the check deliberately across all of them. Two live regressions — features that landed in an earlier milestone but never reached the client because the packet was stubbed before them: **`PartySmallWindowAll` summon count** (pets/servitors exist since slices 1-8; now writes Java's per-summon block — object id, `npcId+1000000`, the 1=pet/2=servitor discriminator, name, HP/MP, level) and **`ExSubjobInfo` subclass count** (subclasses landed in G17; Java puts the **base class first** so the count is never 0 even with no subclasses — the client's class list was empty for everyone). Three other zero counts verified as genuinely-absent features; the dead `enter_world::henna_info` stub (superseded by the real `HennaInfo`) deleted. Also replaced the `pet_of`/`servitor_of` store sweeps with a `SummonRef` link on the owner — **closer to Java** (`getPet()` is a field read, not a scan), O(1), and readable from `&World`, which is what the packet builders have; ids are validated on read so a missed clear yields `None`, not a dangling id. servitor_tests 51 → 54 (all 51 pre-existing passed unchanged through the refactor). **Cubic `power` fix** (addendum in the cubics plan): the previous slice flagged template `power` as "parsed but unconsumed" — checking the Java showed the port was consuming the **wrong** thing. `Cubic extends Creature` with `getBasePAtk()/getBaseMAtk()` = `power / 10`, and casts via `skill.activateSkill(this, target)`: **the cubic is the caster, not the owner**. The port passed the owner, so cubic damage scaled off the player's m.atk (Storm Cubic lvl 1 is power=282 → m.atk 28.2, vs a levelled mage's several hundred). Fixed with a stats-only caster entity — `CombatStats`/`Vitals`/`Position` but no `Npc`/`Player`/`RegionCell`/`Movement`, which every store sweep is anchored on, verified by enumerating the `for_each_mut` call sites — despawned with the cubic. Found two more bugs while fixing it: `Cubic.getLevel()` delegates to the **owner's** level (without it every cast resisted and cubics did zero damage — new `CubicOf` component), and `add_components` silently no-ops on an unspawned id (`spawn` first). cubic_tests 13 → 16, incl. one asserting a 500x swing in owner m.atk leaves cubic damage identical. **Pet exp + levelling landed** (plan: [PLAN_G29_PET_EXP.md](PLAN_G29_PET_EXP.md)): slice 7 made level/exp/sp round-trip but nothing awarded them, so every pet stayed at its summon level. A nearby pet's cut comes **out of** the owner's award, not on top — `get_exp_type` (73) is the share the *owner keeps*, the pet takes the remainder, split after the vitality/premium bonuses so it shares them. **A starving pet earns nothing** (`isUncontrollable()` guards `PetStat.addExp`) — a real link between the feeding loop and progression. Levelling advances through every earned level at once, caps at the species table's top level, moves `max_meal` with it, sends no system message (just `SocialAction(LEVEL_UP)`), and stamps the pet's level onto the **collar's enchant level** (`getControlItem().setEnchantLevel`) — which was a separate remaining-work item and turned out to be three lines here. servitor_tests 54 → 62, incl. an end-to-end test through the real reward path (owner keeps 1000 alone vs 730 with a pet in range). Next: per-level pet **stats** are still parsed-but-unread, so a levelled pet's level moves but it doesn't get stronger. **Per-level pet stats landed** (plan: [PLAN_G29_PET_STATS.md](PLAN_G29_PET_STATS.md)): slice 12 levelled pets but combat still read the NPC template, so a levelled pet's number moved while it stayed as strong as at level 1. Following the cubic-`power` lesson, checked *who consumes* the columns first: Java overrides at the **finalizer** level (`MaxHp`/`MaxMp`/`PDefense`/`MDefence`/`calcWeaponBaseValue`/`Regen*`), uniformly substituting the per-level pet row wherever an NPC would use its template base. Ported as `pet_template_at_level` — clone the template with the pet row's stats **and the pet's own level** (which drives `levelMod`) patched in, then reuse the existing `npc_finalized_stats` pipeline, rather than growing a parallel pet stat path that would drift. Levelling preserves the HP/MP **fraction** (a refill would be a free heal; an absolute carry would wound the pet as max HP rose). A row missing a stat falls back to the NPC template — not speculative: without it the shared fixture produced pets at **0 max HP**. `org_hp_regen`/`org_mp_regen` parsed but still unread (`NpcTemplate` has no regen fields — its own slice). servitor_tests 62 → 66, pet_data 2 → 3 incl. datapack-backed assertions on the shipped Wolf's exact stat values and that its top level is strictly stronger than level 1. **Pet death landed** (plan: [PLAN_G29_PET_DEATH.md](PLAN_G29_PET_DEATH.md)), closing the `TODO(G29)` slice 7 left at the restore site: `deathPenalty` (`-0.07×level + 6.5` percent of the **current level's band**, so it shrinks as the pet levels; skipped for duel/arena deaths), `_expBeforeDeath` captured pre-penalty and **not persisted** (Java holds it on the live instance), `restoreExp(percent)` handing back a share and zeroing the record so a second revive restores nothing, a floor so the penalty can't de-level, and a zero-penalty no-op at the species cap where there is no next-level band. A pet stored with `curHp < 1` now restores as a corpse. **Fixture bug found:** the first draft reported "exp lost (6000 → 6000)" — the shared fixture had only two levels, so a level-2 pet was already capped and every death test measured the empty-band case; fixed with a third level and the cap case pinned separately. **Bug found incidentally:** `YOUR_SERVITOR_PASSED_AWAY` was 1519 (written in slice 1) but is **1520** — 1519 is "The pet has been killed…", so expiring servitors told owners their pet had died. servitor_tests 66 → 73; the duel test also puts `is_in_duel` to use, clearing a long-standing dead-code warning. **Pet resurrection landed** (plan: [PLAN_G29_PET_REVIVE.md](PLAN_G29_PET_REVIVE.md)), closing slice 14's dangling `pet_restore_exp` (wired and tested but called by nothing). Java's `Resurrection` calls `effected.getActingPlayer().reviveRequest(…, effected.isPet(), …)` — `getActingPlayer()` on a pet returns its **owner**, so the `ConfirmDlg` goes to the owner, who answers for it; one `_reviveRequested` block on the player carries both cases via `_revivePet`. Ported by turning the five-element proposal tuple into a named `ReviveRequest` struct with the flag. **`PcBody` was rejecting pets** (`targethandlers/PcBody.java` is `!isPlayer() && !isPet()`; the port had only the player half), so a dead pet could not be targeted at all. A pet's restorable exp is the gap the death penalty opened, not `lost_exp_on_death`, so the dialog's number branches on the flag. Reviving restarts the food clock and syncs the pet row. servitor_tests 73 → 78, incl. one pinning that a pet revival does **not** revive a dead owner; all 10 player-resurrection tests passed unchanged through the struct conversion. **Pet corpse decay landed** (plan: [PLAN_G29_PET_DECAY.md](PLAN_G29_PET_DECAY.md)). Slice 15 closed by claiming the corpse "persists indefinitely" and needed Java's 24-hour timer — **both halves were wrong**, and the datapack caught it: `npc_do_die` already schedules decay, `DecayTaskManager.add` has **no pet branch**, no pet NPC template overrides `corpseTime`, and `DefaultCorpseTime = 7`, so Java also decays a pet corpse after **7 seconds**. The "24 hours" in the death message is flavour text that contradicts the mechanic; trusting it would have replaced faithful behaviour with a divergence. The real gap was what happens *at* decay: `Summon.onDecay` → `Pet.deleteMe` transfers the pet's inventory to the owner, then **`destroyControlItem`** — letting a dead pet rot **destroys it permanently** (collar consumed, row deleted). Previously a decayed corpse just despawned, so death cost only the exp penalty and the pet could be re-summoned free. servitor_tests 78 → 82, incl. the slice-15 interaction (resurrecting before decay saves the pet; the decay task fires anyway and must no-op) and a guard that servitors don't take the pet path. **Pet regen landed** (plan: [PLAN_G29_PET_REGEN.md](PLAN_G29_PET_REGEN.md)). The carried-forward claim that `NpcTemplate` "has no regen fields at all" — repeated across three plan docs and three PROGRESS rows — was **false**: the fields are `base_hp_reg`/`base_mp_reg` and `run_npc_regen_tick` already read them; the original grep said `hp_regen`. Second wrong carried-forward TODO in three slices (after the corpse "24 hours"). The real change is ten lines: Java's `RegenHPFinalizer` pet branch substitutes the per-level pet row's regen under `PetHpRegenMultiplier`/`PetMpRegenMultiplier` (now real config keys, 100/×1.0 here — inlining 1.0 would be invisible today and wrong for a retuned server, and a monster-regen retune must not retune pets). Lives in the regen tick rather than `pet_template_at_level` because regen re-reads the template each tick instead of caching. servitor_tests 82 → 86 (incl. a test that sets the monster multiplier to 100× to prove it does *not* apply to pets) + datapack assertions that the shipped Wolf's regen is 2.0 at level 1 and grows. **Summon shots landed** (plan: [PLAN_G29_PET_SHOTS.md](PLAN_G29_PET_SHOTS.md)): the autoshot handler carried an explicit "summon shots aren't in scope" narrowing and `soulshot_count` was unparsed, so pets could not use shots. Java `Summon.rechargeShots` reads the **owner's** auto-shot list, spends from the **owner's** inventory and charges the **summon** — three actors in one flow. Cost is the pet's **per-level** `soulshot_count`, so a levelled pet is more expensive to keep shotted. Java's `isSummonShot` branch checks `hasSummon()` and **never looks at the player's weapon**, so reusing the player grade check would have rejected every Beast Soulshot; it also charges the summon immediately on toggle. `_chargedShots` lives on **Creature** in Java, so NPC attackers were skipping charge/spend entirely — added a `ChargedShots` component for summons (`TODO(G29+)`: fold the player's bits in). A partial stack buys nothing rather than a partial charge. servitor_tests 86 → 92 + datapack assertions that the shipped Wolf's shot cost is 1 at level 1 and grows. Spiritshots parse but stay unwired until pets cast. **`SUMMON` target type landed** (plan: [PLAN_G29_SUMMON_TARGET.md](PLAN_G29_SUMMON_TARGET.md)) — found while sweeping the "Java-on-Creature vs port-on-Player" bug class from slice 18, which led somewhere else entirely: `TargetType::SUMMON` was **never implemented**, so all **18 learnable** summon-targeted skills fell through to `INVALID_TARGET`. Ranked by learnable skills it outranks `NpcBody` (5), `EnemyNot` (4) and `PcBody` (2) **combined**, all of which the port already handled. What was dead: the **entire Summoner support kit** — Servitor Heal/Recharge/Magic Shield/Physical Shield/Haste/Wind Walk/Magic Boost/Empowerment/Cure/Blessing, Mighty Servitor, the four class servitor buffs (Warrior/Wizard/Assassin/Final) and Mass Surrender ×3. A Summoner could summon a servitor and then do nothing for it. Java's quirk kept as written: `getAnyServitor()` is null for a **pet**-only owner (and `hasSummon()` is true for a pet, so the `getPet()` fallback is unreachable), so "Servitor Heal" does nothing for a Wolf owner — thematically right, and pinned by a test so a later "fix" must be deliberate. servitor_tests 92 → 97 incl. a datapack-backed parse check on the real kit. **Summon buff visibility landed** (plan: [PLAN_G29_SUMMON_BUFF_INFO.md](PLAN_G29_SUMMON_BUFF_INFO.md)), running the `Creature`-vs-`Player` sweep slice 19 admitted it had skipped. First two probes came up **clean** (NPCs do get `Buffs`; `apply_buff_to_npc` does recompute stats) — recorded as such rather than manufacturing a finding, and now pinned end-to-end since slice 19 only proved a *heal* lands on a servitor, never that a **stat buff** moves its numbers. The real gap was the NPC buff path's own admission — *"no `NpcInfo` re-broadcast, so a speed change isn't reflected client-side until respawn"* — tolerable for a mob, a bug for a servitor: Servitor Haste and Wind Walk both land in fields `PetInfo`/`SummonInfo` carry and are cast by an owner expecting to see the difference, so the buff worked and looked broken. Summons (only) now re-send `PetInfo`/`SummonInfo` on buff land **and expiry** (without the expiry half the summon keeps showing the buffed speed). The new packet-presence test was **verified to fail with the fix disabled** before being kept. servitor_tests 97 → 99. **Summon PvP flagging fixed** (plan: [PLAN_G29_SUMMON_PVP_FLAG.md](PLAN_G29_SUMMON_PVP_FLAG.md)) — the `Creature`-vs-`Player` sweep's probe with teeth. Java flags `getActingPlayer()`, which for a `Summon` is its **owner**; the port had no equivalent and kept its flag/stance block inside a player-only `else`, so **a summon attacking a player flagged nobody** — exploit-shaped, since a player could set their pet on someone and never go purple while the victim couldn't retaliate without taking the karma. Added `pvp::acting_player` and resolved inside `update_pvp_status_target` so every flagging path gets summons for free. **That alone did not work**: the block never ran for NPC attackers, and only the end-to-end test (a real `do_auto_attack`) caught it — the unit test calling the helper directly passed. The block now runs for both branches gated on the *resolved* actor being a player, which is safe precisely because `acting_player` maps a mob to itself; a test pins that a monster still flags nobody. servitor_tests 99 → 103, pvp/duel/combat/social re-run clean. **Summon kill credit fixed** (plan: [PLAN_G29_SUMMON_KILL_CREDIT.md](PLAN_G29_SUMMON_KILL_CREDIT.md)) — the `getActingPlayer()` audit's biggest find. Java's `calculateRewards` resolves every damage dealer with `info.getAttacker().getActingPlayer()`; the port keyed the aggro list by the dealer's own id and never resolved it, so **a summoner whose pet did the fighting earned nothing** — no exp, no drops, no quest kill credit. The core summoner loop was completely broken. Resolved in the damage-share loop, the looter fallback and `notify_kill`, with range measured from the **earner** as Java does. Resolution creates a new hazard the fix has to handle: an owner fighting *alongside* their summon now appears twice in the aggro list, so shares **merge per resolved player** — a test pins that owner 100 + summon 100 earns the same as a rival's 200. The probe test needed three corrections before it measured anything (no damage history; a real swing lands on a *scheduled* tick; `default_template` awards 0 exp), then was confirmed to fail with the fix disabled. servitor_tests 103 → 105; drop/quest/party/social/combat groups re-run clean. **`getActingPlayer()` audit part 2** (plan: [PLAN_G29_ACTING_PLAYER_AUDIT.md](PLAN_G29_ACTING_PLAYER_AUDIT.md)): two more live bugs from the same root. **PK/karma** — `Player.doDie`'s reputation block reads `killer.getActingPlayer()`, but the port gated on "is the killer a player", so **a summon killing a player produced no PK counter and no karma**: set your pet on someone and walk away clean. **Duels** — `duel_lethal_guard` exists to hold *a duel never kills*, and began with `are_dueling(attacker, …)`; a summon carries no `DuelRef`, so its blow wasn't recognised as duel damage and slipped past the cap, really killing the opponent. A guard whose whole purpose is an invariant was violable by an actor it never considered. Also corrected a test that asserted an *intermediate* (1 HP) rather than the observable outcome — capping ends the duel and `restorePlayerConditions` heals both sides. **Audit is four for four**: every `getActingPlayer()` site probed (flagging, rewards, PK/karma, duels) was a live bug, so the remaining sites (clan-war kill counting, `OnAttackableKill`'s `isSummon` flag) deserve the same treatment. servitor_tests 105 → 107; duel/pvp/death/combat/quest groups re-run clean. **`getActingPlayer()` audit closed** (plan: [PLAN_G29_ACTING_PLAYER_AUDIT_3.md](PLAN_G29_ACTING_PLAYER_AUDIT_3.md)). The last two flagged sites — clan-war kill counting and the clan-war death-exp relief — turned out to be **already covered, by accident**: slice 23's resolution was a `let` shadow part-way down `player_do_die`, and nothing between it and them used the raw id. Coverage by luck, so it is hoisted to the **top of the function** where insertion order can't defeat it, and both behaviours are now pinned by tests (unresolved, a summon killer has no clan, so a victim paid **four times** the exp they should for dying to an enemy's pet). Final tally: **four genuine bugs from four probes** (flagging, reward attribution, PK/karma, duel lethal guard) plus two sites made robust; the only remaining Java call sites are event dispatch this port has no equivalent of. Generalisable finding: **when the reference implementation routes through a resolver, port the resolver, not the common case** — expressing `getActingPlayer()` as "is this a player" compiles, runs, and is wrong only for summons, which no existing test exercised. servitor_tests 107 → 109; clan/death/pvp/duel re-run clean. **Pet equipment landed** (plan: [PLAN_G29_PET_EQUIP.md](PLAN_G29_PET_EQUIP.md)), closing the `TODO(G29)` slice 8 left in `PetInventory::to_rows`. 96 equippable pet-armour items ship on this dist; pet **evolution** has no item handler at all here and is struck rather than scheduled. `PetInventory` already wraps `Inventory`, which owns the paperdoll and every slot rule, so pet armour reuses the player equip path wholesale — as Java does (`PetInventory extends Inventory`) — with the click-to-remove toggle. Two halves had to be added: **stats** (the NPC pipeline has no inventory step, so `recalculate_pet_stats` now sums the pet's own paperdoll via `item_stats`; defensive stats only) and **persistence** (`to_rows` emits `PET_EQUIP` for worn rows, `PET` for carried; the slot already rides in `loc_data`, so renaming the location preserves it and `from_rows` renames back — a pet's armour comes back **on**, not loose in its bag). servitor_tests 111 → 114; inventory/items/char_persistence re-run clean. **Pet reconnect resummon landed** (plan: [PLAN_G29_RECONNECT_RESUMMON.md](PLAN_G29_RECONNECT_RESUMMON.md)), honouring the `pets.restore` column slice 7 hard-coded to `'false'`. `RestorePetOnReconnect`/`RestoreServitorOnReconnect` are **both True** on this dist, so this is the normal path — checking the config first is what made it the pick. The flag is set in `sync_pet_row`, which `on_owner_leave_world` already calls **before** the unsummon precisely so it observes a live pet: no separate logout hook and no way for the two to disagree. Restoring reuses `summon_pet` via `pending_pet_collar` rather than a parallel path, so a restored pet is identical to a freshly summoned one; guarded on the collar still being in the inventory, since it can be traded away between sessions and a dangling holder would leak into an unrelated cast. servitor_tests 114 → 118 (incl. the pet coming back *in the state it left in*, and a missing collar leaving no dangling holder) + `char_persistence` round-tripping `restore` both ways — it is a **string** column in Java, which a bool binding would quietly get wrong. Servitor reconnect (`character_summons`) still open. **Servitor reconnect landed** (plan: [PLAN_G29_SERVITOR_RECONNECT.md](PLAN_G29_SERVITOR_RECONNECT.md)) — a different shape from the pet case: a servitor has no collar, so Java rebuilds it by **re-casting the summoning skill** and stamping the saved vitals/lifetime onto the result. `character_summons` therefore stores a *skill id*, and a restored servitor comes back at the player's **current** level of it. Remaining lifetime is preserved (relogging is not a free duration reset); the row is consumed *before* the re-cast so an unlearned skill isn't retried every login; an empty row is written when nothing is out. **The write nearly cost characters their data**: `DELETE FROM character_summons` with `?` aborts the entire save transaction on any schema lacking the table — six unrelated persistence tests failed on it, which is how it was caught. Now best-effort, same rationale as `load_account_var` but applied to a write, since a failing write inside the transaction takes every other write down with it. servitor_tests 118 → 121 + a real-schema round trip asserting the lifetime survives a relog. **Servitor buff persistence landed** (plan: [PLAN_G29_SUMMON_BUFF_PERSIST.md](PLAN_G29_SUMMON_BUFF_PERSIST.md)), completing slice 27 — the servitor came back but stripped of everything cast on it, arguably worse than not restoring it since slice 19 had just turned on the Summoner support kit. **The remaining-work note was mislabelled**: `SummonEffectTable` is not "master-buff inheritance" (a Freya-era mechanic, **struck** — not on this chronicle) but persistence of the summon's *own* buffs via `character_summon_skills_save`. Third mislabelled carried-forward note this milestone. Reuses `SkillBuffRow` verbatim and restores through `restore_persisted_buffs`, the player's own login path, so a servitor's buffs can't drift from a player's; `ORDER BY buff_index` preserves application order for the slot cap, and expired buffs are filtered at capture. Writes are best-effort, applying slice 27's lesson without relearning it. servitor_tests 121 → 123, asserted on the buff's actual effect (run speed) rather than row presence. **`ServitorSkillUse` landed** (plan: [PLAN_G29_SERVITOR_SKILL_USE.md](PLAN_G29_SERVITOR_SKILL_USE.md)) — the summon's action-bar buttons. `ActionData.xml` ships **105** bindings; the port matched three hard-coded ids (hold/attack/stop) and returned early on the rest, so every one was dead. **13** name a skill the six summonable servitors here actually have (measured before building — the rest bind later-chronicle summons). The `action_data` loader already existed but kept only the id list for `ExBasicActionList`, discarding `handler`/`option`; widened so this is a lookup rather than 105 match arms. Guard that matters: the skill must be in the servitor's **own** `skill_list`, since the table binds every summon in the game and casting blind would let one summon borrow another's abilities. Ordered casts go through `npc_cast::start_cast` behind the same `check_use_conditions` gate as AI casts, so they pay the same MP, mutes and cooldown. servitor_tests 123 → 126 incl. a datapack-backed binding check; the cast test was confirmed to fail with `start_cast` disabled. **G29's summon subsystem is complete for this chronicle**; only pet spiritshots remain (they need pets to cast first). **Summon spiritshots landed — G29 COMPLETE** (plan: [PLAN_G29_SUMMON_SPIRITSHOTS.md](PLAN_G29_SUMMON_SPIRITSHOTS.md)). The "blocked on pets casting" note was **wrong**: `npc_ai_tick`'s summon branch already runs `think_attack` → `try_cast`, so summons have cast since G21 (53 active skills across 56 pet species). Fourth mislabelled carried-forward note this milestone; the check cost one grep. Mirror of the soulshot slice with one real difference — a magic shot is charged before the **cast** and spent by the cast itself, so the charge sits in `start_cast` and the spend in the effect path (Java splits them the same way). Cost is the level's `spiritshot_count`, parsed in slice 18 and unread until now. `apply_skill_effects` read the shot flags off `Player`, so an NPC caster silently got no bonus — **third instance** of that gate shape in this subsystem. Blessed Beast Spiritshots don't exist here, so only the ×2 tier is reachable. servitor_tests 126 → 130, with the bonus measured by running the same cast charged and uncharged rather than asserting a flag. |
| Game  | G30 Mail, community board & party matching                  | ✅ **community board: home + buffer + gatekeeper + premium + scheme buffer landed** (`ShowBoard` window + chunked `sendCBHtml`; `RequestShowBoard`/`_bbs*` bypass routing; custom `HomeBoard` render with navigation; `_bbsheal`/`_bbsteleport`/`_bbsbuff` actions + karma/combat gates; `_bbspremium` account-premium buy; `_bbs_buff_scheme_create`/`_delete`/`_execute` backed by the `buffer_schemes` table + `SchemeBufferSkills.xml` levels; `FavoriteBoard` `_bbsgetfav`/`bbs_add_fav`/`_bbsdelfav_` backed by the `bbs_favorites` table + `HomepageBoard` `_bbslink` + `DropSearchBoard` `_bbs_search_item`/`_bbs_search_drop`/`_bbs_npc_trace` — drop index, server-rate drop list, item-icon side-map, new `RadarControl` 0xF1 packet; **merchant multisell** `MultisellData` + `MultiSellList` 0xD0 + `MultiSellChoose` 0xB0 exchange behind `_bbsmultisell`/`_bbsexcmultisell`). **Party matching rooms landed** (plan: [PLAN_G30_MAIL_PARTY_MATCHING.md](PLAN_G30_MAIL_PARTY_MATCHING.md)) — the looking-for-party board: `model/matching_room.rs` (`MatchingRoom` + the party half of Java `MatchingRoomManager`) on `World.matching_rooms`, with room membership **derived** from the registry instead of mirrored on the player (Java's `Player._matchingRoom`), so the two can't disagree. `RequestPartyMatchConfig` 0x7F (Java's only looking-for-party registration entry point), `RequestPartyMatchList` 0x80 create+edit, `RequestPartyMatchDetail` 0x81 join, ex 0x09 oust / 0x0A dismiss / 0x0B withdraw / 0x25 exit-waiting-room / 0x2F ask-join + 0x30 answer (through a new `RequestKind::PartyRoomInvite` on the existing `PendingRequest` slot) / 0x31 browse-waiting-list → `ListPartyWaiting` 0x9C, `PartyRoomInfo` 0x9D, `ExPartyRoomMember` 0xFE 0x08, `ExClosePartyRoom` 0xFE 0x09, `ExAskJoinPartyRoom` 0xFE 0x35, `ExListPartyMatchingWaitingRoom` 0xFE 0x36. Also parses the `bbs` map-region attribute (`MapRegionManager.getBBs`) — the room "location" — which was in the XML but never read, and the `UserInfo` CLAN-block byte now carries `isInMatchingRoom` instead of a hardcoded 0. Cross-hooks: logout leaves the room *then* the waiting list (order is load-bearing — leaving re-adds you), leaving your party leaves the room, and accepting a party invite from a room leader joins his room. **Six Java defects deliberately not reproduced**, each pinned by a test: `deleteMember` never removes a solo room (the ctor put the leader in `_members`, so `isEmpty()` is never true → leaked rooms in every later list); the `MY_LEVEL_RANGE` room filter is inverted (`min >= lvl && max <= lvl`, matching only a `[lvl,lvl]` band); the oust handler reads `player.getParty()` for *both* sides so "cannot dismiss a party member by force" never fires; `notifyRemovedMember` announces a leader change unconditionally and builds the member packet from the *leaver*; `AnswerJoinPartyRoom` strands `activeRequester` on an early return; `RequestAskJoinPartyRoom` NPEs when the inviter has no room. 25 tests. **Mail landed** — Java's mail is *world* state, not player state (both parties can be offline), so `model/mail.rs` (`Message`/`MailManager` + the attachment containers) lives on `World` with write-through persistence (the clan-warehouse discipline, not the memory-first player one). The full ex 0x62–0x6C family: item-list, send (the whole guard chain + the `100 + 1000/slot` fee), inbox/outbox listing, open, delete, receive-attachment with COD, cancel, reject → `ExShowReceivedPostList`/`ExShowSentPostList`/`ExReplyReceivedPost`/`ExReplySentPost`/`ExReplyPostItemList`/`ExChangePostState`/`ExNoticePostArrived`/`ExNoticePostSent`/`ExUnReadMailCount`, plus the enter-world unread badge (`EnterWorld` sent neither before). `ScheduledTask::MailExpire` replaces Java's 10 s polling `MessageDeletionTaskManager` with a per-message timer (15-day regular / 12-hour COD), returning attachments to the sender's **warehouse**; a timer that fires early re-arms rather than deleting. **This milestone forced the `CharInfoTable` equivalent** (`World.char_ids_by_name`) the port had explicitly gone without — three separate comments noted its absence — because mail is addressed *by name* to a character who need not be online. Notable fidelity points: adena being *attached* can't also pay the fee; a partial-stack attachment allocates a fresh object id and the send path re-sends the whole item list (an `InventoryUpdate` delta can't express a new id); a batch delete aborts entirely on the first bad id; `AllowAttachments=False` *coerces* (message still goes, minus items/COD/price) rather than rejecting. One deliberate divergence: Java pays an **offline** COD sender by writing an adena `items` row straight into their inventory location; the port has no second write path there, so an offline payout is delivered as a system mail with the adena attached. 35 tests, the COD round-trip / offline payout / expiry return all sabotage-verified. **Both G30 gates met.** Remaining G30: `_bbssell` (needs buylist 423, absent) and `_bbsdelevel` (config-off) board actions, the retail forum boards (`TODO(G30)`), and AdminBBS. |
| Game  | G30.5 Item auction                                          | ✅ **COMPLETE** — **Data + model + DB foundation landed** (slice 1, plan: [PLAN_G30_5_ITEM_AUCTION.md](PLAN_G30_5_ITEM_AUCTION.md)): `data/item_auction_data.rs` (`ItemAuctions.xml` parser — interval + weekday schedules, empty on this dist), `model/item_auction.rs` (`AuctionState`/`ExtendState`/`ItemAuctionBid`/`ItemAuction` + `ItemAuctionManager` on `World` + the pure `next_date` `AuctionDateGenerator` math), `item_auction::on_loaded` boot restore (config-gated on `AltItemAuctionEnabled`, dist `True`; `auctionId` allocator from `MAX+1`), `item_auction`/`item_auction_bid` DB load (`DbEvent::ItemAuctionsLoaded`) + writes (`DbCommand::{StoreItemAuction,StoreItemAuctionBid,DeleteItemAuctionBid,DeleteItemAuction}`). Config-enabled but `ItemAuctions.xml` ships empty → gate via a synthetic auction. 9 tests, sabotage-verified. **Slice 2 landed — the lifecycle + scheduling:** `check_and_set_current_and_next` (per-instance current/next pick + fresh-auction creation, `START_TIME_SPACE`/`FINISH_TIME_SPACE`), `create_auction` (random item + `next_date` + `storeMe`), and `run_state_task` (CREATED→STARTED→FINISHED on `ScheduledTask::ItemAuctionState`, with the bid-driven ending-extend re-arm inert until slice 3); `InstanceRuntime` per auctioneer; `on_loaded` schedules each configured instance. 5 tests, sabotage-verified. **Slice 3 landed — bidding + packets + NPC dialog:** `register_bid` (adena escrow — full new / delta on raise / full after cancel; ≥init/>highest/≤999.9bn gates; outbid notify; the last-10-min ending-extend state machine +5/+3/config, activating the slice-2 re-arm), `cancel_bid` (loser refund + highest-holds-reserve branch), the `ItemAuctionLink` NPC bypass (`show`/`cancel`), the two client packets (`RequestBid` 0x36 / `RequestInfo` 0x37), and `ExItemAuctionInfoPacket` (0xFE 0x69). +14 SM ids + `AltItemAuctionExpiredAfter`/`AltItemAuctionTimeExtendsOnBid` config. 18 tests, sabotage-verified. **Slice 4 landed — finish/delivery/expiry (G30.5 COMPLETE):** `on_auction_finished` hands the reward to the winner's warehouse (live `Warehouse` component online, else a direct `items` insert via `DbCommand::StoreOfflineWarehouseItem`), `clear_canceled_bids` on finish, and boot expiry cleanup (finished auctions past `AltItemAuctionExpiredAfter` dropped + `DeleteItemAuction`). **A full auction now runs: schedule → bid → finish → winner gets the item.** 22 tests, sabotage-verified. G30.5 done (an operator-defined auction runs start-to-finish; `ItemAuctions.xml` ships empty). |
| Game  | G31 Moderation, accounts, petitions & HWID                  | ✅ **Slice 1 (punishment foundation + jail) landed — the gate clause "jail a player" is met** (plan: [PLAN_G31_MODERATION.md](PLAN_G31_MODERATION.md)): the whole punishment substrate the milestone rides on. `model/punishment.rs` — `PunishmentType`{BAN,CHAT_BAN,PARTY_BAN,JAIL} / `PunishmentAffect`{ACCOUNT,CHARACTER,IP,HWID} (both round-tripping the DB enum name), `Punishment{id,key,affect,type,expiration,reason,by}`, and the `PunishmentManager` registry on `World` (keyed lookups + the `player_has` four-affect OR that is Java `Player.isJailed`). DB: the `punishments` table loads at boot (`DbEvent::PunishmentsLoaded`, before `ClansLoaded`, expired rows filtered like Java's `load`) with writes `DbCommand::{StorePunishment,DeletePunishment}` (row id allocated game-side so a release deletes by id — Java expires the row in place, behaviourally identical since the load skips expired). `ZoneKind::Jail` parses `gm_room.xml`'s 3 `JailZone`s (geometry-queried, no mask bit — the u8 mask is full, like Fishing/ClanHall/DerbyTrack). The JAIL effect (Java `JailHandler`): `jail_character` registers + persists + arms a `ScheduledTask::PunishmentExpire` for timed jails + teleports the online character to `JAIL_IN_LOC` and flags `Player.jailed`; `unjail_character` / expiry teleports to `JAIL_OUT_LOC` and clears it; the JailZone **keep-in** re-teleports a jailed wanderer back (hooked into `revalidate_zone`'s tail, GMs exempt); and login re-apply (Java `onPlayerLogin`) puts a returning inmate back in / lets a lifted one out. Admin `//jail <name> [minutes]` / `//unjail <name>` (`admin_jail`/`admin_unjail`, unlisted → auto-granted to level-100 GMs like the port's other unlisted commands). The CHARACTER key is the char object id (== DB id here), so a jail survives relog. 20 tests (12 game-loop + 8 model), sabotage-verified; jail-zone parse + `in_jail_zone` pinned against the real dist. **Slice 2 (ban + chat-ban + party-ban) landed** — the slice-1 model generalised into a `start_punishment`/`stop_punishment` engine that dispatches each type's Java handler `onStart`/`onEnd` over the affected online players. **BAN** (`BanHandler`): onStart disconnects the player (`Disconnection` → persist + despawn + drop session), and the **character-select login gate** (`CharacterSelect`'s char/account/IP ban check) refuses re-entry by closing the connection. **CHAT_BAN** (`ChatBanHandler`): `Say2` drops any non-`.`-prefixed message from a chat-banned player (SM 966), and the `EtcStatusUpdate` mask now ORs `is_chat_banned` so the chat-block icon lights (login re-apply included). **PARTY_BAN**: `RequestJoinParty` blocks a party-banned requestor (SM 2484 + ActionFailed) or target (SM 2482) — CHARACTER-affect only, matching Java `isPartyBanned`. Admin `//ban_char`/`//ban_acc`/`//ban_chat`/`//ban_party` + `//un*` (Java exposes only the add shortcuts via HTML-remove; the explicit un-commands are a documented port convenience, and take a name **or** a raw char id since the port has no offline name→id table like Java's `CharInfoTable`). 3 SM ids added (966/2482/2484). 9 new tests (17 total in the file), the two new gates (chat-block, login refusal) sabotage-verified. **Slice 3 (petitions) landed — gate clause "file + answer a petition" met** — the in-memory GM petition system (Java `PetitionManager`/`Petition`, `RequestPetition`/`Cancel`/`Feedback`, `AdminPetition`), entirely transient bar the feedback row. `model/petition.rs`: `PetitionType`(9)/`PetitionState`(7) + `Petition`{id,type,state,content,petitioner,responder,submit_time,log} + `PetitionManager`{pending,completed} on `World`. **Submit** (`RequestPetition` 0x89): the full validation chain (GM-online / petitioning-allowed / one-pending-per-player / queue cap / per-day cap / 255-char) → registers pending + 3 receipt SMs + a `HeroVoice` "new petition" broadcast to all GMs. **Accept** (`AdminPetition` `//accept_petition`): responder set, state→IN_PROCESS, both parties notified, `Player.last_petition_gm_name` stamped (the "answer"). **Consultation chat**: `Say2` types PETITION_PLAYER(6)/PETITION_GM(7) route through `send_active_petition_message` to both participants and append to the transcript (replayable on reconnect). **End** (`RequestPetitionCancel` 0x8A by GM, or `//reject`, or petitioner cancel): `endPetitionConsultation` moves it to completed and fires the `PetitionVote` (0xFC) feedback prompt. **Feedback** (`RequestPetitionFeedback` 0xC9): the sole persisted state — `StorePetitionFeedback` → `petition_feedback`, gated on `last_petition_gm_name`. New: `ChatType` PetitionPlayer/PetitionGm/HeroVoice, opcodes 0x89/0x8A/0xC9 + PETITION_VOTE 0xFC, 18 petition SM ids, `petition_vote()` builder, 3 Character.ini config keys (PetitioningAllowed/MaxPetitionsPerPlayer=5/MaxPetitionsPending=25). 9 tests (submit/accept-gate/chat/end/feedback/cancel), the accept-consultation transition sabotage-verified. **Slice 4 (login-ban relay + IP tools) landed — gate clause "ban via the login link" met** — the account-ban relay to the login server (Java `Player.setAccountAccesslevel` → `LoginServerThread.sendAccessLevel` → the `ChangeAccessLevel` 0x04 login-server packet). `LoginLinkCommand::SetAccountAccessLevel{account,level}` + the `change_access_level` packet builder wire it through the existing login link; `//login_ban <account>` relays access level −1 (login refuses the account's next login) **and** disconnects any character on it currently online (so the ban bites immediately, a small port addition over Java's relay-only path), `//login_unban <account>` relays 0. This is distinct from slice 2's game-side `//ban_acc` punishment (character-select gate) — the two account-ban mechanisms Java keeps separate. Plus the editchar IP tools off `Session.addr` (the per-client peer address): `//find_ip <ip>` (online characters from an IP), `//find_dualbox [n]` (IPs with ≥ n online chars, default 2), `//tracert <name>` (a target's connecting IP — Java dumps the client's route trace, which needs client plumbing the port lacks, so it reports the peer address, a documented simplification). The IP-tool logic sits in testable `characters_from_ip`/`dualbox_ips` pure helpers. 4 tests (relay + kick / unban / find_ip / find_dualbox), the login-ban relay sabotage-verified. **Slice 5 (snoop + HWID) landed.** **`//snoop`** (Java `AdminGmChat.snoop` + `Player.broadcastSnoop`): `Player.snoop_listeners`/`snooped` sets, the `Snoop` packet (0xDB), and a `broadcast_snoop` hook in `Say2` mirroring a snooped player's every chat line to the watching GM. **HWID**: the `RequestHardWareInfo` client packet (ex 0xAE, the 19-field fingerprint) parses into a `HardwareInfo` stored on `World.hwids` (keyed by client id, cleared on disconnect); the previously-inert HWID punishment affect now **matches** (`players_matching`/`player_has` read the MAC), the character-select ban gate + a post-enter `on_hwid_received` re-check (the packet is client-timed, so it can land after enter-world) both enforce HWID bans/jails, and `//hwid`/`//hwinfo` displays the fingerprint. `EnableHardwareInfo = False` on this dist so it stays dormant until enabled — ported anyway per the config-disabled convention. 5 tests (snoop mirror/no-leak, hwinfo parse, HWID-ban disconnect, post-login re-check), the HWID match + the login-ban relay sabotage-verified. **Deferred:** `AdminFakePlayers` (`//fakechat`) needs the whole fake-player subsystem (`FakePlayerData` + `FakePlayerChatManager` + fake-NPC spawns) the port lacks — a separate content system, its own milestone. **G31's five gate clauses (jail / ban / chat-ban / party-ban / file+answer a petition / ban via login link) are all met.** |
| Game  | G32 Fishing                                                 | ✅ **Slice 1 (cast → hook → land a fish) landed** — the new single-action fishing system: FishingData loader (Fishing.xml baits/catch-tables/rods), the cast→reel engine (ExRequestAutoFish toggle → scheduled reel on the bait win chance → consume bait + reward fish/XP → auto-recast), ExFishingStart/End/UserInfoFishing packets. Slice 2: FishingZone geometry landed — a cast requires standing in a FishingZone and the bob landing over a WaterZone (upper-Z = bob depth), else it fails; fishing.xml (13 zones) + ZoneKind::Fishing (geometry-queried, no mask bit). Slice 3: canFish gates — premium-only bait requires a premium account; a player standing in a WaterZone can't fish (isInsideZone WATER). Slice 4: fishing shots landed — ShotType::FishSoulshots + ItemHandler::FishShots; recharge_shots gained a fish arg (charge_fish_shot consumes 1 shot); the cast charges fish shots (if auto-on), the reel doubles the win chance when charged, and a catch spends+recharges the shot. Slice 5: ExAutoFishAvailable — entering a FishingZone (rod+bait ready) lights the client auto-fish button, leaving dims it (fired from revalidate_zone via a ZoneFlags.fishing_available bool). CORE + DISCOVERY COMPLETE. Not ported (off-chronicle/absent/blocked): fishing skill tree (fishingSkillTree.xml costs GoD Elcyum + is inventory-expand skills), fishing championship (no Java class/data in this dist), fishing system messages (YOU_CAN_T_FISH_HERE etc — client numeric ids unverifiable), RequestExFishRanking (niche ranking window). |
| Game  | G33 Misc parity & finishing sweep                           | ✅ **All four named slices done** — **Slice 1 (DailyTaskManager + vitality refills) landed** (plan: [PLAN_G33_MISC_PARITY.md](PLAN_G33_MISC_PARITY.md)) — the wall-clock daily reset (Java `DailyTaskManager.onReset` at 06:30 UTC) generalised from the reco-only skeleton into a `daily_tasks` module: `DailyRecoReset` → `DailyReset`, which runs `reco::reset_recommends` + the new `vitality::reset_vitality` and re-arms 24 h out. **This closes the standing vitality drain-only bug** — deferred from G16, vitality had no refill path, so it only ever went down; now the daily reset **adds `MAX/4` (35000)** (weekly-full on UTC-Wednesday, Java's `Calendar.WEDNESDAY`), to online players (via `set_vitality_points`, so the gauge + notices update) and the offline population (`DbCommand::ResetVitality` → the two `characters`/`character_subclasses` `CASE WHEN` UPDATEs, uncapped like Java since the read clamps). Gated on `EnableVitality` (True on this dist). 7 tests (weekday math, daily add/clamp, weekly full, disabled no-op, the reset runs both sub-resets + reschedules), the daily add sabotage-verified. Boot catch-up (`GlobalVariablesManager.DAILY_TASK_RESET`) still `TODO(G33)` — no GlobalVariables table. **Slice 2 (game-time clock) landed** — the `GameTimeTaskManager` clock (`game_loop/game_time.rs`): `game_time_minutes()` (game-minutes since in-game midnight, 0..1439) now fills `CharSelected`, which had hardcoded `0` (permanent midnight) since G4. One in-game day is 4 real hours (`IG_DAYS_PER_DAY = 6`), so a game-minute is 10 real seconds; the client self-advances from the value, matching Java (this dist broadcasts no SunRise/SunSet on transition). **No stored anchor needed** — a real-day (86.4M ms) is exactly 6 in-game days (6 × 14.4M ms), so any midnight is `≡ 0 (mod MILLIS_PER_IG_DAY)` and the clock is a pure function of `now_millis` (Java's boot-midnight anchor is equivalent). `is_night()` (hour < 6) is exposed for the future day/night scripts (`DayNightSpawns`/`NightStatModify`). 3 tests (day wrap / midnight-zero / night boundary), the clock math sabotage-verified. **Slice 3 (packet-parity audit) landed** ([PARITY_CHECKLIST_G33.md](PARITY_CHECKLIST_G33.md)) — the planned "periodic autosave" was found **already done** (`autosave_tick` = Java `PlayerAutoSaveTaskManager`, one due player/sweep, since the persistence work; a stale roadmap item), so this slice did the milestone's actual gate: a mechanical opcode-keyed diff of Java's 353 `::new` client-packet handlers against the Rust dispatch. **175 handled / 178 not-dispatched**, and every not-dispatched packet is either a **deferred-by-design subsystem** (clan wars G18, mail + party-matching G30, MPCC, private buy-store, siege-info UI) or **later-chronicle / niche** off-chronicle content — no gameplay family silently slipped. The one genuine slip found, **`RequestQuestList` 0x62** (quest-journal open — quests fully ported, only the journal-refresh packet was missing), is closed here (empty body → resend `QuestList`, Java verbatim, 1 test, sabotage-verified). **Tail triage (G33 close-out):** ported the one genuinely-useful, cleanly-portable tail item — **`AdminRepairChar`** (`//repair`/`//restore <name>`): unstick a broken **offline** character via a DB-only `RepairCharacter` command (teleport to Giran, wipe shortcuts, un-equip all items — Java verbatim, SQLite single-quoted), guarded against an online target (the memory-first autosave would overwrite it). 1 test, guard sabotage-verified. The rest of the tail is deliberately **not** ported: `AdminPcCondOverride` is inert in this architecture (`is_gm()` already stands in for "all cond-overrides enabled", so the flags would drive nothing without a cross-cutting rewire); `AdminPForge` (659-line packet-forge), `AdminMissingHtmls`, `AdminFightCalculator`, `//geosave` are dev/ops tooling with no gameplay effect; **Offline shops landed (2026-07-31, the last audit item)** — `OfflineTradeUtil` + `OfflineTraderTable` + the `.offline` voiced command, unblocked by row 6's private buy-store. Logging out (or `.offline` → its `ConfirmDlg`) with a store open now **leaves the `Player` in the world with its shop trading**: the session is dropped (so `client_for_player` returns `None` and every send/broadcast no-ops, exactly like Java's *detached* `GameClient`), the account logout still goes to the login server, party/olympiad/pets/cubics are shed — but **no friend/clan "logged off" notice**, because those fire from `deleteMe()`, which Java skips, so an unattended shop still reads as online to its friends. New `World::offline_traders` index: player visibility is answered from `world.clients`, so session-less players needed their own subject source in `visibility::on_enter_world` and the `update_region` deltas — otherwise nobody would see the shop. Persistence rides the two Java tables (entities already existed): `DbCommand::StoreOfflineTrader`/`ClearOfflineTrader`, written after every transaction (`StoreOfflineTradeInRealtime = True` here), cleared at enter-world, swept at shutdown only when realtime storing is off (Java's own gate). Boot restore (`DbEvent::OfflineTradersLoaded`, pushed unprompted like the other restores) rebuilds each character through the normal `from_char` → `spawn_into` path plus the enter-world stat pumps and `restoreEffects`, re-opens the store, and honours `OfflineMaxDays` — a sell line naming an instance the character no longer holds is dropped, as Java's `addItem(...) == null → continue` does. Also `OfflineDisconnectFinished` (a sold-out shop leaves the world), `OfflineModeNoDamage`, `OfflineSetNameColor`, `OfflineDisconnectSameAccount`, and `OfflineModeInPeaceZone`. **Java quirks kept:** `MANUFACTURE` is gated by `OfflineTradeEnable`, not `OfflineCraftEnable` (the craft flag covers only the store-less `isCrafting()` branch, which the port has no state for — `TODO(G33)`); and `PlayerStatus.reduceHp`'s no-damage list is *narrower* than the go-offline list, so a `PACKAGE_SELL` shop is killable. `db::load_characters` was split into `char_data_of` + a new `load_character(char_id)` so a shop's character can be loaded without an account. New `config/offline_trade.rs`. 10 tests; entry, the visibility index, realtime storing, the transaction rewrite, disconnect-finished, no-damage, max-days, the missing-instance drop and the name colour were each sabotage-verified. **G33 complete** — the parity gate is met and the residuals are closed or documented-as-off-scope. **Slice 4 (the `Custom/*.ini` enable-flag audit) landed** ([PLAN_G33_CUSTOM_INI_AUDIT.md](PLAN_G33_CUSTOM_INI_AUDIT.md)) — the one-time audit the ROADMAP scope gate promised for G33 and that the "complete" mark was given without ever running. Every `Custom/*.ini` was checked on three axes (flag on in the dist / parsed by `Config.java` / actually *consumed* by Java code), because a shipped ini proves nothing on its own — `Custom/PcCafe.ini` is again the counter-example and is **confirmed dead in Java**, `Config.java` never opens it. **The audit found 17 features enabled on this dist, live in Java, and absent from the port**, which the scope gate's own "except any the operator explicitly enables" clause rules back *in*. 14 further files are genuinely disabled here and stay out; `ClassBalance.ini` is parsed but ships every multiplier list empty. **Champion monsters ported** (the first of the 17): the `Attackable.onRespawn` lottery with Java's whole guard chain (needing two previously-unparsed template flags — `<status undying>`, 681 NPCs on this dist, and `isQuestMonster()`, which Java computes as `title.contains("Quest")` rather than reading from XML), `ChampionAtk`/`ChampionSpdAtk` threaded through the NPC stat pipeline as `ChampionStatMods` **so a buff recompute keeps them** (a neutral recompute there would have stripped a champion's P.Atk the first time anything buffed it), `ChampionHpRegen` in `regen.rs` (no MP twin — `RegenMPFinalizer` has no champion arm in Java either), the `Creature.reduceCurrentHp` **damage divisor** (Java models bulk as `damage / ChampionHp`, *not* as a bigger pool, so the health bar still reads 100 % and hate still keys off the undivided damage), `ChampionRewardsExpSp` on both the solo and party reward branches, the drop multipliers with Java's **two-arm split** preserved (the adena multipliers fire only inside the `RATE_DROP_CHANCE_BY_ID` branch, the generic ones only in the flat `else` — collapsing them would have changed the payout on this dist, where adena *does* carry a per-id rate), the `ChampionRewardItems` tail, the `Champion` title (two arms: the decorated branch **prefixes**, the plain branch **replaces**), and the `Team.RED` aura as a new `TEAM` component in `NpcInfo` between `MOVE_MODE` and `ENCHANT`. **`useVitalityRate()` is now real** — it had been hard-coded `true` with a comment saying champions weren't ported, and it gates three things at once (the bonus argument to `addExpAndSp`, the vitality charge, and the PA-point award). **Java quirks kept deliberately:** `ChampionRewardLowerLvlItemChance`/`…HigherLvlItemChance` are **inverted** — both arms `return` *before* adding the reward, so each is a *suppression* chance despite the ini calling it a "% Chance to obtain"; with this dist's `0`/`100` that means a champion below your level always pays out and one above your level never does. The reward-item guard is Java's all-or-nothing `containsAll`, not per-item. And `ChampionEnable` is re-checked at every consumer rather than trusted from the stored flag. 21 tests; the damage divisor, passive-AI gate, reward-item tail, team aura and stat multipliers were each sabotage-verified one at a time — **which caught one vacuous test**: the passive-AI case re-ticked a mob whose intention the first half had already moved to `Attack`, so the second tick ran the attack loop instead of the aggro scan and the assert held regardless of the champion gate. **The remaining 16 features are unported** and listed with a cost ordering in the plan doc. | **Slice 3 (periodic autosave) verified already complete 2026-08-03** — `autosave_tick` is `PlayerAutoSaveTaskManager.run` on the same 1 s sweep, flushing at most one due player per pass (Java's `break; // Prevent SQL flood`) and rescheduling one `CharacterDataStoreInterval` out; the snapshot covers `storeMe` + `storeRecommendations` + all three item containers, and a test already pinned the one-per-sweep guard. The plan entry was stale, so this was recorded rather than re-ported. **The `Custom/*.ini` audit is fully closed** — all **17** features are ported, consumed and tested (re-verified 2026-08-03 flag-by-flag against the code, not the docs: every master flag parses, every one has a live consumer, and each has at least one test). An earlier revision of this row claimed 16 were still unported; that was stale. **What actually remains under G33 is optional tooling**, not parity: `//geosave`, the niche admin tools (FightCalculator / RepairChar / PForge / MissingHtmls / PcCondOverride), scheduled-restart + deadlock detector, `NpcNameLocalisationData`/multilang, and Dockerfile parity. |
| Game  | G34 Skills, effects & abnormal-state parity                 | ✅ **CLOSED 2026-08-03** (plan: [PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md); nine slices, S4 alone took 17 sub-slices) — the skill parser was *fail-open*: an unrecognised `<effect name>` yielded no `SkillEffect`, an empty effect list was dropped by `apply_skill_effects`' guard, and an unrecognised `<condition>` was never enforced, so a skill could cast, animate, burn MP and reuse and do nothing — or fire where Java refuses it. **S0** built the census that makes the gap measurable (`SkillGaps` records every drop at the fallback arm itself, across 7 axes); **S1** the condition engine (28 kinds; condition axis 111 pairs / 215 learnable skills → 69 / 1); **S2** `BasicPropertyResist`; **S3** nine flag-only `EffectFlag`s plus the three-tag `isStayAfterDeath()` fold; **S4** the effect sweep in 17 sub-slices; **S5** the three non-effect axes — including the epic's only gap that failed *closed*, seven learnable skills that could not be cast at all because `use_magic_on` bails on an unmapped `operateType`; **S6** the item tail (**every destination Scroll of Escape was inert** — 107 skills loaded with an empty effect list); **S7** the skill-tag tail (`<nextAction>`, `<abnormalResists>`, and the finding that `magicCriticalRate` is dead data in Java); **S8** the gate, which found Anchor (1170) doing half its job — its second, paralysing stage is an `<endEffects>` `CallSkill`. **Final: 275 → 11 of 758 learnable skills wrong, and all 11 are recorded out-of-scope decisions** (the nine Territory Benefaction skills, Acrobatics, Sweeper). Effect names 216 → 142; reachable 1154; every non-effect axis at 0 learnable. The gate is a named `(skill_id, reason)` list asserted in both directions, not a count — a new gap cannot hide inside the number, and a since-ported id must come off the list. Six dead stats/tags were found along the way where the correct outcome was to record the finding and change no code. |
| Game  | (out of scope) Gracia/Hellbound/elemental, sayune/shuttle/airship, `tools/`, MariaDB/Postgres, Swing UI, Mobius `Custom/*` | ⛔ non-Interlude / per PLAN §11 + ROADMAP scope gate |


> **Every row above is ✅ or an explicit scope-out — and that is not the whole
> picture.** A milestone is marked complete when its *gate* is met; each one
> also shipped a handful of narrow behaviours deliberately deferred and marked
> at the site. There are **145** such `TODO(G<N>)` markers, inventoried in
> [DEFERRALS.md](DEFERRALS.md) and asserted by
> `deferral_markers_match_the_recorded_inventory`, so the number cannot drift
> without someone deciding it should. Read that file alongside this table: the
> ✅ marks say the gates were met, not that nothing is left.
>
> This exists because prose about "what remains" is the least reliable artefact
> in the repo. On 2026-08-03 two documents were found claiming work that had
> been done for milestones, and five `TODO` markers PROGRESS said existed were
> absent from the code entirely — all five turned out to be finished work.

## G34 S8 — the epic gate, and G34 closes (2026-08-03)

Branch `feat/g34-s8-gate`. The gate was meant to be bookkeeping; forcing every
residual entry to be *examined* rather than counted turned up one more real gap.

**Anchor (1170) was doing half its job.** Its own description promises the body
goes "completely rigid for 5 seconds **and** causes paralysis for 5 seconds",
and that second half is an `<endEffects>` block firing `CallSkill(6091)` when
the first stage comes off. Neither the `END` effect scope nor `CallSkill`
existed here — and the scope enum carried a comment saying `START`/`END` "hang
off lifecycle hooks this port doesn't have (cast start, buff end)" when
`handle_buff_expire` has existed for milestones. That is the **fifth**
deviation comment this epic caught resting on a false premise.

`handle_buff_expire` is now a thin wrapper that reads the end-effects, runs the
removal, then applies them — the removal body has several early exits (the NPC
path returns before the player broadcasts), and hanging the END scope off the
wrapper makes every removal route fire it exactly once, which is what Java's
single call site does.

**The gate is a named list, not a number.** A count says nothing about whether
what remains was *decided* or merely never looked at, so the census now asserts
two things: every learnable skill still carrying an unhandled effect or
unenforced condition appears in a `(skill_id, reason)` table, and nothing in
that table has stopped failing — a since-ported id must come *off* the list
rather than sit there excusing a gap that no longer exists. Both directions
sabotage-verified, and the first check caught a real mistake while it was being
written: the ids guessed for `StatUp` and `SafeFallHeight` were wrong, and the
assertion named the right ones immediately.

**The recorded residue is 11 learnable skills of 758:** the nine
`<Town> Territory Benefaction` skills (848-856, `StatUp` — Territory War
content this chronicle does not have), Acrobatics (173, `SafeFallHeight` — the
port has no fall damage, so the stat would have no consumer), and Sweeper (42,
`OpSweeper` — enforced at apply time by `effects::sweep` with the right
per-corpse messages; gating the cast too would double every one).

**G34 is closed.** Learnable skills that drop an effect or ignore a condition:
**275 → 11**, every one recorded with a reason. Effect names 216 → **142**,
reachable **1154**. Every non-effect axis is at **0 learnable**.

## G34 S7 — the skill-tag & formula tail (2026-08-03)

Branch `feat/g34-s7-tags`. Two tags with real consumers, and three places where
the answer turned out to be "the data is dead".

- **`<nextAction>`** — `SkillCaster.finishSkill`'s "attack target after skill
  use" block, on **339** skills declaring `ATTACK` and 11 declaring `CAST`.
  Without it every offensive skill *ends* your combat: you fire Power Strike
  and then stand there. Java gates it on the AI having no queued intention, a
  real target that is neither the caster nor un-attackable, and — for `ATTACK`
  only — shift not being held, which is vacuous here (no shift-cast). The
  `CAST` branch re-queues the same skill through an intention queue this port
  does not have; re-casting inline would loop, so it is a deliberate
  `TODO(G34)` rather than a fake.
- **`<abnormalResists>`** — `calcEffectSuccess`'s *first* resist clause, ahead
  of any roll: a target part-way through a cast whose skill names this abnormal
  type shrugs the debuff off outright. 176 skills declare a list, 146 of them
  the full crowd-control set; this is what makes the long rituals
  uninterruptible.

**`magicCriticalRate` is dead data, which is exactly what the plan predicted.**
Every magic damage handler passes `skill.getMagicCriticalRate()` into
`Formulas.calcCrit`, and the magic branch's *first line* overwrites it with
`creature.getStat().getValue(MAGIC_CRITICAL_RATE)`. The per-skill value on 756
learnable skills is read and thrown away — so this port reading the creature
stat was right all along. Recorded rather than "fixed", which is the whole
point of having checked.

`soulMaxConsumeCount` (49 skills) and `specialLevel` (1622) are parsed by
Java's `Skill` and read by **nothing at all** — two more dead tags, bringing
this epic's tally to six. Not modelled.

One divergence recorded rather than closed, with a `TODO(G34)` at the formula:
Java's `calcCrit` magic branch adds `sqrt(casterLevel) + (levelDiff / 25)` and
raises the cap from 200 to 320 when **both** sides are level 78+. The
`DEFENCE_MAGIC_CRITICAL_RATE` term beside it has no reachable source on this
dist (all nine carriers are High Five+), so it is identity here.

Census unchanged: these are skill *tags*, not effect names, so no axis counts
them. Three tests, all sabotage-verified.

## G34 S6 — the item/NPC effect tail, first pass (2026-08-03)

Branch `feat/g34-s6-item-tail`. The plan's headline item, closed.

**Every destination Scroll of Escape was inert.** `Teleport` was an unparsed
effect name, so all **107** reachable skills carrying it loaded with an empty
effect list: the scroll was consumed, the cast animated, and the player did not
move. The destination is keyed on the skill *level* — skill 2213 alone carries
22 towns, one per level — so the coordinates are ordinary per-level values
rather than constants, which is what a "one destination per skill" reading
would have got wrong.

Alongside it, **`Hp`** — the raw instant HP change behind Elixir of Life (2287)
and the food/snack items, which also parsed to nothing. It is deliberately not
a `Heal`: no `calcHeal` pipeline, no healing-stat scaling, no overheal message.
Java's guards are dead / door / HP-blocked / **raid**, that last clause being
one the `Heal` family does not have, and the gain is clamped to the recoverable
ceiling — so a Noblesse Harmony aura caps an elixir the same way it caps a heal.

Census: effect names **145 → 143**, reachable **1303 → 1167** — 136 skills off
the list in one pass. Four tests, all sabotage-verified.

`print_coverage_report` now also ranks each axis by **reachable** count. Past
S4 the learnable ranking is nearly empty and the remaining work is exactly the
item- and NPC-triggered tail that ranking cannot see; without it there was no
way to pick S6's targets by leverage rather than by guess.

**What is left in S6, and why it is not being chased:** the next names down the
reachable ranking — `StatUp` (268), `SummonAgathion` (162), `SetSkill` (144),
`ExpModify` (99), `VitalityPointsRate` (58), `ResetInstanceEntry` (40),
`TalismanSlot` (35), `ChangeHairStyle` (14), `CrystalGradeModify` (10) — are
Territory War, Gracia+ or Freya+ content. They inflate the reachable count
without being reachable *on this chronicle*. Recorded so the number is
understood rather than pursued.

## G34 S5 — targeting, scope & operate-type breadth (2026-08-02)

Branch `feat/g34-s5-targeting`. Every learnable entry on the three non-effect
axes — and the sharpest finding of this epic so far, because unlike everything
before it this one fails **closed**.

- **`operateType` A3 and CA5: seven learnable skills that could not be cast at
  all.** `use_magic_on` returns outright for anything that is neither `Active`
  nor `Channeling`, and the parser dropped every unmapped `operateType` to
  `Other`. Blinding Blow (321), Vengeance (368), Evade Shot (369), Critical
  Blow (409), Aura Flare (1231), Battle Stance (426) and Spell Stance (427)
  therefore did nothing — not because an effect was missing, but because the
  cast never started. Java's `isActive()` covers A1-A6 and `isChanneling()`
  CA1/CA2/CA5; both are mapped as written now. The parser carried a comment
  asserting "CA5 doesn't occur on this dist's reachable content" — it is on two
  learnable skills.
- **`UNDEAD_REAL_ENEMY`** — the priest anti-undead auras (Sanctuary 97, Holy
  Aura 107, Repose 1034, Requiem 1049) are `SELF` + `POINT_BLANK`, so with the
  filter falling through to "no filtering" they swept **everything** in range:
  friendly players and every non-undead mob alike. Java's rule is not yourself,
  `isUndead()` (an NPC whose template race is `UNDEAD`; a player never is) and
  `isAutoAttackable(caster)`.
- **`TargetType::OTHERS`** (Battle Stance 426, Spell Stance 427, Summon Friend
  1403) — the current selection, with one rule: it may not be you, and Java
  refuses with its own message rather than the generic invalid-target one.

Census: target types **10 → 9** (learnable **3 → 0**), affect objects **5 → 4**
(learnable **4 → 0**), operate types **13 → 7** (learnable **7 → 0**). All
three sabotage-verified. The headline stays **12** by construction: it counts
the effect and condition axes only.

`TODO(G34)` at the operate-type parser: A3 also sets `isSelfContinuous()`,
whose only consumer is `BuffInfo.isDisplayedForEffected` — an A3 skill carrying
`<selfEffects>` hides its buff *icon* on a target that is not the caster. Not
ported: `ActiveBuff` records no effector, so the rule has nothing to test.
Display-only; the effects themselves are unaffected.

## G34 S4 sub-slice 17 — NightStatModify, and S4 closes (2026-08-02)

Branch `feat/g34-effect-sweep-17`.

**`NightStatModify`** (Shadow Sense 294) — "increases Accuracy by 3 **at
night**". The stat belongs to the *clock*, not to the buff: it has to appear at
dusk and vanish at dawn while the buff sits there unchanged, which is what a
plain stat grant gets wrong in both directions.

Java's `pump` returns early during the day, and one global `OnDayNightChange`
listener re-pumps every bearer on the flip (tracked through a static
`Set<Creature>`). This port reaches the same behaviour from the other end:
`stat_modifier_effects`, which has no clock, never emits the grant, and the new
`game_loop::night_stats` rewrites the **landed buff's** stored modifiers
whenever the answer changes — on each day/night flip, and at cast time so a
skill used at night takes effect at once rather than at the next dawn. The stat
hot path stays clock-free. There is no bearer registry here, so the flip sweeps
the in-game players and asks each buff list: the same scan-instead-of-subscribe
trade sub-slice 14's triggers make, for the same reason.

The per-flip message is Java's, quirk included — it goes only to characters who
actually **know** Shadow Sense, so somebody carrying the effect from another
source gets the stat and no message at all.

Also dropped the now-false `#[allow(dead_code, reason = "day/night query wired
when the day/night scripts land")]` from `game_time::is_night_at`.

Census: effect names **146 → 145**, learnable-affected **11 → 10**, headline
**13 → 12**. Sabotage-verified. The empty-effects guard bit a **tenth** time:
the grant is written *after* the buff lands, so at guard time the skill looks
modifier-less.

**S4 is closed.** Both names still carrying a learnable source are recorded
out-of-scope decisions rather than unexamined gaps: `StatUp` (9 learnable, all
Territory War) and `SafeFallHeight` (needs fall damage, which this port lacks).

## G34 S4 sub-slice 16 — the death pair (2026-08-02)

Branch `feat/g34-effect-sweep-16`.

- **`ReduceDropPenalty`** (Residence Death Fortune 610, Noblesse Fortune 1325)
  scales the exp lost on death by **what killed you**: a raid, an ordinary
  monster and a playable each read a different stat, in Java's `if/else if`
  order raid → monster → playable, with a `null` killer skipping all three.
  Threading the killer through `apply_death_exp_penalty_ex` was the work.
  **Fourth dead stat of this epic:** the same handler merges
  `REDUCE_DEATH_PENALTY_BY_MOB`/`_PVP`/`_RAID`, and **nothing in Java reads
  them** — so Noblesse Fortune, whose only param is `deathPenalty -100` with
  `type RAID`, does nothing whatever on this dist. Ported as written; the dead
  twin is not modelled and the census stops counting the name.
- **`ResurrectionSpecial`** (Salvation 1410, Soul of the Phoenix 438) is the
  auto-resurrect, and its mechanic sits in the lifecycle hook nobody would
  guess by eye: the buff does nothing at all while it is up, and fires its
  revive proposal from **`onExit`** — which is what death does to it. Wiring it
  to `onStart` would propose a revive to a living player and then do nothing
  when they actually died. Its `RESURRECTION_SPECIAL` flag has a second,
  separate job in `Playable.doDie`: exactly like Noblesse Blessing, the holder
  stops *only that effect* and keeps every other buff through death — without
  which the auto-resurrect would bring you back stripped. `death.rs` already
  carried a comment noting this second source was missing.

Census: effect names **148 → 146**, learnable-affected **15 → 11**, headline
**17 → 13**. Three tests, all sabotage-verified.

Two familiar traps recurred. The empty-effects guard bit a **ninth** time, and
here the fix was the faithful one rather than another guard-list entry: Java
stamps `EffectFlag.RESURRECTION_SPECIAL`, so stamping it too both keeps the
buff alive and lands the `doDie` behaviour. And the empty-fixture-table trap —
`xp_lost` is empty in the fixture, and the first draft also set `exp` exactly
on the level threshold, so the delevel clamp zeroed the loss and the test
measured nothing at all.

## G34 S4 sub-slice 15 — servitors and recall (2026-08-02)

Branch `feat/g34-effect-sweep-15`.

- **`Betray`** (1380) turns somebody's servitor against them, and it takes three
  things — a port doing only the first would look plausible. The AI points at
  the **owner** (routed through the ordinary attack order, so it stops
  following and arms the attack timeout); the servitor stops taking commands,
  with Java's own "your servitor is unresponsive and will not obey any orders";
  and `SummonInfo` status bit `0x01` goes up, which is what makes it
  **auto-attackable** so the owner can put their own pet down. The `BETRAYED`
  flag was one of the five S3 held back for S4.
- **`ImmobilePetBuff`** (Servitor Empowerment 1299) roots the servitor for the
  duration — the same `IMMOBILIZED` flag `BlockMove` uses, and it has to come
  back off at expiry or the servitor is stuck for good. Java's
  `effector == effected || owner == effector` gate is satisfied by construction:
  the skill is `targetType SUMMON`, resolving to the caster's *own* servitor, so
  it cannot be aimed at someone else's pet. `TODO(G34)` at the site in case a
  carrier ever uses a wider target type.
- **`CallParty`** (Chant of Gate 1429) recalls every *other* party member to the
  caster. It is **not** Summon Friend: Java calls `teleToLocation` outright, so
  there is no `ConfirmDlg` and the members get no say in it. Each member is
  gated by CallPc's shared `checkSummonTargetStatus`, whose refusals are
  messaged to the **caster** rather than the member left behind. The ported
  subset is dead / private store / in combat, with a `TODO(G34)` naming the
  states this port does not model yet (rooted, olympiad, observer, flying
  mount, combat flag, the `NO_SUMMON_FRIEND`/`JAIL` zones, instance
  permissions).

Census: effect names **151 → 148**, learnable-affected **18 → 15**, headline
**20 → 17**. All three tests sabotage-verified.

## G34 S4 sub-slice 14 — the trigger pair (2026-08-02)

Branch `feat/g34-effect-sweep-14`. The two remaining `TriggerSkillBy*` shapes,
both listener-driven in Java.

- **`TriggerSkillByDamage`** (Mirage 445) is the mirror of
  `TriggerSkillByAttack`: it fires when the bearer **takes** a hit, and casts
  back at the attacker rather than on itself. Two gates separate it from the
  attack-side twin and both are exactly what a copy-the-other-one port would
  drop — `attackerType` (Mirage restricts to `Playable`, so a monster hitting
  you never sets it off) and `hpPercent`, an *upper* bound that arms the
  trigger only once the bearer is hurt enough. Hooked at `apply_attack_damage`,
  the shared choke point, because Java fires `OnCreatureDamageReceived` from
  `reduceCurrentHp` — so unlike the attack twin this one sees **skill damage
  too**.
- **`TriggerSkillByMagicType`** (Dance of Shadows 366) fires when the bearer
  *finishes casting* a skill whose `magicType` is in its list, which is how the
  dance's stealth ends the moment you act: any ordinary cast fires Cancel
  Shadow Move on the party. Hooked at `handle_skill_finish`.

Census: effect names **153 → 151**, learnable-affected **20 → 18**, headline
**22 → 20**. Both tests sabotage-verified.

Two things worth carrying forward:

- **Carriers are buffs, not book entries.** The first cut scanned the bearer's
  `SkillBook`, copying `fire_attack_triggers`. That is correct *there* — its
  carriers are weapon-mastery passives, folded into `StatModifiers` and so
  absent from the buff list — and wrong here: Java attaches the listener to the
  **effect instance**, and Mirage is a timed buff, so knowing the skill is not
  the same as being under it. Both new evaluators scan `Buffs` instead, and the
  Mirage test's negative case is "not cast yet".
- **The empty-effects guard bit again**, seventh and eighth times. A trigger
  carries no stat modifier, no periodic tick and no `effect_flag`, so its buff
  was dropped on landing and the trigger could never fire at all. Any new
  modifier-less effect has to join one of that guard's three categories.

## G34 S4 sub-slice 13 — the physical-attack pair (2026-08-02)

Branch `feat/g34-effect-sweep-13`.

- **`PhysicalAttackHpLink`** (Fatal Counter 314, Fatal Arrow 10905) is
  structurally `PhysicalAttack` — identical fields and formula, so the two
  share one match arm rather than duplicating forty lines of damage assembly —
  with `DeathLink`'s multiplier on the end, keyed on the **caster's** missing
  HP (`−(curHp·2 / maxHp) + 2`). A healthy archer's Fatal Counter does nothing
  at all; a dying one's hits for double, exactly as the skill describes itself.
  Two Java defaults differ from `PhysicalAttack`'s and both matter:
  `criticalChance` defaults to **0** rather than 10 (Fatal Counter declares
  none, so it never crits), and there is no `ignoreShieldDefence` param, so
  `calcShldUse` always runs.
- **`PolearmSingleTarget`** (Focus Attack 317) is the **cost** half of a toggle
  whose two bonuses — accuracy and critical damage — had landed through the
  effect registry long ago. Until this slice the skill was a pure bonus with
  nothing given up: the polearm sweep it exists to trade away kept happening.
  `sweep_targets` carried a `TODO(G20)` noting that no ported effect set the
  stat; it does now, as an ordinary additive 1 (Java's `addFixedValue`, but
  nothing else on this dist touches the stat and the single read site only
  asks `> 0`).

Census: effect names **155 → 153**, learnable-affected **22 → 20**, headline
**24 → 22**. Three tests, all sabotage-verified.

The third test exists because of a trap pointing the *opposite* way from the
usual one. Checking the sweep gate against a hand-inserted stat modifier proves
the **consumer** and says nothing whatever about the **grant** — the mirror
image of the registry-line-without-a-consumer failure this epic keeps finding.
Focus Attack is therefore tested twice: that casting it grants the stat and
expiry hands it back, and that the stat actually suppresses the sweep.

## G34 S4 sub-slice 12 — the PvP/PvE balance family (2026-08-02)

Branch `feat/g34-effect-sweep-12`. **Fifteen effect names in one slice**, and
in the right order: the consumer first, the registry lines after. All fifteen
feed one function — `Formulas.calculatePvpPveBonus` — a term in *every* damage
formula that this port hard-coded to 1.0 in three separate places, each behind
a comment asserting the pvp/pve mods were 1.0. True only while nothing granted
the stats; the dist has **~1300 effects that do**.

- **The shape** is a difference of multipliers, not a product. Each side merges
  as a `mul` (`amount 5` → ×1.05), and the result is
  `max(0.05, 1 + (attackMul − defenceMul))` — so +50 % attack against +50 %
  defence cancels to exactly 1.0 where a product would read 2.25.
- **The branch** is chosen by pairing *and* delivery: playable-vs-playable
  reads the `PVP_*` triple, anything involving an `Attackable` reads `PVE_*`,
  and within each, an auto-attack (Java's `skill == null`), a magic skill and a
  physical skill read three different stat pairs.
- **The PvE level-difference penalty** came with it —
  `SkillDmgPenaltyForLvLDifferences`, an eight-entry table on this dist
  bottoming out at **×0.25** (much steeper than Mobius' four-entry default)
  that the port never parsed. It bites only on a non-raid NPC at or above
  `MinNPCLevelForDmgPenalty` (78) standing 2+ levels above the attacker.

Wired into all five Java call sites: `calcBlowDamage`, `calcMagicDam`,
`calcManaDam`, `calcAutoAttackDamage`, and the physical-skill handlers.

Census: effect names **170 → 155**, learnable-affected **24 → 22**, headline
**26 → 24** — and *reachable* **1684 → 1355**, the largest single drop of this
epic. Four new tests, all sabotage-verified, one of them end-to-end: a helper
that computes the right number and is never multiplied in is exactly the
failure mode this epic keeps turning up, and the three "pvp-pve mods 1.0"
comments were three call sites to edit, not just stats to register.

Three findings recorded rather than smoothed over:

- `DmgPenaltyForLvLDifferences` and `CritDmgPenaltyForLvLDifferences` are
  parsed by Java's `Config` and read by **nothing at all**. Dead config on both
  sides — deliberately not modelled here.
- Java binds `targetPlayer = attacker.getActingPlayer()`, the *attacker* again.
  It is used only to index class-balance config arrays, and this dist's
  `Custom/ClassBalance.ini` ships every multiplier blank, so the slip cannot be
  observed. Ported as written.
- The raid `*_DEFENCE` terms are likewise read off the **attacker** rather than
  the target. Same treatment; inert here, since the only carriers are three
  item skills consulted only while the attacker is a raid.

## G34 S4 sub-slice 11 — the sustain family (2026-08-02)

Branch `feat/g34-effect-sweep-11`. Three effects that all live on the periodic
tick chain or the party, and all three had substrate waiting for them.

- **`ChameleonRest`** (296) is `Relax` with two differences that matter. It
  carries `SILENT_MOVE` as well as `RELAXING`, so resting under it hides you
  from a monster's pre-emptive aggro — which *is* the skill, per its own
  description. And it has **no HP-full stop**: Relax retires itself once there
  is nothing left to heal, while this one runs until you stand up or run out of
  MP. Reusing Relax's arm wholesale would switch the skill off exactly when a
  healthy player wanted to hide.
- **`ManaHealOverTime`** (Force Meditation 441, Invocation 1430, Soul Harmony
  1480) — the mirror of `ManaDamOverTime`. Java's early-outs are asymmetric and
  both are kept: a *positive* power stops once MP is full, a *negative* one
  stops when the tick would reach zero, and the write floors at **1** rather
  than 0 — a drain wearing this handler can never empty the pool outright.
- **`RebalanceHP`** (Balance Life 1043) — pool the HP of every living party
  member in range (plus their pets and servitors), take the party's average
  *percentage*, and set everyone to it. A **redistribution, not a heal**: the
  total is conserved, so the healthy pay for the dying. Only a member whose HP
  goes up is clamped by `MAX_RECOVERABLE_HP` (and one already above that
  ceiling keeps what they have); a member losing HP is written unconditionally.
  Java's `if (party != null)` guard means an unpartied cast is simply wasted —
  it does *not* fall back to the "party of one" reading the other party-scoped
  effects use.

Census: effect names **173 → 170**, learnable-affected **28 → 24**, headline
**30 → 26**. All five new tests sabotage-verified.

The test-trap streak continued, and this one is the sharpest yet. The no-party
test originally used a **solo caster** — and a solo rebalance is arithmetically
a no-op, since the average of one member is that member's own percentage. The
sabotage (swapping in a "party of one" fallback) left it green. Giving the
caster a **pet** makes the guard observable: under the fallback the pair
rebalance against each other, under Java's guard neither moves, and the
sabotage now reads 250 → 625.

## G34 S4 sub-slice 10 — Unlock, end to end (2026-08-02)

Branch `feat/g34-effect-sweep-10`. Skill 27 in full, which took three pieces
and closed a **target-type** gap alongside two effect names.

- **`TargetType::DOOR_TREASURE`** (`targethandlers/DoorTreasure.java`) — the
  selection itself is the whole validation: a door or a chest passes, anything
  else is `THAT_IS_AN_INCORRECT_TARGET`. It runs no range, LOS, peace-zone or
  alive/dead gate, which is exactly what lets one skill target a closed door
  (not attackable) and a chest (attackable) down the same path. This was the
  last learnable entry on the target axis.
- **`OpenDoor`** — a per-level chance (30/50/75, then 100 from level 4) and
  **two different refusals**: a door that is not `openMethod="BY_SKILL"` cannot
  be picked at all, and says so; a `BY_SKILL` door that misses its roll gets
  the softer "you have failed to unlock the door" and can be retried. Java also
  refuses fort doors — this port has no forts, and for the *skill* path that
  gate is vacuous here (none of the 34 `BY_SKILL` doors is a fort door: they
  are Cruma, Devil's Isle, the Water Garden, Rune ToH and the Four Sepulchers),
  but it is **not** vacuous for an item-cast unlock, so a `TODO(G34)` marks it.
- **`OpenChest`** — a level *band*, not a roll: within 6 levels (5 above 77)
  the box opens, outside it the box turns on you. Opening it kills the chest
  with `setSpecialDrop()` + `setMustRewardExpSp(false)`, and the other half of
  the effect therefore lands in `death.rs`: an unlocked box pays no exp/sp, and
  a box that was merely smashed rolls a **different npc id's** drop list.

Two dist findings, recorded rather than papered over:

- **No `type="Chest"` NPC is spawned anywhere on this datapack.** All 48 chest
  templates exist; none appears in `spawns/`, and the Four Sepulchers boxes
  (31467/31468) are `type="Folk"`. `OpenChest` is reachable today only through
  `//spawn`. Ported regardless — chest spawns are a data change, not a code one.
- **The smashed-chest drop remap points at ids that do not exist here.**
  18265-18286 shift by +3536 into 21801-21822 and the six fixed pairs map onto
  21671/21694/21717/21740/21763/21786; not one of those is an NPC template on
  this dist, and the chest templates carry no `<drops>` of their own either.
  Java feeds that null straight into `calculateDrops` and throws. The port
  implements the swap and falls back to the chest's own list when the target is
  missing — the only non-crashing reading of the same code.

Census: effect names **175 → 173**, learnable-affected **29 → 28**, target
types **11 → 10** (learnable 4 → 3), headline **31 → 30**. All three fixes
sabotage-verified.

One more test trap, the same family as sub-slice 9's pair: the exp-gate
assertion first passed for the wrong reason. `cc2_world` ships an **empty**
experience table, so the level cap is −1 and *every* award clamps to −1; and
18265 declares no `<acquire>` at all, so there was no exp to withhold in the
first place. Loading a real table and giving the chest a reward turns the
sabotage signal from "0 vs −1" into "0 vs 500".

## G34 S4 sub-slice 9 — the bespoke three (2026-08-02)

Branch `feat/g34-effect-sweep-9`. Three effects with nothing in common except
that each needs its own code path — the point where the stat-shaped bulk of S4
runs out.

- **`Bluff`** (Blinding Blow 321, Bluff 358) spins the target to face the
  **caster's** heading, which is what sets a Backstab up. "Set the heading" is
  only half of it: the turn must be *broadcast*, and the two packets that do so
  did not exist here — `StartRotating` (0x7A) and `StopRotating` (0x61), now in
  `network/server_packets/movement.rs`. Java bails on `isRaid()`; it also bails
  on `isRaidMinion()`, for which this port has no predicate (raid minions carry
  ordinary `Monster` templates and are tracked through the leader's
  `MinionList`), so a `TODO(G34)` marks that half at the site.
- **`Unsummon`** — the servitor-removal half, distinct from the summon-side
  unsummon already ported.
- **`DeathLink`** (Death Link 1177) scales its power by the caster's
  **missing** HP: `power * (2 − 2·curHp/maxHp)`. At full health the multiplier
  is **0** — the skill lands, costs MP and reuse, and does nothing at all. A
  port that dropped the scaling reads plausibly at every HP except the two
  ends, which is why this one is easy to leave broken.

Census: effect names **178 → 175**, learnable-affected **33 → 29**, headline
**35 → 31**. Both new tests sabotage-verified.

Two test traps, both of the "the test passes for the wrong reason" family:

- The `Bluff` test gave the raid boss level 40 against a level-20 mob. The
  *land rate* then spared the boss, so the test passed with the raid exemption
  deleted — it was measuring the level gap, not the exemption. Equalising the
  levels leaves the template as the only difference.
- The `DeathLink` test read 1 damage at full HP and I went looking for a
  minimum-damage floor. There isn't one: `calc_magic_dam` pins a **magic
  failure** at 1 damage regardless of power, and the forced-roll sequence
  landed there. The test now turns `magic_failures` off so it measures the
  multiplier rather than the failure roll.

Also fixed, unrelated and pre-existing: `an_hour_chosen_after_arming_is_honoured`
asserted `next_siege_millis(now, 6, 20) > next_siege_millis(now, 6, 16)`, which
is false for the four hours between them on a Sunday — at 18:39 the next 16:00
is a week out while 20:00 is 81 minutes away. It now derives the later hour
from the earlier one, so it no longer depends on when the suite runs.

## G34 S4 sub-slice 8 — the heal ceiling (2026-08-02)

Four interlocking effects, branch `feat/g34-effect-sweep-8`: the cap and the
heals that read it.

- **`LimitHp` / `LimitCp` → `MAX_RECOVERABLE_HP` / `_CP`** is the ceiling a
  **heal** may restore to — `getValue(stat, getMaxHp())`, so identity is the
  full pool. The learnable sources are **restrictions**, not bonuses: Noblesse
  Harmony (1326) and Symphony (1327) grant them `PER −30` / `−40`, so under
  those auras a character can only be healed back to 70 % HP and 60 % CP. The
  port clamped every heal to the raw pool, which is **identical until someone
  casts them** — which is exactly why it looked right for so long, and the
  shape to watch for in the rest of this sweep.
- **`CpHealPercent`** (Victories of Pa'agrio 1414 at 20 %) restores a share of
  **max CP** and honours `MAX_RECOVERABLE_CP`. Java's guards are dead / door /
  *HP*-blocked — the last is not a typo: the CP heal reads `isHpBlocked`.
- **`HpByLevel`** (Life Scavenge 46, Corpse Life Drain 1151) heals the
  **effector** — the caster, not the target. Every other heal in the family
  reads `effected`, so pointing this one at the target would heal the corpse
  being drained. It also clamps to `getMaxHp()` rather than the recoverable
  cap: the one heal here that ignores it, ported as written.

Census: effect names **182 → 178**, learnable-affected **39 → 33**, headline
**41 → 35**. Sabotage-verified both the ceiling and the heal direction.

## G34 S4 sub-slice 7 — positional crit rate and MP vampirism (2026-08-02)

Branch `feat/g34-effect-sweep-7`.

- **`CriticalRatePositionBonus`** (Focus Chance 356) is the crit-*rate* twin of
  `CriticalDamagePosition`. `calcCriticalPositionBonus` hard-coded Java's
  1.0/1.1/1.3, i.e. the positional `CRITICAL_RATE` stat pinned at identity, so
  the skill did nothing. It is the only skill on this dist declaring **all
  three** positions — −30 % front, +30 % side, +60 % back — so it rewards a
  rogue who circles and *punishes* one who stands in front. Implementing only
  the side/back terms would read as a pure buff and pass any back-attack test.
  The `by_position` merge map already existed from `CriticalDamagePosition`, so
  this was a parser arm plus threading the multiplier into two call sites.
- **`MpVampiricAttack`** (Weapon Mastery 250) is the MP twin of the HP drain,
  and **its config gate is shaped the opposite way** — the detail worth
  keeping. HP vampirism asks `skill == null || VAMPIRIC_ATTACK_WORKS_WITH_SKILLS`
  (melee by default, config to reach skills); MP vampirism asks
  `skill != null || MP_VAMPIRIC_ATTACK_WORKS_WITH_MELEE` (**skills** by
  default, config to reach melee). Both configs are off here, so Weapon Mastery
  drains MP on skill hits and nothing at all on a swing. Java's "do not absorb
  if weapon is ranged" guard wraps the **HP** block only and is deliberately
  absent. One `<amount>` pumps two values: the percentage and a `sum` that the
  chance finalizer divides back out.
- **`CubicMastery` → `Stat.MAX_CUBIC` is dead in Java** — the second such find
  after `ABNORMAL_SHIELD`. Nothing in `java/` or the datapack scripts reads it;
  the cubic limit is `Config.ALLOWED_CUBIC_COUNT`. Registered so the skill's
  buff survives the empty-effects guard, with **no consumer**, because Java has
  none either.

Census: effect names **185 → 182**, learnable-affected **43 → 39**, headline
**45 → 41**.

**Left out deliberately:** `SafeFallHeight` (Acrobatics 173) needs `Stat.FALL`,
whose only consumer is fall damage — **which this port does not implement**.
Registering it would have shrunk the census for an effect that could not
possibly work, so it stays on the list where the harness keeps naming it.

**Flaky-test note.** The MP-vampiric regression first used Weapon Mastery's own
numbers (`amount 10` → sum 300), which give a chance of
`min(1, 300 / (0.1 × 100) / 100)` = **0.3** — Java's own "Classic: 30% chance".
The test passed or failed at random until the `sum` was raised to make the
chance exactly 1.0, so the assertion is about the *gate* and not the roll.

## G34 S4 sub-slice 6 — skill mastery and Lucky (2026-08-02)

Branch `feat/g34-effect-sweep-6`.

**`SkillMastery` + `SkillMasteryRate`** (Skill Mastery 330 STR / 331 INT, Focus
Skill Mastery 334) — the cooldown-collapse proc: a successful roll drops a
cast's reuse to 100 ms and announces "A skill is ready to be used again".

The trap is that **`Stat.SKILL_MASTERY` is not a magnitude**. It stores the
*ordinal of the `BaseStat`* that drives the chance, which `calcSkillMastery`
reads back with `BaseStat.values()[val]` — and **Java's enum order differs from
this port's**: Java is `STR, INT, DEX, WIT, CON, MEN, CHA, LUC`, the port is
`Str, Dex, Con, Int, Wit, Men`. Copying Java's number across would make Skill
Mastery 331 (INT) select DEX, silently and plausibly. Parsed by **name** into
the port's own discriminant instead, with the reason written at
`BaseStat::from_name`. The regression sabotages exactly that swap.

Java's three exclusions come with it — static skills, item-cast skills
(`getReferenceItemId() != 0`) and anything not `operateType A1` never proc. The
last is expressible here because `OperateType::Active` collapses A1/A2 while
`is_continuous` is precisely the A2..A6/DA2..DA5 family, so `Active &&
!is_continuous` **is** A1. `Config.SKILL_MASTERY_CHANCE_MULTIPLIERS` is left
out: per-class, default `1f`, unset on this dist.

**`Lucky`** (194) is an **empty effect** in Java: its handler carries only a
`canStart` player guard and no mechanic at all. `Player.isLucky()` is
`getLevel() <= 9 && isAffectedBySkill(194)`, so the buff's *presence* is the
entire implementation — it exempts a newbie from the death exp penalty. Both
halves of that predicate are asserted, since the buff alone must not carry a
level-10 character. Java's second reader (the vitality-consumption branch) has
no counterpart in this port yet; `TODO(G34)` at the site.

Census: effect names **188 → 185**, learnable-affected **47 → 43**, headline
**49 → 45**.

**Test note.** The stat-selection assertion needs the *real* `statBonus` table:
`GameData::for_test`'s stub returns 1.0 for every stat, which makes "which
`BaseStat` was selected" unobservable — the exact property under test. Loading
`StatBonus` alone (not the whole dist) keeps it at 0.03 s. The discriminating
roll is then derived from the two real chances rather than guessed, so the
assertion cannot pass for the wrong reason.

## G34 S4 sub-slice 5 — buff slots and self-dispel (2026-08-02)

Two effects with existing consumers, branch `feat/g34-effect-sweep-5`.

- **`EnlargeAbnormalSlot`** (Divine Inspiration 1405, +1..+6 slots) raises the
  **good-buff** cap and only that pool — Java's `setMaxBuffCount` is read by
  `EffectList` for buffs, never for dances. Modelled as a `Stat`
  (`MaxBuffSlots`) rather than Java's direct setter **on purpose**:
  `apply_buff` rebuilds `StatModifiers` from the surviving buffs on every
  change, so the bonus is *derived* rather than accumulated and cannot drift
  the way an add-on-start/subtract-on-exit pair can when a buff leaves by some
  path that skips `onExit` — which, given the empty-effects guard has now
  bitten six slices, is a failure mode worth designing out rather than
  testing for. It also reads `<slots>`, not `<amount>`, so the generic stat
  registry could not have taken it.
- **`DispelBySlotMyself`** (Flames of Invincibility 1427) strips the bearer's
  own buffs of the listed abnormal types. Two differences from `DispelBySlot`,
  both load-bearing: the list carries **no levels** (`TYPE`, not `TYPE=level`),
  and an **`irreplacableBuff` is spared** — the same tag S3 folded into
  `stay_after_death`, so the two fixes compose.

Census: effect names **190 → 188**, learnable-affected **49 → 47**, headline
**51 → 49**. Sabotage-verified both.

## G34 S4 sub-slice 4 — the mitigation / counter family (2026-08-02)

`AreaDamage`, `TransferDamageToSummon`, `CounterPhysicalSkill`, `SkillEvasion`
and `SkillTurning` — five names, eight learnable skills, five distinct
consumers. Branch `feat/g34-effect-sweep-4`.

- **`AreaDamage` → `DAMAGE_ZONE_VULN`**, folded into the damage-zone tick as
  `1 + (value / 100)`. The stat's name is misleading and the datapack settles
  it: Iron Body (295) grants **−40**, Dance of Protection (311) **−30**, so
  both learnable sources are *mitigation*.
- **`TransferDamageToSummon`** redirects a share of incoming player damage to
  the first servitor **within 1000 units**, clamped to `currentHp − 1` so
  Transfer Pain can never kill the pet it is protecting you with — ahead of the
  CP pool, exactly where `PlayerStatus.reduceHp` puts it.
- **`CounterPhysicalSkill`** grants a **chance** (20 % / 90 %), not a
  multiplier, and Java runs the counter *before* the damage lands. Two guards
  matter, and both would look correct in a melee-only test: **magic is never
  counterable**, and neither is anything with `castRange > 40`.
- **`SkillEvasion`** lives in a **per-`magicType` map**, not a `Stat`. Both
  learnable sources are bucket 0, so the buff dodges physical skills and leaves
  magic alone; a single global dodge stat would pass any test that fires one
  kind of skill.
- **`SkillTurning`** — Spell Turning (1412) is, despite the name, an offensive
  `ENEMY_ONLY` instant that breaks the *target's* cast. Self-casts and raid
  bosses are exempt.

Census: effect names **195 → 190**, learnable-affected **57 → 49**, headline
**59 → 51**.

Three things worth keeping from how this went wrong first:

- **`CounterPhysicalSkill` was briefly in `EFFECT_REGISTRY` with no consumer.**
  The census shrank while the effect did nothing — the exact anti-pattern S4's
  own preamble warns about, committed by the person who wrote the warning.
  Caught before commit. **A registry line is not a port.**
- **The empty-effects guard claimed a sixth victim.** `SkillEvasion` merges into
  a per-bucket map that only `handle_buff_expire` unmerges, so a dropped buff
  made the dodge **permanent**.
- **A `get_component_mut` write silently no-ops when the component is absent**
  ([[l2r-conditional-writes-fail-open]]): NPCs do not all carry
  `StatModifiers`, so `SkillEvasion`'s merge did nothing on a mob until it was
  rewritten as insert-then-merge.

Also fixed here: `the_skill_power_stats_scale_finished_skill_damage` (sub-slice
2) loaded the whole dist **four times** and began timing out under parallel
load. It now loads once and resets the target between measurements.

## G34 S4 sub-slice 3 — the aggro family (2026-08-02)

`HateAttack` (Sword/Blunt Weapon Mastery 217), `TargetMe` (Aggression 28,
Aggression Aura 18) and `TargetMeProbability` (Vengeance 368), branch
`feat/g34-effect-sweep-3`. Each carries a Java guard that is easy to miss and
that a naive test would not notice.

- **`HATE_ATTACK` scales auto-attack hate only.** Java applies it inside
  `Attackable.reduceCurrentHp`'s `if (skill == null)` branch. So the mastery
  helps a tank hold aggro through ordinary swings and does **nothing** for
  their taunts — implementing it as a blanket hate multiplier would be wrong in
  precisely the situation tanks care about. `apply_physical_damage` grew a
  `from_skill` flag to carry Java's `skill != null`; reflect and zone damage
  pass `false`, matching Java's null skill on both paths.
- **`TargetMe`/`TargetMeProbability` only affect playables.** Both handlers are
  wrapped in `if (effected.isPlayable())`, so taunting a *monster* through them
  does nothing at all — which is exactly why Aggression declares `GetAgro` as
  well. One skill needs both effects to taunt both kinds of target, and the
  old parser test asserting "TargetMe stays unported, dropped" has been updated
  to expect the pair.
- **`TargetMe` also locks the target.** `Npc.canTarget` then refuses any
  *other* NPC with "Failed to change enmity" — NPC-side only, so the victim can
  still click players and items.

**The empty-effects guard has now caught a fifth slice.** `TargetMe` carries no
stat modifier and stamps no `effect_flag`, so its buff was dropped by the
guard, `handle_buff_expire`'s `onExit` hook never ran, and **the taunt lock
became permanent** — a worse bug than the one being fixed. It now joins the
guard's icon-only category. The rule stands: any new modifier-less effect must
join one of the guard's three categories (*periodic* / *icon-only* / *state
flag*), and the failure mode is never "nothing happens" — it is whatever the
missing `onExit` was supposed to undo.

Census: effect names **198 → 195**, learnable-affected **61 → 57**, headline
**63 → 59**. Sabotage-verified both gates (apply `HATE_ATTACK` regardless of
skill; drop the `isPlayable()` guard).

## G34 S4 sub-slice 2 — the skill-damage multipliers (2026-08-02)

Four stats that were pinned at identity on the damage paths
([PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md)), branch
`feat/g34-effect-sweep-2`.

- **`PhysicalSkillPower` / `MagicalSkillPower`** — the last multiplier a
  skill's damage passes through. Java applies the physical one from each
  `PhysicalAttack`-family **effect handler**, but the magical one from **inside
  `calcMagicDam`** — so every caller of that function gets it, `HpDrain`
  included, even though `HpDrain.java` never mentions the stat. Reading only
  the handlers would have missed that path
  ([[l2r-two-java-call-sites]] again: grep the helper, not the feature).
  Focus Skill Mastery (334) is the learnable source.
- **`PhysicalSkillCriticalDamage` + `DefencePhysicalSkillCriticalDamage`** —
  `Formulas.calcCritDamage` reads the **skill** crit stats when a skill is
  involved, not `CRITICAL_DAMAGE`. The port's physical branch was a literal
  `2.0`, i.e. both stats pinned at 1, so Heroic Berserker (396) — the learnable
  source — did nothing. `balanceMod` stays 1: its
  `Config.PV*_*_CRITICAL_DAMAGE_MULTIPLIERS` tables are per-class, default
  `1f`, and this dist sets none of them.

Census: effect names **202 → 199**, learnable-affected **63 → 62**, headline
**65 → 64**. Sabotage-verified both (return identity from `skill_power_mul`;
force the magic branch in `crit_damage_skill`).

**Fixture trap worth remembering:** the skill-power regression first ran
against the standard fixture mob's 100 HP, so the doubled hit was clamped to
the mob's remaining HP and the ×2 read as ×1.1. **A damage-multiplier test
needs a pool deeper than the biggest hit under test** — otherwise the clamp,
not the multiplier, is what the assertion measures.

## G34 S4 sub-slice 1 — breath and carrying capacity (2026-08-02)

First sub-slice of the epic's largest remaining piece
([PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md)), branch
`feat/g34-effect-sweep`.

**Why S4 can't be done in bulk.** Of the 49 effect names left with a learnable
source, 14 are plain `AbstractStat*Effect` — one `Stat` and a mode. That looks
like 14 registry lines. It is not: **none of those `Stat`s exists in this
port**, so each needs a variant, a registry entry *and* a consumer in the right
formula. Adding only the registry line would shrink the census while the effect
still does nothing — exactly what the S0 harness warns about ("recognised ≠
correctly ported"). So every entry lands with its consumer or not at all, and
this slice took the three whose consumers were already in place.

- **`Breath`** — `water.rs` hard-coded a 60 s gauge behind a comment saying "no
  skill or item on this dist declares `Stat.BREATH`". **21 skills do**: Boost
  Breath (195) and Eva's Kiss (1073) are learnable, plus the 19 Doom-armour-set
  item skills. That is the **third self-justifying deviation comment** this epic
  has turned up (after `formulas.rs`' `BasicPropertyResist` note and the
  transform-gate merge), all the same shape — a written-down claim that was
  half-checked once and believed thereafter.
- **`WeightLimit`** — the CON formula is the *base* Java's
  `getValue(WEIGHT_LIMIT, …)` applies add/mul to. Weight Limit (150) is
  `PER 300`, i.e. ×4 carrying capacity; the port ignored it entirely.
- **`WeightPenalty`** — **the name lies.** It reads like a penalty *band*, and
  the first pass here implemented it that way. Every Java caller actually
  subtracts it from the carried weight
  (`weightproc = (getCurrentLoad() - getBonusWeightPenalty()) * 1000 / getMaxLoad()`),
  and the datapack settles it: Decrease Weight (1257) grants 3000/6000/9000,
  which are weight units, not bands. Ported as the code behaves
  ([[l2r-port-behaviour-not-intent]]).

Both modes matter for `Breath` and read very differently against the 60 000 ms
base: Eva's Kiss is `PER 400` (×5 — five minutes), Boost Breath is `DIFF 180`
(+0.18 s). The second looks like a datapack unit slip, but Java computes
exactly that, so it is ported as written rather than "corrected".

Census: effect names **205 → 202**, learnable-affected **70 → 63**, headline
**72 → 65**.

**Test note worth keeping.** The breath regression first asserted only
`breath_ms()`, and **survived its own sabotage** — reverting the call site to
the old constant left the function correct and the test green. It now also
asserts `start_water_task`'s armed tick. Testing the helper is not testing the
consumer; sabotage the *call site*, not just the calculation.

## G34 S3 — the flag-only abnormal states (2026-08-02)

Fourth slice of the G34 epic
([PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md)), branch
`feat/g34-effect-flags`. Started by auditing **all 23 missing `EffectFlag`s**
against the dist for a reachable source, which split them three ways — writing
the split down was most of the value, since the next audit would otherwise
re-derive it.

**Nine flag-only effects ported**, each one bit plus the single Java gate that
reads it: `BuffBlock` (Dance of Medusa 367 — `EffectList.add`),
`PhysicalShieldAngleAll` (Aegis 316/318 — `calcShldUse`'s `degreeside`),
`Passive` (Veil 106, Requiem 1049 — `Monster.isAggressive()`), `Untargetable`,
`DisableTargeting` (the two halves of `Action`/`AttackRequest`'s targeting
gate), `PhysicalAttackMute` (`isAttackDisabled()`), `BlockResurrection` and
`BlockEscape` (whose gates already existed from S1), and `AbnormalShield`.

Three things worth keeping:

- **`ABNORMAL_SHIELD` is dead in Java too.** Its handler returns both the flag
  *and* `EffectType.ABNORMAL_SHIELD`, and **nothing in the entire tree reads
  either** — grepped `java/` and `dist/game/data/scripts/`. Its two item
  sources are inert on Java as well. Defined with no consumer, same shape as
  `FEAR`/`CONFUSED`/`MP_BLOCK`. Grep for readers *before* writing a gate.
- **`BUFF_BLOCK` is not the mirror image of `DEBUFF_BLOCK` it looks like.**
  Java gates on `isBuffBlocked() && !skill.isBad()` — `isBad()` (effectPoint
  < 0), *not* `isDebuff()` — and there is **no self-cast exemption**, unlike
  the debuff-block gate. Dance of Medusa stops its victim buffing themselves,
  which is the point of it.
- **`PSYCHICAL_ATTACK_MUTED` is a different lock from `PHYSICAL_MUTED`.** The
  first blocks the auto-attack (`isAttackDisabled()`), the second refuses
  non-magic *skills* (`checkUseConditions`). Sabotaging the stamp to the wrong
  one of the two is the plausible mistake, and the test catches it.

**Five flags deliberately left to S4**, where the effect has real mechanics
beyond the bit — `BETRAYED` (Betray), `RELAXING` (Relax), `SILENT_MOVE` via
`ChameleonRest`, `RESURRECTION_SPECIAL` (Salvation, Soul of the Phoenix),
`DISARMED`. The flags are defined and gated, so S4 only has to stamp them.
**Nine have no source at all on this dist** (`ATTACK_BEHIND`, `CHAT_BLOCK`,
`CHEAPSHOT`, `DOUBLE_CAST`, `DUELIST_FURY`, `FACEOFF`, `HPCPHEAL_CRITICAL`,
`IGNORE_DEATH`, `PROTECT_DEATH_PENALTY`, `PROTECTION_BLESSING`) and are not
ported.

**Buff lifecycle — a getter over three tags.** `Skill.isStayAfterDeath()` is
`_stayAfterDeath || _irreplacableBuff || _isNecessaryToggle`, and the port read
only the first. **30 learnable skills** declare `<irreplacableBuff>` with no
`<stayAfterDeath>` of their own — the whole Transform Grail Apostle / Unicorn /
Lilim Knight / Golem Guardian family — so every one of them was stripped on
death where Java keeps it. Folded at parse; an existing `TODO` closed. The
remaining lifecycle tags (`removedOnAnyActionExceptMove`,
`subordinationAbnormalType`, `abnormalInstant`, `blockActionUseSkill`,
`abnormalResists`) are **deferred to S7**, the tag/formula tail they belong
with: none is a flag, and they want the same pass as `specialLevel` and
`nextAction`.

Census: effect names **214 → 205**, learnable-affected **77 → 70**, headline
**79 → 72**. Regressions in `abnormal_tests`, sabotage-verified three ways
(drop the buff-block gate; mis-stamp `PhysicalAttackMute` as `PHYSICAL_MUTED`;
revert the three-tag fold).

## Mobs cast half as often as Java — Porta never summoned (2026-08-02)

**Reported from the live server: Porta (20213) never used its skills, above
all the "Summon" (4161) that yanks the player onto it.** The skill, the
`CallPc` effect and the AI buckets were all correct — `npc_ai_skills` puts
4073 Stun in `SHORT_RANGE`+`GENERAL` and 4161 in `GENERAL`. What was missing
was the *rate*. Branch `fix/porta-npc-skills`, plan notes in
[PLAN_G21_NPC_CASTING.md](PLAN_G21_NPC_CASTING.md).

`think_attack` opened with a "Busy swinging" early return whenever
`AttackState.attack_end_tick > now`. **Java has no such gate in
`thinkAttack`** — the mid-swing refusal lives one level down, in
`Creature.doAutoAttack`'s `isAttackDisabled()` (`isAttackingNow() ||
isDisabled()`). In Java a mob whose swing is winding down still calls its
faction, still walks, and above all still runs the cast ladder; only the *next
swing* is refused.

Because the port returned early instead, every periodic 1 s think that landed
inside a swing window died before the ladder, leaving exactly **one
`hasSkillChance()` roll per swing** (the one `ScheduledTask::NpcAttackReady`
fires at the swing's end). At Porta's 253 atk. spd. that is a roll per ~2 s
against Java's per second — and since the roll is only ~11 %, opportunities
came ~18 s apart while Stun's reuse is 6 s. The `SHORT_RANGE` rung therefore
*always* had Stun ready, and the `GENERAL` rung that holds Summon was never
reached.

Second, smaller divergence found alongside it: `thinkAttack`'s literal first
line is `if ((npc == null) || npc.isCastingNow()) return;`, which the port
never had. The 1 s think landing inside a 2 s cast fell through to the swing
tail, so a casting mob also punched.

Measured in a 3000 s melee simulation against a Porta-shaped fighter (253 atk.
spd., Stun 6 s reuse / Summon 20 s):

| | stuns | summons |
|---|---|---|
| before | 131 | **8** (one per ~6 min — never within one fight) |
| after | 264 | **44** (one per ~68 s) |

Three regression tests in `npc_cast_tests.rs`, all sabotage-verified (two fail
with the fix stashed): a mid-swing mob still casts; a mid-swing mob does *not*
start a second swing (the `doAutoAttack` half must survive); a casting mob does
not swing.

**Not fixed, noted here:** `npc_cast::check_skill_target`'s good-skill refusal
reads `!(skill.is_debuff || skill.is_bad())` where Java has `(!skill.isDebuff()
|| !skill.isBad())` — Java refuses a continuous skill on an auto-attackable
target unless it is *both* a debuff and bad; the port refuses only when it is
neither. Tightening it would make mobs cast *fewer* skills, so it is left for a
deliberate slice rather than bundled into a fix that is about casting more.

## G34 S2 — chain-stunning a mob now stops working (2026-08-02)

**Retail's PvE stun-lock resistance was missing, behind a comment that said it
couldn't exist.** `formulas.rs` carried: *"`getAbnormalResist(basicProperty,
target)` stays 0: `BasicPropertyResist` is granted by no skill on this dist …
so it can never leave its identity."* That conflates **two** terms
`Formulas.calcEffectSuccess` reads off the same `basicProperty` on adjacent
lines, and only the first half of it is true. Third slice of the G34 epic
([PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md)), branch
`feat/g34-basic-property`.

| Java | what it is | where it enters the formula |
|---|---|---|
| `getAbnormalResist` | the `ABNORMAL_RESIST_PHYSICAL/_MAGICAL` **stat** | **subtracted inside `baseMod`**, so still inside the 10–90 clamp |
| `getBasicPropertyResistBonus` | the **accrual chain** | **multiplied after the clamp**, so it can reach 0 |

The stat half really is 0 for anything a player can build toward (its two
effects have no learnable source). The chain half is granted by *nothing* — it
is earned by **being debuffed**. `Skill.applyEffects` calls
`increaseResistLevel()` after every landed debuff with a non-`NONE`
`<basicProperty>` (390 learnable skills declare one), and the counter is worth
**1.0 / 0.6 / 0.3 / 0** at level 0/1/2/3+, decaying **15 s after the last one
landed**.

Four things that are easy to get wrong, all pinned by tests:

- **Level 3 is a hard immunity, and only because of the clamp order.** Java
  multiplies the chain in *after* `constrain(rate, minChance, maxChance)`.
  Multiply before it instead — the natural-looking port — and a chain-stunned
  mob keeps taking a 10 % stun forever, since the floor rescues it. Sabotaged
  exactly that way to confirm the test catches it.
- **Accrual is on the *landed* path.** Java's call sits inside the
  `if (addContinuousEffects)` branch, past `calcEffectSuccess`, so a debuff you
  keep failing to land never builds the resistance that would lock you out of
  it. An expired chain restarts at 1 rather than resuming.
- **Mobs accrue, players do not.** `Creature.hasBasicPropertyResist()` is
  unconditionally `true`; `Player` overrides it to
  `isInCategory(SIXTH_CLASS_GROUP)`, which this dist populates with awakened
  (148+) classes only. So PvE gains stun-lock resistance and PvP chain-CC is
  untouched — backwards would silently rewrite PvP, so it is asserted directly.
- **Expiry is checked on read, never swept** — Java's own `isExpired()` inside
  `getResistLevel`. No scheduler entry, no cleanup pass, no stale-entry class of
  bug.

`PhysicalAbnormalResist`/`MagicalAbnormalResist` (both plain
`AbstractStatAddEffect`s) joined `EFFECT_REGISTRY` now that the stat has a
consumer — **effect names 216 → 214**, learnable count unchanged, which is the
shape to expect from the item-only tail.

Regressions in `basic_property_tests`: the ladder, the decay window, the
mob/player asymmetry, both formula insertion points, and an end-to-end real-dist
Stun Attack (100) that accrues when it lands and does not when it is resisted.
Sabotage-verified twice (drop the accrual → the end-to-end test fails; move the
chain term before the clamp → the formula test fails).

**One test trap worth remembering:** `forced_rolls` is a queue shared by *every*
roll a cast makes, and a physical skill rolls for crit before the effect-land
roll — seeding one value seeds the wrong one. The end-to-end test fills the
queue with a uniform value instead, chosen so the outcome is the same wherever
in the sequence it lands ([[l2r-forced-rolls-flake]]).

## G34 S1 — skill conditions are enforced (2026-08-01)

**215 of the 758 learnable skills on this dist fired where Java refuses them,**
because the parser read exactly one condition (`OpExistNpc`) and ignored the
other 110 `block/name` pairs. Bow and dagger skills cast bare-handed, force
skills with no charges, party-only skills on strangers, Revival (the emergency
self-heal, `RemainHpPer LESS 10`) at any HP at all. Second slice of the G34 epic
([PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md)), branch
`feat/g34-skill-conditions`.

`<conditions>` / `<targetConditions>` / `<passiveConditions>` now parse into a
`Vec<SkillCondition>` per Java `SkillConditionScope` — the latter two were never
even *entered* by the old parser — and `skills::conditions::check_cast`
evaluates GENERAL then TARGET from `use_magic_on`, **after target resolution**,
exactly where `Player.useMagic` calls `skill.checkCondition(this, target)`.
**28 condition kinds** cover every one with a learnable source, led by
`EquipWeapon` (88 skills), `CanTransform` (32) and `CanSummon` (24).

| | before | after |
|---|---|---|
| unported condition `block/name` pairs | 111 | **69** |
| learnable skills with an unenforced condition | 215 | **1** |
| learnable skills wrong (effect **or** condition) | 275 | **79** |

Design notes worth keeping:

- **Conditions are level-tabled like effects.** `OpEnergyMax`'s `amount` is a
  7-level `<value level="N">` table and `RemainHpPer`'s uses ranged
  `fromLevel`/`fromSubLevel` rows, so they reuse the effect param machinery
  rather than being read as flat scalars.
- **Java sends *both* messages.** The failing handler's own line first (e.g.
  "your force has reached maximum capacity"), then
  `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` — suppressed only when the caster
  aimed a *bad* skill at themselves. The inline transform block this replaces
  sent only the first.
- **Both ad-hoc gates were folded in.** `OpExistNpc` lost its dedicated `Skill`
  field and its inline check; `CanTransform` lost its inline block. One
  representation each. That also fixed an ordering divergence: Java checks
  `isSkillDisabled` *before* target and conditions, so re-clicking a 4-hour
  transform gives the **reuse** message, not "already polymorphed" — the old
  test asserted the port's own ordering.
- **The census is driven by the builder, not the parse.** A condition is
  recorded as a gap only when `build_condition` returns `None`, so porting one
  shrinks the census automatically; there is no second "ported names" list to
  keep in step.
- **Four fixtures were wrong and are now right.** Sonic Focus, Sonic Blaster and
  both Lethal Blow tests cast bare-handed; the Revival test cast at 20 % HP
  against its own `LESS 10` gate. They passed only because nothing was enforced
  — the inverse of [[l2r-fixture-hides-testcase]].

Two things left out on purpose, recorded rather than hidden:

- **`OpSweeper`** — Java re-runs the skill's whole affect scope and asks each
  corpse about spoil ownership, corpse age and free inventory; `effects::sweep`
  already does all of it at *apply* time with the right per-corpse messages, so
  gating the cast too would double every message. It is the single remaining
  learnable entry in the condition census.
- **`<passiveConditions>` is wired but inert on this dist**, and that is a
  finding, not an omission. Both learnable users are covered elsewhere: Sword
  /Blunt Weapon Mastery (205) carries the same `<weaponType>` on its own `PAtk`
  effect, and Inner Rhythm (428) declares `TargetMyParty` in a passive block,
  which Java answers `false` to (no target) — disabling the passive outright,
  which is not reproduced. A first draft of the regression test claimed 205's
  block was "the only thing tying the bonus to a sword"; that came from a grep
  truncated before the `<weaponType>` at the *end* of the effect, and the test
  duly survived its own sabotage. Deleted rather than shipped: **a test that
  passes under sabotage is proving something else.**

One deliberate deviation kept: Java has *two* transform gates —
`ConditionPlayerCanTransform` (the `DocumentBase` item-condition system) ends
with a registered-on-an-event leg, while `CanTransformSkillCondition` (what a
`<skill><conditions>` block resolves to) does not. The port keeps the stricter
leg on the skill path rather than silently opening it up, documented at the one
site instead of implied by a merged block.

Regressions in `skill_condition_tests` (equip-weapon refusal incl. the
wrong-weapon case, `OpEnergyMax`'s two-message refusal, `RemainHpPer` banding,
the GM bypass), sabotage-verified: disabling `check_cast` fails three of the
four plus `op_exist_npc_gates_recasting_next_to_a_seal`.

## G34 S0 — the skill parser now says what it drops (2026-08-01)

**The skill parser is fail-open, and nothing said so.** An `<effect name>` it
doesn't recognise yields no `SkillEffect`; an effect list that ends up empty is
then dropped by `apply_skill_effects`' empty-effects guard; a `<condition>` it
doesn't recognise is simply not enforced. The skill still loads, still casts,
still plays its animation, still burns MP and enters reuse — and does nothing,
or fires where Java would have refused it. G19 grew coverage on demand across
~25 slices, so the residue was never measured, only rediscovered one player
report at a time ("Bluff doesn't do anything", "I can cast Rapid Shot without a
bow"). First slice of the **G34 epic**
([PLAN_G34_SKILL_PARITY.md](PLAN_G34_SKILL_PARITY.md)), branch
`feat/g34-skill-census`.

`SkillGaps` now records every drop at the fallback arm itself — so it cannot
drift from the match — in seven categories: unrecognised `<effect name>`;
effects declared in an `<*Effects>` scope this port never builds
(`startEffects`/`endEffects`); `<condition name>` in any of Java's three
`SkillConditionScope` blocks (`conditions`/`targetConditions`/
`passiveConditions` — the latter two weren't even entered before); and
`<targetType>`/`<affectScope>`/`<affectObject>`/`<operateType>` values that fell
to the `Other` catch-all. `log_gaps` warns per category at boot, worst-first.
`datapack_skill_coverage_census` intersects the record with the datapack's own
reachability and asserts the exact learnable-source name list per category plus
the totals.

**Measured against the 758 learnable skill ids in `data/skillTrees/**`:**

| | |
|---|---|
| unhandled `<effect name>`s | **216** (54 with a learnable source, 1 902 reachable skills) |
| unported `<condition>` `block/name` pairs | **111** (215 learnable skills) |
| unhandled `targetType` / `affectScope` / `affectObject` / `operateType` | 11 / 7 / 5 / 13 |
| **learnable skills that drop an effect or ignore a condition** | **275 of 758 (36 %)** |

Three findings the census produced that the plan had wrong:

- **Anchor (1170) is a *scope* drop, not a name drop** — its `CallSkill` sits in
  `<endEffects>`, an `EffectScope` this port never builds. Porting the
  `CallSkill` handler would not fix it; the `END` lifecycle hook has to exist
  first. That is why `effect-scope` is its own category rather than a false
  entry under `effects`.
- **`affectScope` has zero learnable-source gaps** — confirming G19's deferral
  of the geometric scopes with a number rather than an estimate.
- `EquipWeapon` gates **88** learnable skills in `<conditions>` plus one more in
  `<passiveConditions>`, not the 89 a hand-rolled script had counted.

Verified by sabotage twice, since a census that cannot fail is decoration:
faking a `"Bluff"` handler arm, and faking an enforced `EquipWeapon`, each fail
the intended assertion with a diff naming the entry that moved.

**Two things the harness deliberately does not claim.** Absence from the record
means *recognised*, **not** *correctly ported* — an effect can resolve to a
`SkillEffect` variant nothing downstream consumes, which this cannot see. And
reachability is a text scan of the raw XML (`skillTrees/**`, `stats/npcs`,
`stats/items`, `PetSkillData.xml`), not the ported loaders: going through
`SkillTreeData`/`NpcData` would measure the port against itself and quietly
shrink the very gap the census exists to expose.

## Sleep never broke on damage (2026-08-01)

Reported from play: *"when a monster casts sleep on me he can hit me and I
don't wake up."* Correct — and the cause was not in the sleep effect at all.

`BlockActions` (G19) gave sleep its action lock, but the thing that takes the
lock back off early is a **skill-level tag**, `<removedOnDamage>`, which
`EffectList.stopEffectsOnDamage()` reads from `CreatureStatus.reduceHp` /
`PlayerStatus.reduceHp`. The tag was **unparsed** — `Skill` had no field for it
— and no damage path called anything like it, so a slept target stayed locked
for the buff's full duration no matter how hard it was hit. Sleep was a hard
crowd control instead of a one-hit one.

36 skills on this dist carry the tag: mostly `SLEEP` (player 981/1069/1072/
1097/1394 and the mob casts 4046/4185/4201/4640/4660-4662/5735/6853), plus
`HIDE` (922, 6093, …) and `FORCE_MEDITATION` (441, 1430) — so the same fix makes
a hit break stealth and meditation too, both of which were equally permanent.

Ported as `Skill::removed_on_damage` (parsed in `skill_data`),
`skills::effects::stop_effects_on_damage`, and two call sites in
`game_loop::combat`. **The two call sites differ on `isDOT` deliberately**:
Java's `CreatureStatus` wraps the whole wake block in `if (!isDOT &&
!isHPConsumption)`, while `PlayerStatus` puts `stopEffectsOnDamage()` *above*
its `if (!isDOT)` guard — so a poison tick wakes a sleeping player but not a
sleeping mob. The buff's `(skill_id, skill_level)` is resolved back through the
skill table per hit rather than stamping a bool on `ActiveBuff`, which is what
Java does too (`info.getSkill().isRemovedOnDamage()`) and means DB-restored
buffs behave like freshly-cast ones.

Checked and deliberately **not** ported alongside it: `Formulas.calcStunBreak`
(14 % chance for a hit to break a *stun*) is gated on `Config
.ALT_GAME_STUN_BREAK` ← `BreakStun`, which neither this dist's configs nor
Java's default sets — dead code on this dist, and a stun correctly survives
being hit. `calcRealTargetBreak`'s `REAL_TARGET` abnormal has no Interlude
carrier.

3 tests, all three confirmed to fail with the call sites disabled. The stun in
the same fixture is the control, so the removal is proven to key off the tag
rather than clearing crowd control wholesale. Detail in
[PLAN_G19_ABNORMAL_STATES.md](PLAN_G19_ABNORMAL_STATES.md) §6b.

## UserInfo: attack range, and two more false markers (2026-08-01)

A targeted sweep of the packet writers, prompted by the previous slice's lesson
that asserting a *message id* says nothing about its *contents*. Three `UserInfo`
markers, and only one was a gap:

| Block | Marker | Verdict |
|-------|--------|---------|
| STATS | `TODO(G7)`: "base values… full combat-stat calc" | **stale** — the values have been real for milestones. But its leading short was hard-coded, and that *was* a bug |
| ELEMENTALS | `TODO(G6)`: attribute attack/defense | **false** — Java's own writer emits six literal zeros and never reads the attribute stats |
| SLOTS | `TODO(G6)`: talisman/brooch slots | **moot** — nothing in the datapack grants `talismanSlots`/`broochJewels`; both are post-Interlude, so Java would send 0 here too |

**The real find.** The STATS block leads with Java's `getActiveWeaponItem() !=
null ? 40 : 20` — the character's physical attack range, which the client uses
to decide how close to walk before swinging. The port sent a hard-coded **20**,
the unarmed value, so every armed character told the client to close to
bare-handed reach. Fourth hard-coded value found in a packet writer this sweep.

**A test-shape note.** The first draft located the STATS block by searching for
its length short (56) and matched a coincidental `[56, 0]` in an earlier field,
reading 51. The test now **diffs the armed and unarmed packets** and isolates
the one i16 that flips 20 → 40 — no offset arithmetic to get wrong. Byte-offset
assertions into a masked, variable-length packet are worth avoiding when a
differential assertion says the same thing.

Also checked and left alone: `SiegeAttackerList`'s "signed time" zero (Java
writes `0` there itself, commented "not storated by L2J") and the golden-bytes
`user_info_packet` test, whose fixture is unarmed so its pinned `20` stays
correct.

**Verification:** 2706/2706 (+1 test). One mechanism sabotage-verified.

## Region names in system messages (2026-08-01)

The last subsystem flagged as blocked, and the marker had the **mechanism**
wrong, not just the status. Four sites carried some version of *"MapRegion
carries no sysstring id yet, so the region renders blank"*.

There is no server-side region table involved. Java's `addZoneName(x, y, z)`
appends parameter type **7** carrying the raw coordinates, and the *client*
resolves the region name from them. The port was sending `SysString(0)` —
parameter type **13**, a system-string id of zero — which is a different
parameter entirely and renders as nothing.

So the fix was a new `SmParam::ZoneName { x, y, z }` and passing the right
coordinates at four sites: the monster-kill drop (the kill point), the
drop-from-wielder (where the wielder fell), the owner's login, and `//cw_add`'s
activation. No map-region data needed.

**The test decodes the parameter, not the message id.** Every existing
cursed-weapon test asserted on `sm_ids_of(...)` alone, so all 42 of them passed
happily with the wrong parameter type — confirmed by sabotage. An assertion that
a message was *sent* says nothing about whether its contents are right.

`area_npcs.rs`'s `TODO(G22)` is a genuinely different problem and stays: it wants
`MapRegionManager.getMapRegionLocId` to scope a *broadcast audience*, which does
need a region table.

**Verification:** 2694/2694 (+1 test). One mechanism sabotage-verified. Note:
one full-suite run failed before this without naming a test; two subsequent
full runs and a gameserver-only run all passed, so the failure is unattributed
rather than diagnosed — worth watching for a flake.

## GlobalVariables persistence (2026-08-01)

The second blocked subsystem — except it was not blocked. Both markers said
some version of *"no GlobalVariables table in the port"*, and the
`global_variables` table and its sea-orm entity have been in the schema baseline
all along, with **zero readers and zero writers**. Same dormant-carrier shape as
the CB pet-buff button and `Relax`: the plumbing existed, nothing was attached.

Now `World.global_vars` + `game_loop/global_vars.rs` (typed accessors,
write-through persistence). Writes go through on change rather than on Java's
30-minute `onSave` timer — matching how the rest of this port persists small
global state, and closing the crash window on values whose entire purpose is
surviving a restart.

**Four Sepulchers** — the real gap. Each hall's entry stamp now persists and
rehydrates at boot, so the 60-minute re-entry gate survives a restart instead of
handing everyone a free re-entry on every reboot. Java keys these by **manager
NPC id** (`"FourSepulchers" + npcId`), not by hall index; that mapping is part
of the storage format, so the test asserts it rather than assuming it.

**Core** — `Core_Attacked` persists, and is restored **at spawn**. My first
attempt restored it inside `on_core_attacked`, where the value is overwritten
two lines later; Java restores on the spawn path, which is the only place it can
matter. A restart between the intro and the kill no longer replays the intro.

**The daily-reset catch-up is not ported, because Java's cannot fire.**
`DailyTaskManager`'s constructor computes `calendarTime` = the *next* 06:30 and
runs `onReset()` only when the stored stamp is **not less** than it. The stamp
is always in the past and `calendarTime` always in the future, so the comparison
is always true and the catch-up branch is dead — despite the comment above it
reading "Check if 24 hours have passed since the last daily reset", which would
need a comparison against the *previous* occurrence. A missed reset waits for the
next 06:30 in Java too, which is what this port already did. The stamp **is**
now written, so the value is there if a later chronicle fixes the comparison.

That is the fourth marker this sweep whose promised behaviour turned out not to
exist in Java either ([l2r-port-behaviour-not-intent]).

**Verification:** 2687/2687 (+4 tests). Three mechanisms sabotage-verified — the
manager-id keying, the spawn-time restore, and the clear-on-death. One existing
integration test needed the new boot event added to its allow-list, which is
that test working as intended.

## Carried weight (2026-08-01)

The subsystem three separate markers were blocked on: `//diet`'s overload
immunity, TvT's `isInventoryUnder80` + `getWeightPenalty()` registration gates,
and the inventory-full refusals. All three now read from `game_loop/weight.rs`.

**The penalty is a skill, not arithmetic.** Java's `refreshOverloaded` applies
**4270 "Weight Penalty"** at level 1-4 — a passive carrying the real speed and
HP/MP-regen maluses — which makes this a sibling of `refresh_expertise_penalty`
and it is built the same way: swap a passive buff, resend `EtcStatusUpdate` +
`UserInfo`. Java's bands are permille of the limit: `<500` → 0, `<666` → 1,
`<800` → 2, `<1000` → 3, else 4, with diet mode short-circuiting to 0.

`maxLoad = floor(CON bonus × 69000 × AltWeightLimit)`, and **`AltWeightLimit` is
3 on this dist** — inlining Java's 1.0 would have left every character
permanently overloaded.

**The enforcement is movement, not pickup.** `Creature.isMovementDisabled()` ORs
`_isOverloaded` in beside the crowd-control flags: past your limit you are
rooted where you stand. The 4270 passive only slows you down. That wiring was
found by clippy — `is_overloaded` was flagged as never used, which was the
honest signal that the query had been built and the enforcement forgotten.

**Two hard-coded values fell out of this**, both of the
[l2r-stubbed-counts] shape:
- `ExUserInfoInvenWeight` computed the carried weight correctly but sent a
  literal `80000` as the limit, so the client's weight bar has been drawn
  against the wrong denominator for every character regardless of CON;
- `EtcStatusUpdate`'s weight-penalty byte was a literal `0`, so the overweight
  icon never appeared. The packet carries all three penalties at once, so the
  expertise path had to start passing the weight level through or it would
  clear it on the client.

**A documented deviation.** Java hangs `refreshOverloaded` off
`ItemContainer.refreshWeight`, which every add and remove funnels through. This
port has no such funnel — inventories are mutated through the component
directly — so an event-driven port would mean annotating every mutation site and
would rot the first time someone added another. Instead the **gates**
(`is_overloaded`, `current_penalty`, `is_inventory_under_80`) read live and are
always exact; only the passive's stat malus and the client icon settle on the
regen sweep, with equip / enter-world / offline-restore refreshing immediately.

**Verification:** 2683/2683 (+9 tests). Three mechanisms sabotage-verified —
the `AltWeightLimit` multiplier, diet immunity, and the remove-before-re-add
that stops band changes stacking both levels' maluses.

**A fixture trap worth recording:** the synthetic test world uses
`StatBonus::empty()`, where every stat bonus is 1.0 — so a "more CON carries
more" assertion silently compares a constant to itself. The weight fixture loads
the real table ([l2r-fixture-hides-testcase] again).

## TODO-marker sweep — slice 5: channeled buffs (2026-08-01)

The gap slice 4's re-verification uncovered: Battle Stance **426** (→ Battle
Force 5104) and Spell Stance **427** (→ Spell Force 5105) are learnable at 77 —
427 by thirteen classes — and both channeled their MP upkeep while applying
nothing.

**The mechanic is the registry size.** Java's `SkillChannelizer` applies the
named `channelingSkillId` at a level equal to **how many distinct casters are
channeling it at that target**, capped at the channeled skill's max level:
`min(getChannerlizersSize(id), maxLevel)`. One Warcryer holding Battle Stance
gives an ally Battle Force level 1; two give level 2; four still give 3, because
5104 has three levels. So `World.channelized` (target → channeled-skill-id → set
of channelers) is not bookkeeping around the feature — the set's size *is* the
level. Re-application is skipped while an equal-or-stronger stack is up, so a
steady two-channeler stack refreshes at level 2 rather than flickering.

**What actually blocked it** was one line: the tick returned early when
`channeling_effects.is_empty()`. A `channelingSkillId` skill has no
`<channelingEffects>` **by construction** — that is the whole distinction
between the two branches — so every such skill bailed out before doing anything.
The MP upkeep ran because it sits above that return, which is why the symptom
was "costs mana, does nothing" rather than "does nothing at all".

Unregistration hangs off `stop_casting`, the funnel every cast-stop path already
used. Easy to leave unhooked, and the failure would be quiet and long-lived: a
logged-off channeler propping up someone's stack indefinitely. Pinned.

**Verification:** 2666/2666 (+5 tests). Three mechanisms sabotage-verified — the
level-from-count (a fixed level 1 passes the single-channeler test and fails the
other three), the stop-path unregistration, and the early return that caused the
bug in the first place.

## TODO-marker sweep — slice 4: the "no carrier" claims (2026-08-01)

Slice 4 was meant to be mechanical — convert the markers that say *"no learnable
carrier"* / *"off-chronicle"* from `TODO(G<N>)` into plain notes, shrinking the
set future readers must re-triage. It was not mechanical, because **two of the
seven claims were false**, and one of those hid an inert level-5 skill.

| Claim | Verdict |
|-------|---------|
| `sit_stand.rs`: "no learnable Relax skill exists on this dist" | **FALSE** — skill 226, learnable at **level 5** by Human and Orc Fighters |
| `cast.rs`: "no reachable channeler uses `channelingSkillId`" | **FALSE** — Battle Stance 426 / Spell Stance 427, learnable at 77 (427 by thirteen classes) |
| `walkers.rs`: no route uses `TeleportFirst` | holds — de-`TODO`'d |
| `crafting.rs`: nothing grants `CRAFTING_CRITICAL` | holds — de-`TODO`'d |
| `effects.rs`: no skill sets `irreplacableBuff` | holds (22800+/23200+/27800+ only) — de-`TODO`'d |
| `effects.rs`: no `SummonNpc` Decoy carrier | holds (525 is in no tree) — de-`TODO`'d |
| `effects.rs`: no `MpConsumePerLevel` + `abnormalTime` pairing | holds — de-`TODO`'d |

**Relax (226) was entirely inert.** The `<effect name="Relax">` had no arm in the
parser, so it was silently dropped — the [l2r-instant-damage-effect-gaps]
pattern, on a skill that every Human and Orc Fighter learns at level 5. Only the
`HpRegen` half of the skill worked; the toggle never seated anyone, never charged
MP and never switched itself off.

Now ported as `SkillEffect::Relax`, reusing the existing periodic-tick chain:
- `onStart` seats the caster — Java's `sitDown(false)`, the un-toggleable form;
- each tick pays MP upkeep, with **three** stop conditions, not one: stood up,
  HP back to full (its own message, SM 175), or out of MP (SM 140). Collapsing
  the full-HP branch into the MP check is the obvious wrong port, so it has its
  own test asserting the right message *and* the absence of the other;
- standing up ends it at once (`stopEffects(EffectFlag.RELAXING)`) rather than
  leaving the player paying upkeep until the next tick.

**The channeling gap is now stated honestly** rather than denied. Battle/Spell
Stance currently channel their MP upkeep and apply nothing, because the
`channelingSkillId > 0` branch (apply the named skill as a stacking buff for as
long as the cast is held) is unported. Left as a `TODO(G19)` naming the real
carriers — it is a genuine deferral, unlike the five above.

**Verification:** 2657/2657 (+6 tests). Four mechanisms sabotage-verified —
parser arm, sit-on-start, stand-up stop, and the full-HP branch.

**On the de-`TODO` conversions:** the point is not tidiness. A marker reading
`TODO(G19)` claims work is pending in a milestone; a note reading "no carrier
exists in this datapack" claims a fact that can be re-checked in one grep. Only
the second kind is falsifiable — and two of the seven turned out to be false.

## TODO-marker sweep — slice 3: the siege auto-task chain (2026-08-01)

The largest *real* item left in the marker set, and the marker's own diagnosis
was wrong in an instructive way:

> the auto-start still fires at the fixed `SiegeSchedule.xml` hour, not the
> owner's chosen one — honoring the chosen hour in the timer **needs task
> cancellation the scheduler doesn't have yet**.

It does not. Java never cancels anything here. `ScheduleStartSiegeTask.run()`
**re-reads `getSiegeDate()` on every wake-up** and re-arms itself closer, only
calling `startSiege()` once the date is behind it. A hop armed against the old
date simply sees the new one when it fires. The port had flattened that chain
into a single fire-at-the-computed-tick timer — and *that* is what made the
chosen hour unreachable. The fix was the chain, not a cancellable scheduler.

Worth keeping in mind next time a marker names a missing primitive: the marker
proposes a solution, and the solution can be wrong even when the symptom is real.

**A latent bug the tests surfaced.** `next_siege_millis` is strictly future by
construction, so a re-reading chain computing "time remaining" from it would
never reach zero — the siege could never fire at all. Java avoids this because
`castle.siegeDate` is a **stored** moment the clock passes, not a derived one.
Ported as `set_next_siege_date()`: stamp at boot and roll forward after each
siege, and let `effective_siege_millis` return the stored date **even once it is
in the past**, which is exactly how "the moment has arrived" is detected.

**The other half.** `Siege.saveCastleSiege()` reopens the hour-picking window for
24 h when a siege ends (`regTimeEnd = now + 1 day`, `regTimeOver = false`). Without
it the flag defaults `true` forever, so `RequestSetCastleSiegeTime` was dormant —
the feature had no way to ever become reachable. Java's `setNextSiegeDate()`'s
two-week push is deliberately *not* ported: this dist is schedule-driven, and a
stored "two weeks out" would fight `SiegeSchedule.xml` rather than agree with it.

**A Java quirk ported as-is.** The second ladder rung is `13600000` ms while its
own comment says "1 hr left" — 3 h 46 m 40 s, an apparent stray digit in
`3600000`. That rung is when attacker/defender registration closes and the
waiting list is cleared, so "correcting" it would hand clans 2 h 46 m more
registration time than retail gives. The value is what the server runs on
([l2r-port-behaviour-not-intent]).

New state: `Castle.siege_time_registration_end` (`castle.regTimeEnd`, a column
the entity already had but nothing read), and `UpdateCastleSiegeTime` grew an
optional `regTimeEnd` so callers that do not own that column leave it alone.

`run_auto_task(world, castle_id, now)` is the test seam. Not cosmetic: the chain
converges only because real time passes between hops, so a test firing the
handler repeatedly against a fixed wall clock spins on one rung forever. The
first draft of the ladder test did exactly that.

**Verification:** 2643/2643 (+3 tests, 1 rewritten). Both halves
sabotage-verified. The rewritten test previously asserted the old one-shot
behaviour — it now pins that a distant hop re-arms *without* starting.

## TODO-marker sweep — slice 2: pet-aware sites (2026-08-01)

Same discipline as slice 1 — verify the blocker claim before reading a marker as
a work estimate. Five candidates, and the triage split them three ways, which is
the useful part: only **two were real work**.

| Site | Marker claimed | Verdict |
|------|----------------|---------|
| `admin/editchar.rs` `//fullfood` | "pets are not modelled yet (G29)" | **real** — implemented |
| `network/trade.rs` sell tab | "pet-control exclusion is TODO(G29)" | **real** — implemented |
| `components.rs` `silence` | "honored once whisper delivery exists" | **stale** — `chat.rs` has honored it for milestones; prose corrected |
| `cubic.rs` `MAX_CUBIC` | `TODO(G29)`: read `cubicCount` | **never a gap** — no carrier exists in the datapack; de-`TODO`'d |
| `components.rs` `diet` | "honored once the overload calc exists" | **genuinely blocked** — kept, with the real reason recorded |

`diet` is the one worth keeping honest: this port models **no carried weight at
all** — no inventory weight total, no slot limit, no `getWeightPenalty()`. The
flag is stored and echoed to the GM and can be read by nothing. Same missing
subsystem that keeps TvT's `isInventoryUnder80` deferred, so both markers now
name it explicitly instead of implying two unrelated small tasks.

**`//fullfood`** gates on `isPet()`, which is narrower than "an owned summon": a
skill-summoned servitor has no food bar (its `PetInfo` fed slot carries remaining
lifetime), so targeting one is `INVALID_TARGET`. Java's `broadcastStatusUpdate()`
becomes a `PetInfo` here — the food bar does not ride in a `StatusUpdate`, so a
literal port would have filled a bar the client never redrew.

**The active pet's collar** is withheld from the sell tab (Java `(pet == null) ||
(item.getObjectId() != pet.getControlObjectId())`). Keyed on the **object** id, so
a second collar of the same kind stays sellable. Note this is presentational:
Java's `RequestSellItem` re-checks only `isSellable()`, so a hand-built packet can
still sell it there. The port matches that rather than "fixing" it — diverging on
the handler would change what a client can *do*, not just what it can see.

**Three test bugs worth recording**, all caught rather than shipped:
- the collar test first re-implemented the sell filter *inline*, so deleting the
  production filter changed nothing. Sabotage caught it; it now builds the real
  `ExBuySellList` and counts entries. A test that re-derives the logic it is
  testing proves only that it agrees with itself.
- `give_collar` registers the pet and NPC templates but **not the collar's own
  `ItemTemplate`**, so the first draft's list was empty and every assertion
  passed vacuously — [l2r-fixture-hides-testcase] again. There is now an explicit
  baseline assertion that both collars *are* offered unguarded.
- `use_admin_command` resolves `is_gm` through `AdminData`, which the synthetic
  test world loads **empty**; and `AdminCommands.xml` puts `admin_fullfood` at
  accessLevel **100**, not 70. Two silent-return gates before the handler.

**Verification:** 2638/2638 (+4 tests). Both mechanisms sabotage-verified — the
second time round for the collar.

## TODO-marker sweep — slice 1: "the blocker landed" (2026-08-01)

With every milestone and audit closed, the ~150 per-site `TODO(G<N>)` markers
became the remaining parity surface. Triaging all of them turned up a pattern
worth naming: the dominant class is not *hard work deferred* but **markers whose
stated blocker has since landed**. A marker reading "unported (G25)" about a
system finished weeks ago is worse than no marker at all — it tells the next
reader the gap is still structural, so nobody re-checks it.

That triage found a gap nobody had marked:

**Buff sharing with servitors was missing entirely.** Java's
`Skill.applyEffects` re-applies every continuous, non-debuff buff onto the
caster's servitors (`isSharedWithSummon`, **default `true`**). The port had no
such path anywhere, so every summoner's servitor fought permanently unbuffed.
The `TODO(G30)` on the community board's buff button hinted at it, but framed it
as a board feature rather than the core cast-path mechanic it is.

Two traps in that one flag:
- **The default is `true`.** Only three skills in the whole datapack declare
  `<isSharedWithSummon>`, so parsing it as an ordinary `false`-default flag looks
  perfectly healthy in a unit test while silently disabling sharing game-wide.
  Pinned against the real datapack, not a fixture.
- **A pet is not a servitor.** Java shares through `getServitors()`, and `_pet`
  is a separate field — a wolf receives nothing. Easy to get backwards here
  because this port hangs `ServitorOf` on pets too (they share the owner/follow/
  AI relationship). `servitor_of` — the `SummonRef.servitor` link — is the
  correct query; a component scan would sweep the pet in.

The one Interlude skill flagged `false` is **1557 "Servitor Share"**, which is
its own explanation: it is the skill that copies the owner's stats onto the
summon, so re-sharing it would double-apply. Java's source carries that exact
comment.

Also closed, each a marker whose blocker had landed:

| Site | Marker said | Reality |
|------|-------------|---------|
| `community_board.rs` `is_busy` | "duel, olympiad, SIEGE/PVP zones and event state once those exist" | all four exist — `COMBAT_CHECK` was missing 5 of its 9 clauses, so a player could heal free mid-duel and mid-siege |
| `clans.rs` clan dissolve | "the noble system is unported" | nobless landed at G17; every ex-member lost their title, nobles included. The single-member *leave* path already had it right — only dissolve was wrong |
| `events/tvt.rs` `can_register` | "isInDuel, isInInstance, isInSiege… as its subsystem exposes the query" | all exposed; 6 gates wired |
| `net.rs` `buffs_to_save` | "`isDeleteAbnormalOnLeave` isn't parsed yet" | **not a gap** — all 8 carriers in the datapack are off-chronicle or event-only. Marker corrected to say so rather than implying work |

`isInventoryUnder80` / `getWeightPenalty()` stay deferred and are now the honest
reason: no inventory slot limit or carried-weight calc exists anywhere in this
port, so there is no state to read.

**Verification:** 2615/2615 (+10 tests). Five mechanisms sabotage-verified —
sharing disabled, the pet included in sharing, the parser default flipped, the
combat-check clauses removed, the TvT instance gate removed; each failed the
expected test and only that test.

## Milestone verification pass (2026-07-31)

With the remaining-ports audit closed, the milestone table itself was the
last thing nobody had checked. Every row still carrying 🚧/🔨 — plus the ✅ rows
whose trailing "still open" clauses looked old — was re-read **against the
ROADMAP gate clause and the code**, not against its own row text. That is the
[l2r-verify-milestone-status] discipline: a marker is evidence of when someone
last wrote prose, not of what the code does.

| Row | Was | Verdict |
|-----|-----|---------|
| G7.8 Geodata | ✅ "zones still ⏳" | stale — zones landed at G12 |
| G8 Static world | ✅ "zones/doors still ⏳" | stale — both landed at G12 |
| G13 Admin | 🚧 | **→ ✅** (2026-07-31) — the only named gap, `//manor`, is ported (`admin/castle.rs`, the full `AdminManor` cost table); the pending-leader panel got its data and its Force button in the same sweep. Every remaining absent command is off-chronicle, dev tooling, architecturally N/A, or inert in Java itself. |
| G15 Economy | 🚧 | **→ ✅** — all five gate clauses met and pinned; the row's three "pending" items landed in audit row 9 |
| G15.5 Teleporters | 🚧 | **→ ✅** — both gate clauses met; every "pending" item landed in row 9, except bookmarks, which are **not portable** (null handler in this build) |
| G22 Quests | 🔨 | **→ ✅** — one-time/repeatable/class-transfer all present; the gate's "instance" and "daily" kinds **do not exist in this dist** |
| G23 Bosses | 🚧 | **→ ✅** — schedule-spawn, raid curse and `StoreNpcRespawn` persistence all pinned; 10/10 bosses ported |
| G29 Summons | ✅ w/ stale tail | ✅ confirmed — but the tail was wrong in both directions: the listed gaps had landed, and **pet evolution** (unlisted) is genuinely missing |

**Two findings the pass produced that were in nobody's list:**

1. **Pet evolution is unported and its pages are already reachable.**
   `PetManager`'s `evolve` / `exchange` / `restore` bypass verbs have no Rust
   handler, yet `petmanager/evolve.htm` and `exchange.htm` *are* in the Link
   whitelist — so the buttons render and do nothing. Same shape as the
   unported-`addFirstTalkId` and `Chat <page>` findings: the page being served
   is not evidence the verb behind it exists.
   **→ CLOSED 2026-07-31**: all three verbs ported (`game_loop/pet_evolve.rs`).
   See the G29 row.

2. **G18 reads COMPLETE but its gate names academy + sub-pledges.** The
   ROADMAP's G18 entry lists "sub-pledges (royal guard / order of knights) +
   academy"; the code carries **9 `TODO(G18.6)`** markers for exactly that
   (academy rejection SM 1754, `lvlJoinedAcademy`, apprentice/sponsor cleanup,
   SUBPLEDGE squad skills, per-tab `PledgeShowMemberListAll`). The eight
   landed slices are real and G18's own slice list is met, so the row keeps its
   ✅ — but **the clan academy is the largest single unbuilt subsystem left**,
   and it is not on the milestone table under any number.
   **→ CLOSED 2026-07-31**: G18.6 landed (graduation + reputation reward,
   rank/leadership/clan-war restrictions, apprentice-sponsor mentorship, per-tab
   rosters). Squad skills are a documented verified skip. See the G18 row.

**The parity tail, measured:** 250 `TODO(G…)` markers across the tree — the
porting convention's own paper trail. Largest clusters: G24 sieges/residence
skills (30), G22 per-script side effects (30), G19 effects breadth (27), G29
summons (18), G21 NPC AI (16), then G16/G28/G33/G30 at 13–14 each. These are
per-site behaviours rather than missing features; the G13.9 sweep is the
precedent for closing them in milestone-scoped batches.

**Item transfer restrictions — `is_dropable` / `is_tradable` / `is_destroyable`
/ `is_depositable` 2026-08-01.** Live bug report: a bound reward box (*Mage
Class Equipment Set (10-day)*, 15195 — the XML declares all of `is_tradable`,
`is_dropable`, `is_sellable` false) **dropped on a PK death**. Root cause: the
item parser never read any of those tags — `ItemTemplate` carried only
`is_sellable`/`is_freightable`, and every transfer path used `is_quest_item` as
its stand-in for "bound". So merchant selling was the *only* restriction the
datapack could actually express.

The four tags now parse into `ItemTemplate::trade_flags` (a sub-struct, so the
derived `Default` keeps Java's permissive defaults instead of flipping every
`..Default::default()` fixture to "forbidden"), alongside `time` for
time-limited items. Enforced at: `onDieDropItem` (also skipping time-limited
items, per Java), `RequestDropItem` (refused with
`THAT_ITEM_CANNOT_BE_DISCARDED`), player trade — window listing *and*
`AddTradeItem` — private store sell/manage lists and the sell-into-a-buy-store
path, mail attachments (this closes an explicit `TODO(G30+)`), warehouse
deposit (`isDepositable(isPrivateWareHouse)`: a **private** warehouse still
takes bound items, the clan warehouse and freight do not) and `RequestDestroyItem`.
Merchant sell already honoured `is_sellable`.

Net effect for a bound box: use, warehouse or destroy — nothing else. 3 tests,
the drop paths sabotage-verified.

**`Custom/*.ini` slice 8 — auto use, and the audit closes 2026-08-01.**
`AutoUseTaskManager`'s four loops (supply items, healing potion, buffs, attack
skills) plus the `.playskills` / `.playitems` / `.playpotion` pages. **Buffs run
in town and everything else does not** — that asymmetry is what the peace-zone
gate is for. A configured entry the player no longer has is *dropped from the
list*, not merely skipped, so the panel self-cleans.

**Slice 2 caught a real bug in slice 1.** `AutoPlayConfig` derived `Default`, so
`EnableAutoPotion`/`EnableAutoSkill`/`EnableAutoItem` fell back to `false` while
**Java's defaults are `true`**. The dist ships the ini so it was invisible
there; only a missing file — or a test world — would have shown it, silently
disabling all three sub-panels. `Default` is now Java's, pinned by its own test.
The lesson generalises: a derived `Default` on a config struct is only right
when every Java default is the zero value.

7 tests, 5 mechanisms sabotage-verified. **The G33 `Custom/*.ini` audit is
complete — all 17 features ported across 8 slices.**

**`Custom/*.ini` slice 7 — auto play, part 1 2026-08-01.** The audit's last
feature, and **much smaller than its name suggests**: this build registers **no
Classic auto-hunt packet family** (`ExClientPackets` has no `ExAutoPlay*`
opcode, and nothing in `java/` reads one). The whole thing hangs off a voiced
command and an html panel, so the port adds no opcodes at all. Plan:
[PLAN_G33_AUTO_PLAY.md](PLAN_G33_AUTO_PLAY.md).

This slice: `config/auto_play.rs`, the `AutoPlaySettings` component, the
`.play` panel with its toggles (auto-attack, loot, respect, range, the four
target modes, potion percent), and `AutoPlayTaskManager`'s loop — validate the
held target, else acquire the **nearest** reachable creature within 600/1400
units honouring the mode filter and respectful hunting, attack it, and pick up
loot within 200. Java's idle-count nudge (after ten idle passes, step past the
target so a wedged melee unsticks) is ported; the loop runs every 3 ticks,
Java's 300 ms.

`isMageCaster` is a misnomer worth keeping in mind: it means auto-attack is
**off**, so an unticked box acquires a target and never swings — that is the
intended "let the skills do it" mode, not a bug.

7 tests, 5 mechanisms sabotage-verified. Auto-use (the buff/skill/supply-item
and potion loops, and the three sub-pages that choose them) is slice 2.

**`Custom/*.ini` slice 6 — custom mail manager 2026-08-01.** The `custom_mail`
table as an inbound interface: an operator or web shop writes a row, the server
polls every 30 s, converts it into a real message with attachments, and deletes
it. A `LoadCustomMail`/`CustomMailLoaded` round trip plus a `DeleteCustomMail`
keyed on Java's composite `(date, receiver)`.

**An offline recipient's row is left alone** — not delivered, not deleted — so
a gift waits for them instead of vanishing; the delete only happens on the pass
that delivers. The item list's three shapes (`id count enchant`, `id count`,
bare `id`) are pinned by a test, since a silently-dropped attachment is
invisible to whoever wrote the row.

Documented narrowing: Java tags a row with items as `PRIME_SHOP_GIFT`, a
Kamael-era `MailType` outside this port's enum — and because those ordinals are
the wire values, inventing one would send the client a number it does not know.
A gift therefore arrives as `REGULAR`, differing only in the icon.

4 tests, 4 mechanisms sabotage-verified. **15 of the audit's 17 features are
done**; only auto-play remains (plus champion monsters, which landed first).

**`Custom/*.ini` slice 5 — auto potions 2026-08-01.** `.apon`/`.apoff` plus the
one-second sweep: three pools (HP/CP/MP), each with a threshold and an **ordered**
potion list the loop walks as a preference ranking. Drinking reuses the ordinary
item-skill path, so cast, cooldown and consumption match drinking by hand.

Two Java behaviours kept deliberately, both pinned by tests: the **"out of
potions" line fires every tick** for a player carrying none, even at full health
(Java's `success` flag tracks *carrying* a potion, not drinking one); and the
sweep **drops** rather than skips — dead, offline or in the Olympiad removes the
player from the loop, so reviving does not resume it.

A fixture detail worth remembering: the port only consumes an item when
`default_action`/`immediate_effect` say the *handler* owns the destruction. My
first potion fixture omitted them and the loop appeared to do nothing —
the dist's real potions carry `SKILL_REDUCE` + `immediate_effect`.

6 tests, 5 mechanisms sabotage-verified. **14 of the audit's 17 features are
done**; two remain (custom mail manager, auto-play).

**`Custom/*.ini` slice 4 — sell buffs 2026-08-01.** The player buff shop, ported
whole: the `SellBuffData.xml` whitelist (149 skills, **99 of them learnable**
here, so the feature is genuinely reachable), the nine `sellbuff*` bypasses, the
community-board menus, and the transaction. Details in
[PLAN_G33_CUSTOM_INI_AUDIT.md](PLAN_G33_CUSTOM_INI_AUDIT.md).

The shop rides the `PACKAGE_SELL` store type for its label and seat, so
clicking a seller must check the buff shop **before** the ordinary store or the
buyer opens an empty package-sale window. Buying is asymmetric on purpose: the
buyer pays the price, the **seller** pays the MP, and the seller casts the buff
on the buyer.

It also closes the `_isSellingBuffs` leg of `canOpenPrivateStore` that slice 2
left open — and that leg is why one test failed first time round: a
`python`-driven edit had silently not matched, so the check was never inserted.
The test was asserting exactly that behaviour, which is the only reason it
surfaced instead of shipping as a quiet gap.

5 tests, 5 mechanisms sabotage-verified. **13 of the audit's 17 features are
done**; three remain (custom mail manager, auto-play, auto-potions).

**`Custom/*.ini` slice 3 — the six moderate features 2026-08-01.** PvP reward
item (300 000 adena a kill here), the PvP title/colour ladder, random spawn
jitter, the `.banchat` family, the Noblesse Master NPC, and the character-select
dualbox cap. Details in
[PLAN_G33_CUSTOM_INI_AUDIT.md](PLAN_G33_CUSTOM_INI_AUDIT.md); two things worth
repeating here:

- **A sabotage run caught a real bug in my own code.** The PvP reward first hung
  off `on_kill_update_pvp_reputation` — which returns early inside a PvP zone,
  so `DisableRewardsInPvpZones` could never be reached and the config key was
  meaningless. Removing the guard *didn't* fail the test, which is what exposed
  it: the assertion was passing for the wrong reason. Java puts the reward
  beside the reputation block in `doDie`, not inside it; so does the port now,
  and the test asserts both directions.
- **The Noblesse Master has no spawn on this dist** — template present, no
  spawn file places it, so `//spawn 1003000` is the only way to meet him. Java
  is identical, so it is parity rather than a gap, but "flag on + script exists"
  would otherwise read as a working feature.

7 tests, 5 mechanisms sabotage-verified. **12 of the audit's 17 features are
done**; the four left are the large tier (sell buffs, custom mail manager,
auto-play, auto-potions).

**`Custom/*.ini` slice 2 — the six cheap features 2026-08-01.** Working the
audit's own queue ([PLAN_G33_CUSTOM_INI_AUDIT.md](PLAN_G33_CUSTOM_INI_AUDIT.md))
rather than the TODO clusters. All of tier 1 in one `config/custom_misc.rs`:
`.online`, banking (`.bank`/`.deposit`/`.withdraw`), L2Walker protection, the
boss spawn announcements, the private-store spacing rule and the allowed-races
gate. Two are worth noting beyond the list:

- **The port had no `canOpenPrivateStore` gate at all** — every Java caller
  runs one, and the port opened the manage window unconditionally. Added, with
  the `Custom/PrivateStoreRange.ini` spacing as its first half and the state
  checks (dead / mounted / olympiad / casting) as its second. The player half
  of the spacing only counts **seated** players, because Java's
  `getMinShopDistance` returns 0 while standing — it spaces shops apart, it
  does not block on a passer-by. Getting that backwards would have made a
  crowded town unshoppable.
- **The boss announcement could not be placed where Java puts it.** Java
  announces from `Npc.onSpawn` and excludes minions with `!isMinion()`; the
  port attaches `MinionOf` *after* the entity exists, so the same check inside
  the spawn would be dead code — the shape I keep finding in other people's
  work, caught here in my own before it shipped. Suppression moved to the call
  site (`spawn_minion_npc_at`), matching what the champion lottery beside it
  already does.

8 tests, 5 mechanisms sabotage-verified.

**G19 affect-scope audit + NpcInfo's team/display blocks 2026-08-01.** Next
cluster down (20 markers). The headline question was the **unported affect
scopes**, which silently fall back to *single-target* — a skill that should hit
a group quietly hitting one is exactly the kind of gap that doesn't announce
itself. Each was checked against the datapack the
[[l2r-abnormal-resist-dispel]] way, by carrier rather than by count:

| scope | skills | carrier |
|---|---|---|
| `SUMMON_EXCEPT_MASTER` | 22 | all id 11269+ — the Freya-era summoner revamp, in no skill tree |
| `PARTY_PLEDGE` | 5 | the Pa'agrio clan buffs (1534–1563), in no skill tree |
| `RANGE_SORT_BY_HP` | 4 | Chain Heal + later-chronicle heals, likewise |
| `STATIC_OBJECT_SCOPE` | 2 | Nornil's Power and `Test - …` debug skills |
| `WYVERN_SCOPE`/`BALAKAS_SCOPE` | 5 | boss scripting |

**None is reachable** — not in a class tree, not on an NPC, not on an item. So
the narrowing is honest, and the comments describing it are now accurate: they
had gone stale in both directions, listing `RING_RANGE` and the whole `DEAD_*`
family as unported when both had landed, and blaming `SUMMON_EXCEPT_MASTER` on
missing summons when summons arrived at G29 and the real reason is that the
skills are off-chronicle.

The one real gap in the cluster was in `NpcInfo`: the **`TEAM` and
`DISPLAY_EFFECT`** blocks were never emitted, and the display effect was not
even stored — `//set_displayeffect` broadcast `ExChangeNpcState` and nothing
else, so the change was lost on anyone who walked up afterwards, and
`//setteam` refused NPC targets outright because there was no field to carry.
Java stores both on the NPC precisely so a late observer sees them. Both blocks
are emitted now, in Java's positional order (`SWIM_OR_FLY`, **`TEAM`**,
`ENCHANT`, `FLYING`, `CLONE`, `PET_EVOLUTION_ID`, **`DISPLAY_EFFECT`**), and
`//setteam` takes a `Creature` like Java's single-target form does. Player
teams were already fine — `CharInfo`/`UserInfo` have carried the byte since
G28, so TvT's colours were never affected. 1 test, 3 mechanisms
sabotage-verified. Markers: 154 → 151.

**Siege HQ zones + gate damage 2026-08-01.** The G24 cluster was the largest
left (22 markers), so it got the same read-against-the-code treatment. Most of
it is honest skips — fame has no earning path anywhere in Interlude, castle
upgrades aren't modelled, `AttackerRespawn = 0` on this dist — but three
markers were **stale prose** (the HQ-flag and artifact-capture mechanics they
said were "unported" had since landed) and two were real:

- **The headquarters zones were never loaded.** `BuildCampSkillCondition`'s
  last gate — the one with its own message — is `isInsideZone(ZoneId.HQ)`:
  an attacker may plant a base camp only on the battlefield's marked patches.
  `castle_hq.xml` ships **19** of them, but `HqZone` was not a parsed kind, so
  the file was skipped and the gate had nothing to consult: a camp could go up
  anywhere in the siege zone, courtyard included. New `ZoneKind::Hq` +
  `hq_castle_at`, and the cast now refuses elsewhere with
  `YOU_CAN_T_BUILD_HEADQUARTERS_HERE`. A pre-existing HQ-flag test started
  failing on the new gate — correctly, its fixture had no patch — which is the
  [[l2r-census-tests]] discipline paying off twice in one run (the zone count
  moved 1234 → 1253 in the same pass).
- **A besieged gate looked untouched until it burst.** `DoorStatusUpdate` wrote
  `damage = 0` and `currentHp = maxHp` unconditionally, and the port only
  broadcast on the *breach*. Java re-broadcasts on every hit through
  `Door.reduceCurrentHp`, carrying the real HP and `getDamage()` — the 0..6
  **crack grade** the client draws on the gate mesh. Both are real now. The
  grade is a sixth rather than a percentage (`6 - ceil(cur/max * 6)`), so the
  first crack only appears below 5/6 HP and a gate on one HP still shows 5 —
  the test pins that shape, because the intuitive reading is wrong.

3 tests, 3 mechanisms sabotage-verified. Markers: 158 → 153 (G24: 22 → 17).

**Mount feeding 2026-08-01.** The item deferred from the last tranche, and the
last of the mount markers. Java `Player.startFeed` + `PetFeedTask`: riding
burns the mount's feed every 10 s (`consume_meal_in_battle` while swinging,
else `consume_meal_in_normal`), and the tick that cannot cover the cost
**force-dismounts** with "You are out of feed. Mount status canceled." The bar
refills by using the mount's food while riding — that is what the `Feed`
effect's `ride`/`wyvern` params are for, and they were parsed away: the port
carried only `normal` (the pet's share), so the rider half of the same food
item did nothing.

The starting value is a Java subtlety worth keeping: `mount(Summon pet)` calls
`startFeed` **before** unsummoning the pet, so mounting your own half-starved
strider hands you a half-empty bar, while every pet-less path (admin `//ride_*`,
the wyvern manager, the enter-world restore) starts full.

**`isHungry()` is inert in this Java build, and that is now recorded rather than
guessed at.** The predicate reads `hasPet() && …`, but `mount()` unsummons the
pet one line after starting the feed — so a *rider* never has one, and both
consumers are dead code: the `SpeedFinalizer`'s -50 % hungry-mount halving and
the "a hungry strider cannot be mounted or dismounted" refusal. The refusal is
ported anyway, for shape, at the `/dismount` action; the speed site cannot reach
the pet registry from `recalculate_stats` and says so instead of leaving a bare
gap. Reading `isHungry` as live would have been a divergence *in the port's
favour* — exactly the kind the [[l2r-port-behaviour-not-intent]] lesson is about.

**One Java bug deliberately not reproduced.** All four feed `SetupGauge` call
sites read `new SetupGauge(3, cur, max)` — the *three*-argument constructor,
whose first parameter is the **object id**. Mobius added `objectId` to that
signature and never updated these lines, so retail-Mobius sends
`objectId = 3, colour = cur`: a packet the client cannot draw. Every other
`SetupGauge` call site in the tree passes `getObjectId()` first. The port sends
the four-argument form with the rider's id and colour 3 (green), with the bug
documented at the site. 2 tests, 4 mechanisms sabotage-verified. Markers:
163 → 158 (the feed markers plus three stale mount ones:
`AllowRideMountsDuringSiege`'s "when pet mounting lands", the `/dismount`
action's "mounting is TODO", and the speed finalizer's hunger note).

**Olympiad leaderboard, siege mounts, TvT dualbox 2026-07-31.** The next three
markers from the staleness sweep, each waiting on a subsystem that had since
landed:

- **The Olympiad Manager's class rank pages were blank by construction.** Java
  freezes the cycle's nobles into `olympiad_nobles_eom` at the round end
  (`updateMonthlyData`, run right after `saveOlympiadStatus`) and
  `getClassLeaderBoard` reads *that* — `AltOlyShowMonthlyWinners = True` here,
  so the board is the **last completed** cycle, not the live one. The table was
  in the schema but nothing ever wrote it. Now `handle_olympiad_end` snapshots
  it in memory and over a new `DbCommand::SnapshotOlympiadEom` (Java's
  TRUNCATE + `INSERT … SELECT`, ordered behind `SaveOlympiad` on the same
  channel so it copies rows already written), boot restores it beside the live
  nobles, and `olympiad::class_leader_board` ranks by points → matches → wins
  with Java's `LIMIT 10` and `AltOlyMinMatchesForPoints` floor. The page has
  fifteen rows, so five are always blank — as they are in Java.
- **`AllowRideMountsDuringSiege` had no consumer.** It has three in Java, two
  reachable here: `Player.mount` refuses outright inside a live siege zone, and
  `SiegeZone.onEnter` **dismounts** a rider who walks in — plus untransforms one
  wearing a `RIDING_MODE` transformation, so `TransformData` gained a `riding`
  flag off the `type` attribute it was already reading for `FLYING`. Both legs
  are silent in Java. (The wyvern leg beside them is gated on
  `AllowRideWyvernDuringSiege`, True here, so it never fires.)
- **TvT had no dualbox cap.** `AntiFeedManager.tryAddPlayer` with
  `DualboxCheckMaxL2EventParticipantsPerIP` — **1** on this dist, so a second
  character from one address is turned away with its own `registration-ip.html`.
  New `config/dualbox.rs` reads `Custom/DualboxCheck.ini` including the
  `address,extra;…` whitelist that raises the cap per address. **The port counts
  the live roster rather than keeping Java's own per-event IP counter**, which
  cannot drift: Java's counter leaks a slot when a registrant disconnects
  without cancelling. `0` means unlimited, and Java skips the check rather than
  reading it as a cap of zero — a test pins that, since getting it backwards
  would lock everyone out.

Deferred with its marker intact: the **mounted feed gauge** (Java
`Player.mount` → `startFeed`, the hunger that halves speed and force-dismounts
at 0). The `Feed` effect's `ride`/`wyvern` params feed exactly that gauge, so
the two are one feature and neither is a stale marker — the pet half is ported,
the rider half is not. 5 tests, 6 mechanisms sabotage-verified. Markers: 165 →
163.

**The stale-marker sweep 2026-07-31.** The `TODO(G…)` markers are the port's
own work estimate, so a batch of them was re-read **against the code they name**
rather than against their own prose — the [[l2r-verify-milestone-status]]
discipline applied one level down. Several were written before the subsystem
they wait on landed, and three of those were hiding real behaviour gaps:

- **Hero status was granted by the crown, not the claim.** Java's `Hero.isHero`
  — what `EnterWorld` reads to call `setHero` — is *crowned **and** claimed*,
  and `computeNewHeroes` deliberately never calls `setHero(true)`: a hero
  collects the title at the **Monument of Heroes**. The port crowned and
  granted in one step, so `heroConfirm` had nothing to do and the monument's
  certification pages were unreachable. Now `heroes.claimed` round-trips
  (`HeroRow.claimed`, a new `DbCommand::ClaimHero`), `OlympiadState` splits
  `is_hero` / `is_unclaimed_hero` / `is_crowned`, and `olympiad::claim_hero`
  (the monument's `heroConfirm` and the GM's `//givehero`) pays the clan its
  `HeroPoints` (new `Feature.ini` key, 1000), grants the status and skills,
  broadcasts the hero animation and writes the diary deed. **A re-crown clears
  the claim**, so each cycle is collected afresh. One Java subtlety kept: the
  *trade-point* hero bonus reads `isHero || isUnclaimedHero`, so it rides the
  crown — that line is now `is_crowned`, and the three round-end tests that
  caught the difference say so.
- **A delegated clan-leader transfer never fired.** The village master stamps
  `new_leader_id`, and Java applies it in `DailyTaskManager.clanLeaderApply` —
  in the **Wednesday** branch, beside the weekly vitality refill. The port had
  the stamp and (since G33 slice 1) the scheduler, but nothing joined them, so
  a transfer waited forever. Ported, including Java's `continue` for a nominee
  who has left the clan: the stamp stays, the transfer just never fires.
  `//clan_show_pending` now lists those clans instead of rendering empty, and
  `//clan_force_pending` (absent) runs one immediately.
- **Transform-while-sitting was unguarded.** `ConditionPlayerCanTransform`'s
  `isSitting()` leg was written as vacuous ("`ChangeWaitType` is unported, so
  nothing can be sitting") — true when it was written, false since `SitStand`
  landed. Ported with SM 2283, and the cursed-weapon leg moved to Java's own
  position (first, silent, sharing the `isAlikeDead` branch) while the order
  was being corrected. **Superseded 2026-08-01:** `Player.useMagic` refuses a
  seated caster with SM 31 long before `checkCondition` runs, so this leg is
  unreachable down the cast path — in Java as much as here. It is kept (Java
  keeps it) and answers only for transforms that skip `useMagic`.

Smaller: the community board's `_bbsheal` now tops up the **pet/servitor** too
(summonable since G29), and `Fishing.castLine` sends **`YOU_CAN_T_FISH_HERE`**
on a bad spot (Java splits on `_isFishing`: a fresh cast is told why, a re-cast
gets `ActionFailed` alone). Two markers were **documentation debt, not gaps**:
`//debug`'s doors/geodata/movement visualizers claimed `ExServerPrimitive`
"isn't ported" when `admin::debug_draw` had since landed, and `register_gm`'s
hidden flag is **inert in Java too** — every `getAllGms` call site passes
`includeHidden = true` and there is no `//gmlist` command at all, so the
port's no-op is exact (a [[l2r-pc-cafe-points]]-shaped find). 6 tests,
6 mechanisms sabotage-verified. Markers: 173 → 165.

**Book-gated skill learning 2026-07-31.** Found by auditing the skill trees'
XML against what the parser keeps: `data/skill_tree.rs` reduced each `<skill>`'s
`<item id count/>` children to a **boolean** `requires_item`, so the manual
learn path had nothing to charge and `AcquireSkillList` wrote a hard-zero
required-item count. Net effect: **Divine Inspiration (1405) was learnable for
SP alone**, its Ancient Book (8618–8621) neither shown by the client nor taken.
Now parsed into `SkillLearn.required_items` (Java's `ItemHolder` list — the
parser had to grow an `</skill>` end-event so children can accumulate),
written into `AcquireSkillList`, and verified + consumed by
`RequestAcquireSkill`: the whole list is checked before anything is destroyed
(`YOU_DO_NOT_HAVE_ENOUGH_ITEMS_TO_LEARN_THIS_SKILL`, sm 276), then each book is
taken with its disappear message. New config `DivineInspirationSpBookNeeded`
(Java default `True`, **`False` on this dist**), and with it a **Java quirk kept
verbatim**: `checkPlayerSkill`'s early `return true` for 1405 sits *above* the
SP deduction, so waiving the book waives the SP cost too — on this dist Divine
Inspiration is now free of both (the "enough SP?" gate above it still applies).
**Measured against the datapack:** every `<item>` in the class trees belongs to
1405, so this one skill is the whole of the required-item leg — except Sorcerer
(12), which hands out level 2 as a book-less `autoGet` at 52; the test pins that
quirk instead of asserting the tidy iff. 4 tests, 6 mechanisms
sabotage-verified. Closes three `TODO(G6)` markers (173 left, from 176).

**`NpcInfo`'s abnormal visuals 2026-07-31.** The same stub, one packet over: the
`ABNORMALS` component was never emitted, so **a stunned, poisoned or feared mob
looked completely untouched to every client** — the exact shape `CharInfo` had
before G19 fixed it for players. The state was already there
(`abnormal::visual_effects` reads an NPC's buffs the same way it reads a
player's); only the packet never carried it.
**Two flaky tests fixed with it, both my own regressions from earlier today.**
`physical_skill_damages_monster_and_soulshot_doubles` forced two rolls per cast,
but the G20 shield slice inserted two more ahead of the crit roll — the crit
then fell through to real RNG and the two casts could disagree, failing about
two full-suite runs in three while passing in isolation. And
`target_cancel_clears_the_target_and_aborts` stopped being deterministic when
`TargetCancel` moved onto `calcProbability`, whose threshold is below 100 even
for a 100-chance skill. *Adding a roll upstream silently shifts every forced
sequence downstream* — the suite was run four times clean to confirm.
1 test, 3 mechanisms sabotage-verified.

**`UserInfo`'s last three stubbed fields 2026-07-31.** Finishing the same
sweep, this time by walking `UserInfo` against Java's block by block. Three
fields wrote a hard 0 while the state behind them existed: the **large clan
crest** (`getClanCrestLargeId`), the **raid-boss points**, and the
**cursed-weapon stage** — Java's `isCursedWeaponEquipped() ? getLevel(id) : 0`,
which is what colours a wielder's name. The crest is mirrored onto the player
beside the small one (the builder cannot reach `World.clans`), and the stage
needs the world rather than the entity store, so `PlayerView::of_world` now
resolves it for every `UserInfo` caller.
**Checked and left alone:** `ATK_ELEMENTAL` and the vitality bonus byte are
Java's own hard zeros, not port stubs. 2 tests, 5 mechanisms sabotage-verified.

**`UserInfo`'s craft byte 2026-07-31.** Same audit, same shape as the `Die`
find: the STATUS block's third byte is Java's
`hasDwarvenCraft() || getSkillLevel(248) > 0` and **it is what opens the
client's create-item window**. The port wrote a hard 0, so the entire crafting
subsystem (G15.7, fully ported and tested) had no way in from the UI. Now read
from the skill book — Create Item (172), or Crystallize (248) for a non-Dwarf.
`PlayerView` gained the skill book to make it reachable.
**Noted, not ported:** `EtcStatusUpdate`'s `getWeightPenalty()` is also a hard
0, but inventory weight/overload is not modelled at all (only
`GMStartupDietMode` exists), so that is a feature rather than a stub. 1 test,
3 mechanisms sabotage-verified.

**The `Die` packet's restart buttons 2026-07-31.** Found by a third audit
pattern — hard-coded `0` counts and flags in packet writers ([[stubbed
counts]]'s lesson) — and the largest single find of the day. **Every flag in
`Die` was a literal 0**, and the client only sends a `RequestRestartPoint` for
a button it was told exists. So `clanhall_restart_location` and
`siege_restart_location` — both fully implemented, both tested — were
**unreachable in play**: a siege defender could never choose "to castle", an
attacker never "to siege HQ", and no one could restart at their clan hall.
All of Java's `Die` constructor is now ported: `to_village` (hidden while a
resurrection is already proposed), `to_clan_hall`, `to_castle` (owning a castle
*or* defending one), `to_outpost` (an attacker **with a base camp still
standing** — Java reads `!getFlag().isEmpty()`, so a razed camp removes the
button rather than offering a respawn that fails), and `sweepable` on a spoiled
corpse. Fortress and feather flags stay false — off-chronicle and no Interlude
item family respectively.
Also fixed: a test from this morning's trait slice compared two auto-attacks
whose miss/crit rolls were left to the RNG. It passed in isolation and failed
about one full-suite run in ten; every roll in the swing is forced now.
4 tests, 8 mechanisms sabotage-verified.

**Expired-assumption sweep 2026-07-31.** Acting on the pattern the previous
slice exposed: a grep for helpers whose simplification is justified by a stat's
*absence* ("trait mods 1.0", "no traits/attributes", "unported"). Today's own
work had invalidated six such claims and left **one real behaviour gap**:
`Lethal`'s `chanceMultiplier` is
`calcAttributeBonus · calcGeneralTraitBonus(…, false)` and the port applied only
the attribute half, under a comment reading "its trait half stays unported with
the trait system". The trait system landed hours earlier. Backstab/Lethal
Blow/Deadly Blow now respect a victim's trait resistance when rolling their
kill chance. The other five were doc-only (`formulas`' module header,
`calc_magic_dam`, `calc_physical_skill_damage`, `calc_blow_damage`,
`TraitType::Weapon`, and a resist test claiming "Detect Beast Weakness is inert
on this dist" — it is not, since the damage side landed). 1 test,
1 mechanism sabotage-verified.

**G19 `TargetCancel`'s chance gate 2026-07-31.** Not a missing effect —
`TargetCancel` (10 learnable carriers: Shield Bash/Slam, Stun Blast/Shot/Stomp,
Earthquake, Aura Flash, Trick, Switch, Bluff) was ported — but its **gate was
wrong in two ways**. Java rolls it through `Formulas.calcProbability`, so the
victim's *level* counts; the port compared a flat percentage, and a level-100
target was as easy to shake off its mark as a level-1 one. And
`TargetCancel.calcSuccess` vetoes outright on `ABNORMAL_INVINCIBILITY` /
`INVINCIBILITY_SPECIAL` / `INVINCIBILITY`, which was missing entirely.
**A self-correction rides along:** `calc_probability` dropped Java's attribute
and trait multipliers with the justification "both are 1.0 for every actor this
port models". That was true when it was written and stopped being true when the
attribute (G19) and trait (G20, today) tables landed — a claim invalidated by
later work, with nothing to make it fail. Both are now real inputs, so a victim
resisting the skill's element or trait is correspondingly harder to disarm.
5 tests, 7 mechanisms sabotage-verified.

**G24 mid-victory's tail 2026-07-31.** `Siege.midVictory` does far more than
swap the deed, and the port stopped at the deed. Now also ported: the **new**
attackers (the clans that were defending a second ago) are evicted from the
castle; `removeDefenderFlags()` runs *after* the role reshuffle, so the base
camp it tears down is the **captor's own** — you do not keep a siege HQ once the
castle is yours; and the control/flame towers are removed and rebuilt with
`_controlTowerCount = 0` in between ("each new siege midvictory CT are
completely respawned"). That count reset is load-bearing: without it the
respawn adds to a stale count and the guardian-tower resurrection message can
never fire again. The 50 %-HP door respawn and the state-flag re-push were
already there, so those two clauses of the old TODO were stale.
**Verified not portable:** `Castle.removeUpgrade()` — castle upgrades (the
door/trap tiers bought from the chamberlain) are not modelled at all, so there
is nothing to strip; noted at the site. 1 test, 6 mechanisms sabotage-verified.

**G24 siege resurrection 2026-07-31 (27 → 24).** A stale marker again, and it
hid a real siege bug: `Siege.control_tower_count`'s doc said "no effect until
the resurrection subsystem lands" — that subsystem landed in G19, and nothing
had gone back. So `ConditionPlayerCanResurrect`'s **whole siege block was
missing** and a Bishop could freely raise defenders mid-siege.
Ported in full. **Every branch of Java's condition refuses** once a siege is in
progress; the control-tower count and the attacker's flag count only pick which
of three messages the caster reads (guardian-tower destroyed / no base camp /
the generic battleground line). **Two things get through**: the Blessed Scroll
of Resurrection (Battleground) skill **2393**, and — because the condition opens
with `if (skill.getAffectRange() > 0) return true;`, carrying Java's own "Need
skill rework for fix that properly" — *any* AoE resurrection, which on this dist
means Mass Resurrection 1254. That shortcut is load-bearing, not decoration.
Also corrected: the castle mass gatekeeper's comment had its timings inverted
(it is **30 s normally, 8 minutes once the towers are down** — the real cost of
losing them; the code was right, the comment was not). 4 tests, 8 mechanisms
sabotage-verified.

**G20 trait damage 2026-07-31 (9 → 7).** The *damage* consumers of the trait
tables the G16 slice built, which until now only fed the **landing roll**:
`calcWeaponTraitBonus`, `calcWeaknessBonus` and `calcAttackTraitBonus`, plus the
attacker-side `AttackTrait` accumulator all three read. **Deflect Arrow now
deflects arrows** (BOW 16-40 %), Provoke's POLE **−10** really does make pole
hits land harder, and the Hunter/Slayer "Detect … Weakness" line (7 learnable)
finally pays off against the race skills that make it reachable — `Undead`
(4416) sits on **13 547** NPC templates carrying negative `*_WEAKNESS` defence
traits.
**The attack table's identity is 1.0, not 0** — the opposite of the defence
table — because the pair is consumed as `attackTrait − defenceTrait`; and
`hasAttackTrait` (membership) is a *different* question from the value, which
the group-2 branch gates on separately. `calcGeneralTraitBonus` gained Java's
`ignoreResistance` flag: the damage formulas pass **true** (a stun resistance
does not soften a stun's *damage*), the landing roll passes false. Wired into
the auto-attack, both magic-damage paths, `PhysicalAttack`, `EnergyAttack` and
`calcBlowDamage` — the last four through one `skill_trait_mod` helper that keeps
Java's `generalTraitMod == 0 ? 1` guard, which is what stops an invulnerable
trait from zeroing *damage* as well as the roll. 12 tests, 14 mechanisms
sabotage-verified.

**G20 vampiric absorb + damage reflect 2026-07-31 (12 → 9).** The two on-hit
reactions in Java's `Creature.doAttack`, both of which had been landing as
icon-only markers: **Vampiric Rage healed nothing and Reflect Damage bounced
nothing** (7 and 6 learnable carriers). Java `pump`s both as ordinary additive
stats, so they now ride `stat_modifier_effects` — `VampiricAttack` grants a
*pair* (`ABSORB_DAMAGE_PERCENT = amount/100` and the `amount · chance` term
`VampiricChanceFinalizer` divides back out), `DamageShield` a single
`REFLECT_DAMAGE_PERCENT`. A new `apply_attack_damage` layer sits above
`apply_physical_damage`, which stays the raw `reduceCurrentHp` analog so a
`DamageZone` tick neither feeds a vampire nor gets reflected.
**Java's gates, all live:** a **bow drains nothing** ("do not absorb if weapon
is ranged"); `VampiricAttackWorkWithSkills` is **False** here, so Vampiric Rage
feeds off auto-attacks only; the absorb is capped by the victim's remaining HP;
and the reflect is skipped on a DoT, on reflected damage itself, and — the one
that is easy to miss — **on a killing blow** ("when killing blow is made, the
target doesn't reflect"). The bounce is capped by the reflector's own defence,
`pDef` or `mDef · 1.5` for a magic skill, and Java's `int` truncation there is
load-bearing. 13 tests, 16 mechanisms sabotage-verified.

**G20 shield block + ranged skill damage 2026-07-31 (14 → 12).** Two gaps that
touched every shield user and every archer. **`calcShldUse` was ported for
auto-attacks and mana drains but consulted by none of the three *skill* damage
paths** — `PhysicalAttack`, `EnergyAttack` and `calcBlowDamage` all open on the
same shield switch, so a shield did nothing at all against a skill. All three
now go through one `defence_after_shield` helper: a normal block adds the
shield's `sDef` to the divisor, a perfect block cuts the hit to a flat **1**
(Java's `defence = -1` / `return 1`). `<ignoreShieldDefence>` is parsed (55
skills, **14 learnable** — Triple Slash, Armor Crush, Hammer Crush, …) and skips
the switch **and its two rolls**. Note the ordering: Java folds `pDefMod` in
*before* the add, so the shield's own sDef is never scaled by it.
**The ranged branch of the physical-skill formula** is the other half: a
bow/crossbow uses `weaponMod` **70** *and* adds a second `pAtk + power` term
inside the bracket, so an archer's skill hits **harder**, not `70/77` as hard —
and that bonus reads the raw `pAtk`, with the level modifier applying only to
the first term. 6 tests, 9 mechanisms sabotage-verified.

**G16 CLOSED 2026-07-31 (6 → 0).** The residual four clusters, three of which
turned out to be larger than their markers claimed.
**Premium drops were entirely inert:** `PremiumRateDropChance/Amount` were
parsed and read by *nobody*, so a premium killer's loot was identical to
everyone else's — the marker only promised the missing *per-item* maps. Both
halves are now applied in `roll_drops`, plus the spoil pair in
`roll_spoil_drops`. Java's chain is `byId → herb → raid → flat` with the **herb
and raid arms empty**, so premium buys nothing on a herb or a raid drop; and the
per-item map **replaces** the flat rate rather than stacking, which is what
pins this dist's jewels (6656-6662, 8191, 10170, 10314) to ×1 against a flat ×2.
**`PremiumRateQuestXp/Sp`** now apply, before the server's `RateQuestReward*`.
**The `hasEffectType(HATE)` marker was stale** — it claimed no HATE effect was
modelled, which stopped being true when `DeleteHate`/`DeleteHateOfMe` landed in
G19. So Bluff, Forget, Trick, Repose, Peace and Eva's Serenade were waking the
very mob they had just made forget the caster. Java gates **only** the
`EVT_ATTACKED` notify on it; the `-effectPoint` hate beside it is ungated, and
Bluff really does carry `effectPoint -1`.
**The magic-crit `DamOverTime` burst moved behind the land roll.** Java puts it
in `onStart`, which `EffectList.add` only reaches once `calcEffectSuccess`
passed — so a resisted poison deals nothing at all. The port had it in the
instant pass, bursting for `power × 10` on a debuff that was about to be
resisted. (Java's inline `// TODO: M.Crit can occur even if this skill is
resisted` at that spot is aspirational, not shipped.) 12 tests, 17 mechanisms
sabotage-verified. **No `TODO(G16)` markers remain.**

**G16 PC-café (PA) points done 2026-07-31 (8 → 6).** The *store* was already
there — `characters.pccafe_points`, the `//pccafepoints` GM command,
`ExPCCafePointInfo` — but no way to **earn**. `PcCafePointsManager` is now
ported as `game_loop::pc_cafe`, with all five Java call sites wired: `run` at
enter-world, on a community-board premium purchase and on `//premium_add`;
`givePcCafePoint` on a solo kill, on each party member's share, and on a quest
XP reward. The two modes are **mutually exclusive** — `givePcCafePoint`'s first
guard is `PC_CAFE_RETAIL_LIKE`, which this dist sets, so it is the 5-minute
timer or nothing.
**Two upstream bugs are reproduced rather than fixed**, because they are what a
player on the reference server actually sees: `givePcCafePoint` sends the
*double-points* string on **both** branches of its if/else (its sibling
`giveRetailPcCafePont` gets it right), and `giveRetailPcCafePont`'s max check
compares the **award** to the ceiling instead of the player's balance, so a
capped player is told they earned points while the clamp hands them zero. Both
are pinned by tests that say so.
**One divergence is deliberate and the dist data decides it:**
`Config.PC_CAFE_REWARD_TIME` is declared in Java and **never assigned**, so it
is 0 and `scheduleAtFixedRate(…, 0, 0)` throws — the reference server's
retail-like timer never starts at all. `PremiumSystem.ini` declares
`PcCafeRewardTime = 300000`, and the dist is the specification, so the port
reads it. (Relatedly: `Custom/PcCafe.ini` exists on this dist and **no Java
constant names it** — its `PcCafeEnabled = True` is inert, and the
`//pccafepoints` ceiling that used to be inlined from it now comes from
`PremiumSystem.ini`.) 17 tests, 20 mechanisms sabotage-verified.

**G16 MP-cost / reuse rates done 2026-07-31 (11 → 8).** The next-largest G16
cluster after trait resistance: `MagicMpCost` (277 skills, **18 learnable**) and
`Reuse` (126 / **8**), both of which had been landing as icon-only markers —
Arcane Wisdom, Zealot, Clarity, Song of Meditation and Quick Recovery all cost
and saved exactly nothing. Java keeps two `magicType → factor` maps on
`CreatureStat` (`_mpConsumeStat` / `_reuseStat`); the handlers merge
`amount/100 + 1` with **`mul`** on start and **`div`** on exit, and
`getMpConsume(skill)` / `getReuseTime(skill)` read the bucket matching the
*cast* skill's own `magicType`. All of that is now ported as a `SkillRateStats`
component plus the two accessors, wired into `use_magic`'s precheck,
`handle_skill_finish`'s consume and `set_skill_reuse`.
**Three details worth keeping:** the bucket is the **effect's** `<magicType>`,
not the carrying skill's (Zealot is a physical-bucket discount on a magic-ish
buff); the merge is multiplicative, so two −10 % songs are 0.81 rather than
0.80; and `getReuseTime` returns **before** the multiply for `staticReuse` or
`isMagic == 2` skills — `<staticReuse>` was unparsed here despite **1 297**
skills declaring it, which would have let Super Haste's −99 % loose on fixed
cooldowns. Java's `> 10 ms` cooldown gate is applied to the *scaled* delay, so
a large discount can take a skill out of the reuse map entirely. Also ported:
`DanceConsumeAdditionalMP` (the dance-stacking surcharge in the same Java
method) — **False on this dist**, so wired to the flag rather than assumed away.
The stale `AttackTrait` note claiming no monster carries a `*_WEAKNESS`
`DefenceTrait` is corrected: the race skill `Undead` (4416) does, on 13 549
NPCs. 10 tests, 14 mechanisms sabotage-verified.

**G16 sweep done 2026-07-31 (14 → 11).** The cluster's real content was
**trait resistance** — the pairing of a debuff's `<trait>` tag with the
`DefenceTrait` effect. It is the largest unported effect cluster on this dist
(**899 skills, 34 of them learnable**, against `AttackTrait`'s 7 and `Reuse`'s
8), and until now the whole Stun/Mental/Poison-resistance line landed as an
icon and changed nothing. Ported: a `TraitType` enum with Java's numeric
groups, `<trait>` parsing on the skill, the `DefenceTrait` params (percent over
100, per level), a `DefenceTraits` component merged at `onStart` and unmerged at
`onExit`, and `calcGeneralTraitBonus` folded into the landing roll as a fourth
multiplier. **Two Java details are load-bearing and were both got wrong first
time round:** invulnerability is tested **before** the group switch (so a
weapon- or weakness-trait immunity zeroes the chance too), and it then
**skips the clamp** — `finalRate = traitMod > 0 ? constrain(rate, 10, 90) : 0`
— so an immune target refuses the debuff outright instead of taking it one roll
in ten. Note also that a *negative* defence trait is a **vulnerability**, which
is how the race skill `Undead` (4416, on 13 549 NPCs) works.
**Verified not a gap:** the group-2 `*_WEAKNESS` branch of the landing roll —
five skills declare such a `<trait>`, but Java's own guard needs the attacker to
carry a matching `AttackTrait`, and nothing grants one, so the branch returns
1.0 on the reference server too. Left as a `TODO(G20)`: the *damage*-side
consumers of the same tables (`calcWeaknessBonus`,
`calcAttackTraitBonus`/`calcWeaponTraitBonus`), which is where the Hunter's
"Detect … Weakness" line and the weapon-type resistances actually pay off.
6 tests, 11 mechanisms sabotage-verified.

**G21 sweep done 2026-07-31 (16 → 12).** Three real fixes and one
verified non-gap. **Herbs now run their own auto-destroy clock**: the TODO
claimed the item template carried no herb flag, but `ex_immediate_effect` has
been parsed since G15 — so the marker was stale and herbs were lying on the
ground for the ordinary 600 s instead of `AutoDestroyHerbTime`'s 60. Java's
gate is an **either/or** (`(AUTODESTROY > 0 && !herb) || (HERB_TIME > 0 &&
herb)`), so a herb is swept even with the ordinary destroyer switched off —
the port's single early-return got that wrong too, and both halves are now
pinned. **The `//grandboss` panel counts the nest's occupants** (Antharas
70050 / Baium 70051) instead of always printing "Zone not found!"; that
string is Java's fallback for the four panel bosses with no zone, and an
existing test asserted the stub. **`//cw_add` arms the removal task**
(`reActivate`'s other half) — without it a GM-granted cursed weapon never
expired and the duration argument was decorative.
**Verified not a gap:** the `//grandboss_skip|respawn|minions|abort` buttons.
`AdminGrandBoss.antharasAi()` and `baiumAi()` are literally `return null;` in
this build (the `QuestManager` lookup beneath them is commented out), so every
one of those buttons NPEs in Java whatever the boss AI does — the port
reproduces the NPE rather than wiring its (ported) Antharas/Baium AI behind a
button that is dead upstream. 3 tests, 3 mechanisms sabotage-verified.

**G29 sweep done 2026-07-31 (18 → 16, and it reached across five milestones).**
The cluster's real content was **sitting**, which the port had never modelled —
a gap that had left TODOs in G14 (`/mount`), G15.7 (the manufacture store),
G19 (the transform condition), G29 (the regen move-type, `ChangeWaitType`) and
G33 (offline shops, `//transform`). One mechanism closed all of them.
`game_loop/sit_stand.rs` ports `Player.sitDown()`/`standUp()` plus the
`SitStand` player action (`ActionData.xml` id 0 = `/sit`, `/stand`).
**Sitting is two-phase and the phases are different predicates** — `sitDown`
flips the flag immediately and blocks actions for the 2.5 s animation, while
`standUp` broadcasts first and only clears the flag 2.5 s later, so "is
seated" (regen, the refusals) and "may act" (the block) are separate reads.
The seated `MoveType` now has a source, so the ×1.5 regen bonus finally
applies; `/mount` refuses a seated rider (SM 1013); `//transform` refuses a
seated target (SM 2283); the manufacture store and the offline shop both sit
their owner down, and `standUp` refuses while a store is open, which is what
keeps a vendor behind their wares. **Taking a hit stands you up** and clears
the store (Java `PlayerStatus.reduceHp`) — which, with
`OfflineDisconnectFinished`, means a damageable unattended shop is ended by
the first blow; an existing offline-shop test asserted the old outcome and was
corrected. 4 tests, 5 mechanisms sabotage-verified. The 16 left are mount
feeding, pet mounting, cubic-count, and per-site summon plumbing.

**Seated refusals + the stuck combat stance (fix, 2026-08-01, reported from
live play: "sit while in combat and the character still casts and does stuff,
and the stance never comes off").** Two bugs, both in the sitting slice above.
(1) The only thing refusing actions to a seated player was the **2.5 s
`SitBlock`** — an *animation* block that lapses while the character stays in
the chair. Java refuses on the seat itself, from places none of which had been
ported: `Player.useMagic`'s `_waitTypeSitting` branch (**SM 31**, checked
*before* `usedSkill.checkCondition` — which is why the transform condition's
own SM 2283 sitting leg is unreachable down the cast path in Java too, so an
existing test asserting 2283 was corrected),
`PlayableAI.onIntentionAttack`'s `isSitting()` early return (**silent** — no
packet at all), `CreatureAI`'s `AI_INTENTION_REST` branch on `MOVE_TO`,
`PICK_UP` and `INTERACT` (`clientActionFailed`), and `Npc.canInteract`'s
sitting leg. The port has no REST intention slot, so `sit_stand::is_resting`
stands in for it: `sitDown` and `StandUpTask` set and clear the seated flag at
exactly the moments Java enters and leaves REST. `sit_down` also does the rest
of `PlayerAI.onIntentionRest` now (`setTarget(null)` → `TargetUnselected`), and
its refusal condition gained the two missing legs of
`!isAttackDisabled() && !isControlBlocked()` (mid-swing, control-blocked).
(2) **Sitting killed the combat stance permanently.** `sit_down` removed the
whole `AttackState` component to model `breakAttack()` — but that component
also carries `stance_until_tick`, so the player left the 1 s stance sweep for
good: `AutoAttackStop` never fired (sword drawn forever, matching the report)
and `refresh_attack_stance`'s `if let Some` write silently no-opped from then
on, so no later fight could re-arm the stance either. Java's `breakAttack` →
`abortAttack` ends **only the current swing**; `AttackStanceTaskManager` is a
separate map that `sitDown` never touches. Only `attack_end_tick` is cleared
now. *Another conditional-component write failing open — and a second lesson:
when one component carries two lifetimes, removing it to end one ends both.*
2 new tests (`movement_tests`), both sabotage-verified; **2479 gameserver
tests green**, clippy clean.

**Mobs cast while running (fix, 2026-08-02, reported from live play: "monsters
run after me and cast at the same time; a mob can't cast and run at the
same time").** Java enforces that with **four** interlocks and the NPC path had
only one of them. The one that was ported is `AttackableAI.thinkAttack`'s ladder
guard, `(!npc.isMoving() && npc.hasSkillChance()) || (aiType == MAGE)` — and
that guard is exactly the one that *doesn't* stop a mage, because the MAGE arm
deliberately bypasses the `isMoving()` test (its job is to skip the
skill-chance roll). The three missing ones: (1) **`Creature.doCast`'s first
statement**, `if (isAttackable() && isMoving()) return;` — the blanket refusal
that catches the 402 MAGE templates on this dist *and* every boss/quest script
that calls `doCast` directly. It lives in `npc_cast::start_cast` for the same
reason it lives in `doCast` in Java: every NPC cast funnels through there.
Note it is gated on `isAttackable()` — a servitor is `Playable`, not
`Attackable`, and is not refused. (2) **`SkillCaster.startCasting`'s
`clientStopMoving(null)`** — the player path (`skills/cast.rs`) had it, the NPC
path never did, so nothing dropped the move data or broadcast `StopMove`;
`npc_ai::stop_npc` is now shared for it. (3) **`thinkAttack`'s opening
`if (npc.isCastingNow()) return;`** — which produced the reported symptom on its
own, for *any* caster and not just mages. `try_cast` correctly refuses a second
concurrent cast and returns `false`, but `false` means "no cast this think", so
the think fell straight through to the range tail and re-issued `chase()` every
second for the whole duration of the cast. The mob ran at the player with its
cast bar up. *A guard that reads "already busy → return false" is not the same
as "already busy → stop thinking"; the caller decides which one it got.*

**Guard (3) was found twice, independently and on the same day** — the Porta
fix above ("Mobs cast half as often as Java") landed it first, having hit the
*other* tail it lets through: the mob **swings** mid-cast. Same missing line,
two different symptoms, and neither investigation would have found the other's.
The branches were reconciled at merge (one guard, one comment naming both
tails). Guards (1) and (2) are this fix's own, and (1) is what the live report
was actually about — with (3) alone a mage still nukes mid-sprint, because the
refusal it needs is in `doCast`, not in the think.

2 new tests (`npc_cast_tests`), each with its zero case in the same test so the
assertion can't pass for an unrelated reason; both sabotage-verified.

**G19 sweep done 2026-07-31 (27 → 21).** Ranked by *learnable* carriers
first — the lesson from the earlier effects work — which is what made the
cluster tractable. Of the six unported affect scopes, **exactly one has a
learnable skill**: `DEAD_PLEDGE` carries the Bishop's **Mass Resurrection
(1254)**, which was falling back to single-target on a `SELF` cast and so
**did nothing at all**. The `DEAD_*` family is now ported
(`DEAD_PLEDGE`/`DEAD_PARTY`/`DEAD_UNION`): mirror images of PLEDGE/PARTY with
the liveness test inverted, and — the part that is easy to get wrong — the
**origin is filtered rather than assumed in**, because the caster of a SELF
mass-res is alive and must not appear in their own resurrection; the affect
limit therefore counts from 0, not 1. The other five scopes have **zero**
learnable carriers and stay documented fallbacks.
**Verified inert, not gaps:** `Stat.VITALITY_CONSUME_RATE` and
`Stat.BONUS_EXP`/`BONUS_SP` — a sweep of `data/stats/skills` finds **no skill
on this dist granting any of the three**, so they can never leave their
identity values and the two arithmetic sites are exact. The TODOs are replaced
with that fact.
Also landed: a **per-instance NPC title** (`Npc.title_override`) so an
`EffectPoint` seal wears its caster's name (Java `setTitle`) — with the
matching finding that `NpcInfo` only emits the TITLE block for a template with
`usingServerSideTitle`, which no EffectPoint on this dist sets, so **Java
stores it and never transmits it either**; and the transform gate's
**registered-on-event** leg, which G28's TvT roster unblocked (the sitting leg
alone remains, and has no state to read). 3 tests; **one initially passed
under sabotage** — the transform refusal was the skill's *cooldown* talking,
not the event gate, until the reuse was cleared first. 6 mechanisms
sabotage-verified.

**G22 sweep done 2026-07-31 (30 → 15).** The cluster's dominant shape was
`getSummonedNpcCount()` guards — 10 sites across six class-transfer quests,
all blocked on one missing mechanism. `Creature._summonedNpcs` is now a
`SummonedNpcs` component on the *parent* NPC, written by the quest spawn
helpers (`addSpawn(summoner, …)`) and read as `ctx.summoned_npc_count()`;
dead children are pruned on read rather than unhooked at despawn, which needs
no despawn hook and gives Java's answer — a **corpse still counts**, because
`removeSummonedNpc` fires at `onDecay`, not at death. The caps now bite in
Q225 (<5), Q226 (<1/<4/<36), Q227, Q229, Q232 (<1). **Java's guard placement
is not decoration**: in Q225 it wraps the key hand-out and the cond bump too
(a sixth attempt gets nothing), while in Q226 the cond bump sits *outside* it
(a 37th ambush still advances the quest) — both kept as written.
Second group: **`onKill`'s `isSummon`**, which the port never carried. The
notification now passes it and `ctx.killing_playable()` resolves Java's
`isSummon ? killer.getServitors()…orElse(getPet()) : killer`, so the Cave
Maiden's banshee, Pytan's Knoriks and the Primeval Isle ambusher go after the
**pet that landed the kill** instead of its owner across the map.
**Verified not-a-gap:** Q211's three "spawn caps" are
`SpawnTable.getSpawns(npc.getId()).size() < 10` — the number of spawn *points*
the killed mob has, not how many chests exist. Shyslassys, Baraham and the
Queen of Succubus have **one each** on this dist, so the condition is always
true and the port's unconditional spawn was already exact; the TODOs are
replaced with that explanation. 2 tests, 3 mechanisms sabotage-verified. The
remaining 15 are single-site tails (four-sepulcher persistence, the tamed
beast's buff task, NpcStringId chatter ids, `isDeleteAbnormalOnLeave`) or
blocked on unported subsystems.

**G24 sweep done 2026-07-31 (30 → 25).** The cluster's coherent half was
**siege sides**, which nothing modelled: `Player.siege_state` (1 attacker /
2 defender) + `siege_side` (the castle), stamped on every online member of a
registered clan by the new `siege::update_player_siege_state_flags` at siege
start, cleared at end, and re-pushed on a mid-siege capture. With them,
**same-side clans stop being able to attack each other** (Java
`isAutoAttackable`'s siege block: two defenders never fight; two attackers only
after the castle's **first mid victory**, which `capture` now sets), and the
`RelationChanged` siege icon becomes Java's — INSIEGE off the *subject's* own
state, ENEMY vs ALLY by comparing the two, ATTACKER on a besieger.
**Two latent bugs fell out of it**: (a) the port showed the siege crown to
*any* two players in an active zone, clanless bystanders included, because the
icon was derived from the zone rather than from registration — the existing
test asserted the wrong thing and was corrected; (b) `siege_relation_bits`'s
call sites passed viewer-then-subject, which was harmless while the function
was symmetric and wrong the moment it stopped being. Also: residential skills
now follow a member **joining** a castle-owning clan (not just logging in) and
the GM `//castlemanage` set/take-owner actions; `showRegWindow` serves the real
`SiegeInfo` packet, which had landed since the TODO was written. **Verified
not-a-gap**: leaving a clan needs no residential teardown — those skills ride
the same transient `ClanSkills` component the general clan-skill strip already
clears (a sabotage proved the added code was dead, and it was removed rather
than kept as belt-and-braces). 3 tests, 6 mechanisms sabotage-verified. The
remaining 25 are the genuinely bigger pieces — control-tower destruction,
weakened-door respawn, teleport-to-flag, castle crests, the HQ sub-zone — plus
the resurrection-blocked ones.

---

## Remaining-ports audit (2026-07-27)

A full sweep of the snapshot above + [PARITY_CHECKLIST_G33.md](PARITY_CHECKLIST_G33.md)
against the actual Rust tree, to scope the next priority. Quest porting is
**effectively done**: 163 Rust quest engines cover every in-scope Java quest —
the only unported ids (Q500 daily, Q933/935, Q10866, Q10993–11023) are
off-chronicle skips already ruled out. ~~Q255 Tutorial~~ landed 2026-07-28:
the full newbie flow (login → 5 s intro timer → TutorialShowHtml window +
voice, memoState machine through Newbie Helper / gremlin Blue Gemstone /
shot handouts / supervisor reward / Newbie Guide second batch), the tutorial
packet family (0xA6–0xA9 out, 0x85–0x88 in), the quest engine's
ON_PLAYER_LOGIN/PRESS_TUTORIAL_MARK/ITEM_PICKUP global-event hooks, and
`DisableTutorial` config. G18 clan
wars/sub-pledges/warehouse are verified present in code (the G33 checklist's
"G18 pending" bucket is stale). G24, G24.5, G29, G31, G32, G33 are complete.

| # | Gap | Milestone | What's actually missing | Player impact |
|---|-----|-----------|------------------------|---------------|
| ~~1~~ | ~~**Mail / post system**~~ | G30 ✅ | **DONE** (2026-07-27) — full ex 0x62–0x6C family, attachments, COD, expiry | — |
| ~~2~~ | ~~**Party matching room**~~ | G30 ✅ | **DONE** (2026-07-27) — rooms, waiting list, join/kick/disband/invite | — |
| ~~3~~ | ~~**Command channels / MPCC**~~ | ✅ | **DONE** (2026-07-28) — ex 0x06–0x08/0x2D (form/join/oust/roster: clan-5 / Strategy Guide 8871 / Baron+Clan Imperium forming right, party-side propagation, SM 1580–1594 family), chat channels 15/16, MPCC rooms 0x5A–0x61 sharing the matching-room registry, CC-wide XP/SP share + `partyLvl = cc.level` + raid-point split (`game_loop/command_channel.rs`), and **raid loot rights** (ground-drop ownership: killer-owned 15 s normal drops, CC-leader-owned `RaidLootRightsInterval` raid drops incl. the `AutoLootRaids=False` ground-drop gate + "You have looting rights!" announce + `isInLooterParty` pickup widening) | — |
| ~~4~~ | ~~**ai/areas zone scripts**~~ | G22 ✅ | **DONE** (2026-07-30, feat/g22-areas). **Slice 1 DONE** (2026-07-28): the talk/teleporter NPCs — Toma (script-owned wandering spawn, `area_npcs.rs` 30-min beat), ElrokiTeleporters (combat-gated ferry), PaganTeleporters (mark-gated doors + 10 s auto-close via new `doors::open_door_timed`), Tunatun (whip handout). **Slice 2 DONE** (2026-07-28): the small combat scripts — CaveMaiden + Pytan (kill-proc replacement ambush), FrozenLabyrinth (physical-skill shatter into six), PaganKeys (10% key drops honoring AutoLoot with killer-owned ground drops), HotSprings (escalating disease casts, level = victim's +1), PlainsOfDion (duel-interruption clan call with `$s1` NpcSay shouts — new `npc_say_param` packet), EilhalderVonHellmann (night raid boss on the new `DayNightCheck` beat off the G33 clock, 30 s in-combat despawn retry). **Slice 3 DONE** (2026-07-28): Den of Evil's Ragna Orcs — leaders pick *named* `<minions>` groups at spawn (parser now keeps `<minions name>`; the generic escort path spawns only the default `"Privates"` group, fixing an all-groups over-spawn) + the Frightened Ragna Orc bribe flow (whimper timer, 10M-adena promise at <20% HP, 10/1000-in-100k payouts as owned ground stacks, vanish); the zone's own Kasha-eye script is `@Disabled` on this dist with no eye spawns — skipped as dead content. **Slice 4 DONE** (2026-07-30): Ketra/Varka support NPCs as a mirror pair on one engine (`tribe_support.rs`) — 7 service roles each, alliance level from the highest mark item, the 8-buff horn/seed price list (cast + NPC HP/MP top-up), alliance-gated teleport menus. **Slice 5 DONE** (2026-07-30): ForgeOfTheGods — global kill-streak counter (`World::fog_kill_count`, cooled by the 15 s `FogRefresh` beat) escalating Lavasaurus eruptions by streak and forge floor (spawn z < -5000 = lower), erupted lavasauruses hate the killer and expire after 60 s. **Slice 6 DONE** (2026-07-30): the Beast Farm feeding chain — FeedableBeasts (growth tables ported verbatim incl. the Buffalo `{21481,21482}` quirk, spice-skill skill-see feeding, feeder lock, mad cows reverting after 10 s) + a new `TamedBeast` runtime (`game_loop/tamed_beast.rs`: `TamedBeastOf` component, 60 s spice clock — net -40 s/min with food, 5-min no-food grace — 1 s follow beat, starve/owner-gone despawn; TODO(G22) the owner-buff task). `BeastFarm.java` (Gracia revamp, NPCs 18874+ never spawn) skipped as dead content; BabyPets/ImprovedBabyPets deferred — pet-behavior add-ons needing a pet-summon hook. **Slice 7 DONE** (2026-07-30): two NEW script hooks — `on_aggro_range_enter` (fired from the aggro scan's first-hate seed) and `on_spell_finished` (fired from `handle_cast_end` for NPC casters) — plus Primeval Isle: Ancient Egg aggro-burst, Sprigant 15 s poison traps, and the Tyrannosaurus curiosity pause (bark → aggro clear → 6 s `TrexAttack` → stun + charge) with its Berserk ladder (`onSpellFinished` state machine, ported verbatim incl. the unreachable <30% branch). MonasteryOfSilence.java skipped — none of its Gracia-era NPCs (18909-12, 22789-95) spawn on this dist. Deferred TODO(G22): creature-see infra (Trex herbivore hunt, dino herd flee, Baium cleric-threat) + `<parameters>` skill holders. **Slice 8 DONE** (2026-07-30): **Four Sepulchers** — the full party dungeon: admission ritual (4+ party, leader-only, per-member Four Goblets + Entrance Pass checks, hall-occupied and 60-min window gates), hall sweep + door reset on entry, 3-min mysterious chest, data-driven wave table (new `data/four_sepulchers_data.rs` reading the datapack's `FourSepulchers.xml`, 700+ rows), clear-the-room waves with the 5 s defeat poll paying key chests, chapel gatekeepers consuming Chapel Keys and opening gates for 15 s, room-3 fleeing victim, room-4 charm/trap-zone toggles, room-5 petrified statue guards, room-6 adena chest, and the hall bosses paying per-member goblets + the exit teleporter; 60-min oust bell. TODO(G22): entry-clock persistence (Java GlobalVariables). **Row 4 / the ai/areas sweep is COMPLETE.** | High — hunting zones lose their signature behavior |
| ~~5~~ | ~~**ai/others NPC scripts**~~ | G22 ✅ | Plan: [PLAN_G22_AI_OTHERS.md](PLAN_G22_AI_OTHERS.md) — a 2026-07-30 re-audit (ids grepped against the Rust tree, `spawns/**` and `stats/npcs`) corrected two entries: **CastleTeleporter and SymbolMaker are *not* covered** (SymbolMaker's `Draw`/`Remove` bypass verbs exist, but its first-talk html is unported, so the dye NPCs are mute), while `SiegeGuards`/`SeeThroughSilentMove`/`Servitors`/`WyvernManager`/`CastleChamberlain`/`ClanHall*`/`MonumentOfHeroes`/`NewbieGuide`/`OlyManager`/race-track/charm/Valakas teleporters are. Verified **dead content** (no spawns, no spawner): Proclaimer (also Seven Signs), OlyBuffer, Scarecrow, DivineBeast, Incarnation, TreeOfLife; ClassMaster stays out of scope. **Slice 1 landed** (2026-07-30): the **`multisell`/`exc_multisell` NPC bypass** — the multisell engine existed but only the `_bbs*` entry points did, leaving the exchange button in **97 dist htmls** (44 `html/merchant`, 12 `html/petmanager`, the `ai/` scripts…) dead; `separate_and_send` now takes the NPC so `<npcs>` allow-lists match (Java `MultisellData.separateAndSend(id, player, npc, …)`) — plus the **three Mammons** (Merchant 31113 / Blacksmith 31126 / Priest 33511): the script-owned wandering spawn on the Toma pattern (`area_npcs::relocate_mammon`, 30-min `ScheduledTask::MammonRelocate`, `World.mammon_spawns` tracking Java's `_lastSpawn` — the Priest also has 7 *static* dist spawns, so a find-by-id despawn would eat a town NPC), `AnnounceMammonSpawn` (True here) naming the castle via the new `ZoneData::nearest_castle_at` (= `CastleManager.findNearestCastle`, corner-distance like Java's `ZoneForm.getDistanceToZone`), and the three chat windows (`scripts/mammons.rs`). 7 tests, both mechanisms sabotage-verified. **Slice 2 landed** (2026-07-30): the **castle staff** — `CastleBlacksmith` / `CastleWarehouse` (incl. the Blood Alliance claim + 30-Blood-Oath exchange) / `CastleMercenaryManager` (`CS_MERCENARIES` console, `buy <n>` buy lists, the `%feud_name%` limit pages) / `CastleDoorManager` (gate open/close off the template `<parameters>` `DoorId1`/`DoorId2`, frozen mid-siege, post teleports) / `CastleSiegeManager` (owner console, else the `SiegeInfo` registration window — which makes **audit row 11's siege-info window reachable from an NPC**) / `CastleTeleporter` (defender-only battlefield posts + the mass gatekeeper's `MASS_TELEPORT`), all in `scripts/castle_services.rs` over one rights layer that resolves the castle through `nearest_castle_at` (= Java `npc.getCastle()`), no id table. Underneath: `ResidenceTeleportZone` (`castle_teleport.xml`, 9 zones + their oust points) now loads, backing the new `siege::oust_all_players`; `ClanPrivilege.CS_OPEN_DOOR`(16)/`CS_MERCENARIES`(22) are named — which exposed and **fixed a bug in `RANK9_PRIVS_MASK`** (it used bit 15 = `CH_SET_FUNCTIONS` for `CS_OPEN_DOOR`, so academy ranks kept hall-function rights and lost the castle-door right). `CastleSideEffect` skipped (`ExCastleState` is Grand Crusade). 9 tests, the siege door-freeze and the oust territory filter sabotage-verified. **Slice 3 landed** (2026-07-30): the **small combat behaviours** in `scripts/mob_behaviours.rs` — `PolymorphingOnAttack` (15 morph chains: HP threshold + chance + the stage bark, the new form inheriting the attacker's hate), `PolymorphingAngel` (kill → the twin rises), `TimakOrcTroopLeader` (one private per swing on `SummonPrivateRate`, capped at 3 — new `minions::add_minion`/`count_spawned_minions`/`minion_of_id_alive`, since the existing path only tops a whole group up), `FleeMonsters` (Elpy runs 500 units away on the Fear geometry), `FairyTrees` (immobile; 20 Soul Guardians on a kill within 1500, half opening with Venomous Poison) and `NonLethalableNpcs` (new `NotLethalable` marker read by the `Lethal` effect — the siege HQ can't be lethal-blown). Also **fixed quest 421**, whose guardian swarm was missing Java's `ALT_PARTY_RANGE` gate (both scripts fire on a tree kill — 40 guardians — as in Java, but neither should fire from 2000 units away). 9 tests, the tree's range gate and the one-private-per-swing cap sabotage-verified. **Slice 4 landed** (2026-07-30): **day/night spawn groups** (`game_loop/spawn_scripts.rs`) — the spawn loader now keeps a template's `ai=` and `<parameters>` and a group's `name=`/`spawnByDefault`, which **fixed a live double-population bug**: `spawnByDefault="false"` was unparsed, so boot placed *both* halves of all 50 `DayNightSpawns` templates (95 groups — every Devil's Isle and Interlude day/night tile stood with its day *and* night mobs at once). Boot now skips script-owned groups (Java `spawnAll(SpawnGroup::isSpawningByDefault)`), `activate_at_boot` places the half matching the G33 clock, and the existing minute beat (`area_npcs::handle_day_night_check`, which already drove Eilhalder) swaps them on transition; `respawn_is_in_phase` drops a scheduled respawn for an out-of-phase group so a mob killed at dusk doesn't climb back out at noon. `NoRandomActivity` (Rune's Chapel Guards) landed with it as a per-NPC `SpawnActivity` override, since random-walk/animation are otherwise read off the shared template. 6 tests, the boot skip and the respawn guard sabotage-verified. **Slice 5 landed — ROW 5 CLOSED** (2026-07-30): the talk/utility tail — `ArenaManager` (adena-priced CP/HP recovery, paid up front and cast 2 s later unless the buyer stepped into a PVP zone, + the six-buff package), `ToIVortex` (ten floors, each eating a dimension stone of its colour, + the 100k-adena stone counter), `SymbolMaker` (the dye window itself: its `Draw`/`Remove` buttons were wired to `game_loop::henna` a milestone ago, but nothing served the page carrying them, so the dye NPCs were mute), `RandomWalkingGuards` (`Guard`-type NPCs have random walking off by template, hence the script — a 15–45 s `GuardRandomWalk` beat strolls them around their post) and `Servitors/SinEater` (the one pet with a voice: greeting, 60 s gripe beat, on-attacked and on-death lines on the existing pet-summon/damage/death paths; `onSummonTalk` is a documented `TODO(G22)`). 6 tests, the dimension-stone gate and the stroll re-arm sabotage-verified — the re-arm assertion was rewritten after the first version passed against the sabotage (the spawn hook's own task was still queued). **Every `ai/others` script is now ported, covered by another module, or in the plan's verified-skip table.** | Medium–high, varies per script |
| ~~6~~ | ~~**Private buy store**~~ | G15 ✅ | **DONE** (2026-07-30) — the mirror of the sell store, in `game_loop/private_store.rs`: `RequestPrivateStoreManageBuy` 0x99 (the manage window, `PrivateStoreManageListBuy` 0xBD), `SetPrivateStoreListBuy` 0x9A (Java's gate ladder in order — attack stance, the `MaxPvtStoreBuySlots*` limit (5 Dwarf / 4 other, new config keys), per-line and total `MAX_ADENA` overflow, and "can you afford this list"), `RequestPrivateStoreQuitBuy` 0x9C, `RequestPrivateStoreSell` 0x9F (a customer selling in: items customer→owner, adena owner→customer, clamped to what is still wanted and what they actually hold, with the owner's adena **re-checked** at sale time since they can spend elsewhere while the store stands), plus the customer's view (`PrivateStoreListBuy` 0xBE, filtered to lines the viewer can fill) and the click routing. **Store titles landed with it**: `SetPrivateStoreMsgSell` 0x97 / `SetPrivateStoreMsgBuy` 0x9D + `PrivateStoreMsgBuy` 0xBF — 0x97 was missing too, so sell stores were nameless. New `PrivateBuyStore`/`WantedItem` components (wanted lines are keyed by **item id** — the owner holds nothing yet). Skill-enchant's "no private store open" gate now counts both kinds. 3 tests; the affordability gate and the wanted-count clamp sabotage-verified (the clamp test had to be strengthened with an over-sell case first — offering 10 against 4 wanted). **Not ported:** the wholesale `SetPrivateStoreWholeMsg` ex 0x47 (package-sell's sibling, and package sell itself is still deferred). Offline-trader restore is now unblocked | Medium-high — economy staple |
| ~~7~~ | ~~**Zaken**~~ | G23 ✅ | **DONE** (2026-07-28) — the Java script is only the shared lifecycle (already generic) + the `BS01_A`/`BS02_D` roars, now broadcast from `grand_boss` for all four simple bosses | — |
| ~~8~~ | ~~**Manor economics + Mammon NPCs**~~ | G26 ✅ | **DONE** (2026-07-30) — castle treasury + manor rollover settlement + Mammon economics: the vault, tax zones, liege cascade, the merchant/multisell/manor income paths, the chamberlain vault console, and the full `changeMode` economics (crop payout → clan warehouse, treasury refund/charge, next-period gating, leader notifications, `storeMe` persistence), and the inventory-only `exc_multisell` windows the Mammon (and town-blacksmith) exchanges run on — see the G26 row. `//mammon_*` has no handler in this Java build | — |
| ~~9~~ | ~~**G15/G15.5 small tails**~~ | ✅ | **Six slices landed 2026-07-30 — the row is closed.** *(a)* The **user-command sweep**: every `usercommandhandlers/*` this build registers is wired — `/time`, `/mount`, `/partyinfo`, the clan-war lists, `/instancezone`, the command-channel trio, `/siegestatus`, `/clanpenalty`, `/olympiadstat`, `/mybirthday` (+ `ExMultiPartyCommandChannelInfo` and `ExInzoneWaiting`; `characters.create_date` now reaches `Player`). *(b)* The **gatekeeper tails**: Mon/Tue 20:00+ half price, `isSubClassActive()` in the fee, both siege gates (`TeleportWhileSiegeInProgress=False` — besieged destination refused, castle gatekeeper's busy/owner/no landing pages), the combat-flag gate, and the noble list page. **Teleport bookmarks are NOT portable** — Java registers `EX_BOOKMARK_PACKET` (0x4E) with a `null` handler, so the feature does not exist in this build. *(c)* **Augment option effects**: `data/xml/OptionData` ported (`data/stats/augmentation/options`, 342 files → 34 k options; ~19.4 k of them rollable by this dist's `Variations.xml`, ~80 % stat-only). An equipped augmented item's two option ids now pump the wearer through the same passive-buff mechanism the clan-skill/grade-penalty pumps use, applied on equip and removed on unequip and on augment-cancel — Java's `VariationInstance.applyBonus`/`removeBonus` equip listeners. Passive-skill options fold their skill's own stat effects in too; the option's **active** and **activation** (`attack`/`magic`/`critical`) skills are parsed and carried but not yet granted (`TODO(G15.5)` — they need a temporary-skill grant path and a trigger registry keyed off something other than a learned skill). *(d)* **Package sell** (`PrivateStoreType.PACKAGE_SELL`, 8): `/packagesale` (player action 61 — the one private-store action with no client packet of its own) opens the manage window in package mode; `SetPrivateStoreListSell`'s leading flag (previously read and discarded) now sets the store type, the manage/list packets carry the `packaged` byte, the title rides `SetPrivateStoreWholeMsg` (ex 0x47) → `ExPrivateStoreSetWholeMsg` (0xFE:0x81) instead of `PrivateStoreMsgSell`, and a buyer asking for fewer lines than the store holds is refused (Java's anti-bot all-or-nothing check). *(e)* **Freight send** (`package_deposit` → `PackageToList` → `RequestPackageSendableItemList` → `RequestPackageSend`): the account's other characters now ride the session (Java `Player._chars`, snapshotted at character select), the send window lists the sender's `is_freightable` items, and the send charges `FreightPrice` per slot and delivers — to a live `Freight` component when the recipient is online, otherwise straight to their `items` rows (`loc = FREIGHT`) through a new `DbCommand::AddFreightItems`, since a component only exists for a logged-in character. New `is_freightable` item flag + `FreightPrice`/`MaximumFreightSlots`/`AltKarmaPlayerCanUseWareHouse` config. **Dist finding: no item below id 10000 is freightable on this dist** — all 3416 that declare the flag are later-chronicle (10649+), and Java gates on the same flag, so on Interlude content the freight send has no legal cargo. The gate is ported faithfully rather than loosened. *(f)* **`SKILL_REDUCE_ON_SKILL_SUCCESS` timing**: the triggering item now rides the cast (`CastState.trigger_item_object_id`, Java `SkillCaster._item`) and is spent by the **finish** phase, between the MP/HP consume and the effects, exactly where Java's `finishSkill` does — so an interrupted cast no longer costs the item, and a failed spend aborts the cast with no effects (Java's `return false`). Interlude's pair is 8058/8060 → skill 2260. **Row 9 is COMPLETE.** | — |
| ~~10~~ | ~~**TvT polish**~~ | G28 ✅ | **DONE** (2026-07-30) — *(a)* the per-phase countdown screens (Java's `"10"`…`"1"` timers, carried as one `TvtCountdown` task with a generation a forfeit bumps) and the arena manager's **BuffHeal** (class buff set + full HP/MP/CP, refused in combat; the two arena manager copies are recorded so the in-arena NPC serves `manager-buffheal.html`). *(b)* **HQ-zone kicks + the inactivity clock**: the zone mask is per *kind*, so the named colosseum peace zones needed their own edge-triggered field (`ZoneFlags.tvt_hq_zone` + `ZoneData::tvt_hq_zone_at`) fired from `revalidate_zone` — the enemy bounce, the warning/kick pair (with Java's warm-up branch), cancel-on-exit, and the kick's forfeit-or-announce tail; a respawn re-arms it, as in Java. *(c)* The **cron auto-schedule**: `commons::cron` ports `SchedulingPattern` (five fields, `*`/ranges/lists/steps, `0`=`7`=Sunday, UTC), `events::schedule_at_boot` reads each event's `config.xml` and each firing re-arms itself — **this dist ships the TvT schedule commented out**, so nothing auto-starts until an operator uncomments it. *(d)* The **cursed-weapon window** (ex 0x2A/0x2B → `ExCursedWeaponList` / `ExCursedWeaponLocation`), including Java's send-nothing-when-none-live. | — |
| ~~11~~ | ~~**Packet stragglers**~~ | G33 ✅ | **DONE** (2026-07-30) — all four are dispatched. **Dist finding: `RequestSiegeInfo` 0xAA has an empty `readImpl` *and* `runImpl` in this Java build** — the packet does nothing; the `SiegeInfo` window is pushed by the castle Siege Manager's bypass (landed in row 5, slice 2), so the feature is already reachable and the opcode is now a documented empty dispatch arm, matching Java. `CannotMoveAnymore` 0x47 → `position::handle_cannot_move_anymore` = Java's `EVT_ARRIVED_BLOCKED`: the in-flight `Movement` (and any pending `PathWait`) is dropped, a `MOVE_TO`/`CAST` intention falls back to ACTIVE (the port has no Move intent variant — a walk *is* the `Movement` component — so only `Cast` is cleared; Attack/Interact survive and re-issue their own walk, as in Java), the player is planted where the client says it stopped, the zone is revalidated, and `StopMove` broadcasts **including the mover**. `ExRequestSaveKeyMapping` ex 0x22 → new `game_loop/settings.rs`: the blob is stored tab-joined as **signed** bytes in the `UI_KEY_MAPPING` player variable (Java's `SPLIT_VAR` encoding, `StoreUISettings=True` here) so it persists with the character's other variables, and both `RequestKeyMapping` ex 0x21 and the **enter-world burst** now replay it — `ex_ui_setting` previously always sent an empty payload, so a saved layout was silently lost on relogin. Augment confirm dialogs ex 0x26/0x28/0x3F → `augment::handle_confirm_target_item` / `handle_confirm_gemstone` / `handle_confirm_cancel_item`, echoing the weapon, the gemstone fee and the augmented item (with its two option ids + cancel price) back to the client, refusing with `THIS_IS_NOT_A_SUITABLE_ITEM` / `AUGMENTATION_REMOVAL_ONLY_ON_AN_AUGMENTED_ITEM`; `VariationData::has_fee_data` is the Java `hasFeeData` gate. 3 tests, every mechanism sabotage-verified. **THE AUDIT IS CLOSED** (offline-trader restore, its last open item, landed 2026-07-31). | — |

Open scope decision: **mentoring** — the 2026-07 ROADMAP audit ruled it
in-scope (→ G17), but the G33 checklist buckets its packets as
later-chronicle skip. Settle which is right before anyone picks it up.

**Rows 1–9 are now done** (mail + party matching 2026-07-27, command
channels + Zaken 2026-07-28, the `ai/areas` sweep and the whole `ai/others`
sweep 2026-07-30 — see [PLAN_G22_AI_OTHERS.md](PLAN_G22_AI_OTHERS.md) for the
five slices and the verified-skip table). With the script breadth and the buy
store closed, **row 8 is now done too** (2026-07-30): the castle treasury
(vault + tax zones + liege cascade + merchant/multisell/manor income + the
chamberlain vault console), the manor rollover settlement (`changeMode`
economics + `storeMe` persistence) and the Mammon economics (the inventory-only
`exc_multisell` windows their exchanges run on) — **G26 is complete** — and
**row 9 closed the same day** in six slices (user-command sweep, gatekeeper
tails, augment option effects, package sell, freight send, skill-reduce
timing). **Rows 10 and 11 closed 2026-07-30** as well — row 11's four packet
stragglers are dispatched, and `RequestSiegeInfo` 0xAA turned out to be an
**empty handler in this Java build** (the siege-info window is pushed by the
castle Siege Manager bypass, which row 5 already landed). **The audit is now
closed**: offline-trader restore, its last open item, landed 2026-07-31 (see
the G33 row) — **every audit row is done.**

---

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
- **Fix (2026-08-02) — `maybeMoveToPawn` was never actually ported.** Java runs
  *one* helper for `thinkAttack`, `thinkCast`, `thinkInteract` and `thinkPickUp`
  alike (`CreatureAI.maybeMoveToPawn(target, offsetValue)`); only the offset
  differs — `getPhysicalAttackRange()`, `getMagicalAttackRange(skill)` (= the
  skill's `castRange`), or the flat 36 of the two interaction paths. That is
  exactly why a bow's 500 and a dagger's 40, and an attack and a cast, cannot
  drift apart: they are the same code with a different number. The port had
  instead grown an independent `chase_target`/`chase_pawn` pair — consistent
  between attack and cast, but missing three of the helper's behaviours:
  1. the **100-unit engage hysteresis** granted while a follow is running
     (`if (isFollowing()) { if (!isInsideRadius2D(target, offsetWithCollision +
     100)) return true; stopFollow(); return false; }`). Java re-checks its
     follow once per second; the port re-checked the strict gate 10× a second,
     so a chase after anything that kept walking re-pathed forever and never
     got to swing or cast — worst for archers, whose 500-unit reach makes that
     band the easiest to sit in;
  2. the **100-unit deeper aim at a moving pawn** (`if (target.isMoving())
     offset -= 100`, floored at 5), which is what makes the walk converge on a
     runner instead of trailing it at exactly reach; and
  3. the **`isMovementDisabled()` branch** — and with it a real bug: neither
     the chase leg nor `model::movement::tick` consulted it, so a *rooted*
     player who clicked a distant target walked anyway (only `position.rs`'s
     client-move handler ever refused). Java is deliberately asymmetric here —
     an ATTACK intention gives up (`setIntention(AI_INTENTION_IDLE)`), every
     other intention stands still and keeps waiting.
  Ported as `combat::maybe_move_to_pawn`, now the single gate all four think
  loops call, plus a `Following` component standing in for the actor's row in
  `CreatureFollowTaskManager.ATTACK_FOLLOW_CREATURES` (same payload — the
  follow range recorded at `startFollow`, already shrunk for a moving target,
  never refreshed while the follow lives). `startFollow` is for
  creature-and-not-door pawns only, so a siege gate or a ground item takes the
  plain `moveToPawn` branch and earns neither the slack nor the −100, matching
  `isFollowing()`'s own `_target.isCreature()` test. The latch is released by
  the engage path and swept once per tick against the intent that started it
  (Java's `changeIntention` → `stopFollow()`).

  The same audit's remaining six landed in a follow-up pass:
  4. **The walk destination no longer carries the collision radii.**
     `maybeMoveToPawn` hands `moveToPawn`/`startFollow` the *raw* offset —
     radii are part of `offsetWithCollision`, i.e. the range **test**, and of
     nothing else — so the walk ends `offset − 5` from the pawn's centre and
     the `MoveToPawn` packet carries that same raw offset for the client to
     stop at. (`AttackableAI.thinkAttack` genuinely *does* pass a
     radii-inclusive range to its own `moveToPawn`, so `pawn_destination` keeps
     taking a plain distance-from-centre and the question stays at the call
     sites.) `moveToPawn`'s own `max(offset, 10)` floor came with it.
  5. **Re-path cadence.** `moveToPawn` refuses to re-path — and to re-broadcast
     — toward the same pawn at the same offset inside `_moveToPawnTimeout`
     (1 s; 2 s more while the live path came from the pathfinder and the offset
     *changed*). Java's follow task fires at 500 ms but lands on this throttle,
     so the real cadence is 1 s; the port's 5-tick gate was the follow period
     mistaken for the whole story, at twice the packet rate. Kept as a
     `MoveToPawnState` component — Java's `_target`/`_clientMovingToPawnOffset`/
     `_moveToPawnTimeout` triple, with `Movement` standing in for
     `_clientMoving`.
  6. **The follow task's own gates**, in `follow_step`: its range test is 3D and
     centre-to-centre (*not* the engage gate's 2D-plus-radii), and past 3000
     units it gives the intention up outright rather than start a cross-map
     walk — "if the target is too far (maybe also teleported)".
  7. **`moveToLocation`'s z compensation**, `offset -= Math.abs(dz)` floored at
     5: a pawn up a slope is walked to that much more tightly, since the 2D
     geometry can't see the height the offset is really being spent on.
  8. **Shift is not a melee `dontMove`.** `AttackRequest` reads its trailing
     byte into `_attackId`, which Java marks `@SuppressWarnings("unused")` —
     so a shift-attack chases exactly like a plain click, and the port's SM 22
     refusal was a behaviour retail does not have. `Action` case 1 is a
     separate story with the same conclusion: non-GM, `AltGameViewNpc` off, it
     falls back to `obj.onAction(player, false)`, which selects and skips the
     entire `else if (interact)` arm where attacking and talking live. The
     `shift` parameter is gone from `start_attack_intent` and
     `interact_with_npc` accordingly. Conversely the *cast* `dontMove` is real,
     and its metric is not the walk gate's: the target handlers test
     `calculateDistance2D(target) > skill.getCastRange()` with **no collision
     radii**, strictly tighter than the `castRange + radii` the AI would have
     walked into, so the two now measure separately.
  9. **An NPC measures its cast range in 3D.** `AttackableAI.checkSkillTarget`
     passes `includeZAxis = true` to `Util.checkIfInRange`, unlike the player's
     2D `SkillCaster.castSkill` gate — a mob won't open fire on something far
     above or below it. `npc_cast` measured 2D.

  13 tests across `chase_parity_tests`, `combat_tests`, `npc_tests` and
  `npc_cast_tests`; every one of the nine fixes is sabotage-verified
  individually. No `TODO` left behind from this audit.
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

### Post-G23 — Archangel target picks zone-gated ✅ (2026-07-25)
Baium's archangels locked onto players on the tower floor *below* the lobby:
`SELECT_TARGET`'s pick measured 2D (post 4 stands ~85 from the reported 13F
spot, ~930 z apart) and skipped Java's `zone.isInsideZone(creature)` gate on
`baium_no_restart` (70051, z 10061–11061). Ported: 3D reach, zone gate on
both the candidate scan and the held-target keep-check, and stale hate on
players who left the zone is dropped at each re-pick (Java parks the mob in
FOLLOW instead; this AI has no FOLLOW intention, so the entry must go or
`think_attack` chases the departed player). Falls open when the zone table
isn't loaded (test worlds). Tests: the exact 13F report (fails pre-fix) +
abandon-on-zone-exit.

### Post-G23 — Vertical aggro/chase geodata parity ✅ (2026-07-25)
Aggro mobs could engage and *move vertically between tower levels* (Cruma/ToI
floors) and never dropped a target they reached by gliding — none of Java's
vertical protections were in the AI layer (the geo raytracer itself was fine).
Ported, all in `npc_ai.rs`:

- **Aggro scans are 3D spheres** — Java `World.forEachVisibleObjectInRange`
  measures `calculateDistance3D`; the monster/siege/guard scans measured 2D,
  so a player a floor above was "in range".
- **`thinkAttack`'s LOS gate** (AttackableAI: "Actor should be able to see
  target"): a mob that cannot see its hated target neither calls the faction,
  casts, chases nor swings — it issues `moveTo(target)`, an ordinary
  geo-validated walk (clamp + path worker), i.e. it takes the stairs.
- **`chase()` runs through the shared geodata block** (`npc_geo_move`): the
  pawn destination is `getValidLocation`-clamped and re-routed through the
  path worker when the straight line is cut (>30 shortfall); `MoveToPawn` is
  broadcast only for direct moves, routed moves announce `MoveToLocation`
  (Java `AbstractAI.moveToPawn`). Before this the chase wrote a straight-line
  `Movement` with no geodata at all — the literal vertical glide.
  **Deliberate divergence:** Java's "Monsters can move on ledges" exception
  (skip the clamp when `|dz| > 100`) is *not* ported — in Mobius it lets a
  monster with cross-floor hate move in an unchecked 3D line, which is the
  exact reported bug; the clamp+path route is what the LOS-gated design
  intends.
- **`AggroInfo.checkHate`**: hate zeroes for an attacker who is dead,
  despawned, or outside the NPC's 3×3 surrounding regions — run before every
  most-hated pick. This is Java's actual "loses aggro" mechanism; without it
  a departed player stayed most-hated forever.
- **Attack-timeout parity**: on the 2-minute timeout the aggro list is *kept*
  (Java keeps it; `checkHate` forgets departed targets), and a monster still
  in combat stance (`has_attack_stance`) — or with no players left watching —
  **teleports** back to its spawn (`relocate_npc` = `Npc.teleToLocation`)
  instead of walking.
- **`faction_call` z-band** now measures against the *target*'s z (Java
  compares `finalTarget.getZ() - nearby.getZ()`), and the recruit range is 3D.

Tests (`geo_vertical_tests.rs`, 7): 3D-sphere scan; wall = no engagement +
path request aimed at the target; see-over fence = chase re-routes instead of
`MoveToPawn`; **real Cruma Tower geodata** — a mob on the ground layer of a
stacked cell neither aggros a player on the floor above nor glides up to one
that shot it (fails against the pre-fix `npc_ai.rs`, verified by stash-swap);
checkHate region leash; timeout teleport-home (in combat) vs stay-put (idle,
players watching).

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
  ALLIANCE answer SM 4202/4203. Guards: 105-char cap (SM 1078), raised to
  500 for a line carrying a shift-clicked item link and skipped entirely for
  a GM; malformed type/empty text **log-and-drop instead of Java's force
  disconnect** (deliberate deviation). Shift-click item links are live —
  `Say2.parseAndPublishItem` + `RequestExRqItemLink`/`ExRpItemLink`, see the
  entry at the end of this file. Chat bans/jail/olympiad/block-list/
  say-filter/voiced commands skipped with their systems.
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
- **Water** (completed 2026-08-01, `feat/water-parity`): `Speeds.swimming`
  flips on enter/exit (`getMoveSpeed`'s swim branch) + `broadcastUserInfo`.
  **The swim speeds now also reach the client**: `UserInfo`/`CharInfo` fill
  their run/walk slots from Java's `getRunSpeed()`/`getWalkSpeed()`, which
  return the *swim* stats while `isInsideZone(WATER)` — the port sent the land
  speeds there, so the client kept predicting and animating at 120 while the
  server swam at 50, and entering water felt like no slowdown at all.
  `getMovementSpeedMultiplier` likewise picks its divisor among all four
  template bases (`Speeds::base_{walk,swim_run,swim_walk}_spd` are new), so the
  leg cadence matches the mode. **Drowning** is ported (`game_loop/water.rs`):
  `checkWaterState` on every revalidate under the new `General.ini`
  `AllowWater`, the 60 s cyan `SetupGauge` breath bar (`Stat.BREATH`'s default —
  nothing on this dist declares the stat), then 1% of max HP a second with
  SM 297, `directlyToHp` so CP does not soak it (new
  `combat::player_receive_damage_ex`), cancelled on surfacing and in `doDie`.
  Damage you deal to yourself is **silent**: `PlayerStatus.reduceHp` wraps both
  its CP absorb and the `C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2` line in
  `attacker != getActiveChar()`, and every environmental source names the victim
  as its own attacker — without that guard drowning printed a second, redundant
  "Bob has received 4 damage from Bob" and lava let CP soak the tick.
  **Movement**: `moveToLocation`'s `isInWater` (`WATER && !CASTLE`) is now a
  real predicate — `castle_hall.xml`'s 9 `CastleZone`s load as
  `ZoneKind::Castle` — and it drives the geodata exemption, the
  dz-counts-as-travel timing, and the newly ported 700-unit swim-click clamp.
  Also: `WaterZone.onEnter` cancels a transform whose template lacks `can_swim`
  (157 of 174 on this dist, now parsed), `onExit` skips the broadcast
  mid-teleport, and `dismount()`'s 1.5 s post-dismount-into-water `UserInfo`
  resend is armed (`ScheduledTask::DismountWaterUserInfo`). NO_RESTART only
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
    `//remove_clan_penalty`. `//character_info <name>` (and `//current_player`)
    now also **re-targets the GM** onto that character, as Java
    `showCharacterInfo` does (2026-08-03): the port only rendered the html, so
    picking a name off the `//show_characters` roster left the GM's target
    untouched and every `charinfo.htm` button behind it (`Lv/Exp/Sp` first)
    answered `INVALID_TARGET`.
  - **B2 items** (`items`): `//create_item`/`//give_item_target`/
    `//give_item_to_all`/`//create_coin`/`//itemcreate`/`//enchant` menus,
    `//destroy_items`/`//destroy_all_items` (+`destroyitems`/`destroyallitems`).
  - **B3 spawns** (`spawn`): `//spawn`/`//spawn_monster`/`//spawn_once`/
    `//spawnat`, spawn+npc HTML menus, `//list_spawns`/`//list_positions`/
    `//top_spawn_count`/`//spawn_debug_print`/`//scan`, `//summon`, `//delete`.
    `//scan` re-ported to full `AdminScan` parity (2026-07-25): **3D** radius
    (default 1000) with `id=`/`name=`/`radius=` filters and 15-row
    `PageBuilder` pagination — the old port dumped the whole 3×3 region block
    (every stacked ToI/Cruma floor) into one unpaginated html and crashed the
    client. `AbstractHtmlPacket.setHtml`'s 17 200-char clip is now also in the
    `NpcHtmlMessage`/`ExNpcQuestHtmlMessage` builders as the generic guard.
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
    char/party/clan, kick/kill menu). The Character panel's **name-carrying**
    buttons — "Go To" (`admin_goto_char_menu`), "Get Party"
    (`admin_recall_party_menu`) and "Get Clan" (`admin_recall_clan_menu`) — now
    resolve the character already chosen on the previous page (`%name%` /
    `$qbox`, Java `World.getPlayer(command.substring(n))`) instead of demanding
    a live GM target: "Go To" used to be a bare alias of `//teleto`, so it
    answered "Select a target first." on a character picked from the roster.
    The GM's own selection is now only the blank-QuickBox fallback.
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
  + `//unride*`. `Player.mount_type`/`mount_npc_id`/`mount_level` are durable
  state serialized into UserInfo/CharInfo (mount byte identical to the old
  hardcoded 0 when unmounted — the real-capture byte test still passes) plus a
  `Ride` (0x8C) broadcast. The mounted speed swap is real: `recalculate_stats`
  substitutes the pet's `speed_on_ride` row (halved at a 10+ mount/rider level
  gap; hungry-halving is TODO(G29) with mount feeding), and mounting swaps the
  collision cylinder to the mount NPC template's. **Wyvern flight** works
  end-to-end: `Player::is_flying` (= wyvern mount) exempts movement from the
  geodata clamp/pathfinder and lets `ValidatePosition` trust the client Z
  (swimming shares both floating exemptions), UserInfo/CharInfo carry the fly
  speeds, and `Player.dismount`'s mid-air gates (z > 10000 / >300 above the
  geo floor, water-below exception) are ported with their SMs. Player-facing:
  action 38 + `/dismount` (user command 62) dismount; the **WyvernManager**
  script (`scripts/wyvern_manager.rs`, 11 NPCs) trades a ridden level-55+
  strider + 25 B-crystals for a wyvern, gated on residence ownership +
  `Feature.ini` (`config/feature.rs`: `AllowRideWyvernAlways` False on this
  dist keeps castle managers Dusk-blocked, exactly like Java). Pet mounting
  itself (action 38 mount half, `/mount`) stays TODO(G29).
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
- **Geodata editor** — ✅ **COMPLETE 2026-08-01** (`admin/geo_editor.rs`,
  `admin/world_cmds.rs`, `admin/debug_draw.rs` + `geo`). Every `AdminGeodata`
  command and `AdminPathNode` are ported:
  - `//geo_pos`/`//geo_spawn_pos`/`//geo_can_move`/`//geo_can_see`/`//geomap`/
    `//geocell` — the read-only reports.
  - Runtime NSWE editing — `//geoenable*`/`//geodisable*` **and Java's
    `//en`/`//dn`/`//es`/`//ds`/`//ee`/`//de`/`//ew`/`//dw` aliases**, with the
    optional `<geoX> <geoY>` pair; the short forms re-open the cell panel on the
    edited cell (Java's `!actualCommand.contains("geo")` branch), the long forms
    report in chat. Edits go into a `GeoEngine` override map (`RwLock<HashMap>`
    gated by an `AtomicBool`, so the pathfinding hot path is one relaxed load
    when nothing is edited) and apply immediately to movement/pathfinding.
  - `//geoedit` + `//ge <geoX> <geoY>` — the community-board cell panels over
    the dist `geoedit.htm`/`geoedit_cell.htm`, **heading-rotated** exactly as
    Java (19×19 grid of `//ge` buttons; four-arrow single-cell editor whose
    buttons are the short aliases), green = passable, red = blocked.
  - `//geogrid [off]` — the one-shot `ExServerPrimitive` NSWE arrow overlay
    (`GeoUtils.debugGrid`/`hideDebugGrid`), sharing the Debug panel's renderer
    and its `DebugGrid_<n>` names, as Java's `AdminDebug.setGeodataDebugging`
    re-issues `admin_geogrid` on its timer.
  - `//geosave`/`//geosaveall` — **the binary region serializer is ported**
    (`geo::region::Region::write_to`): the region is re-emitted in the on-disk
    `.l2j` layout with the override map folded into the cells it edits,
    including Java's flat→complex block promotion on a *disable*
    (`Region.convertFlatToComplex`, truncation quirk and all). Untouched blocks
    are copied verbatim, so an unedited region round-trips byte for byte. Output
    goes to `GeoEngine.ini`'s `GeoEditPath`. **Deviation:** `//geosaveall` runs
    on a worker thread (Java writes ~1.4 GB on the caller's thread, which here
    is the game loop) and streams Java's per-region lines back as they land.
  - `//path_find` (`AdminPathNode`) — runs the cell pathfinder GM→target on the
    game thread as Java does and dumps every node, with Java's `No Target!` /
    `No Route!` / `PathFinding is disabled.` answers.
- Tests: 5 `admin_data` units + 74 synthetic-world dispatch/handler tests
  (gating, confirm round-trip, colors, one+ per handler group, mount +
  transform round-trips, mob-group lifecycle) + a geo NSWE-override unit test +
  **6 geo-editor tests** (`tests/geo_editor_tests.rs`: panel cell resolution,
  heading rotation asserted per *screen slot* — asserting a cell appears
  anywhere is vacuous, cell-panel edit + re-open, long-form edit, `//geosave`
  reload round-trip, `//path_find`) and **5 region-serializer units**
  (byte-for-byte round-trip, complex nibble patch, flat promotion, enable-only
  no-op, multilayer layer targeting). All sabotage-verified.
- **GM shift-click NPC view** — ✅ **LANDED 2026-08-01**
  (`admin/npc_info.rs`). Reported as "shift-click on an NPC should bring up the
  admin view". `NpcActionShift` has **two** branches and only the non-GM one was
  ported: every shift-click took the `AltGameViewNpc` player path (and with that
  config off — the default — did nothing at all), because `npc_view.rs` was
  written before `Player` carried an access level and its module doc still said
  the GM branch "is not modeled". `handle_action` now tests `is_gm()` first,
  exactly like Java's `Action` case 1, and serves `data/html/admin/npcinfo.htm`
  through the existing `menu::show_admin_html_replace` channel
  (`NpcHtmlMessage(0, 1)`, so the window survives its own bypass buttons):
  identity/race/spawn line/respawn/chase range/distance, the combat + basic-stat
  blocks, the clan-hall agent lookup, the patrol-route row, and the five `%ai*%`
  rows (intention, AI, AIType, clan & range, ignore & range) that Java emits only
  for an NPC with an AI. The spawn *name*/*group*/*AI* labels resolve through
  `Npc.spawn_ref`, guarded by an npc-id match so a runtime spawn's placeholder
  `(0, 0, 0)` reference cannot report an unrelated spawn line; `%spawnfile%`
  stays Java's `--` (the loader keeps no per-template source path). 2 tests
  (GM gets the admin window with **every** placeholder substituted and no
  attack/interact intent; a non-GM with `AltGameViewNpc` on still gets the
  player view), sabotage-verified.
- **…and its sub-pages** — ✅ **LANDED 2026-08-01**, on the report that "there
  are problems with sub pages". Opening the window exposed that the whole
  `NpcViewMod` surface behind it was partial; a Java-line-by-line audit found
  **five** defects, each its own bug with its own test:
  1. **`Skills` / `AggroList` buttons were dead** — the two verbs were unrouted.
     `sendNpcSkillView` (icon/name/id/level per template skill, `Skills.htm`)
     and `sendAggroListView` (name/hate/damage per aggro entry, a vanished
     attacker rendering as Java's literal `NULL`, `AggroList.htm`) are ported.
     The skill rows needed `Skill.getIcon()`, so the skill parser now keeps
     `<icon>` (`Skill.icon`, Java's `icon.skill0000` default) — the generic
     per-level value reader already collected it, only the field was missing.
  2. **The drop page went out on the wrong channel.** Java sends it through
     `Util.sendCBHtml` — a chunked **community-board** page, which is what
     `DropList.htm`'s two-column 332px layout and the 16000-char row budget are
     built for; the port sent an `NpcHtmlMessage`, i.e. an NPC dialog with a far
     smaller ceiling. Now `community_board::send_cb_html`, as in Java.
  3. **Every drop row drew the question-mark icon** — the placeholder was
     hard-coded instead of `ItemData::icon` (which already applies exactly
     Java's `item.getIcon() == null` fallback).
  4. **Chances were truncated to 2 decimals** (`{:.2}`) where Java uses
     `DecimalFormat("0.00##")`: 2–4 decimals, trailing zeros trimmed no further
     than the 2nd. A 0.0123% drop read `0.01%`.
  5. **The `Quests` button opened the wrong page.** `//show_quests` and
     `//charquestmenu` are two different Java handlers — `AdminQuest`'s NPC
     script listing (`npc-quests.htm`) and `AdminShowQuests`' *player*
     quest-state editor — and were aliased to the latter, so the button
     answered `INVALID_TARGET` on an NPC. Split, with a new
     `QuestRegistry::scripts_for_npc` (Java walks the NPC's event listeners and
     dedups through a `TreeSet`; the port asks the compiled-in registry the same
     question one indirection earlier).
  Plus the `Buffs` button: `//getbuffs` resolved through `target_player`
  (player target, else self), so a GM with a mob selected saw their **own**
  buffs; Java's gate is `isCreature()`. It now follows an NPC target and takes
  Java's `<playername>` argument. Also corrected: Info/Skills/AggroList use
  Java's *no-arg* `new NpcHtmlMessage()`, i.e. npcObjId `0`, not the NPC's
  object id. 6 tests, every one sabotage-verified. Remaining gap, marked
  `TODO(G33)`: Java paginates the buff window 3-per-page via `PageBuilder`;
  the port renders one page and leaves `%pages%` empty.
- **Deferred**: nothing geo-related. Still blocked: clan-skill grants (no
  clan-skill system), `AdminFence` (no spawnable fence), the AdminEffects
  **abnormal-visual-effect / team / targetable** subset, `//setnoble`/`//rec`/
  premium/prime/pc-cafe (fields not modelled), and the IP/dualbox tools (no
  per-client IP). **G13.C** (sieges/olympiad/instances/events/petitions/
  punishment/…) stays gated-but-bodiless.

---

### Post-G33 — Shadow Weapon Exchange Coupons + shadow-item mana ✅ (2026-08-01)

Plan: [PLAN_SHADOW_WEAPONS.md](PLAN_SHADOW_WEAPONS.md). Reported as "I can't use
shadow coupons" — every class transfer paid out 15 Shadow Item Exchange Coupons
(8869/8870) and nothing in the port ever took one back.

- **The exchange desk was missing from the dist, not just the port.** This dist
  ships no `custom/ShadowWeapons/` script, none of the three exchange multisells
  (306893001/2/3), and has the `<Button … _Quest ShadowWeapons>` line
  **commented out** in all 81 `html/villagemaster/*.htm` — the Java reference
  server dead-ends the same way. Restored from the authentic Interlude datapack
  (`L2J_Mobius_CT_0_Interlude`): the three multisells, the four `exchange_*.html`
  pages, and `scripts/shadow_weapons.rs` (Java's `onTalk` — coupons held pick
  one of four pages, each carrying its multisell link, 1 coupon → 1 weapon).
  Every id was re-validated against *this* dist first (19 products, 2
  ingredients, 80 npcs all resolve). The button is uncommented for the 78
  masters on both the script's list and this dist's htmls; the three whose
  master is in no multisell allow-list (30508/30594/31279) stay commented rather
  than become buttons whose exchange link refuses.
- **Shadow-item mana was entirely unported**, which would have made the coupon
  pay out a *permanent* free weapon. `<set name="duration">` is now parsed into
  `ItemTemplate.duration`, `Inventory::add_item` stamps it into `mana_left`
  (Java: `_mana = _itemTemplate.getDuration()`, and `isShadowItem()` is just
  `mana >= 0`), and `game_loop/item_mana.rs` is `Item.decreaseMana` +
  `ItemManaTaskManager`: one point per minute **while worn**, warnings at
  10/5/1, unequip + destroy at 0, consumed at all three Java sites (equip, the
  60 s beat, the `EnterWorld` sweep). `mana_left` was a persisted column nothing
  ever wrote — the same stubbed-field shape as the old `curCp`.
- **`Inventory::insert_instance` gained a `mana` parameter.** It rebuilds the
  instance rather than moving Java's `Item` object, so a transfer had to carry
  mana explicitly: re-deriving it from the template would have **refilled** a
  worn shadow weapon on every private-warehouse round trip (the one container
  that accepts a bound item). Trade/store/mail pass `-1` under a comment — all
  demand tradability, which no shadow item has.
- **Two exploit guards became reachable only once mana was real** and are now
  enforced: `RequestCrystallizeItem`'s `isShadowItem()` refusal and
  `AbstractRefinePacket.isValid`'s.
- **Upstream quirk kept deliberately:** the task manager passes
  `decreaseMana(item.isEquipped())`, so an item taken off before its beat lands
  still spends the point but never re-arms — and since nothing clears
  `_consumingMana` again, it stops draining for the rest of its life.
  Reproduced and documented at the site.
- 8 tests against the real dist catalog + multisell loader; the `duration` stamp
  and the crystallize guard sabotage-verified. One test was **rewritten after
  passing vacuously**: shadow weapons declare no `crystal_count`, so the
  crystallize case was already refused by an unrelated branch — the subject is
  now a crystallizable Bastard Sword with a `duration` stamped on.

#### Follow-up — the mana clock burned points Java never charges (2026-08-02)

Reported as "equipping a shadow item takes mana unconditionally, so
equip/unequip cycles wear it out early". The per-equip point is genuine Java
(`Player.useEquipableItem` calls `decreaseMana(false)` inside its
`if (item.isEquipped())` branch, and nothing at all on the unequip side), and
that is now pinned by its own test. Two things around it were **not** Java:

- **The burn hung off `finish_equip_change`, the shared paperdoll-change tail**,
  and fired for every still-worn item it was handed. That helper stands in for
  much more than the equip click — an enchant refreshing a worn item's glow, an
  augment re-applying its options, `//mount` stripping a weapon — so a shadow
  weapon lost mana to events Java never charges for. Moved to the
  `use_equipable_item` equip branch, for the clicked item alone, which is
  exactly where Java's single call site sits.
- **`_consumingMana` outlived the session.** Java's flag is a field on the
  `Item`, thrown away at logout, so the next `EnterWorld` sweep re-arms the 60 s
  beat; ours is a `World` map keyed by an object id that the next login reads
  straight back out of the `items` table. A logout taken mid-beat therefore left
  the flag set for good and the weapon **never ticked again** — after that it
  only lost the one point per equip, which is what made the equip charge look
  like the whole story. Cleared in `store_and_remove_player` now. The map also
  gained the tick each beat is *due*, so the beat the old session left in flight
  is dropped instead of racing the new one (Java drops it for free: the orphaned
  entry's `Item` has no acting player). Without that guard the relog would have
  doubled the drain rather than stopped it.
- 3 tests, each sabotage-verified against the bug it guards: one point per
  equip and none per removal, a paperdoll refresh spends nothing, and a relog
  inside the beat window leaves exactly one beat running.

### Post-G33 — quest items duplicated in the inventory until relog ✅ (2026-08-02)

Reported against Q00217 *Testimony of Trust*: the quest tab showed several
"Scroll of Elf Trust" / "Basilisk Plasma" rows for items handed out once, and a
relog collapsed them back to one. Server state was never wrong — the inventory
merges stacks correctly and the DB held a single row, which is exactly why the
relog "fixed" it. Both causes were in the two packets a quest gain sends.

- **`ExQuestItemList` was fired bare on every quest `giveItems`/`takeItems`.**
  Java sends that packet from exactly two places — `EnterWorld` and
  `Player.sendItemList`, which always puts a full `ItemList` in front of it —
  so the client treats it as a list to append to the inventory it was just
  handed, not as a standalone refresh. Sent alone it re-appends the whole quest
  tab, one visible duplicate row per gain, until the next `ItemList` rebuilds
  the window. Java's quest paths (`PlayerInventory.addItem`,
  `destroyItemByItemId`) refresh the client through `InventoryUpdate` alone;
  both bare sends are gone.
- **Every `InventoryUpdate` entry claimed change type 2 (modify)**, including
  brand-new stacks. Java picks per entry in `PlayerInventory.addItem`:
  `isStackable() && getCount() > count` → `addModifiedItem` (2), else
  `addNewItem` (1) — a modify names an object id the client has no slot for.
  New `items::add_inventory_item_tracked` reports new-vs-merged per object id
  and `enter_world::inventory_update_added` writes the matching type; the plain
  `inventory_update` keeps its hard-coded 2, which is right for its
  equip/unequip callers (the instance already exists client-side). Fixing this
  is what lets the `ExQuestItemList` crutch be removed safely.
- The quest script itself was faithful: Q00217's Guardian Basilisk arm gives 1
  Blood per kill and swaps 5 Blood → 1 Plasma behind `!hasQuestItems(PLASMA)`,
  matching Java line for line. Blood appearing "alongside" the Plasma was the
  same stale client rows — `takeItems`' change-type-3 entry retires one row, not
  the phantom copies of it.
- 4 assertions on the existing Q00258 collect/turn-in test, each
  sabotage-verified: change type 1 on the first pelt, 2 on a merge, and no bare
  `ExQuestItemList` on either the gain or the take.

### Post-G33 — the Quest Items tab reported "N/0" ✅ (2026-08-02)

The inventory's Quest Items tab showed the slot counter as `5/0`: the right
count, no capacity. The value the client wants is `Character.ini`'s
`MaximumSlotsForQuestItems` (100), and `ExStorageMaxCount` was already carrying
it — **in the wrong field**.

- **`_inventoryExtraSlots` must be written *after* `_inventoryQuestItems`.**
  Stock L2J Mobius writes the belt bonus first, so the protocol-110 client reads
  the quest-tab capacity out of the belt field. That field is 0 for every
  character in this chronicle (no belt items exist, and the only
  `Stat::InventoryNormal` contributor is a skill almost nobody has), so the
  quest limit was invisible and the real 100 landed in a field the client
  ignores. This is **an upstream bug, not a port bug** — the port was a faithful
  copy. Both fields are swapped here and in the Java reference tree, which is
  the ground truth this port follows; the two trailing `40`s and the packet
  length are unchanged. Ordinary inventory capacity was never affected: it is
  field 0, and `UserInfo`'s INVENTORY_LIMIT block carries it too.
- **The extra-slots field now carries a real number** instead of a hard-coded 0
  — Java's `getStat().getValue(Stat.INVENTORY_NORMAL, 0)`, i.e. the bonus
  *alone* (field 0 already includes it in the total). Only `EnlargeSlot`'s
  Expand Inventory (1372) feeds it on this dist.
- **Every reported and enforced bag size now comes from one helper**,
  `CharacterConfig::inventory_limit_for(race, is_gm)`, so raising a
  `MaximumSlotsFor…` key in `Character.ini` moves all of them together. This
  closed a real gap: `ExStorageMaxCount` and `UserInfo` both skipped Java's
  `isGM()` branch and told a GM the plain race base, while `weight::
  inventory_limit` (the enforcing side) honoured `MaximumSlotsForGMPlayer` —
  the report and the enforcement disagreed for GMs.
- **`RequestAcquireSkill` now resends the packet for skills 1368-1372**, Java's
  "if skill is expand type then sends packet" — the `EnlargeSlot` passives
  (Expand Dwarven Craft / Common Craft / Trade / Warehouse / Inventory). Without
  it the client kept the capacity it cached at login and the bought slots only
  appeared after a relog. (Java's other resend sites — subclass change,
  `ClassChange`, and the equip path's `getInventoryLimit() != oldInvLimit` —
  are `TODO(G34+)`: nothing on this dist changes those limits by those routes.)
- `skills_tests::ex_storage_max_count_reports_the_configured_capacities` decodes
  all 12 ints and pins the quest limit to field 8, the bonus to field 9, the GM
  bag, and the Expand-Inventory total. Sabotage-verified: restoring the upstream
  field order fails it with `left: 0, right: 100` — the exact symptom.

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
  ~~onAttack/onSpawn~~ ✅ G12); ~~tutorial (Q00255)~~ (✅ 2026-07-28);
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
  commands (item links in chat are done); `GlobalChat`/`TradeChat` OFF/GM modes;
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
  branch (memoState 5 → 6, second shot batch) landed with the tutorial port.
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
- **`InventoryUpdate` never travels alone (2026-07-31):** Java funnels *every*
  player-facing inventory change through `Player.sendInventoryUpdate`, which
  sends `InventoryUpdate` + `ExAdenaInvenCount` (0x13E) + `ExUserInfoInvenWeight`
  (0x166) — that trio is what drives the client's horizontal **status bar**
  (adena / inventory-slot counter / weight). `PlayerInventory.addItem` and
  `Player.destroyItem` call it internally, so the invariant holds even at call
  sites that look like they only touch the container; `sendItemList` sends the
  same footers. The Rust port had a `helpers::send_inventory_update` funnel but
  ~25 sites still sent the bare `InventoryUpdate`, so the status bar went stale
  on auto-looted mob drops, pickups, drops, shop/multisell trades, crafting,
  shot consumption, pet transfers and more until the next relog. All converted
  to the funnel; regression asserted in `combat_tests::melee_kill_rewards_and_decay`.
  **When adding any new inventory mutation, send through the helper, not
  `cs.send(inventory_update…)`.**
- **An NPC teleport is `decayMe()` first, and that releases every holder's
  target (2026-08-01):** Java's `Creature.teleToLocation` un-spawns the object
  before it moves it, and `World.removeVisibleObject` inside `decayMe()` walks
  the old 3×3 block clearing the target of every creature that held it
  (`setTarget(null)` → `TargetUnselected` for players) and sending each a
  `DeleteObject` — **unconditionally**, not only when the region index changes.
  The port's `death::relocate_npc` did neither: it sent `DeleteObject` only on a
  cross-region hop and never touched anyone's `TargetRef`. Reported symptom: drag
  a mob past its leash, it snaps back to spawn and the ground **selection ring
  stays behind** at the drag spot — this client keeps a deleted/moved object id
  locked as its selection until an explicit `TargetUnselected` arrives (the same
  failure family as the corpse ring at decay and the visibility-drop ring).
  `relocate_npc` now runs the `decayMe` order for real — `drop_target_notify`
  for every player holding the NPC, then the `DeleteObject`, then the move and
  `NpcInfo` — which fixes all four teleport callers at once (leash, attack
  timeout, Antharas' lair entry, Dr. Chaos). Regression:
  `mob_leash_tests::a_leashed_mob_clears_the_selection_ring_it_leaves_behind`,
  sabotage-verified. **Any new server-side relocation of a live object owes the
  client a `TargetUnselected` before its `DeleteObject`.**
- **The client's own paperdoll rides `ExUserInfoEquipSlot`, not `UserInfo`
  (2026-08-01):** `UserInfo` carries only the right-hand *enchant level*; the
  equipped item ids live in `ExUserInfoEquipSlot` (Ex 0x156). Java sends it from
  inside `Inventory.setPaperdollItem` — the choke point *every* paperdoll change
  goes through, including the implicit ones: `ItemContainer.removeItem` is
  overridden by `Inventory.removeItem` to unequip whatever it takes out of the
  bag, so dropping/destroying/transferring a worn item refreshes the paperdoll
  for free. The port's paperdoll is a plain data component that can't reach the
  client, so `items::finish_equip_change` sent the packet and four paths that
  bypass it did not: cursed-weapon drop-on-death, cursed-weapon `endOfLife` on a
  logged-in wielder, the stray-cursed-item sweep in `EnterWorld` (which runs
  *after* the enter-world snapshot) and the ordinary death item scatter
  (`onDieDropItem`). Reported symptom: die holding Akamanah → the sword is on
  the ground and the inventory window shows nothing equipped, but the character
  goes on rendering it. All four now call `items::refresh_equip_state` (stat
  recompute + `ExUserInfoEquipSlot` + `UserInfo`, extracted from
  `finish_equip_change`); regression in
  `cursed_weapon_tests::wielder_death_resends_the_paperdoll_snapshot`, verified
  by sabotage. Trade/warehouse/mail/private-store/shop/multisell are safe only
  because they refuse equipped items — **a new path that removes a worn item
  must call `refresh_equip_state`, and `ItemList` is not a substitute.**
- **A persisted column is only persisted once *both* ends carry it (2026-07-31):**
  `characters.curCp` was written by `store_player_tx` on every flush but never
  read back — `char_data_of` had no `cur_cp` field to map it into, and
  `Player::from_char` hard-coded `cur_cp: 0.0`. Every login therefore started at
  0 CP and visibly regenerated up, while the DB held the right value the whole
  time. The write side passing review is not evidence the round trip works:
  when adding a column to `PlayerSnapshot`, add the matching field to `CharData`
  + `char_data_of` + the `from_char` component it feeds, and assert the reload
  (`char_persistence::current_cp_persists`,
  `misc_tests::stored_cp_is_restored_on_login`). Java's `Player.restore` reads
  `curCp`/`curHp`/`curMp` together and replays them through the `setCurrentX`
  clamps after stats are recomputed — the port mirrors that clamp order.
- **Panic policy (2026-07-24):** a panic in a packet handler is caught per-packet
  in `drain_network` (`catch_unwind`, Java-parity with `ExecuteThread`'s
  catch-Throwable) — the offending client is disconnected (their mid-mutation
  session state is suspect; they relog clean), the server lives on. A
  panic that still kills the game thread no longer leaves a zombie process:
  `main` selects on the game-thread join alongside the shutdown signal and exits
  nonzero, so systemd's `Restart=on-failure` restarts the server (previously the
  listener stayed up with a dead game loop and nothing restarted). Trigger was
  `//spawn` with no args indexing `args[0]` (also fixed + regression test).
- **Teleport completion depends on a client packet — `TeleportWatchdogTimeout`
  is the escape hatch (2026-08-01):** `Creature.teleToLocation` deliberately
  stops half-way: `setTeleporting(true)` → `decayMe()` → `TeleportToLocation` →
  `ExTeleportToLocationActivate`, and `spawnMe` only happens when the *client*
  answers with `Appearing` (0x30). A client that never answers — hung zone load,
  dropped packet, crash on the loading screen — leaves the character decayed out
  of the world: invisible to everyone, `ValidatePosition` ignored, fixable only
  by relogging. Java's escape hatch is `TeleportWatchdogTask`, armed by
  `Player.setTeleporting(true)` when `TELEPORT_WATCHDOG_TIMEOUT > 0` and
  cancelled by `setTeleporting(false)` / `stopAllTasks()`; on expiry it calls
  `onTeleported()` server-side. Ported as `world.teleport_watchdog_due`
  (oid → due tick, swept once a second by `death::teleport_watchdog_tick`) —
  a **map rather than a `scheduler` entry precisely because it must be
  cancellable**; the `Scheduler` has no cancel, and a stale entry firing into a
  *later* teleport would spawn a character in before their client had loaded.
  The `Appearing` path and the watchdog now share one `death::on_teleported`.
  **Ships off (`0`), Java's default and this dist's** — the Rust dist ini had
  drifted to `10`, which is both a divergence from the authoritative Java dist
  and below the ~60 s the ini itself recommends, so it was reset to `0`.
  Regressions: `movement_tests::teleport_watchdog_{off_by_default,
  forces_completion_when_appearing_never_arrives}` +
  `appearing_cancels_the_watchdog_so_the_next_teleport_arms_fresh`, all verified
  by sabotage (drop the arm → two fail; drop the cancel → the third fails).
  Known gap, same as Java's: a player with no live session (logout raced the
  sweep) is skipped rather than spawned, since the visibility exchange needs a
  client to send to.

- **Shift-clicked item links in chat (2026-08-03):** linking an item into a
  chat line produced a link the reader could see but not open — clicking the
  "?" showed nothing. Two halves were missing. (1) The client answers a click
  by sending `RequestExRqItemLink` (**ex `0x1E`**, body = the item's object id)
  and expects `ExRpItemLink` (**`0xFE:0x6D`**, body = one `AbstractItemPacket.
  writeItem` row) back; ex `0x1E` was not in the dispatch table at all, so the
  request fell through unhandled and the client, having nothing to render, left
  the link as a bare "?". (2) `Say2.parseAndPublishItem` was unported: Java
  walks each `\x08 … ID=<objectId> … \x08` span, verifies the speaker actually
  owns that inventory item, and calls `item.publish()` — and `isPublished()` is
  the *only* gate `RequestExRqItemLink` checks, so without it every answer
  would either be refused or (worse, if ungated) let a client read any
  inventory by guessing object ids. A line linking an item the speaker does not
  own is dropped whole, as in Java. The publish flag lives world-side as
  `World.published_items` (item oid → publisher oid) rather than on
  `ItemInstance`, because it is session state, not saved item state; the
  publisher id lets `chat::on_player_leave_world` drop those entries at logout,
  matching Java's flag dying with the `Item` instance. Lookup at click time
  scans loaded inventories (Java's `World.findObject` narrowed to items), so the
  reader sees the item's *current* enchant/count and the link survives a trade.
  Also fixed alongside: the chat length cap ignored Java's item-link branch —
  Java allows 500 chars when the text contains `\x08` (105 otherwise) and
  exempts GMs entirely, so a longer linked line was being swallowed as spam.
  Regressions: `item_link_tests::{a_shift_clicked_item_can_be_inspected_by_the_
  reader, an_unpublished_item_is_never_answered, linking_an_item_you_do_not_own_
  drops_the_line, an_item_link_raises_the_chat_length_cap, logging_out_kills_
  the_publishers_links}`; the round trip was verified by sabotage (drop the
  dispatch arm → it fails, the other four still pass).
