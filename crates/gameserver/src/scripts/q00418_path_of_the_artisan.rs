//! Path Of The Artisan (418) — port of
//! `dist/game/data/scripts/quests/Q00418_PathOfTheArtisan/`.
//!
//! Awards the **Final Pass Certificate** (1635), one of `DwarfBlacksmithChange1`'s
//! proofs. Opens the Dwarf tier — the last race whose `*Change1` script is
//! still proof-starved.
//!
//! Silvera's ring → 10 Boogle Ratman teeth + 2 leader teeth → 1st pass →
//! Kluto's letter → Pinter swaps it for the thief's footprint → a Vuku Orc
//! drops the stolen box → Pinter's 2nd pass + the secret box → back to Kluto.
//!
//! ## The leader-tooth roll has a hole in it, and it is Java's
//!
//! ```java
//! if (getRandom(10) < 5) {
//!     if (getQuestItemsCount(killer, BOOGLE_RATMAN_LEADERS_TOOTH) == 1) { …give, MIDDLE… }
//!     // …and nothing at all when the count is 0
//! } else {
//!     giveItems(killer, BOOGLE_RATMAN_LEADERS_TOOTH, 1);  // always
//! }
//! ```
//!
//! On a roll below 5 the kill pays **only** if you already hold exactly one
//! tooth; at zero teeth that half of the roll does nothing. So the first tooth
//! comes at 50%, the second at 100%. Reading it as a flat "50% per tooth"
//! would be wrong in both directions.
//!
//! A consequence worth not "fixing": the `else` branch hands over the second
//! tooth **without** the `cond 2` check that the `< 5` branch performs, so
//! finishing the leader teeth through that path never sets cond 2. The quest
//! still completes — every downstream branch tests item counts, not the cond —
//! so this is a cosmetic Java bug (a stale quest window), ported verbatim.
//!
//! ## Two routes to Kluto's letter, differing only in the sound
//!
//! `30317-04.html` uses `setCond(4, true)` and `30317-07.html` uses the
//! single-argument `setCond(4)`. Same item, same cond, one plays the middle
//! chime and the other doesn't. Preserved.
//!
//! ## Dead at both ends again — fourth quest running
//!
//! `30527-08c` sets `memoState = 10` and, with NPCs **31956 / 31963 / 32052**,
//! opens alternate routes including their own certificate hand-outs and
//! Lockirin's `memoState == 101` branch. Nothing reaches any of it: only
//! `30527-08b` is offered by a page, and none of those three NPCs is
//! registered. Their pages ship and are orphaned. Omitted rather than stubbed,
//! as in quest 416.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const BLACKSMITH_PINTER: i32 = 30298;
const BLACKSMITH_KLUTO: i32 = 30317;
const BLACKSMITH_SILVERA: i32 = 30527;
const IRON_GATES_LOCKIRIN: i32 = 30531;

const SILVERYS_RING: i32 = 1632;
const PASS_1ST_CERTIFICATE: i32 = 1633;
const PASS_2ND_CERTIFICATE: i32 = 1634;
const FINAL_PASS_CERTIFICATE: i32 = 1635;
const BOOGLE_RATMAN_TOOTH: i32 = 1636;
const BOOGLE_RATMAN_LEADERS_TOOTH: i32 = 1637;
const KLUTOS_LETTER: i32 = 1638;
const FOOTPRINT_OF_THIEF: i32 = 1639;
const STOLEN_SECRET_BOX: i32 = 1640;
const SECRET_BOX: i32 = 1641;

const VUKU_ORC_FIGHTER: i32 = 20017;
const BOOGLE_RATMAN: i32 = 20389;
const BOOGLE_RATMAN_LEADER: i32 = 20390;

const DWARVEN_FIGHTER: i32 = 53;
const ARTISAN: i32 = 56;
const MIN_LEVEL: i32 = 19;

const TEETH_NEEDED: i64 = 10;
const LEADER_TEETH_NEEDED: i64 = 2;

const QUEST_ITEMS: [i32; 9] = [
    SILVERYS_RING, PASS_1ST_CERTIFICATE, PASS_2ND_CERTIFICATE, BOOGLE_RATMAN_TOOTH,
    BOOGLE_RATMAN_LEADERS_TOOTH, KLUTOS_LETTER, FOOTPRINT_OF_THIEF, STOLEN_SECRET_BOX, SECRET_BOX,
];

pub struct Q00418PathOfTheArtisan;

