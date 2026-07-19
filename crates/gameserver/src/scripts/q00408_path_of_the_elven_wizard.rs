//! Path Of The Elven Wizard (408) — port of
//! `dist/game/data/scripts/quests/Q00408_PathOfTheElvenWizard/`.
//!
//! Awards the **Eternity Diamond** (1230) — the last of `ElfHumanWizardChange1`'s
//! four proofs. With this, every Elf/Human first occupation is earnable in
//! normal play.
//!
//! Rossela runs **three parallel errands**, all required, in any order. Each is
//! the same four beats: Rossela hands out an introduction, the specialist swaps
//! it for a charm, the charm gates a monster drop, and the specialist trades
//! the full set for a gem. Three gems buy the diamond.
//!
//! | # | Introduction | Specialist | Charm | Mob | Material | Need | Chance | Gem |
//! |---|---|---|---|---|---|---|---|---|
//! | 1 | Rossela's Letter | Greenis | Greenis's Charm | Pincer Spider | Red Down | 5 | 70% | Ruby |
//! | 2 | Appetizing Apple | Thalia | Sap of the Mother Tree | Dryad Elder | Gold Leaves | 5 | 40% | Aquamarine |
//! | 3 | Immortal Love | Northwind | Lucky Potpourri | Sukar Wererat Leader | Amethyst | 2 | 40% | Nobility Amethyst |
//!
//! ## The third errand is missing a step, and the dist proves it
//!
//! Errands 1 and 2 perform the introduction → charm swap in a **dialog event**
//! (`30157-02.html`, `30371-02.html`). Errand 3 has no such event: Northwind
//! does the swap inline in `onTalk`. That looks like an oversight to normalise
//! until you count pages — Greenis and Thalia each ship four, **Northwind ships
//! only three** (`30423-01..03`). There is no fourth page to route an event to,
//! so inventing one would 404. The page test asserts the absence.
//!
//! ## Never advances `cond`
//!
//! Like quest 402, this one calls `setCond` **zero** times in 446 lines
//! (verified by grep, not inferred): `startQuest` sets cond 1 and it stays
//! there. Progress is tracked purely by which items you hold, which is why the
//! `onTalk` chains read as long item interrogations. The cap-reached case plays
//! the middle sound and nothing else.
//!
//! Chance denominator is `/100` here, as in 404/406 — not the `/10` of 401/403.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const ROSSELA: i32 = 30414;
const GREENIS: i32 = 30157;
const THALIA: i32 = 30371;
const NORTHWIND: i32 = 30423;

const ROSELLAS_LETTER: i32 = 1218;
const RED_DOWN: i32 = 1219;
const MAGICAL_POWERS_RUBY: i32 = 1220;
const PURE_AQUAMARINE: i32 = 1221;
const APPETIZING_APPLE: i32 = 1222;
const GOLD_LEAVES: i32 = 1223;
const IMMORTAL_LOVE: i32 = 1224;
const AMETHYST: i32 = 1225;
const NOBILITY_AMETHYST: i32 = 1226;
const FERTILITY_PERIDOT: i32 = 1229;
const ETERNITY_DIAMOND: i32 = 1230;
const GREENISS_CHARM: i32 = 1272;
const SAP_OF_THE_MOTHER_TREE: i32 = 1273;
const LUCKY_POTPOURRI: i32 = 1274;

const DRYAD_ELDER: i32 = 20019;
const SUKAR_WERERAT_LEADER: i32 = 20047;
const PINCER_SPIDER: i32 = 20466;

const ELVEN_MAGE: i32 = 25;
const ELVEN_WIZARD: i32 = 26;
const MIN_LEVEL: i32 = 19;

/// One of Rossela's three errands. `swap_event` is `None` for Northwind, who
/// swaps inline in `onTalk` — see the module header.
struct Errand {
    npc: i32,
    intro: i32,
    charm: i32,
    mob: i32,
    material: i32,
    need: i64,
    chance: i32,
    gem: i32,
    swap_event: Option<&'static str>,
    /// Rossela's: (offer event, give-intro page, holding-intro page,
    /// collecting page, collected page).
    offer_event: &'static str,
    give_intro: &'static str,
    holding_intro: &'static str,
    collecting: &'static str,
    collected: &'static str,
    /// The specialist's: (holding-intro, collecting, trade).
    npc_intro: &'static str,
    npc_collecting: &'static str,
    npc_trade: &'static str,
}

