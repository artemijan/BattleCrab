//! The Name of Evil - 2 (126) — `quests/Q00126_TheNameOfEvil2`. The level-77
//! conclusion of the Primeval Isle story (after
//! [`Q125`](super::q00125_the_name_of_evil_1)): Asamah leads the player back
//! through the three singing Kaimu pillars, then to the Warrior's Grave to play
//! three melodies (a musical variant of Q125's letter puzzle) that raise the
//! Bone Powder, which Shilen's Stone Statue reads — earning an A-grade Weapon
//! Enchant, Adena and exp. Completing it unlocks
//! [`Q641 Attack Sailren`](super::q00641_attack_sailren).

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
/// Skill 5089 — the Warrior's Grave flourish `npc.broadcastPacket(new
/// MagicSkillUse(npc, player, 5089, 1, 1000, 0))`. Same effect and same
/// caller→target shape as the prequel's `PUZZLE_FLOURISH` in Q125.
const GRAVE_FLOURISH: i32 = 5089;
const SHILENS_STONE_STATUE: i32 = 32109;
const MUSHIKA: i32 = 32114;
const ASAMAH: i32 = 32115;
const ULU_KAIMU: i32 = 32119;
const BALU_KAIMU: i32 = 32120;
const CHUTA_KAIMU: i32 = 32121;
const WARRIORS_GRAVE: i32 = 32122;
// Items
const GAZKH_FRAGMENT: i32 = 8782;
const BONE_POWDER: i32 = 8783;
// Reward
const ENCHANT_WEAPON_A: i32 = 729;
// Misc
const MIN_LEVEL: i32 = 77;
const PREREQ: &str = "Q00125_TheNameOfEvil1";
// Elroki flute cues (only FULL is in quest_sounds).
const SONG_1ST: &str = "EtcSound.elcroki_song_1st";
const SONG_2ND: &str = "EtcSound.elcroki_song_2nd";
const SONG_3RD: &str = "EtcSound.elcroki_song_3rd";

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

/// A melody's final note: set it, and if all five notes of the melody are now
/// held **and** the quest is at `active_cond`, advance to `active_cond + 1`.
/// Always clears the melody afterward. Returns the success or retry html.
fn finish_melody(
    ctx: &mut QuestCtx,
    last: &str,
    active_cond: i32,
    notes: &[&str],
    ok_html: &str,
    fail_html: &str,
) -> String {
    ctx.set_var(last, "1");
    let solved = ctx.is_cond(active_cond) && notes.iter().all(|n| ctx.get_int(n) > 0);
    if solved {
        ctx.set_cond(active_cond + 1, true);
    }
    for n in notes {
        ctx.unset(n);
    }
    if solved { ok_html } else { fail_html }.to_string()
}

pub struct Q00126TheNameOfEvil2;