impl Q00418PathOfTheArtisan {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn has_any(&self, ctx: &QuestCtx, items: &[i32]) -> bool {
        items.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }
    fn teeth_done(&self, ctx: &QuestCtx) -> bool {
        ctx.quest_items_count(BOOGLE_RATMAN_TOOTH) >= TEETH_NEEDED
            && ctx.quest_items_count(BOOGLE_RATMAN_LEADERS_TOOTH) >= LEADER_TEETH_NEEDED
    }
    /// Both of Kluto's finish buttons do the same thing.
    fn finish(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !self.has(ctx, PASS_2ND_CERTIFICATE) || !self.has(ctx, SECRET_BOX) {
            return None;
        }
        ctx.give_items(FINAL_PASS_CERTIFICATE, 1);
        // Java's three-way level branch awards identical exp/sp.
        ctx.add_exp_and_sp(80314, 5087);
        ctx.exit_quest(false, true);
        ctx.social_action(3);
        Some(event.to_string())
    }
}

impl QuestScript for Q00418PathOfTheArtisan {
    fn id(&self) -> i32 {
        418
    }
    fn name(&self) -> &'static str {
        "Q00418_PathOfTheArtisan"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00418_PathOfTheArtisan"
    }
    fn start_npcs(&self) -> &[i32] {
        &[BLACKSMITH_SILVERA]
    }
    /// 31956 / 31963 / 32052 are deliberately absent — see the module header.
    fn talk_npcs(&self) -> &[i32] {
        &[BLACKSMITH_SILVERA, BLACKSMITH_PINTER, BLACKSMITH_KLUTO, IRON_GATES_LOCKIRIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[VUKU_ORC_FIGHTER, BOOGLE_RATMAN, BOOGLE_RATMAN_LEADER]
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
                    DWARVEN_FIGHTER if ctx.player_level() < MIN_LEVEL => "30527-03.htm",
                    DWARVEN_FIGHTER if self.has(ctx, FINAL_PASS_CERTIFICATE) => "30527-04.htm",
                    DWARVEN_FIGHTER => "30527-05.htm",
                    ARTISAN => "30527-02a.htm",
                    _ => "30527-02.htm",
                }
                .to_string(),
            ),
            "30527-06.htm" => {
                ctx.start_quest();
                ctx.give_items(SILVERYS_RING, 1);
                Some(event.to_string())
            }
            // Teeth in, first pass out. (Java runs this unconditionally.)
            "30527-08b.html" => {
                ctx.take_items(SILVERYS_RING, 1);
                ctx.take_items(BOOGLE_RATMAN_TOOTH, -1);
                ctx.take_items(BOOGLE_RATMAN_LEADERS_TOOTH, -1);
                ctx.give_items(PASS_1ST_CERTIFICATE, 1);
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            // Pure navigation.
            "30298-02.html" | "30317-02.html" | "30317-03.html" | "30317-05.html"
            | "30317-06.html" | "30317-11.html" | "30531-02.html" | "30531-03.html"
            | "30531-04.html" => Some(event.to_string()),
            // Two routes to the same letter — one chimes, one doesn't.
            "30317-04.html" => {
                ctx.give_items(KLUTOS_LETTER, 1);
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            "30317-07.html" => {
                ctx.give_items(KLUTOS_LETTER, 1);
                ctx.set_cond(4, false);
                Some(event.to_string())
            }
            "30298-03.html" => {
                if self.has(ctx, KLUTOS_LETTER) {
                    ctx.take_items(KLUTOS_LETTER, 1);
                    ctx.give_items(FOOTPRINT_OF_THIEF, 1);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30298-06.html" => {
                if self.has(ctx, FOOTPRINT_OF_THIEF) && self.has(ctx, STOLEN_SECRET_BOX) {
                    ctx.give_items(PASS_2ND_CERTIFICATE, 1);
                    ctx.take_items(FOOTPRINT_OF_THIEF, 1);
                    ctx.take_items(STOLEN_SECRET_BOX, 1);
                    ctx.give_items(SECRET_BOX, 1);
                    ctx.set_cond(7, true);
                    return Some(event.to_string());
                }
                None
            }
            "30317-10.html" | "30317-12.html" => self.finish(ctx, event),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        match ctx.npc_id {
            VUKU_ORC_FIGHTER => {
                if self.has(ctx, FOOTPRINT_OF_THIEF)
                    && !self.has(ctx, STOLEN_SECRET_BOX)
                    && ctx.roll(10) < 2
                {
                    ctx.give_items(STOLEN_SECRET_BOX, 1);
                    ctx.set_cond(6, true);
                }
            }
            BOOGLE_RATMAN => {
                if !self.has(ctx, SILVERYS_RING)
                    || ctx.quest_items_count(BOOGLE_RATMAN_TOOTH) >= TEETH_NEEDED
                    || ctx.roll(10) >= 7
                {
                    return;
                }
                let last = ctx.quest_items_count(BOOGLE_RATMAN_TOOTH) == TEETH_NEEDED - 1;
                ctx.give_items(BOOGLE_RATMAN_TOOTH, 1);
                if last {
                    ctx.play_sound(quest_sounds::MIDDLE);
                    if ctx.quest_items_count(BOOGLE_RATMAN_LEADERS_TOOTH) >= LEADER_TEETH_NEEDED {
                        ctx.set_cond(2, false);
                    }
                } else {
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            // The lopsided roll — see the module header.
            BOOGLE_RATMAN_LEADER => {
                if !self.has(ctx, SILVERYS_RING)
                    || ctx.quest_items_count(BOOGLE_RATMAN_LEADERS_TOOTH) >= LEADER_TEETH_NEEDED
                {
                    return;
                }
                if ctx.roll(10) < 5 {
                    // Pays only when one tooth is already held; at zero this
                    // half of the roll does nothing at all.
                    if ctx.quest_items_count(BOOGLE_RATMAN_LEADERS_TOOTH) == 1 {
                        ctx.give_items(BOOGLE_RATMAN_LEADERS_TOOTH, 1);
                        ctx.play_sound(quest_sounds::MIDDLE);
                        if ctx.quest_items_count(BOOGLE_RATMAN_TOOTH) >= TEETH_NEEDED {
                            ctx.set_cond(2, false);
                        }
                    }
                } else {
                    // Always pays — and deliberately performs no cond check,
                    // so finishing here leaves the quest window stale.
                    ctx.give_items(BOOGLE_RATMAN_LEADERS_TOOTH, 1);
                    ctx.play_sound(quest_sounds::ITEMGET);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == BLACKSMITH_SILVERA {
                return Some("30527-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        match npc {
            BLACKSMITH_SILVERA => self.talk_silvera(ctx),
            BLACKSMITH_PINTER => self.talk_pinter(ctx),
            BLACKSMITH_KLUTO => self.talk_kluto(ctx),
            // Only reachable through the dead `memoState == 101` route.
            IRON_GATES_LOCKIRIN => Some(ctx.no_quest_html()),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00418PathOfTheArtisan {
    fn talk_silvera(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, SILVERYS_RING) {
            let total = ctx.quest_items_count(BOOGLE_RATMAN_TOOTH)
                + ctx.quest_items_count(BOOGLE_RATMAN_LEADERS_TOOTH);
            if total < 12 {
                return Some("30527-07.html".to_string());
            }
            if self.teeth_done(ctx) {
                return Some("30527-08a.html".to_string());
            }
        }
        if self.has(ctx, PASS_1ST_CERTIFICATE) {
            return Some("30527-09.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_pinter(&self, ctx: &mut QuestCtx) -> Option<String> {
        if !self.has(ctx, PASS_1ST_CERTIFICATE) {
            return Some(ctx.no_quest_html());
        }
        if self.has(ctx, KLUTOS_LETTER) {
            return Some("30298-01.html".to_string());
        }
        let footprint = self.has(ctx, FOOTPRINT_OF_THIEF);
        let stolen = self.has(ctx, STOLEN_SECRET_BOX);
        if footprint && !stolen {
            return Some("30298-04.html".to_string());
        }
        if footprint && stolen {
            return Some("30298-05.html".to_string());
        }
        if self.has(ctx, PASS_2ND_CERTIFICATE) && self.has(ctx, SECRET_BOX) {
            return Some("30298-07.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_kluto(&self, ctx: &mut QuestCtx) -> Option<String> {
        if !self.has(ctx, PASS_1ST_CERTIFICATE) {
            return Some(ctx.no_quest_html());
        }
        if self.has(ctx, PASS_2ND_CERTIFICATE) && self.has(ctx, SECRET_BOX) {
            return Some("30317-09.html".to_string());
        }
        if self.has_any(ctx, &[KLUTOS_LETTER, FOOTPRINT_OF_THIEF]) {
            return Some("30317-08.html".to_string());
        }
        if !self.has_any(
            ctx,
            &[FOOTPRINT_OF_THIEF, KLUTOS_LETTER, PASS_2ND_CERTIFICATE, SECRET_BOX],
        ) {
            return Some("30317-01.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
