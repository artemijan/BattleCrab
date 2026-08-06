# Recorded deferrals — the `TODO(<tag>)` inventory

Every milestone row in [PROGRESS.md](PROGRESS.md) is ✅ or an explicit
scope-out. That is true, and it is also **not the whole picture**: a milestone
is marked complete when its *gate* is met, and each one shipped with a handful
of narrow behaviours deferred and marked at the site. There are **10** such
markers — the sum of the inventory below, and of the expected list the
`deferral_markers_match_the_recorded_inventory` test holds the code to. A reader
looking only at the status table cannot see them.

Because every milestone row is now ✅, this residue **is** the remaining work on
the port. The list below is the backlog, not a footnote to it.

This file is that missing half — generated from the code, not written by hand,
so it cannot drift into fiction the way prose does.

## Why this exists

Two documentation bugs found on 2026-08-03 prompted it, both the same shape:

- G33's plan listed "Slice 3 — periodic autosave" as pending work that had in
  fact been complete and tested for milestones.
- The `Custom/*.ini` row claimed "the 16 unported operator features" when all
  17 were ported, consumed and tested. Two separate documents had drifted to
  the same wrong claim, and one of them had been *re-introduced* from memory.

Both rotted in the safe direction — the code was ahead of the notes. A sweep of
the five `TODO(G<N>)` markers PROGRESS claimed but the code did not contain
found the same: all five were done (the manual-learn `<item>` cost, the
HATE-effect aggro skip, academy-graduation rewards, sub-unit tabs, ally
crests). Prose about what is left is the least reliable artefact in the repo.
Markers in the code are the reliable one, so they are what this file counts.

## What a marker means

Per `CLAUDE.md`: when a port intentionally skips part of the Java behaviour,
leave a `TODO(G<N>): …` at the exact site naming the Java source of the skipped
behaviour. A marker is therefore **a deliberate, recorded gap** — not a bug and
not a reminder to tidy. `TODO(G24)` on a closed G24 means "G24 shipped its gate
and left this specific thing", which is the intended end state, not a
contradiction.

The count is asserted by `deferral_markers_match_the_recorded_inventory` in
`crates/gameserver/src/data/skill_data/coverage_census.rs`. It moves only when
someone changes it deliberately: adding a gap without recording it fails, and
closing one without updating the number fails too — the same two-way discipline
G34's close-out gate uses.

Markers come in two families. **Milestone tags** (`G<N>`) hang off a shipped
milestone's gate. **Topic tags** (lowercase — `pets`, `manor`, `radar`, …) are
gaps that never had a milestone to hang off; they are deferrals just the same.
A tag must parse as alphanumerics plus `. - _ + ? /` — no spaces, no `<`, no
quotes — or the scanner cannot see it. That is also why prose in a `.rs` file
must never spell a *parseable* tag: it would be counted as a marker.

### `TODO` versus `SKIP`

A `TODO(<tag>)` is **work that is missing**. A `SKIP(<tag>)` is **work that was
examined and deliberately not done**, where the reason is a property of the
datapack rather than of the port — Java code that no route on this dist can
reach, content from a later chronicle, a branch whose other end does not exist.
`SKIP` is not counted by the inventory, because counting it would inflate the
backlog with entries nobody should ever action.

The distinction is load-bearing in one direction only: **downgrading a real gap
to `SKIP` hides it**, and the inventory will not catch that. So a `SKIP` must
carry the evidence in the comment — which page fails to offer the button, which
NPC id is registered nowhere — not merely the assertion. `academy.rs`'s
squad-skills note is the model: it names the item ids, the clan level and the
chronicle marker that put the content out of reach.

When a `SKIP` marks dead Java that was nonetheless ported verbatim, say so and
say what would have to change to revive it. Half-restoring a two-ended dead
route is how you get a trap: quest 415's alternate ending consumes Rosheek's
letter and hands out no recommendation, so wiring up its entry page without
also registering its NPCs would strand the player.

## Inventory

| marker | count | files |
|---|---:|---|
| `TODO(G15.5)` | 1 | `game_loop/options.rs` |
| `TODO(G17)` | 1 | `game_loop/subclass.rs` |
| `TODO(G22)` | 1 | `scripts/primeval_isle.rs` |
| `TODO(G24/G26)` | 1 | `scripts/castle_chamberlain.rs` |
| `TODO(G25)` | 1 | `game_loop/olympiad.rs` |
| `TODO(G27)` | 1 | `game_loop/duel.rs` |
| `TODO(G30)` | 1 | `game_loop/community_board.rs` |
| `TODO(G33)` | 1 | `game_loop/offline_trade.rs` |
| `TODO(G35)` | 1 | `crates/commons/src/audit.rs` |
| `TODO(login-playauth)` | 1 | `crates/gameserver/tests/e2e_create.rs` |

## Closed

Markers retired, newest first. A row here is a marker that left the code *and*
the inventory in the same commit — the two-way discipline in both directions.

