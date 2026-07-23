//! Path Of The Human Knight (402) — port of
//! `dist/game/data/scripts/quests/Q00402_PathOfTheHumanKnight/`.
//!
//! Awards the **Sword of Ritual** (1161) and, with it, completes the proof set
//! for `ElfHumanFighterChange1` — every one of that script's five targets is
//! now reachable in normal play.
//!
//! Structurally unlike the other Path quests: it is **six independent
//! sub-quests and you only need three.** Sir Klaus Vasper hands out a Squire's
//! Mark, then six different officers each offer the same deal — take my badge,
//! bring back N trophies, receive a Coin of Lords. Three coins is enough.
//!
//! ## The completion path forks on the coin count, and the 6-coin case is odd
//!
//! Talking to Vasper with the mark:
//!
//! | Coins | Page | Completes? |
//! |---|---|---|
//! | < 3 | `30417-09` | no |
//! | 3 | `30417-10` | no — a confirm button posts `30417-13` |
//! | 4–5 | `30417-11` | no — a confirm button posts `30417-14` |
//! | 6 | `30417-12` | **yes, immediately in `onTalk`** |
//!
//! So the "collected everything" case is the *only* one with no confirmation
//! step. It reads like an oversight, but it is what the dist does and the
//! pages back it up (`-12` is a completion page, not a prompt). Kept, and
//! tested, because a reader "fixing" the asymmetry would either add a prompt
//! nobody can answer or drop the 6-coin completion entirely.
//!
//! The two confirm handlers also take **all** the leftover badges and
//! trophies, not just the coins — a player who part-finished the other
//! sub-quests would otherwise keep them. The 6-coin path takes only coins and
//! the mark, which is correct there: every badge was already consumed paying
//! for a coin.
//!
//! ## Two smaller quirks
//!
//! - **The quest never calls `setCond`.** Not once in 629 lines: `startQuest`
//!   sets cond 1 and it stays there, so the client quest window shows a single
//!   step throughout. Verified by grep (`setCond` count: 0) rather than
//!   assumed from the parts I read.
//! - **NPC 30417's page extensions alternate**: `-01..-05` and `-07`/`-08` are
//!   `.htm`, while `-06` and `-09..-15` are `.html`. Not a prefix split like
//!   the other Path quests — copied verbatim per page.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const SIR_KLAUS_VASPER: i32 = 30417;
const SIR_ARON_TANFORD: i32 = 30653;

const SQUIRES_MARK: i32 = 1271;
const SWORD_OF_RITUAL: i32 = 1161;

const COINS: [i32; 6] = [1162, 1163, 1164, 1165, 1166, 1167];

const FIGHTER: i32 = 0;
const KNIGHT: i32 = 4;
const MIN_LEVEL: i32 = 19;

/// One of the six officers. `badge_event` is the bypass that hands the badge
/// over; the four pages are offer / need-more / turn-in / already-paid.
struct Branch {
    npc: i32,
    badge: i32,
    coin: i32,
    material: i32,
    required: i64,
    badge_event: &'static str,
    offer: &'static str,
    need_more: &'static str,
    turn_in: &'static str,
    has_coin: &'static str,
}

