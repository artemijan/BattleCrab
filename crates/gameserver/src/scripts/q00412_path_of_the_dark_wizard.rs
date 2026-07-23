//! Path Of The Dark Wizard (412) — port of
//! `dist/game/data/scripts/quests/Q00412_PathOfTheDarkWizard/`.
//!
//! Awards the **Jewel of Darkness** (1261), `DarkElfChange1`'s third proof.
//!
//! Varika plants the Seed of Despair, then three specialists each grow one
//! more seed: hand you a tool, wait while the tool gates a drop, trade the full
//! set for their seed. Four seeds buy the jewel. Structurally the twin of quest
//! 408 (Elven Wizard) — three parallel errands sharing one shape — so it ports
//! as the same kind of table.
//!
//! | Seed | Specialist | Tool | Mob | Material | Need |
//! |---|---|---|---|---|---|
//! | Anger | Charkeren | Lucky Key | Marsh Zombie | Family's Remains | 3 |
//! | Horror | Annika | Candle | Misery Skeleton / Hunter / Archer | Knee Bone | 2 |
//! | Lunacy | Arkenia | Hub Scent | Skeleton Scout | Heart of Lunacy | 3 |
//!
//! ## The same third-errand asymmetry as 408 — twice is a convention
//!
//! Charkeren and Annika hand over their tool through a **dialog event**
//! (`30415-03.html`, `30418-02.html`). Arkenia hands over the Hub Scent
//! **inline in `onTalk`**, with no event — exactly the shape quest 408 has,
//! where Greenis and Thalia use events and Northwind doesn't. Two independent
//! quests doing this makes it a datapack convention rather than a one-off
//! oversight, so it is modelled (`tool_event: Option<&str>`) rather than
//! normalised.
//!
//! Arkenia's branch also lacks the `hasQuestItems(SEEDS_OF_DESPAIR)` guard its
//! two siblings carry. Kept: her errand is reachable slightly earlier, and
//! adding the guard would change who can start it.
//!
//! ## The chance is a coin flip written as `== 0`
//!
//! All three drops roll `getRandom(2) == 0` — **equality, not the `<`
//! threshold** every other Path quest uses. Same 50% either way here, but the
//! form matters: reading it as `getRandom(2) < 2` would make every kill pay.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const CHARKEREN: i32 = 30415;
const ANNIKA: i32 = 30418;
const ARKENIA: i32 = 30419;
const VARIKA: i32 = 30421;

const SEEDS_OF_ANGER: i32 = 1253;
const SEEDS_OF_DESPAIR: i32 = 1254;
const SEEDS_OF_HORROR: i32 = 1255;
const SEEDS_OF_LUNACY: i32 = 1256;
const FAMILYS_REMAINS: i32 = 1257;
const KNEE_BONE: i32 = 1259;
const HEART_OF_LUNACY: i32 = 1260;
const JEWEL_OF_DARKNESS: i32 = 1261;
const LUCKY_KEY: i32 = 1277;
const CANDLE: i32 = 1278;
const HUB_SCENT: i32 = 1279;

const MARSH_ZOMBIE: i32 = 20015;
const MISERY_SKELETON: i32 = 20022;
const SKELETON_SCOUT: i32 = 20045;
const SKELETON_HUNTER: i32 = 20517;
const SKELETON_HUNTER_ARCHER: i32 = 20518;

const DARK_MAGE: i32 = 38;
const DARK_WIZARD: i32 = 39;
const MIN_LEVEL: i32 = 19;

struct Errand {
    npc: i32,
    seed: i32,
    tool: i32,
    material: i32,
    need: i64,
    mobs: &'static [i32],
    /// `None` for Arkenia, who hands the tool over in `onTalk`.
    tool_event: Option<&'static str>,
    /// offer / collecting / trade pages.
    offer: &'static str,
    collecting: &'static str,
    trade: &'static str,
}

const ERRANDS: [Errand; 3] = [
    Errand {
        npc: CHARKEREN,
        seed: SEEDS_OF_ANGER,
        tool: LUCKY_KEY,
        material: FAMILYS_REMAINS,
        need: 3,
        mobs: &[MARSH_ZOMBIE],
        tool_event: Some("30415-03.html"),
        offer: "30415-01.html",
        collecting: "30415-04.html",
        trade: "30415-05.html",
    },
    Errand {
        npc: ANNIKA,
        seed: SEEDS_OF_HORROR,
        tool: CANDLE,
        material: KNEE_BONE,
        need: 2,
        mobs: &[MISERY_SKELETON, SKELETON_HUNTER, SKELETON_HUNTER_ARCHER],
        tool_event: Some("30418-02.html"),
        offer: "30418-01.html",
        collecting: "30418-03.html",
        trade: "30418-04.html",
    },
    Errand {
        npc: ARKENIA,
        seed: SEEDS_OF_LUNACY,
        tool: HUB_SCENT,
        material: HEART_OF_LUNACY,
        need: 3,
        mobs: &[SKELETON_SCOUT],
        tool_event: None,
        offer: "30419-01.html",
        collecting: "30419-02.html",
        trade: "30419-03.html",
    },
];

const SEEDS: [i32; 4] = [
    SEEDS_OF_DESPAIR,
    SEEDS_OF_ANGER,
    SEEDS_OF_HORROR,
    SEEDS_OF_LUNACY,
];

const QUEST_ITEMS: [i32; 10] = [
    SEEDS_OF_ANGER,
    SEEDS_OF_DESPAIR,
    SEEDS_OF_HORROR,
    SEEDS_OF_LUNACY,
    FAMILYS_REMAINS,
    KNEE_BONE,
    HEART_OF_LUNACY,
    LUCKY_KEY,
    CANDLE,
    HUB_SCENT,
];

