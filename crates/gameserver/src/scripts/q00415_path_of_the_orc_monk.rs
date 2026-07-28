//! Path Of The Orc Monk (415) — port of
//! `dist/game/data/scripts/quests/Q00415_PathOfTheOrcMonk/`. At 652 Java lines
//! the widest quest in the Path family.
//!
//! Awards the **Khavatari Totem** (1615), `OrcChange1`'s second proof.
//!
//! ## The weapon gate is the *inverse* of quests 401/403
//!
//! Those demand a specific quest weapon. This one demands **bare hands or a
//! fist weapon**:
//!
//! ```java
//! return ((weapon == null) || (weapon.getItemType() == WeaponType.FIST)
//!                          || (weapon.getItemType() == WeaponType.DUALFIST));
//! ```
//!
//! An Orc Monk fights unarmed, so "no weapon" is the **pass** case — the exact
//! opposite of the `quest_common` tag, where an empty hand disqualifies you.
//! Reusing that helper here would have silently inverted the whole quest.
//! `QuestCtx::is_bare_or_fist_handed` is added for it (weapon *type*, not id).
//!
//! The 0 → 1 → 2 attacker/weapon state machine is otherwise the familiar one,
//! keyed on `Q00415_last_attacker` — a third distinct variable name after
//! `lastAttacker` (401/403) and `firstAttacker` (409).
//!
//! ## The pouch stages take five kills each, not four
//!
//! Java gives a trophy per kill and converts the pouch when the count is
//! *already* 4:
//!
//! ```java
//! if (getQuestItemsCount(killer, KASHA_BEAR_CLAW) == 4) { …convert… }
//! else { giveItems(killer, KASHA_BEAR_CLAW, 1); }
//! ```
//!
//! so the fifth kill is the one that fills the pouch, and the four trophies
//! are consumed. Reading it as "collect 4" would leave the pouch unfillable.
//! The fourth pouch works the same way across **four** mobs at three each,
//! converting once the combined count reaches 11 (i.e. on the twelfth kill).
//!
//! ## Half this quest is unreachable — dead at both ends, like 414
//!
//! `30587-09c` sets `memoState = 2` and opens an entire alternate ending
//! through NPCs **31979** and **32056** (Kasha spiders, Baar Dre Vanul, its own
//! reward hand-out at `31979-03`). None of it can be reached:
//!
//! - **`30587-09a.html` offers only the `09b` button** — nothing posts `09c`.
//! - **31979 and 32056 are registered nowhere** — not here, not in any other
//!   shipped script — so their **13 pages** are orphaned.
//!
//! Same two-sided orphaning as quest 414, and again checked in both
//! directions: had only the serving end been missing, `09c` would be a trap
//! (it consumes Rosheek's letter and hands out no recommendation). Ported
//! verbatim with `TODO(dead)` markers; the `KASHA_SPIDERS_TOOTH` and
//! `BAAR_DRE_VANUL` kill handlers below are part of the same dead route.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const PREFECT_KASMAN: i32 = 30501;
const GANTAKI_ZU_URUTU: i32 = 30587;
const KHAVATARI_ROSHEEK: i32 = 30590;
const KHAVATARI_TORUKU: i32 = 30591;

const POMEGRANATE: i32 = 1593;
const LEATHER_POUCH_1ST: i32 = 1594;
const LEATHER_POUCH_2ND: i32 = 1595;
const LEATHER_POUCH_3RD: i32 = 1596;
const LEATHER_POUCH_1ST_FULL: i32 = 1597;
const LEATHER_POUCH_2ND_FULL: i32 = 1598;
const LEATHER_POUCH_3RD_FULL: i32 = 1599;
const KASHA_BEAR_CLAW: i32 = 1600;
const KASHA_BLADE_SPIDER_TALON: i32 = 1601;
const SCARLET_SALAMANDER_SCALE: i32 = 1602;
const FIERY_SPIRIT_SCROLL: i32 = 1603;
const ROSHEEKS_LETTER: i32 = 1604;
const GANTAKIS_RECOMMENDATION: i32 = 1605;
const FIG: i32 = 1606;
const LEATHER_POUCH_4TH: i32 = 1607;
const LEATHER_POUCH_4TH_FULL: i32 = 1608;
const VUKU_ORC_TUSK: i32 = 1609;
const RATMAN_FANG: i32 = 1610;
const LANGK_LIZARDMAN_TOOTH: i32 = 1611;
const FELIM_LIZARDMAN_TOOTH: i32 = 1612;
const IRON_WILL_SCROLL: i32 = 1613;
const TORUKUS_LETTER: i32 = 1614;
const KHAVATARI_TOTEM: i32 = 1615;
/// Dead-route items — see the module header.
const KASHA_SPIDERS_TOOTH: i32 = 8545;
const HORN_OF_BAAR_DRE_VANUL: i32 = 8546;

