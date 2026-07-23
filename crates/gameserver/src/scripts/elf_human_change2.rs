//! Elf/Human second-class transfers — ports of
//! `dist/game/data/scripts/village_master/ElfHumanFighterChange2/`,
//! `ElfHumanWizardChange2/` and `ElfHumanClericChange2/`.
//!
//! The three widest village-master scripts (477 / 291 / 222 Java lines), and —
//! unlike the Orc/Dark Elf pair that opened this group — they are genuinely
//! **uniform**: same level 40 gate, same three-proof `AND`, same 15 C-grade
//! coupons, same `.htm` extension, and the same page order inside a row
//! (`low, lowNoProof, done, noProof`). So everything that differs between them
//! lives in [`Spec`] as data; there is no per-branch code path at all.
//!
//! What *does* differ is which race/class categories gate the greeting. Each
//! script serves a Human line and an Elven line from one NPC set, and Java
//! gates on a different pair of "call class" categories per branch:
//!
//! | Script | class group | race categories | pages |
//! |---|---|---|---|
//! | Fighter | `FIGHTER_GROUP` | `HUMAN_FALL_CLASS` / `ELF_FALL_CLASS` | `30109-01..79` |
//! | Wizard | `WIZARD_GROUP` | `HUMAN_MALL_CLASS` / `ELF_MALL_CLASS` | `30115-01..41` |
//! | Cleric | `CLERIC_GROUP` | `HUMAN_CALL_CLASS` / `ELF_CALL_CLASS` | `30120-01..27` |
//!
//! As with every `*Change2` script, **all** pages are hard-coded to the first
//! NPC's id whichever of the masters you talk to — the dist ships exactly one
//! page set per script, so this cannot be tidied into per-NPC pages.
//!
//! The bypass event is the **target class id** (as in Orc, not the row index
//! Dark Elf uses). `THIRD_CLASS_GROUP` is checked *before* the source-class
//! match, so a third-class player asking for anything gets the refusal page
//! rather than silence.

use crate::game_loop::quests::{QuestCtx, QuestScript};

const SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE: i32 = 8870;

// Marks. Each row needs all three.
const MARK_OF_CHALLENGER: i32 = 2627;
const MARK_OF_DUTY: i32 = 2633;
const MARK_OF_SEEKER: i32 = 2673;
const MARK_OF_SCHOLAR: i32 = 2674;
const MARK_OF_PILGRIM: i32 = 2721;
const MARK_OF_TRUST: i32 = 2734;
const MARK_OF_DUELIST: i32 = 2762;
const MARK_OF_SEARCHER: i32 = 2809;
const MARK_OF_HEALER: i32 = 2820;
const MARK_OF_REFORMER: i32 = 2821;
const MARK_OF_MAGUS: i32 = 2840;
const MARK_OF_LIFE: i32 = 3140;
const MARK_OF_CHAMPION: i32 = 3276;
const MARK_OF_SAGITTARIUS: i32 = 3293;
const MARK_OF_WITCHCRAFT: i32 = 3307;
const MARK_OF_SUMMONER: i32 = 3336;

/// One transfer: `(to_class, from_class, first_page, [three proofs])`.
/// The four pages are `first_page ..= first_page + 3` in Java's order:
/// low, lowNoProof, done, noProof.
type Row = (i32, i32, u32, [i32; 3]);