impl QuestScript for Q00126TheNameOfEvil2 {
    fn id(&self) -> i32 {
        126
    }
    fn name(&self) -> &'static str {
        "Q00126_TheNameOfEvil2"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00126_TheNameOfEvil2"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ASAMAH]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            ASAMAH,
            ULU_KAIMU,
            BALU_KAIMU,
            CHUTA_KAIMU,
            WARRIORS_GRAVE,
            SHILENS_STONE_STATUE,
            MUSHIKA,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[GAZKH_FRAGMENT, BONE_POWDER]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return Some(ctx.no_quest_html());
        }
        // Linear cond advances gated on the current cond.
        let advance: &[(&str, i32, i32)] = &[
            ("32115-1b.html", 1, 2),
            ("32119-3.html", 2, 3),
            ("32119-4.html", 3, 4),
            ("32119-5.html", 4, 5),
            ("32120-3.html", 5, 6),
            ("32120-4.html", 6, 7),
            ("32120-5.html", 7, 8),
            ("32121-3.html", 8, 9),
            ("32121-4.html", 9, 10),
            ("32122-3.html", 12, 13),
            ("32122-4.html", 13, 14),
            ("32122-8.html", 17, 18),
            ("32109-2.html", 18, 19),
            ("32115-4.html", 20, 21),
            ("32115-5.html", 21, 22),
            ("32114-2.html", 22, 23),
        ];
        if let Some(&(_, from, to)) = advance.iter().find(|&&(e, ..)| e == event) {
            if ctx.is_cond(from) {
                ctx.set_cond(to, true);
            }
            return Some(event.to_string());
        }

        match event {
            "32115-1.html" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            // The Kaimu flute cues (cosmetic).
            "32119-4a.html" | "32119-5b.html" => {
                ctx.play_sound(SONG_1ST);
                Some(event.to_string())
            }
            "32120-4a.html" | "32120-5b.html" => {
                ctx.play_sound(SONG_2ND);
                Some(event.to_string())
            }
            "32121-4a.html" | "32121-5b.html" => {
                ctx.play_sound(SONG_3RD);
                Some(event.to_string())
            }
            // Chuta hands over the Gazkh Fragment.
            "32121-5.html" => {
                if ctx.is_cond(10) {
                    ctx.give_items(GAZKH_FRAGMENT, 1);
                    ctx.set_cond(11, true);
                }
                Some(event.to_string())
            }
            // Warrior's Grave: the flourish plays on the grave toward the player.
            "32122-2a.html" => {
                ctx.cast_visual_at(ctx.npc, ctx.player, GRAVE_FLOURISH, 1, 1000);
                Some(event.to_string())
            }
            "32122-2d.html" => {
                ctx.take_items(GAZKH_FRAGMENT, -1);
                Some(event.to_string())
            }
            // --- Melody 1 (cond 14): DO-MI-FA-SOL-FA2 ---
            "DO_One" => {
                ctx.set_var("DO", "1");
                Some("32122-4d.html".to_string())
            }
            "MI_One" => {
                ctx.set_var("MI", "1");
                Some("32122-4f.html".to_string())
            }
            "FA_One" => {
                ctx.set_var("FA", "1");
                Some("32122-4h.html".to_string())
            }
            "SOL_One" => {
                ctx.set_var("SOL", "1");
                Some("32122-4j.html".to_string())
            }
            "FA2_One" => Some(finish_melody(
                ctx,
                "FA2",
                14,
                &["DO", "MI", "FA", "SOL", "FA2"],
                "32122-4n.html",
                "32122-4m.html",
            )),
            "32122-4m.html" => {
                for n in ["DO", "MI", "FA", "SOL", "FA2"] {
                    ctx.unset(n);
                }
                Some(event.to_string())
            }
            // --- Melody 2 (cond 15): FA-SOL-TI-SOL2-FA2 ---
            "FA_Two" => {
                ctx.set_var("FA", "1");
                Some("32122-5a.html".to_string())
            }
            "SOL_Two" => {
                ctx.set_var("SOL", "1");
                Some("32122-5c.html".to_string())
            }
            "TI_Two" => {
                ctx.set_var("TI", "1");
                Some("32122-5e.html".to_string())
            }
            "SOL2_Two" => {
                ctx.set_var("SOL2", "1");
                Some("32122-5g.html".to_string())
            }
            "FA2_Two" => Some(finish_melody(
                ctx,
                "FA2",
                15,
                &["FA", "SOL", "TI", "SOL2", "FA2"],
                "32122-5j.html",
                "32122-5i.html",
            )),
            "32122-5i.html" => {
                for n in ["FA", "SOL", "TI", "SOL2", "FA2"] {
                    ctx.unset(n);
                }
                Some(event.to_string())
            }
            // --- Melody 3 (cond 16): SOL-FA-MI-FA2-MI2 ---
            "SOL_Three" => {
                ctx.set_var("SOL", "1");
                Some("32122-6a.html".to_string())
            }
            "FA_Three" => {
                ctx.set_var("FA", "1");
                Some("32122-6c.html".to_string())
            }
            "MI_Three" => {
                ctx.set_var("MI", "1");
                Some("32122-6e.html".to_string())
            }
            "FA2_Three" => {
                ctx.set_var("FA2", "1");
                Some("32122-6g.html".to_string())
            }
            "MI2_Three" => Some(finish_melody(
                ctx,
                "MI2",
                16,
                &["SOL", "FA", "MI", "FA2", "MI2"],
                "32122-6j.html",
                "32122-6i.html",
            )),
            "32122-6i.html" => {
                for n in ["SOL", "FA", "MI", "FA2", "MI2"] {
                    ctx.unset(n);
                }
                Some(event.to_string())
            }
            // The grave yields the Bone Powder, with the same flourish. Java's
            // order is give → sound → flourish; kept, since the sound and the
            // cast are both broadcast and a reader diffing the two will look.
            "32122-7.html" => {
                ctx.give_items(BONE_POWDER, 1);
                ctx.play_sound(quest_sounds::ELROKI_SONG_FULL);
                ctx.cast_visual_at(ctx.npc, ctx.player, GRAVE_FLOURISH, 1, 1000);
                Some(event.to_string())
            }
            // Shilen's Stone Statue reads the powder.
            "32109-3.html" => {
                if ctx.is_cond(19) {
                    ctx.take_items(BONE_POWDER, -1);
                    ctx.set_cond(20, true);
                }
                Some(event.to_string())
            }
            // Mushika's reward.
            "32114-3.html" => {
                ctx.reward_items(ENCHANT_WEAPON_A, 1);
                ctx.give_adena(460_483, true);
                ctx.add_exp_and_sp(1_015_973, 102_802);
                ctx.exit_quest(false, true);
                Some(event.to_string())
            }
            _ if event.ends_with(".html") || event.ends_with(".htm") => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            ASAMAH => asamah_talk(ctx, cond),
            ULU_KAIMU if ctx.is_started() => match cond {
                1 => "32119-1.html".to_string(),
                2 => "32119-2.html".to_string(),
                3 => "32119-3c.html".to_string(),
                4 => "32119-4c.html".to_string(),
                5 => "32119-5a.html".to_string(),
                _ => ctx.no_quest_html(),
            },
            BALU_KAIMU if ctx.is_started() => match cond {
                1..=4 => "32120-1.html".to_string(),
                5 => "32120-2.html".to_string(),
                6 => "32120-3c.html".to_string(),
                7 => "32120-4c.html".to_string(),
                _ => "32120-5a.html".to_string(),
            },
            CHUTA_KAIMU if ctx.is_started() => match cond {
                1..=7 => "32121-1.html".to_string(),
                8 => "32121-2.html".to_string(),
                9 => "32121-3e.html".to_string(),
                10 => "32121-4e.html".to_string(),
                _ => "32121-5a.html".to_string(),
            },
            WARRIORS_GRAVE if ctx.is_started() => grave_talk(ctx, cond),
            SHILENS_STONE_STATUE if ctx.is_started() => statue_talk(ctx, cond),
            MUSHIKA if ctx.is_started() => {
                if cond < 22 {
                    "32114-4.html".to_string()
                } else if cond == 22 {
                    "32114-1.html".to_string()
                } else {
                    "32114-2.html".to_string()
                }
            }
            _ => ctx.no_quest_html(),
        };
        // Java broadcasts the flourish from each of the three Kaimu brothers
        // the moment their `-2` page is served (three separate
        // `npc.broadcastPacket(new MagicSkillUse(npc, player, 5089, 1, 1000, 0))`
        // calls). Keyed on the **page** rather than the cond on purpose: each
        // brother's page sits at a different cond here (2 / 5 / 8), so a
        // cond-based check would silently miss two of the three.
        if matches!(
            html.as_str(),
            "32119-2.html" | "32120-2.html" | "32121-2.html"
        ) {
            ctx.cast_visual_at(ctx.npc, ctx.player, GRAVE_FLOURISH, 1, 1000);
        }
        Some(html)
    }
}

