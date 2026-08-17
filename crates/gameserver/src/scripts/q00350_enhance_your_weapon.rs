//! Enhance Your Weapon (350) — the **Soul Crystal** system. Talk to one of
//! three Wise Men (Rolento / Silvia / a Grand Master) at level 40+, take a
//! Red / Green / Blue **Soul Crystal** (stage 0), and level it by killing
//! monsters: each eligible kill has a chance to raise the crystal one stage,
//! which is how weapons get their Special Ability (SA) soul-crystal fuel.
//!
//! Two absorb modes, from `data/LevelUpCrystalData.xml` (see
//! [`SoulCrystalData`](crate::data::SoulCrystalData)):
//!   * plain mobs — just carry a crystal and land the kill;
//!   * `skill="true"` mobs (the large majority) — you must first cast the
//!     **Soul Crystal** skill (2096) on the mob while it is at ≤ half HP
//!     (`onSkillSee` → the mob's absorber list), then kill it.
//!
//! Party absorb modes (FULL_PARTY / PARTY_ONE_RANDOM / PARTY_RANDOM) collapse to
//! the killer here, matching the project-wide `onKill` party deviation.
//!
//! The success/fail/refuse flavour SystemMessages are sent. Java's fourth,
//! "the crystal broke", is unreachable here: Q350's only `exchangeCrystal`
//! call passes `broke = false`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::sm_ids;

const STARTING_NPCS: [i32; 3] = [30115, 30856, 30194];
const RED_SOUL_CRYSTAL0: i32 = 4629;
const GREEN_SOUL_CRYSTAL0: i32 = 4640;
const BLUE_SOUL_CRYSTAL0: i32 = 4651;
const SOUL_CRYSTAL_SKILL: i32 = 2096;
const MIN_LEVEL: i32 = 40;

pub struct Q00350EnhanceYourWeapon {
    /// Every monster that can level a crystal (`addKillId` + `addSkillSeeId`),
    /// sourced from `LevelUpCrystalData.xml` at boot rather than a static table.
    leveling_npc_ids: Vec<i32>,
}

impl Q00350EnhanceYourWeapon {
    pub fn new(leveling_npc_ids: Vec<i32>) -> Self {
        Self { leveling_npc_ids }
    }

    /// Java `check`: does the player hold any Soul Crystal in the contiguous
    /// stage block (4629..=4664)? Faithfully narrow — a higher-stage crystal
    /// isn't matched, exactly as in the Java.
    fn has_any_crystal(&self, ctx: &QuestCtx) -> bool {
        (4629..4665).any(|i| ctx.quest_items_count(i) > 0)
    }

