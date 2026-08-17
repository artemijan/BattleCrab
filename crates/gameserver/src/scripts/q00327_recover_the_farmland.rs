//! Recover the Farmland (327) — `quests/Q00327_RecoverTheFarmland`. Leikan
//! (30382) and Piotur (30597) send level-25–34 hunters against the Turek Orc
//! camp threatening the Gludio farmland. Kills drop **Dog Tags** / **Medallions**
//! (cashed in with Piotur) and, by chance, one of four **relic fragments**. The
//! fragments feed three side-vendors: Asha (30313) gambles five fragments into
//! an **ancient relic**, Iris (30034) buys fragments/relics for XP, and Nestle
//! (30314) trades relics for random consumables (soul/spiritshots, potions,
//! scrolls). Repeatable.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const IRIS: i32 = 30034;
const ASHA: i32 = 30313;
const NESTLE: i32 = 30314;
const LEIKAN: i32 = 30382;
const PIOTUR: i32 = 30597;
// Monsters
const MOBS: [i32; 7] = [20495, 20496, 20497, 20498, 20499, 20500, 20501];
const TUREK_ORK_WARLORD: i32 = 20495;
const TUREK_ORK_SHAMAN: i32 = 20501;
// Kill tokens
const TUREK_DOG_TAG: i32 = 1846;
const TUREK_MEDALLION: i32 = 1847;
const LEIKANS_LETTER: i32 = 5012;
// Relic fragments (1848..=1851, the four collectibles) and their relics.
const CLAY_URN_FRAGMENT: i32 = 1848;
const JADE_NECKLACE_BEAD: i32 = 1851;
const ANCIENT_CLAY_URN: i32 = 1852;
const ANCIENT_BRASS_TIARA: i32 = 1853;
const ANCIENT_BRONZE_MIRROR: i32 = 1854;
const ANCIENT_JADE_NECKLACE: i32 = 1855;
// Nestle's reward pool
const QUICK_STEP_POTION: i32 = 734;
const SWIFT_ATTACK_POTION: i32 = 735;
const SCROLL_OF_ESCAPE: i32 = 736;
const SCROLL_OF_RESURRECTION: i32 = 737;
const HEALING_POTION: i32 = 1061;
const SOULSHOT_D: i32 = 1463;
const SPIRITSHOT_D: i32 = 2510;
// Misc
const MIN_LEVEL: i32 = 25;
const MAX_LEVEL: i32 = 34;

/// Per-mob chance (percent) to also drop a random relic fragment.
fn fragment_drop_prob(npc_id: i32) -> i32 {
    match npc_id {
        20496 => 21, // Archer
        20499 => 19, // Footman
        20500 => 18, // Sentinel
        20501 => 22, // Shaman
        20497 => 21, // Skirmisher
        20498 => 20, // Supplier
        20495 => 26, // Warlord
        _ => 0,
    }
}

/// Iris's XP buy-back: which fragment/relic each page cashes, and the XP each.
/// `(event, item, xp_each)`.
const FRAGMENT_XP: [(&str, i32, i64); 4] = [
    ("30034-03.html", CLAY_URN_FRAGMENT, 307),
    ("30034-04.html", 1849, 368), // Brass Trinket Piece
    ("30034-05.html", 1850, 368), // Bronze Mirror Piece
    ("30034-06.html", JADE_NECKLACE_BEAD, 430),
];
/// Iris's "full set" buy-back (relics): `(relic, xp_each)`.
const RELIC_XP: [(i32, i64); 4] = [
    (ANCIENT_CLAY_URN, 2766),
    (ANCIENT_BRASS_TIARA, 3227),
    (ANCIENT_BRONZE_MIRROR, 3227),
    (ANCIENT_JADE_NECKLACE, 3919),
];
/// Asha's gamble: `(event, fragment, relic, low_html, success_out_of)` — five
/// fragments have an `(n-1)/n` chance to become the relic.
const ASHA_GAMBLE: [(&str, i32, i32, &str, i32); 4] = [
    (
        "30313-03.html",
        CLAY_URN_FRAGMENT,
        ANCIENT_CLAY_URN,
        "30313-02.html",
        6,
    ),
    (
        "30313-05.html",
        1849,
        ANCIENT_BRASS_TIARA,
        "30313-04.html",
        7,
    ),
    (
        "30313-07.html",
        1850,
        ANCIENT_BRONZE_MIRROR,
        "30313-06.html",
        7,
    ),
    (
        "30313-09.html",
        JADE_NECKLACE_BEAD,
        ANCIENT_JADE_NECKLACE,
        "30313-08.html",
        8,
    ),
];

