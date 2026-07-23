//! Path Of The Orc Shaman (416) — port of
//! `dist/game/data/scripts/quests/Q00416_PathOfTheOrcShaman/`.
//!
//! Awards the **Mask of Medium** (1631), `OrcChange1`'s third and last proof.
//! **The Orc first-occupation tier is complete with this.**
//!
//! A long single-track chain: fire charm → three trophies → Hestui mask →
//! totem spirit claw → Tataru's letter → flame charm → grizzly blood → blood
//! cauldron → spirit net → a conjured Durka Spirit → totem spirit blood →
//! the mask.
//!
//! ## `ItemChanceHolder.count` is repurposed as a *cond selector*
//!
//! The drop table looks ordinary and is not. Java gates each entry with
//!
//! ```java
//! final ItemChanceHolder item = MOBS.get(npc.getId());
//! if (item.getCount() == qs.getCond())
//! ```
//!
//! so the holder's **count** field carries the cond in which that mob is live
//! (1, 6 or 9), while **chance** is a 0..1 probability handed to
//! `giveItemRandomly`. Read `count` as a quantity — its normal meaning, and
//! what it means in quests 403/406 — and grizzly bears would drop six bloods
//! a kill while the cond gate vanished entirely.
//!
//! That is the **fourth** distinct reading of this one type across the Path
//! family: `/100` (404, 406, 408), `/10` (401, 403), `== 0` equality (412),
//! and now count-as-cond.
//!
//! ## Two summon meters that differ in one crucial way
//!
//! Like quest 414's green blood, the Durka parasites are an escalating summon
//! meter rather than loot — 5 parasites gives a 1-in-10 chance, 6 and 7 give
//! 2-in-10, and 8 is certain; on success the stack is wiped and a Durka Spirit
//! appears. But **Java does not set this one on the player**: `addSpawn` here
//! has no `attackPlayer` call, unlike 414. So it is
//! [`QuestCtx::spawn_near_npc`] (added for this) rather than `spawn_attacker`
//! — aggroing it would be an invention.
//!
//! Killing the spirit then falls through the same `else` and pays the bound
//! spirit, consuming both the parasites and the net.
//!
//! ## Deviations and dead weight
//!
//! - Java selects the quest state with `getRandomPartyMemberState(player, -1,
//!   3, npc)`. The port has no party-aware selection; as in
//!   `q00303_collect_arrowheads` this reduces to the killer, which is the same
//!   thing solo. TODO(G13+): revisit when quest party support lands.
//! - The accept event is **`START`**, not the `ACCEPT` every other Path quest
//!   uses.
//! - `cond 10` is never assigned — the chain jumps 9 → 11.
//! - The whole `memoState` 100–110 branch (Black Leopard, NPCs 31979 / 32057 /
//!   32090) is **dead at both ends**, the third Orc-tier quest in a row: its
//!   only entry is `30585-14.html`, which no page offers, and none of those
//!   three NPCs is registered anywhere. Both of Java's `NpcSay` lines with a
//!   player-name parameter live inside it, which is why this port needs no
//!   string-parameter support in `NpcSay`. Ported as `TODO(dead)` stubs are
//!   *not* included: unlike 414/415 the dead branch here is large and touches
//!   a packet feature we don't have, so it is deliberately **omitted** and
//!   documented rather than half-ported.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

const UMOS: i32 = 30502;
const TATARU_ZU_HESTUI: i32 = 30585;
const HESTUI_TOTEM_SPIRIT: i32 = 30592;
const DUDA_MARA_TOTEM_SPIRIT: i32 = 30593;

const FIRE_CHARM: i32 = 1616;
const KASHA_BEAR_PELT: i32 = 1617;
const KASHA_BLADE_SPIDER_HUSK: i32 = 1618;
const FIRST_FIERY_EGG: i32 = 1619;
const HESTUI_MASK: i32 = 1620;
const SECOND_FIERY_EGG: i32 = 1621;
const TOTEM_SPIRIT_CLAW: i32 = 1622;
const TATARUS_LETTER: i32 = 1623;
const FLAME_CHARM: i32 = 1624;
const GRIZZLY_BLOOD: i32 = 1625;
const BLOOD_CAULDRON: i32 = 1626;
const SPIRIT_NET: i32 = 1627;
const BOUND_DURKA_SPIRIT: i32 = 1628;
const DURKA_PARASITE: i32 = 1629;
const TOTEM_SPIRIT_BLOOD: i32 = 1630;
const MASK_OF_MEDIUM: i32 = 1631;