const KILL_NPCS: [i32; 5] = [
    MARSH_ZOMBIE,
    MISERY_SKELETON,
    SKELETON_SCOUT,
    SKELETON_HUNTER,
    SKELETON_HUNTER_ARCHER,
];

pub struct Q00412PathOfTheDarkWizard;

impl Q00412PathOfTheDarkWizard {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
}

impl QuestScript for Q00412PathOfTheDarkWizard {
    fn id(&self) -> i32 {
        412
    }
    fn name(&self) -> &'static str {
        "Q00412_PathOfTheDarkWizard"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00412_PathOfTheDarkWizard"
    }
    fn start_npcs(&self) -> &[i32] {
        &[VARIKA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[VARIKA, CHARKEREN, ANNIKA, ARKENIA]
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
        // The two specialists that hand their tool over via dialog.
        if let Some(e) = ERRANDS.iter().find(|e| e.tool_event == Some(event)) {
            ctx.give_items(e.tool, 1);
            return Some(event.to_string());
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                DARK_MAGE if ctx.player_level() < MIN_LEVEL => "30421-02.htm".to_string(),
                DARK_MAGE if self.has(ctx, JEWEL_OF_DARKNESS) => "30421-04.htm".to_string(),
                DARK_MAGE => {
                    ctx.start_quest();
                    ctx.give_items(SEEDS_OF_DESPAIR, 1);
                    "30421-05.htm".to_string()
                }
                DARK_WIZARD => "30421-02a.htm".to_string(),
                _ => "30421-03.htm".to_string(),
            }),
            // Varika's three "how is X going?" buttons, each answering
            // differently depending on whether that seed is already grown.
            "30421-06.html" => Some(
                if self.has(ctx, SEEDS_OF_ANGER) {
                    event
                } else {
                    "30421-07.html"
                }
                .to_string(),
            ),
            "30421-09.html" => Some(
                if self.has(ctx, SEEDS_OF_HORROR) {
                    event
                } else {
                    "30421-10.html"
                }
                .to_string(),
            ),
            "30421-11.html" => {
                if self.has(ctx, SEEDS_OF_LUNACY) {
                    return Some(event.to_string());
                }
                if self.has(ctx, SEEDS_OF_DESPAIR) {
                    return Some("30421-12.html".to_string());
                }
                None
            }
            "30421-08.html" | "30415-02.html" => Some(event.to_string()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        let Some(e) = ERRANDS.iter().find(|e| e.mobs.contains(&npc_id)) else {
            return;
        };
        if !self.has(ctx, e.tool) || ctx.quest_items_count(e.material) >= e.need {
            return;
        }
        // `getRandom(2) == 0` — equality, not a `<` threshold.
        if ctx.roll(2) != 0 {
            return;
        }
        ctx.give_items(e.material, 1);
        // Java plays a sound and never touches the cond in this quest.
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
            if npc == VARIKA {
                return Some(
                    if self.has(ctx, JEWEL_OF_DARKNESS) {
                        "30421-04.htm"
                    } else {
                        "30421-01.htm"
                    }
                    .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        if npc == VARIKA {
            return self.talk_varika(ctx);
        }
        let Some(e) = ERRANDS.iter().find(|e| e.npc == npc) else {
            return Some(ctx.no_quest_html());
        };
        // Charkeren and Annika additionally require the Seed of Despair;
        // Arkenia does not (Java omits the guard on her branch alone).
        let gated = e.npc != ARKENIA;
        if self.has(ctx, e.seed) || (gated && !self.has(ctx, SEEDS_OF_DESPAIR)) {
            // Charkeren alone has a "nothing more for you" page.
            return Some(if e.npc == CHARKEREN {
                "30415-06.html".to_string()
            } else {
                ctx.no_quest_html()
            });
        }
        let has_tool = self.has(ctx, e.tool);
        let material = ctx.quest_items_count(e.material);
        if !has_tool && material == 0 {
            // Arkenia hands her tool over right here; the others use an event.
            if e.tool_event.is_none() {
                ctx.give_items(e.tool, 1);
            }
            return Some(e.offer.to_string());
        }
        if has_tool && material < e.need {
            return Some(e.collecting.to_string());
        }
        ctx.give_items(e.seed, 1);
        ctx.take_items(e.material, -1);
        ctx.take_items(e.tool, 1);
        Some(e.trade.to_string())
    }
}

impl Q00412PathOfTheDarkWizard {
    fn talk_varika(&self, ctx: &mut QuestCtx) -> Option<String> {
        if SEEDS.iter().all(|id| ctx.quest_items_count(*id) > 0) {
            ctx.give_items(JEWEL_OF_DARKNESS, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30421-13.html".to_string());
        }
        if !self.has(ctx, SEEDS_OF_DESPAIR) {
            return Some(ctx.no_quest_html());
        }
        let carrying = [
            FAMILYS_REMAINS,
            LUCKY_KEY,
            CANDLE,
            HUB_SCENT,
            KNEE_BONE,
            HEART_OF_LUNACY,
        ]
        .iter()
        .any(|id| ctx.quest_items_count(*id) > 0);
        if !carrying {
            return Some("30421-14.html".to_string());
        }
        Some(
            if !self.has(ctx, SEEDS_OF_ANGER) {
                "30421-08.html"
            } else if !self.has(ctx, SEEDS_OF_HORROR) {
                "30421-15.html"
            } else {
                "30421-12.html"
            }
            .to_string(),
        )
    }
}
