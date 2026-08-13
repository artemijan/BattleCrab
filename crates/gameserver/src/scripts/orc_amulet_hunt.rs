//! The shared engine behind the two orc-amulet newbie quests — Q00260 *Orc
//! Hunting* (Elf, Rayen, Kaboo orcs) and Q00263 *Orc Subjugation* (Dark Elf,
//! Kayleen, Balor orcs). Java ships them as two near-identical classes; here
//! they are one engine over an [`OrcAmuletHuntData`] table.
//!
//! The loop is the same in both: a race- and level-gated start under a
//! `addCondMaxLevel(16)` ceiling, a 50 % drop of whichever item the dead orc
//! carries, and a turn-in that pays per item plus a flat bonus once ten or
//! more are handed over at once.

use crate::game_loop::quests::{QuestCtx, QuestScript};

/// Java's `addCondMaxLevel(16, getNoQuestMsg(null))` — shared by both.
const MAX_LEVEL: i32 = 16;
/// The item count that earns [`OrcAmuletHuntData::bulk_bonus`].
const BULK_AT: i64 = 10;

/// Per-quest data for one orc-amulet quest.
pub struct OrcAmuletHuntData {
    pub id: i32,
    pub name: &'static str,
    pub html_dir: &'static str,
    /// The quest-giver — whose id also prefixes every html page below.
    pub npc: i32,
    pub amulet: i32,
    pub necklace: i32,
    /// monster id → dropped item.
    pub monsters: &'static [(i32, i32)],
    pub min_level: i32,
    pub race: i32,
    /// Adena per amulet and per necklace, plus the flat bonus for turning in
    /// ten or more items at once.
    pub amulet_price: i64,
    pub necklace_price: i64,
    pub bulk_bonus: i64,
    /// Pages 1 and 2 — the "wrong race" and "too low" refusals. Spelled out
    /// rather than derived, because the dist disagrees on `.htm` vs `.html`
    /// for exactly these two; every other page follows [`OrcAmuletHunt::page`].
    pub wrong_race_page: &'static str,
    pub too_low_page: &'static str,
}

pub struct OrcAmuletHunt {
    data: OrcAmuletHuntData,
    npcs: [i32; 1],
    kill_ids: Vec<i32>,
    quest_items: [i32; 2],
}

impl OrcAmuletHunt {
    pub fn new(data: OrcAmuletHuntData) -> Self {
        let npcs = [data.npc];
        let kill_ids = data.monsters.iter().map(|(npc, _)| *npc).collect();
        let quest_items = [data.amulet, data.necklace];
        Self {
            data,
            npcs,
            kill_ids,
            quest_items,
        }
    }

    /// `<quest-giver id>-<suffix>` — the html naming both quests share.
    fn page(&self, suffix: &str) -> String {
        format!("{}-{suffix}", self.data.npc)
    }
}

impl QuestScript for OrcAmuletHunt {
    fn id(&self) -> i32 {
        self.data.id
    }
    fn name(&self) -> &'static str {
        self.data.name
    }
    fn html_dir(&self) -> &'static str {
        self.data.html_dir
    }
    fn start_npcs(&self) -> &[i32] {
        &self.npcs
    }
    fn talk_npcs(&self) -> &[i32] {
        &self.npcs
    }
    fn kill_npcs(&self) -> &[i32] {
        &self.kill_ids
    }
    fn quest_items(&self) -> &[i32] {
        &self.quest_items
    }

    /// `addCondMaxLevel(16, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > MAX_LEVEL).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        if event == self.page("04.htm") {
            ctx.start_quest();
        } else if event == self.page("07.html") {
            ctx.exit_quest(true, true);
        } else if event != self.page("08.html") {
            return None;
        }
        Some(event.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        if ctx.roll(10) > 4 {
            ctx.give_table_drop(self.data.monsters, self.data.amulet);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_race() != self.data.race {
                self.data.wrong_race_page.to_string()
            } else if ctx.player_level() >= self.data.min_level {
                self.page("03.htm")
            } else {
                self.data.too_low_page.to_string()
            });
        }
        if ctx.is_started() {
            let amulets = ctx.quest_items_count(self.data.amulet);
            let necklaces = ctx.quest_items_count(self.data.necklace);
            return Some(if amulets + necklaces > 0 {
                ctx.give_adena(
                    (amulets * self.data.amulet_price)
                        + (necklaces * self.data.necklace_price)
                        + if amulets + necklaces >= BULK_AT {
                            self.data.bulk_bonus
                        } else {
                            0
                        },
                    true,
                );
                ctx.take_items(self.data.amulet, -1);
                ctx.take_items(self.data.necklace, -1);
                self.page("06.html")
            } else {
                self.page("05.html")
            });
        }
        Some(ctx.no_quest_html())
    }
}