/// A class-list page and the classes that see it.
type List = (&'static [i32], u32);

/// Everything that differs between the three scripts.
struct Spec {
    name: &'static str,
    html_dir: &'static str,
    npcs: &'static [i32],
    /// The one NPC that owns every page of this script.
    page_npc: i32,
    group: &'static str,
    human_class: &'static str,
    elf_class: &'static str,
    rows: &'static [Row],
    lists: &'static [List],
    /// Shown to someone in the group who has no first occupation yet.
    first_class_page: u32,
    /// Shown to anyone outside the group / the two races.
    mismatch_page: u32,
    third_class_page: u32,
}

const FIGHTER: Spec = Spec {
    name: "ElfHumanFighterChange2",
    html_dir: "village_master/ElfHumanFighterChange2",
    // Hannavalt, Klaus, Siria, Sedrick, Marcus, Bernhard, Siegmund, Hector.
    npcs: &[30109, 30187, 30689, 30849, 30900, 31276, 31321, 31965],
    page_npc: 30109,
    group: "FIGHTER_GROUP",
    human_class: "HUMAN_FALL_CLASS",
    elf_class: "ELF_FALL_CLASS",
    rows: &[
        (
            2,
            1,
            40,
            [MARK_OF_CHALLENGER, MARK_OF_TRUST, MARK_OF_DUELIST],
        ), // Gladiator ← Warrior
        (
            3,
            1,
            44,
            [MARK_OF_CHALLENGER, MARK_OF_TRUST, MARK_OF_CHAMPION],
        ), // Warlord ← Warrior
        (5, 4, 48, [MARK_OF_DUTY, MARK_OF_TRUST, MARK_OF_HEALER]), // Paladin ← Knight
        (6, 4, 52, [MARK_OF_DUTY, MARK_OF_TRUST, MARK_OF_WITCHCRAFT]), // Dark Avenger ← Knight
        (8, 7, 56, [MARK_OF_SEEKER, MARK_OF_TRUST, MARK_OF_SEARCHER]), // Treasure Hunter ← Rogue
        (
            9,
            7,
            60,
            [MARK_OF_SEEKER, MARK_OF_TRUST, MARK_OF_SAGITTARIUS],
        ), // Hawkeye ← Rogue
        (20, 19, 64, [MARK_OF_DUTY, MARK_OF_LIFE, MARK_OF_HEALER]), // Temple Knight ← Elven Knight
        (
            21,
            19,
            68,
            [MARK_OF_CHALLENGER, MARK_OF_LIFE, MARK_OF_DUELIST],
        ), // Swordsinger ← Elven Knight
        (23, 22, 72, [MARK_OF_SEEKER, MARK_OF_LIFE, MARK_OF_SEARCHER]), // Plains Walker ← Elven Scout
        (
            24,
            22,
            76,
            [MARK_OF_SEEKER, MARK_OF_LIFE, MARK_OF_SAGITTARIUS],
        ), // Silver Ranger ← Elven Scout
    ],
    lists: &[
        (&[1, 2, 3], 2),     // Warrior line
        (&[4, 5, 6], 9),     // Knight line
        (&[7, 8, 9], 16),    // Rogue line
        (&[19, 20, 21], 23), // Elven Knight line
        (&[22, 23, 24], 30), // Elven Scout line
    ],
    first_class_page: 37,
    mismatch_page: 38,
    third_class_page: 39,
};

const WIZARD: Spec = Spec {
    name: "ElfHumanWizardChange2",
    html_dir: "village_master/ElfHumanWizardChange2",
    // Jurek, Arkenias, Valleria, Scraide, Drikiyan, Valdis, Halaster, Javier.
    npcs: &[30115, 30174, 30176, 30694, 30854, 31331, 31755, 31996],
    page_npc: 30115,
    group: "WIZARD_GROUP",
    human_class: "HUMAN_MALL_CLASS",
    elf_class: "ELF_MALL_CLASS",
    rows: &[
        (12, 11, 22, [MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_MAGUS]), // Sorcerer ← Wizard
        (
            13,
            11,
            26,
            [MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_WITCHCRAFT],
        ), // Necromancer ← Wizard
        (
            14,
            11,
            30,
            [MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_SUMMONER],
        ), // Warlock ← Wizard
        (27, 26, 34, [MARK_OF_SCHOLAR, MARK_OF_LIFE, MARK_OF_MAGUS]),  // Spellsinger ← Elven Wizard
        (
            28,
            26,
            38,
            [MARK_OF_SCHOLAR, MARK_OF_LIFE, MARK_OF_SUMMONER],
        ), // Elemental Summoner ← Elven Wizard
    ],
    lists: &[
        (&[11, 12, 13, 14], 2), // Wizard line
        (&[26, 27, 28], 12),    // Elven Wizard line
    ],
    first_class_page: 19,
    mismatch_page: 20,
    third_class_page: 21,
};

const CLERIC: Spec = Spec {
    name: "ElfHumanClericChange2",
    html_dir: "village_master/ElfHumanClericChange2",
    // Maximilian, Hollint, Orven, Squillari, Gregory, Innocentin, Baryl.
    npcs: &[30120, 30191, 30857, 30905, 31279, 31328, 31968],
    page_npc: 30120,
    group: "CLERIC_GROUP",
    human_class: "HUMAN_CALL_CLASS",
    elf_class: "ELF_CALL_CLASS",
    rows: &[
        (16, 15, 16, [MARK_OF_PILGRIM, MARK_OF_TRUST, MARK_OF_HEALER]), // Bishop ← Cleric
        (
            17,
            15,
            20,
            [MARK_OF_PILGRIM, MARK_OF_TRUST, MARK_OF_REFORMER],
        ), // Prophet ← Cleric
        (30, 29, 24, [MARK_OF_PILGRIM, MARK_OF_LIFE, MARK_OF_HEALER]),  // Elder ← Oracle
    ],
    lists: &[
        (&[15, 16, 17], 2), // Cleric line
        (&[29, 30], 9),     // Oracle line
    ],
    first_class_page: 13,
    // Page 15 being the third-class refusal while 15 is also the Cleric class
    // id — and row one starting at page 16, the Bishop class id — is pure
    // coincidence. The two numbering spaces never mix.
    mismatch_page: 14,
    third_class_page: 15,
};

#[derive(Clone, Copy)]
pub enum Branch {
    Fighter,
    Wizard,
    Cleric,
}

pub struct ElfHumanChange2(Branch);

impl ElfHumanChange2 {
    pub const fn fighter() -> Self {
        Self(Branch::Fighter)
    }
    pub const fn wizard() -> Self {
        Self(Branch::Wizard)
    }
    pub const fn cleric() -> Self {
        Self(Branch::Cleric)
    }

    fn spec(&self) -> &'static Spec {
        match self.0 {
            Branch::Fighter => &FIGHTER,
            Branch::Wizard => &WIZARD,
            Branch::Cleric => &CLERIC,
        }
    }

    fn page(&self, n: u32) -> String {
        format!("{}-{n:02}.htm", self.spec().page_npc)
    }

    /// Java's `ClassChangeRequested`. Returns `None` where Java returns
    /// `null` — a target this master does not serve from your current class
    /// produces no reply at all.
    fn class_change(&self, ctx: &mut QuestCtx, class_id: i32) -> Option<String> {
        let spec = self.spec();
        if ctx.is_in_category("THIRD_CLASS_GROUP") {
            return Some(self.page(spec.third_class_page));
        }
        let (_, _, first, proofs) = *spec
            .rows
            .iter()
            .find(|(to, from, _, _)| *to == class_id && *from == ctx.player_class_id())?;

        // `hasQuestItems(a, b, c)` is an AND — one mark is not enough.
        let has_all = proofs.iter().all(|id| ctx.quest_items_count(*id) > 0);
        if ctx.player_level() < 40 {
            return Some(self.page(if has_all { first } else { first + 1 }));
        }
        if !has_all {
            return Some(self.page(first + 3));
        }
        for id in proofs {
            ctx.take_items(id, -1);
        }
        ctx.set_class_id(class_id);
        ctx.give_items(SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, 15);
        Some(self.page(first + 2))
    }
}

