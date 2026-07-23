//! Elrokian Hunter's Proof (111) — `quests/Q00111_ElrokianHuntersProof`. The
//! deep Primeval-Isle chain: Marquez (32113, level 75+) walks the player through
//! the Elroki hunters via Mushika (32114), Asamah (32115) and Kirikachin
//! (32116) in a **12-step `memoState` machine** — gather 50 Diary Fragments,
//! learn the Elroki flute, then hunt 10 each of Ornithomimus Claws / Deinonychus
//! Bones / Pachycephalosaurus Skins into a Practice Elrokian Trap, redeemed for
//! the real Elrokian Trap + 100 Trap Stones + a fat reward.
//!
//! `memoState` (1–12) is the real progress axis; `cond` mirrors it for the UI
//! and deliberately skips values (some steps advance only the memo). The two
//! collection stages key their drop off **`ItemChanceHolder.count == memoState`**
//! (a stage selector, not a quantity): Diary Fragments (count 4) drop at memo 4,
//! the claw/bone/skin (count 11) at memo 11.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const MARQUEZ: i32 = 32113;
const MUSHIKA: i32 = 32114;
const ASAMAH: i32 = 32115;
const KIRIKACHIN: i32 = 32116;

const ELROKIAN_TRAP: i32 = 8763;
const TRAP_STONE: i32 = 8764;
const DIARY_FRAGMENT: i32 = 8768;
const EXPEDITION_MEMBERS_LETTER: i32 = 8769;
const ORNITHOMINUS_CLAW: i32 = 8770;
const DEINONYCHUS_BONE: i32 = 8771;
const PACHYCEPHALOSAURUS_SKIN: i32 = 8772;
const PRACTICE_ELROKIAN_TRAP: i32 = 8773;
const MIN_LEVEL: i32 = 75;

const KILL_NPCS: [i32; 20] = [
    22196, 22197, 22198, 22218, 22223, // velociraptors → diary fragment
    22200, 22201, 22202, 22219, 22224, // ornithomimus → claw
    22203, 22204, 22205, 22220, 22225, // deinonychus → bone
    22208, 22209, 22210, 22221, 22226, // pachycephalosaurus → skin
];

