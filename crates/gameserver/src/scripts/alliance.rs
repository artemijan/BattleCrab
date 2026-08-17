//! Shared engine for the two opposing **faction-alliance** grinds of the
//! Ketra Valley — `Q00605_AllianceWithKetraOrcs` and
//! `Q00611_AllianceWithVarkaSilenos`. The two Java scripts are line-for-line
//! identical bar their NPC / mob / item ids (an author-confirmed mirror pair:
//! ally with Ketra by hunting Varka, or the reverse), so both reduce to one
//! generic [`AllianceQuest`] driven by a per-quest [`AllianceData`] table — the
//! same treatment the Saga super-class got.
//!
//! The quest is a six-rank reputation ladder. You hunt the *enemy* faction's
//! camp for three tiers of **badges** (Soldier / Officer / Captain), turn them
//! in to your patron NPC to climb from Mark of Alliance Lv1 to Lv5, and two
//! **totems** (from the sibling collection quests) gate the last two ranks.
//! Killing an enemy drops the badge that matches *your current rank*, capped so
//! you never bank more than the next turn-in needs.
//!
//! Party sharing (`getRandomPartyMemberState`) collapses to the killer, the
//! project-wide documented `onKill` deviation.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const MIN_LEVEL: i32 = 74;
// Badges required *in inventory* to advance out of each rank (index = cond-1).
// A tier that reads 0 is not yet demanded at that rank.
const SOLDIER_BADGE_COUNT: [i32; 5] = [100, 200, 300, 300, 400];
const OFFICER_BADGE_COUNT: [i32; 5] = [0, 100, 200, 300, 400];
const CAPTAIN_BADGE_COUNT: [i32; 5] = [0, 0, 100, 200, 200];

/// One enemy mob's drop: `(npc_id, chance_out_of_1000, min_cond)`. The badge it
/// yields is chosen by `min_cond` (1 → Soldier, 2 → Officer, 3+ → Captain).
pub type MobDrop = (i32, i32, i32);

/// Per-quest data — everything the two Alliance scripts differ by.
pub struct AllianceData {
    /// The patron NPC (Wahkan / Naran Ashanuk); also the html-file id prefix.
    pub start_npc: i32,
    /// Mark of *your* alliance, Lv1..Lv5 — awarded as you climb.
    pub own_marks: [i32; 5],
    /// Mark of the *opposing* alliance — holding any blocks starting here.
    pub enemy_marks: [i32; 5],
    /// The three enemy badge tiers: `[soldier, officer, captain]`.
    pub badges: [i32; 3],
    /// Totems gating cond 4→5 (valor) and 5→6 (wisdom).
    pub valor_totem: i32,
    pub wisdom_totem: i32,
    /// The enemy camp's mobs and their badge drops.
    pub mobs: &'static [MobDrop],
}

pub struct AllianceQuest {
    data: AllianceData,
    /// The enemy mob ids (`addKillId`), materialised for the registry's `&[i32]`.
    mob_ids: Vec<i32>,
    /// The three badges (`registerQuestItems`), materialised likewise.
    badges_reg: Vec<i32>,
}

impl AllianceQuest {
    pub fn new(data: AllianceData) -> Self {
        let mob_ids = data.mobs.iter().map(|(id, _, _)| *id).collect();
        let badges_reg = data.badges.to_vec();
        Self {
            data,
            mob_ids,
            badges_reg,
        }
    }

    fn badge_soldier(&self) -> i32 {
        self.data.badges[0]
    }
    fn badge_officer(&self) -> i32 {
        self.data.badges[1]
    }
    fn badge_captain(&self) -> i32 {
        self.data.badges[2]
    }

    /// Resolve one of this quest's html files by suffix (`"04.htm"` →
    /// `"31371-04.htm"` / `"31378-04.htm"`).
    fn html(&self, suffix: &str) -> String {
        format!("{}-{}", self.data.start_npc, suffix)
    }

    /// The badge a mob with `min_cond` yields (Java `DropInfo`'s switch).
    fn badge_for_min_cond(&self, min_cond: i32) -> i32 {
        match min_cond {
            1 => self.badge_soldier(),
            2 => self.badge_officer(),
            _ => self.badge_captain(),
        }
    }

