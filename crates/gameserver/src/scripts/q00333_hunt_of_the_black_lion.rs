//! Hunt of the Black Lion (333) — `quests/Q00333_HuntOfTheBlackLion`. The
//! datapack's largest quest: a Black Lion mercenary-guild grind out of Giran.
//! Mercenary Captain Sophya (30735) hands out four hunting **orders**; killing
//! the matching mobs while an order is held drops trophy **materials** and, by
//! chance, sealed **cargo boxes**. Materials are turned back in to Sophya for
//! **Lion's Claws** + adena; ten claws buy escalating **Lion's Eye** reward
//! draws. Cargo boxes feed two other guildsmen — Reedfoot (30736) gambles a box
//! for random trade goods (or, rarely, a Statue/Tablet piece), and Morgon
//! (30737) converts boxes into **Guild Coins** → adena. Statue-of-Shilen and
//! Ancient-Tablet sets are assembled by Rupio (30471) and cashed with Undrias
//! (30130) / Lockirin (30531). Completing with a Black Lion Mark pays 12,400
//! adena. Repeatable.
//!
//! Faithful to the Java, including two of its quirks kept verbatim: Reedfoot's
//! trade-good gamble and Sophya's `30735-22` adena formula's stray `fang + 7`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const UNDRIAS: i32 = 30130;
const RUPIO: i32 = 30471;
const LOCKIRIN: i32 = 30531;
const SOPHYA: i32 = 30735;
const REEDFOOT: i32 = 30736;
const MORGON: i32 = 30737;
// Progress items
const BLACK_LION_MARK: i32 = 1369;
const ADENA: i32 = 57;
const CARGO_BOX_1ST: i32 = 3440; // 3440..=3443 consecutive
const STATUE_HEAD: i32 = 3457; // 3457..=3460 consecutive
const COMPLETE_STATUE: i32 = 3461;
const TABLET_1ST: i32 = 3462; // 3462..=3465 consecutive
const COMPLETE_TABLET: i32 = 3466;
const SOPHYAS_1ST_ORDER: i32 = 3671; // 3671..=3674 consecutive
const LIONS_CLAW: i32 = 3675;
const LIONS_EYE: i32 = 3676;
const GUILD_COIN: i32 = 3677;
const UNDEAD_ASH: i32 = 3848;
const BLOODY_AXE_INSIGNIA: i32 = 3849;
const DELU_FANG: i32 = 3850;
const STAKATO_TALON: i32 = 3851;
// Reward consumables
const ALACRITY_POTION: i32 = 735;
const SCROLL_OF_ESCAPE: i32 = 736;
const HEALING_POTION: i32 = 1061;
const SOULSHOT_D: i32 = 1463;
const SPIRITSHOT_D: i32 = 2510;
// Boss mobs
const DELU_HEADHUNTER: i32 = 27151;
const MARSH_STAKATO_MARQUESS: i32 = 27152;
const MIN_LEVEL: i32 = 25;

/// Standard drop mobs: `(mob, order, material, material_pct, cargo_pct)`. `order`
/// is 1–4 (Sophya's Nth order gates the drop; the cargo box and order item are
/// the `order`-th of their consecutive id runs). The order-3 mobs also risk a
/// twin Headhunter ambush, the order-4 mobs a lone Marquess.
const DROPS: &[(i32, i32, i32, i32, i32)] = &[
    (20160, 1, UNDEAD_ASH, 50, 11),         // Neer Crawler
    (20171, 1, UNDEAD_ASH, 60, 8),          // Specter
    (20197, 1, UNDEAD_ASH, 60, 9),          // Sorrow Maiden
    (20198, 1, UNDEAD_ASH, 50, 12),         // Neer Crawler Berserker
    (20200, 1, UNDEAD_ASH, 50, 13),         // Strain
    (20201, 1, UNDEAD_ASH, 50, 15),         // Ghoul
    (20207, 2, BLOODY_AXE_INSIGNIA, 50, 9), // Ol Mahum Guerilla
    (20208, 2, BLOODY_AXE_INSIGNIA, 50, 10),
    (20209, 2, BLOODY_AXE_INSIGNIA, 50, 11),
    (20210, 2, BLOODY_AXE_INSIGNIA, 50, 12),
    (20211, 2, BLOODY_AXE_INSIGNIA, 50, 13),
    (20251, 3, DELU_FANG, 50, 14),     // Delu Lizardman
    (20252, 3, DELU_FANG, 50, 14),     // Delu Lizardman Scout
    (20253, 3, DELU_FANG, 50, 15),     // Delu Lizardman Warrior
    (20157, 4, STAKATO_TALON, 55, 12), // Marsh Stakato
    (20230, 4, STAKATO_TALON, 60, 13), // Marsh Stakato Worker
    (20232, 4, STAKATO_TALON, 56, 14), // Marsh Stakato Soldier
    (20234, 4, STAKATO_TALON, 60, 15), // Marsh Stakato Drone
];

