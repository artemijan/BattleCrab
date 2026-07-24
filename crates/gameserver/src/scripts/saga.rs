//! The shared 3rd-class **Saga** engine — the Rust counterpart of L2J's
//! `SagasSuperClass`, restored to authentic Interlude (the datapack ships the
//! off-chronicle L2 Classic versions, which reference a reward item that does
//! not exist in this dist). Each of the 31 Saga quests (Q70..Q100) is a thin
//! [`SagaData`] table over this one generic engine.
//!
//! The 20-condition ladder threads the player through twelve NPCs (`npc[0..11]`
//! — quest-giver, intermediaries, four Tablets of Vision, a battle companion),
//! trading quest items (`items[0..11]`, `items[10]` the starter), farming
//! Guardian Angels and Archon minions, and slaying three scripted spawns
//! (`mob[0..2]`) before the quest-giver performs the class transfer.
//!
//! The finale is fully wired: the boss and companion spawn and duel each other,
//! trade opening lines, the companion keeps up a timed battle-banter cadence
//! (Java's repeating `Mob_2` taunt timers, driven by [`SagaQuest::on_timer`]),
//! and the boss is driven off after 15 player hits (unlocking the reward).
//! Progression glows (`MagicSkillUse` 4546) and the transform flash (4339) fire
//! on each tablet step and the class change. The chatter lines are one generic
//! set across all 31 Sagas rather than the per-class `_text` arrays.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

/// Per-class data for one Saga quest.
pub struct SagaData {
    pub id: i32,
    pub name: &'static str,
    pub html_dir: &'static str,
    /// Twelve NPC ids (`_npc`): 0 quest-giver, 1/2 intermediaries, 3 the
    /// Divine-Stone checker, 4 the battle companion, 5..8 & 11 tablet/guides,
    /// 9/10 the finale guides.
    pub npc: [i32; 12],
    /// Twelve item ids (`_items`); `items[10]` is the starter, `items[3]` the
    /// Halisha mark currency, `items[8]`/`items[9]` the archon/battle rewards.
    pub items: [i32; 12],
    /// Three scripted spawns (`_mob`): 0 the cond-8 knight, 1 Archon Hellisha,
    /// 2 the finale boss.
    pub mob: [i32; 3],
    /// Target 3rd-class id and the required 2nd-class id.
    pub class_id: i32,
    pub prev_class: i32,
    /// Spawn points for `mob[0]`, `mob[2]`, `npc[4]`.
    pub spawn: [(i32, i32, i32); 3],
}

// Shared across every Saga (verbatim from SagasSuperClass).
const REWARD_EXP: i64 = 2_299_404;
const REWARD_ADENA: i64 = 5_000_000;
const MARK_OF_SAGA: i32 = 6622;
const ARCHON_HALISHA_NORM: [i32; 5] = [18212, 18214, 18215, 18216, 18218];
const MIN_LEVEL: i32 = 76;
// Finale battle chatter. The authentic `_text` lines are quest-specific (18
// per Saga); one generic set serves all 31, in keeping with the shared htmls.
const BOSS_TAUNT: &str = "So, another has come to be broken. You are no different from the rest!";
const COMPANION_CALL: &str = "Steel yourself — I will stand with you against this thing!";
const BOSS_RETREAT: &str = "Impossible... I cannot... I must withdraw!";
/// The companion's timed battle-banter during the finale duel — cycled by
/// [`SagaQuest::on_timer`] on a fixed cadence while the boss still stands
/// (Java's `Mob_2` taunt timers 1-3). The cadence stops the moment the fight
/// ends (cond leaves 17) or the boss is driven off (`Tab` set by `on_attack`).
const COMPANION_TAUNTS: [&str; 3] = [
    "Hold your ground — its strength wanes!",
    "Strike now, while it staggers!",
    "We have it reeling — do not relent!",
];
/// Timer key + cadence for the companion's banter: the first line lands a few
/// seconds into the duel, then every 12s until the boss retreats.
const TAUNT_TIMER: &str = "SagaTaunt";
const TAUNT_FIRST_MS: u64 = 4_000;
const TAUNT_EVERY_MS: u64 = 12_000;

