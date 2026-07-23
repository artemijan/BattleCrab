//! Family Honor (355) — `quests/Q00355_FamilyHonor`. Galibredo (30181, level
//! 36–49) buys Galfredo Romer's Busts dropped by Timak Orc Troops in the Forest
//! of Mirrors (20 adena each, or a 120-adena "sell to a collector" exit). The
//! same orcs rarely drop a Sculptor Berona statue, which Patrin (30929) appraises
//! into one of four ancient statues on a weighted roll. `addCondMaxLevel(49)`.
use crate::game_loop::quests::{QuestCtx, QuestScript};

const GALIBREDO: i32 = 30181;
const PATRIN: i32 = 30929;
const GALFREDO_ROMERS_BUST: i32 = 4252;
const SCULPTOR_BERONA: i32 = 4350;
const ANCIENT_STATUE_PROTOTYPE: i32 = 4351;
const ANCIENT_STATUE_ORIGINAL: i32 = 4352;
const ANCIENT_STATUE_REPLICA: i32 = 4353;
const ANCIENT_STATUE_FORGERY: i32 = 4354;
const MIN_LEVEL: i32 = 36;

/// `MOBS`: per-mob (first, second) thresholds out of 1000 — below `first` drops
/// a bust, below `second` drops a Berona.
fn drop_info(npc_id: i32) -> Option<(i32, i32)> {
    match npc_id {
        20767 => Some((560, 684)), // timak_orc_troop_leader
        20768 => Some((530, 650)), // timak_orc_troop_shaman
        20769 => Some((420, 516)), // timak_orc_troop_warrior
        20770 => Some((440, 560)), // timak_orc_troop_archer
        _ => None,
    }
}

pub struct Q00355FamilyHonor;

impl QuestScript for Q00355FamilyHonor {
    fn id(&self) -> i32 {
        355
    }
    fn name(&self) -> &'static str {
        "Q00355_FamilyHonor"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00355_FamilyHonor"
    }
    fn start_npcs(&self) -> &[i32] {
        &[GALIBREDO]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[GALIBREDO, PATRIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[20767, 20768, 20769, 20770]
    }
    fn quest_items(&self) -> &[i32] {
        &[GALFREDO_ROMERS_BUST]
    }

    /// `addCondMaxLevel(49, getNoQuestMsg(null))`.
    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        (ctx.player_level() > 49).then(|| ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "30181-02.htm" | "30181-09.html" | "30929-01.html" | "30929-02.html" => {
                Some(event.to_string())
            }
            "30181-03.htm" => {
                ctx.start_quest();
                Some(event.to_string())
            }
            "30181-06.html" => {
                let busts = ctx.quest_items_count(GALFREDO_ROMERS_BUST);
                if busts < 1 {
                    Some(event.to_string())
                } else {
                    ctx.give_adena(busts * 20, true);
                    ctx.take_items(GALFREDO_ROMERS_BUST, -1);
                    Some(if busts >= 100 { "30181-07.html" } else { "30181-08.html" }.to_string())
                }
            }
            "30181-10.html" => {
                let busts = ctx.quest_items_count(GALFREDO_ROMERS_BUST);
                if busts > 0 {
                    ctx.give_adena(busts * 120, true);
                }
                ctx.take_items(GALFREDO_ROMERS_BUST, -1);
                ctx.exit_quest(true, true);
                Some(event.to_string())
            }
            "30929-03.html" => {
                if ctx.quest_items_count(SCULPTOR_BERONA) == 0 {
                    return Some("30929-08.html".to_string());
                }
                let random = ctx.roll(100);
                let (item, html) = if random < 2 {
                    (ANCIENT_STATUE_PROTOTYPE, "30929-03.html")
                } else if random < 32 {
                    (ANCIENT_STATUE_ORIGINAL, "30929-04.html")
                } else if random < 62 {
                    (ANCIENT_STATUE_REPLICA, "30929-05.html")
                } else if random < 77 {
                    (ANCIENT_STATUE_FORGERY, "30929-06.html")
                } else {
                    (0, "30929-07.html")
                };
                if item != 0 {
                    ctx.give_items(item, 1);
                }
                ctx.take_items(SCULPTOR_BERONA, 1);
                Some(html.to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // `Util.checkIfInRange(ALT_PARTY_RANGE, npc, killer, true)` is trivially
        // true for the killer; Java gates only on the quest state existing.
        if !ctx.has_qs() {
            return;
        }
        let Some((first, second)) = drop_info(ctx.npc_id) else {
            return;
        };
        let random = ctx.roll(1000);
        if random < first {
            ctx.give_item_randomly(GALFREDO_ROMERS_BUST, 1, 0, 1.0, true);
        } else if random < second {
            ctx.give_item_randomly(SCULPTOR_BERONA, 1, 0, 1.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(
                if ctx.player_level() >= MIN_LEVEL { "30181-01.htm" } else { "30181-04.html" }
                    .to_string(),
            );
        }
        if ctx.is_started() {
            if ctx.npc_id == GALIBREDO {
                return Some(
                    if ctx.quest_items_count(SCULPTOR_BERONA) > 0 {
                        "30181-11.html"
                    } else {
                        "30181-05.html"
                    }
                    .to_string(),
                );
            }
            return Some("30929-01.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
