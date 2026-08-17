//! Test of the Maestro (231) — `quests/Q00231_TestOfTheMaestro`. The Warsmith
//! 2nd-class proof (Artisan → Warsmith): Iron Gate's Lockirin (30531, level 39+)
//! sends the player to earn **three guild recommendations** — Balanki, Filaur,
//! Arin — each its own errand, run **one at a time** (`memoState` 2/3/4 marks
//! which is active, returning to the hub `1` when done). Holding all three sets
//! `cond` 2 and Lockirin awards the **Mark of Maestro** (2867).
//!
//! - **Balanki** (memo 2): Croto's Paint of Kamuru → an Evil Eye Lord drops the
//!   Necklace of Kamutu → Croto's Letter of Solder Detachment → the rec.
//! - **Arin** (memo 3): Toma turns a Paint of Teleport Device into a Broken one
//!   (a `teleTo` + a 5 s timer that ambushes with King Bugbears) → 5 Teleport
//!   Devices → the rec.
//! - **Filaur** (memo 4): Lorain's antidote errand — 10 each of Wasp Needle /
//!   Spider Web / Leech Blood → Report of Cruma → the rec.
use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const LOCKIRIN: i32 = 30531;
const BALANKI: i32 = 30533;
const FILAUR: i32 = 30535;
const ARIN: i32 = 30536;
const TOMA: i32 = 30556;
const CROTO: i32 = 30671;
const DUBABAH: i32 = 30672;
const LORAIN: i32 = 30673;

const REC_BALANKI: i32 = 2864;
const REC_FILAUR: i32 = 2865;
const REC_ARIN: i32 = 2866;
const MARK_OF_MAESTRO: i32 = 2867;
const LETTER: i32 = 2868;
const PAINT_KAMURU: i32 = 2869;
const NECKLACE_KAMUTU: i32 = 2870;
const PAINT_TELEPORT: i32 = 2871;
const TELEPORT_DEVICE: i32 = 2872;
const ARCHITECTURE: i32 = 2873;
const REPORT_CRUMA: i32 = 2874;
const INGREDIENTS_ANTIDOTE: i32 = 2875;
const WASP_NEEDLE: i32 = 2876;
const SPIDER_WEB: i32 = 2877;
const LEECH_BLOOD: i32 = 2878;
const BROKEN_TELEPORT: i32 = 2916;

const KING_BUGBEAR: i32 = 20150;
const GIANT_MIST_LEECH: i32 = 20225;
const STINGER_WASP: i32 = 20229;
const MARSH_SPIDER: i32 = 20233;
const EVIL_EYE_LORD: i32 = 27133;

const MIN_LEVEL: i32 = 39;
const ARTISAN: i32 = 56;

/// Give one collection item and chime — the "middle" cue once it caps at 10.
fn collect(ctx: &mut QuestCtx, item: i32) {
    ctx.give_items(item, 1);
    ctx.play_sound(if ctx.quest_items_count(item) >= 10 {
        quest_sounds::MIDDLE
    } else {
        quest_sounds::ITEMGET
    });
}

fn has_all_recs(ctx: &QuestCtx) -> bool {
    ctx.quest_items_count(REC_BALANKI) > 0
        && ctx.quest_items_count(REC_FILAUR) > 0
        && ctx.quest_items_count(REC_ARIN) > 0
}

pub struct Q00231TestOfTheMaestro;