| date | marker | what closed it |
|---|---|---|
| 2026-08-06 | `TODO(G7.5)` | **Closed — implemented.** `MagicalAttackRange` now rolls `calcShldUse` for the target before the damage calc: a block adds `shieldDef × shieldDefPercent / 100` to m.def, a perfect block caps the hit at 1, and a player target hears SHIELD_DEFENSE_SUCCEEDED — Java's exact three-way ladder. Tested front-vs-back arc, block, and perfect block. |
| 2026-08-06 | `TODO(G13+)` | **Closed — implemented.** `QuestCtx::retarget_random_party_member` ports `getRandomPartyMemberState`: candidates are the killer's party members whose quest state qualifies (any-STARTED or an exact cond), the killer weighted `playerChance`×, the pick range-gated 3D by `AltPartyRange` (the solo arm range-checks too, as Java does); on success the quest context re-targets to the picked member. Wired into Q303 (`(1, 1)`) and Q416 (`(-1, 3)`), the two quests whose Java uses it. Tested: a questless killer's kill credits an in-range started party mate; an out-of-range mate gets nothing. |
| 2026-08-06 | Singleton pass: 19 → 12 | Seven closed; every survivor is a deliberate, precisely-worded feature deferral. |
| 2026-08-06 | `TODO(G?)` (NPC conditioned passives) | **Closed.** Weapon-conditioned template passives (4415 sword mastery, …) now evaluate against the template's `<equipment>` right hand, the NPC counterpart of the paperdoll check; armor-conditioned ones stay skipped because Java's `<using kind>` evaluates false for an NPC too. Tested: the mastery applies armed, contributes nothing bare-handed. |
| 2026-08-06 | `TODO(G14)`, `TODO(G32)`, `TODO(G-later)`, `TODO(G9+)`, `TODO(G7)` — all stale | Item maxHp/maxMp bonuses were already folded (in `calc_max_hp`/`calc_max_mp` themselves — the finalizer comment predated them); fishing's zone gate existed at both entry points; the UI keymap store/load round-trip was complete (`settings.rs` + the lobby replay) under a doc saying "null for now"; the effect-registry growth note described a process G9–G34 finished; and the player-template "full stat calc" note predated the finalizers by ~25 milestones. |
| 2026-08-06 | `TODO(G24.5)` → `SKIP(census)` | The busy-dock messages guard a branch this dist can't reach (one boat per route); recorded with its revive condition. |
| 2026-08-06 | `TODO(G25)` reworded | The stadium-instancing blocker (G27) is gone — arena templates + instance engine are ported; the marker now states the remaining work (per-match instancing) and its live symptom (concurrent matches share coordinates). |
| 2026-08-06 | The ten 2-marker groups: 20 → 3 | G15, G18, G18.6, G21+, G23, G26.5, G29+ closed whole; G17, G7.5, login-playauth each consolidated to one precise survivor. |
| 2026-08-06 | `TODO(G15)` ×2 | **Closed.** The sweep now runs `checkInventorySlotsAndWeight` before claiming loot (slots per Java's stackable rule, weight summed per *line* — Java's own quirk — refused with SM 1036's sibling YOUR_INVENTORY_IS_FULL, loot staying on the corpse); and a timed item skill that loses the race against a running cast queues as `QueuedAction::UseItem` and replays when the cast ends — the port's `_queuedSkill`. |
| 2026-08-06 | `TODO(G18)` ×2 | **One stale, one real.** `check_if_pvp`'s doc still said clan wars "need systems this port lacks" directly above the line implementing the MUTUAL-war leg (the academy exemption is now exact too). The real half: `onDieDropItem`'s first gate — a clean victim killed by a clan-war enemy drops nothing — is ported and tested. |
| 2026-08-06 | `TODO(G18.6)` ×2 | **Closed.** The apprentice/sponsor pair now lives on `ClanMember` (loaded with the clan, kept in step by `set_mentorship`), so an offline member's academy pane shows their partner; and a departing sub-pledge leader vacates their unit's slot (`getLeaderSubPledge` leg), persisted like a leader reassignment. |
| 2026-08-06 | `TODO(G23)` ×2 | **Both stale.** Valakas's entry cinematic had been wired (`valakas::"beginning"`, ten camera beats) since his own slice; and `Chat 0` on a first-talk NPC now routes through `notify_first_talk` — the hook existed, only the bypass arm hadn't been told. |
| 2026-08-06 | `TODO(G26.5)` ×2 | **Closed.** The Monster Race tick carries Java's full reminder cadence (the 30 s drumbeat, the 10/5/1-minute closing warnings, the pre-race countdown ladder); the lottery's draw date formats through `commons::util::format_date`, which had existed all along — "DateFormat isn't wired" was stale. |
| 2026-08-06 | `TODO(G29+)` ×2 | **One real, one refactor-note.** The reported capacities are now enforced: warehouse deposits refuse over the active container's cap (private = `Stat::StoragePrivate` over the dwarf/human base; clan/freight their constants) and the sell store refuses more lines than `getPrivateSellStoreLimit` (new `MaxPvtStoreSellSlots*` config + `Stat::TradeSell`), both with SM 1036 — the buy side already had its cap. The `ChargedShots` fold-in was a refactor wish, not a gap: recorded as a deliberate non-refactor. |
| 2026-08-06 | `TODO(G21+)` ×2 → prose | Both were argued deviations wearing marker tags: GetAgro's one-think-tick faction pre-seed gap and quest 414's skipped spawn animation. Each keeps its revisit sentence without a countable tag. |
| 2026-08-06 | `TODO(G7.5)` ×2 → 1 | `MagicalSoulAttack`'s soul scaling closes as SKIP(census) — all 15 carriers are Kamael skills with no route, same evidence as its physical twin. The survivor is real and precise: `shieldDefPercent` needs a shield roll in the magic-damage path (Java's `MagicalAttackRange` does call `calcShldUse`). |
| 2026-08-06 | `TODO(G17)` ×2 → 1, `TODO(login-playauth)` ×2 → 1 | Consolidations: the subclass deferral bundle (hennas/shortcuts/certification/cancel-replace/lock) now lives in one header marker its site refers to; the ignored e2e test's duplicate tag in its `#[ignore]` string is de-parseabled, leaving the investigation note itself. |
| 2026-08-06 | `TODO(G-pvp)` ×3 — the whole family (one feature) | **Closed.** All three marked the same gap: Blessing of Protection's (5182) PK immunity. `pvp::protection_blessing_blocks` ports `PlayableAI`'s pair exactly — a chaotic character 10+ levels above a blessed newbie is refused (INCORRECT_TARGET + ActionFailed), the shield is symmetric, both ends resolve through `acting_player`, the blessing is the `PK_PROTECT` abnormal, and a PVP zone on the target suspends it. Wired into both intention paths: `start_attack_intent` (every attack entry point) and `use_magic_on`'s bad-skill-at-playable branch. |
| 2026-08-06 | `TODO(G33)` ×2 of 3 | **Closed.** `broadcastCharInfo` now matches Java's shape: the `CharInfo` half of `broadcastUserInfo` schedules a 50 ms `BroadcastCharInfo` task and coalesces every call in the window (`Player.char_info_pending`), so onlookers get one packet per burst, after the actor swap — the one divergence the hero-glow field investigation had left open. And the offline-trader visibility claim was stale: the aggro scans became region-index-driven (`players_visible_from`, whose plain form deliberately yields unattended shops) some sessions ago. The survivor is precise: `AltGameCreation = True`'s staged multi-pass craft machinery, whose absence makes the offline-craft branch unreachable. |
| 2026-08-06 | `TODO(G29)` ×4 — the whole group | `storePetFood` is real: the ride path records the pet's collar object id (`Player.mount_collar_object_id`, Java's `_controlItemId`) and the dismount writes the drained gauge back onto the `PlayerPets` row. Chasing it exposed a **bug family beside the marker**: four unsummon sites (the ride, the collar recall, `//unsummon`, the unsummon skill effect) skipped the `sync_pet_row` capture every other site runs, silently dropping a pet's hp/exp/fed deltas — all four fixed. The Wyvern Breath leg was dead in Java too (`setMount` never grants 4289, so its removal removes nothing). The other three markers were prose: two test headers describing already-closed deferrals spelled parseable tags, and the karma-drop note's "revisit on capture" keeps its sentence without one. |
| 2026-08-06 | `TODO(G28)` ×4 — the whole group (all TvT) | Three of four were stale in place: the logout-forfeit listener was fully implemented (`on_player_logout` + `manage_forfeit`, wired at both disconnect paths in `net.rs`) with the marker still sitting on the registration branch; `canRegister`'s "the rest are TODO at the site" annotated a function that had since ported every Java gate; and the module header still claimed the freeze had "no such flag" after `Immobilized`+`SkillsDisabled` landed. The one real gap is done: the arena stand-up now leaves each participant's old party (`Player.leaveParty` = DISCONNECTED) and regroups each team into parties of 7 under FINDERS_KEEPERS with a per-team command channel (`group_team`), exercising the ported party/CC primitives the marker said didn't exist. |
| 2026-08-06 | `TODO(G27)` ×3 of 4 — instances group | **`ExInzoneWaiting`'s empty list turned out to be exact parity**: the only `<reenter>` template on this dist (LastImperialTomb 136) has no `apply` attribute, which Java parses as `InstanceReenterType.NONE` — `setReenterTime` never fires anywhere, so Java's own penalty list is permanently empty too. `//instancedestroy` now warns everyone inside with the "destroyed by Game Master" banner before the teleport-out (its test also documents the command's `confirmDlg` flow). The two party-duel markers consolidate into one precise TODO at the duel module header: the instances blocker is gone (engine + all four olympiad arena templates ported), what remains is the party flow itself. |
| 2026-08-06 | `TODO(G20)` ×5 — the whole group | Independent dual rolls, the duel snapshot, the blow formula's real weapon crit; one SKIP and one stale reference. |
| 2026-08-06 | `TODO(G20)` ×1 (`combat/attack.rs`, dual second roll) | **Closed.** The swing is refactored into Java's `generateHit` shape: a dual weapon rolls the whole ladder twice (miss, shield, crit, damage), each hit halved, the soulshot consumed by the first non-missing hit with its boost riding the rest of the swing. The old test pinned the shared-roll deviation ("the two halves are equal") and now pins independence — one crit half, one plain. |
| 2026-08-06 | `TODO(G20)` ×1 (`duel.rs`, restore snapshot) | **Closed.** The duel stores both sides' HP/MP/CP at creation and restores that exact snapshot at the end, replacing the full heal. The other half of the marker dissolved on inspection: Java's duel-debuff removal list is dead code in this tree — `DuelManager.onBuff` has **no caller** — so nothing is ever registered or stripped there either. |
| 2026-08-06 | `TODO(G20)` ×1 (`formulas.rs`, blow weapon crit) | **Closed.** "Once weapon stats are exposed" — they were (`ItemStats.bonuses` carries the weapon's raw `rCrit`). `calc_blow_success` now receives Java's `weaponCritical`: the equipped weapon's raw crit stat, falling back to the caster template's `baseCritRate` bare-handed, instead of the finalized-rate ÷10 proxy. |
| 2026-08-06 | `TODO(G20)` ×1 → `SKIP(census)` (`skill_data/build.rs`, PhysicalSoulAttack souls) | The `1 + souls·0.04` boost has no reachable caster: all 30 carriers are Kamael skills with no tree row and no item grant, and the one NPC carrier (Twin Shot 507) would NPE in Java's own handler. Two stale sentences beside it also fixed — the blow skills it claimed "fall through until calcBlowDamage is ported" parse to `SkillEffect::Blow` over the ported formula. |
| 2026-08-06 | `TODO(G20)` ×1 (`skills/effects/mod.rs`, AttackTrait) | **Stale reference** — it deferred to "the TODO(G20) on its doc comment", which no longer exists: `AttackTrait`'s accumulator is read by `calc_attack_trait_bonus` on every swing and physical skill. |
| 2026-08-06 | `TODO(G30)` ×5 of 6 — the community-board group | The deferral list had rotted whole: the merchant actions it claimed needed "no multisell/buy-list system" were already implemented and dispatched two screens below, and the `maintainEnchantment` validation the packet doc deferred existed at `multisell.rs:235`. Real work found and done: `_bbsdelevel` (config-off, ported per the rule: funds → level-1 floor → charge → one-level drop with full top-up), the `_bbsteleport` 3 s skill lock (`SkillsDisabled` + a `SkillsReenable` schedule — the component G28's TvT freeze introduced), and the retail home's favorite counter over the `FavoriteBoard`'s own store. One precise marker remains: the retail forum boards, unreachable under this dist's `CustomCommunityBoard = True`. |
| 2026-08-06 | `TODO(G21)` ×7 — the whole group | The suicide bucket wired end to end, the faction-call script event's two listeners dispatched, and three stale-claim rewrites. |
| 2026-08-06 | `TODO(G21)` ×1 (`npc_ai_skills.rs`, isSuicideAttack) | **False absence claim.** "No skill in this dist declares it" — 18 do, with reachable NPC carriers (Four Sepulchers' Hall Keeper Suicidal Soldier 18333, Flame of the Branded 18299, …). The flag now parses into `Skill.is_suicide_attack`, classifies exclusively into the SUICIDE bucket (Java's ladder), and `try_cast` detonates below 30 % HP with its own `hasSkillChance()` roll, before the moving/mage gate — Java's placement. |
| 2026-08-06 | `TODO(G21)` ×2 (`npc_ai.rs`, faction-call events) | **Closed.** `EVT_AGGRESSION`'s Summon leg never applies (a faction recruit is always an `Attackable`), and the `OnAttackableFactionCall` script event has exactly two listeners on this dist — Queen Ant (`addFactionCallId(NURSE)`) and Orfen (`registerMobs`). `on_faction_call_script` dispatches them from both faction-call sites: the nurse's opportunistic Recovery at a hurt caller, Raikel Leos' 1-in-20 Blow at the attacker, Riba Iren's 9-in-10 (Orfen) / 1-in-10 heal at a half-dead caller. |
| 2026-08-06 | `TODO(G21)` ×3 (`npc_cast.rs` header + Other-targets + heal deviation) | **Stale prose.** The header still claimed heal/buff "resolve to the caster itself" (faction-scoped reconsideration had landed one screen below) and "no resurrect effect is ported" (one is; re-verified: 52 resurrection skills, zero in any NPC `<skillList>`). The `Other` target pass-through turned out load-bearing — `OWNER_PET` backs the tamed-beast buffs with the tamer passed explicitly — and is now documented per handler. The heal-scoping deviation keeps its revisit note without a marker: no work is missing there. |
| 2026-08-06 | `TODO(G24)` ×7 — the whole group | Castle crests modelled end to end, the mercenary-ticket pickup refusal, and the door-swing hit-time delay. |
| 2026-08-06 | `TODO(G24)` ×4 (crests: `siege.rs`, `admin/castle.rs` ×2 + header) | **Closed.** The display is inert on this dist (both gate halves ship false and Java never sets `showNpcCrest` true), but per the config-disabled-still-port rule the chain now exists: `spawn_npc_entity` records the tax-zone castle owner (`Npc.onSpawn`'s `setClanId` gate), `NpcInfo` grew its CLAN component (`visibility::npc_clan_block` — non-monster + peace zone, resolved positionally since NPCs never carry `ZoneFlags`), `Castle.show_npc_crest` loads from the DB column that already existed, `ShowCrestWithoutQuest` is parsed, and every ownership change resets the flag like `Castle.setOwner`. |
| 2026-08-06 | `TODO(G24)` ×1 (`target.rs`, mercenary tickets) | **Closed.** The hiring system stays unported, but the refusal doesn't need it: `data/residences/castles/*.xml` `<siegeGuards>` now loads as an itemId → castle table, and `ItemAction`'s gate refuses a ticket lying in a castle's siege zone (SM 654) to anyone but an owning-clan `CS_MERCENARIES` holder. Reachable because the mercenary manager's buylists sell tickets a player can drop. No siege-active check, as in Java. |
| 2026-08-06 | `TODO(G24)` ×1 (`combat/intent.rs`, door swing) | **Mostly stale** — the marker's own doc block had been superseded in place: the chase, auto-repeat and swing-period pacing all existed one paragraph below it. The one live remainder, the `timeToHit` damage delay, is now real: the door swing rides the shared `AttackHit` queued-hit (door branch in `handle_attack_hit`, re-checking the siege gate at fire time), so an abort mid-swing drops the hit. |
| 2026-08-06 | `TODO(G22)` ×7 of 8 — the Beast Farm / scripts group | Five implementations, one evidence-backed `SKIP`, one placeholder-data fix; only Primeval Isle's creature-see hook remains (its marker now names the one true blocker — the `<parameters>` half of its claim was stale, `ai_skill_params` has parsed those for milestones). |
| 2026-08-06 | `TODO(G22)` ×1 (`tamed_beast.rs`) | **Closed.** The 5 s `CheckOwnerBuffs` beat: "deferred until the beast's buff lists are wired" — `<skillList>` had been parsed into `NpcTemplate.skill_list` for milestones. When the tamer holds fewer than ⅔ of the beast's continuous non-debuff template skills, a random one is sit-cast on them, with Java's owner-offline despawn, distance/dead/casting skips, and its roll-before-count pick. |
| 2026-08-06 | `TODO(G22)` ×1 (`area_npcs.rs`) | **Stale premise.** "This port has no map-region table" — `world.data.map_region` had existed since the SHOUT chat path used it. The castle mass-teleport warning now reaches every player in the gatekeeper's map region (`MapRegionManager.getMapRegionLocId`), not just the broadcast radius. |
| 2026-08-06 | `TODO(G22)` ×2 (`q00230_test_of_the_summoner.rs`) | **Closed.** The duel-taunt NpcStringIds were invented placeholders (23001–23011) "pending the client NpcString table" — Java's `@ClientString` annotations carry the real ids (23060–23077 block); replaced. And `addSkillCastDesire(4126)` "had no cast-desire helper" — `ctx.npc_cast` was that helper; the trainer now casts Reduce Delay before each duel. |
| 2026-08-06 | `TODO(G22)` ×1 (`q00227_test_of_the_reformer.rs`) | **Closed.** Both staged duels ported: decoy Ol Mahum Pilgrim + attacker at Java's fixed spots, attacker's hate seeded at the decoy. The Krudel branch had also silently dropped Java's `getSummonedNpcCount() < 1` gate — restored. |
| 2026-08-06 | `TODO(G22)` ×1 (`sin_eater.rs`) | **Closed.** `onSummonTalk`: `interact_with_npc` now short-circuits an owner interacting with their own summon (`ServitorOf` owner match) — Java's `PetAction`/`SummonAction` never reach the NPC talk flow — and fires the Sin Eater's 10 % grumble (42239–42242), this dist's only `ON_PLAYER_SUMMON_TALK` listener. |
| 2026-08-06 | `TODO(G22)` ×1 → `SKIP(off-chronicle)` (`feedable_beasts.rs`) | "Java hooks quests 20 and 655 here" — it does not: both call sites are commented out in the reference (`// TODO: Q00020…`), and neither quest exists anywhere in this datapack. |
| 2026-08-06 | `TODO(G19)` ×9 + `TODO(G19+)` ×1 — the whole G19 group | See the rows below; three were implementations, four were stale or misworded claims converted to evidence-backed `SKIP`s/prose, and chasing the target-handler one exposed a live bug beside it. |
| 2026-08-06 | `TODO(G19)` ×1 (`skills/cast.rs`, NpcBody) | **Closed — live bug beside the marker.** The spoil/owner gate was applied to *every* `NPC_BODY` cast, but `OpSweeper` belongs to Sweeper 42 alone; the learnable corpse skills (Life Scavenge 46, Corpse Plague 103, Corpse Life Drain 1151, Corpse Burst 1155) were refused with SWEEPER_FAILED on any unspoiled corpse where Java casts them. The gate now keys off the `Sweeper` effect (same carrier set on this dist). |
| 2026-08-06 | `TODO(G19)` ×1 (`skill_enchant.rs`) | **Closed.** "The olympiad/sell-buff gates (neither system is modeled)" — both had landed (G25, Custom SellBuffs). `busy_for_enchant` now carries Java's transaction-only refusals (transform, attack stance, casting, boat, stores, sell-buff, olympiad), the private-store gate moved off the info packets (Java gates only the transaction), a sub-level change now drops the running cooldown (Java's reuse hash spans sub-level), and UNTRAIN turned out to be exact parity — Java's own `runImpl` switch has no UNTRAIN case. |
| 2026-08-06 | `TODO(G19)` ×2 (`admin/effects.rs`, `enter_world.rs`) | **Closed.** `//playmovie` now carries Java's full `MovieHolder` bookkeeping: the 95-row `Movie` table (id + escapable), the already-playing refusal, `abortAttack` + stop-move on start (a cast survives, "confirmed in retail"), `EndScenePlayer` (ex 0x58) and `RequestExEscapeScene` (ex 0x90) handled, `ExStopScenePlayer` (ex 0xE7) added. No consumer in Java gates on the movie state, so there was no further freeze to port. |
| 2026-08-06 | `TODO(G19)` ×1 → implementation + `SKIP(G19)` (`skills/effects/mod.rs`) | "`Decoy` and the default plain-spawn branch (no learnable carriers)" — **the reachability test was wrong**: `SummonNpc`'s plain-spawn carriers are item-cast (Holiday Trees 5560/5561 → skills 2137/2138, Squash/Watermelon Seeds → 2003/2004/2508/2509/9029-9032). The default branch is now ported (`spawn_plain_summon`: summoner link, template-name title, despawn schedule). The Decoy branch stays out as `SKIP`: no reachable skill summons a type=`Decoy` template — item 13769's "Life-size Decoy" 32544 is type `Folk`. |
| 2026-08-06 | `TODO(G19)` ×2 → `SKIP(G19)` + prose (`model/skill.rs`, BlockActions whitelist) | Censused: every `allowedSkills` carrier on this dist lists the same ten post-Interlude ids, and none of the ten has a skill-tree row, an item `<skills>` grant, or an NPC template carrying it. Nobody can know a whitelisted skill, so folding `CONDITIONAL_BLOCK_ACTIONS` into `BLOCK_ACTIONS` is observably exact. |
| 2026-08-06 | `TODO(G19)` ×1 → `SKIP(G19)` (`model/skill.rs`, MpConsumePerLevel) | Java's level-scaled branch needs a carrier with `abnormalTime`; all 19 carriers re-verified `abnormalTime`-less, so no cast can reach it. |
| 2026-08-06 | `TODO(G19+)` ×1 (`skill_data/build.rs`) | **Stale.** "`TargetMe` and `RandomizeHate` fall through unregistered" — both were registered in the very same match (G34 S4, build.rs:523 and :1139). The `power`-default guidance for `MagicalAttack` in the same comment block was the only live part and stays. |
| 2026-08-06 | `TODO(G19)` ×1 (`tests/mana_restore_tests.rs`) | **Scanner artefact** — prose describing a deferral *closed by that very slice* spelled a parseable tag. Reworded; the rule "never write a parseable tag in prose" bit again. |
| 2026-08-06 | `TODO(G34)` ×2 — olympiad gate + raid-minion predicate (`skills/effects/ticks.rs`, `effects/mod.rs`, `minions.rs`) | Both were "waiting on a subsystem that has since landed". The auto-resurrect (`ResurrectionSpecial.onExit`) now refuses inside an olympiad match — where firing would decide a duel to the death — via a new shared `olympiad::in_match`, distinct from the registered/observing composite `offline_trade` builds. And Confuse's raid immunity now covers minions: Java's `isRaidMinion()` is just `Monster.onSpawn`'s `setIsRaidMinion(_master.isRaid())`, and the port's `MinionOf` component already carried the link. |
| 2026-08-06 | `TODO(G24)` ×3 — castle crests, **justified not closed** (`siege.rs`, `admin/castle.rs` ×2) | "Castle crests" turns out to mean one display feature: `Npc.onSpawn` assigns the castle owner's clan id to NPCs in the **tax zone** so the client draws their crest, gated on `(SHOW_CREST_WITHOUT_QUEST \|\| castle.getShowNpcCrest()) && ownerId != 0`. **Both halves are false on this dist, permanently** — `ShowCrestWithoutQuest = False`, `showNpcCrest` defaults to `'false'` in both schemas, and `setShowNpcCrest(true)` appears *nowhere* in the Java tree. Kept as deferrals (an operator can flip the ini or the column), now naming what implementing needs: a `Castle` field, the tax-zone assignment, and a clan-id field `NpcInfo` does not carry. |
| 2026-08-06 | `TODO(G24)` ×1 — advanced HQ **implemented** (`combat/damage.rs`, `model/components.rs`, `siege.rs`) | Skill 326's camp now takes **half** damage. Deliberately not Java's arithmetic: `SiegeFlagStatus.reduceHp` omits an `else` and applies `value/2` *and* `value` for 1.5×, which makes the noble-only skill worse than the basic one. Operator decision, recorded in [CUSTOM_DIST_DEVIATIONS.md](CUSTOM_DIST_DEVIATIONS.md) — a future line-by-line parity pass will see a mismatch and must argue with the note rather than "correct" it. |
| 2026-08-06 | `TODO(G24)` ×1 — clan-hall lease broadcasts (`game_loop/clan_hall_auction.rs`) | Java broadcasts to the owning clan's online members twice: a daily reminder carrying the outstanding lease (1051), and the eviction notice (1052) sent **before** ownership is cleared. Neither was ported, so a clan hall simply disappeared with no explanation. Both wired through `clans::broadcast_to_clan`, keeping Java's order — tell them, then take the hall. |
| 2026-08-06 | `TODO(G24)` ×1 — advanced HQ, **closed** (operator chose to drop the bug) (`model/skill.rs`) | The marker said `isAdvanced` was "collapsed for now". Now it says what that costs. Skill 326 sets it, `autoGet="true"` in the noble tree, so it is live content; both skills plant the same NPC (35062); and the only difference is `SiegeFlagStatus.reduceHp`, which has **no `else` and no `return`** — an advanced HQ takes `value/2 + value`, i.e. **1.5× damage rather than half**. A Java bug, and porting it bug-for-bug makes a noble-only skill strictly worse than the basic one, so it wants an explicit decision rather than a quiet implementation. |
| 2026-08-05 | `TODO(G24)` ×2 — siege registration refusals (`game_loop/siege.rs`) | Five refusals were silent: the window simply did not change, so a player could not tell "the deadline passed" from "you are allied with the owner". Ported all of them plus the dissolution-grace guard. Two were not plain ids, which is why they lagged behind the other six — the deadline message takes a **castle-name parameter** (`sm.addCastleId`), and the NPC-castle refusal is a `sendMessage` free-text line in Java with no id at all, so it goes out as `S1_TEXT`. |
| 2026-08-05 | `TODO(G24)` ×1 — Q234 chest despawn (`scripts/q00234_fates_whisper.rs`) | **Stale.** It said "a general timed-despawn for world NPCs is not modelled" — `QuestCtx::schedule_despawn` exists and quest 421 already uses it, and `spawn_near_npc` was already returning the oid needed to address the spawn. Two lines. Java's `addSpawn(…, true, 120000)` puts the chest on a two-minute fuse, so an uncollected drop now clears itself instead of standing until restart. |
| 2026-08-05 | `TODO(G24)` ×1 — castle circlets (`game_loop/clans/membership.rs`, `castle.rs`, `siege.rs`, `admin/castle.rs`) | `CastleManager.removeCirclet` ported with its id table, wired to all three Java call sites — siege capture, `//castle` remove-owner, and a member leaving a castle-owning clan — each gated on `RemoveCastleCirclets` (True here) as Java gates them. A worn circlet is unequipped before destruction, so the paperdoll cannot reference a deleted object. Recorded limitation: Java also edits the `items` rows of **offline** members, which this memory-first port cannot; an offline member keeps the circlet until next login. Castle **crests** are a separate thing and remain deferred. |
| 2026-08-05 | `TODO(G33)` ×1 — `//getbuffs` pagination (`game_loop/admin/skills.rs`) | Ported via the existing `spawn::default_pager`, including Java's `>`-not-`>=` page clamp (already replicated in `flags::show_ave_menu`). Two details recorded rather than glossed: `%effectSize%` counts the **whole** list, not the page; and Java's page size of 3 pages *buffs* while rendering one row per `AbstractEffect` inside each, so its pages are longer than this port's one-row-per-buff table — a pre-existing row-shape difference, not something pagination introduced. |
| 2026-08-05 | `TODO(G33)` ×1 — non-combat cast walk (`game_loop/combat/intent.rs`) | Java's "while flying there is no move to cast": a player who must walk into range to finish a cast is refused (SM 748 + `ActionFailed`) while in a **non-combat** transform. Needed a new `Transform::combat` flag — the parser read `type=` only to spot `RIDING_MODE`, so the `COMBAT` / `NON_COMBAT` / `PURE_STAT` / `MODE_CHANGE` / `FLYING` / `CURSED` split was thrown away. The test discriminates all three cases (non-combat refused, COMBAT walks, untransformed walks), since asserting only the refusal would pass equally if the gate ignored the flag. |
| 2026-08-05 | `TODO(G33)` ×1 — detached teleport (`game_loop/death/restart.rs`) | Java completes a teleport inline for a character with no client to answer `Appearing` (`if (!isPlayer() \|\| client.isDetached()) onTeleported()`). Offline shops are the case that reaches it, and without it they stayed `teleporting` **for ever** — a flag that gates position validation, which the watchdog also could not clear. `on_teleported` now takes `Option<u32>` and skips only the client-facing halves; the visibility half needs nothing, because `set_player_region` has already re-indexed the shop and other players' scans read that index. Sabotage-verified. |
| 2026-08-05 | **Justification audit** — 4 markers corrected, none closed (`game_loop/siege.rs` ×2, `config/offline_trade.rs`, `game_loop/offline_trade.rs`) | Three fame markers said fame has "no earning path" / is "a later-chronicle stat with no ported source on Interlude". Both false: `SiegeZone.startFameTask` is the source and castle sieges *are* ported. It is inert because this dist sets `CastleZoneFameAquirePoints = 0` — a config an operator can raise, so the deferral stands but for a different reason. The offline-**craft** marker read as unported work; in fact `setCrafting(true/false)` both happen inside `RecipeItemMaker`'s constructor and `AltGameCreation = False` runs the craft inline, so no other packet can observe `isCrafting()` — the branch is *unreachable*, and Java behaves identically here. A marker that justifies itself with a wrong reason is worse than a bare one: it stops the next reader checking. |
| 2026-08-05 | `TODO(G33)` ×1 — wyvern NO_LANDING (`game_loop/servitor.rs`) | The marker's blocker was `no_landing.xml` being unloaded, and that file **is in the dist** — the port's zone loader simply never listed it. Added `ZoneKind::NoLanding` (no mask bit: the u8 is full, and this is a geometry query like `Fishing`), loaded its 9 zones, and ported Java's refusal — checked *before* the hungry branch, as Java orders it. Unlike the hungry branch, which the port notes can never fire, this one is live: the zones cover the airspace over the Grand Boss lairs and the Tower of Insolence. |
| 2026-08-05 | `TODO(G33)` ×1 — `isInventoryDisabled` (`game_loop/dispatch.rs`, `items.rs`) | Ported, and the **marker misnamed the mechanic**: it said "enchant/crystallize in progress", but Java sets `_inventoryDisable` from `Merchant.showBuyWindow` and the private/clan warehouse and wear bypasses, cleared 1500 ms later by `InventoryEnableTask`. It exists so the client's own spurious `RequestItemList` cannot redraw the inventory over a shop window that is still opening. Implemented as a `HashSet` + scheduled task rather than an expiry timestamp, because Java's task clears unconditionally — a second window opened inside the window is unblocked by the *first* task, which a timestamp would silently extend. |
| 2026-08-05 | `TODO(G33)` ×1 — `//transform` in water (`game_loop/admin/transforms.rs`) | Ported, and it nearly went in with the **wrong predicate**. `position::is_in_water` is the WATER-*zone* test (swim speed, geodata) and its own doc comment warns it is not Java's `Player.isInWater()` — which is `_taskWater != null`, the *drowning task*. `water::is_drowning_task_active` is the counterpart. Fixing the gate also exposed that the port tested the **target's** posture for the sitting refusal where Java tests the **GM's** (`activeChar.isSitting()`), and ran the checks in a different order — both corrected to Java's. |
| 2026-08-05 | `TODO(G33)` ×1 — `%protocol%` (`game_loop/admin/editchar.rs`) | The marker's blocker was real when written and had just been dissolved by someone else's work: the client's protocol version lives on the connection-side `GameClient`, and main's threading refactor gave the network layer a unified `NetEvent` channel to the game thread. Added `NetEvent::ProtocolVersion`, forwarded from the handshake, stored in `World::protocol_versions` and cleared on disconnect beside `hwids` — same lifetime, since both belong to the connection rather than the character. **Worth re-checking markers after a refactor lands**: this one aged out without anyone touching it. |
| 2026-08-05 | `TODO(G33)` ×3 — first pass on the tooling tail | **`.premium`** ported (`handlers/voicedcommandhandlers/Premium`): the account panel, gated on `EnablePremiumSystem` exactly as Java gates registration, so with the system off the line is said aloud instead. The premium system itself was already fully ported — this was only a display over it. `.password` documented as correctly absent: `AllowChangePassword = False`, so Java does not register it either, and it would need the login server's credential path. Two more converted to `SKIP`, being decisions rather than work: the GM **Sayune** hop (`SKIP(protocol)` — `ExFlyMove`/`ExFlyMoveBroadcast` are Ertheia-era opcodes the Interlude client has no handler for, so no milestone can deliver them) and **item expiry** (`SKIP(off-chronicle)` — re-verified against the datapack, not the prose: 3230 items with a positive `time`, lowest id exactly 10015, none below). |
| 2026-08-05 | `TODO(D4)` ×1 (`dashboard_api: routes/status.rs`) | Not a deferral but an **open design question** (DASHBOARD.md §12 q3), now decided with the operator: an internal TCP status channel on the **login server**, loopback by default, answering one JSON line. It lives there because the login server already tracks each game server's live link and account count — the client's server-select screen needs exactly that — so one channel covers the cluster. `/server/status` reads it instead of `characters.online`, whose rows survive a crash and kept reporting players on a dead server. |
| 2026-08-05 | `TODO(skill-see-range)` ×1 (`game_loop/skills/cast.rs`) | Java notifies **every NPC within 1000 units of the caster**, not just the skill's targets, and the *same* scan carries the "On Skill See logic" that makes a beneficial cast near a fighting mob pull it onto the caster (`effectPoint * 150 / (level + 7)` hate). The port had narrowed to the target set, which silently dropped both. Widened, and the support-aggro rule ported alongside it — porting the scan without it would have been another half-port. Sabotage-verified. |
| 2026-08-05 | `TODO(manor)` ×2 (`game_loop/manor.rs`) | Both real, both closed. **Persistence:** Java saves per action when `AltManorSaveAllActions` is on and otherwise runs a `storeMe` autosave every `AltManorSavePeriodRate` hours *plus* one on shutdown — the port had `store_manor` but called it only at rollover, and the shutdown sweep (which saves players, bosses, olympiad, cursed weapons) did not include the manor. All three paths ported, with the new `AltManorSavePeriodRate` key. **Weight/capacity:** added `weight::validate_weight` / `validate_capacity` / `slots_needed` mirroring `PlayerInventory`, and wired them into `RequestBuySeed` in **Java's order** — weight, then slots, then adena, so an overloaded pauper is told about the weight. |
| 2026-08-05 | `TODO(frintezza-4b)` ×1 (`game_loop/frintezza.rs`) | The marker was right, and **undercounted**: Java calls `playRandomSong` at *four* sites — the intro, both Scarlet morphs, and the 90 s timer — and the port only had the timer. Only the first morph carried a marker. Split `handle_song` (timer: play + re-arm) from `play_song` (Java's `playRandomSong`), because calling the timer entry point from a morph would have given each morph its own duplicate 90 s timer. Also cleared a stale doc comment claiming the 5008 debuff was unported, contradicted by the code 15 lines below it. |
| 2026-08-05 | `TODO(saga)` ×1 (`scripts/saga.rs`) | **The marker named something that does not exist.** Neither `givePormanders` nor `SkillTransfer` appears anywhere in this dist's Java or datapack. Reading what the saga quests *actually* do at the transfer exposed a real bug instead: the port cast `4339` (quest 235's elixir flash) via `cast_visual`, which emits two *self*-casts, where all 40 sites across the 31 saga quests broadcast `MagicSkillUse(npc, player, 5103, 1, 1000, 0)`. Fixed with `cast_visual_at`. Note the transfer legitimately produces **two** 5103 casts — `Player.setClassId` broadcasts its own self-cast first — which the test now pins by caster/target rather than by skill id. |
| 2026-08-05 | `TODO(q214-gargoyle-name)` ×1 (`scripts/q00214_trial_of_the_scholar.rs`) | Settled on **"Reinforced Gargoyle"** — the name three of the five disagreeing surfaces already used, including the client's own `ItemName` table. The operator renamed the client `QuestName` entry; `30612-04.html` was aligned to match and the constants renamed `ENCHANTED_*` → `REINFORCED_*`. Recorded in [CUSTOM_DIST_DEVIATIONS.md](CUSTOM_DIST_DEVIATIONS.md), because a re-sync from the Java reference dist would silently restore the retail wording. Sabotage-verified. |
| 2026-08-05 | `TODO(sieges)` ×1 (`network/server_packets/residence.rs`) | A **subsystem-level claim** that G24 falsified — the header said "sieges aren't ported yet", so the world-map overlay reported every castle unowned. Everything it needed already existed (`world.castles`, the clan `castle_id` back-reference, `castle::tax_percent`, `world.sieges`). `ExShowCastleInfo` now carries owner, tax, siege date and side. The **fortress** overlay stays static, and that is not a deferral: fort sieges are an explicit scope-out, so no fort on this dist can have an owner — the header now says that instead of implying deferred work. |
| 2026-08-05 | `TODO(quests)` ×1 (`scripts/q00641_attack_sailren.rs`) | **Stale.** "Gated until Q00126_TheNameOfEvil2 is ported" — it is ported *and* registered, the gate already calls `other_quest_completed`, and the test exercises both branches. Nothing to do but delete the claim. |
| 2026-08-05 | `TODO(reco)` ×1 (`game_loop/reco.rs`) | **Not a gap** — retagged `SKIP(fake-players)`. Recommending a `fakePlayerTalkable` NPC belongs to a Mobius `config/Custom/*` feature that ROADMAP.md scopes out except where an operator enables it, and this dist ships `EnableFakePlayers = False`. Reviving it means porting `FakePlayerData`/`FakePlayerInfo` first, not adding a branch. |
| 2026-08-05 | `TODO(login-playauth)` ×2 — **not closed, corrected** (`tests/e2e_create.rs`) | The recorded cause was false. It claimed `RequestServerLogin` answers PlayFail instead of PlayOk; instrumenting the handshake shows **PlayOk on both logins** and the whole login half completing cleanly. The real failure is on relogin: the list loads and `CharSelectionInfo` is sent, then `handle_request_restart` runs *unprompted* (the test never sends 0x57, and that handler has one non-test caller), which reloads the list and puts a second `CharSelectionInfo` where the client expects `CharSelected`. The exchange never resynchronises, so the test **hangs** rather than failing an assertion — which is why the wrong guess survived: nothing contradicted it. Marker rewritten with the walkthrough; still open, still `#[ignore]`. |
| 2026-08-05 | `TODO(pets)` ×5 (`scripts/q00421_little_wings_big_adventure.rs`) | Java's `npc.setTarget(x); npc.doCast(skill)` — **real** casts (root, DoT, debuff), not visuals. Added `QuestCtx::npc_cast`, which routes through the same `npc_cast::start_cast` the boss AIs use, plus `npc_hp_ratio` for the HP-gated arm. Dryad Root is cast at **level 33**, not 1; the level sets the root's strength and Java's `SkillHolder` carries it. The test initially passed with the casts silently doing nothing, because the fixture uses `SkillData::empty()` and an unregistered skill id makes `npc_cast` return false — see the note in `tests/mod.rs`. |
| 2026-08-05 | `TODO(cinematic)` ×1, `TODO(cosmetic)` ×1 (`scripts/q00235_mimirs_elixir.rs`, `q00125_the_name_of_evil_1.rs`) | Both wanted one `MagicSkillUse` from a named caster at a named target, which `cast_visual` could not express (it emits *two* self-casts). Added `QuestCtx::cast_visual_at`. The Q125 marker **undercounted its own gap threefold**: Java casts the pillar flourish at all three Kaimu (Ulu cond 5, Balu 6, Chuta 7) and only Ulu carried a marker — the other two were missing it with nothing to say so. |
| 2026-08-05 | `TODO(newbie-guide)` ×2 (`scripts/q00261_collectors_dream.rs`, `q00276_totem_of_the_hestui.rs`) | Ported `giveNewbieReward` for real. The marker's stated blockers were half-right: `ExShowScreenMessage` existed but only in its TEXT form, so the NpcString variant was added (4155, "Last duty complete"). `GUIDE_MISSION` still has no reader — the mission-list UI is unported — but it is written anyway because it **persists**, so credit earned now survives until that UI lands. Two traps found while wiring: it must be a *player* variable, not a `QuestState` one (both callers `exit_quest`, which would drop it on the very turn-in that earned it), and Java's `getString(key, null)` check means an absent variable and a stored 0 take different branches. |
| 2026-08-05 | `TODO(dead)` ×6 (`scripts/q00414`, `q00415`, `q00416`) | **Not gaps.** These mark Java branches that were ported verbatim but that no route on this dist can reach — no page offers the entry html, and the NPCs are registered nowhere. Retagged `SKIP(dead)`; see the `TODO` versus `SKIP` note above. Nothing changed in behaviour, and the comments' warnings against half-restoring a two-ended dead route are kept intact. |
| 2026-08-05 | `TODO(manor)` ×3 (`game_loop/skills/effects/gathering.rs`) | Same shape as the soul-crystal set: the ids (889/890/891) were in `commons::system_messages` all along, absent only from the hand-maintained `sm_ids`. Wiring them surfaced two things the markers did not mention — Java **party-broadcasts** the sow result (`party.broadcastPacket`), and the success leg also plays `ITEMSOUND_QUEST_ITEMGET`, which this port dropped silently. Both ported. |
| 2026-08-05 | `TODO(soul-crystal)` ×4 (`scripts/q00350_enhance_your_weapon.rs`) | **Half stale.** The `sm_ids` claim was true — that hand-maintained list had no soul-crystal entries — but `commons::system_messages` has carried all four ids (974/975/976/978) the whole time. Added them to `sm_ids` and wired Q350's three sites. The fifth marker with this tag was *not* closed: it sits in `skills/cast.rs`, describes `onSkillSee` breadth rather than anything about crystals, and was retagged `skill-see-range`. |
| 2026-08-05 | `TODO(radar)` ×3 (`scripts/q00348_an_arrogant_search.rs`) | **Stale.** They said the radar pings were unported; `QuestCtx::add_radar` / `add_quest_radar` / `clear_radar` all exist and Q211/Q214 use them. Wired Q348's two sites to the helpers. Closing them first exposed a real bug in `add_radar` itself — see the commit before. |

**First use, 2026-08-03.** Triaging the 15 `TODO(G33)` markers against the code
found two describing work that is **already done**:

- the delegated clan-leader transfer, delivered by
  `daily_tasks::clan_leader_apply` on the weekly reset — exactly as Java gates
  it on Wednesday. Marker removed (15 → 14).
- the auto-play/auto-potions voiced commands, which landed with the
  `Custom/*.ini` audit. That marker also covered `.premium` and `.password`,
  which really are unported, so it was **narrowed** rather than deleted.

A third, item expiry, is a *real* deferral that now records why: all 3230
time-limited items on this dist are id ≥ 10015 — entirely post-Interlude — so
nothing reachable here expires and the timer would have no consumer.

**Second pass, same day: G24, G22 and G21.** Five more markers described
finished work.

- Two siege **module headers** (`game_loop/siege.rs`, `model/siege.rs`) still
  called the battlefield "a later milestone" — control and flame towers, siege
  guards, the siege zone and its PvP, siege flags, teleport and
  ownership-on-victory all landed with G24. A third header
  (`admin/castle.rs`) already said so, which is how the contradiction showed.
- `model/castle.rs` listed residential skills as deferred; they landed with
  G24 (`clans::grant_residential_skills_to_clan`). Castle *functions* and
  crests really are still out, so that header was narrowed rather than cleared.
- `model/skill.rs` said `RESURRECTION_SPECIAL` "has no source yet" — it landed
  in **this repo's own G34 S4.16**, and the flag sits 111 lines below the
  comment denying it. Obsoleted by work in the same session that later wrote
  this file.
- `data/npc_ai_skills.rs` said "no Resurrection effect is ported yet". One is
  (G17's `revive_request`); the real reason the AI never revives is that **no
  NPC on this dist carries a resurrect skill**, which is a datapack fact and is
  now what the comment says.

**Third pass: G30, G28, G19.** Two more, and a distinction worth keeping.

- The **TvT module header** still called per-kill scoring, respawn and winner
  rewards "slices 3–4". All three landed: `on_player_death` is called from
  `death::player_do_die` for every death, and `reward_team` pays the winners.
  What is genuinely left — party/command-channel grouping, the logout forfeit
  listener, and Java's immobilise + skill-lock during the freeze — is now what
  the header says.
- The **`Lethal` effect's** deferral list named two subsystems as missing that
  have since landed: `abnormal::is_hp_blocked` (G19's `DamageBlock`) and
  `effects::calc_counter_attack` (G34 S4.4). **The gaps are real — the lethal
  arm does not consult either — but the stated reasons were wrong.** That is a
  different failure from a stale marker and a more insidious one: the deferral
  looks justified by an absence that no longer exists, so nobody re-examines
  it. Corrected to say what is actually left: wiring, not absence.
- The cursed-weapon markers (drop-on-PK-death, the "hungry" HP drain) were
  checked and are **accurate** — neither is implemented.

**Fourth pass: G20, G23, G25, G27, G29 and the small groups.** Four more, all
of the same kind — a marker naming a *subsystem* as absent when that subsystem
had since landed:

- Two sites (`skills/effects.rs`'s fear check, `skills/cast.rs`'s `isPlayable`
  narrowing) said "servitors are `TODO(G29)`". Servitors landed **with G29**.
  Neither site had a gap at all: Java's summon leg folds into the same branch,
  so the comments described a missing subsystem that exists and a gap that
  never was.
- Two more (`model/mod.rs`, twice) said "Olympiad crowning is unported
  (TODO(G25))". It is ported — `olympiad::crown` calls
  `admin::hero::set_hero`, and the period end clears it.

That is the failure this file exists to catch: markers rot in the code the same
way prose rots in the docs, and a marker describing done work is worse than no
marker — it makes finished work look outstanding.

**Fifth pass: the module headers.** The rule the fourth pass produced —
subsystem-level claims rot, itemised ones do not — was used to aim this one.
Of the 22 markers sitting in `//!` headers, two were stale, and both were
exactly that shape:

- `scripts/oly_manager.rs` said the class leaderboards, the point→mark exchange
  and the reward multisell "need persistence/scoring that lands in later
  slices". All three landed with G25 and are dispatched in that same file —
  `rank_detail`, `calculate_points_done`, and `showEquipmentReward` calling
  `multisell::separate_and_send`.
- `scripts/alliance_master.rs` said both of its buttons "log-drop until G18
  lands", the alliance system being "G18 work". G18 landed; `bypass.rs` routes
  `create_ally` and `dissolve_ally` to `clans::handle_create_ally` /
  `handle_dissolve_ally`.

**Sixth pass: the remainder, line by line.** Six more, and one of them cuts
against the pattern:

- `db.rs` said `storeCharSub`/`storeEffect` "need systems that don't exist yet
  (subclasses, buff restore on login)". Both exist — subclasses with G17, buff
  restore with G19's relative `remaining_time` rows.
- `items.rs` narrowed item use with "no pets (none exist yet), no Olympiad
  guard (no Olympiad)". Both subsystems landed (G29, G25); the path simply
  never routed to them, which is a different and smaller statement.
- `skills/effects.rs`'s sweep said "item weight/slot limits aren't modeled".
  Weight is modelled (`game_loop::weight`, plus G34 S4.1's `WEIGHT_LIMIT` /
  `WEIGHT_PENALTY`); the sweep just does not consult it.
- `model/mod.rs` carried `TODO(G21): cursedOnLogin`; `cursed_weapon::
  on_enter_world` (G28) does exactly that.
- **`death.rs` claimed shadow items were "absent from this dist". They are
  not — 295 sit in the Interlude id range, and `items.rs` already models their
  mana.** Correcting that exposed a **real bug**: the drop filter implements
  every leg of Java's `isShadowItem() || isTimeLimitedItem() || !isDropable()
  || ADENA || TYPE2_QUEST` **except `isShadowItem()`**, so a shadow item can
  drop on death here when Java would keep it. **Fixed** rather than recorded:
  the filter now carries the missing leg, reading the *instance's* `mana_left`
  (Java `Item._mana >= 0`) rather than the template, because two copies of one
  item id can differ. Sabotage-verified.

That last one is the counter-example worth recording: a **false** claim of
absence hid a live gap, where every other stale marker had merely overstated
one. The first draft of this pass removed the line as "not a real deferral" —
reading the filter, rather than the comment above it, is what caught that.

**Tally across six passes: 21 of ~134 markers examined were stale or
misjustified (16 %)**, every one understating progress. The dominant failure is
not "this small thing is still missing" — those held up almost perfectly — but
*"subsystem X is not ported"* written before X landed and never revisited.
Module headers are where those live, which makes them the place to look first.

## How to read the big ones

- **`TODO(G24)` (14)** — sieges shipped their gate; what is deferred is narrow
  and named: castle crests, the members-inside-the-zone fame task,
  `Castle.removeUpgrade()`, `ItemAction`'s mercenary-ticket refusal, a
  scheduled hit-time delay.
- **`TODO(G33)` (15)** — the optional tooling tail: `//geosave`, the niche
  admin tools, scheduled restart, multilang, Dockerfile parity. One is not
  optional but *impossible*: the GM Sayune click-to-move mode
  (`game_loop/position.rs`) needs `ExFlyMove`/`ExFlyMoveBroadcast`
  (`0xFE:0xE8` / `0xFE:0x108`), Ertheia-era opcodes with no counterpart in the
  Interlude protocol — the hop is ported with a `FlyToLocation(DUMMY)`
  substitute.
- **`TODO(G34)` (13)** — the skill epic's recorded residue, each argued in
  `PLAN_G34_SKILL_PARITY.md`: A3's `isSelfContinuous` icon rule (no effector on
  `ActiveBuff` to test), `nextAction=CAST` (needs an intention queue),
  `calcCrit`'s level-78 branch, the fort gate on `OpenDoor`, raid-minion
  detection for `Bluff`. Plus one added since: `BlockActions.onStart` ports
  Java's `startParalyze` *cast* abort but not its `abortAttack()` leg — a
  scheduled `AttackHit` has no cancel handle in this port, so a stun landing
  between a swing's start and its hit tick still lets that hit land
  (`game_loop/skills/effects.rs`, `apply_block_actions_interrupt`).

**Seventh pass, 2026-08-05: the counter could not see 46 of its own subjects.**
Every pass above triaged markers the inventory *knew about*. This one audited
the inventory itself, and found the number it asserts — 134 — was never the
number in the code. It was 180.

The scan looked for the literal `TODO(G` and accepted only alphanumerics, `.`,
`-` and `_` in a tag. Two families fell through:

- **Nine milestone markers** written with a `+`, `?` or `/` suffix — `G9+`,
  `G13+`, `G19+`, `G21+` (×2), `G29+` (×2), `G?`, `G24/G26`. Every one is a
  deliberate, correctly-placed deferral. The tag simply failed the character
  allowlist, so it was dropped in silence.
- **Thirty-seven topic-tagged markers** — `pets`, `manor`, `soul-crystal`,
  `radar`, `dead`, `login-playauth` and the rest — which never matched the
  `TODO(G` prefix at all, so no version of this file ever listed them.

The `Regenerating` recipe below had the identical blind spot (it required a
`)` immediately after `[0-9._a-z-]*`), which is why the doc and the test agreed
with each other and both disagreed with the code. **Two artefacts derived from
the same wrong assumption do not cross-check each other** — that is the lesson
worth keeping, and it is a sharper version of the one this file opens with.

The fix widens the scan to every `TODO(<tag>)` and moves the burden onto the
allowlist: `+ ? /` are now legal tag characters, `<`, quotes and whitespace are
not. One marker was renamed to comply (`frintezza slice 4b` → `frintezza-4b`;
a tag cannot contain spaces). The widened scan immediately caught a parseable
tag written inside a doc comment in its own source file, which is the intended
behaviour and the reason that constraint is documented above.

No marker was closed by this pass and none was stale — 134 → 180 is a
correction to the *count*, not a regression in the code. The port did not get
worse; the ledger got honest.

## Regenerating

```sh
grep -rn -oE 'TODO\([A-Za-z0-9][A-Za-z0-9._/+?-]*\)' crates/ | \
  awk -F: '{print $3}' | sort | uniq -c | sort -rn
```
| 2026-08-06 | `TODO(G34)` ×1 (`skills/conditions.rs`) | **Closed.** `ConditionPlayerCanResurrect`'s summon leg ported: dead → resurrection-blocked → owner's open prompt. It waited on a servitor subsystem that had already landed (`ServitorOf`), and a pet carries the same component, so the one branch covers both. |
| 2026-08-06 | `TODO(G34)` ×1 → `SKIP(off-chronicle)` (`skills/instant.rs`) | Fort half of the door-unlock gate. Fort sieges are an explicit scope-out for this build, so "once forts exist" names a milestone that cannot arrive; the skill path is vacuous regardless, since none of the 34 `BY_SKILL` doors belongs to a fort. |
| 2026-08-06 | `TODO(G34)` ×1 (`skills/effects/control.rs`) | **Stale.** "Extend the recall gate as those states land" — they had all landed: `ROOTED`, `olympiad::in_match`, `is_registered`, `OlympiadObserver`, `is_flying`, `has_combat_flag`, `is_on_event`, `jailed`. Gate extracted as `check_summon_target_status` and completed in Java's branch order, which is load-bearing (a rooted olympiad fighter reads the *combat* line). Only `isInTraingCamp` and the instance summon permission stay out, both noted at their branch. |
| 2026-08-06 | `TODO(G34)` ×1 (`skills/effects/control.rs`) | **Closed.** `abortAttack()` — the swing in flight — now dies with the cast. The blocker was real (a heap scheduler has no cancel handle), so it was solved rather than waited on: `AttackState::swing_seq` is bumped by an abort and every `AttackHit` carries the value it was queued with, so a stale hit is dropped when it fires. Wired to stun/sleep/paralyze, fake death, and **physical** mute only — Java's `Mute.onStart` never aborts the swing, only `startPhysicalAttackMuted` does. |
| 2026-08-06 | `TODO(G34)` ×3 → `SKIP` (`skills/cast.rs` ×2, `npc_cast.rs`) | Both waited on capability the dist cannot exercise. `NextAction::Cast` needs an AI intention queue, but all 11 `<nextAction>CAST` skills are off-chronicle and appear in no skill tree or NPC list. The instant-cast move-stop exemption needs a `SIMULTANEOUS`/`abnormalInstant`/`withoutAction` skill on an NPC; 57 skills carry those markers and none is in any of the 2159 NPC-castable ids or any script's `getSkill`. |
| 2026-08-06 | `TODO(G34)` ×1 (`skill_data/build.rs`) | **Closed.** `isSelfContinuous()` / `isDisplayedForEffected()`. The marker said `ActiveBuff` records no effector so the rule had nothing to test — but the rule only needs the *answer*, not the effector, so `displayed` is stamped at creation where the caster is still in scope. Gates both channels Java gates: the icon row and the abnormal-visual fold. Six skills qualify on this dist (321, 368, 369, 409, 1231, 1996). |
| 2026-08-06 | `TODO(G34)` ×1 (`death/player_death.rs`) | **Stale.** "The vitality-consumption exemption, once that branch exists" — `vitality::update_vitality_points` has had the branch since G33, complete with its own private `is_lucky`. Two copies of one predicate is two places for the level bound to drift, so the duplicate is gone and vitality calls the death one. |
| 2026-08-06 | `TODO(G34)` ×1 → `SKIP(census)` (`skills/effects/mod.rs`) | "Re-check if any `ImmobilePetBuff` carrier uses a wider target type." Censused: the effect appears on exactly one skill in the whole dist — Servitor Empowerment 1299, `SUMMON`/`SINGLE` — so Java's owner gate is satisfied by construction and there is no wider carrier to re-check. |
| 2026-08-06 | `TODO(G33)` ×1 (`config/offline_trade.rs`) | **Stale reason.** "No way to hold a config-driven visual on a player with no buff behind it" — `AdminVisuals` is precisely that component, and had been for milestones. `OfflineAbnormalEffect` now resolves names to client ids at load and `enter_offline_mode` applies **one at random**, as Java does (`Rnd.get(size())`), not the whole list. Inert on this dist, which ships the key empty. |
| 2026-08-06 | `TODO(G24)` + `TODO(G33)` ×1 each (`siege.rs`, `config/offline_trade.rs`) | **Closed together** — one was the other's blocker. Ported `SiegeZone`'s fame task: armed on siege-zone entry for a registered participant, re-arming itself instead of holding a cancel handle, with Java's three `FameTask.run` refusals (dead + `FameForDeadPlayers` off, detached + `OfflineFame` off, out of zone). Inert on this dist (`CastleZoneFameAquirePoints = 0`), and deliberately ported as Java writes it — the amount is not part of the arming gate — so raising the config is enough. |
| 2026-08-06 | `TODO(G30)` ×3 (`model/skill.rs`, `coverage_census.rs` ×2) | **Closed.** Summon Friend — `CallPc`'s player half. Needed a full `ConfirmDlg` (params + time + requesterId), a `SummonRequest` holder, and the recall gate, which had landed earlier the same day. Toll is charged to the **target** before the prompt, as Java does, so declining still costs the crystal. Two census caveats warning that "`CallPc` handled" overstated things are retired with it. |
| 2026-08-06 | `TODO(G30)` ×3 (`multisell.rs`) | Censused across all 101 lists, and the three split apart. **Special products**: none exist — every negative id in the tree is an ingredient. **Enchanted ingredients**: none exist — `enchantmentLevel` appears on 3 productions and no ingredient. **Chance**: only 2 non-degenerate entries, both owned by NPC 34262 (HappyHours), which has no spawn row. All three become `SKIP(census)`. The one real case — 10 `-200` clan-reputation *ingredients* on the spawned Clan Traders' list 1235 — is now implemented, with Java's refusal order (membership → leadership → balance). |
| 2026-08-06 | `TODO(G22)` ×2 (`q00224`, `q00230`) | Both **stale**. Kadesh's 300 s despawn needed `schedule_despawn`, which the quest ctx already had; without it an unfought Kadesh stood in the field forever and the next roll spawned another beside him. The `onKill` range gate needed a distance helper — `AltPartyRange` was parsed and `party.rs` already measured against it — so `QuestCtx::in_range_of_npc` now exists and q230 hoists the check that Java repeats on every branch. |
| 2026-08-06 | `TODO(G22)` ×1 (`forge_of_the_gods.rs`) | **Closed.** A lavasaurus outliving its minute now runs Java's "suicide" event — `doDie(null)` via a new `ScheduledTask::NpcSuicide` — instead of a silent despawn, so it dies where it stands and leaves a corpse. Killer 0 is inert on every reward and aggro path (both gate on the killer being playable), which is what makes the null-killer death safe. |
| 2026-08-06 | `TODO(G19)` ×1 (`model/skill.rs`, Lethal) | **Closed** — and two of its three bullets were already false when read. `isHpBlocked()` *was* consulted, and grand bosses *were* immune (`is_raid()` matches the `GrandBoss` type name). Real work: the door case (`Door` component existed) and `calcCounterAttack`, which Java fires from `Lethal.instant` on top of the one `reduceCurrentHp` already ran — so a lethal cast counters **twice**, and suppressing the second would be the deviation. |
| 2026-08-06 | `TODO(G19)` ×1 (`model/mod.rs`, charges) | **Closed.** `ResetChargesTask` — Force decays after ten idle minutes, the clock restarts on every gain and partial spend, and stops when the pool empties. Same generation-counter shape as `AttackState::swing_seq`, since the scheduler has no cancel; bumping the counter without re-arming *is* `stopChargeTask`. |
| 2026-08-06 | `TODO(G14)` ×1 (`config/general.rs`) | **Stale.** "Honor these once `SkillTreeData.addSkills` has the special-skill data" — `gameMasterSkillTree.xml` and its aura twin ship in the dist and parse with the same shape as the hero/noble trees the loader already read. Granted at enter-world under the two config flags, and **filtered out of the persistence flush**: Java's `addSkill(skill, false)` means session-only, or turning the config back off leaves every GM who ever logged in holding Super Haste. |
| 2026-08-06 | `TODO(G34)` ×1 (`skills/conditions.rs`, Inner Rhythm) | **Closed — the premise was false.** The marker asked whether the port should copy Java's disabling of Inner Rhythm's passive. Testing it in-game exposed that the port applied *nothing*: `conditioned_passive_buffs` keeps only `StatModifier` effects, so a passive whose only effect is a rate (`MagicMpCost`/`Reuse`) produced no buff and reached no rate table. Seven learnable passives were inert (164, 428, 435, 436, 615, 945, 1527) plus the Clarity/Apella/boss-jewel item skills. Fixed with a separate, wholesale-rebuilt passive half of `SkillRateStats`; the deviation from Java is kept and its reasoning rewritten. |
| 2026-08-06 | `TODO(G28)` ×1 (`events/tvt.rs`) | **Half stale.** "No immobilize / skill-lock flag on this port" — `Immobilized` existed; the skill lock did not, and is now `SkillsDisabled` feeding `abnormal::all_skills_disabled` (Java's `_allSkillsDisabled \|\| hasBlockActions()`, which the cast gate had been answering with only its second half). TvT's `EndFight` freeze now covers servitors, and the thaw releases them — a deliberate deviation from Java, whose thaw block re-freezes summons. |
| 2026-08-06 | `TODO(G28)` ×4 + `TODO(G21)` ×1 (cursed weapons) | Three module headers listed drop-on-PK-death, the kill-count stage-up and a "hungry HP/time decay" as deferred; the first two had landed and the HP half **never existed in Java** — its only HP touch is the full heal on pickup, which is also ported. The `//cw_add` marker was misattributed: Java's admin path does not route through `CursedWeaponsManager.activate`, so it has no two-weapon branch either; the port implements that branch where it belongs, on the pickup path. Genuinely missing and now done: `//cw_goto`'s ground-drop branch (`dropped_item_oid` was already tracked). |
| 2026-08-06 | `TODO(G23)` ×1 (`target.rs`) | **Mostly stale.** "Needs the three karma configs, which we don't parse yet" — two were parsed *and* consumed (warehouse access, gatekeeper refusal); only `AltKarmaPlayerCanShop` was missing. `showChatWindow`'s reputation gate is now ported: a criminal gets `<npcId>-pk.htm` plus `ActionFailed` instead of the dialog, at the 117 NPCs the datapack wrote a refusal for (92 merchant, 24 teleporter, 1 fisherman) and nowhere else — Java falls through when the file is absent. |
| 2026-08-06 | `TODO(G23)` ×2 (`valakas.rs`) | **Closed.** The lair theme now uses a `play_music` builder (`PlaySound(1, …)`) instead of the type-0 quest-sound form — a different packet to the client, not a variant. The exit cubes carry Java's 15-minute `addSpawn` lifetime. Chasing the first turned up a **silent hole beside it**: Java's `"broadcast_spawn"` (theme + social action 3, armed 100 ms into `"beginning"`) was unported and unmarked, so the fight opened in silence. |