/// Note Raymond (30289): he alone ships six pages, because an extra
/// intermediate page `-02` sits between the offer and the badge hand-over, so
/// every later page of his shifts up by one. Encoded per branch rather than
/// derived from the offer page for exactly that reason.
const BRANCHES: [Branch; 6] = [
    Branch {
        npc: 30332, // Captain Bathis
        badge: 1168,
        coin: 1162,
        material: 1169, // Bugbear Necklace
        required: 10,
        badge_event: "30332-02.html",
        offer: "30332-01.html",
        need_more: "30332-03.html",
        turn_in: "30332-04.html",
        has_coin: "30332-05.html",
    },
    Branch {
        npc: 30289, // High Priest Raymond
        badge: 1170,
        coin: 1163,
        material: 1171, // Einhasad Crucifix
        required: 12,
        badge_event: "30289-03.html",
        offer: "30289-01.html",
        need_more: "30289-04.html",
        turn_in: "30289-05.html",
        has_coin: "30289-06.html",
    },
    Branch {
        npc: 30379, // Captain Bezique
        badge: 1172,
        coin: 1164,
        material: 1173, // Venomous Spider's Leg
        required: 20,
        badge_event: "30379-02.html",
        offer: "30379-01.html",
        need_more: "30379-03.html",
        turn_in: "30379-04.html",
        has_coin: "30379-05.html",
    },
    Branch {
        npc: 30037, // Levian
        badge: 1174,
        coin: 1165,
        material: 1175, // Lizardman's Totem
        required: 20,
        badge_event: "30037-02.html",
        offer: "30037-01.html",
        need_more: "30037-03.html",
        turn_in: "30037-04.html",
        has_coin: "30037-05.html",
    },
    Branch {
        npc: 30039, // Captain Gilbert
        badge: 1176,
        coin: 1166,
        material: 1177, // Giant Spider's Husk
        required: 20,
        badge_event: "30039-02.html",
        offer: "30039-01.html",
        need_more: "30039-03.html",
        turn_in: "30039-04.html",
        has_coin: "30039-05.html",
    },
    Branch {
        npc: 30031, // High Priest Biotin
        badge: 1178,
        coin: 1167,
        material: 1179, // Skull of Silent Horror
        required: 10,
        badge_event: "30031-02.html",
        offer: "30031-01.html",
        need_more: "30031-03.html",
        turn_in: "30031-04.html",
        has_coin: "30031-05.html",
    },
];

