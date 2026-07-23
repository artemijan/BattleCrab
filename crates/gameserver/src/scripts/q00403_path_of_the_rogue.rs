//! Path Of The Rogue (403) — port of
//! `dist/game/data/scripts/quests/Q00403_PathOfTheRogue/`.
//!
//! Awards **Beziques' Recommendation** (1190), the proof
//! `ElfHumanFighterChange1` takes to turn a Human Fighter into a Rogue.
//!
//! Bezique → Neti (who lends you a bow and a dagger) → 10 Spartoi bones →
//! a horseshoe → the most-wanted list → hunt the Cat's Eye Bandit until all
//! four stolen goods are recovered.
//!
//! Shares quest 401's "kill it solo with the quest weapon" gate
//! ([`quest_common::tag_attacker_with_weapon`]) — here either of Neti's two
//! loaned weapons qualifies.
//!
//! **The chance denominator is not what the shared type suggests.** The drop
//! table is `ItemChanceHolder`, the same type quest 406 uses with
//! `getRandom(100) < chance`, but this quest rolls
//! `getRandom(REQUIRED_ITEM_COUNT)` — i.e. **`getRandom(10)`**. So a "chance"
//! of 2 means 20% here and 2% there. Reading the holder as a percentage would
//! have made every bone drop ~10× too rare. The denominator is per-call, not a
//! property of the table.
//!
//! The Cat's Eye Bandit's two lines also differ in audience: the taunt on the
//! first hit is sent to the **attacker only** (`sendPacket`), the death line
//! is **broadcast**.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;
use crate::scripts::quest_common;

const CAPTAIN_BEZIQUE: i32 = 30379;
const NETI: i32 = 30425;

const BEZIQUES_LETTER: i32 = 1180;
const NETIS_BOW: i32 = 1181;
const NETIS_DAGGER: i32 = 1182;
const SPARTOIS_BONES: i32 = 1183;
const HORSESHOE_OF_LIGHT: i32 = 1184;
const MOST_WANTED_LIST: i32 = 1185;
const STOLEN_JEWELRY: i32 = 1186;
const STOLEN_TOMES: i32 = 1187;
const STOLEN_RING: i32 = 1188;
const STOLEN_NECKLACE: i32 = 1189;
const BEZIQUES_RECOMMENDATION: i32 = 1190;

const STOLEN_ITEMS: [i32; 4] = [STOLEN_JEWELRY, STOLEN_TOMES, STOLEN_RING, STOLEN_NECKLACE];

const CATS_EYE_BANDIT: i32 = 27038;
/// `(npc id, chance out of TEN)` — see the module note on the denominator.
const MONSTER_DROPS: [(i32, i32); 6] = [
    (20035, 2),
    (20042, 3),
    (20045, 2),
    (20051, 2),
    (20054, 8),
    (20060, 8),
];

/// "You childish fool, do you think you can catch me?"
const NS_TAUNT: i32 = 40306;
/// "I must do something about this shameful incident..."
const NS_DEFEATED: i32 = 40307;

const FIGHTER: i32 = 0;
const ROGUE: i32 = 7;
const MIN_LEVEL: i32 = 19;
const REQUIRED_ITEM_COUNT: i64 = 10;

const QUEST_ITEMS: [i32; 10] = [
    BEZIQUES_LETTER,
    NETIS_BOW,
    NETIS_DAGGER,
    SPARTOIS_BONES,
    HORSESHOE_OF_LIGHT,
    MOST_WANTED_LIST,
    STOLEN_JEWELRY,
    STOLEN_TOMES,
    STOLEN_RING,
    STOLEN_NECKLACE,
];

const KILL_NPCS: [i32; 7] = [20035, 20042, 20045, 20051, 20054, 20060, CATS_EYE_BANDIT];

pub struct Q00403PathOfTheRogue;

