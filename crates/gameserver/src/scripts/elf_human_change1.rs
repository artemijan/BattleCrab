//! Elf/Human first-class transfers — ports of
//! `dist/game/data/scripts/village_master/ElfHumanFighterChange1/` and
//! `ElfHumanWizardChange1/`.
//!
//! These serve **two races from one script**: a Human Fighter (0) and an Elven
//! Fighter (18) talk to the same four NPCs and see different class lists, and
//! likewise Mage (10) / Elven Mage (25) on the wizard side.
//!
//! | Script | From → to | Proof |
//! |---|---|---|
//! | Fighter | Fighter → Warrior (1) | Medallion of Warrior |
//! | | Fighter → Knight (4) | Sword of Ritual |
//! | | Fighter → Rogue (7) | Beziques' Recommendation |
//! | | Elven Fighter → Elven Knight (19) | Elven Knight Brooch |
//! | | Elven Fighter → Elven Scout (22) | Reisa's Recommendation |
//! | Wizard | Mage → Wizard (11) | Bead of Season |
//! | | Mage → Cleric (15) | Mark of Faith |
//! | | Elven Mage → Elven Wizard (26) | Eternity Diamond |
//! | | Elven Mage → Oracle (29) | Leaf of Oracle |
//!
//! Each target owns four consecutive html pages in a fixed order —
//! `lowLevel`, `lowLevelNoProof`, `afterClassChange`, `noProof` — so the whole
//! matrix is a table rather than a wall of branches. Same 20+ level gate,
//! proof-consumed and 15 D-grade shadow coupons as the other Change1 scripts.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE: i32 = 8869;

// Base classes.
const FIGHTER: i32 = 0;
const MAGE: i32 = 10;
const ELVEN_FIGHTER: i32 = 18;
const ELVEN_MAGE: i32 = 25;

const RACE_HUMAN: i32 = 0;
const RACE_ELF: i32 = 1;

/// One transfer target: `(to_class, from_class, proof_item, first_page)`.
/// The four pages are `first_page ..= first_page + 3`, in Java's order.
type Target = (i32, i32, i32, u32);

/// Pabris, Rains, Ramos, Schule.
const FIGHTER_NPCS: [i32; 4] = [30066, 30288, 30373, 32094];
/// Levian, Sylvain, Raymond, Marie, Celes.
const WIZARD_NPCS: [i32; 5] = [30037, 30070, 30289, 32095, 32098];

const FIGHTER_TARGETS: [Target; 5] = [
    (1, FIGHTER, 1145, 21),        // Warrior — Medallion of Warrior
    (4, FIGHTER, 1161, 25),        // Knight — Sword of Ritual
    (7, FIGHTER, 1190, 29),        // Rogue — Beziques' Recommendation
    (19, ELVEN_FIGHTER, 1204, 33), // Elven Knight — Elven Knight Brooch
    (22, ELVEN_FIGHTER, 1217, 37), // Elven Scout — Reisa's Recommendation
];

const WIZARD_TARGETS: [Target; 4] = [
    (11, MAGE, 1292, 18),       // Wizard — Bead of Season
    (15, MAGE, 1201, 22),       // Cleric — Mark of Faith
    (26, ELVEN_MAGE, 1230, 26), // Elven Wizard — Eternity Diamond
    (29, ELVEN_MAGE, 1235, 30), // Oracle — Leaf of Oracle
];

#[derive(Clone, Copy)]
pub enum Branch {
    Fighter,
    Wizard,
}

pub struct ElfHumanChange1(Branch);

impl ElfHumanChange1 {
    pub const fn fighter() -> Self {
        Self(Branch::Fighter)
    }
    pub const fn wizard() -> Self {
        Self(Branch::Wizard)
    }

