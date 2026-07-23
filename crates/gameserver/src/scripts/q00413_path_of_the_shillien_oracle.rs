//! Path Of The Shillien Oracle (413) — port of
//! `dist/game/data/scripts/quests/Q00413_PathOfTheShillienOracle/`.
//!
//! Awards the **Orb of Abyss** (1270), the last of `DarkElfChange1`'s four
//! proofs. **The Dark Elf first-occupation tier is complete with this.**
//!
//! Sidra → Talbot (five blank sheets) → five Dark Succubi → Talbot again
//! (Garmiel's Book + Adonius' prayer) → Adonius (penitent's mark) → ten ashen
//! bones → Adonius (Andariel's Book) → Sidra.
//!
//! ## The succubus kill is a swap, not a drop
//!
//! Every other collection in the Path family *adds* an item. This one
//! **consumes** one: each Dark Succubus takes a Blank Sheet and gives a Bloody
//! Rune back, so the two counts move in opposite directions and the stage ends
//! when the sheets run out:
//!
//! ```java
//! giveItems(killer, BLOODY_RUNE, 1);
//! takeItems(killer, BLANK_SHEET, 1);
//! if (!hasQuestItems(killer, BLANK_SHEET) && (getQuestItemsCount(killer, BLOODY_RUNE) == 5))
//! ```
//!
//! Modelling it as a plain capped drop would leave five sheets in the bag
//! forever and never fire the cond, because the cond tests *both* — sheets
//! exhausted **and** five runes. Tested in both directions.
//!
//! Neither drop rolls a chance: five succubi is five runes, ten undead is ten
//! bones. (Its sibling 412 rolls a coin flip on all three of its drops — the
//! conventions differ quest by quest even inside one race tier.)
//!
//! Talbot hands over **five** blank sheets in a single `giveItems(..., 5)`,
//! the same stack-not-singleton shape as Simplon in quest 405.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const MAGISTER_SIDRA: i32 = 30330;
const PRIEST_ADONIUS: i32 = 30375;
const MAGISTER_TALBOT: i32 = 30377;

const SIDRAS_LETTER: i32 = 1262;
const BLANK_SHEET: i32 = 1263;
const BLOODY_RUNE: i32 = 1264;
const GARMIELS_BOOK: i32 = 1265;
const PRAYER_OF_ADONIUS: i32 = 1266;
const PENITENTS_MARK: i32 = 1267;
const ASHEN_BONES: i32 = 1268;
const ANDARIEL_BOOK: i32 = 1269;
const ORB_OF_ABYSS: i32 = 1270;

const ZOMBIE_SOLDIER: i32 = 20457;
const ZOMBIE_WARRIOR: i32 = 20458;
const SHIELD_SKELETON: i32 = 20514;
const SKELETON_INFANTRYMAN: i32 = 20515;
const DARK_SUCCUBUS: i32 = 20776;

const DARK_MAGE: i32 = 38;
const SHILLIEN_ORACLE: i32 = 43;
const MIN_LEVEL: i32 = 19;

const SHEETS: i64 = 5;
const RUNES_NEEDED: i64 = 5;
const BONES_NEEDED: i64 = 10;

const UNDEAD: [i32; 4] = [
    ZOMBIE_SOLDIER,
    ZOMBIE_WARRIOR,
    SHIELD_SKELETON,
    SKELETON_INFANTRYMAN,
];
const KILL_NPCS: [i32; 5] = [
    ZOMBIE_SOLDIER,
    ZOMBIE_WARRIOR,
    SHIELD_SKELETON,
    SKELETON_INFANTRYMAN,
    DARK_SUCCUBUS,
];

const QUEST_ITEMS: [i32; 8] = [
    SIDRAS_LETTER,
    BLANK_SHEET,
    BLOODY_RUNE,
    GARMIELS_BOOK,
    PRAYER_OF_ADONIUS,
    PENITENTS_MARK,
    ASHEN_BONES,
    ANDARIEL_BOOK,
];

pub struct Q00413PathOfTheShillienOracle;

impl Q00413PathOfTheShillienOracle {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn has_any(&self, ctx: &QuestCtx, items: &[i32]) -> bool {
        items.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }
}