pub struct Q00333HuntOfTheBlackLion {
    kill_ids: Vec<i32>,
}

impl Default for Q00333HuntOfTheBlackLion {
    fn default() -> Self {
        Self::new()
    }
}

impl Q00333HuntOfTheBlackLion {
    pub fn new() -> Self {
        let mut kill_ids: Vec<i32> = DROPS.iter().map(|d| d.0).collect();
        kill_ids.push(DELU_HEADHUNTER);
        kill_ids.push(MARSH_STAKATO_MARQUESS);
        Self { kill_ids }
    }

    fn count(&self, ctx: &QuestCtx, id: i32) -> i64 {
        ctx.quest_items_count(id)
    }
    fn has(&self, ctx: &QuestCtx, id: i32) -> bool {
        self.count(ctx, id) > 0
    }
    fn orders_held(&self, ctx: &QuestCtx) -> i64 {
        (0..4).map(|i| self.count(ctx, SOPHYAS_1ST_ORDER + i)).sum()
    }
    fn materials_held(&self, ctx: &QuestCtx) -> i64 {
        [UNDEAD_ASH, BLOODY_AXE_INSIGNIA, DELU_FANG, STAKATO_TALON]
            .iter()
            .map(|&id| self.count(ctx, id))
            .sum()
    }
    fn cargo_held(&self, ctx: &QuestCtx) -> i64 {
        (0..4).map(|i| self.count(ctx, CARGO_BOX_1ST + i)).sum()
    }
    /// Take the highest-priority cargo box the player holds (1st → 4th).
    fn take_one_cargo(&self, ctx: &mut QuestCtx) {
        for i in 0..4 {
            if self.count(ctx, CARGO_BOX_1ST + i) > 0 {
                ctx.take_items(CARGO_BOX_1ST + i, 1);
                return;
            }
        }
    }

    /// One Lion's Eye reward tier (Java's `30735-16` branches). `heal`/`ss`/`sps`/
    /// `esc`/`alac` are the tier's payout counts.
    fn lion_eye_reward(
        &self,
        ctx: &mut QuestCtx,
        heal: i64,
        ss: i64,
        sps: i64,
        esc: i64,
        alac: i64,
    ) {
        let chance = ctx.roll(100);
        if chance < 25 {
            ctx.give_items(HEALING_POTION, heal);
        } else if chance < 50 {
            if ctx.is_in_category("FIGHTER_GROUP") {
                ctx.give_items(SOULSHOT_D, ss);
            } else if ctx.is_in_category("MAGE_GROUP") {
                ctx.give_items(SPIRITSHOT_D, sps);
            }
        } else if chance < 75 {
            ctx.give_items(SCROLL_OF_ESCAPE, esc);
        } else {
            ctx.give_items(ALACRITY_POTION, alac);
        }
    }

    /// Turn in trophy materials to Sophya for Lion's Claws (by material count)
    /// and adena, then clear the materials. `cargo_present` picks the payout
    /// variant (Java's `30735-22` vs `-23`, whose adena formulas differ — the
    /// `22` one's `fang + 7` is a datapack quirk, kept as-is).
    fn sophya_material_turnin(&self, ctx: &mut QuestCtx, cargo_present: bool) {
        let item_count = self.materials_held(ctx);
        let claws = if item_count < 20 {
            0
        } else if item_count < 50 {
            1
        } else if item_count < 100 {
            2
        } else {
            3
        };
        if claws > 0 {
            ctx.give_items(LIONS_CLAW, claws);
        }
        let ash = self.count(ctx, UNDEAD_ASH);
        let insignia = self.count(ctx, BLOODY_AXE_INSIGNIA);
        let fang = self.count(ctx, DELU_FANG);
        let talon = self.count(ctx, STAKATO_TALON);
        let adena = if cargo_present {
            ash * 10 + insignia * 10 + fang * 7 + talon * 8
        } else {
            ash * 10 + insignia * 10 + (fang + 7) + talon * 8 // quirk kept verbatim
        };
        ctx.give_adena(adena, true);
        ctx.take_items(UNDEAD_ASH, -1);
        ctx.take_items(BLOODY_AXE_INSIGNIA, -1);
        ctx.take_items(DELU_FANG, -1);
        ctx.take_items(STAKATO_TALON, -1);
        ctx.set_memo_state(0);
    }