impl QuestScript for ElfHumanChange2 {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        self.spec().name
    }
    fn html_dir(&self) -> &'static str {
        self.spec().html_dir
    }
    fn start_npcs(&self) -> &[i32] {
        self.spec().npcs
    }
    fn talk_npcs(&self) -> &[i32] {
        self.spec().npcs
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if let Ok(class_id) = event.parse::<i32>() {
            if self.spec().rows.iter().any(|(to, ..)| *to == class_id) {
                return self.class_change(ctx, class_id);
            }
            return None;
        }
        // The dialog pages echo straight back.
        if event.ends_with(".htm") && event.starts_with(&self.spec().page_npc.to_string()) {
            return Some(event.to_string());
        }
        None
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        let spec = self.spec();
        let in_group = ctx.is_in_category(spec.group);
        let right_race = ctx.is_in_category(spec.human_class) || ctx.is_in_category(spec.elf_class);
        if !in_group || !right_race {
            return Some(self.page(spec.mismatch_page));
        }
        if ctx.is_in_category("FOURTH_CLASS_GROUP") {
            return Some(self.page(1));
        }
        let class_id = ctx.player_class_id();
        let page = spec
            .lists
            .iter()
            .find(|(classes, _)| classes.contains(&class_id))
            .map_or(spec.first_class_page, |(_, page)| *page);
        Some(self.page(page))
    }
}