const FELIM_LIZARDMAN_WARRIOR: i32 = 20014;
const VUKU_ORC_FIGHTER: i32 = 20017;
const LANGK_LIZARDMAN_WARRIOR: i32 = 20024;
const RATMAN_WARRIOR: i32 = 20359;
const SCARLET_SALAMANDER: i32 = 20415;
const KASHA_FANG_SPIDER: i32 = 20476;
const KASHA_BLADE_SPIDER: i32 = 20478;
const KASHA_BEAR: i32 = 20479;
const BAAR_DRE_VANUL: i32 = 21118;

const ORC_FIGHTER: i32 = 44;
const ORC_MONK: i32 = 47;
const MIN_LEVEL: i32 = 19;

/// Java's variable key — note the quest-scoped name, unlike 401/403's
/// `lastAttacker` and 409's `firstAttacker`.
const LAST_ATTACKER: &str = "Q00415_last_attacker";

/// The first three pouches: `(pouch, full pouch, trophy, mob, cond)`. Each
/// needs four trophies, and the *fifth* kill converts.
type Pouch = (i32, i32, i32, i32, i32);
const POUCHES: [Pouch; 3] = [
    (
        LEATHER_POUCH_1ST,
        LEATHER_POUCH_1ST_FULL,
        KASHA_BEAR_CLAW,
        KASHA_BEAR,
        3,
    ),
    (
        LEATHER_POUCH_2ND,
        LEATHER_POUCH_2ND_FULL,
        KASHA_BLADE_SPIDER_TALON,
        KASHA_BLADE_SPIDER,
        5,
    ),
    (
        LEATHER_POUCH_3RD,
        LEATHER_POUCH_3RD_FULL,
        SCARLET_SALAMANDER_SCALE,
        SCARLET_SALAMANDER,
        7,
    ),
];

/// One rung of Rosheek's pouch ladder: `(carried, full, next pouch, cond,
/// still-collecting page, hand-over page)`. `next` is `None` on the last rung,
/// which pays the scroll and his letter instead.
type Rung = (i32, i32, Option<i32>, i32, &'static str, &'static str);

/// The fourth pouch's four tributaries: `(mob, trophy)`, three of each.
const FOURTH: [(i32, i32); 4] = [
    (FELIM_LIZARDMAN_WARRIOR, FELIM_LIZARDMAN_TOOTH),
    (VUKU_ORC_FIGHTER, VUKU_ORC_TUSK),
    (LANGK_LIZARDMAN_WARRIOR, LANGK_LIZARDMAN_TOOTH),
    (RATMAN_WARRIOR, RATMAN_FANG),
];
const FOURTH_TROPHIES: [i32; 4] = [
    VUKU_ORC_TUSK,
    RATMAN_FANG,
    LANGK_LIZARDMAN_TOOTH,
    FELIM_LIZARDMAN_TOOTH,
];

const TAGGED_MOBS: [i32; 9] = [
    FELIM_LIZARDMAN_WARRIOR,
    VUKU_ORC_FIGHTER,
    LANGK_LIZARDMAN_WARRIOR,
    RATMAN_WARRIOR,
    SCARLET_SALAMANDER,
    KASHA_FANG_SPIDER,
    KASHA_BLADE_SPIDER,
    KASHA_BEAR,
    BAAR_DRE_VANUL,
];

const QUEST_ITEMS: [i32; 24] = [
    POMEGRANATE,
    LEATHER_POUCH_1ST,
    LEATHER_POUCH_2ND,
    LEATHER_POUCH_3RD,
    LEATHER_POUCH_1ST_FULL,
    LEATHER_POUCH_2ND_FULL,
    LEATHER_POUCH_3RD_FULL,
    KASHA_BEAR_CLAW,
    KASHA_BLADE_SPIDER_TALON,
    SCARLET_SALAMANDER_SCALE,
    FIERY_SPIRIT_SCROLL,
    ROSHEEKS_LETTER,
    GANTAKIS_RECOMMENDATION,
    FIG,
    LEATHER_POUCH_4TH,
    LEATHER_POUCH_4TH_FULL,
    VUKU_ORC_TUSK,
    RATMAN_FANG,
    LANGK_LIZARDMAN_TOOTH,
    FELIM_LIZARDMAN_TOOTH,
    IRON_WILL_SCROLL,
    TORUKUS_LETTER,
    KASHA_SPIDERS_TOOTH,
    HORN_OF_BAAR_DRE_VANUL,
];