const SCARLET_SALAMANDER: i32 = 20415;
const KASHA_BLADE_SPIDER: i32 = 20478;
const KASHA_BEAR: i32 = 20479;
const GRIZZLY_BEAR: i32 = 20335;
const POISON_SPIDER: i32 = 20038;
const BIND_POISON_SPIDER: i32 = 20043;
const DURKA_SPIRIT: i32 = 27056;

const ORC_MAGE: i32 = 49;
const ORC_SHAMAN: i32 = 50;
const MIN_LEVEL: i32 = 19;

const GRIZZLY_BLOOD_NEEDED: i64 = 3;

/// `(mob, item, cond in which it drops)` — the third field is Java's
/// `ItemChanceHolder.count`, used as a cond selector, not a quantity.
const MOBS: [(i32, i32, i32); 6] = [
    (SCARLET_SALAMANDER, FIRST_FIERY_EGG, 1),
    (KASHA_BLADE_SPIDER, KASHA_BLADE_SPIDER_HUSK, 1),
    (KASHA_BEAR, KASHA_BEAR_PELT, 1),
    (GRIZZLY_BEAR, GRIZZLY_BLOOD, 6),
    (POISON_SPIDER, DURKA_PARASITE, 9),
    (BIND_POISON_SPIDER, DURKA_PARASITE, 9),
];

const FIRST_TROPHIES: [i32; 3] = [FIRST_FIERY_EGG, KASHA_BLADE_SPIDER_HUSK, KASHA_BEAR_PELT];

const KILL_NPCS: [i32; 7] = [
    SCARLET_SALAMANDER,
    KASHA_BLADE_SPIDER,
    KASHA_BEAR,
    GRIZZLY_BEAR,
    POISON_SPIDER,
    BIND_POISON_SPIDER,
    DURKA_SPIRIT,
];

const QUEST_ITEMS: [i32; 15] = [
    FIRE_CHARM,
    KASHA_BEAR_PELT,
    KASHA_BLADE_SPIDER_HUSK,
    FIRST_FIERY_EGG,
    HESTUI_MASK,
    SECOND_FIERY_EGG,
    TOTEM_SPIRIT_CLAW,
    TATARUS_LETTER,
    FLAME_CHARM,
    GRIZZLY_BLOOD,
    BLOOD_CAULDRON,
    SPIRIT_NET,
    BOUND_DURKA_SPIRIT,
    DURKA_PARASITE,
    TOTEM_SPIRIT_BLOOD,
];

pub struct Q00416PathOfTheOrcShaman;

impl Q00416PathOfTheOrcShaman {
    fn has(&self, ctx: &QuestCtx, item: i32) -> bool {
        ctx.quest_items_count(item) > 0
    }
    fn has_any(&self, ctx: &QuestCtx, items: &[i32]) -> bool {
        items.iter().any(|id| ctx.quest_items_count(*id) > 0)
    }

    /// Java's parasite escalation: 5 → 1-in-10, 6 and 7 → 2-in-10, 8+ certain.
    fn durka_spirit_appears(&self, ctx: &mut QuestCtx) -> bool {
        let count = ctx.quest_items_count(DURKA_PARASITE);
        if count >= 8 {
            return true;
        }
        let roll = ctx.roll(10);
        match count {
            5 => roll < 1,
            6 | 7 => roll < 2,
            _ => false,
        }
    }
}