pub struct Q00327RecoverTheFarmland;

impl Q00327RecoverTheFarmland {
    fn has(&self, ctx: &QuestCtx, item_id: i32) -> bool {
        ctx.quest_items_count(item_id) > 0
    }
}

impl QuestScript for Q00327RecoverTheFarmland {
    fn id(&self) -> i32 {
        327
    }
    fn name(&self) -> &'static str {
        "Q00327_RecoverTheFarmland"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00327_RecoverTheFarmland"
    }
    fn start_npcs(&self) -> &[i32] {
        &[LEIKAN, PIOTUR]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[LEIKAN, PIOTUR, IRIS, ASHA, NESTLE]
    }
    fn kill_npcs(&self) -> &[i32] {
        &MOBS
    }
    fn quest_items(&self) -> &[i32] {
        &[TUREK_DOG_TAG, TUREK_MEDALLION, LEIKANS_LETTER]
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        // `addCondMaxLevel(34, …)`.
        (ctx.player_level() > MAX_LEVEL).then(|| ctx.no_quest_html())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let created = ctx.is_created();
        let started = ctx.is_started();
        match ctx.npc_id {
            LEIKAN => {
                if created {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "30382-02.htm"
                        } else {
                            "30382-01.htm"
                        }
                        .to_string(),
                    );
                }
                if started {
                    if self.has(ctx, LEIKANS_LETTER) {
                        return Some("30382-04.html".to_string());
                    }
                    ctx.set_cond(5, true);
                    return Some("30382-05.html".to_string());
                }
            }
            PIOTUR => {
                if created {
                    return Some(
                        if ctx.player_level() >= MIN_LEVEL {
                            "30597-02.htm"
                        } else {
                            "30597-01.htm"
                        }
                        .to_string(),
                    );
                }
                if started {
                    if self.has(ctx, LEIKANS_LETTER) {
                        ctx.take_items(LEIKANS_LETTER, -1);
                        ctx.set_cond(3, true);
                        return Some("30597-03a.htm".to_string());
                    }
                    if !self.has(ctx, TUREK_DOG_TAG) && !self.has(ctx, TUREK_MEDALLION) {
                        return Some("30597-04.html".to_string());
                    }
                    let dog_tags = ctx.quest_items_count(TUREK_DOG_TAG);
                    let medallions = ctx.quest_items_count(TUREK_MEDALLION);
                    let bonus = if dog_tags + medallions >= 10 { 1000 } else { 0 };
                    ctx.give_adena((dog_tags + medallions) * 8 + bonus, true);
                    ctx.take_items(TUREK_DOG_TAG, -1);
                    ctx.take_items(TUREK_MEDALLION, -1);
                    ctx.set_cond(4, true);
                    return Some("30597-05.html".to_string());
                }
            }
            IRIS if started => return Some("30034-01.html".to_string()),
            ASHA if started => return Some("30313-01.html".to_string()),
            NESTLE if started => return Some("30314-01.html".to_string()),
            _ => {}
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            // Informational pages, echoed straight back.
            "30034-01.html" | "30313-01.html" | "30314-02.html" | "30314-08.html"
            | "30314-09.html" | "30382-05a.html" | "30382-05b.html" | "30597-03.html"
            | "30597-07.html" => Some(event.to_string()),
            "30382-03.htm" => {
                ctx.start_quest();
                ctx.give_items(LEIKANS_LETTER, 1);
                ctx.set_cond(2, false);
                Some(event.to_string())
            }
            "30597-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30597-06.html" => {
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30034-07.html" => {
                // Iris's relic buy-back: cash every relic held, for XP.
                let mut rewarded = false;
                for (relic, xp) in RELIC_XP {
                    let count = ctx.quest_items_count(relic);
                    if count > 0 {
                        ctx.add_exp_and_sp(count * xp, 0);
                        ctx.take_items(relic, -1);
                        ctx.play_sound(quest_sounds::ITEMGET);
                        rewarded = true;
                    }
                }
                Some(if rewarded { event } else { "30034-02.html" }.to_string())
            }
            _ => self
                .iris_fragment_xp(ctx, event)
                .or_else(|| self.asha_gamble(ctx, event))
                .or_else(|| self.nestle_trade(ctx, event)),
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        // Shamans and Warlords carry medallions; everyone else a dog tag.
        if npc_id == TUREK_ORK_SHAMAN || npc_id == TUREK_ORK_WARLORD {
            ctx.give_items(TUREK_MEDALLION, 1);
        } else {
            ctx.give_items(TUREK_DOG_TAG, 1);
        }
        // A chance to also drop one of the four relic fragments.
        if ctx.roll(100) < fragment_drop_prob(npc_id) {
            let fragment = CLAY_URN_FRAGMENT + ctx.roll(4); // getRandom(1848, 1851)
            ctx.give_items(fragment, 1);
        }
    }
}