impl QuestScript for Q00231TestOfTheMaestro {
    fn id(&self) -> i32 {
        231
    }
    fn name(&self) -> &'static str {
        "Q00231_TestOfTheMaestro"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00231_TestOfTheMaestro"
    }
    fn start_npcs(&self) -> &[i32] {
        &[LOCKIRIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            LOCKIRIN, 30532, BALANKI, 30534, FILAUR, ARIN, TOMA, CROTO, DUBABAH, LORAIN,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[
            KING_BUGBEAR,
            GIANT_MIST_LEECH,
            STINGER_WASP,
            MARSH_SPIDER,
            EVIL_EYE_LORD,
        ]
    }
    fn quest_items(&self) -> &[i32] {
        &[
            REC_BALANKI,
            REC_FILAUR,
            REC_ARIN,
            LETTER,
            PAINT_KAMURU,
            NECKLACE_KAMUTU,
            PAINT_TELEPORT,
            TELEPORT_DEVICE,
            ARCHITECTURE,
            REPORT_CRUMA,
            INGREDIENTS_ANTIDOTE,
            WASP_NEEDLE,
            SPIDER_WEB,
            LEECH_BLOOD,
            BROKEN_TELEPORT,
        ]
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_completed() {
            return Some(if ctx.npc_id == LOCKIRIN {
                ctx.already_completed_html()
            } else {
                ctx.no_quest_html()
            });
        }
        if ctx.is_created() {
            if ctx.npc_id == LOCKIRIN {
                if ctx.player_class_id() == ARTISAN {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "30531-03.htm"
                        } else {
                            "30531-01.html"
                        }
                        .to_string(),
                    );
                }
                return Some("30531-02.html".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let memo = ctx.memo_state();
        match ctx.npc_id {
            LOCKIRIN => {
                if has_all_recs(ctx) {
                    ctx.give_adena(372154, true);
                    ctx.give_items(MARK_OF_MAESTRO, 1);
                    ctx.add_exp_and_sp(2085244, 141240);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some("30531-06.html".to_string());
                }
                if memo >= 1 {
                    return Some("30531-05.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            30532 => Some("30532-01.html".to_string()),
            BALANKI => {
                if memo == 1 && ctx.quest_items_count(REC_BALANKI) == 0 {
                    Some("30533-01.html".to_string())
                } else if memo == 2 {
                    if ctx.quest_items_count(LETTER) == 0 {
                        Some("30533-03.html".to_string())
                    } else {
                        ctx.give_items(REC_BALANKI, 1);
                        ctx.take_items(LETTER, 1);
                        ctx.set_memo_state(1);
                        if ctx.quest_items_count(REC_ARIN) > 0
                            && ctx.quest_items_count(REC_FILAUR) > 0
                        {
                            ctx.set_cond(2, true);
                        }
                        Some("30533-04.html".to_string())
                    }
                } else if ctx.quest_items_count(REC_BALANKI) > 0 {
                    Some("30533-05.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            30534 => Some("30534-01.html".to_string()),
            FILAUR => {
                if memo == 1 && ctx.quest_items_count(REC_FILAUR) == 0 {
                    ctx.give_items(ARCHITECTURE, 1);
                    ctx.set_memo_state(4);
                    Some("30535-01.html".to_string())
                } else if memo == 4 {
                    if ctx.quest_items_count(ARCHITECTURE) > 0
                        && ctx.quest_items_count(REPORT_CRUMA) == 0
                    {
                        Some("30535-02.html".to_string())
                    } else if ctx.quest_items_count(REPORT_CRUMA) > 0
                        && ctx.quest_items_count(ARCHITECTURE) == 0
                    {
                        ctx.give_items(REC_FILAUR, 1);
                        ctx.take_items(REPORT_CRUMA, 1);
                        ctx.set_memo_state(1);
                        if ctx.quest_items_count(REC_BALANKI) > 0
                            && ctx.quest_items_count(REC_ARIN) > 0
                        {
                            ctx.set_cond(2, true);
                        }
                        Some("30535-03.html".to_string())
                    } else {
                        Some(ctx.no_quest_html())
                    }
                } else if ctx.quest_items_count(REC_FILAUR) > 0 {
                    Some("30535-04.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            ARIN => {
                if memo == 1 && ctx.quest_items_count(REC_ARIN) == 0 {
                    ctx.give_items(PAINT_TELEPORT, 1);
                    ctx.set_memo_state(3);
                    Some("30536-01.html".to_string())
                } else if memo == 3 {
                    if ctx.quest_items_count(PAINT_TELEPORT) > 0
                        && ctx.quest_items_count(TELEPORT_DEVICE) == 0
                    {
                        Some("30536-02.html".to_string())
                    } else if ctx.quest_items_count(TELEPORT_DEVICE) >= 5 {
                        ctx.give_items(REC_ARIN, 1);
                        ctx.take_items(TELEPORT_DEVICE, -1);
                        ctx.set_memo_state(1);
                        if ctx.quest_items_count(REC_BALANKI) > 0
                            && ctx.quest_items_count(REC_FILAUR) > 0
                        {
                            ctx.set_cond(2, true);
                        }
                        Some("30536-03.html".to_string())
                    } else {
                        Some(ctx.no_quest_html())
                    }
                } else if ctx.quest_items_count(REC_ARIN) > 0 {
                    Some("30536-04.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            TOMA => {
                if memo == 3 {
                    if ctx.quest_items_count(PAINT_TELEPORT) > 0 {
                        return Some("30556-01.html".to_string());
                    }
                    if ctx.quest_items_count(BROKEN_TELEPORT) > 0 {
                        ctx.give_items(TELEPORT_DEVICE, 5);
                        ctx.take_items(BROKEN_TELEPORT, 1);
                        return Some("30556-06.html".to_string());
                    }
                    if ctx.quest_items_count(TELEPORT_DEVICE) == 5 {
                        return Some("30556-07.html".to_string());
                    }
                }
                Some(ctx.no_quest_html())
            }
            CROTO => {
                if memo == 2
                    && ctx.quest_items_count(PAINT_KAMURU) == 0
                    && ctx.quest_items_count(NECKLACE_KAMUTU) == 0
                    && ctx.quest_items_count(LETTER) == 0
                {
                    Some("30671-01.html".to_string())
                } else if ctx.quest_items_count(PAINT_KAMURU) > 0
                    && ctx.quest_items_count(NECKLACE_KAMUTU) == 0
                {
                    Some("30671-03.html".to_string())
                } else if ctx.quest_items_count(NECKLACE_KAMUTU) > 0 {
                    ctx.give_items(LETTER, 1);
                    ctx.take_items(NECKLACE_KAMUTU, 1);
                    ctx.take_items(PAINT_KAMURU, 1);
                    Some("30671-04.html".to_string())
                } else if ctx.quest_items_count(LETTER) > 0 {
                    Some("30671-05.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            DUBABAH => {
                if ctx.quest_items_count(PAINT_KAMURU) > 0 {
                    Some("30672-01.html".to_string())
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            LORAIN => {
                if memo == 4 {
                    if ctx.quest_items_count(ARCHITECTURE) > 0
                        && ctx.quest_items_count(INGREDIENTS_ANTIDOTE) == 0
                        && ctx.quest_items_count(REPORT_CRUMA) == 0
                    {
                        ctx.give_items(INGREDIENTS_ANTIDOTE, 1);
                        ctx.take_items(ARCHITECTURE, 1);
                        return Some("30673-01.html".to_string());
                    }
                    if ctx.quest_items_count(INGREDIENTS_ANTIDOTE) > 0
                        && ctx.quest_items_count(REPORT_CRUMA) == 0
                    {
                        return Some(
                            if ctx.quest_items_count(WASP_NEEDLE) >= 10
                                && ctx.quest_items_count(SPIDER_WEB) >= 10
                                && ctx.quest_items_count(LEECH_BLOOD) >= 10
                            {
                                "30673-03.html"
                            } else {
                                "30673-02.html"
                            }
                            .to_string(),
                        );
                    }
                    if ctx.quest_items_count(REPORT_CRUMA) > 0 {
                        return Some("30673-05.html".to_string());
                    }
                }
                Some(ctx.no_quest_html())
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => {
                if ctx.is_created() {
                    ctx.start_quest();
                    ctx.set_memo_state(1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                }
                None
            }
            "30533-02.html" => {
                ctx.set_memo_state(2);
                Some(event.to_string())
            }
            "30556-02.html" | "30556-03.html" | "30556-04.html" => Some(event.to_string()),
            "30556-05.html" => {
                if ctx.quest_items_count(PAINT_TELEPORT) > 0 {
                    ctx.give_items(BROKEN_TELEPORT, 1);
                    ctx.take_items(PAINT_TELEPORT, 1);
                    ctx.teleport_to(140352, -194133, -3146);
                    ctx.start_quest_timer("SPAWN_KING_BUGBEAR", 5000);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            "30671-02.html" => {
                ctx.give_items(PAINT_KAMURU, 1);
                Some(event.to_string())
            }
            "30673-04.html" => {
                if ctx.quest_items_count(INGREDIENTS_ANTIDOTE) > 0
                    && ctx.quest_items_count(WASP_NEEDLE) >= 10
                    && ctx.quest_items_count(SPIDER_WEB) >= 10
                    && ctx.quest_items_count(LEECH_BLOOD) >= 10
                {
                    ctx.give_items(REPORT_CRUMA, 1);
                    ctx.take_items(WASP_NEEDLE, -1);
                    ctx.take_items(SPIDER_WEB, -1);
                    ctx.take_items(LEECH_BLOOD, -1);
                    ctx.take_items(INGREDIENTS_ANTIDOTE, 1);
                    Some(event.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let memo = ctx.memo_state();
        match ctx.npc_id {
            GIANT_MIST_LEECH => {
                if memo == 4
                    && ctx.quest_items_count(INGREDIENTS_ANTIDOTE) > 0
                    && ctx.quest_items_count(LEECH_BLOOD) < 10
                {
                    collect(ctx, LEECH_BLOOD);
                }
            }
            STINGER_WASP => {
                if memo == 4
                    && ctx.quest_items_count(INGREDIENTS_ANTIDOTE) > 0
                    && ctx.quest_items_count(WASP_NEEDLE) < 10
                {
                    collect(ctx, WASP_NEEDLE);
                }
            }
            MARSH_SPIDER => {
                if memo == 4
                    && ctx.quest_items_count(INGREDIENTS_ANTIDOTE) > 0
                    && ctx.quest_items_count(SPIDER_WEB) < 10
                {
                    collect(ctx, SPIDER_WEB);
                }
            }
            EVIL_EYE_LORD if memo == 2 && ctx.quest_items_count(PAINT_KAMURU) > 0 => {
                ctx.award_once(NECKLACE_KAMUTU);
            }
            _ => {}
        }
    }

    fn on_timer(&self, ctx: &mut QuestCtx, name: &str) {
        if name == "SPAWN_KING_BUGBEAR" {
            for _ in 0..3 {
                ctx.spawn_attacker_at(KING_BUGBEAR, 140395, -194147, -3146);
            }
        }
    }
}