impl QuestScript for Q00413PathOfTheShillienOracle {
    fn id(&self) -> i32 {
        413
    }
    fn name(&self) -> &'static str {
        "Q00413_PathOfTheShillienOracle"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00413_PathOfTheShillienOracle"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MAGISTER_SIDRA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MAGISTER_SIDRA, PRIEST_ADONIUS, MAGISTER_TALBOT]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(
                match ctx.player_class_id() {
                    DARK_MAGE if ctx.player_level() < MIN_LEVEL => "30330-02.htm",
                    DARK_MAGE if self.has(ctx, ORB_OF_ABYSS) => "30330-04.htm",
                    DARK_MAGE => "30330-05.htm",
                    SHILLIEN_ORACLE => "30330-02a.htm",
                    _ => "30330-03.htm",
                }
                .to_string(),
            ),
            "30330-06.htm" => {
                if !self.has(ctx, SIDRAS_LETTER) {
                    ctx.give_items(SIDRAS_LETTER, 1);
                }
                ctx.start_quest();
                Some(event.to_string())
            }
            "30330-06a.html" | "30375-02.html" | "30375-03.html" => Some(event.to_string()),
            // Adonius: the prayer becomes the penitent's mark.
            "30375-04.html" => {
                if self.has(ctx, PRAYER_OF_ADONIUS) {
                    ctx.take_items(PRAYER_OF_ADONIUS, 1);
                    ctx.give_items(PENITENTS_MARK, 1);
                    ctx.set_cond(5, true);
                }
                // Java echoes the page either way.
                Some(event.to_string())
            }
            // Talbot: Sidra's letter becomes *five* blank sheets.
            "30377-02.html" => {
                if self.has(ctx, SIDRAS_LETTER) {
                    ctx.take_items(SIDRAS_LETTER, 1);
                    ctx.give_items(BLANK_SHEET, SHEETS);
                    ctx.set_cond(2, true);
                }
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        if UNDEAD.contains(&npc_id) {
            if !self.has(ctx, PENITENTS_MARK) || ctx.quest_items_count(ASHEN_BONES) >= BONES_NEEDED
            {
                return;
            }
            ctx.give_items(ASHEN_BONES, 1);
            if ctx.quest_items_count(ASHEN_BONES) == BONES_NEEDED {
                ctx.set_cond(6, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
            return;
        }
        if npc_id == DARK_SUCCUBUS && self.has(ctx, BLANK_SHEET) {
            // A swap, not a drop: the sheet is spent to make the rune.
            ctx.give_items(BLOODY_RUNE, 1);
            ctx.take_items(BLANK_SHEET, 1);
            if !self.has(ctx, BLANK_SHEET) && ctx.quest_items_count(BLOODY_RUNE) == RUNES_NEEDED {
                ctx.set_cond(3, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == MAGISTER_SIDRA {
                return Some("30330-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            MAGISTER_SIDRA => self.talk_sidra(ctx),
            PRIEST_ADONIUS => self.talk_adonius(ctx),
            MAGISTER_TALBOT => self.talk_talbot(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00413PathOfTheShillienOracle {
    fn talk_sidra(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, SIDRAS_LETTER) {
            return Some("30330-07.html".to_string());
        }
        if self.has_any(ctx, &[BLANK_SHEET, BLOODY_RUNE]) {
            return Some("30330-08.html".to_string());
        }
        if !self.has(ctx, ANDARIEL_BOOK)
            && self.has_any(
                ctx,
                &[
                    PRAYER_OF_ADONIUS,
                    GARMIELS_BOOK,
                    PENITENTS_MARK,
                    ASHEN_BONES,
                ],
            )
        {
            return Some("30330-09.html".to_string());
        }
        if self.has_any(ctx, &[ANDARIEL_BOOK, GARMIELS_BOOK]) {
            ctx.give_items(ORB_OF_ABYSS, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30330-10.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_adonius(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, PRAYER_OF_ADONIUS) {
            return Some("30375-01.html".to_string());
        }
        if self.has(ctx, PENITENTS_MARK) {
            if !self.has_any(ctx, &[ASHEN_BONES, ANDARIEL_BOOK]) {
                return Some("30375-05.html".to_string());
            }
            if ctx.quest_items_count(ASHEN_BONES) < BONES_NEEDED {
                return Some("30375-06.html".to_string());
            }
            ctx.take_items(PENITENTS_MARK, 1);
            ctx.take_items(ASHEN_BONES, -1);
            ctx.give_items(ANDARIEL_BOOK, 1);
            ctx.set_cond(7, true);
            return Some("30375-07.html".to_string());
        }
        if self.has(ctx, ANDARIEL_BOOK) {
            return Some("30375-08.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_talbot(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, SIDRAS_LETTER) {
            return Some("30377-01.html".to_string());
        }
        let runes = ctx.quest_items_count(BLOODY_RUNE);
        if runes == 0 && ctx.quest_items_count(BLANK_SHEET) == SHEETS {
            return Some("30377-03.html".to_string());
        }
        if runes > 0 && runes < RUNES_NEEDED {
            return Some("30377-04.html".to_string());
        }
        if runes >= RUNES_NEEDED {
            ctx.take_items(BLOODY_RUNE, -1);
            ctx.give_items(GARMIELS_BOOK, 1);
            ctx.give_items(PRAYER_OF_ADONIUS, 1);
            ctx.set_cond(4, true);
            return Some("30377-05.html".to_string());
        }
        if self.has_any(ctx, &[PRAYER_OF_ADONIUS, PENITENTS_MARK, ASHEN_BONES]) {
            return Some("30377-06.html".to_string());
        }
        if self.has(ctx, ANDARIEL_BOOK) && self.has(ctx, GARMIELS_BOOK) {
            return Some("30377-07.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