/// `(mob ids, badge that gates the drop, material, cap, chance out of ten)`.
/// A `None` chance means **every** kill pays — two of the six trophies have no
/// roll at all, which is easy to miss in a table of six near-identical blocks.
type Drop = (&'static [i32], i32, i32, i64, Option<i32>);
const DROPS: [Drop; 6] = [
    (&[20775], 1168, 1169, 10, None),                  // Bugbear Raider
    (&[27024], 1170, 1171, 12, Some(5)),               // Undead Priest (quest monster)
    (&[20038, 20043, 20050], 1172, 1173, 20, None),    // Venomous spiders
    (&[20024, 20027, 20030], 1174, 1175, 20, Some(5)), // Langk Lizardmen
    (&[20103, 20106, 20108], 1176, 1177, 20, Some(4)), // Giant spiders
    (&[20404], 1178, 1179, 10, Some(4)),               // Silent Horror
];

const KILL_NPCS: [i32; 12] = [
    20024, 20027, 20030, 20038, 20043, 20050, 20103, 20106, 20108, 20404, 20775, 27024,
];

const TALK_NPCS: [i32; 8] = [30417, 30031, 30037, 30289, 30039, 30332, 30379, 30653];

const QUEST_ITEMS: [i32; 19] = [
    SQUIRES_MARK,
    1162,
    1163,
    1164,
    1165,
    1166,
    1167,
    1168,
    1169,
    1170,
    1171,
    1172,
    1173,
    1174,
    1175,
    1176,
    1177,
    1178,
    1179,
];

pub struct Q00402PathOfTheHumanKnight;

impl Q00402PathOfTheHumanKnight {
    fn coin_count(&self, ctx: &QuestCtx) -> i64 {
        COINS
            .iter()
            .filter(|id| ctx.quest_items_count(**id) > 0)
            .count() as i64
    }

    /// The reward, plus Java's sweep of every leftover badge and trophy.
    /// `full_set` is the 6-coin path, which has no leftovers to sweep.
    fn award_sword(&self, ctx: &mut QuestCtx, full_set: bool) {
        ctx.give_items(SWORD_OF_RITUAL, 1);
        for id in COINS {
            ctx.take_items(id, 1);
        }
        if !full_set {
            for (_, badge, material, _, _) in DROPS {
                ctx.take_items(badge, 1);
                ctx.take_items(material, 1);
            }
        }
        ctx.take_items(SQUIRES_MARK, 1);
        // Java's three-way level branch awards identical exp/sp.
        ctx.add_exp_and_sp(80314, 5087);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
    }
}

impl QuestScript for Q00402PathOfTheHumanKnight {
    fn id(&self) -> i32 {
        402
    }
    fn name(&self) -> &'static str {
        "Q00402_PathOfTheHumanKnight"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00402_PathOfTheHumanKnight"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SIR_KLAUS_VASPER]
    }
    fn talk_npcs(&self) -> &[i32] {
        &TALK_NPCS
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
        // Any officer's badge hand-over.
        if let Some(b) = BRANCHES.iter().find(|b| b.badge_event == event) {
            ctx.give_items(b.badge, 1);
            return Some(event.to_string());
        }
        match event {
            "ACCEPT" => Some(
                match ctx.player_class_id() {
                    FIGHTER if ctx.player_level() < MIN_LEVEL => "30417-02.htm",
                    FIGHTER if ctx.quest_items_count(SWORD_OF_RITUAL) > 0 => "30417-04.htm",
                    FIGHTER => "30417-05.htm",
                    KNIGHT => "30417-02a.htm",
                    _ => "30417-03.htm",
                }
                .to_string(),
            ),
            "30417-08.htm" => {
                ctx.start_quest();
                ctx.give_items(SQUIRES_MARK, 1);
                Some(event.to_string())
            }
            // Pure navigation. Note the mixed extensions — verbatim.
            "30289-02.html" | "30417-06.html" | "30417-07.htm" | "30417-15.html" => {
                Some(event.to_string())
            }
            // The two confirmation buttons, for exactly 3 and for 4–5 coins.
            "30417-13.html" | "30417-14.html" => {
                if ctx.quest_items_count(SQUIRES_MARK) == 0 {
                    return None;
                }
                let coins = self.coin_count(ctx);
                let ok = if event == "30417-13.html" {
                    coins == 3
                } else {
                    coins > 3 && coins < 6
                };
                if !ok {
                    return None;
                }
                self.award_sword(ctx, false);
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
        let Some((_, badge, material, cap, chance)) =
            DROPS.iter().find(|(mobs, ..)| mobs.contains(&npc_id))
        else {
            return;
        };
        if ctx.quest_items_count(*badge) == 0 || ctx.quest_items_count(*material) >= *cap {
            return;
        }
        // `getRandom(10) < chance`, or no roll at all for the two that lack one.
        if let Some(c) = chance {
            if ctx.roll(10) >= *c {
                return;
            }
        }
        ctx.give_items(*material, 1);
        // Java plays a sound here and, unusually, never advances the cond.
        if ctx.quest_items_count(*material) == *cap {
            ctx.play_sound(quest_sounds::MIDDLE);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == SIR_KLAUS_VASPER {
                return Some("30417-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let has_mark = ctx.quest_items_count(SQUIRES_MARK) > 0;
        if npc == SIR_KLAUS_VASPER {
            if !has_mark {
                return Some(ctx.no_quest_html());
            }
            return Some(match self.coin_count(ctx) {
                0..=2 => "30417-09.html".to_string(),
                3 => "30417-10.html".to_string(),
                4 | 5 => "30417-11.html".to_string(),
                // Six coins completes here and now — no confirm step.
                _ => {
                    self.award_sword(ctx, true);
                    "30417-12.html".to_string()
                }
            });
        }
        if npc == SIR_ARON_TANFORD {
            // Pure hint NPC.
            return Some(if has_mark {
                "30653-01.html".to_string()
            } else {
                ctx.no_quest_html()
            });
        }
        let Some(b) = BRANCHES.iter().find(|b| b.npc == npc) else {
            return Some(ctx.no_quest_html());
        };
        let has_badge = ctx.quest_items_count(b.badge) > 0;
        let has_coin = ctx.quest_items_count(b.coin) > 0;
        if has_mark && !has_badge && !has_coin {
            return Some(b.offer.to_string());
        }
        if has_badge {
            if ctx.quest_items_count(b.material) < b.required {
                return Some(b.need_more.to_string());
            }
            ctx.give_items(b.coin, 1);
            ctx.take_items(b.badge, 1);
            ctx.take_items(b.material, -1);
            return Some(b.turn_in.to_string());
        }
        if has_coin {
            return Some(b.has_coin.to_string());
        }
        Some(ctx.no_quest_html())
    }
}