    /// Java `canGetItem`: whether the killer may still bank this badge — i.e.
    /// they hold fewer than the *current rank's* turn-in demands for it.
    fn can_get_item(&self, ctx: &QuestCtx, item_id: i32) -> bool {
        let cond = ctx.cond();
        let needed = if item_id == self.badge_soldier() {
            SOLDIER_BADGE_COUNT[(cond - 1) as usize]
        } else if item_id == self.badge_officer() {
            OFFICER_BADGE_COUNT[(cond - 1) as usize]
        } else if item_id == self.badge_captain() {
            CAPTAIN_BADGE_COUNT[(cond - 1) as usize]
        } else {
            0
        };
        ctx.quest_items_count(item_id) < needed as i64
    }

    fn count(&self, ctx: &QuestCtx, item_id: i32) -> i32 {
        ctx.quest_items_count(item_id) as i32
    }

    fn has(&self, ctx: &QuestCtx, item_id: i32) -> bool {
        ctx.quest_items_count(item_id) > 0
    }

    fn has_any_enemy_mark(&self, ctx: &QuestCtx) -> bool {
        self.data.enemy_marks.iter().any(|&m| self.has(ctx, m))
    }
}

impl QuestScript for AllianceQuest {
    fn id(&self) -> i32 {
        // Derived from the patron NPC so the two share no state key.
        if self.data.start_npc == 31371 {
            605
        } else {
            611
        }
    }
    fn name(&self) -> &'static str {
        if self.data.start_npc == 31371 {
            "Q00605_AllianceWithKetraOrcs"
        } else {
            "Q00611_AllianceWithVarkaSilenos"
        }
    }
    fn html_dir(&self) -> &'static str {
        if self.data.start_npc == 31371 {
            "quests/Q00605_AllianceWithKetraOrcs"
        } else {
            "quests/Q00611_AllianceWithVarkaSilenos"
        }
    }
    fn start_npcs(&self) -> &[i32] {
        std::slice::from_ref(&self.data.start_npc)
    }
    fn talk_npcs(&self) -> &[i32] {
        std::slice::from_ref(&self.data.start_npc)
    }
    fn kill_npcs(&self) -> &[i32] {
        &self.mob_ids
    }
    fn quest_items(&self) -> &[i32] {
        &self.badges_reg
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() >= MIN_LEVEL {
                self.html("01.htm")
            } else {
                self.html("02.htm")
            });
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let m = &self.data.own_marks;
        let soldier = self.badge_soldier();
        let officer = self.badge_officer();
        let captain = self.badge_captain();
        match ctx.cond() {
            1 => Some(if self.count(ctx, soldier) >= SOLDIER_BADGE_COUNT[0] {
                self.html("11.html")
            } else {
                self.html("10.html")
            }),
            2 => Some(
                if self.has(ctx, m[0])
                    && self.count(ctx, soldier) >= SOLDIER_BADGE_COUNT[1]
                    && self.count(ctx, officer) >= OFFICER_BADGE_COUNT[1]
                {
                    self.html("14.html")
                } else {
                    self.html("13.html")
                },
            ),
            3 => Some(
                if self.has(ctx, m[1])
                    && self.count(ctx, soldier) >= SOLDIER_BADGE_COUNT[2]
                    && self.count(ctx, officer) >= OFFICER_BADGE_COUNT[2]
                    && self.count(ctx, captain) >= CAPTAIN_BADGE_COUNT[2]
                {
                    self.html("17.html")
                } else {
                    self.html("16.html")
                },
            ),
            4 => Some(
                if self.has(ctx, m[2])
                    && self.has(ctx, self.data.valor_totem)
                    && self.count(ctx, soldier) >= SOLDIER_BADGE_COUNT[3]
                    && self.count(ctx, officer) >= OFFICER_BADGE_COUNT[3]
                    && self.count(ctx, captain) >= CAPTAIN_BADGE_COUNT[3]
                {
                    self.html("20.html")
                } else {
                    self.html("19.html")
                },
            ),
            5 => {
                if !self.has(ctx, m[3])
                    || !self.has(ctx, self.data.wisdom_totem)
                    || self.count(ctx, soldier) < SOLDIER_BADGE_COUNT[4]
                    || self.count(ctx, officer) < OFFICER_BADGE_COUNT[4]
                    || self.count(ctx, captain) < CAPTAIN_BADGE_COUNT[4]
                {
                    return Some(self.html("22.html"));
                }
                ctx.set_cond(6, true);
                ctx.take_items(soldier, -1);
                ctx.take_items(officer, -1);
                ctx.take_items(captain, -1);
                ctx.take_items(self.data.wisdom_totem, -1);
                ctx.take_items(m[3], -1);
                ctx.give_items(m[4], 1);
                Some(self.html("23.html"))
            }
            6 => {
                if self.has(ctx, m[4]) {
                    Some(self.html("24.html"))
                } else {
                    Some(ctx.no_quest_html())
                }
            }
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        let prefix = format!("{}-", self.data.start_npc);
        let suffix = event.strip_prefix(&prefix)?;
        match suffix {
            // Informational pages — echoed straight back.
            "12a.html" | "12b.html" | "25.html" => Some(event.to_string()),
            "04.htm" => {
                if self.has_any_enemy_mark(ctx) {
                    return Some(self.html("03.htm"));
                }
                ctx.set_state(crate::model::quest::state::STARTED);
                ctx.play_sound(quest_sounds::ACCEPT);
                // Rejoining with an existing mark resumes at the matching rank.
                for i in 0..self.data.own_marks.len() {
                    if self.has(ctx, self.data.own_marks[i]) {
                        ctx.set_cond((i + 2) as i32, false);
                        return Some(self.html(&format!("0{}.htm", i + 5)));
                    }
                }
                ctx.set_cond(1, false);
                Some(event.to_string())
            }
            "12.html" => {
                if self.count(ctx, self.badge_soldier()) < SOLDIER_BADGE_COUNT[0] {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(self.badge_soldier(), -1);
                ctx.give_items(self.data.own_marks[0], 1);
                ctx.set_cond(2, true);
                Some(event.to_string())
            }
            "15.html" => {
                if self.count(ctx, self.badge_soldier()) < SOLDIER_BADGE_COUNT[1]
                    || self.count(ctx, self.badge_officer()) < OFFICER_BADGE_COUNT[1]
                {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(self.badge_soldier(), -1);
                ctx.take_items(self.badge_officer(), -1);
                ctx.take_items(self.data.own_marks[0], -1);
                ctx.give_items(self.data.own_marks[1], 1);
                ctx.set_cond(3, true);
                Some(event.to_string())
            }
            "18.html" => {
                if self.count(ctx, self.badge_soldier()) < SOLDIER_BADGE_COUNT[2]
                    || self.count(ctx, self.badge_officer()) < OFFICER_BADGE_COUNT[2]
                    || self.count(ctx, self.badge_captain()) < CAPTAIN_BADGE_COUNT[2]
                {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(self.badge_soldier(), -1);
                ctx.take_items(self.badge_officer(), -1);
                ctx.take_items(self.badge_captain(), -1);
                ctx.take_items(self.data.own_marks[1], -1);
                ctx.give_items(self.data.own_marks[2], 1);
                ctx.set_cond(4, true);
                Some(event.to_string())
            }
            "21.html" => {
                if !self.has(ctx, self.data.valor_totem)
                    || self.count(ctx, self.badge_soldier()) < SOLDIER_BADGE_COUNT[3]
                    || self.count(ctx, self.badge_officer()) < OFFICER_BADGE_COUNT[3]
                    || self.count(ctx, self.badge_captain()) < CAPTAIN_BADGE_COUNT[3]
                {
                    return Some(ctx.no_quest_html());
                }
                ctx.take_items(self.badge_soldier(), -1);
                ctx.take_items(self.badge_officer(), -1);
                ctx.take_items(self.badge_captain(), -1);
                ctx.take_items(self.data.valor_totem, -1);
                ctx.take_items(self.data.own_marks[2], -1);
                ctx.give_items(self.data.own_marks[3], 1);
                ctx.set_cond(5, true);
                Some(event.to_string())
            }
            "26.html" => {
                // Renounce the alliance: surrender every mark and totem.
                for &m in &self.data.own_marks {
                    ctx.take_items(m, -1);
                }
                ctx.take_items(self.data.valor_totem, -1);
                ctx.take_items(self.data.wisdom_totem, -1);
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let Some(&(_, chance, min_cond)) =
            self.data.mobs.iter().find(|(id, _, _)| *id == ctx.npc_id)
        else {
            return;
        };
        let cond = ctx.cond();
        let badge = self.badge_for_min_cond(min_cond);
        if cond >= min_cond && cond < 6 && self.can_get_item(ctx, badge) && ctx.roll(1000) < chance
        {
            ctx.give_items(badge, 1);
        }
    }
}