pub struct SagaQuest {
    data: SagaData,
    kill_ids: Vec<i32>,
}

impl SagaQuest {
    pub fn new(data: SagaData) -> Self {
        // The three scripted spawns + the shared Guardian Angel / Archon ranges.
        let mut kill_ids: Vec<i32> = data.mob.to_vec();
        kill_ids.extend(21646..=21651); // Archon minions
        kill_ids.extend(ARCHON_HALISHA_NORM);
        kill_ids.extend(27214..=27216); // Guardian Angels
        Self { data, kill_ids }
    }

    fn npc(&self, i: usize) -> i32 {
        self.data.npc[i]
    }
    fn item(&self, i: usize) -> i32 {
        self.data.items[i]
    }
    fn mob(&self, i: usize) -> i32 {
        self.data.mob[i]
    }

    /// The final class-transfer + reward, shared by the "0-2" event and the
    /// cond-20 recovery path in `on_talk`.
    fn finish(&self, ctx: &mut QuestCtx) {
        ctx.exit_quest(false, false);
        ctx.set_var("cond", "0");
        ctx.take_items(self.item(10), -1);
        ctx.add_exp_and_sp(REWARD_EXP, 0);
        ctx.give_items(57, REWARD_ADENA);
        ctx.give_items(MARK_OF_SAGA, 1);
        ctx.set_class_id(self.data.class_id);
        ctx.cast_visual(4339, 1); // the transform flash
                                  // TODO(saga): the SkillTransfer "givePormanders" hand-off.
    }
}

