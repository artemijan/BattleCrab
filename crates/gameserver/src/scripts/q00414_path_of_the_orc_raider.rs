//! Path Of The Orc Raider (414) — port of
//! `dist/game/data/scripts/quests/Q00414_PathOfTheOrcRaider/`.
//!
//! Awards the **Mark of Raider** (1592), one of `OrcChange1`'s proofs. Opens
//! the Orc tier.
//!
//! ## The green blood is an escalating summon, not a collection
//!
//! Killing Goblin Tomb Raider Leaders looks like a normal collect step but
//! isn't. Java rolls the *held count against the RNG*:
//!
//! ```java
//! if (getQuestItemsCount(killer, GREEN_BLOOD) <= getRandom(20)) {
//!     giveItems(killer, GREEN_BLOOD, 1);        // gain one
//! } else {
//!     takeItems(killer, GREEN_BLOOD, -1);       // lose the lot
//!     attackPlayer(addSpawn(KURUKA_RATMAN_LEADER, ...), killer);
//! }
//! ```
//!
//! `getRandom(20)` is 0..=19, so at 0 blood the gain is certain, at 19 it is
//! 5%, and at 20 the outer gate still admits you but the roll can never
//! succeed — the summon is guaranteed. **Blood is a rising summon meter, not
//! loot**: it is spent (wiped) the moment Kuruka appears, and the tooth you
//! actually need comes from killing *him*. Porting this as a capped
//! collection would make the quest unfinishable, since nothing else drops the
//! tooth.
//!
//! Uses [`QuestCtx::spawn_attacker`] from slice 13. One cosmetic fidelity
//! gap, deliberate: Java passes `isSummonSpawn = true` (a spawn animation)
//! and seeds hate 999 via `addDamageHate`; our helper seeds dominant hate and
//! skips the animation — same fight, no flash. Revisit only if the visual
//! ever matters.
//!
//! ## A branch that is dead at both ends — verified, then kept
//!
//! Karukia's `30570-07b` route sets `memoState = 2`, `cond = 5` and leads to
//! events on NPC **31978**, who ships five pages in this quest's directory.
//! Neither end is wired up:
//!
//! - `31978` is **not** in `addStartNpc`/`addTalkId` — here or in any other
//!   shipped script (grepped the whole `data/scripts` tree), so its pages are
//!   orphaned.
//! - `30570-07.htm` offers **only** the `07a` button, so nothing in the UI
//!   reaches `07b` either.
//!
//! So the whole route is unreachable in both directions. That matters: had
//! only the *serving* end been missing, a player taking `07b` would be
//! stranded — it consumes the map and teeth but hands out no reports, and the
//! reports are the only path to the reward. Because the button doesn't exist,
//! there is no such trap. Ported verbatim anyway (it costs nothing and keeps
//! the diff against Java honest). This note is the record, so nobody
//! "restores" the button without also registering 31978.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const PREFECT_KARUKIA: i32 = 30570;
const PREFECT_KASMAN: i32 = 30501;

const GREEN_BLOOD: i32 = 1578;
const GOBLIN_DWELLING_MAP: i32 = 1579;
const KURUKA_RATMAN_TOOTH: i32 = 1580;
const BETRAYER_UMBAR_REPORT: i32 = 1589;
const BETRAYER_ZAKAN_REPORT: i32 = 1590;
const HEAD_OF_BETRAYER: i32 = 1591;
const MARK_OF_RAIDER: i32 = 1592;
/// Registered by Java but never given or taken — dead in the shipped quest.
const TIMORA_ORC_HEAD: i32 = 8544;

const KURUKA_RATMAN_LEADER: i32 = 27045;
const UMBAR_ORC: i32 = 27054;
const GOBLIN_TOMB_RAIDER_LEADER: i32 = 20320;

const ORC_FIGHTER: i32 = 44;
const ORC_RAIDER: i32 = 45;
const MIN_LEVEL: i32 = 19;

const TEETH_NEEDED: i64 = 10;
const HEADS_NEEDED: i64 = 2;
/// `getRandom(20)` — the summon meter's ceiling.
const BLOOD_BOUND: i32 = 20;

const QUEST_ITEMS: [i32; 7] = [
    GREEN_BLOOD,
    GOBLIN_DWELLING_MAP,
    KURUKA_RATMAN_TOOTH,
    BETRAYER_UMBAR_REPORT,
    BETRAYER_ZAKAN_REPORT,
    HEAD_OF_BETRAYER,
    TIMORA_ORC_HEAD,
];

pub struct Q00414PathOfTheOrcRaider;

impl Q00414PathOfTheOrcRaider {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn has_a_report(&self, ctx: &QuestCtx) -> bool {
        self.has(ctx, BETRAYER_UMBAR_REPORT) || self.has(ctx, BETRAYER_ZAKAN_REPORT)
    }
}

