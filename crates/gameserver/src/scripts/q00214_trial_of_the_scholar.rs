//! Trial of the Scholar (214) — `quests/Q00214_TrialOfTheScholar`. The mage
//! trial (Wizard / Elven Wizard / Dark Wizard, level 35+). Magister Mirien hands
//! out three sigils, each unlocking a Symbol sub-quest: the Symbol of Sylvain
//! (a long letter/painting relay through Maria, Lucas and Creta), the Symbol of
//! Jurek (a hunt for monster trophies), and the Symbol of Cronos (recover the
//! four Scripture Chapters via Dieter, Edroc, Raut, Triff, Valkon, Poitan,
//! Casian and Grandis). All three Symbols earn the Mark of the Scholar.
//!
//! Pure item-gate (cond 1..31) — no `memoState`, weapon or skill mechanics, just
//! the longest linear chain in the group.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const HIGH_PRIEST_SYLVAIN: i32 = 30070;
const CAPTAIN_LUCAS: i32 = 30071;
const WAREHOUSE_KEEPER_VALKON: i32 = 30103;
const MAGISTER_DIETER: i32 = 30111;
const GRAND_MAGISTER_JUREK: i32 = 30115;
const TRADER_EDROC: i32 = 30230;
const WAREHOUSE_KEEPER_RAUT: i32 = 30316;
const BLACKSMITH_POITAN: i32 = 30458;
const MAGISTER_MIRIEN: i32 = 30461;
const MARIA: i32 = 30608;
const ASTROLOGER_CRETA: i32 = 30609;
const ELDER_CRONOS: i32 = 30610;
const DRUNKARD_TRIFF: i32 = 30611;
const ELDER_CASIAN: i32 = 30612;
// Items
const MIRIENS_1ST_SIGIL: i32 = 2675;
const MIRIENS_2ND_SIGIL: i32 = 2676;
const MIRIENS_3RD_SIGIL: i32 = 2677;
const MIRIENS_INSTRUCTION: i32 = 2678;
const MARIAS_1ST_LETTER: i32 = 2679;
const MARIAS_2ND_LETTER: i32 = 2680;
const LUCASS_LETTER: i32 = 2681;
const LUCILLAS_HANDBAG: i32 = 2682;
const CRETAS_1ST_LETTER: i32 = 2683;
const CRERAS_PAINTING1: i32 = 2684;
const CRERAS_PAINTING2: i32 = 2685;
const CRERAS_PAINTING3: i32 = 2686;
const BROWN_SCROLL_SCRAP: i32 = 2687;
const CRYSTAL_OF_PURITY1: i32 = 2688;
const HIGH_PRIESTS_SIGIL: i32 = 2689;
const GRAND_MAGISTER_SIGIL: i32 = 2690;
const CRONOS_SIGIL: i32 = 2691;
const SYLVAINS_LETTER: i32 = 2692;
const SYMBOL_OF_SYLVAIN: i32 = 2693;
const JUREKS_LIST: i32 = 2694;
const MONSTER_EYE_DESTROYER_SKIN: i32 = 2695;
const SHAMANS_NECKLACE: i32 = 2696;
const SHACKLES_SCALP: i32 = 2697;
const SYMBOL_OF_JUREK: i32 = 2698;
const CRONOS_LETTER: i32 = 2699;
const DIETERS_KEY: i32 = 2700;
const CRETAS_2ND_LETTER: i32 = 2701;
const DIETERS_LETTER: i32 = 2702;
const DIETERS_DIARY: i32 = 2703;
const RAUTS_LETTER_ENVELOPE: i32 = 2704;
const TRIFFS_RING: i32 = 2705;
const SCRIPTURE_CHAPTER_1: i32 = 2706;
const SCRIPTURE_CHAPTER_2: i32 = 2707;
const SCRIPTURE_CHAPTER_3: i32 = 2708;
const SCRIPTURE_CHAPTER_4: i32 = 2709;
const VALKONS_REQUEST: i32 = 2710;
const POITANS_NOTES: i32 = 2711;
const STRONG_LIGUOR: i32 = 2713;
const CRYSTAL_OF_PURITY2: i32 = 2714;
const CASIANS_LIST: i32 = 2715;
const GHOULS_SKIN: i32 = 2716;
const MEDUSAS_BLOOD: i32 = 2717;
const FETTERED_SOULS_ICHOR: i32 = 2718;
const ENCHANTED_GARGOYLES_NAIL: i32 = 2719;
const SYMBOL_OF_CRONOS: i32 = 2720;
// Reward
const MARK_OF_SCHOLAR: i32 = 2674;
// Monsters
const MONSTER_EYE_DESTREOYER: i32 = 20068;
const MEDUSA: i32 = 20158;
const GHOUL: i32 = 20201;
const SHACKLE1: i32 = 20235;
const BREKA_ORC_SHAMAN: i32 = 20269;
const SHACKLE2: i32 = 20279;
const FETTERED_SOUL: i32 = 20552;
const GRANDIS: i32 = 20554;
/// TODO(q214-gargoyle-name): this mob carries four different names across the
/// server data, the NPC dialogue and the client's own text tables, so Casian's
/// reagent step reads as an errand against a monster the player never finds:
///
/// - `stats/npcs/20500-20599.xml` names 20567 "Reinforced Gargoyle",
/// - `stats/items/02700-02799.xml` names 2719 "Reinforced Gargoyle's Nail",
/// - the client's `ItemName` table agrees — id 2719 is "Reinforced Gargoyle's
///   Nail" there too,
/// - but the client's `QuestName` journal entry for quest 214 step 28
///   ("Casian's Magic Ingredient") tells the player to hunt "Enchanted
///   Gargoyles" in the Crater of Ivory Tower,
/// - and `Q00214_TrialOfTheScholar/30612-04.html` calls them "Enhanced
///   Gargoyles" dropping "Enhanced Gargoyle Nails",
/// - these constants follow the journal: `ENCHANTED_GARGOYLE` /
///   `ENCHANTED_GARGOYLES_NAIL`.
///
/// Note this is not fixable server-side alone: the journal line the player reads
/// in the quest window lives in the client's `QuestName` .dat, so whichever name
/// wins, that entry has to be repacked and shipped with a client patch.
///
/// Cheapest consistent fix is "Reinforced Gargoyle" everywhere — three of the
/// five surfaces, including the client's own item table, already say it, so only
/// the journal step and `30612-04.html` change and the mob keeps its name. The
/// alternative, renaming the NPC to match the journal, additionally means
/// editing the client npcname table and both item names.
///
/// Either way rename these constants to match whichever name wins.
const ENCHANTED_GARGOYLE: i32 = 20567;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
// Misc
const MIN_LEVEL: i32 = 35;
const LEVEL: i32 = 36;
const WIZARD: i32 = 11;
const ELVEN_WIZARD: i32 = 26;
const DARK_WIZARD: i32 = 39;
/// Maria's spawn in the Town of Dion (`spawns/Dion/Dion.xml`), and the target
/// the client's own quest data gives for her steps.
///
/// Deviation from the Java datapack: Valkon sends the player after a Crystal of
/// Purity without naming its only maker, and no journal step names her either,
/// so the errand is unfindable without a walkthrough. We drop a radar marker on
/// her when the request is handed over (and on every reminder), the way the
/// quest engine already guides players elsewhere.
const MARIA_LOC: (i32, i32, i32) = (19041, 145964, -3068);

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// True when the Jurek trophy set (5 skins, 5 necklaces, 2 scalps) is complete.
fn jurek_set_done(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(MONSTER_EYE_DESTROYER_SKIN) >= 5
        && ctx.quest_items_count(SHAMANS_NECKLACE) >= 5
        && ctx.quest_items_count(SHACKLES_SCALP) >= 2
}