impl QuestScript for SagaQuest {
    fn id(&self) -> i32 {
        self.data.id
    }
    fn name(&self) -> &'static str {
        self.data.name
    }
    fn html_dir(&self) -> &'static str {
        // All 31 Sagas share one generic, `%questname%`-templated html set (the
        // authentic per-class htmls would be ~1000 files). `data.html_dir` is
        // kept for reference / a future per-quest override.
        let _ = self.data.html_dir;
        "quests/_SagaShared"
    }
    fn start_npcs(&self) -> &[i32] {
        std::slice::from_ref(&self.data.npc[0])
    }
    fn talk_npcs(&self) -> &[i32] {
        &self.data.npc
    }
    fn kill_npcs(&self) -> &[i32] {
        &self.kill_ids
    }
    fn attack_npcs(&self) -> &[i32] {
        std::slice::from_ref(&self.data.mob[2])
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "0-011.htm" | "0-012.htm" | "0-013.htm" | "0-014.htm" | "0-015.htm" | "3-07.htm"
            | "4-010.htm" => Some(event.to_string()),
            "accept" => {
                ctx.set_var("cond", "1");
                ctx.start_quest();
                ctx.play_sound(quest_sounds::ACCEPT);
                ctx.give_items(self.item(10), 1);
                Some("0-03.htm".to_string())
            }
            "0-1" => {
                if ctx.player_level() < MIN_LEVEL {
                    Some("0-02.htm".to_string())
                } else {
                    Some("0-05.htm".to_string())
                }
            }
            "0-2" => {
                if ctx.player_level() >= MIN_LEVEL {
                    self.finish(ctx);
                    Some("0-07.htm".to_string())
                } else {
                    ctx.take_items(self.item(10), -1);
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.set_var("cond", "20");
                    Some("0-08.htm".to_string())
                }
            }
            "1-3" => {
                ctx.set_var("cond", "3");
                Some("1-05.htm".to_string())
            }
            "1-4" => {
                ctx.set_var("cond", "4");
                ctx.take_items(self.item(0), 1);
                if self.item(11) != 0 {
                    ctx.take_items(self.item(11), 1);
                }
                ctx.give_items(self.item(1), 1);
                Some("1-06.htm".to_string())
            }
            "2-1" => {
                ctx.set_var("cond", "2");
                Some("2-05.htm".to_string())
            }
            "2-2" => {
                ctx.set_var("cond", "5");
                ctx.take_items(self.item(1), 1);
                ctx.give_items(self.item(4), 1);
                Some("2-06.htm".to_string())
            }
            "3-6" => {
                ctx.set_var("cond", "11");
                Some("3-02.htm".to_string())
            }
            "3-7" => {
                ctx.set_var("cond", "12");
                Some("3-03.htm".to_string())
            }
            "3-8" => {
                ctx.set_var("cond", "13");
                ctx.take_items(self.item(2), 1);
                ctx.give_items(self.item(7), 1);
                Some("3-08.htm".to_string())
            }
            "4-2" | "4-3" => {
                ctx.give_items(self.item(9), 1);
                ctx.set_var("cond", "18");
                ctx.play_sound(quest_sounds::MIDDLE);
                Some("4-011.htm".to_string())
            }
            "5-1" => self.progress(ctx, "6", 4, "5-02.htm"),
            "6-1" => self.progress(ctx, "8", 5, "6-03.htm"),
            "7-2" => self.progress(ctx, "10", 6, "7-06.htm"),
            "8-1" => self.progress(ctx, "14", 7, "8-02.htm"),
            "9-1" => self.progress(ctx, "17", 8, "9-03.htm"),
            "10-2" => self.progress(ctx, "19", 9, "10-06.htm"),
            "7-1" => {
                if ctx.npc_var_int("spawned") == 1 {
                    Some("7-03.htm".to_string())
                } else {
                    let (x, y, z) = self.data.spawn[0];
                    ctx.spawn_attacker_at(self.mob(0), x, y, z);
                    ctx.set_npc_var_int("spawned", 1);
                    Some("7-02.htm".to_string())
                }
            }
            "10-1" => {
                let (bx, by, bz) = self.data.spawn[1];
                let (cx, cy, cz) = self.data.spawn[2];
                // The finale boss (hostile) and the companion (neutral, talked
                // to for the reward once the boss is driven off).
                let boss = ctx.spawn_attacker_at(self.mob(2), bx, by, bz);
                let _ = (cx, cy, cz); // companion spawns beside the guide (talkable)
                let companion = ctx.spawn_near_npc(self.npc(4), false);
                // Choreography: the companion and boss set upon each other and
                // trade opening lines, then the companion keeps up a timed
                // battle-banter cadence (`on_timer`) until the boss retreats.
                if let (Some(b), Some(c)) = (boss, companion) {
                    ctx.seed_npc_attack(b, c);
                    ctx.seed_npc_attack(c, b);
                    ctx.broadcast_npc_text(b, BOSS_TAUNT);
                    ctx.broadcast_npc_text(c, COMPANION_CALL);
                    ctx.set_var("SagaCompanion", c.to_string()); // whom the cadence speaks from
                    ctx.set_var("SagaTauntIdx", "0");
                    ctx.start_quest_timer(TAUNT_TIMER, TAUNT_FIRST_MS);
                }
                ctx.set_var("Quest0", "1"); // the boss's hit counter
                ctx.set_var("Tab", "0"); // set once the boss retreats
                Some("10-02.htm".to_string())
            }
            "11-9" => {
                ctx.set_var("cond", "15");
                Some("11-03.htm".to_string())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let npc_id = ctx.npc_id;
        let cond = ctx.cond();
        // Archon minions → the Halisha mark (cond 15).
        if (21646..=21651).contains(&npc_id) {
            if cond == 15 {
                ctx.give_items(self.item(3), 1);
            }
            return;
        }
        // Archon Halisha (fixed spawns) → the archon reward, cond 16.
        if ARCHON_HALISHA_NORM.contains(&npc_id) {
            if cond == 15 {
                ctx.give_items(self.item(8), 1);
                ctx.take_items(self.item(3), -1);
                ctx.set_var("cond", "16");
                ctx.play_sound(quest_sounds::MIDDLE);
            }
            return;
        }
        // Guardian Angels → 10 kills, then the cond-6→7 item.
        if (27214..=27216).contains(&npc_id) {
            if cond == 6 {
                let kills = ctx.get_int("kills");
                if kills < 9 {
                    ctx.set_var("kills", (kills + 1).to_string());
                } else {
                    ctx.play_sound(quest_sounds::MIDDLE);
                    ctx.give_items(self.item(5), 1);
                    ctx.set_var("cond", "7");
                }
            }
            return;
        }
        // mob[0] (cond 8) → its reward, cond 9.
        if npc_id == self.mob(0) && cond == 8 {
            ctx.give_items(self.item(6), 1);
            ctx.set_var("cond", "9");
            ctx.play_sound(quest_sounds::MIDDLE);
            return;
        }
        // mob[1] = Archon Hellisha (cond 15) → the archon reward, cond 16.
        if npc_id == self.mob(1) && cond == 15 {
            ctx.give_items(self.item(8), 1);
            ctx.take_items(self.item(3), -1);
            ctx.set_var("cond", "16");
            ctx.play_sound(quest_sounds::MIDDLE);
        }
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        // The finale boss (mob[2]) is never killed — it is driven off. Java
        // counts hits in `Quest0`; after the 15th the boss retreats (despawns)
        // and `Tab` is set, unlocking the companion's reward.
        if !ctx.has_qs() || ctx.cond() != 17 || ctx.npc_id != self.mob(2) {
            return;
        }
        let hits = ctx.get_int("Quest0") + 1;
        ctx.set_var("Quest0", hits.to_string());
        if hits > 15 {
            ctx.set_var("Quest0", "1");
            ctx.set_var("Tab", "1");
            ctx.npc_say_text(BOSS_RETREAT); // the boss's parting cry
            ctx.delete_npc(); // the boss retreats
        }
    }

    fn on_timer(&self, ctx: &mut QuestCtx, name: &str) {
        // The companion's finale battle-banter, from Java's repeating `Mob_2`
        // taunt timers. It runs only while the duel is live (cond 17) and the
        // boss still stands (`Tab` unset), then reschedules itself; when the
        // player advances (cond leaves 17) or the boss retreats, the next
        // firing sees the gate closed and the cadence lapses.
        if name != TAUNT_TIMER {
            return;
        }
        if !ctx.has_qs() || ctx.cond() != 17 || ctx.get_int("Tab") == 1 {
            return;
        }
        let companion = ctx.get_int("SagaCompanion");
        let idx = ctx
            .get_int("SagaTauntIdx")
            .rem_euclid(COMPANION_TAUNTS.len() as i32);
        ctx.broadcast_npc_text(companion, COMPANION_TAUNTS[idx as usize]);
        ctx.set_var("SagaTauntIdx", (idx + 1).to_string());
        ctx.start_quest_timer(TAUNT_TIMER, TAUNT_EVERY_MS);
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc_id = ctx.npc_id;
        if ctx.is_completed() {
            if npc_id == self.npc(0) {
                return Some(
                    "<html><body>You have already completed this quest!</body></html>".to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        // The whole ladder is gated on the player still being the 2nd class.
        if ctx.player_class_id() != self.data.prev_class {
            return Some(ctx.no_quest_html());
        }
        let cond = ctx.cond();
        let n = |i: usize| self.npc(i);
        let html: Option<&str> = match cond {
            0 => (npc_id == n(0)).then_some("0-01.htm"),
            1 => {
                if npc_id == n(0) {
                    Some("0-04.htm")
                } else if npc_id == n(2) {
                    Some("2-01.htm")
                } else {
                    None
                }
            }
            2 => {
                if npc_id == n(2) {
                    Some("2-02.htm")
                } else if npc_id == n(1) {
                    Some("1-01.htm")
                } else {
                    None
                }
            }
            3 => {
                if npc_id == n(1) && ctx.quest_items_count(self.item(0)) != 0 {
                    if self.item(11) == 0 || ctx.quest_items_count(self.item(11)) != 0 {
                        Some("1-03.htm")
                    } else {
                        Some("1-02.htm")
                    }
                } else {
                    None
                }
            }
            4 => {
                if npc_id == n(1) {
                    Some("1-04.htm")
                } else if npc_id == n(2) {
                    Some("2-03.htm")
                } else {
                    None
                }
            }
            5 => {
                if npc_id == n(2) {
                    Some("2-04.htm")
                } else if npc_id == n(5) {
                    Some("5-01.htm")
                } else {
                    None
                }
            }
            6 => {
                if npc_id == n(5) {
                    Some("5-03.htm")
                } else if npc_id == n(6) {
                    Some("6-01.htm")
                } else {
                    None
                }
            }
            7 => (npc_id == n(6)).then_some("6-02.htm"),
            8 => {
                if npc_id == n(6) {
                    Some("6-04.htm")
                } else if npc_id == n(7) {
                    Some("7-01.htm")
                } else {
                    None
                }
            }
            9 => (npc_id == n(7)).then_some("7-05.htm"),
            10 => {
                if npc_id == n(7) {
                    Some("7-07.htm")
                } else if npc_id == n(3) {
                    Some("3-01.htm")
                } else {
                    None
                }
            }
            11 | 12 => (npc_id == n(3)).then_some(if ctx.quest_items_count(self.item(2)) > 0 {
                "3-05.htm"
            } else {
                "3-04.htm"
            }),
            13 => {
                if npc_id == n(3) {
                    Some("3-06.htm")
                } else if npc_id == n(8) {
                    Some("8-01.htm")
                } else {
                    None
                }
            }
            14 => {
                if npc_id == n(8) {
                    Some("8-03.htm")
                } else if npc_id == n(11) {
                    Some("11-01.htm")
                } else {
                    None
                }
            }
            15 => {
                if npc_id == n(11) {
                    Some("11-02.htm")
                } else if npc_id == n(9) {
                    Some("9-01.htm")
                } else {
                    None
                }
            }
            16 => (npc_id == n(9)).then_some("9-02.htm"),
            17 => {
                if npc_id == n(9) {
                    Some("9-04.htm")
                } else if npc_id == n(10) {
                    Some("10-01.htm")
                } else if npc_id == n(4) {
                    // The companion offers the reward only once the boss has
                    // been driven off (Tab set by `on_attack`); otherwise it
                    // urges the player back into the fight.
                    if ctx.get_int("Tab") == 1 {
                        Some("4-010.htm")
                    } else {
                        Some("10-02.htm")
                    }
                } else {
                    None
                }
            }
            18 => (npc_id == n(10)).then_some("10-05.htm"),
            19 => {
                if npc_id == n(10) {
                    Some("10-07.htm")
                } else if npc_id == n(0) {
                    Some("0-06.htm")
                } else {
                    None
                }
            }
            20 => {
                if npc_id == n(0) {
                    if ctx.player_level() >= MIN_LEVEL {
                        self.finish(ctx);
                        Some("0-09.htm")
                    } else {
                        Some("0-010.htm")
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        Some(
            html.map(str::to_string)
                .unwrap_or_else(|| ctx.no_quest_html()),
        )
    }
}

impl SagaQuest {
    /// The recurring "hand the tablet its item, glow, advance" step.
    fn progress(
        &self,
        ctx: &mut QuestCtx,
        cond: &str,
        take_item: usize,
        html: &str,
    ) -> Option<String> {
        ctx.set_var("cond", cond.to_string());
        ctx.take_items(self.item(take_item), 1);
        ctx.cast_visual(4546, 1); // the tablet's progression glow
        ctx.play_sound(quest_sounds::MIDDLE);
        Some(html.to_string())
    }
}
