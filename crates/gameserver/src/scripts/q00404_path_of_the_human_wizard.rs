//! Path Of The Human Wizard (404) — port of
//! `dist/game/data/scripts/quests/Q00404_PathOfTheHumanWizard/`.
//!
//! Awards the **Bead of Season** (1292), the first of the four proofs
//! `ElfHumanWizardChange1` consumes.
//!
//! A strictly linear elemental chain — Fire → Wind → Water → Earth — where
//! each spirit runs the identical three-step bargain: hand you a token, wait
//! while you collect something, then trade token + collection for its trinket.
//! Parina takes all four trinkets for the bead. Because the shape repeats
//! exactly, including the page numbering (`{npc}-01..04.html` for all four),
//! the branches are a table.
//!
//! **The chance denominator here is `/100`** (`getRandom(100) < 20|80`), unlike
//! quests 401/403 which roll `getRandom(10)`. Same family, same author, three
//! quests apart — checked at each call site rather than carried over.
//!
//! One branch breaks the pattern: **Wind's collectable is not a drop.** The
//! feather comes from talking to the Wasteland Lizardman (bypass
//! `30410-03.html`), who has his own pages outside the four-page scheme. Every
//! other branch collects by killing.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const PARINA: i32 = 30391;
const EARTH_SNAKE: i32 = 30409;
const WASTELAND_LIZARDMAN: i32 = 30410;
const FLAME_SALAMANDER: i32 = 30411;
const WIND_SYLPH: i32 = 30412;
const WATER_UNDINE: i32 = 30413;

const MAP_OF_LUSTER: i32 = 1280;
const KEY_OF_FLAME: i32 = 1281;
const FLAME_EARING: i32 = 1282;
const BROKEN_BRONZE_MIRROR: i32 = 1283;
const WIND_FEATHER: i32 = 1284;
const WIND_BANGLE: i32 = 1285;
const RAMAS_DIARY: i32 = 1286;
const SPARKLE_PEBBLE: i32 = 1287;
const WATER_NECKLACE: i32 = 1288;
const RUSTY_COIN: i32 = 1289;
const RED_SOIL: i32 = 1290;
const EARTH_RING: i32 = 1291;
const BEAD_OF_SEASON: i32 = 1292;

const RED_BEAR: i32 = 20021;
const RATMAN_WARRIOR: i32 = 20359;
const WATER_SEER: i32 = 27030;

const MAGE: i32 = 10;
const WIZARD: i32 = 11;
const MIN_LEVEL: i32 = 19;

const TRINKETS: [i32; 4] = [FLAME_EARING, WIND_BANGLE, WATER_NECKLACE, EARTH_RING];

/// One elemental spirit. `entry` is the previous element's trinket (0 for
/// Fire, which opens the chain). Pages are always `{npc}-01..04.html`.
struct Element {
    npc: i32,
    entry: i32,
    token: i32,
    collect: i32,
    need: i64,
    trinket: i32,
    cond_token: i32,
    cond_trade: i32,
}

const ELEMENTS: [Element; 4] = [
    Element {
        npc: FLAME_SALAMANDER,
        entry: 0, // opens the chain
        token: MAP_OF_LUSTER,
        collect: KEY_OF_FLAME,
        need: 1,
        trinket: FLAME_EARING,
        cond_token: 2,
        cond_trade: 4,
    },
    Element {
        npc: WIND_SYLPH,
        entry: FLAME_EARING,
        token: BROKEN_BRONZE_MIRROR,
        collect: WIND_FEATHER,
        need: 1,
        trinket: WIND_BANGLE,
        cond_token: 5,
        cond_trade: 7,
    },
    Element {
        npc: WATER_UNDINE,
        entry: WIND_BANGLE,
        token: RAMAS_DIARY,
        collect: SPARKLE_PEBBLE,
        need: 2, // the only branch wanting two
        trinket: WATER_NECKLACE,
        cond_token: 8,
        cond_trade: 10,
    },
    Element {
        npc: EARTH_SNAKE,
        entry: WATER_NECKLACE,
        token: RUSTY_COIN,
        collect: RED_SOIL,
        need: 1,
        trinket: EARTH_RING,
        cond_token: 11,
        cond_trade: 13,
    },
];

/// `(mob, gating token, collectable, needed, percent chance, cond when full)`.
type Drop = (i32, i32, i32, i64, i32, i32);
const DROPS: [Drop; 3] = [
    (RATMAN_WARRIOR, MAP_OF_LUSTER, KEY_OF_FLAME, 1, 80, 3),
    (WATER_SEER, RAMAS_DIARY, SPARKLE_PEBBLE, 2, 80, 9),
    (RED_BEAR, RUSTY_COIN, RED_SOIL, 1, 20, 12),
];