const ERRANDS: [Errand; 3] = [
    Errand {
        npc: GREENIS,
        intro: ROSELLAS_LETTER,
        charm: GREENISS_CHARM,
        mob: PINCER_SPIDER,
        material: RED_DOWN,
        need: 5,
        chance: 70,
        gem: MAGICAL_POWERS_RUBY,
        swap_event: Some("30157-02.html"),
        offer_event: "30414-10.html",
        give_intro: "30414-07.html",
        holding_intro: "30414-08.html",
        collecting: "30414-09.html",
        collected: "30414-21.html",
        npc_intro: "30157-01.html",
        npc_collecting: "30157-03.html",
        npc_trade: "30157-04.html",
    },
    Errand {
        npc: THALIA,
        intro: APPETIZING_APPLE,
        charm: SAP_OF_THE_MOTHER_TREE,
        mob: DRYAD_ELDER,
        material: GOLD_LEAVES,
        need: 5,
        chance: 40,
        gem: PURE_AQUAMARINE,
        swap_event: Some("30371-02.html"),
        offer_event: "30414-12.html",
        give_intro: "30414-13.html",
        holding_intro: "30414-14.html",
        collecting: "30414-15.html",
        collected: "30414-22.html",
        npc_intro: "30371-01.html",
        npc_collecting: "30371-03.html",
        npc_trade: "30371-04.html",
    },
    Errand {
        npc: NORTHWIND,
        intro: IMMORTAL_LOVE,
        charm: LUCKY_POTPOURRI,
        mob: SUKAR_WERERAT_LEADER,
        material: AMETHYST,
        need: 2,
        chance: 40,
        gem: NOBILITY_AMETHYST,
        // No dialog event: Northwind ships only three pages.
        swap_event: None,
        offer_event: "30414-16.html",
        give_intro: "30414-17.html",
        holding_intro: "30414-18.html",
        collecting: "30414-19.html",
        collected: "30414-23.html",
        npc_intro: "30423-01.html",
        npc_collecting: "30423-02.html",
        npc_trade: "30423-03.html",
    },
];

/// Every item that means "an errand is in flight" — Rossela's idle/completion
/// branches both require none of these.
const IN_FLIGHT: [i32; 6] = [
    ROSELLAS_LETTER, APPETIZING_APPLE, IMMORTAL_LOVE, GREENISS_CHARM, SAP_OF_THE_MOTHER_TREE,
    LUCKY_POTPOURRI,
];

const QUEST_ITEMS: [i32; 13] = [
    ROSELLAS_LETTER, RED_DOWN, MAGICAL_POWERS_RUBY, PURE_AQUAMARINE, APPETIZING_APPLE, GOLD_LEAVES,
    IMMORTAL_LOVE, AMETHYST, NOBILITY_AMETHYST, FERTILITY_PERIDOT, GREENISS_CHARM,
    SAP_OF_THE_MOTHER_TREE, LUCKY_POTPOURRI,
];

pub struct Q00408PathOfTheElvenWizard;

impl Q00408PathOfTheElvenWizard {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn any_in_flight(&self, ctx: &QuestCtx) -> bool {
        IN_FLIGHT.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }
    fn has_all_gems(&self, ctx: &QuestCtx) -> bool {
        ERRANDS.iter().all(|e| ctx.quest_items_count(e.gem) > 0)
    }

    /// Rossela's per-errand offer button: echo if the gem is already won,
    /// otherwise hand out the introduction.
    fn offer(&self, ctx: &mut QuestCtx, e: &Errand, event: &str) -> Option<String> {
        if self.has(ctx, e.gem) {
            return Some(event.to_string());
        }
        if !self.has(ctx, FERTILITY_PERIDOT) {
            return None;
        }
        if !self.has(ctx, e.intro) {
            ctx.give_items(e.intro, 1);
        }
        Some(e.give_intro.to_string())
    }

    /// The introduction → charm swap. Errands 1 and 2 reach this from a dialog
    /// event; errand 3 from `onTalk` directly.
    fn swap_intro_for_charm(&self, ctx: &mut QuestCtx, e: &Errand) {
        if self.has(ctx, e.intro) {
            ctx.take_items(e.intro, 1);
            if !self.has(ctx, e.charm) {
                ctx.give_items(e.charm, 1);
            }
        }
    }
}

