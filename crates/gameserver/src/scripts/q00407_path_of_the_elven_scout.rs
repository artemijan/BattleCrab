//! Path Of The Elven Scout (407) — port of
//! `dist/game/data/scripts/quests/Q00407_PathOfTheElvenScout/`.
//!
//! The first-occupation quest awarding **Reisa's Recommendation** (1217), the
//! proof `ElfHumanFighterChange1` consumes to make an Elven Fighter an Elven
//! Scout. Like its Elven Knight sibling, the proof had no source in the port
//! before this.
//!
//! A four-NPC courier chain: Reoria → Moretti → Prias → back to Moretti →
//! Reoria, driven almost entirely by which letter you are carrying rather than
//! by `cond` (cond only trails the item state for the client's quest window).
//!
//! **The tag mechanic is the interesting part.** `onAttack` stamps the mob's
//! script value with the attacker's object id, and `onKill` pays out only if
//! the killer matches — so a mob someone *else* softened up drops nothing for
//! you. Both hooks are needed; porting only `onKill` would make every kill
//! fail the check, and porting only `onAttack` would leak the tag. There is a
//! test for the mismatch case.
//!
//! Page extensions are mixed exactly as in the sibling quest: `.htm` for the
//! pre-accept dialog, `.html` after. Prias ships `-01`, `-02` and `-04` but no
//! `-03`, and Java never names one — asserted in the page test so the gap is
//! not "filled in".

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const MASTER_REORIA: i32 = 30328;
const GUARD_BABENCO: i32 = 30334;
const GUARD_MORETTI: i32 = 30337;
const PRIAS: i32 = 30426;

const REISAS_LETTER: i32 = 1207;
const TORN_LETTERS: [i32; 4] = [1208, 1209, 1210, 1211];
const MORETTIES_HERB: i32 = 1212;
const MORETTIS_LETTER: i32 = 1214;
const PRIASS_LETTER: i32 = 1215;
const HONORARY_GUARD: i32 = 1216;
const REISAS_RECOMMENDATION: i32 = 1217;
const RUSTED_KEY: i32 = 1293;

const OL_MAHUM_PATROL: i32 = 20053;
/// A quest monster, not a normal spawn.
const OL_MAHUM_SENTRY: i32 = 27031;

const ELVEN_FIGHTER: i32 = 18;
const ELVEN_SCOUT: i32 = 22;
const MIN_LEVEL: i32 = 19;

/// Java's `qs.set("variable", 1)` — set once Moretti's chain is underway.
const VARIABLE: &str = "variable";

const QUEST_ITEMS: [i32; 10] = [
    REISAS_LETTER, 1208, 1209, 1210, 1211, MORETTIES_HERB, MORETTIS_LETTER, PRIASS_LETTER,
    HONORARY_GUARD, RUSTED_KEY,
];

pub struct Q00407PathOfTheElvenScout;

impl Q00407PathOfTheElvenScout {
    fn letter_count(&self, ctx: &QuestCtx) -> i64 {
        TORN_LETTERS.iter().map(|id| ctx.quest_items_count(*id)).sum()
    }

    /// Java's `giveLetterAndCheckState`: the fourth letter advances the cond,
    /// the others just chime.
    fn give_letter(&self, ctx: &mut QuestCtx, letter_id: i32) {
        ctx.give_items(letter_id, 1);
        if self.letter_count(ctx) >= 4 {
            ctx.set_cond(3, true);
        } else {
            ctx.play_sound(quest_sounds::ITEMGET);
        }
    }
}

