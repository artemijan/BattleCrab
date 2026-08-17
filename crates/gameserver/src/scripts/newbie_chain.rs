//! The level 2–20 newbie chain's shared skeleton (`Q10993`–`Q11023`).
//!
//! Thirty quests across five race lines run the same script: a starter NPC
//! points you at a second one, who hands over a notes item and names the first
//! monster group; each group drops one quest item until you have enough, and
//! the last group sends you back for a choice of two reward bundles. Only the
//! ids, counts, chances and html names differ.
//!
//! Rather than thirty copies of the same `on_kill`/`on_talk` walk, the shape
//! lives here once and each quest file is the table that fills it in. Every
//! per-quest oddity still lives in that quest's own file — this module holds
//! only what is genuinely identical, and each field below names the Java
//! expression it stands for so a reader never has to guess which variant a
//! given quest picked.
//!
//! Two variants of the kill stage exist in Java and both are represented:
//!
//! - Most stages guard the drop with `getQuestItemsCount(killer, item) < need`
//!   and roll `getRandom(100) < chance`, so the count stops at `need`.
//! - A few (`Q11013` and its siblings) omit **both**: every kill drops, and
//!   nothing stops the count. [`Stage::capped`] picks between them.
//!
//! On *this dist* the two are indistinguishable, and the reason is worth
//! writing down so nobody "simplifies" the flag away: every uncapped stage
//! here waits on a single item, so the kill that reaches the requirement also
//! advances the cond, and the stage stops being live before an extra drop can
//! happen. The flag would only become visible on an uncapped stage whose
//! `advance_when` names a *second* item — the two-drop shape `Q11001` uses —
//! and no quest currently combines the two. It is modelled because it is
//! Java's structure, not because it changes anything today.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

/// `showOnScreenMsg(..., ExShowScreenMessage.TOP_CENTER, 10000)` — the banner
/// every step of the chain uses.
pub const MSG_POSITION: i32 = 2;
pub const MSG_TIME: i32 = 10_000;

/// One hunting stage: a monster group, the item it drops, and the cond it
/// advances to once the requirement is met.
pub struct Stage {
    /// The `case` labels of one `onKill` branch.
    pub monsters: &'static [i32],
    /// `qs.isCond(n)` — the stage is only live at this cond.
    pub cond: i32,
    pub item: i32,
    /// The `< need` cap and the `>= need` advance threshold.
    pub need: i64,
    /// `getRandom(100) < chance`. 100 means the roll always passes, and the
    /// roll is still consumed — kept so the RNG stream matches Java's.
    pub chance: i32,
    /// Whether Java guards the drop with `getQuestItemsCount(...) < need`.
    /// `false` lets the count run past the requirement — unobservable on this
    /// dist's data; see the module note for why.
    pub capped: bool,
    pub next_cond: i32,
    /// The `NpcStringId` banner shown when the stage completes.
    pub msg: i32,
    /// Everything the advance waits on, as `(item, count)`. Usually just this
    /// stage's own item; the two-drop stages list both, and **each of their
    /// branches waits for the pair** — that is what stops a player who
    /// finished one half from being stuck at the shared cond.
    pub advance_when: &'static [(i32, i64)],
}

/// A reward branch: what the turn-in takes, what it gives, and its page.
pub struct Reward {
    /// The `Quest <name> <event>` bypass the html button carries.
    pub event: &'static str,
    /// `qs.isCond(n)` — the turn-in only pays at the final cond.
    pub cond: i32,
    /// `takeItems`, in Java's order. A count of -1 is "all of them".
    pub take: &'static [(i32, i64)],
    pub give: &'static [(i32, i64)],
    pub exp: i64,
    pub sp: i64,
    pub html: &'static str,
}

/// Run every stage against one kill. Call from `on_kill`.
pub fn run_stages(ctx: &mut QuestCtx, stages: &[Stage]) {
    if !ctx.has_qs() {
        return;
    }
    let npc_id = ctx.npc_id;
    for stage in stages {
        if !stage.monsters.contains(&npc_id) || !ctx.is_cond(stage.cond) {
            continue;
        }
        if stage.capped && ctx.quest_items_count(stage.item) >= stage.need {
            return;
        }
        // Java rolls even at chance 100, so the draw happens either way.
        if ctx.roll(100) >= stage.chance {
            return;
        }
        ctx.give_items(stage.item, 1);
        ctx.play_sound(quest_sounds::MIDDLE);
        if stage
            .advance_when
            .iter()
            .all(|&(item, need)| ctx.quest_items_count(item) >= need)
        {
            ctx.send_screen_message_npc_string(stage.msg, MSG_POSITION, MSG_TIME);
            // Java's stage advances pass `setCond(n)` without the sound flag —
            // the `playSound` above already fired for this kill.
            ctx.set_cond(stage.next_cond, false);
        }
        return;
    }
}