impl QuestScript for Q00416PathOfTheOrcShaman {
    fn id(&self) -> i32 {
        416
    }
    fn name(&self) -> &'static str {
        "Q00416_PathOfTheOrcShaman"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00416_PathOfTheOrcShaman"
    }
    fn start_npcs(&self) -> &[i32] {
        &[TATARU_ZU_HESTUI]
    }
    /// 31979 / 32057 / 32090 are deliberately absent — see the module header.
    fn talk_npcs(&self) -> &[i32] {
        &[
            TATARU_ZU_HESTUI,
            UMOS,
            DUDA_MARA_TOTEM_SPIRIT,
            HESTUI_TOTEM_SPIRIT,
        ]
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
        match event {
            // Not `ACCEPT` — this quest alone names it `START`.
            "START" => Some(
                match ctx.player_class_id() {
                    ORC_MAGE if ctx.player_level() < MIN_LEVEL => "30585-04.htm",
                    ORC_MAGE if self.has(ctx, MASK_OF_MEDIUM) => "30585-05.htm",
                    ORC_MAGE => "30585-06.htm",
                    ORC_SHAMAN => "30585-02.htm",
                    _ => "30585-03.htm",
                }
                .to_string(),
            ),
            "30585-07.htm" => {
                ctx.start_quest();
                ctx.set_memo_state(1);
                ctx.give_items(FIRE_CHARM, 1);
                Some(event.to_string())
            }
            "30585-12.html" => self.has(ctx, TOTEM_SPIRIT_CLAW).then(|| event.to_string()),
            "30585-13.html" => {
                if self.has(ctx, TOTEM_SPIRIT_CLAW) {
                    ctx.take_items(TOTEM_SPIRIT_CLAW, -1);
                    ctx.give_items(TATARUS_LETTER, 1);
                    ctx.set_cond(5, true);
                    return Some(event.to_string());
                }
                None
            }
            "30592-02.html" => (self.has(ctx, HESTUI_MASK) && self.has(ctx, SECOND_FIERY_EGG))
                .then(|| event.to_string()),
            "30592-03.html" => {
                if self.has(ctx, HESTUI_MASK) && self.has(ctx, SECOND_FIERY_EGG) {
                    ctx.take_items(HESTUI_MASK, -1);
                    ctx.take_items(SECOND_FIERY_EGG, -1);
                    ctx.give_items(TOTEM_SPIRIT_CLAW, 1);
                    ctx.set_cond(4, true);
                    return Some(event.to_string());
                }
                None
            }
            "30593-02.html" => self.has(ctx, BLOOD_CAULDRON).then(|| event.to_string()),
            "30593-03.html" => {
                if self.has(ctx, BLOOD_CAULDRON) {
                    ctx.take_items(BLOOD_CAULDRON, -1);
                    ctx.give_items(SPIRIT_NET, 1);
                    ctx.set_cond(9, true);
                    return Some(event.to_string());
                }
                None
            }
            // The finish.
            "30502-07.html" => {
                if self.has(ctx, TOTEM_SPIRIT_BLOOD) {
                    ctx.take_items(TOTEM_SPIRIT_BLOOD, -1);
                    ctx.give_items(MASK_OF_MEDIUM, 1);
                    // Java's three-way level branch awards identical exp/sp.
                    ctx.add_exp_and_sp(80314, 5087);
                    ctx.exit_quest(false, true);
                    ctx.social_action(3);
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
        let npc_id = ctx.npc_id;
        let cond = ctx.cond();

        // Killing the conjured spirit ends the parasite stage.
        if npc_id == DURKA_SPIRIT {
            if cond == 9
                && self.has(ctx, SPIRIT_NET)
                && !self.has(ctx, BOUND_DURKA_SPIRIT)
                && ctx.quest_items_count(DURKA_PARASITE) <= 8
            {
                ctx.give_items(BOUND_DURKA_SPIRIT, 1);
                ctx.take_items(DURKA_PARASITE, -1);
                ctx.take_items(SPIRIT_NET, -1);
            }
            return;
        }

        // `item.getCount() == qs.getCond()` — count is the cond gate.
        let Some(&(_, item, gate)) = MOBS.iter().find(|(mob, ..)| *mob == npc_id) else {
            return;
        };
        if gate != cond {
            return;
        }
        match cond {
            1 if self.has(ctx, FIRE_CHARM) => {
                // chance 1.0, limit 1 — one of each trophy.
                if ctx.give_item_randomly(item, 1, 1, 1.0, true)
                    && FIRST_TROPHIES
                        .iter()
                        .all(|id| ctx.quest_items_count(*id) > 0)
                {
                    ctx.set_cond(2, true);
                }
            }
            6 if self.has(ctx, FLAME_CHARM) => {
                if ctx.give_item_randomly(item, 1, GRIZZLY_BLOOD_NEEDED, 1.0, true) {
                    ctx.set_cond(7, false); // Java's single-arg setCond
                }
            }
            9 if self.has(ctx, SPIRIT_NET)
                && !self.has(ctx, BOUND_DURKA_SPIRIT)
                && ctx.quest_items_count(DURKA_PARASITE) <= 8 =>
            {
                if self.durka_spirit_appears(ctx) {
                    ctx.take_items(DURKA_PARASITE, -1);
                    // Conjured, but NOT set on the player — see the header.
                    ctx.spawn_near_npc(DURKA_SPIRIT, true);
                    ctx.play_sound(quest_sounds::BEFORE_BATTLE);
                } else {
                    ctx.give_items(DURKA_PARASITE, 1);
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
            if npc == TATARU_ZU_HESTUI {
                return Some("30585-01.htm".to_string());
            }
            return Some(ctx.no_quest_html());
        }
        if !ctx.is_started() || ctx.memo_state() != 1 {
            // `memoState == 100` would open the dead branch; it is unreachable.
            return Some(ctx.no_quest_html());
        }
        match npc {
            TATARU_ZU_HESTUI => self.talk_tataru(ctx),
            UMOS => self.talk_umos(ctx),
            DUDA_MARA_TOTEM_SPIRIT => self.talk_duda_mara(ctx),
            HESTUI_TOTEM_SPIRIT => self.talk_hestui(ctx),
            _ => Some(ctx.no_quest_html()),
        }
    }
}

impl Q00416PathOfTheOrcShaman {
    fn talk_tataru(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, FIRE_CHARM) {
            let trophies: i64 = FIRST_TROPHIES
                .iter()
                .map(|id| ctx.quest_items_count(*id))
                .sum();
            if trophies < 3 {
                return Some("30585-08.html".to_string());
            }
            ctx.take_items(FIRE_CHARM, -1);
            for id in FIRST_TROPHIES {
                ctx.take_items(id, -1);
            }
            ctx.give_items(HESTUI_MASK, 1);
            ctx.give_items(SECOND_FIERY_EGG, 1);
            ctx.set_cond(3, true);
            return Some("30585-09.html".to_string());
        }
        if self.has(ctx, HESTUI_MASK) && self.has(ctx, SECOND_FIERY_EGG) {
            return Some("30585-10.html".to_string());
        }
        if self.has(ctx, TOTEM_SPIRIT_CLAW) {
            return Some("30585-11.html".to_string());
        }
        if self.has(ctx, TATARUS_LETTER) {
            return Some("30585-15.html".to_string());
        }
        if self.has_any(
            ctx,
            &[
                GRIZZLY_BLOOD,
                FLAME_CHARM,
                BLOOD_CAULDRON,
                SPIRIT_NET,
                BOUND_DURKA_SPIRIT,
                TOTEM_SPIRIT_BLOOD,
            ],
        ) {
            return Some("30585-16.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_umos(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, TATARUS_LETTER) {
            ctx.give_items(FLAME_CHARM, 1);
            ctx.take_items(TATARUS_LETTER, -1);
            ctx.set_cond(6, true);
            return Some("30502-01.html".to_string());
        }
        if self.has(ctx, FLAME_CHARM) {
            if ctx.quest_items_count(GRIZZLY_BLOOD) < GRIZZLY_BLOOD_NEEDED {
                return Some("30502-02.html".to_string());
            }
            ctx.take_items(FLAME_CHARM, -1);
            ctx.take_items(GRIZZLY_BLOOD, -1);
            ctx.give_items(BLOOD_CAULDRON, 1);
            ctx.set_cond(8, true);
            return Some("30502-03.html".to_string());
        }
        if self.has(ctx, BLOOD_CAULDRON) {
            return Some("30502-04.html".to_string());
        }
        if self.has_any(ctx, &[BOUND_DURKA_SPIRIT, SPIRIT_NET]) {
            return Some("30502-05.html".to_string());
        }
        if self.has(ctx, TOTEM_SPIRIT_BLOOD) {
            return Some("30502-06.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_duda_mara(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, BLOOD_CAULDRON) {
            return Some("30593-01.html".to_string());
        }
        let net = self.has(ctx, SPIRIT_NET);
        let bound = self.has(ctx, BOUND_DURKA_SPIRIT);
        if net && !bound {
            return Some("30593-04.html".to_string());
        }
        if !net && bound {
            ctx.take_items(BOUND_DURKA_SPIRIT, -1);
            ctx.give_items(TOTEM_SPIRIT_BLOOD, 1);
            // Java jumps 9 → 11; cond 10 is never used.
            ctx.set_cond(11, true);
            return Some("30593-05.html".to_string());
        }
        if self.has(ctx, TOTEM_SPIRIT_BLOOD) {
            return Some("30593-06.html".to_string());
        }
        Some(ctx.no_quest_html())
    }

    fn talk_hestui(&self, ctx: &mut QuestCtx) -> Option<String> {
        if self.has(ctx, HESTUI_MASK) && self.has(ctx, SECOND_FIERY_EGG) {
            return Some("30592-01.html".to_string());
        }
        if self.has(ctx, TOTEM_SPIRIT_CLAW) {
            return Some("30592-04.html".to_string());
        }
        if self.has_any(
            ctx,
            &[
                GRIZZLY_BLOOD,
                FLAME_CHARM,
                BLOOD_CAULDRON,
                SPIRIT_NET,
                BOUND_DURKA_SPIRIT,
                TOTEM_SPIRIT_BLOOD,
                TATARUS_LETTER,
            ],
        ) {
            return Some("30592-05.html".to_string());
        }
        Some(ctx.no_quest_html())
    }
}