impl QuestScript for Q00403PathOfTheRogue {
    fn id(&self) -> i32 {
        403
    }
    fn name(&self) -> &'static str {
        "Q00403_PathOfTheRogue"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00403_PathOfTheRogue"
    }
    fn start_npcs(&self) -> &[i32] {
        &[CAPTAIN_BEZIQUE]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[CAPTAIN_BEZIQUE, NETI]
    }
    fn kill_npcs(&self) -> &[i32] {
        &KILL_NPCS
    }
    fn attack_npcs(&self) -> &[i32] {
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
                    FIGHTER if ctx.player_level() < MIN_LEVEL => "30379-03.htm",
                    FIGHTER if ctx.quest_items_count(BEZIQUES_RECOMMENDATION) > 0 => "30379-04.htm",
                    FIGHTER => "30379-05.htm",
                    ROGUE => "30379-02a.htm",
                    _ => "30379-02.htm",
                }
                .to_string(),
            ),
            "30379-06.htm" => {
                ctx.start_quest();
                ctx.give_items(BEZIQUES_LETTER, 1);
                Some(event.to_string())
            }
            "30425-02.html" | "30425-03.html" | "30425-04.html" => Some(event.to_string()),
            "30425-05.html" => {
                // Neti loans both weapons. Java echoes the page either way,
                // only the hand-over is conditional.
                if ctx.quest_items_count(BEZIQUES_LETTER) > 0 {
                    ctx.take_items(BEZIQUES_LETTER, 1);
                    if ctx.quest_items_count(NETIS_BOW) == 0 {
                        ctx.give_items(NETIS_BOW, 1);
                    }
                    if ctx.quest_items_count(NETIS_DAGGER) == 0 {
                        ctx.give_items(NETIS_DAGGER, 1);
                    }
                    ctx.set_cond(2, true);
                }
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        let first_qualifying =
            quest_common::tag_attacker_with_weapon(ctx, &[NETIS_BOW, NETIS_DAGGER]);
        if first_qualifying && ctx.npc_id == CATS_EYE_BANDIT {
            // Attacker only — not a broadcast.
            ctx.npc_say_to_player(NS_TAUNT);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() || !quest_common::is_tag_qualified(ctx) {
            return;
        }
        if ctx.npc_id == CATS_EYE_BANDIT {
            ctx.npc_say(NS_DEFEATED);
            if ctx.quest_items_count(MOST_WANTED_LIST) == 0 {
                return;
            }
            // One of the four at random — a duplicate simply pays nothing,
            // so the last pieces take progressively longer.
            let pick = STOLEN_ITEMS[ctx.roll(STOLEN_ITEMS.len() as i32) as usize];
            if ctx.quest_items_count(pick) > 0 {
                return;
            }
            ctx.give_items(pick, 1);
            if STOLEN_ITEMS.iter().all(|id| ctx.quest_items_count(*id) > 0) {
                ctx.set_cond(6, true);
            } else {
                ctx.play_sound(quest_sounds::ITEMGET);
            }
            return;
        }
        let Some((_, chance)) = MONSTER_DROPS.iter().find(|(id, _)| *id == ctx.npc_id) else {
            return;
        };
        // `getRandom(REQUIRED_ITEM_COUNT)` — out of 10, not 100.
        if ctx.quest_items_count(SPARTOIS_BONES) < REQUIRED_ITEM_COUNT
            && ctx.roll(REQUIRED_ITEM_COUNT as i32) < *chance
        {
            ctx.give_items(SPARTOIS_BONES, 1);
            if ctx.quest_items_count(SPARTOIS_BONES) >= REQUIRED_ITEM_COUNT {
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
            if npc == CAPTAIN_BEZIQUE {
                return Some("30379-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            CAPTAIN_BEZIQUE => self.talk_bezique(ctx),
            NETI => self.talk_neti(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00403PathOfTheRogue {
    fn talk_bezique(&self, ctx: &mut QuestCtx) -> Option<String> {
        if STOLEN_ITEMS.iter().all(|id| ctx.quest_items_count(*id) > 0) {
            ctx.take_items(NETIS_BOW, 1);
            ctx.take_items(NETIS_DAGGER, 1);
            ctx.take_items(MOST_WANTED_LIST, 1);
            for id in STOLEN_ITEMS {
                ctx.take_items(id, 1);
            }
            ctx.give_items(BEZIQUES_RECOMMENDATION, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30379-09.html".to_string());
        }
        if ctx.quest_items_count(HORSESHOE_OF_LIGHT) == 0
            && ctx.quest_items_count(BEZIQUES_LETTER) > 0
        {
            return Some("30379-07.html".to_string());
        }
        if ctx.quest_items_count(HORSESHOE_OF_LIGHT) > 0 {
            ctx.take_items(HORSESHOE_OF_LIGHT, 1);
            ctx.give_items(MOST_WANTED_LIST, 1);
            ctx.set_cond(5, true);
            return Some("30379-08.html".to_string());
        }
        if ctx.quest_items_count(NETIS_BOW) > 0
            && ctx.quest_items_count(NETIS_DAGGER) > 0
            && ctx.quest_items_count(MOST_WANTED_LIST) == 0
        {
            return Some("30379-10.html".to_string());
        }
        if ctx.quest_items_count(MOST_WANTED_LIST) > 0 {
            return Some("30379-11.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_neti(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(BEZIQUES_LETTER) > 0 {
            return Some("30425-01.html".to_string());
        }
        if ctx.quest_items_count(HORSESHOE_OF_LIGHT) > 0 {
            return Some("30425-08.html".to_string());
        }
        // Neither the horseshoe nor Bezique's letter in hand.
        if ctx.quest_items_count(MOST_WANTED_LIST) > 0 {
            return Some("30425-08.html".to_string());
        }
        if ctx.quest_items_count(SPARTOIS_BONES) < REQUIRED_ITEM_COUNT {
            return Some("30425-06.html".to_string());
        }
        ctx.take_items(SPARTOIS_BONES, REQUIRED_ITEM_COUNT);
        ctx.give_items(HORSESHOE_OF_LIGHT, 1);
        ctx.set_cond(4, true);
        Some("30425-07.html".to_string())
    }
}
