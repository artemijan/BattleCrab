//! Path Of The Elven Oracle (409) — port of
//! `dist/game/data/scripts/quests/Q00409_PathOfTheElvenOracle/`.
//!
//! Awards the **Leaf of Oracle** (1235), the third of `ElfHumanWizardChange1`'s
//! four proofs.
//!
//! The first quest in the port that **spawns its own monsters**. Allana's
//! re-enactment and Perrin's Tamil are ambushes conjured next to the NPC you
//! are talking to and set on you, not wandering spawns you go and find. That
//! needed three new framework pieces, all added with this slice:
//! [`QuestCtx::memo_state`]/`set_memo_state`, and
//! [`QuestCtx::spawn_attacker`] (Java's `addSpawn` + `addAttackPlayerDesire`).
//!
//! ## `memoState` is a second progress axis, and it is not `cond`
//!
//! Java drives this quest on **both**: `cond` for the client's quest window,
//! `memoState` for the script's own bookkeeping (stored as the quest variable
//! `memoState`, never displayed). They move independently and sometimes in
//! opposite directions — talking to Manuel empty-handed while `memoState == 2`
//! *rewinds* it to 1 and pushes `cond` to 8. Collapsing the two into one
//! counter would break the re-enactment's restart path, which is exactly what
//! `memoState` exists to track.
//!
//! ## The ambush tag differs from the one in `quest_common`
//!
//! Quests 401/403 gate their drops on "right weapon, one attacker". This one
//! gates on **one attacker only** — no weapon check — and keys the variable
//! `firstAttacker` rather than `lastAttacker`. Same 0 → 1 → 2 shape, different
//! predicate, so it is written out here rather than forced through
//! [`crate::scripts::quest_common`]; sharing would have silently added a
//! weapon requirement this quest does not have.
//!
//! Each ambusher also shouts on the first hit — the lizardmen a war cry, Tamil
//! an "as you wish, master" — and the warrior alone has a death line.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const PRIEST_MANUEL: i32 = 30293;
const ALLANA: i32 = 30424;
const PERRIN: i32 = 30428;

const CRYSTAL_MEDALLION: i32 = 1231;
const SWINDLERS_MONEY: i32 = 1232;
const ALLANA_OF_DAIRY: i32 = 1233;
const LIZARD_CAPTAIN_ORDER: i32 = 1234;
const LEAF_OF_ORACLE: i32 = 1235;
const HALF_OF_DAIRY: i32 = 1236;
const TAMIL_NECKLACE: i32 = 1275;

const LIZARDMAN_WARRIOR: i32 = 27032;
const LIZARDMAN_SCOUT: i32 = 27033;
const LIZARDMAN_SOLDIER: i32 = 27034;
const TAMIL: i32 = 27035;

/// "The sacred flame is ours!"
const NS_SACRED_FLAME: i32 = 40909;
/// "Arrghh...we shall never.. surrender..."
const NS_NEVER_SURRENDER: i32 = 40910;
/// "As you wish, master!"
const NS_AS_YOU_WISH: i32 = 40913;

const ELVEN_MAGE: i32 = 25;
const ORACLE: i32 = 29;
const MIN_LEVEL: i32 = 19;

/// Java's `npc.getVariables()` key — note it is `firstAttacker` here, not the
/// `lastAttacker` used by quests 401/403.
const FIRST_ATTACKER: &str = "firstAttacker";

const AMBUSHERS: [i32; 4] = [LIZARDMAN_WARRIOR, LIZARDMAN_SCOUT, LIZARDMAN_SOLDIER, TAMIL];

const QUEST_ITEMS: [i32; 6] = [
    CRYSTAL_MEDALLION,
    SWINDLERS_MONEY,
    ALLANA_OF_DAIRY,
    LIZARD_CAPTAIN_ORDER,
    HALF_OF_DAIRY,
    TAMIL_NECKLACE,
];

pub struct Q00409PathOfTheElvenOracle;

impl Q00409PathOfTheElvenOracle {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    /// `hasAtLeastOneQuestItem(...)`.
    fn has_any(&self, ctx: &QuestCtx, items: &[i32]) -> bool {
        items.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }
}

