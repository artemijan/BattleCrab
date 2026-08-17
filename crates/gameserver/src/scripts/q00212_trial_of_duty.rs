//! Trial of Duty (212) — `quests/Q00212_TrialOfDuty`. The `KNIGHT_GROUP` trial
//! (level 35+). Hannavalt and Dustin send the knight through a long chain of
//! spirit-hunts: prove worth to Sir Herod's spirit with the Old Knight's Sword,
//! gather Talianus's scattered report, collect militia articles and Athebaldt's
//! bones, and carry a relay of letters back for the Mark of Duty.
//!
//! `memoState`-driven (states 1..14). Three mechanics of note:
//!   * An escalating **"flag" counter** (a named quest var) drives two rare
//!     spawns — each qualifying kill rolls `flag * 10` (Sir Herod off skeletons)
//!     or `(flag - 3) * 33` (Sir Talianus off Hangman Trees) percent to conjure
//!     the spirit and reset the flag, otherwise the flag climbs.
//!   * Sir Herod's spirit only yields the Knight's Tear when slain **with the
//!     Old Knight's Sword equipped** (`getActiveWeaponItem`).
//!   * Two collect legs use `giveItemRandomly` (10 report pieces, 20 militia
//!     articles). NB: militia article and Saint's Ashes Urn share item id 2641.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const HANNAVALT: i32 = 30109;
const DUSTIN: i32 = 30116;
const SIR_COLLIN_WINDAWOOD: i32 = 30311;
const SIR_ARON_TANFORD: i32 = 30653;
const SIR_KIEL_NIGHTHAWK: i32 = 30654;
const ISAEL_SILVERSHADOW: i32 = 30655;
const SPIRIT_OF_SIR_TALIANUS: i32 = 30656;
// Items
const LETTER_OF_DUSTIN: i32 = 2634;
const KNIGHTS_TEAR: i32 = 2635;
const MIRROR_OF_ORPIC: i32 = 2636;
const TEAR_OF_CONFESSION: i32 = 2637;
const REPORT_PIECE: i32 = 2638;
const TALIANUSS_REPORT: i32 = 2639;
const TEAR_OF_LOYALTY: i32 = 2640;
const MILITAS_ARTICLE: i32 = 2641;
const SAINTS_ASHES_URN: i32 = 2641; // shares the militia-article item id
const ATHEBALDTS_SKULL: i32 = 2643;
const ATHEBALDTS_RIBS: i32 = 2644;
const ATHEBALDTS_SHIN: i32 = 2645;
const LETTER_OF_WINDAWOOD: i32 = 2646;
const OLD_KNIGHTS_SWORD: i32 = 3027;
// Monsters
const HANGMAN_TREE: i32 = 20144;
const SKELETON_MARAUDER: i32 = 20190;
const SKELETON_RAIDER: i32 = 20191;
const STRAIN: i32 = 20200;
const GHOUL: i32 = 20201;
const BREKA_ORC_PREFECT: i32 = 20270;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const SPIRIT_OF_SIR_HEROD: i32 = 27119;
// Reward
const MARK_OF_DUTY: i32 = 2633;
// Misc
const MIN_LEVEL: i32 = 35;
const REPORT_PIECE_LIMIT: i64 = 10;
const MILITAS_LIMIT: i64 = 20;