pub struct Q00214TrialOfTheScholar;

impl QuestScript for Q00214TrialOfTheScholar {
    fn id(&self) -> i32 {
        214
    }
    fn name(&self) -> &'static str {
        "Q00214_TrialOfTheScholar"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00214_TrialOfTheScholar"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MAGISTER_MIRIEN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MAGISTER_MIRIEN,
            HIGH_PRIEST_SYLVAIN,
            CAPTAIN_LUCAS,
            WAREHOUSE_KEEPER_VALKON,
            MAGISTER_DIETER,
            GRAND_MAGISTER_JUREK,
            TRADER_EDROC,
            WAREHOUSE_KEEPER_RAUT,
            BLACKSMITH_POITAN,
            MARIA,
            ASTROLOGER_CRETA,
            ELDER_CRONOS,
            DRUNKARD_TRIFF,
            ELDER_CASIAN,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            MONSTER_EYE_DESTREOYER,
            MEDUSA,
            GHOUL,
            SHACKLE1,
            BREKA_ORC_SHAMAN,
            SHACKLE2,
            FETTERED_SOUL,
            GRANDIS,
            ENCHANTED_GARGOYLE,
            LETO_LIZARDMAN_WARRIOR,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            MIRIENS_1ST_SIGIL,
            MIRIENS_2ND_SIGIL,
            MIRIENS_3RD_SIGIL,
            MIRIENS_INSTRUCTION,
            MARIAS_1ST_LETTER,
            MARIAS_2ND_LETTER,
            LUCASS_LETTER,
            LUCILLAS_HANDBAG,
            CRETAS_1ST_LETTER,
            CRERAS_PAINTING1,
            CRERAS_PAINTING2,
            CRERAS_PAINTING3,
            BROWN_SCROLL_SCRAP,
            CRYSTAL_OF_PURITY1,
            HIGH_PRIESTS_SIGIL,
            GRAND_MAGISTER_SIGIL,
            CRONOS_SIGIL,
            SYLVAINS_LETTER,
            SYMBOL_OF_SYLVAIN,
            JUREKS_LIST,
            MONSTER_EYE_DESTROYER_SKIN,
            SHAMANS_NECKLACE,
            SHACKLES_SCALP,
            SYMBOL_OF_JUREK,
            CRONOS_LETTER,
            DIETERS_KEY,
            CRETAS_2ND_LETTER,
            DIETERS_LETTER,
            DIETERS_DIARY,
            RAUTS_LETTER_ENVELOPE,
            TRIFFS_RING,
            SCRIPTURE_CHAPTER_1,
            SCRIPTURE_CHAPTER_2,
            SCRIPTURE_CHAPTER_3,
            SCRIPTURE_CHAPTER_4,
            VALKONS_REQUEST,
            POITANS_NOTES,
            STRONG_LIGUOR,
            CRYSTAL_OF_PURITY2,
            CASIANS_LIST,
            GHOULS_SKIN,
            MEDUSAS_BLOOD,
            FETTERED_SOULS_ICHOR,
            ENCHANTED_GARGOYLES_NAIL,
            SYMBOL_OF_CRONOS,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    if !has(ctx, MIRIENS_1ST_SIGIL) {
                        ctx.give_items(MIRIENS_1ST_SIGIL, 1);
                    }
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
                None
            }
            "30103-02.html" | "30103-03.html" | "30111-02.html" | "30111-03.html"
            | "30111-04.html" | "30111-08.html" | "30111-14.html" | "30115-02.html"
            | "30316-03.html" | "30461-09.html" | "30608-07.html" | "30609-02.html"
            | "30609-03.html" | "30609-04.html" | "30609-08.html" | "30609-13.html"
            | "30610-02.html" | "30610-03.html" | "30610-04.html" | "30610-05.html"
            | "30610-06.html" | "30610-07.html" | "30610-08.html" | "30610-09.html"
            | "30610-13.html" | "30611-02.html" | "30611-03.html" | "30611-06.html"
            | "30612-03.html" => Some(event.to_string()),
            "30461-10.html" => guarded(
                ctx,
                has(ctx, MIRIENS_2ND_SIGIL) && has(ctx, SYMBOL_OF_JUREK),
                event,
                |c| {
                    c.take_items(MIRIENS_2ND_SIGIL, 1);
                    c.give_items(MIRIENS_3RD_SIGIL, 1);
                    c.take_items(SYMBOL_OF_JUREK, 1);
                    c.set_cond(19, true);
                },
            ),
            "30070-02.html" => {
                ctx.give_items(HIGH_PRIESTS_SIGIL, 1);
                ctx.give_items(SYLVAINS_LETTER, 1);
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "30071-04.html" => guarded(ctx, has(ctx, CRERAS_PAINTING2), event, |c| {
                c.take_items(CRERAS_PAINTING2, 1);
                c.give_items(CRERAS_PAINTING3, 1);
                c.set_cond(10, true);
            }),
            "30103-04.html" => {
                ctx.give_items(VALKONS_REQUEST, 1);
                // Mark Maria - the only maker of the Crystal of Purity (see MARIA_LOC).
                ctx.add_quest_radar(MARIA_LOC.0, MARIA_LOC.1, MARIA_LOC.2);
                Some(event.to_string())
            }
            "30111-05.html" => guarded(ctx, has(ctx, CRONOS_LETTER), event, |c| {
                c.take_items(CRONOS_LETTER, 1);
                c.give_items(DIETERS_KEY, 1);
                c.set_cond(21, true);
            }),
            "30111-09.html" => guarded(ctx, has(ctx, CRETAS_2ND_LETTER), event, |c| {
                c.take_items(CRETAS_2ND_LETTER, 1);
                c.give_items(DIETERS_LETTER, 1);
                c.give_items(DIETERS_DIARY, 1);
                c.set_cond(23, true);
            }),
            "30115-03.html" => {
                ctx.give_items(JUREKS_LIST, 1);
                ctx.give_items(GRAND_MAGISTER_SIGIL, 1);
                ctx.set_cond(16, true);
                Some(event.to_string())
            }
            "30230-02.html" => guarded(ctx, has(ctx, DIETERS_LETTER), event, |c| {
                c.take_items(DIETERS_LETTER, 1);
                c.give_items(RAUTS_LETTER_ENVELOPE, 1);
                c.set_cond(24, true);
            }),
            "30316-02.html" => guarded(ctx, has(ctx, RAUTS_LETTER_ENVELOPE), event, |c| {
                c.take_items(RAUTS_LETTER_ENVELOPE, 1);
                c.give_items(SCRIPTURE_CHAPTER_1, 1);
                c.give_items(STRONG_LIGUOR, 1);
                c.set_cond(25, true);
            }),
            "30608-02.html" => guarded(ctx, has(ctx, SYLVAINS_LETTER), event, |c| {
                c.give_items(MARIAS_1ST_LETTER, 1);
                c.take_items(SYLVAINS_LETTER, 1);
                c.set_cond(3, true);
            }),
            "30608-08.html" => guarded(ctx, has(ctx, CRETAS_1ST_LETTER), event, |c| {
                c.give_items(LUCILLAS_HANDBAG, 1);
                c.take_items(CRETAS_1ST_LETTER, 1);
                c.set_cond(7, true);
            }),
            "30608-14.html" => guarded(ctx, has(ctx, CRERAS_PAINTING3), event, |c| {
                c.take_items(CRERAS_PAINTING3, 1);
                c.take_items(BROWN_SCROLL_SCRAP, -1);
                c.give_items(CRYSTAL_OF_PURITY1, 1);
                c.set_cond(13, true);
            }),
            "30609-05.html" => guarded(ctx, has(ctx, MARIAS_2ND_LETTER), event, |c| {
                c.take_items(MARIAS_2ND_LETTER, 1);
                c.give_items(CRETAS_1ST_LETTER, 1);
                c.set_cond(6, true);
            }),
            "30609-09.html" => guarded(ctx, has(ctx, LUCILLAS_HANDBAG), event, |c| {
                c.take_items(LUCILLAS_HANDBAG, 1);
                c.give_items(CRERAS_PAINTING1, 1);
                c.set_cond(8, true);
            }),
            "30609-14.html" => guarded(ctx, has(ctx, DIETERS_KEY), event, |c| {
                c.take_items(DIETERS_KEY, 1);
                c.give_items(CRETAS_2ND_LETTER, 1);
                c.set_cond(22, true);
            }),
            "30610-10.html" => {
                ctx.give_items(CRONOS_SIGIL, 1);
                ctx.give_items(CRONOS_LETTER, 1);
                ctx.set_cond(20, true);
                Some(event.to_string())
            }
            "30610-14.html" => guarded(
                ctx,
                has(ctx, SCRIPTURE_CHAPTER_1)
                    && has(ctx, SCRIPTURE_CHAPTER_2)
                    && has(ctx, SCRIPTURE_CHAPTER_3)
                    && has(ctx, SCRIPTURE_CHAPTER_4),
                event,
                |c| {
                    c.take_items(CRONOS_SIGIL, 1);
                    c.take_items(DIETERS_DIARY, 1);
                    c.take_items(TRIFFS_RING, 1);
                    c.take_items(SCRIPTURE_CHAPTER_1, 1);
                    c.take_items(SCRIPTURE_CHAPTER_2, 1);
                    c.take_items(SCRIPTURE_CHAPTER_3, 1);
                    c.take_items(SCRIPTURE_CHAPTER_4, 1);
                    c.give_items(SYMBOL_OF_CRONOS, 1);
                    c.set_cond(31, true);
                },
            ),
            "30611-04.html" => guarded(ctx, has(ctx, STRONG_LIGUOR), event, |c| {
                c.give_items(TRIFFS_RING, 1);
                c.take_items(STRONG_LIGUOR, 1);
                c.set_cond(26, true);
            }),
            "30612-04.html" => {
                ctx.give_items(CASIANS_LIST, 1);
                ctx.set_cond(28, true);
                Some(event.to_string())
            }
            "30612-07.html" => {
                ctx.give_items(SCRIPTURE_CHAPTER_4, 1);
                ctx.take_items(POITANS_NOTES, 1);
                ctx.take_items(CASIANS_LIST, 1);
                ctx.take_items(GHOULS_SKIN, -1);
                ctx.take_items(MEDUSAS_BLOOD, -1);
                ctx.take_items(FETTERED_SOULS_ICHOR, -1);
                ctx.take_items(ENCHANTED_GARGOYLES_NAIL, -1);
                ctx.set_cond(30, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            LETO_LIZARDMAN_WARRIOR => {
                if has(ctx, MIRIENS_1ST_SIGIL)
                    && has(ctx, HIGH_PRIESTS_SIGIL)
                    && has(ctx, CRERAS_PAINTING3)
                    && ctx.quest_items_count(BROWN_SCROLL_SCRAP) < 5
                {
                    ctx.give_items(BROWN_SCROLL_SCRAP, 1);
                    if ctx.quest_items_count(BROWN_SCROLL_SCRAP) == 5 {
                        ctx.set_cond(12, true);
                    } else {
                        ctx.play_sound(quest_sounds::ITEMGET);
                    }
                }
            }
            MONSTER_EYE_DESTREOYER => jurek_kill(ctx, MONSTER_EYE_DESTROYER_SKIN, 5),
            BREKA_ORC_SHAMAN => jurek_kill(ctx, SHAMANS_NECKLACE, 5),
            SHACKLE1 | SHACKLE2 => jurek_kill(ctx, SHACKLES_SCALP, 2),
            GRANDIS => {
                if has(ctx, MIRIENS_3RD_SIGIL)
                    && has(ctx, CRONOS_SIGIL)
                    && has(ctx, TRIFFS_RING)
                    && !has(ctx, SCRIPTURE_CHAPTER_3)
                {
                    ctx.give_items(SCRIPTURE_CHAPTER_3, 1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
            }
            MEDUSA => casian_kill(ctx, MEDUSAS_BLOOD, 12),
            GHOUL => casian_kill(ctx, GHOULS_SKIN, 10),
            FETTERED_SOUL => casian_kill(ctx, FETTERED_SOULS_ICHOR, 5),
            ENCHANTED_GARGOYLE => casian_kill(ctx, ENCHANTED_GARGOYLES_NAIL, 5),
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            if ctx.npc_id == MAGISTER_MIRIEN {
                let class = ctx.player_class_id();
                if class == WIZARD || class == ELVEN_WIZARD || class == DARK_WIZARD {
                    return Some(if ctx.player_level() < MIN_LEVEL {
                        "30461-02.html".to_string()
                    } else {
                        "30461-03.htm".to_string()
                    });
                }
                return Some("30461-01.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if ctx.is_completed() {
            if ctx.npc_id == MAGISTER_MIRIEN {
                return Some(ctx.already_completed_html());
            }
            return Some(ctx.no_quest_html());
        }
        match ctx.npc_id {
            MAGISTER_MIRIEN => Some(mirien_talk(ctx)),
            HIGH_PRIEST_SYLVAIN => Some(sylvain_talk(ctx)),
            CAPTAIN_LUCAS => Some(lucas_talk(ctx)),
            WAREHOUSE_KEEPER_VALKON => Some(valkon_talk(ctx)),
            MAGISTER_DIETER => Some(dieter_talk(ctx)),
            GRAND_MAGISTER_JUREK => Some(jurek_talk(ctx)),
            TRADER_EDROC => Some(edroc_talk(ctx)),
            WAREHOUSE_KEEPER_RAUT => Some(raut_talk(ctx)),
            BLACKSMITH_POITAN => Some(poitan_talk(ctx)),
            MARIA => Some(maria_talk(ctx)),
            ASTROLOGER_CRETA => Some(creta_talk(ctx)),
            ELDER_CRONOS => Some(cronos_talk(ctx)),
            DRUNKARD_TRIFF => Some(triff_talk(ctx)),
            ELDER_CASIAN => Some(casian_talk(ctx)),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

/// Run `action` and return `event` when `cond` holds, else `None`.
fn guarded(
    ctx: &mut QuestCtx,
    cond: bool,
    event: &str,
    action: impl FnOnce(&mut QuestCtx),
) -> Option<String> {
    if cond {
        action(ctx);
        Some(event.to_string())
    } else {
        None
    }
}

/// A Jurek trophy leg; cond 17 once all three trophies are collected.
fn jurek_kill(ctx: &mut QuestCtx, item: i32, cap: i64) {
    if has(ctx, MIRIENS_2ND_SIGIL)
        && has(ctx, GRAND_MAGISTER_SIGIL)
        && has(ctx, JUREKS_LIST)
        && ctx.quest_items_count(item) < cap
    {
        ctx.give_items(item, 1);
        if jurek_set_done(ctx) {
            ctx.set_cond(17, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

/// True when all four of Casian's reagents are at their limits — the same set
/// the talk handler tests as a sum of 32.
fn casian_set_done(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(GHOULS_SKIN) >= 10
        && ctx.quest_items_count(MEDUSAS_BLOOD) >= 12
        && ctx.quest_items_count(FETTERED_SOULS_ICHOR) >= 5
        && ctx.quest_items_count(ENCHANTED_GARGOYLES_NAIL) >= 5
}

/// A Casian reagent leg (Symbol of Cronos, Chapter 4). Cond 29 ("return to
/// Casian") belongs to the *whole* set, like the Jurek trophies above.
///
/// The Mobius Classic-Interlude script fires it from the Ghoul leg alone, at 10
/// skins, so a player who hunted ghouls first was sent back to Casian while
/// still owing blood, ichor and nails. `L2J_Mobius_CT_0_Interlude` tests all
/// four counts in each leg and l2j-server gates the same `setCond(29)` behind
/// `hasItemsAtLimit(...)` over all four; follow them.
fn casian_kill(ctx: &mut QuestCtx, item: i32, cap: i64) {
    if has(ctx, TRIFFS_RING)
        && has(ctx, POITANS_NOTES)
        && has(ctx, CASIANS_LIST)
        && ctx.quest_items_count(item) < cap
    {
        ctx.give_items(item, 1);
        if casian_set_done(ctx) {
            ctx.set_cond(29, true);
        } else if ctx.quest_items_count(item) >= cap {
            ctx.play_sound(quest_sounds::MIDDLE);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

fn mirien_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_1ST_SIGIL) {
        if !has(ctx, SYMBOL_OF_SYLVAIN) {
            "30461-05.html".to_string()
        } else {
            ctx.take_items(MIRIENS_1ST_SIGIL, 1);
            ctx.give_items(MIRIENS_2ND_SIGIL, 1);
            ctx.take_items(SYMBOL_OF_SYLVAIN, 1);
            ctx.set_cond(15, true);
            "30461-06.html".to_string()
        }
    } else if has(ctx, MIRIENS_2ND_SIGIL) {
        if !has(ctx, SYMBOL_OF_JUREK) {
            "30461-07.html".to_string()
        } else {
            "30461-08.html".to_string()
        }
    } else if has(ctx, MIRIENS_INSTRUCTION) {
        if ctx.player_level() < LEVEL {
            "30461-11.html".to_string()
        } else {
            ctx.take_items(MIRIENS_INSTRUCTION, 1);
            ctx.give_items(MIRIENS_3RD_SIGIL, 1);
            ctx.set_cond(19, true);
            "30461-12.html".to_string()
        }
    } else if has(ctx, MIRIENS_3RD_SIGIL) {
        if !has(ctx, SYMBOL_OF_CRONOS) {
            "30461-13.html".to_string()
        } else {
            ctx.give_adena(319628, true);
            ctx.give_items(MARK_OF_SCHOLAR, 1);
            ctx.add_exp_and_sp(1753926, 113754);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            "30461-14.html".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn sylvain_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_1ST_SIGIL) && !has(ctx, HIGH_PRIESTS_SIGIL) && !has(ctx, SYMBOL_OF_SYLVAIN)
    {
        "30070-01.html".to_string()
    } else if !has(ctx, CRYSTAL_OF_PURITY1)
        && has(ctx, HIGH_PRIESTS_SIGIL)
        && has(ctx, MIRIENS_1ST_SIGIL)
    {
        "30070-03.html".to_string()
    } else if has(ctx, HIGH_PRIESTS_SIGIL)
        && has(ctx, MIRIENS_1ST_SIGIL)
        && has(ctx, CRYSTAL_OF_PURITY1)
    {
        ctx.take_items(CRYSTAL_OF_PURITY1, 1);
        ctx.take_items(HIGH_PRIESTS_SIGIL, 1);
        ctx.give_items(SYMBOL_OF_SYLVAIN, 1);
        ctx.set_cond(14, true);
        "30070-04.html".to_string()
    } else if has(ctx, MIRIENS_1ST_SIGIL) && has(ctx, SYMBOL_OF_SYLVAIN) {
        "30070-05.html".to_string()
    } else if has(ctx, MIRIENS_2ND_SIGIL) || has(ctx, MIRIENS_3RD_SIGIL) {
        "30070-06.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn lucas_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_1ST_SIGIL) && has(ctx, HIGH_PRIESTS_SIGIL) {
        if has(ctx, MARIAS_1ST_LETTER) {
            ctx.take_items(MARIAS_1ST_LETTER, 1);
            ctx.give_items(LUCASS_LETTER, 1);
            ctx.set_cond(4, true);
            "30071-01.html".to_string()
        } else if has(ctx, MARIAS_2ND_LETTER)
            || has(ctx, CRETAS_1ST_LETTER)
            || has(ctx, LUCILLAS_HANDBAG)
            || has(ctx, CRERAS_PAINTING1)
            || has(ctx, LUCASS_LETTER)
        {
            "30071-02.html".to_string()
        } else if has(ctx, CRERAS_PAINTING2) {
            "30071-03.html".to_string()
        } else if has(ctx, CRERAS_PAINTING3) {
            if ctx.quest_items_count(BROWN_SCROLL_SCRAP) < 5 {
                "30071-05.html".to_string()
            } else {
                "30071-06.html".to_string()
            }
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, SYMBOL_OF_SYLVAIN)
        || has(ctx, MIRIENS_2ND_SIGIL)
        || has(ctx, MIRIENS_3RD_SIGIL)
        || has(ctx, CRYSTAL_OF_PURITY1)
    {
        "30071-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn valkon_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, TRIFFS_RING) {
        if !has(ctx, VALKONS_REQUEST)
            && !has(ctx, CRYSTAL_OF_PURITY2)
            && !has(ctx, SCRIPTURE_CHAPTER_2)
        {
            "30103-01.html".to_string()
        } else if has(ctx, VALKONS_REQUEST)
            && !has(ctx, CRYSTAL_OF_PURITY2)
            && !has(ctx, SCRIPTURE_CHAPTER_2)
        {
            // Re-mark Maria on the reminder page, so a player who logged out
            // mid-errand can recover the marker by asking Valkon again.
            ctx.add_quest_radar(MARIA_LOC.0, MARIA_LOC.1, MARIA_LOC.2);
            "30103-05.html".to_string()
        } else if has(ctx, CRYSTAL_OF_PURITY2) && !has(ctx, SCRIPTURE_CHAPTER_2) {
            ctx.give_items(SCRIPTURE_CHAPTER_2, 1);
            ctx.take_items(CRYSTAL_OF_PURITY2, 1);
            "30103-06.html".to_string()
        } else if has(ctx, SCRIPTURE_CHAPTER_2) {
            "30103-07.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn dieter_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_3RD_SIGIL) && has(ctx, CRONOS_SIGIL) {
        if has(ctx, CRONOS_LETTER) {
            "30111-01.html".to_string()
        } else if has(ctx, DIETERS_KEY) {
            "30111-06.html".to_string()
        } else if has(ctx, CRETAS_2ND_LETTER) {
            "30111-07.html".to_string()
        } else if has(ctx, DIETERS_DIARY) && has(ctx, DIETERS_LETTER) {
            "30111-10.html".to_string()
        } else if has(ctx, DIETERS_DIARY) && has(ctx, RAUTS_LETTER_ENVELOPE) {
            "30111-11.html".to_string()
        } else if has(ctx, DIETERS_DIARY) {
            if has(ctx, SCRIPTURE_CHAPTER_1)
                && has(ctx, SCRIPTURE_CHAPTER_2)
                && has(ctx, SCRIPTURE_CHAPTER_3)
                && has(ctx, SCRIPTURE_CHAPTER_4)
            {
                "30111-13.html".to_string()
            } else {
                "30111-12.html".to_string()
            }
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, SYMBOL_OF_CRONOS) {
        "30111-15.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn jurek_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_2ND_SIGIL) {
        if !has(ctx, GRAND_MAGISTER_SIGIL) && !has(ctx, SYMBOL_OF_JUREK) {
            "30115-01.html".to_string()
        } else if has(ctx, JUREKS_LIST) {
            if ctx.quest_items_count(MONSTER_EYE_DESTROYER_SKIN)
                + ctx.quest_items_count(SHAMANS_NECKLACE)
                + ctx.quest_items_count(SHACKLES_SCALP)
                < 12
            {
                "30115-04.html".to_string()
            } else {
                ctx.take_items(GRAND_MAGISTER_SIGIL, 1);
                ctx.take_items(JUREKS_LIST, 1);
                ctx.take_items(MONSTER_EYE_DESTROYER_SKIN, -1);
                ctx.take_items(SHAMANS_NECKLACE, -1);
                ctx.take_items(SHACKLES_SCALP, -1);
                ctx.give_items(SYMBOL_OF_JUREK, 1);
                ctx.set_cond(18, true);
                "30115-05.html".to_string()
            }
        } else if has(ctx, SYMBOL_OF_JUREK) {
            "30115-06.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, MIRIENS_1ST_SIGIL) || has(ctx, MIRIENS_3RD_SIGIL) {
        "30115-07.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn edroc_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, DIETERS_DIARY) {
        if has(ctx, DIETERS_LETTER) {
            "30230-01.html".to_string()
        } else if has(ctx, RAUTS_LETTER_ENVELOPE) {
            "30230-03.html".to_string()
        } else if has(ctx, STRONG_LIGUOR) || has(ctx, TRIFFS_RING) {
            "30230-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn raut_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, DIETERS_DIARY) {
        if has(ctx, RAUTS_LETTER_ENVELOPE) {
            "30316-01.html".to_string()
        } else if has(ctx, SCRIPTURE_CHAPTER_1) && has(ctx, STRONG_LIGUOR) {
            "30316-04.html".to_string()
        } else if has(ctx, SCRIPTURE_CHAPTER_1) && has(ctx, TRIFFS_RING) {
            "30316-05.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn poitan_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, TRIFFS_RING) {
        if !has(ctx, POITANS_NOTES) && !has(ctx, CASIANS_LIST) && !has(ctx, SCRIPTURE_CHAPTER_4) {
            ctx.give_items(POITANS_NOTES, 1);
            "30458-01.html".to_string()
        } else if has(ctx, POITANS_NOTES)
            && !has(ctx, CASIANS_LIST)
            && !has(ctx, SCRIPTURE_CHAPTER_4)
        {
            "30458-02.html".to_string()
        } else if has(ctx, POITANS_NOTES)
            && has(ctx, CASIANS_LIST)
            && !has(ctx, SCRIPTURE_CHAPTER_4)
        {
            "30458-03.html".to_string()
        } else if has(ctx, SCRIPTURE_CHAPTER_4) {
            "30458-04.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn maria_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_1ST_SIGIL) && has(ctx, HIGH_PRIESTS_SIGIL) {
        if has(ctx, SYLVAINS_LETTER) {
            "30608-01.html".to_string()
        } else if has(ctx, MARIAS_1ST_LETTER) {
            "30608-03.html".to_string()
        } else if has(ctx, LUCASS_LETTER) {
            ctx.give_items(MARIAS_2ND_LETTER, 1);
            ctx.take_items(LUCASS_LETTER, 1);
            ctx.set_cond(5, true);
            "30608-04.html".to_string()
        } else if has(ctx, MARIAS_2ND_LETTER) {
            "30608-05.html".to_string()
        } else if has(ctx, CRETAS_1ST_LETTER) {
            "30608-06.html".to_string()
        } else if has(ctx, LUCILLAS_HANDBAG) {
            "30608-09.html".to_string()
        } else if has(ctx, CRERAS_PAINTING1) {
            ctx.take_items(CRERAS_PAINTING1, 1);
            ctx.give_items(CRERAS_PAINTING2, 1);
            ctx.set_cond(9, true);
            "30608-10.html".to_string()
        } else if has(ctx, CRERAS_PAINTING2) {
            "30608-11.html".to_string()
        } else if has(ctx, CRERAS_PAINTING3) {
            if ctx.quest_items_count(BROWN_SCROLL_SCRAP) < 5 {
                ctx.set_cond(11, true);
                "30608-12.html".to_string()
            } else {
                "30608-13.html".to_string()
            }
        } else if has(ctx, CRYSTAL_OF_PURITY1) {
            "30608-15.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, SYMBOL_OF_SYLVAIN) || has(ctx, MIRIENS_2ND_SIGIL) {
        "30608-16.html".to_string()
    } else if has(ctx, MIRIENS_3RD_SIGIL) {
        if !has(ctx, VALKONS_REQUEST) {
            "30608-17.html".to_string()
        } else {
            ctx.take_items(VALKONS_REQUEST, 1);
            ctx.give_items(CRYSTAL_OF_PURITY2, 1);
            // Errand done — retire the marker that pointed here (Q348's pattern).
            ctx.clear_radar();
            "30608-18.html".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn creta_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, MIRIENS_1ST_SIGIL) && has(ctx, HIGH_PRIESTS_SIGIL) {
        if has(ctx, MARIAS_2ND_LETTER) {
            "30609-01.html".to_string()
        } else if has(ctx, CRETAS_1ST_LETTER) {
            "30609-06.html".to_string()
        } else if has(ctx, LUCILLAS_HANDBAG) {
            "30609-07.html".to_string()
        } else if has(ctx, CRERAS_PAINTING1)
            || has(ctx, CRERAS_PAINTING2)
            || has(ctx, CRERAS_PAINTING3)
        {
            "30609-10.html".to_string()
        } else {
            ctx.no_quest_html()
        }
    } else if has(ctx, CRYSTAL_OF_PURITY1)
        || has(ctx, SYMBOL_OF_SYLVAIN)
        || has(ctx, MIRIENS_2ND_SIGIL)
    {
        "30609-11.html".to_string()
    } else if has(ctx, MIRIENS_3RD_SIGIL) {
        if has(ctx, DIETERS_KEY) {
            "30609-12.html".to_string()
        } else {
            "30609-15.html".to_string()
        }
    } else {
        ctx.no_quest_html()
    }
}

fn cronos_talk(ctx: &mut QuestCtx) -> String {
    if !has(ctx, MIRIENS_3RD_SIGIL) {
        return ctx.no_quest_html();
    }
    if !has(ctx, CRONOS_SIGIL) && !has(ctx, SYMBOL_OF_CRONOS) {
        "30610-01.html".to_string()
    } else if has(ctx, CRONOS_SIGIL) {
        if has(ctx, SCRIPTURE_CHAPTER_1)
            && has(ctx, SCRIPTURE_CHAPTER_2)
            && has(ctx, SCRIPTURE_CHAPTER_3)
            && has(ctx, SCRIPTURE_CHAPTER_4)
        {
            "30610-12.html".to_string()
        } else {
            "30610-11.html".to_string()
        }
    } else if has(ctx, SYMBOL_OF_CRONOS) {
        "30610-15.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn triff_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, DIETERS_DIARY) && has(ctx, SCRIPTURE_CHAPTER_1) && has(ctx, STRONG_LIGUOR) {
        "30611-01.html".to_string()
    } else if has(ctx, TRIFFS_RING) || has(ctx, SYMBOL_OF_CRONOS) {
        "30611-05.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}

fn casian_talk(ctx: &mut QuestCtx) -> String {
    if has(ctx, TRIFFS_RING) && has(ctx, POITANS_NOTES) {
        if !has(ctx, CASIANS_LIST) {
            if has(ctx, SCRIPTURE_CHAPTER_1)
                && has(ctx, SCRIPTURE_CHAPTER_2)
                && has(ctx, SCRIPTURE_CHAPTER_3)
            {
                "30612-02.html".to_string()
            } else {
                // Casian's refusal *is* the journal step: cond 27 ("Elder Casian
                // regrets that he cannot understand the contents from only one
                // chapter") is what points the player at the chapters they still
                // owe — Valkon in Giran for chapter 2, Grandis in Death Pass for
                // chapter 3. The Mobius Classic-Interlude script this quest was
                // ported from drops the `setCond(27)` that `L2J_Mobius_CT_0_Interlude`
                // fires here, leaving the journal stuck on cond 26 with no guidance
                // (cond 27 is the only step in 1..=31 the script never sets).
                if ctx.is_cond(26) {
                    ctx.set_cond(27, true);
                }
                "30612-01.html".to_string()
            }
        } else if ctx.quest_items_count(GHOULS_SKIN)
            + ctx.quest_items_count(MEDUSAS_BLOOD)
            + ctx.quest_items_count(FETTERED_SOULS_ICHOR)
            + ctx.quest_items_count(ENCHANTED_GARGOYLES_NAIL)
            < 32
        {
            "30612-05.html".to_string()
        } else {
            "30612-06.html".to_string()
        }
    } else if has(ctx, TRIFFS_RING)
        && has(ctx, SCRIPTURE_CHAPTER_1)
        && has(ctx, SCRIPTURE_CHAPTER_2)
        && has(ctx, SCRIPTURE_CHAPTER_3)
        && has(ctx, SCRIPTURE_CHAPTER_4)
        && !has(ctx, POITANS_NOTES)
        && !has(ctx, CASIANS_LIST)
    {
        "30612-08.html".to_string()
    } else {
        ctx.no_quest_html()
    }
}