impl QuestScript for Q00414PathOfTheOrcRaider {
    fn id(&self) -> i32 {
        414
    }
    fn name(&self) -> &'static str {
        "Q00414_PathOfTheOrcRaider"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00414_PathOfTheOrcRaider"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PREFECT_KARUKIA]
    }
    /// Note 31978 is deliberately absent — see the module header.
    fn talk_npcs(&self) -> &[i32] {
        &[PREFECT_KARUKIA, PREFECT_KASMAN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[KURUKA_RATMAN_LEADER, UMBAR_ORC, GOBLIN_TOMB_RAIDER_LEADER]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == PREFECT_KARUKIA {
                return Some("30570-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            PREFECT_KARUKIA => self.talk_karukia(ctx),
            PREFECT_KASMAN => self.talk_kasman(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                ORC_FIGHTER if ctx.player_level() < MIN_LEVEL => "30570-02.htm".to_string(),
                ORC_FIGHTER if self.has(ctx, MARK_OF_RAIDER) => "30570-04.htm".to_string(),
                ORC_FIGHTER => {
                    if !self.has(ctx, GOBLIN_DWELLING_MAP) {
                        ctx.give_items(GOBLIN_DWELLING_MAP, 1);
                    }
                    ctx.start_quest();
                    "30570-05.htm".to_string()
                }
                ORC_RAIDER => "30570-02a.htm".to_string(),
                _ => "30570-03.htm".to_string(),
            }),
            // The teeth buy both betrayer reports.
            "30570-07a.htm" => {
                if self.has(ctx, GOBLIN_DWELLING_MAP)
                    && ctx.quest_items_count(KURUKA_RATMAN_TOOTH) >= TEETH_NEEDED
                {
                    ctx.take_items(GOBLIN_DWELLING_MAP, 1);
                    ctx.take_items(KURUKA_RATMAN_TOOTH, -1);
                    ctx.give_items(BETRAYER_UMBAR_REPORT, 1);
                    ctx.give_items(BETRAYER_ZAKAN_REPORT, 1);
                    ctx.set_cond(3, true);
                    return Some(event.to_string());
                }
                None
            }
            // SKIP(dead): `07b` and the two `31978` events below are
            // unreachable in the shipped datapack — no button posts `07b` and
            // 31978 is registered nowhere. Kept verbatim; do not wire the
            // button back up without also registering 31978 as a talk NPC,
            // because this route hands out no reports and the reports are the
            // only path to the reward.
            "30570-07b.htm" => {
                if self.has(ctx, GOBLIN_DWELLING_MAP)
                    && ctx.quest_items_count(KURUKA_RATMAN_TOOTH) >= TEETH_NEEDED
                {
                    ctx.take_items(GOBLIN_DWELLING_MAP, 1);
                    ctx.take_items(KURUKA_RATMAN_TOOTH, -1);
                    ctx.set_cond(5, true);
                    ctx.set_memo_state(2);
                    return Some(event.to_string());
                }
                None
            }
            "31978-04.htm" => (ctx.memo_state() == 2).then(|| event.to_string()),
            "31978-02.htm" => {
                if ctx.memo_state() == 2 {
                    ctx.set_memo_state(3);
                    ctx.set_cond(6, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            GOBLIN_TOMB_RAIDER_LEADER => {
                let blood = ctx.quest_items_count(GREEN_BLOOD);
                if !self.has(ctx, GOBLIN_DWELLING_MAP)
                    || ctx.quest_items_count(KURUKA_RATMAN_TOOTH) >= TEETH_NEEDED
                    || blood > BLOOD_BOUND as i64
                {
                    return;
                }
                // The meter: held blood raced against `getRandom(20)`.
                if blood <= ctx.roll(BLOOD_BOUND) as i64 {
                    ctx.give_items(GREEN_BLOOD, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                } else {
                    ctx.take_items(GREEN_BLOOD, -1);
                    ctx.spawn_attacker(KURUKA_RATMAN_LEADER, true);
                }
            }
            KURUKA_RATMAN_LEADER => {
                if !self.has(ctx, GOBLIN_DWELLING_MAP)
                    || ctx.quest_items_count(KURUKA_RATMAN_TOOTH) >= TEETH_NEEDED
                {
                    return;
                }
                // The meter resets whichever way the fight went.
                ctx.take_items(GREEN_BLOOD, -1);
                ctx.give_items(KURUKA_RATMAN_TOOTH, 1);
                if ctx.quest_items_count(KURUKA_RATMAN_TOOTH) >= TEETH_NEEDED {
                    ctx.set_cond(2, true);
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            UMBAR_ORC => {
                if !self.has_a_report(ctx)
                    || ctx.quest_items_count(HEAD_OF_BETRAYER) >= HEADS_NEEDED
                    || ctx.roll(10) >= 2
                {
                    return;
                }
                ctx.give_items(HEAD_OF_BETRAYER, 1);
                // Zakan's report is spent first.
                if self.has(ctx, BETRAYER_ZAKAN_REPORT) {
                    ctx.take_items(BETRAYER_ZAKAN_REPORT, 1);
                } else if self.has(ctx, BETRAYER_UMBAR_REPORT) {
                    ctx.take_items(BETRAYER_UMBAR_REPORT, 1);
                }
                if ctx.quest_items_count(HEAD_OF_BETRAYER) == HEADS_NEEDED {
                    ctx.set_cond(4, true);
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            _ => {}
        }
    }
}

impl Q00414PathOfTheOrcRaider {
    fn talk_karukia(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, GOBLIN_DWELLING_MAP) {
            let teeth = ctx.quest_items_count(KURUKA_RATMAN_TOOTH);
            if teeth < TEETH_NEEDED {
                return Some("30570-06.htm".to_string());
            }
            if !self.has_a_report(ctx) {
                return Some("30570-07.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if self.has(ctx, HEAD_OF_BETRAYER) || self.has_a_report(ctx) {
            return Some("30570-08.htm".to_string());
        }
        // Only reachable through the dead `07b` route.
        if ctx.memo_state() == 2 {
            return Some("30570-07b.htm".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_kasman(&self, ctx: &mut QuestCtx) -> Option<String> {
        let heads = ctx.quest_items_count(HEAD_OF_BETRAYER);
        let reports = ctx.quest_items_count(BETRAYER_UMBAR_REPORT)
            + ctx.quest_items_count(BETRAYER_ZAKAN_REPORT);
        if heads == 0 && reports >= 2 {
            return Some("30501-01.htm".to_string());
        }
        if heads == 1 {
            return Some("30501-02.htm".to_string());
        }
        if heads == HEADS_NEEDED {
            ctx.give_items(MARK_OF_RAIDER, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30501-03.htm".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