impl Q00327RecoverTheFarmland {
    /// Iris (30034) buys single fragment/relic stacks for XP.
    fn iris_fragment_xp(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let (_, item, xp) = FRAGMENT_XP.iter().find(|(e, _, _)| *e == event)?;
        if !self.has(ctx, *item) {
            return Some("30034-02.html".to_string());
        }
        let count = ctx.quest_items_count(*item);
        ctx.add_exp_and_sp(count * xp, 0);
        ctx.take_items(*item, -1);
        ctx.play_sound(quest_sounds::ITEMGET);
        Some(event.to_string())
    }

    /// Asha (30313) gambles five fragments into a relic.
    fn asha_gamble(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let &(_, fragment, relic, low_html, success_out_of) =
            ASHA_GAMBLE.iter().find(|(e, ..)| *e == event)?;
        if ctx.quest_items_count(fragment) < 5 {
            return Some(low_html.to_string());
        }
        ctx.take_items(fragment, 5);
        // `getRandom(n) < n-1` — an (n-1)/n success.
        if ctx.roll(success_out_of) < success_out_of - 1 {
            ctx.give_items(relic, 1);
            Some(event.to_string())
        } else {
            Some("30313-10.html".to_string())
        }
    }

    /// Nestle (30314) trades a relic for random consumables.
    fn nestle_trade(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        let relic = match event {
            "30314-03.html" => ANCIENT_CLAY_URN,
            "30314-04.html" => ANCIENT_BRASS_TIARA,
            "30314-05.html" => ANCIENT_BRONZE_MIRROR,
            "30314-06.html" => ANCIENT_JADE_NECKLACE,
            _ => return None,
        };
        if !self.has(ctx, relic) {
            return Some("30314-07.html".to_string());
        }
        match event {
            "30314-03.html" => {
                let count = (ctx.roll(41) + 70) as i64;
                ctx.reward_items(SOULSHOT_D, count);
            }
            "30314-04.html" => {
                let rnd = ctx.roll(100);
                let item = if rnd < 40 {
                    HEALING_POTION
                } else if rnd < 84 {
                    QUICK_STEP_POTION
                } else {
                    SWIFT_ATTACK_POTION
                };
                ctx.reward_items(item, 1);
            }
            "30314-05.html" => {
                let item = if ctx.roll(100) < 59 {
                    SCROLL_OF_ESCAPE
                } else {
                    SCROLL_OF_RESURRECTION
                };
                ctx.reward_items(item, 1);
            }
            "30314-06.html" => {
                let count = (ctx.roll(41) + 50) as i64;
                ctx.reward_items(SPIRITSHOT_D, count);
            }
            _ => unreachable!(),
        }
        ctx.take_items(relic, 1);
        Some(event.to_string())
    }
}