const QUEST_ITEMS: [i32; 12] = [
    MAP_OF_LUSTER, KEY_OF_FLAME, FLAME_EARING, BROKEN_BRONZE_MIRROR, WIND_FEATHER, WIND_BANGLE,
    RAMAS_DIARY, SPARKLE_PEBBLE, WATER_NECKLACE, RUSTY_COIN, RED_SOIL, EARTH_RING,
];

pub struct Q00404PathOfTheHumanWizard;

impl QuestScript for Q00404PathOfTheHumanWizard {
    fn id(&self) -> i32 {
        404
    }
    fn name(&self) -> &'static str {
        "Q00404_PathOfTheHumanWizard"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00404_PathOfTheHumanWizard"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PARINA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[PARINA, EARTH_SNAKE, WASTELAND_LIZARDMAN, FLAME_SALAMANDER, WIND_SYLPH, WATER_UNDINE]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[RED_BEAR, RATMAN_WARRIOR, WATER_SEER]
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
                    MAGE if ctx.player_level() < MIN_LEVEL => "30391-02.htm".to_string(),
                    MAGE if ctx.quest_items_count(BEAD_OF_SEASON) > 0 => "30391-03.htm".to_string(),
                    MAGE => {
                        // `ACCEPT` starts the quest directly here — there is no
                        // separate confirmation page as in 401/403.
                        ctx.start_quest();
                        "30391-07.htm".to_string()
                    }
                    WIZARD => "30391-02a.htm".to_string(),
                    _ => "30391-01.htm".to_string(),
                },
            ),
            "30410-02.html" => Some(event.to_string()),
            // Wind's collectable is handed over in dialog, not dropped.
            "30410-03.html" => {
                ctx.give_items(WIND_FEATHER, 1);
                ctx.set_cond(6, true);
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
        let Some((_, token, collect, need, chance, cond)) =
            DROPS.iter().find(|(mob, ..)| *mob == npc_id)
        else {
            return;
        };
        if ctx.quest_items_count(*token) == 0 || ctx.quest_items_count(*collect) >= *need {
            return;
        }
        // `getRandom(100) < chance` — percent here, unlike quests 401/403.
        if ctx.roll(100) >= *chance {
            return;
        }
        ctx.give_items(*collect, 1);
        if ctx.quest_items_count(*collect) == *need {
            ctx.set_cond(*cond, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == PARINA {
                return Some("30391-04.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        if npc == PARINA {
            if !TRINKETS.iter().all(|id| ctx.quest_items_count(*id) > 0) {
                return Some("30391-05.html".to_string());
            }
            for id in TRINKETS {
                ctx.take_items(id, 1);
            }
            if ctx.quest_items_count(BEAD_OF_SEASON) == 0 {
                ctx.give_items(BEAD_OF_SEASON, 1);
            }
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30391-06.html".to_string());
        }
        if npc == WASTELAND_LIZARDMAN {
            if ctx.quest_items_count(BROKEN_BRONZE_MIRROR) == 0 {
                return Some(ctx.no_quest_html());
            }
            return Some(
                if ctx.quest_items_count(WIND_FEATHER) == 0 { "30410-01.html" } else { "30410-04.html" }
                    .to_string(),
            );
        }
        let Some(e) = ELEMENTS.iter().find(|e| e.npc == npc) else {
            return Some(ctx.no_quest_html());
        };
        let has_token = ctx.quest_items_count(e.token) > 0;
        let has_trinket = ctx.quest_items_count(e.trinket) > 0;
        // Entry: the previous element's trinket in hand (Fire needs nothing),
        // and this branch not yet started or finished.
        let entry_ok = e.entry == 0 || ctx.quest_items_count(e.entry) > 0;
        if entry_ok && !has_token && !has_trinket {
            ctx.give_items(e.token, 1);
            ctx.set_cond(e.cond_token, true);
            return Some(format!("{npc}-01.html"));
        }
        if has_token {
            if ctx.quest_items_count(e.collect) < e.need {
                return Some(format!("{npc}-02.html"));
            }
            ctx.take_items(e.token, 1);
            ctx.take_items(e.collect, -1);
            if ctx.quest_items_count(e.trinket) == 0 {
                ctx.give_items(e.trinket, 1);
            }
            ctx.set_cond(e.cond_trade, true);
            return Some(format!("{npc}-03.html"));
        }
        if has_trinket {
            return Some(format!("{npc}-04.html"));
        }
        Some(ctx.no_quest_html())
    }
}