impl QuestScript for Q00407PathOfTheElvenScout {
    fn id(&self) -> i32 {
        407
    }
    fn name(&self) -> &'static str {
        "Q00407_PathOfTheElvenScout"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00407_PathOfTheElvenScout"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MASTER_REORIA]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[MASTER_REORIA, GUARD_BABENCO, GUARD_MORETTI, PRIAS]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[OL_MAHUM_PATROL, OL_MAHUM_SENTRY]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[OL_MAHUM_PATROL, OL_MAHUM_SENTRY]
    }
    fn quest_items(&self) -> &[i32] {
        &QUEST_ITEMS
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "ACCEPT" => Some(match ctx.player_class_id() {
                ELVEN_FIGHTER => {
                    if ctx.player_level() < MIN_LEVEL {
                        "30328-03.htm".to_string()
                    } else if ctx.quest_items_count(REISAS_RECOMMENDATION) > 0 {
                        "30328-04.htm".to_string()
                    } else {
                        ctx.start_quest();
                        ctx.unset(VARIABLE);
                        ctx.give_items(REISAS_LETTER, 1);
                        "30328-05.htm".to_string()
                    }
                }
                ELVEN_SCOUT => "30328-02a.htm".to_string(),
                _ => "30328-02.htm".to_string(),
            }),
            "30337-02.html" => Some(event.to_string()),
            "30337-03.html" => {
                // Only advances if Reisa's letter is still in hand.
                if ctx.quest_items_count(REISAS_LETTER) > 0 {
                    ctx.take_items(REISAS_LETTER, -1);
                    ctx.set_var(VARIABLE, "1");
                    ctx.set_cond(2, true);
                    return Some(event.to_string());
                }
                None
            }
            _ => None,
        }
    }

    /// `npc.setScriptValue(attacker.getObjectId())` — claim the mob.
    fn on_attack(&self, ctx: &mut QuestCtx) {
        if ctx.has_qs() && ctx.is_started() {
            let player = ctx.player;
            ctx.set_npc_script_value(player);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Only the player who tagged it in `on_attack` is paid.
        if ctx.npc_script_value() != ctx.player || !ctx.has_qs() {
            return;
        }
        if ctx.npc_id == OL_MAHUM_SENTRY {
            // 60% for the key, and only while carrying Moretti's herb+letter.
            if ctx.is_cond(5)
                && ctx.roll(10) < 6
                && ctx.quest_items_count(MORETTIES_HERB) > 0
                && ctx.quest_items_count(MORETTIS_LETTER) > 0
                && ctx.quest_items_count(RUSTED_KEY) == 0
            {
                ctx.give_items(RUSTED_KEY, 1);
                ctx.set_cond(6, true);
            }
            return;
        }
        if !ctx.is_cond(2) {
            return;
        }
        // The four torn letters drop in a fixed order, one per kill.
        if let Some(&next) = TORN_LETTERS.iter().find(|id| ctx.quest_items_count(**id) == 0) {
            self.give_letter(ctx, next);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc = ctx.npc_id;
        if ctx.is_created() {
            if npc == MASTER_REORIA {
                return Some("30328-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() {
            return Some(ctx.no_quest_html());
        }
        let variable = ctx.get_int(VARIABLE);
        match npc {
            // Java orders these letter / variable / honorary-guard; the guard
            // branch is hoisted above the `variable` one here, which is safe
            // because Java's `variable` branch is itself guarded by
            // `!hasAtLeastOneQuestItem(REISAS_LETTER, HONORARY_GUARD)` — the
            // two are mutually exclusive either way.
            MASTER_REORIA => {
                if ctx.quest_items_count(REISAS_LETTER) > 0 {
                    return Some("30328-06.html".to_string());
                }
                if ctx.quest_items_count(HONORARY_GUARD) > 0 {
                    ctx.take_items(HONORARY_GUARD, -1);
                    ctx.give_items(REISAS_RECOMMENDATION, 1);
                    // Same three-way level branch as quest 406, same values.
                    ctx.add_exp_and_sp(80314, 5087);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
                    return Some("30328-07.html".to_string());
                }
                if variable == 1 {
                    return Some("30328-08.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            GUARD_BABENCO => {
                if variable == 1 {
                    return Some("30334-01.html".to_string());
                }
                Some(ctx.no_quest_html())
            }
            GUARD_MORETTI => self.talk_moretti(ctx, variable),
            PRIAS => self.talk_prias(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00407PathOfTheElvenScout {
    fn talk_moretti(&self, ctx: &mut QuestCtx, variable: i32) -> Option<String> {
        let letters = self.letter_count(ctx);
        if ctx.quest_items_count(REISAS_LETTER) > 0 && letters == 0 {
            return Some("30337-01.html".to_string());
        }
        let mid_chain = ctx.quest_items_count(MORETTIS_LETTER) == 0
            && ctx.quest_items_count(PRIASS_LETTER) == 0
            && ctx.quest_items_count(HONORARY_GUARD) == 0;
        if variable == 1 && mid_chain {
            return Some(
                match letters {
                    0 => "30337-04.html",
                    1..=3 => "30337-05.html",
                    _ => {
                        for id in TORN_LETTERS {
                            ctx.take_items(id, -1);
                        }
                        ctx.give_items(MORETTIES_HERB, 1);
                        ctx.give_items(MORETTIS_LETTER, 1);
                        ctx.set_cond(4, true);
                        "30337-06.html"
                    }
                }
                .to_string(),
            );
        }
        if ctx.quest_items_count(PRIASS_LETTER) > 0 {
            ctx.take_items(PRIASS_LETTER, -1);
            ctx.give_items(HONORARY_GUARD, 1);
            ctx.set_cond(8, true);
            return Some("30337-07.html".to_string());
        }
        if ctx.quest_items_count(MORETTIES_HERB) > 0 && ctx.quest_items_count(MORETTIS_LETTER) > 0 {
            return Some("30337-09.html".to_string());
        }
        if ctx.quest_items_count(HONORARY_GUARD) > 0 {
            return Some("30337-08.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_prias(&self, ctx: &mut QuestCtx) -> Option<String> {
        if ctx.quest_items_count(MORETTIS_LETTER) > 0 && ctx.quest_items_count(MORETTIES_HERB) > 0 {
            if ctx.quest_items_count(RUSTED_KEY) == 0 {
                ctx.set_cond(5, true);
                return Some("30426-01.html".to_string());
            }
            ctx.take_items(RUSTED_KEY, -1);
            ctx.take_items(MORETTIES_HERB, -1);
            ctx.take_items(MORETTIS_LETTER, -1);
            ctx.give_items(PRIASS_LETTER, 1);
            ctx.set_cond(7, true);
            return Some("30426-02.html".to_string());
        }
        if ctx.quest_items_count(PRIASS_LETTER) > 0 {
            return Some("30426-04.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