    fn targets(&self) -> &'static [Target] {
        match self.0 {
            Branch::Fighter => &FIGHTER_TARGETS,
            Branch::Wizard => &WIZARD_TARGETS,
        }
    }

    fn npcs(&self) -> &'static [i32] {
        match self.0 {
            Branch::Fighter => &FIGHTER_NPCS,
            Branch::Wizard => &WIZARD_NPCS,
        }
    }

    /// `(second_class_page, third_class_page, fourth_class_htm)`.
    ///
    /// The fourth-class page is hard-coded to the *first* NPC's id in Java,
    /// whichever NPC you are talking to — the same quirk as the Dwarf
    /// scripts, and kept for the same reason (only that NPC ships the page).
    fn refusal_pages(&self) -> (u32, u32, &'static str) {
        match self.0 {
            Branch::Fighter => (19, 20, "30066-41.htm"),
            Branch::Wizard => (16, 17, "30037-34.htm"),
        }
    }

    /// The category `onTalk` gates on, and the two class-list pages
    /// (human, elf).
    fn talk_pages(&self) -> (&'static str, u32, u32) {
        match self.0 {
            Branch::Fighter => ("FIGHTER_GROUP", 1, 11),
            Branch::Wizard => ("MAGE_GROUP", 1, 8),
        }
    }

    fn mismatch_page(&self) -> u32 {
        match self.0 {
            Branch::Fighter => 18,
            Branch::Wizard => 15,
        }
    }

    fn class_change(&self, ctx: &mut QuestCtx, class_id: i32) -> Option<String> {
        let npc = ctx.npc_id;
        let (second, third, fourth) = self.refusal_pages();
        if ctx.is_in_category("SECOND_CLASS_GROUP") {
            return Some(format!("{npc}-{second}.htm"));
        }
        if ctx.is_in_category("THIRD_CLASS_GROUP") {
            return Some(format!("{npc}-{third}.htm"));
        }
        if ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return Some(fourth.to_string());
        }

        // Only the matching (target, current class) pair does anything —
        // asking a fighter master for a mage class falls through to `None`.
        let (_, _, proof, first_page) =
            *self.targets().iter().find(|(to, from, _, _)| {
                *to == class_id && *from == ctx.player_class_id()
            })?;

        let has_proof = ctx.quest_items_count(proof) > 0;
        if ctx.player_level() < 20 {
            // low, then lowNoProof.
            let page = if has_proof { first_page } else { first_page + 1 };
            return Some(format!("{npc}-{page}.htm"));
        }
        if !has_proof {
            return Some(format!("{npc}-{}.htm", first_page + 3));
        }
        ctx.take_items(proof, -1);
        ctx.set_class_id(class_id);
        ctx.give_items(SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, 15);
        Some(format!("{npc}-{}.htm", first_page + 2))
    }
}

impl QuestScript for ElfHumanChange1 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        match self.0 {
            Branch::Fighter => "ElfHumanFighterChange1",
            Branch::Wizard => "ElfHumanWizardChange1",
        }
    }
    fn html_dir(&self) -> &'static str {
        match self.0 {
            Branch::Fighter => "village_master/ElfHumanFighterChange1",
            Branch::Wizard => "village_master/ElfHumanWizardChange1",
        }
    }
    fn start_npcs(&self) -> &[i32] {
        self.npcs()
    }
    fn talk_npcs(&self) -> &[i32] {
        self.npcs()
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if let Ok(class_id) = event.parse::<i32>() {
            if self.targets().iter().any(|(to, ..)| *to == class_id) {
                return self.class_change(ctx, class_id);
            }
        }
        // The dialog pages echo back.
        if event.ends_with(".htm") && self.npcs().iter().any(|id| event.starts_with(&id.to_string())) {
            return Some(event.to_string());
        }
        None
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let npc = ctx.npc_id;
        let (category, human_page, elf_page) = self.talk_pages();
        let race = ctx.player_race();
        if ctx.is_in_category(category) && (race == RACE_HUMAN || race == RACE_ELF) {
            let page = if race == RACE_HUMAN { human_page } else { elf_page };
            // Java pads these as `-01`/`-08`/`-11`.
            return Some(format!("{npc}-{page:02}.htm"));
        }
        Some(format!("{npc}-{}.htm", self.mismatch_page()))
    }
}