impl QuestScript for Q00408PathOfTheElvenWizard {
    fn id(&self) -> i32 {
        408
    }
    fn name(&self) -> &'static str {
        "Q00408_PathOfTheElvenWizard"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00408_PathOfTheElvenWizard"
    }
    fn start_npcs(&self) -> &[i32] {
        &[ROSSELA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[ROSSELA, GREENIS, THALIA, NORTHWIND]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[DRYAD_ELDER, SUKAR_WERERAT_LEADER, PINCER_SPIDER]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        if let Some(e) = ERRANDS.iter().find(|e| e.offer_event == event) {
            return self.offer(ctx, e, event);
        }
        // The two specialists that do have a swap page.
        if let Some(e) = ERRANDS.iter().find(|e| e.swap_event == Some(event)) {
            self.swap_intro_for_charm(ctx, e);
            return Some(event.to_string());
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                ELVEN_MAGE if ctx.player_level() < MIN_LEVEL => "30414-04.htm".to_string(),
                ELVEN_MAGE if self.has(ctx, ETERNITY_DIAMOND) => "30414-05.htm".to_string(),
                ELVEN_MAGE => {
                    if !self.has(ctx, FERTILITY_PERIDOT) {
                        ctx.give_items(FERTILITY_PERIDOT, 1);
                    }
                    ctx.start_quest();
                    "30414-06.htm".to_string()
                }
                ELVEN_WIZARD => "30414-02a.htm".to_string(),
                _ => "30414-03.htm".to_string(),
            }),
            "30414-02.htm" => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        let Some(e) = ERRANDS.iter().find(|e| e.mob == npc_id) else { return };
        // The charm is the drop gate; without it the mob is just a mob.
        if !self.has(ctx, e.charm) || ctx.quest_items_count(e.material) >= e.need {
            return;
        }
        if ctx.roll(100) >= e.chance {
            return;
        }
        ctx.give_items(e.material, 1);
        // No `setCond` anywhere in this quest — just the sound.
        if ctx.quest_items_count(e.material) == e.need {
            ctx.play_sound(quest_sounds::MIDDLE);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == ROSSELA {
                return Some("30414-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        if npc == ROSSELA {
            return self.talk_rossela(ctx);
        }
        let Some(e) = ERRANDS.iter().find(|e| e.npc == npc) else {
            return Some(ctx.no_quest_html());
        };
        // Northwind alone performs the swap here rather than via an event.
        if e.swap_event.is_none() && self.has(ctx, e.intro) {
            self.swap_intro_for_charm(ctx, e);
            return Some(e.npc_intro.to_string());
        }
        if self.has(ctx, e.intro) {
            return Some(e.npc_intro.to_string());
        }
        if self.has(ctx, e.charm) {
            if ctx.quest_items_count(e.material) < e.need {
                return Some(e.npc_collecting.to_string());
            }
            ctx.take_items(e.material, -1);
            if !self.has(ctx, e.gem) {
                ctx.give_items(e.gem, 1);
            }
            ctx.take_items(e.charm, 1);
            return Some(e.npc_trade.to_string());
        }
        Some(ctx.no_quest_html())
    }
}

impl Q00408PathOfTheElvenWizard {
    fn talk_rossela(&self, ctx: &mut QuestCtx) -> Option<String> {
        let idle = !self.any_in_flight(ctx) && self.has(ctx, FERTILITY_PERIDOT);
        if idle && !self.has_all_gems(ctx) {
            // The errand menu.
            return Some("30414-11.html".to_string());
        }
        for e in &ERRANDS {
            if self.has(ctx, e.intro) {
                return Some(e.holding_intro.to_string());
            }
            if self.has(ctx, e.charm) {
                return Some(
                    if ctx.quest_items_count(e.material) < e.need { e.collecting } else { e.collected }
                        .to_string(),
                );
            }
        }
        if idle && self.has_all_gems(ctx) {
            if !self.has(ctx, ETERNITY_DIAMOND) {
                ctx.give_items(ETERNITY_DIAMOND, 1);
            }
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30414-20.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