/// Pay out one reward branch. Returns the page, or `None` when the player is
/// not at the branch's cond (Java's `if (qs.isCond(n))` with no else).
pub fn pay(ctx: &mut QuestCtx, reward: &Reward) -> Option<String> {
    if !ctx.is_cond(reward.cond) {
        return None;
    }
    for &(item, count) in reward.take {
        ctx.take_items(item, count);
    }
    for &(item, count) in reward.give {
        ctx.give_items(item, count);
    }
    if reward.exp > 0 || reward.sp > 0 {
        ctx.add_exp_and_sp(reward.exp, reward.sp);
    }
    ctx.exit_quest(false, true);
    Some(reward.html.to_string())
}

/// The `addCondLevel(min, max, …)` + `addCondRace(…, …)` pair every quest in
/// the chain registers, in Java's registration order — level first, so an
/// under-level character of the wrong race is told about the level.
pub fn level_and_race_gate(
    ctx: &mut QuestCtx,
    levels: std::ops::RangeInclusive<i32>,
    race: i32,
) -> Option<String> {
    if !levels.contains(&ctx.player_level()) {
        return Some("no-level.html".to_string());
    }
    if ctx.player_race() != race {
        return Some("no-race.html".to_string());
    }
    None
}

/// The chain's races, as `Player.race` ordinals.
pub const HUMAN: i32 = 0;
pub const ELF: i32 = 1;
pub const DARK_ELF: i32 = 2;
pub const ORC: i32 = 3;
pub const DWARF: i32 = 4;

/// The talk skeleton: CREATED → the starter's page, COMPLETED → the
/// already-done message, STARTED → whatever the quest's own table says.
///
/// `started` is consulted as `(npc_id, cond)`. A miss falls through to the
/// no-quest message, which is Java's `htmltext` default.
pub fn talk(
    ctx: &mut QuestCtx,
    created_npc: i32,
    created_html: &str,
    started: &[(i32, i32, &str)],
) -> Option<String> {
    // Java `getQuestState(talker, true)` — the first click materialises the
    // CREATED state the start button then starts.
    ctx.ensure_qs();
    if ctx.is_created() {
        return (ctx.npc_id == created_npc).then(|| created_html.to_string());
    }
    if ctx.is_completed() {
        return Some(ctx.already_completed_html());
    }
    if ctx.is_started() {
        for &(npc, cond, html) in started {
            if ctx.npc_id == npc && ctx.is_cond(cond) {
                return Some(html.to_string());
            }
        }
    }
    Some(ctx.no_quest_html())
}

/// A quest in the chain, as a table. [`Chain`] implements [`QuestScript`] so a
/// quest file is its constants plus one of these.
pub struct Chain {
    pub id: i32,
    pub name: &'static str,
    pub html_dir: &'static str,
    pub start_npcs: &'static [i32],
    pub talk_npcs: &'static [i32],
    pub kill_npcs: &'static [i32],
    pub quest_items: &'static [i32],
    pub levels: (i32, i32),
    pub race: i32,
    /// `addCondCompletedQuest(<class>, <html>)`, when the quest has one.
    pub requires: Option<(&'static str, &'static str)>,
    /// The event that calls `startQuest`, and whatever it does besides —
    /// `Some((cond, notes_item, msg))` for the one-NPC quests that brief you
    /// on the spot, `None` for the two-NPC ones that send you on first.
    pub start_event: &'static str,
    pub start_brief: Option<(i32, i32, i32)>,
    /// Plain "show this page" events (the html navigation buttons).
    pub plain_events: &'static [&'static str],
    /// The second NPC's briefing, as `(npc, cond, html, next_cond, notes, msg)`.
    pub brief: Option<(i32, i32, &'static str, i32, i32, i32)>,
    pub created_html: &'static str,
    /// `(npc_id, cond, html)` rows for the STARTED state.
    pub started_html: &'static [(i32, i32, &'static str)],
    pub stages: &'static [Stage],
    pub rewards: &'static [Reward],
}

impl QuestScript for Chain {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn html_dir(&self) -> &'static str {
        self.html_dir
    }
    fn start_npcs(&self) -> &[i32] {
        self.start_npcs
    }
    fn talk_npcs(&self) -> &[i32] {
        self.talk_npcs
    }
    fn kill_npcs(&self) -> &[i32] {
        self.kill_npcs
    }
    fn quest_items(&self) -> &[i32] {
        self.quest_items
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if let Some(html) = level_and_race_gate(ctx, self.levels.0..=self.levels.1, self.race) {
            return Some(html);
        }
        // `addCondCompletedQuest` — registered after the level/race pair, so it
        // is the last to be consulted.
        match self.requires {
            Some((quest, html)) if !ctx.other_quest_completed(quest) => Some(html.to_string()),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        // The second NPC's briefing is a *talk*, not a button: reaching them at
        // the right cond hands over the notes and advances the quest.
        if let Some((npc, cond, html, next, notes, msg)) = self.brief {
            ctx.ensure_qs();
            if ctx.is_started() && ctx.npc_id == npc && ctx.is_cond(cond) {
                ctx.set_cond(next, true);
                ctx.send_screen_message_npc_string(msg, MSG_POSITION, MSG_TIME);
                ctx.give_items(notes, 1);
                return Some(html.to_string());
            }
        }
        talk(
            ctx,
            self.start_npcs[0],
            self.created_html,
            self.started_html,
        )
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        if event == self.start_event {
            ctx.start_quest();
            // The one-NPC quests do the whole briefing here, because there is
            // no second NPC to walk to.
            if let Some((cond, notes, msg)) = self.start_brief {
                ctx.set_cond(cond, false);
                ctx.send_screen_message_npc_string(msg, MSG_POSITION, MSG_TIME);
                ctx.give_items(notes, 1);
            }
            return Some(event.to_string());
        }
        if self.plain_events.contains(&event) {
            return Some(event.to_string());
        }
        self.rewards
            .iter()
            .find(|r| r.event == event)
            .and_then(|r| pay(ctx, r))
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        run_stages(ctx, self.stages);
    }
}