    /// Java `levelSoulCrystals` (solo path): the killer's single crystal has a
    /// chance to level, gated by the mob's skill/absorb requirement.
    fn level_soul_crystals(&self, ctx: &mut QuestCtx) {
        // `getSCForPlayer` returns null unless the quest is started and the
        // player carries exactly one crystal.
        if !ctx.has_qs() || !ctx.is_started() {
            return;
        }
        let Some(sc_item) = ctx.single_soul_crystal() else {
            return;
        };
        // Copy the data out before the mutable give/take borrows.
        let (leveled_item_id, main_info, level_info) = {
            let scd = ctx.soul_crystal_data();
            let Some(crystal) = scd.crystal(sc_item).copied() else {
                return;
            };
            let npc_levels = scd.npc_levels(ctx.npc_id);
            // `maxSCLevel`: the crystal's own level if this mob levels it, else 0.
            let max_sc_level = if npc_levels.is_some_and(|m| m.contains_key(&crystal.level)) {
                crystal.level
            } else {
                0
            };
            (
                crystal.leveled_item_id,
                scd.leveling_info(ctx.npc_id, max_sc_level).copied(),
                scd.leveling_info(ctx.npc_id, crystal.level).copied(),
            )
        };

        // `mainlvlInfo == null` → the mob can't level anything here.
        let Some(main_info) = main_info else {
            return;
        };
        // Skill-absorb mobs: fail unless the killer absorbed below half HP.
        if main_info.skill_needed && !ctx.killer_absorbed_below_half() {
            return;
        }
        // `levelCrystal`: only if the mob actually levels *this* crystal's stage.
        let Some(level_info) = level_info else {
            ctx.send_sm(sm_ids::THE_SOUL_CRYSTAL_IS_REFUSING_TO_ABSORB_THE_SOUL);
            return;
        };
        if ctx.roll(100) <= level_info.chance {
            ctx.take_items(sc_item, 1);
            // Java's `exchangeCrystal` sends the flavour line *before* the
            // `YOU_HAVE_EARNED_S1` + `InventoryUpdate` pair that `give_items`
            // already emits, so this ordering matches the wire.
            //
            // Only the success leg is reachable: Q350's single call site passes
            // `broke = false`, so `..._BROKE_BECAUSE_IT_WAS_NOT_ABLE_TO_ENDURE_
            // THE_SOUL_ENERGY` never fires on this dist.
            ctx.send_sm(sm_ids::THE_SOUL_CRYSTAL_SUCCEEDED_IN_ABSORBING_A_SOUL);
            ctx.give_items(leveled_item_id, 1);
        } else {
            ctx.send_sm(sm_ids::THE_SOUL_CRYSTAL_WAS_NOT_ABLE_TO_ABSORB_THE_SOUL);
        }
    }
}

impl QuestScript for Q00350EnhanceYourWeapon {
    fn id(&self) -> i32 {
        350
    }
    fn name(&self) -> &'static str {
        "Q00350_EnhanceYourWeapon"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00350_EnhanceYourWeapon"
    }
    fn start_npcs(&self) -> &[i32] {
        &STARTING_NPCS
    }
    fn talk_npcs(&self) -> &[i32] {
        &STARTING_NPCS
    }
    fn kill_npcs(&self) -> &[i32] {
        &self.leveling_npc_ids
    }
    fn skill_see_npcs(&self) -> &[i32] {
        &self.leveling_npc_ids
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        let npc_id = ctx.npc_id;
        if ctx.is_created() {
            return Some(if ctx.player_level() < MIN_LEVEL {
                format!("{npc_id}-lvl.htm")
            } else {
                format!("{npc_id}-01.htm")
            });
        }
        // Already carrying a crystal → the "come back when it's leveled" page.
        if self.has_any_crystal(ctx) {
            return Some(format!("{npc_id}-03.htm"));
        }
        // Started but holding no stage-0 crystal → the "pick a crystal" page.
        if ctx.quest_items_count(RED_SOUL_CRYSTAL0) == 0
            && ctx.quest_items_count(GREEN_SOUL_CRYSTAL0) == 0
            && ctx.quest_items_count(BLUE_SOUL_CRYSTAL0) == 0
        {
            return Some(format!("{npc_id}-21.htm"));
        }
        Some(ctx.no_quest_html())
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        if event.ends_with("-04.htm") {
            ctx.start_quest();
        } else if event.ends_with("-09.htm") {
            ctx.give_items(RED_SOUL_CRYSTAL0, 1);
        } else if event.ends_with("-10.htm") {
            ctx.give_items(GREEN_SOUL_CRYSTAL0, 1);
        } else if event.ends_with("-11.htm") {
            ctx.give_items(BLUE_SOUL_CRYSTAL0, 1);
        } else if event.eq_ignore_ascii_case("exit.htm") {
            ctx.exit_quest(false, true);
        }
        Some(event.to_string())
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        self.level_soul_crystals(ctx);
    }

    fn on_skill_see(&self, ctx: &mut QuestCtx, skill_id: i32) {
        // Only the Soul Crystal skill, and only on a mob that can level crystals.
        if skill_id != SOUL_CRYSTAL_SKILL {
            return;
        }
        if ctx.soul_crystal_data().npc_levels(ctx.npc_id).is_none() {
            return;
        }
        ctx.add_absorber();
    }
}