    /// Reedfoot's `30736-03` gamble: one cargo box for a random trade good (or,
    /// rarely, a Statue/Tablet piece). Returns the flavor html.
    fn reedfoot_gamble(&self, ctx: &mut QuestCtx) -> String {
        self.take_one_cargo(ctx);
        let chance = ctx.roll(100);
        let chance1 = ctx.roll(100);
        // `(low_id, mid_id, high_id, suffix_low, suffix_mid, suffix_high)` for a
        // chance1-split tier: 3444.. are the consecutive trade goods.
        let (item, suffix): (i32, &str) = if chance < 40 {
            triple(chance1, (3444, "04a"), (3445, "04b"), (3446, "04c"))
        } else if chance < 60 {
            triple(chance1, (3447, "04d"), (3448, "04e"), (3449, "04f"))
        } else if chance < 70 {
            triple(chance1, (3450, "04g"), (3451, "04h"), (3452, "04i"))
        } else if chance < 75 {
            triple(chance1, (3453, "04j"), (3454, "04k"), (3455, "04l"))
        } else if chance < 76 {
            (3456, "04m") // Imperial Diamond
        } else if ctx.roll(100) < 50 {
            // A random Statue-of-Shilen piece.
            let piece = STATUE_HEAD + quarter(chance1);
            ctx.give_items(piece, 1);
            return format!("30736-{}.html", "04n");
        } else {
            // A random Ancient-Tablet fragment.
            let frag = TABLET_1ST + quarter(chance1);
            ctx.give_items(frag, 1);
            return format!("30736-{}.html", "04o");
        };
        ctx.give_items(item, 1);
        format!("30736-{suffix}.html")
    }

    /// Reedfoot's `30736-07` fortune-telling: costs `200 + memoState·200` adena
    /// for one of twenty flavor readings, then the price rises.
    fn reedfoot_fortune(&self, ctx: &mut QuestCtx) -> String {
        let memo = ctx.memo_state();
        let cost = 200 + memo as i64 * 200;
        if self.count(ctx, ADENA) < cost {
            return "30736-07.html".to_string();
        }
        if memo * 100 > 200 {
            return "30736-08.html".to_string();
        }
        // Twenty readings in 5% bands (a..t).
        let band = (ctx.roll(100) / 5).min(19) as u8;
        let letter = (b'a' + band) as char;
        ctx.take_items(ADENA, cost);
        ctx.set_memo_state(memo + 1);
        format!("30736-08{letter}.html")
    }

    /// Morgon's `30737-06`: one cargo box → a Guild Coin (or, at 80 coins, cash
    /// them all in), then a small adena tip scaled by the coin balance.
    fn morgon_exchange(&self, ctx: &mut QuestCtx) -> String {
        if self.cargo_held(ctx) < 1 {
            return "30737-06.html".to_string();
        }
        self.take_one_cargo(ctx);
        if self.count(ctx, GUILD_COIN) < 80 {
            ctx.give_items(GUILD_COIN, 1);
        } else {
            ctx.take_items(GUILD_COIN, 80);
        }
        let coins = self.count(ctx, GUILD_COIN);
        if coins < 40 {
            ctx.give_adena(100, true);
            "30737-03.html".to_string()
        } else if coins < 80 {
            ctx.give_adena(200, true);
            "30737-04.html".to_string()
        } else {
            ctx.give_adena(300, true);
            "30737-05.html".to_string()
        }
    }

    /// Rupio's assembly: four `parts` (a consecutive run) combine, 50/50, into
    /// `whole` — either way the parts are consumed. Returns `(success_html,
    /// fail_html)` choice.
    fn rupio_assemble(
        &self,
        ctx: &mut QuestCtx,
        first_part: i32,
        whole: i32,
        success: &'static str,
        fail: &'static str,
    ) -> &'static str {
        let success_roll = ctx.roll(100) < 50;
        if success_roll {
            ctx.give_items(whole, 1);
        }
        for i in 0..4 {
            ctx.take_items(first_part + i, 1);
        }
        if success_roll {
            success
        } else {
            fail
        }
    }
}