/// The chain's five capstones (`Q10993`, `Q10994`, `Q11006`, `Q11012`,
/// `Q11018` — "Future \<race\>").
///
/// These have no monsters at all: the starter NPC offers the race's class
/// paths, accepting one sets a cond, and the matching trainer hands over the
/// reward. `Capstone` is their table.
///
/// The trainer pages carry Java's `getClassId() != <class>` guard, which reads
/// backwards until you notice these run *before* the first class transfer: the
/// player is still a Fighter or Mage, so the guard passes, and its only real
/// effect is to hide the page from someone who somehow already transferred.
pub struct Capstone {
    pub id: i32,
    pub name: &'static str,
    pub html_dir: &'static str,
    pub start_npcs: &'static [i32],
    pub talk_npcs: &'static [i32],
    pub min_level: i32,
    pub race: i32,
    pub requires: (&'static str, &'static str),
    /// Pages the html buttons just navigate to.
    pub plain_events: &'static [&'static str],
    /// `(event, cond)` — accepting a class path starts the quest and books it.
    pub accepts: &'static [(&'static str, i32)],
    /// `(npc, forbidden_class, cond, html)` — the trainer's page.
    pub trainers: &'static [(i32, i32, i32, &'static str)],
    /// `(class_id, html)` for the CREATED state: the fighter and mage offers.
    pub created: &'static [(i32, &'static str)],
    /// The starter's page once the quest is running (`getCond() >= 1`).
    pub started_html: Option<(i32, &'static str)>,
    /// The events that pay out, and what they give.
    pub finish_events: &'static [&'static str],
    pub finish_give: &'static [(i32, i64)],
}

impl QuestScript for Capstone {
    fn id(&self) -> i32 {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn html_dir(&self) -> &'static str {
        self.html_dir
    }
    fn start_npcs(&self) -> &[i32] {
        self.start_npcs
    }
    fn talk_npcs(&self) -> &[i32] {
        self.talk_npcs
    }

    fn start_condition_html(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.player_level() < self.min_level {
            return Some("no-level.html".to_string());
        }
        if ctx.player_race() != self.race {
            return Some("no-race.html".to_string());
        }
        let (quest, html) = self.requires;
        (!ctx.other_quest_completed(quest)).then(|| html.to_string())
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            let class_id = ctx.player_class_id();
            // Java tests the starter NPC only on the *first* arm; the mage arm
            // is an `else if` on the class alone. Reproduced.
            if let Some(&(_, html)) = self
                .created
                .first()
                .filter(|(c, _)| ctx.npc_id == self.start_npcs[0] && class_id == *c)
            {
                return Some(html.to_string());
            }
            return self
                .created
                .iter()
                .skip(1)
                .find(|(c, _)| class_id == *c)
                .map(|(_, html)| html.to_string());
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        if ctx.is_started() {
            if let Some((npc, html)) = self.started_html
                && ctx.npc_id == npc
                && ctx.cond() >= 1
            {
                return Some(html.to_string());
            }
            let class_id = ctx.player_class_id();
            for &(npc, forbidden, cond, html) in self.trainers {
                if ctx.npc_id == npc && class_id != forbidden && ctx.is_cond(cond) {
                    return Some(html.to_string());
                }
            }
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        if let Some(&(_, cond)) = self.accepts.iter().find(|(e, _)| *e == event) {
            ctx.start_quest();
            ctx.set_cond(cond, true);
            return Some(event.to_string());
        }
        if self.finish_events.contains(&event) {
            // `if (qs.getCond() > 1)` — cond 1 is "started but no path picked",
            // which no accept leaves you in, so this is really "has a path".
            if ctx.cond() <= 1 {
                return None;
            }
            for &(item, count) in self.finish_give {
                ctx.give_items(item, count);
            }
            ctx.exit_quest(false, true);
            return Some(event.to_string());
        }
        self.plain_events
            .contains(&event)
            .then(|| event.to_string())
    }
}