fn asamah_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    if ctx.is_created() {
        return if ctx.player_level() < MIN_LEVEL {
            "32115-0.htm".to_string()
        } else if ctx.other_quest_completed(PREREQ) {
            "32115-0a.htm".to_string()
        } else {
            "32115-0b.htm".to_string()
        };
    }
    if ctx.is_completed() {
        return ctx.already_completed_html();
    }
    match cond {
        1 => "32115-1d.html".to_string(),
        2 => "32115-1c.html".to_string(),
        3..=19 => "32115-2.html".to_string(),
        20 => "32115-3.html".to_string(),
        21 => "32115-4j.html".to_string(),
        22 => "32115-5a.html".to_string(),
        _ => ctx.no_quest_html(),
    }
}

fn grave_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1..=10 => "32122-1.html".to_string(),
        11 => {
            // Talking here advances the quest and opens the melody sequence.
            ctx.set_cond(12, true);
            "32122-2.html".to_string()
        }
        12 => "32122-2l.html".to_string(),
        13 => "32122-3b.html".to_string(),
        14 => {
            for n in ["DO", "MI", "FA", "SOL", "FA2"] {
                ctx.unset(n);
            }
            "32122-4.html".to_string()
        }
        15 => {
            for n in ["FA", "SOL", "TI", "SOL2", "FA2"] {
                ctx.unset(n);
            }
            "32122-5.html".to_string()
        }
        16 => {
            for n in ["SOL", "FA", "MI", "FA2", "MI2"] {
                ctx.unset(n);
            }
            "32122-6.html".to_string()
        }
        17 => {
            if has(ctx, BONE_POWDER) {
                "32122-7.html".to_string()
            } else {
                "32122-7b.html".to_string()
            }
        }
        18 => "32122-8.html".to_string(),
        _ => "32122-9.html".to_string(),
    }
}

fn statue_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1..=17 => "32109-1a.html".to_string(),
        18 => {
            if has(ctx, BONE_POWDER) {
                "32109-1.html".to_string()
            } else {
                ctx.no_quest_html()
            }
        }
        19 => "32109-2l.html".to_string(),
        20 => "32109-5.html".to_string(),
        _ => "32109-4.html".to_string(),
    }
}