/// Java's `chance1 < 33 / < 66 / else` split over three items.
fn triple(
    chance1: i32,
    low: (i32, &'static str),
    mid: (i32, &'static str),
    high: (i32, &'static str),
) -> (i32, &'static str) {
    if chance1 < 33 {
        low
    } else if chance1 < 66 {
        mid
    } else {
        high
    }
}

/// Java's `chance1 < 25 / < 50 / < 75 / else` → a 0..=3 index.
fn quarter(chance1: i32) -> i32 {
    if chance1 < 25 {
        0
    } else if chance1 < 50 {
        1
    } else if chance1 < 75 {
        2
    } else {
        3
    }
}

impl QuestScript for Q00333HuntOfTheBlackLion {
    fn id(&self) -> i32 {
        333
    }
    fn name(&self) -> &'static str {
        "Q00333_HuntOfTheBlackLion"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00333_HuntOfTheBlackLion"
    }
    fn start_npcs(&self) -> &[i32] {
        &[SOPHYA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[SOPHYA, UNDRIAS, RUPIO, LOCKIRIN, REEDFOOT, MORGON]
    }
    fn kill_npcs(&self) -> &[i32] {
        &self.kill_ids
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30735-04.htm" => {
                if ctx.is_created() {
                    ctx.start_quest();
                }
                Some(event.to_string())
            }
            // Informational pages, echoed straight back.
            "30735-05.html" | "30735-06.html" | "30735-07.html" | "30735-08.html"
            | "30735-09.html" | "30130-05.html" | "30531-05.html" | "30735-21.html"
            | "30735-24a.html" | "30735-25b.html" | "30736-06.html" | "30736-09.html"
            | "30737-07.html" => Some(event.to_string()),
            // Take one of Sophya's four orders.
            "30735-10.html" | "30735-11.html" | "30735-12.html" | "30735-13.html" => {
                // "30735-10".."13" map to the 1st..4th order (index = last digit).
                let order = SOPHYAS_1ST_ORDER + (event.as_bytes()[7] - b'0') as i32;
                if !self.has(ctx, order) {
                    ctx.give_items(order, 1);
                }
                Some(event.to_string())
            }
            "30735-16.html" => {
                let claws = self.count(ctx, LIONS_CLAW);
                let eyes = self.count(ctx, LIONS_EYE);
                if claws < 10 {
                    return Some(event.to_string());
                }
                if eyes < 4 {
                    ctx.give_items(LIONS_EYE, 1);
                    self.lion_eye_reward(ctx, 20, 100, 50, 20, 3);
                    ctx.take_items(LIONS_CLAW, 10);
                    Some("30735-17a.html".to_string())
                } else if eyes <= 7 {
                    ctx.give_items(LIONS_EYE, 1);
                    self.lion_eye_reward(ctx, 25, 200, 100, 20, 3);
                    ctx.take_items(LIONS_CLAW, 10);
                    Some("30735-18b.html".to_string())
                } else {
                    ctx.take_items(LIONS_EYE, 8);
                    self.lion_eye_reward(ctx, 50, 400, 200, 30, 4);
                    ctx.take_items(LIONS_CLAW, 10);
                    Some("30735-19b.html".to_string())
                }
            }
            "30735-20.html" => {
                for i in 0..4 {
                    ctx.take_items(SOPHYAS_1ST_ORDER + i, -1);
                }
                Some(event.to_string())
            }
            "30735-26.html" => {
                if self.has(ctx, BLACK_LION_MARK) {
                    ctx.give_adena(12400, true);
                    ctx.exit_quest(true, true);
                    return Some(event.to_string());
                }
                None
            }
            "30130-04.html" => {
                if self.has(ctx, COMPLETE_STATUE) {
                    ctx.give_adena(30000, true);
                    ctx.take_items(COMPLETE_STATUE, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30471-03.html" => {
                if [0, 1, 2, 3].iter().all(|&i| self.has(ctx, STATUE_HEAD + i)) {
                    Some(
                        self.rupio_assemble(
                            ctx,
                            STATUE_HEAD,
                            COMPLETE_STATUE,
                            "30471-04.html",
                            "30471-05.html",
                        )
                        .to_string(),
                    )
                } else {
                    Some(event.to_string())
                }
            }
            "30471-06.html" => {
                if [0, 1, 2, 3].iter().all(|&i| self.has(ctx, TABLET_1ST + i)) {
                    Some(
                        self.rupio_assemble(
                            ctx,
                            TABLET_1ST,
                            COMPLETE_TABLET,
                            "30471-07.html",
                            "30471-08.html",
                        )
                        .to_string(),
                    )
                } else {
                    Some(event.to_string())
                }
            }
            "30531-04.html" => {
                if self.has(ctx, COMPLETE_TABLET) {
                    ctx.give_adena(30000, true);
                    ctx.take_items(COMPLETE_TABLET, 1);
                    return Some(event.to_string());
                }
                None
            }
            "30736-03.html" => Some(if self.cargo_held(ctx) >= 1 {
                self.reedfoot_gamble(ctx)
            } else {
                "30736-05.html".to_string()
            }),
            "30736-07.html" => Some(self.reedfoot_fortune(ctx)),
            "30737-06.html" => Some(self.morgon_exchange(ctx)),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        // The two ambush bosses drop a bulk of their material.
        if npc_id == DELU_HEADHUNTER {
            if self.has(ctx, SOPHYAS_1ST_ORDER + 2) {
                ctx.give_items(DELU_FANG, 4);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
            return;
        }
        if npc_id == MARSH_STAKATO_MARQUESS {
            if self.has(ctx, SOPHYAS_1ST_ORDER + 3) {
                ctx.give_items(STAKATO_TALON, 8);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
            return;
        }
        let Some(&(_, order, material, mat_pct, cargo_pct)) = DROPS.iter().find(|d| d.0 == npc_id)
        else {
            return;
        };
        let order_item = SOPHYAS_1ST_ORDER + (order - 1);
        if !self.has(ctx, order_item) {
            return;
        }
        if ctx.roll(100) < mat_pct {
            ctx.give_items(material, 1);
        }
        if ctx.roll(100) < cargo_pct {
            ctx.give_items(CARGO_BOX_1ST + (order - 1), 1);
        }
        // Order-3 mobs may conjure two Headhunters, order-4 mobs one Marquess.
        if order == 3 && ctx.roll(100) < 3 {
            ctx.spawn_attacker(DELU_HEADHUNTER, true);
            ctx.spawn_attacker(DELU_HEADHUNTER, true);
        } else if order == 4 && ctx.roll(100) < 2 {
            ctx.spawn_attacker(MARSH_STAKATO_MARQUESS, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc_id = ctx.npc_id;
        if ctx.is_created() {
            if npc_id == SOPHYA {
                return Some(
                    if ctx.player_level() < MIN_LEVEL {
                        "30735-01.htm"
                    } else if !self.has(ctx, BLACK_LION_MARK) {
                        "30735-02.htm"
                    } else {
                        "30735-03.htm"
                    }
                    .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let (orders, materials, cargo) = (
            self.orders_held(ctx),
            self.materials_held(ctx),
            self.cargo_held(ctx),
        );
        match npc_id {
            SOPHYA => Some(if orders == 0 {
                "30735-14.html".to_string()
            } else if orders == 1 && materials < 1 && cargo < 1 {
                "30735-15.html".to_string()
            } else if orders == 1 && materials < 1 && cargo >= 1 {
                "30735-15a.html".to_string()
            } else if orders == 1 && materials >= 1 && cargo == 0 {
                self.sophya_material_turnin(ctx, false);
                "30735-22.html".to_string()
            } else if orders == 1 && materials >= 1 && cargo >= 1 {
                self.sophya_material_turnin(ctx, true);
                "30735-23.html".to_string()
            } else {
                ctx.no_quest_html()
            }),
            UNDRIAS => Some(
                if self.has(ctx, COMPLETE_STATUE) {
                    "30130-03.html"
                } else if (0..4).any(|i| self.has(ctx, STATUE_HEAD + i)) {
                    "30130-02.html"
                } else {
                    "30130-01.html"
                }
                .to_string(),
            ),
            RUPIO => Some(
                if (0..4).any(|i| self.has(ctx, STATUE_HEAD + i))
                    || (0..4).any(|i| self.has(ctx, TABLET_1ST + i))
                {
                    "30471-02.html"
                } else {
                    "30471-01.html"
                }
                .to_string(),
            ),
            LOCKIRIN => Some(
                if self.has(ctx, COMPLETE_TABLET) {
                    "30531-03.html"
                } else if (0..4).any(|i| self.has(ctx, TABLET_1ST + i)) {
                    "30531-02.html"
                } else {
                    "30531-01.html"
                }
                .to_string(),
            ),
            REEDFOOT => Some(
                if cargo >= 1 {
                    "30736-02.html"
                } else {
                    "30736-01.html"
                }
                .to_string(),
            ),
            MORGON => Some(
                if cargo >= 1 {
                    "30737-02.html"
                } else {
                    "30737-01.html"
                }
                .to_string(),
            ),
            _ => Some(ctx.no_quest_html()),
        }
    }
}