pub struct Q00415PathOfTheOrcMonk;

impl Q00415PathOfTheOrcMonk {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn has_any(&self, ctx: &QuestCtx, items: &[i32]) -> bool {
        items.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }
    fn fourth_total(&self, ctx: &QuestCtx) -> i64 {
        FOURTH_TROPHIES
            .iter()
            .map(|id| ctx.quest_items_count(*id))
            .sum()
    }
    /// Fill the fourth pouch and clear all four trophy piles.
    fn fill_fourth_pouch(&self, ctx: &mut QuestCtx) {
        ctx.take_items(LEATHER_POUCH_4TH, 1);
        ctx.give_items(LEATHER_POUCH_4TH_FULL, 1);
        for id in FOURTH_TROPHIES {
            ctx.take_items(id, -1);
        }
        ctx.set_cond(12, true);
    }
}

impl QuestScript for Q00415PathOfTheOrcMonk {
    fn id(&self) -> i32 {
        415
    }
    fn name(&self) -> &'static str {
        "Q00415_PathOfTheOrcMonk"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00415_PathOfTheOrcMonk"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GANTAKI_ZU_URUTU]
    }
    /// 31979 and 32056 are deliberately absent — see the module header.
    fn talk_npcs(&self) -> &[i32] {
        &[
            GANTAKI_ZU_URUTU,
            PREFECT_KASMAN,
            KHAVATARI_ROSHEEK,
            KHAVATARI_TORUKU,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &TAGGED_MOBS
    }
    fn attack_npcs(&self) -> &[i32] {
        &TAGGED_MOBS
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
                    ORC_FIGHTER if ctx.player_level() < MIN_LEVEL => "30587-03.htm",
                    ORC_FIGHTER if self.has(ctx, KHAVATARI_TOTEM) => "30587-04.htm",
                    ORC_FIGHTER => "30587-05.htm",
                    ORC_MONK => "30587-02a.htm",
                    _ => "30587-02.htm",
                }
                .to_string(),
            ),
            "30587-06.htm" => {
                ctx.start_quest();
                ctx.give_items(POMEGRANATE, 1);
                Some(event.to_string())
            }
            // The live continuation: Rosheek's letter becomes Gantaki's
            // recommendation.
            "30587-09b.html" => {
                if self.has(ctx, FIERY_SPIRIT_SCROLL) && self.has(ctx, ROSHEEKS_LETTER) {
                    ctx.take_items(ROSHEEKS_LETTER, 1);
                    ctx.give_items(GANTAKIS_RECOMMENDATION, 1);
                    ctx.set_cond(9, false); // Java's single-arg setCond — no sound
                    return Some(event.to_string());
                }
                None
            }
            // TODO(dead): `09c` and every `31979`/`32056` event below belong to
            // the alternate ending, which nothing can reach — no page offers
            // `09c` and neither NPC is registered. Do not restore one end
            // without the other: this route takes Rosheek's letter and hands
            // out no recommendation.
            "30587-09c.html" => {
                if self.has(ctx, FIERY_SPIRIT_SCROLL) && self.has(ctx, ROSHEEKS_LETTER) {
                    ctx.take_items(ROSHEEKS_LETTER, 1);
                    ctx.set_memo_state(2);
                    ctx.set_cond(14, false);
                    return Some(event.to_string());
                }
                None
            }
            "31979-02.html" => (ctx.memo_state() == 5).then(|| event.to_string()),
            "31979-03.html" => {
                if ctx.memo_state() == 5 {
                    ctx.give_items(KHAVATARI_TOTEM, 1);
                    ctx.add_exp_and_sp(80314, 5087);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some(event.to_string());
                }
                None
            }
            "31979-04.html" => {
                if ctx.memo_state() == 5 {
                    ctx.set_cond(20, false);
                    return Some(event.to_string());
                }
                None
            }
            "32056-02.html" => (ctx.memo_state() == 2).then(|| event.to_string()),
            "32056-03.html" => {
                if ctx.memo_state() == 2 {
                    ctx.set_memo_state(3);
                    ctx.set_cond(15, false);
                    return Some(event.to_string());
                }
                None
            }
            "32056-08.html" => {
                if ctx.memo_state() == 4 && self.has(ctx, HORN_OF_BAAR_DRE_VANUL) {
                    ctx.take_items(HORN_OF_BAAR_DRE_VANUL, -1);
                    ctx.set_memo_state(5);
                    ctx.set_cond(19, false);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    /// The 0 → 1 → 2 tag, but gated on **fists or bare hands**.
    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let ok = ctx.is_bare_or_fist_handed();
        match ctx.npc_script_value() {
            0 => {
                if ok {
                    ctx.set_npc_script_value(1);
                    let attacker = ctx.player;
                    ctx.set_npc_var_int(LAST_ATTACKER, attacker);
                } else {
                    ctx.set_npc_script_value(2);
                }
            }
            1 if !ok || ctx.npc_var_int(LAST_ATTACKER) != ctx.player => {
                ctx.set_npc_script_value(2);
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() || ctx.npc_script_value() != 1 {
            return;
        }
        let npc_id = ctx.npc_id;

        // Pouches 1-3: four trophies, and the fifth kill converts.
        if let Some(&(pouch, full, trophy, _, cond)) =
            POUCHES.iter().find(|(_, _, _, mob, _)| *mob == npc_id)
            && self.has(ctx, pouch)
        {
            if ctx.quest_items_count(trophy) == 4 {
                ctx.take_items(pouch, 1);
                ctx.give_items(full, 1);
                ctx.take_items(trophy, -1);
                ctx.set_cond(cond, true);
            } else {
                ctx.give_items(trophy, 1);
                ctx.play_sound(quest_sounds::ITEMGET);
            }
            return;
        }

        // Pouch 4: three each of four trophies; the twelfth kill converts.
        if let Some(&(_, trophy)) = FOURTH.iter().find(|(mob, _)| *mob == npc_id) {
            if self.has(ctx, LEATHER_POUCH_4TH) && ctx.quest_items_count(trophy) < 3 {
                if self.fourth_total(ctx) >= 11 {
                    self.fill_fourth_pouch(ctx);
                } else {
                    ctx.give_items(trophy, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            return;
        }

        // TODO(dead): the alternate ending's two kill handlers.
        match npc_id {
            KASHA_FANG_SPIDER | KASHA_BLADE_SPIDER
                if ctx.memo_state() == 3
                    && ctx.quest_items_count(KASHA_SPIDERS_TOOTH) < 6
                    && ctx.roll(100) < 70 =>
            {
                ctx.give_items(KASHA_SPIDERS_TOOTH, 1);
                if ctx.quest_items_count(KASHA_SPIDERS_TOOTH) >= 6 {
                    ctx.set_cond(16, true);
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            BAAR_DRE_VANUL
                if ctx.memo_state() == 4
                    && !self.has(ctx, HORN_OF_BAAR_DRE_VANUL)
                    && ctx.roll(100) < 90 =>
            {
                ctx.give_items(HORN_OF_BAAR_DRE_VANUL, 1);
                ctx.set_cond(18, true);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == GANTAKI_ZU_URUTU {
                return Some("30587-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            GANTAKI_ZU_URUTU => self.talk_gantaki(ctx),
            PREFECT_KASMAN => self.talk_kasman(ctx),
            KHAVATARI_ROSHEEK => self.talk_rosheek(ctx),
            KHAVATARI_TORUKU => self.talk_toruku(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00415PathOfTheOrcMonk {
    fn talk_gantaki(&self, ctx: &mut QuestCtx) -> Option<String> {
        // Any pouch, empty or full.
        let pouches = [
            LEATHER_POUCH_1ST,
            LEATHER_POUCH_2ND,
            LEATHER_POUCH_3RD,
            LEATHER_POUCH_1ST_FULL,
            LEATHER_POUCH_2ND_FULL,
            LEATHER_POUCH_3RD_FULL,
        ];
        let pouch_count: i64 = pouches.iter().map(|id| ctx.quest_items_count(*id)).sum();
        let memo = ctx.memo_state();
        // TODO(dead): only reachable via the unreachable `09c`.
        if memo == 2 {
            return Some("30587-09c.html".to_string());
        }
        let scroll = self.has(ctx, FIERY_SPIRIT_SCROLL);
        let letter = self.has(ctx, ROSHEEKS_LETTER);
        let rec = self.has(ctx, GANTAKIS_RECOMMENDATION);
        let pom = self.has(ctx, POMEGRANATE);
        if pom && !scroll && !rec && !letter && pouch_count == 0 {
            return Some("30587-07.html".to_string());
        }
        if !scroll && !pom && !rec && !letter && pouch_count == 1 {
            return Some("30587-08.html".to_string());
        }
        if scroll && letter && !pom && !rec && pouch_count == 0 {
            return Some("30587-09a.html".to_string());
        }
        if memo < 2 {
            if scroll && rec && !pom && !letter && pouch_count == 0 {
                return Some("30587-10.html".to_string());
            }
            if scroll && !pom && !rec && !letter && pouch_count == 0 {
                return Some("30587-11.html".to_string());
            }
        }
        Some(ctx.no_quest_html())
    }

    fn talk_kasman(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, GANTAKIS_RECOMMENDATION) {
            ctx.take_items(GANTAKIS_RECOMMENDATION, 1);
            ctx.give_items(FIG, 1);
            ctx.set_cond(10, false);
            return Some("30501-01.html".to_string());
        }
        let fig = self.has(ctx, FIG);
        let any_pouch4 = self.has_any(ctx, &[LEATHER_POUCH_4TH, LEATHER_POUCH_4TH_FULL]);
        if fig && !any_pouch4 {
            return Some("30501-02.html".to_string());
        }
        if !fig && any_pouch4 {
            return Some("30501-03.html".to_string());
        }
        if self.has(ctx, IRON_WILL_SCROLL) {
            ctx.give_items(KHAVATARI_TOTEM, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30501-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    /// Rosheek runs the three-pouch ladder: hand in a full pouch, get the next
    /// empty one.
    fn talk_rosheek(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, POMEGRANATE) {
            ctx.take_items(POMEGRANATE, 1);
            ctx.give_items(LEATHER_POUCH_1ST, 1);
            ctx.set_cond(2, false);
            return Some("30590-01.html".to_string());
        }
        // (carrying, full, next pouch, cond, busy page, handover page)
        let rungs: [Rung; 3] = [
            (
                LEATHER_POUCH_1ST,
                LEATHER_POUCH_1ST_FULL,
                Some(LEATHER_POUCH_2ND),
                4,
                "30590-02.html",
                "30590-03.html",
            ),
            (
                LEATHER_POUCH_2ND,
                LEATHER_POUCH_2ND_FULL,
                Some(LEATHER_POUCH_3RD),
                6,
                "30590-04.html",
                "30590-05.html",
            ),
            (
                LEATHER_POUCH_3RD,
                LEATHER_POUCH_3RD_FULL,
                None,
                8,
                "30590-06.html",
                "30590-07.html",
            ),
        ];
        for (pouch, full, next, cond, busy, done) in rungs {
            if self.has(ctx, pouch) && !self.has(ctx, full) {
                return Some(busy.to_string());
            }
            if !self.has(ctx, pouch) && self.has(ctx, full) {
                ctx.take_items(full, 1);
                match next {
                    Some(n) => ctx.give_items(n, 1),
                    None => {
                        // The last rung pays the scroll and his letter.
                        ctx.give_items(FIERY_SPIRIT_SCROLL, 1);
                        ctx.give_items(ROSHEEKS_LETTER, 1);
                    }
                }
                ctx.set_cond(cond, false);
                return Some(done.to_string());
            }
        }
        if self.has(ctx, ROSHEEKS_LETTER) && self.has(ctx, FIERY_SPIRIT_SCROLL) {
            return Some("30590-08.html".to_string());
        }
        if !self.has(ctx, ROSHEEKS_LETTER) && self.has(ctx, FIERY_SPIRIT_SCROLL) {
            return Some("30590-09.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_toruku(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, FIG) {
            ctx.take_items(FIG, 1);
            ctx.give_items(LEATHER_POUCH_4TH, 1);
            ctx.set_cond(11, false);
            return Some("30591-01.html".to_string());
        }
        if self.has(ctx, LEATHER_POUCH_4TH) && !self.has(ctx, LEATHER_POUCH_4TH_FULL) {
            return Some("30591-02.html".to_string());
        }
        if !self.has(ctx, LEATHER_POUCH_4TH) && self.has(ctx, LEATHER_POUCH_4TH_FULL) {
            ctx.take_items(LEATHER_POUCH_4TH_FULL, 1);
            ctx.give_items(IRON_WILL_SCROLL, 1);
            ctx.give_items(TORUKUS_LETTER, 1);
            ctx.set_cond(13, false);
            return Some("30591-03.html".to_string());
        }
        if self.has(ctx, IRON_WILL_SCROLL) && self.has(ctx, TORUKUS_LETTER) {
            return Some("30591-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