impl QuestScript for Q00409PathOfTheElvenOracle {
    fn id(&self) -> i32 {
        409
    }
    fn name(&self) -> &'static str {
        "Q00409_PathOfTheElvenOracle"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00409_PathOfTheElvenOracle"
    }
    fn start_npcs(&self) -> &[i32] {
        &[PRIEST_MANUEL]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[PRIEST_MANUEL, ALLANA, PERRIN]
    }
    fn kill_npcs(&self) -> &[i32] {
        &AMBUSHERS
    }
    fn attack_npcs(&self) -> &[i32] {
        &AMBUSHERS
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == PRIEST_MANUEL {
                return Some(
                    if self.has(ctx, LEAF_OF_ORACLE) {
                        "30293-04.htm"
                    } else {
                        "30293-01.htm"
                    }
                    .to_string(),
                );
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        if !self.has(ctx, CRYSTAL_MEDALLION) {
            return Some(ctx.no_quest_html());
        }
        match npc {
            PRIEST_MANUEL => self.talk_manuel(ctx),
            ALLANA => self.talk_allana(ctx),
            PERRIN => self.talk_perrin(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                ELVEN_MAGE if ctx.player_level() < MIN_LEVEL => "30293-03.htm".to_string(),
                ELVEN_MAGE if self.has(ctx, LEAF_OF_ORACLE) => "30293-04.htm".to_string(),
                ELVEN_MAGE => {
                    ctx.start_quest();
                    ctx.set_memo_state(1);
                    ctx.give_items(CRYSTAL_MEDALLION, 1);
                    "30293-05.htm".to_string()
                }
                ORACLE => "30293-02a.htm".to_string(),
                _ => "30293-02.htm".to_string(),
            }),
            "30424-08.html" | "30424-09.html" => Some(event.to_string()),
            "30424-07.html" => (ctx.memo_state() == 1).then(|| event.to_string()),
            // Allana's re-enactment: three lizardmen jump the player.
            "replay_1" => {
                ctx.set_memo_state(2);
                for id in [LIZARDMAN_WARRIOR, LIZARDMAN_SCOUT, LIZARDMAN_SOLDIER] {
                    ctx.spawn_attacker(id, true);
                }
                None // Java returns no html for this event
            }
            "30428-02.html" | "30428-03.html" => (ctx.memo_state() == 2).then(|| event.to_string()),
            // Perrin sets Tamil on the player.
            "replay_2" => {
                if ctx.memo_state() == 2 {
                    ctx.set_memo_state(3);
                    ctx.spawn_attacker(TAMIL, true);
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() || !ctx.is_started() || ctx.npc_script_value() != 1 {
            return;
        }
        match ctx.npc_id {
            LIZARDMAN_WARRIOR | LIZARDMAN_SCOUT | LIZARDMAN_SOLDIER => {
                if self.has(ctx, LIZARD_CAPTAIN_ORDER) {
                    return;
                }
                // Only the warrior has a death line.
                if ctx.npc_id == LIZARDMAN_WARRIOR {
                    ctx.npc_say(NS_NEVER_SURRENDER);
                }
                ctx.give_items(LIZARD_CAPTAIN_ORDER, 1);
                ctx.set_cond(3, true);
            }
            TAMIL if !self.has(ctx, TAMIL_NECKLACE) => {
                ctx.give_items(TAMIL_NECKLACE, 1);
                ctx.set_cond(5, true);
            }
            _ => {}
        }
    }

    /// Java's ambush tag: **one attacker, no weapon requirement**. State 0
    /// shouts and claims the mob; state 1 drops to 2 if anyone else joins in.
    fn on_attack(&self, ctx: &mut QuestCtx) {
        if !ctx.has_qs() {
            return;
        }
        match ctx.npc_script_value() {
            0 => {
                let line = if ctx.npc_id == TAMIL {
                    NS_AS_YOU_WISH
                } else {
                    NS_SACRED_FLAME
                };
                ctx.npc_say(line);
                ctx.set_npc_script_value(1);
                let attacker = ctx.player;
                ctx.set_npc_var_int(FIRST_ATTACKER, attacker);
            }
            1 if ctx.npc_var_int(FIRST_ATTACKER) != ctx.player => {
                ctx.set_npc_script_value(2);
            }
            _ => {}
        }
    }
}

impl Q00409PathOfTheElvenOracle {
    fn talk_manuel(&self, ctx: &mut QuestCtx) -> Option<String> {
        let carrying = self.has_any(
            ctx,
            &[
                SWINDLERS_MONEY,
                ALLANA_OF_DAIRY,
                LIZARD_CAPTAIN_ORDER,
                HALF_OF_DAIRY,
            ],
        );
        if !carrying {
            // Empty-handed. `memoState == 2` means the re-enactment was
            // started and lost — rewind it and advance the *window* to 8.
            if ctx.memo_state() == 2 {
                ctx.set_memo_state(1);
                // Java uses the single-arg `setCond(8)` here — no middle sound,
                // unlike every other cond change in this quest.
                ctx.set_cond(8, false);
                return Some("30293-09.html".to_string());
            }
            ctx.set_memo_state(1);
            return Some("30293-06.html".to_string());
        }
        if self.has(ctx, SWINDLERS_MONEY)
            && self.has(ctx, ALLANA_OF_DAIRY)
            && self.has(ctx, LIZARD_CAPTAIN_ORDER)
        {
            if self.has(ctx, HALF_OF_DAIRY) {
                // Java falls through with no page here.
                return None;
            }
            ctx.give_items(LEAF_OF_ORACLE, 1);
            // Java's three-way level branch awards identical exp/sp.
            ctx.add_exp_and_sp(80314, 5087);
            ctx.exit_quest(false, true);
            ctx.social_action(3);
            return Some("30293-08.html".to_string());
        }
        Some("30293-07.html".to_string())
    }

    fn talk_allana(&self, ctx: &mut QuestCtx) -> Option<String> {
        let money = self.has(ctx, SWINDLERS_MONEY);
        let dairy = self.has(ctx, ALLANA_OF_DAIRY);
        let order = self.has(ctx, LIZARD_CAPTAIN_ORDER);
        let half = self.has(ctx, HALF_OF_DAIRY);
        if !money && !dairy && !order && !half {
            return match ctx.memo_state() {
                2 => Some("30424-05.html".to_string()),
                1 => {
                    ctx.set_cond(2, true);
                    Some("30424-01.html".to_string())
                }
                _ => Some(ctx.no_quest_html()),
            };
        }
        if !money && !dairy && !half && order {
            ctx.set_memo_state(2);
            ctx.give_items(HALF_OF_DAIRY, 1);
            ctx.set_cond(4, true);
            return Some("30424-02.html".to_string());
        }
        if !money && !dairy && order && half {
            // Tamil was conjured but got away: rewind to the re-enactment.
            if ctx.memo_state() == 3 && !self.has(ctx, TAMIL_NECKLACE) {
                ctx.set_memo_state(2);
                ctx.set_cond(4, true);
                return Some("30424-06.html".to_string());
            }
            return Some("30424-03.html".to_string());
        }
        if money && order && half && !dairy {
            ctx.give_items(ALLANA_OF_DAIRY, 1);
            ctx.take_items(HALF_OF_DAIRY, 1);
            ctx.set_cond(9, true);
            return Some("30424-04.html".to_string());
        }
        if money && order && dairy {
            ctx.set_cond(7, true);
            return Some("30424-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_perrin(&self, ctx: &mut QuestCtx) -> Option<String> {
        if !self.has(ctx, LIZARD_CAPTAIN_ORDER) || !self.has(ctx, HALF_OF_DAIRY) {
            return Some(ctx.no_quest_html());
        }
        if self.has(ctx, TAMIL_NECKLACE) {
            ctx.give_items(SWINDLERS_MONEY, 1);
            ctx.take_items(TAMIL_NECKLACE, 1);
            ctx.set_cond(6, true);
            return Some("30428-04.html".to_string());
        }
        if self.has(ctx, SWINDLERS_MONEY) {
            return Some("30428-05.html".to_string());
        }
        if ctx.memo_state() == 3 {
            return Some("30428-06.html".to_string());
        }
        Some("30428-01.html".to_string())
    }
}