fn flag(ctx: &QuestCtx) -> i32 {
    ctx.get_var("flag")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

pub struct Q00212TrialOfDuty;

impl QuestScript for Q00212TrialOfDuty {
    fn id(&self) -> i32 {
        212
    }
    fn name(&self) -> &'static str {
        "Q00212_TrialOfDuty"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00212_TrialOfDuty"
    }
    fn start_npcs(&self) -> &[i32] {
        &[HANNAVALT]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            HANNAVALT,
            DUSTIN,
            SIR_COLLIN_WINDAWOOD,
            SIR_ARON_TANFORD,
            SIR_KIEL_NIGHTHAWK,
            ISAEL_SILVERSHADOW,
            SPIRIT_OF_SIR_TALIANUS,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            HANGMAN_TREE,
            SKELETON_MARAUDER,
            SKELETON_RAIDER,
            STRAIN,
            GHOUL,
            BREKA_ORC_PREFECT,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            SPIRIT_OF_SIR_HEROD,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            LETTER_OF_DUSTIN,
            KNIGHTS_TEAR,
            MIRROR_OF_ORPIC,
            TEAR_OF_CONFESSION,
            REPORT_PIECE,
            TALIANUSS_REPORT,
            TEAR_OF_LOYALTY,
            MILITAS_ARTICLE,
            ATHEBALDTS_SKULL,
            ATHEBALDTS_RIBS,
            ATHEBALDTS_SHIN,
            LETTER_OF_WINDAWOOD,
            OLD_KNIGHTS_SWORD,
        ]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "quest_accept" => {
                if ctx.is_created()
                    && ctx.player_level() >= MIN_LEVEL
                    && ctx.is_in_category("KNIGHT_GROUP")
                {
                    ctx.start_quest();
                    ctx.set_memo_state(1);
                    ctx.set_var("flag", "0");
                }
                None
            }
            "30116-02.html" | "30116-03.html" | "30116-04.html" => {
                if ctx.memo_state() == 10 && ctx.quest_items_count(TEAR_OF_LOYALTY) > 0 {
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30116-05.html" => {
                if ctx.memo_state() == 10 && ctx.quest_items_count(TEAR_OF_LOYALTY) > 0 {
                    ctx.take_items(TEAR_OF_LOYALTY, -1);
                    ctx.set_memo_state(11);
                    ctx.set_cond(14, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        match ctx.npc_id {
            SKELETON_MARAUDER | SKELETON_RAIDER => {
                if ctx.memo_state() == 2 {
                    let f = flag(ctx);
                    if ctx.roll(100) < f * 10 {
                        ctx.spawn_near_npc(SPIRIT_OF_SIR_HEROD, false);
                        ctx.set_var("flag", "0");
                    } else {
                        ctx.set_var("flag", (f + 1).to_string());
                    }
                }
            }
            SPIRIT_OF_SIR_HEROD => {
                if ctx.memo_state() == 2 && ctx.equipped_weapon_id() == OLD_KNIGHTS_SWORD {
                    ctx.give_items(KNIGHTS_TEAR, 1);
                    ctx.set_memo_state(3);
                    ctx.set_cond(3, true);
                }
            }
            STRAIN | GHOUL => {
                if ctx.memo_state() == 5
                    && ctx.quest_items_count(TALIANUSS_REPORT) == 0
                    && ctx.give_item_randomly(REPORT_PIECE, 1, REPORT_PIECE_LIMIT, 1.0, true)
                {
                    ctx.take_items(REPORT_PIECE, -1);
                    ctx.give_items(TALIANUSS_REPORT, 1);
                    ctx.set_cond(6, false);
                }
            }
            HANGMAN_TREE => {
                if ctx.memo_state() == 6 {
                    let f = flag(ctx);
                    if ctx.roll(100) < (f - 3) * 33 {
                        ctx.spawn_near_npc(SPIRIT_OF_SIR_TALIANUS, false);
                        ctx.set_var("flag", "0");
                        ctx.set_cond(8, true);
                    } else {
                        ctx.set_var("flag", (f + 1).to_string());
                    }
                }
            }
            LETO_LIZARDMAN
            | LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR
            | LETO_LIZARDMAN_SHAMAN
            | LETO_LIZARDMAN_OVERLORD => {
                if ctx.memo_state() == 9
                    && ctx.give_item_randomly(MILITAS_ARTICLE, 1, MILITAS_LIMIT, 1.0, true)
                {
                    ctx.set_cond(12, false);
                }
            }
            BREKA_ORC_PREFECT if ctx.memo_state() == 11 => {
                if ctx.quest_items_count(ATHEBALDTS_SKULL) == 0 {
                    ctx.give_items(ATHEBALDTS_SKULL, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                } else if ctx.quest_items_count(ATHEBALDTS_RIBS) == 0 {
                    ctx.give_items(ATHEBALDTS_RIBS, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                } else if ctx.quest_items_count(ATHEBALDTS_SHIN) == 0 {
                    ctx.give_items(ATHEBALDTS_SHIN, 1);
                    ctx.set_cond(15, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let memo = ctx.memo_state();
        match ctx.npc_id {
            HANNAVALT => {
                if ctx.is_created() {
                    if !ctx.is_in_category("KNIGHT_GROUP") {
                        Some("30109-02.html".to_string())
                    } else if ctx.player_level() < MIN_LEVEL {
                        Some("30109-01.html".to_string())
                    } else {
                        Some("30109-03.htm".to_string())
                    }
                } else if ctx.is_completed() {
                    Some(ctx.already_completed_html())
                } else {
                    match memo {
                        1 => Some("30109-04.html".to_string()),
                        14 if ctx.quest_items_count(LETTER_OF_DUSTIN) > 0 => {
                            ctx.take_items(LETTER_OF_DUSTIN, -1);
                            ctx.add_exp_and_sp(762576, 49458);
                            ctx.give_adena(138968, true);
                            ctx.give_items(MARK_OF_DUTY, 1);
                            ctx.exit_quest(false, true);
                            ctx.social_action(3);
                            Some("30109-05.html".to_string())
                        }
                        _ => Some(ctx.no_quest_html()),
                    }
                }
            }
            SIR_ARON_TANFORD => match memo {
                1 => {
                    if ctx.quest_items_count(OLD_KNIGHTS_SWORD) == 0 {
                        ctx.give_items(OLD_KNIGHTS_SWORD, 1);
                    }
                    ctx.set_memo_state(2);
                    ctx.set_cond(2, true);
                    Some("30653-01.html".to_string())
                }
                2 if ctx.quest_items_count(OLD_KNIGHTS_SWORD) > 0 => {
                    Some("30653-02.html".to_string())
                }
                3 if ctx.quest_items_count(KNIGHTS_TEAR) > 0 => {
                    ctx.take_items(KNIGHTS_TEAR, -1);
                    ctx.take_items(OLD_KNIGHTS_SWORD, -1);
                    ctx.set_memo_state(4);
                    ctx.set_cond(4, true);
                    Some("30653-03.html".to_string())
                }
                4 => Some("30653-04.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            SIR_KIEL_NIGHTHAWK => match memo {
                4 => {
                    ctx.set_memo_state(5);
                    ctx.set_cond(5, true);
                    Some("30654-01.html".to_string())
                }
                5 => {
                    if ctx.quest_items_count(TALIANUSS_REPORT) == 0 {
                        Some("30654-02.html".to_string())
                    } else {
                        ctx.set_memo_state(6);
                        ctx.set_cond(7, true);
                        ctx.give_items(MIRROR_OF_ORPIC, 1);
                        Some("30654-03.html".to_string())
                    }
                }
                6 if ctx.quest_items_count(MIRROR_OF_ORPIC) > 0 => {
                    Some("30654-04.html".to_string())
                }
                7 if ctx.quest_items_count(TEAR_OF_CONFESSION) > 0 => {
                    ctx.take_items(TEAR_OF_CONFESSION, -1);
                    ctx.set_memo_state(8);
                    ctx.set_cond(10, true);
                    Some("30654-05.html".to_string())
                }
                8 => Some("30654-06.html".to_string()),
                _ => Some(ctx.no_quest_html()),
            },
            SPIRIT_OF_SIR_TALIANUS => {
                if memo == 6
                    && ctx.quest_items_count(MIRROR_OF_ORPIC) > 0
                    && ctx.quest_items_count(TALIANUSS_REPORT) > 0
                {
                    ctx.take_items(MIRROR_OF_ORPIC, -1);
                    ctx.take_items(TALIANUSS_REPORT, -1);
                    ctx.give_items(TEAR_OF_CONFESSION, 1);
                    ctx.set_memo_state(7);
                    ctx.set_cond(9, true);
                    ctx.delete_npc();
                    Some("30656-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            ISAEL_SILVERSHADOW => match memo {
                8 => {
                    if ctx.player_level() < MIN_LEVEL {
                        Some("30655-01.html".to_string())
                    } else {
                        ctx.set_memo_state(9);
                        ctx.set_cond(11, true);
                        Some("30655-02.html".to_string())
                    }
                }
                9 => {
                    if ctx.quest_items_count(MILITAS_ARTICLE) < MILITAS_LIMIT {
                        Some("30655-03.html".to_string())
                    } else {
                        ctx.give_items(TEAR_OF_LOYALTY, 1);
                        ctx.take_items(MILITAS_ARTICLE, MILITAS_LIMIT);
                        ctx.set_memo_state(10);
                        ctx.set_cond(13, true);
                        Some("30655-04.html".to_string())
                    }
                }
                10 if ctx.quest_items_count(TEAR_OF_LOYALTY) > 0 => {
                    Some("30655-05.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            DUSTIN => match memo {
                10 if ctx.quest_items_count(TEAR_OF_LOYALTY) > 0 => {
                    Some("30116-01.html".to_string())
                }
                11 => {
                    if ctx.quest_items_count(ATHEBALDTS_SKULL) == 0
                        || ctx.quest_items_count(ATHEBALDTS_RIBS) == 0
                        || ctx.quest_items_count(ATHEBALDTS_SHIN) == 0
                    {
                        Some("30116-06.html".to_string())
                    } else {
                        ctx.take_items(ATHEBALDTS_SKULL, -1);
                        ctx.take_items(ATHEBALDTS_RIBS, -1);
                        ctx.take_items(ATHEBALDTS_SHIN, -1);
                        ctx.give_items(SAINTS_ASHES_URN, 1);
                        ctx.set_memo_state(12);
                        ctx.set_cond(16, true);
                        Some("30116-07.html".to_string())
                    }
                }
                12 if ctx.quest_items_count(SAINTS_ASHES_URN) > 0 => {
                    Some("30116-09.html".to_string())
                }
                13 if ctx.quest_items_count(LETTER_OF_WINDAWOOD) > 0 => {
                    ctx.take_items(LETTER_OF_WINDAWOOD, -1);
                    ctx.give_items(LETTER_OF_DUSTIN, 1);
                    ctx.set_memo_state(14);
                    ctx.set_cond(18, true);
                    Some("30116-08.html".to_string())
                }
                14 if ctx.quest_items_count(LETTER_OF_DUSTIN) > 0 => {
                    Some("30116-10.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            SIR_COLLIN_WINDAWOOD => match memo {
                12 if ctx.quest_items_count(SAINTS_ASHES_URN) > 0 => {
                    ctx.take_items(SAINTS_ASHES_URN, -1);
                    ctx.give_items(LETTER_OF_WINDAWOOD, 1);
                    ctx.set_memo_state(13);
                    ctx.set_cond(17, true);
                    Some("30311-01.html".to_string())
                }
                13 if ctx.quest_items_count(LETTER_OF_WINDAWOOD) > 0 => {
                    Some("30311-02.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            },
            _ => Some(ctx.no_quest_html()),
        }
    }
}