/// `MOBS_DROP_CHANCES`: npc → (item, chance 0..1, count). `count` is the
/// `memoState` at which the item drops (4 = diary stage, 11 = trophy stage).
fn drop_for(npc_id: i32) -> Option<(i32, f64, i32)> {
    let v = match npc_id {
        22196 => (DIARY_FRAGMENT, 0.51, 4),
        22197 => (DIARY_FRAGMENT, 0.51, 4),
        22198 => (DIARY_FRAGMENT, 0.51, 4),
        22218 => (DIARY_FRAGMENT, 0.25, 4),
        22223 => (DIARY_FRAGMENT, 0.26, 4),
        22200 => (ORNITHOMINUS_CLAW, 0.66, 11),
        22201 => (ORNITHOMINUS_CLAW, 0.33, 11),
        22202 => (ORNITHOMINUS_CLAW, 0.66, 11),
        22219 => (ORNITHOMINUS_CLAW, 0.33, 11),
        22224 => (ORNITHOMINUS_CLAW, 0.33, 11),
        22203 => (DEINONYCHUS_BONE, 0.65, 11),
        22204 => (DEINONYCHUS_BONE, 0.32, 11),
        22205 => (DEINONYCHUS_BONE, 0.66, 11),
        22220 => (DEINONYCHUS_BONE, 0.32, 11),
        22225 => (DEINONYCHUS_BONE, 0.32, 11),
        22208 => (PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
        22209 => (PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
        22210 => (PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
        22221 => (PACHYCEPHALOSAURUS_SKIN, 0.49, 11),
        22226 => (PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
        _ => return None,
    };
    Some(v)
}

/// Has ≥10 of each of the three final trophies.
fn has_trophies(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(ORNITHOMINUS_CLAW) >= 10
        && ctx.quest_items_count(DEINONYCHUS_BONE) >= 10
        && ctx.quest_items_count(PACHYCEPHALOSAURUS_SKIN) >= 10
}

pub struct Q00111ElrokianHuntersProof;

impl QuestScript for Q00111ElrokianHuntersProof {
    fn id(&self) -> i32 {
        111
    }
    fn name(&self) -> &'static str {
        "Q00111_ElrokianHuntersProof"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00111_ElrokianHuntersProof"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MARQUEZ]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MARQUEZ, MUSHIKA, ASAMAH, KIRIKACHIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn quest_items(&self) -> &[i32] {
        &[
            DIARY_FRAGMENT,
            EXPEDITION_MEMBERS_LETTER,
            ORNITHOMINUS_CLAW,
            DEINONYCHUS_BONE,
            PACHYCEPHALOSAURUS_SKIN,
            PRACTICE_ELROKIAN_TRAP,
        ]
    }

    /// `addCondMinLevel(75, "32113-06.htm")`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() < MIN_LEVEL).then(|| "32113-06.htm".to_string())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            // Pure navigation pages.
            "32113-02.htm" | "32113-05.htm" | "32113-04.html" | "32113-10.html"
            | "32113-11.html" | "32113-12.html" | "32113-13.html" | "32113-14.html"
            | "32113-18.html" | "32113-19.html" | "32113-20.html" | "32113-21.html"
            | "32113-22.html" | "32113-23.html" | "32113-24.html" | "32115-08.html"
            | "32116-03.html" => Some(event.to_string()),
            "32113-03.html" => {
                ctx.start_quest();
                ctx.set_memo_state(1);
                Some(event.to_string())
            }
            "32113-15.html" => {
                if ctx.memo_state() == 3 {
                    ctx.set_memo_state(4);
                    ctx.set_cond(4, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32113-25.html" => {
                if ctx.memo_state() == 5 {
                    ctx.set_memo_state(6);
                    ctx.set_cond(6, true);
                    ctx.give_items(EXPEDITION_MEMBERS_LETTER, 1);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32115-03.html" => {
                if ctx.memo_state() == 2 {
                    ctx.set_memo_state(3);
                    ctx.set_cond(3, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32115-06.html" => {
                if ctx.memo_state() == 9 {
                    ctx.set_memo_state(10);
                    ctx.set_cond(9, false);
                    ctx.play_sound(quest_sounds::ELROKI_SONG_FULL);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32115-09.html" => {
                if ctx.memo_state() == 10 {
                    ctx.set_memo_state(11);
                    ctx.set_cond(10, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32116-04.html" => {
                if ctx.memo_state() == 7 {
                    ctx.set_memo_state(8);
                    ctx.play_sound(quest_sounds::ELROKI_SONG_FULL);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32116-07.html" => {
                if ctx.memo_state() == 8 {
                    ctx.set_memo_state(9);
                    ctx.set_cond(8, true);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "32116-10.html" => {
                if ctx.memo_state() == 12 && ctx.quest_items_count(PRACTICE_ELROKIAN_TRAP) > 0 {
                    if ctx.player_level() >= MIN_LEVEL {
                        ctx.take_items(PRACTICE_ELROKIAN_TRAP, -1);
                        ctx.give_items(ELROKIAN_TRAP, 1);
                        ctx.give_items(TRAP_STONE, 100);
                        ctx.give_adena(1702800, true);
                        ctx.add_exp_and_sp(19973970, 4793);
                        ctx.exit_quest(false, true);
                        Some(event.to_string())
                    } else {
                        // `getNoQuestLevelRewardMsg` — unreachable (75+ to start).
                        Some(ctx.no_quest_html())
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `getRandomPartyMemberState(player, -1, 3, npc)`. Port is killer-only
        // (G11 party deviation).
        if !ctx.has_qs() {
            return;
        }
        let Some((item, chance, count)) = drop_for(ctx.npc_id) else {
            return;
        };
        if count != ctx.memo_state() {
            return;
        }
        if ctx.is_cond(4) {
            if ctx.give_item_randomly(item, 1, 50, chance, true) {
                ctx.set_cond(5, false);
            }
        } else if ctx.is_cond(10)
            && ctx.give_item_randomly(item, 1, 10, chance, true)
            && has_trophies(ctx)
        {
            ctx.set_cond(11, false);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_completed() {
            return Some(if ctx.npc_id == MARQUEZ {
                ctx.already_completed_html()
            } else {
                ctx.no_quest_html()
            });
        }
        if ctx.is_created() {
            return Some(if ctx.npc_id == MARQUEZ {
                "32113-01.htm".to_string()
            } else {
                ctx.no_quest_html()
            });
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let memo = ctx.memo_state();
        match ctx.npc_id {
            MARQUEZ => Some(match memo {
                1 => "32113-07.html".to_string(),
                2 => "32113-08.html".to_string(),
                3 => "32113-09.html".to_string(),
                4 => {
                    if ctx.quest_items_count(DIARY_FRAGMENT) < 50 {
                        "32113-16.html".to_string()
                    } else {
                        ctx.take_items(DIARY_FRAGMENT, -1);
                        ctx.set_memo_state(5);
                        "32113-17.html".to_string()
                    }
                }
                5 => "32113-26.html".to_string(),
                6 => "32113-27.html".to_string(),
                7 | 8 => "32113-28.html".to_string(),
                9 => "32113-29.html".to_string(),
                10 | 11 | 12 => "32113-30.html".to_string(),
                _ => ctx.no_quest_html(),
            }),
            MUSHIKA => {
                if memo == 1 {
                    ctx.set_cond(2, true);
                    ctx.set_memo_state(2);
                    Some("32114-01.html".to_string())
                } else if memo > 1 && memo < 10 {
                    Some("32114-02.html".to_string())
                } else {
                    Some("32114-03.html".to_string())
                }
            }
            ASAMAH => Some(match memo {
                1 => "32115-01.html".to_string(),
                2 => "32115-02.html".to_string(),
                3 | 4 | 5 | 6 | 7 | 8 => "32115-04.html".to_string(),
                9 => "32115-05.html".to_string(),
                10 => "32115-07.html".to_string(),
                11 => {
                    if !has_trophies(ctx) {
                        "32115-10.html".to_string()
                    } else {
                        ctx.set_memo_state(12);
                        ctx.set_cond(12, true);
                        ctx.give_items(PRACTICE_ELROKIAN_TRAP, 1);
                        ctx.take_items(ORNITHOMINUS_CLAW, -1);
                        ctx.take_items(DEINONYCHUS_BONE, -1);
                        ctx.take_items(PACHYCEPHALOSAURUS_SKIN, -1);
                        "32115-11.html".to_string()
                    }
                }
                12 => "32115-12.html".to_string(),
                _ => ctx.no_quest_html(),
            }),
            KIRIKACHIN => Some(match memo {
                1 | 2 | 3 | 4 | 5 => "32116-01.html".to_string(),
                6 => {
                    if ctx.quest_items_count(EXPEDITION_MEMBERS_LETTER) > 0 {
                        ctx.set_memo_state(7);
                        ctx.set_cond(7, true);
                        ctx.take_items(EXPEDITION_MEMBERS_LETTER, -1);
                        "32116-02.html".to_string()
                    } else {
                        ctx.no_quest_html()
                    }
                }
                7 => "32116-05.html".to_string(),
                8 => "32116-06.html".to_string(),
                9 | 10 | 11 => "32116-08.html".to_string(),
                12 => "32116-09.html".to_string(),
                _ => ctx.no_quest_html(),
            }),
            _ => Some(ctx.no_quest_html()),
        }
    }
}
