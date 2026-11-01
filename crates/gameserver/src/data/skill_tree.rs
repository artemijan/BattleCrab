//! Port of `data/xml/SkillTreeData` — the skill trees under
//! `data/stats/players/skillTrees/**`, driving character creation
//! (`initNewChar`), `RequestAcquireSkill`'s `CLASS` learn path, and
//! `Player.rewardSkills` / `checkPlayerSkills` on level and class change.
//!
//! Salvation's loader walks the whole directory and files every
//! `<skillTree>` by its `type` attribute, not by file name; this port does the
//! same. The `classSkillTree` entries live in the tier directories
//! (`StartingClass/`, `1stClass/` … `4rthClass/`, `Ertheia/`) and
//! `BalthusKnightSkillTree.xml`, each `<skillTree>` carrying its `classId` and
//! (for non-base classes) a `parentClassId`. Java's `getCompleteClassSkillTree`
//! walks that parent chain and unions in the class-agnostic `Commons.xml` tree,
//! so a Warlord (3 → Warrior 1 → Human Fighter 0) reaches its own skills *and*
//! every ancestor's plus the common ones. [`SkillTreeData::complete_entries`]
//! reproduces that union; all the per-class queries below run through it.
//!
//! Every tree type Salvation reads is loaded — transfer, collect, race,
//! revelation, sub-class, ability, alchemy, dual-class and awakening-save
//! included — together with every `SkillLearn` attribute and child. Most of
//! those have no consumer in this port yet (S8: awakening, dual class, ability
//! points). The pledge and sub-pledge trees are read by
//! [`crate::data::pledge_skill_tree`].
//!
//! `autoGet` is the attribute and nothing else, as in Java's `SkillLearn`: the
//! Interlude port also treated every `getLevel <= 1`, `skillLevel == 1` entry
//! as auto-granted, which on this dist would hand Orc Mystics Chill Flame.

use std::collections::{HashMap, HashSet};

use crate::data::xml;
use crate::data::xml::{attr_i32, attr_i64, attr_str};
use quick_xml::events::Event;
use tracing::{info, warn};

pub const SKILL_TREES_DIR: &str = "data/stats/players/skillTrees";
/// The deepest `parentClassId` chain the complete-tree walk follows: a
/// fourth-class (awakened) id → 3rd → 2nd → 1st → starting class is five, and
/// the bound only exists to stop a malformed cycle.
const MAX_CLASS_DEPTH: usize = 8;

/// Java `CommonSkill.EXPERTISE` (239): the one skill `checkPlayerSkills`
/// verifies with no level grace — its level *is* the wearable grade, so it may
/// not outrank the character even by the usual 9 levels.
const EXPERTISE_SKILL_ID: i32 = 239;

/// Java `CommonSkill.DIVINE_INSPIRATION` (1405): an item-gated class skill the
/// auto-learn path withholds unless `AutoLearnDivineInspiration` is set, and
/// the one skill `DivineInspirationSpBookNeeded` can waive the book for.
pub const DIVINE_INSPIRATION_SKILL_ID: i32 = 1405;

/// What `getAvailableSkills` / `getNextAvailableSkills` read off the player.
pub struct Learner<'a> {
    pub class_id: i32,
    pub level: i32,
    /// `player.getSkills()`: id → level.
    pub known: &'a HashMap<i32, i32>,
    /// `isInCategory(SIXTH_CLASS_GROUP) && (race != ERTHEIA ||
    /// isDualClassActive())`.
    pub awakened: bool,
}

impl Learner<'_> {
    /// `oldSkill.getLevel() == skillLevel - 1`, or `skillLevel == 1` for an
    /// unknown skill.
    fn is_next_level(&self, s: &SkillLearn) -> bool {
        match self.known.get(&s.skill_id) {
            Some(&old) => old == s.skill_level - 1,
            None => s.skill_level == 1,
        }
    }
}

/// A skill a character knows: `(skill_id, skill_level)`.
pub type Skill = (i32, i32);

/// One `<skill>` entry from a skill tree (Java `SkillLearn`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillLearn {
    pub skill_id: i32,
    pub skill_level: i32,
    pub name: String,
    /// Minimum character level to learn this entry (`getLevel` attribute).
    pub get_level: i32,
    /// SP cost (0 for autoGet skills, which are granted free at creation).
    pub level_up_sp: i64,
    pub auto_get: bool,
    /// The `<item id count/>` children — books learning this entry consumes
    /// (Java `SkillLearn.getRequiredItems`, an `ItemHolder` list). The
    /// `AutoLearnSkills` path skips entries with any, unless
    /// `AutoLearnSkillsWithoutItems`; the manual `RequestAcquireSkill` path
    /// lists them in `AcquireSkillList` and verifies + consumes them.
    pub required_items: Vec<(i32, i64)>,
    /// `getDualClassLevel` (Java `_getDualClassLevel`).
    pub get_dual_class_level: i32,
    /// `residenceSkill`, `learnedByNpc`, `learnedByFS`.
    pub residence_skill: bool,
    pub learned_by_npc: bool,
    pub learned_by_fs: bool,
    /// The ability-tree layout: `treeId`, `row`, `column` (Java reads the
    /// column from the `row` attribute too — upstream's typo, kept) and
    /// `pointsRequired`.
    pub tree_id: i32,
    pub row: i32,
    pub column: i32,
    pub points_required: i32,
    /// `<preRequisiteSkill id lvl/>` children (Java `getPreReqSkills`).
    pub pre_req_skills: Vec<(i32, i32)>,
    /// `<race>` children (Java `getRaces`).
    pub races: Vec<crate::enums::Race>,
    /// `<residenceId>` children (Java `getResidenceIds`).
    pub residence_ids: Vec<i32>,
    /// `<socialClass>` (Java `getSocialClass`), as the enum constant's name.
    pub social_class: Option<String>,
    /// `<removeSkill id onlyReplaceByLearn/>` children: the id, and whether it
    /// stays out of Java's `_removeSkillCache`.
    pub remove_skills: Vec<(i32, bool)>,
}

impl SkillLearn {
    /// Whether learning this entry costs a book (Java
    /// `!getRequiredItems().isEmpty()`).
    pub fn requires_item(&self) -> bool {
        !self.required_items.is_empty()
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillTreeData {
    /// Every `<skill>` entry per class id, in document order (Java
    /// `_classSkillTrees`). Holds only the class's *own* tier entries — the
    /// parent chain and common tree are merged on lookup by [`complete_entries`].
    trees: HashMap<i32, Vec<SkillLearn>>,
    /// `classId → parentClassId` from each `<skillTree parentClassId=…>` (Java
    /// `_parentClassMap`), walked by [`complete_entries`].
    parents: HashMap<i32, i32>,
    /// The class-agnostic tree unioned into every class (Java `_commonSkillTree`,
    /// from `Commons.xml`): Lucky, the Expertise grades, weapon masteries, …
    common: Vec<SkillLearn>,
    /// The hero skill tree (Java `getHeroSkillTree`) — a flat `(id, level)`
    /// list from `heroSkillTree.xml`, granted/removed by `//sethero`.
    hero_skills: Vec<Skill>,
    /// The noble skill tree (Java `getNobleSkillTree`) — 8 skills from
    /// `nobleSkillTree.xml` (Noblesse Blessing, the three Noblesse songs,
    /// Build Advanced Headquarters, …), granted/removed with nobless status.
    noble_skills: Vec<Skill>,
    /// `gameMasterSkillTree.xml` / `gameMasterAuraSkillTree.xml` — the GM
    /// convenience kits (Super Haste, the Master's Blessing buffs, and their
    /// party-wide aura twins), handed out at enter-world under
    /// `GMGiveSpecialSkills` / `GMGiveSpecialAuraSkills`.
    ///
    /// Flat lists like the hero/noble trees: Java's `addSkills` ignores every
    /// learn condition and grants outright.
    gm_skills: Vec<Skill>,
    gm_aura_skills: Vec<Skill>,
    /// `(skill_id, skill_level, races)` from `fishingSkillTree.xml` and
    /// `transformSkillTree.xml` — the input to Java's `_skillsByRaceHashCodes`
    /// (entries that name races) and the race-less part of
    /// `_allSkillsHashCodes`. Read by [`Self::is_skill_allowed`] and nothing
    /// else: this port teaches neither tree.
    check_only_entries: Vec<(i32, i32, Vec<crate::enums::Race>)>,
    /// `transferSkillTree` per `classId` (Java `_transferSkillTrees`).
    transfer: HashMap<i32, Vec<SkillLearn>>,
    /// `raceSkillTree` per race ordinal (Java `_raceSkillTree`).
    race_trees: HashMap<i32, Vec<SkillLearn>>,
    /// `revelationSkillTree` per `subType` (`BASECLASS`/`DUALCLASS`).
    revelation: HashMap<String, Vec<SkillLearn>>,
    /// The flat trees: `collectSkillTree`, `subClassSkillTree`,
    /// `abilitySkillTree`, `alchemySkillTree`, `dualClassSkillTree`.
    collect: Vec<SkillLearn>,
    sub_class: Vec<SkillLearn>,
    ability: Vec<SkillLearn>,
    alchemy: Vec<SkillLearn>,
    dual_class: Vec<SkillLearn>,
    /// `awakeningSaveSkillTree`: class → skill ids kept through awakening
    /// (Java `_awakeningSaveSkillTree`).
    awakening_save: HashMap<i32, HashSet<i32>>,
    /// Java `_removeSkillCache`: class → `<removeSkill>` ids not marked
    /// `onlyReplaceByLearn`.
    remove_skill_cache: HashMap<i32, HashSet<i32>>,
}

impl SkillTreeData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let mut data = Self::empty();
        // Java's `playerClass` local is not reset by a `<skillTree>` without a
        // `classId`, so the class-keyed caches of such a block land on the
        // previous block's class. Kept: it is where Java files them.
        let mut player_class: Option<i32> = None;
        for path in xml::xml_files_under(format!("{file_path}{SKILL_TREES_DIR}")) {
            for block in parse_file(&path) {
                if let Some(cid) = block.class_id {
                    player_class = Some(cid);
                }
                let cid = block.class_id.unwrap_or(-1);
                if let Some(parent) = block.parent_class_id
                    && cid > -1
                    && parent > -1
                    && parent != cid
                {
                    data.parents.entry(cid).or_insert(parent);
                }
                if let Some(pc) = player_class {
                    for learn in &block.skills {
                        for &(id, only_replace) in &learn.remove_skills {
                            if !only_replace {
                                data.remove_skill_cache.entry(pc).or_default().insert(id);
                            }
                        }
                    }
                }
                data.file_block(block, cid, player_class);
            }
        }
        let total: usize = data.trees.values().map(|v| v.len()).sum();
        info!(
            "SkillTreeData: Loaded skill trees for {} classes ({total} skill levels), {} common + {} hero skills.",
            data.trees.len(),
            data.common.len(),
            data.hero_skills.len()
        );
        info!(
            "SkillTreeData: Loaded {} noble, {} transfer, {} race, {} revelation, {} collect, {} sub-class, {} ability, {} alchemy, {} dual-class skills.",
            data.noble_skills.len(),
            data.transfer.values().map(Vec::len).sum::<usize>(),
            data.race_trees.values().map(Vec::len).sum::<usize>(),
            data.revelation.values().map(Vec::len).sum::<usize>(),
            data.collect.len(),
            data.sub_class.len(),
            data.ability.len(),
            data.alchemy.len(),
            data.dual_class.len()
        );
        data
    }

    /// Java `parseDocument`'s `switch (type)` and the per-block puts after it.
    fn file_block(&mut self, block: TreeBlock, cid: i32, player_class: Option<i32>) {
        let flat = |skills: &[SkillLearn]| -> Vec<Skill> {
            skills.iter().map(|l| (l.skill_id, l.skill_level)).collect()
        };
        match block.kind.as_str() {
            "classSkillTree" if cid != -1 => {
                self.trees.entry(cid).or_default().extend(block.skills);
            }
            "classSkillTree" => self.common.extend(block.skills),
            "transferSkillTree" => {
                if let Some(pc) = player_class {
                    self.transfer.insert(pc, block.skills);
                }
            }
            "collectSkillTree" => self.collect.extend(block.skills),
            "raceSkillTree" => {
                if let Some(race) = block.race {
                    self.race_trees
                        .entry(race.ordinal())
                        .or_default()
                        .extend(block.skills);
                }
            }
            "revelationSkillTree" => {
                if let Some(sub_type) = block.sub_type {
                    self.revelation
                        .entry(sub_type)
                        .or_default()
                        .extend(block.skills);
                }
            }
            "fishingSkillTree" | "transformSkillTree" => {
                self.check_only_entries.extend(
                    block
                        .skills
                        .into_iter()
                        .map(|l| (l.skill_id, l.skill_level, l.races)),
                );
            }
            // Read by `PledgeSkillTreeData`, which keeps its own narrower view.
            "pledgeSkillTree" | "subPledgeSkillTree" => {}
            "subClassSkillTree" => self.sub_class.extend(block.skills),
            "nobleSkillTree" => self.noble_skills.extend(flat(&block.skills)),
            "abilitySkillTree" => self.ability.extend(block.skills),
            "alchemySkillTree" => self.alchemy.extend(block.skills),
            "heroSkillTree" => self.hero_skills.extend(flat(&block.skills)),
            "gameMasterSkillTree" => self.gm_skills.extend(flat(&block.skills)),
            "gameMasterAuraSkillTree" => self.gm_aura_skills.extend(flat(&block.skills)),
            "dualClassSkillTree" => self.dual_class.extend(block.skills),
            "awakeningSaveSkillTree" => {
                if let Some(pc) = player_class {
                    self.awakening_save
                        .entry(pc)
                        .or_default()
                        .extend(block.skills.iter().map(|l| l.skill_id));
                }
            }
            other => warn!("SkillTreeData: Unknown Skill Tree type: {other}!"),
        }
    }

    /// Java `getCompleteClassSkillTree`: the class's own `<skill>` entries, then
    /// every ancestor's (following `parentClassId`), then the common tree —
    /// deduped by `(skill_id, skill_level)` so the most-derived definition wins
    /// (child before parent before common, matching Java's `putAll` layering
    /// where the earliest-inserted key survives an identical re-put). Returns
    /// borrowed entries so the per-class queries can filter without cloning.
    fn complete_entries(&self, class_id: i32) -> Vec<&SkillLearn> {
        let mut out: Vec<&SkillLearn> = Vec::new();
        let mut seen: HashSet<(i32, i32)> = HashSet::new();
        let mut current = Some(class_id);
        // Guard against a malformed `parentClassId` cycle in the data.
        for _ in 0..MAX_CLASS_DEPTH {
            let Some(cid) = current else { break };
            if let Some(entries) = self.trees.get(&cid) {
                for s in entries {
                    if seen.insert((s.skill_id, s.skill_level)) {
                        out.push(s);
                    }
                }
            }
            current = self.parents.get(&cid).copied();
        }
        for s in &self.common {
            if seen.insert((s.skill_id, s.skill_level)) {
                out.push(s);
            }
        }
        out
    }

    /// Java `SkillTreeData.getHeroSkillTree` — the `(skill_id, level)` list
    /// granted while a player holds hero status.
    pub fn hero_skills(&self) -> &[Skill] {
        &self.hero_skills
    }

    /// Every skill id some tree here can put on a character: the class trees,
    /// the common tree, and the hero and noble grants.
    ///
    /// Used by the boot-time skill-gap report to tell a parser gap that touches
    /// a skill a player can actually hold from one that only touches datapack
    /// content no Interlude tree references. **GM skills are excluded on
    /// purpose** — they are handed to staff by config rather than learned, so
    /// counting them would inflate "reachable" with skills no player holds.
    ///
    /// A boot-time approximation for a *log line*, not the parity gate:
    /// `coverage_census::datapack_skill_coverage_census` scans the
    /// `skillTrees/**` XML directly, deliberately not through these loaders, so
    /// that it cannot measure the port against itself.
    pub fn all_learnable_skill_ids(&self) -> std::collections::BTreeSet<i32> {
        self.trees
            .values()
            .flatten()
            .chain(self.common.iter())
            .map(|l| l.skill_id)
            .chain(self.hero_skills.iter().map(|&(id, _)| id))
            .chain(self.noble_skills.iter().map(|&(id, _)| id))
            .collect()
    }

    /// Java `SkillTreeData.addSkills(gm, auraSkills)` — the whole tree, no
    /// conditions.
    pub fn gm_skills(&self, aura: bool) -> &[Skill] {
        if aura {
            &self.gm_aura_skills
        } else {
            &self.gm_skills
        }
    }

    /// Java's `isSkillAllowed`, the whole allow-list.
    ///
    /// The question it answers is narrow and worth stating exactly: **may this
    /// `character_skills` row exist for this character?** Not "can they cast it",
    /// not "have they met the level" — those are other gates. A `false` here means
    /// the row could never have been produced legitimately.
    ///
    /// Java's seven arms, in order, and what each is on this dist:
    ///
    /// 1. `skill.isExcludedFromCheck()` — the datapack's own opt-out.
    /// 2. GM holding a GM-tree skill.
    /// 3. `_loading` — a reload guard with no counterpart here: the port's trees
    ///    are built once at boot, before any character loads.
    /// 4. The complete class tree (class chain ∪ commons).
    /// 5. The race-scoped fishing/transform entries.
    /// 6. The race-less fishing/transform/commons entries, plus the whole
    ///    collect, ability and alchemy trees (races or not). 5 and 6 read
    ///    [`Self::check_only_entries`] plus those three trees; the commons are
    ///    already in the class tree.
    /// 7. The class's transfer tree, then the race tree.
    ///
    /// Java clamps the level to `SkillData.getMaxLevel(skillId)` before hashing,
    /// which is what lets an *enchanted* skill (level 100+) match its level-`max`
    /// tree entry. `max_skill_level` supplies that clamp.
    pub fn is_skill_allowed(
        &self,
        class_id: i32,
        race: Option<crate::enums::Race>,
        is_gm: bool,
        skill_id: i32,
        skill_level: i32,
        max_skill_level: i32,
    ) -> bool {
        if is_gm && self.is_gm_skill(skill_id) {
            return true;
        }
        let level = skill_level.min(max_skill_level.max(1));
        if self
            .complete_entries(class_id)
            .iter()
            .any(|e| e.skill_id == skill_id && e.skill_level == level)
        {
            return true;
        }
        if self.check_only_entries.iter().any(|(id, lvl, races)| {
            *id == skill_id
            && *lvl == level
            // Java splits these into two arrays — race-scoped entries are
            // matched against the player's race, race-less ones against
            // everyone — and both are consulted, so one predicate covers it.
            && (races.is_empty() || race.is_some_and(|r| races.contains(&r)))
        }) {
            return true;
        }
        let matches = |l: &SkillLearn| l.skill_id == skill_id && l.skill_level == level;
        if self
            .collect
            .iter()
            .chain(&self.ability)
            .chain(&self.alchemy)
            .any(matches)
        {
            return true;
        }
        if self
            .transfer
            .get(&class_id)
            .is_some_and(|tree| tree.iter().any(matches))
        {
            return true;
        }
        race.and_then(|r| self.race_trees.get(&r.ordinal()))
            .is_some_and(|tree| tree.iter().any(matches))
    }

    /// Whether `skill_id` is a hero or noble grant — the two other trees Java
    /// hands out with `addSkill(skill, false)` (`Player.setHero` carries the
    /// comment *"Don't persist hero skills into database"* verbatim).
    ///
    /// Both are **derived state**: hero from the `heroes` table at enter-world,
    /// nobless from the character's `nobless` column at load. Persisting them
    /// as learned rows is not merely redundant, it is wrong in the direction
    /// that matters — a hero stripped of the crown while offline, or a nobless
    /// revoked, would keep the skills forever, because nothing re-derives a row
    /// that is already there.
    pub fn is_hero_or_noble_skill(&self, skill_id: i32) -> bool {
        self.hero_skills
            .iter()
            .chain(&self.noble_skills)
            .any(|(id, _)| *id == skill_id)
    }

    /// Whether `skill_id` came from a GM tree. Read by the persistence flush:
    /// Java grants these with `addSkill(skill, false)` — **not saved** — so a
    /// GM who logs in once must not carry Super Haste in `character_skills`
    /// forever, least of all after the config is turned back off. Same shape
    /// as the transform-skill filter that flush already has.
    pub fn is_gm_skill(&self, skill_id: i32) -> bool {
        self.gm_skills
            .iter()
            .chain(&self.gm_aura_skills)
            .any(|(id, _)| *id == skill_id)
    }

    /// `PlayerClass.getNextClasses()`: the classes that advance from this
    /// one, lowest id first.
    pub fn next_classes(&self, class_id: i32) -> Vec<i32> {
        let mut out: Vec<i32> = self
            .parents
            .iter()
            .filter(|&(_, &p)| p == class_id)
            .map(|(&c, _)| c)
            .collect();
        out.sort_unstable();
        out
    }

    /// `cleanSkillUponAwakening`'s keep test, short of its item-skill
    /// search: the skill at its datapack `max_level` is in the class's own
    /// tree (`isCurrentClassSkillNoParent`), on its `removeSkill` list, on its
    /// awakening-save list, or is an alchemy skill at its current `level`.
    pub fn kept_upon_awakening(
        &self,
        class_id: i32,
        skill_id: i32,
        max_level: i32,
        level: i32,
    ) -> bool {
        let in_tree = |tree: &[SkillLearn], lvl: i32| {
            tree.iter()
                .any(|l| l.skill_id == skill_id && l.skill_level == lvl)
        };
        self.trees
            .get(&class_id)
            .is_some_and(|t| in_tree(t, max_level))
            || self
                .remove_skill_cache
                .get(&class_id)
                .is_some_and(|ids| ids.contains(&skill_id))
            || self
                .awakening_save
                .get(&class_id)
                .is_some_and(|ids| ids.contains(&skill_id))
            || in_tree(&self.alchemy, level)
    }

    /// `getRaceSkillTree(race)`, by race ordinal.
    pub fn race_skill_tree(&self, race: i32) -> &[SkillLearn] {
        self.race_trees.get(&race).map_or(&[], Vec::as_slice)
    }

    /// The class this one advanced from (`ClassId.getParent`), if any.
    pub fn parent_class(&self, class_id: i32) -> Option<i32> {
        self.parents.get(&class_id).copied()
    }

    /// Walk up to the class's root (1st-occupation) ancestor, inclusive.
    pub fn class_lineage(&self, class_id: i32) -> Vec<i32> {
        let mut out = vec![class_id];
        let mut cur = class_id;
        while let Some(p) = self.parents.get(&cur).copied() {
            if out.contains(&p) {
                break;
            }
            out.push(p);
            cur = p;
        }
        out
    }

    /// Test hook: install a synthetic noble tree.
    #[doc(hidden)]
    pub fn set_noble_skills_for_test(&mut self, skills: Vec<Skill>) {
        self.noble_skills = skills;
    }

    /// Test hook: install a synthetic hero tree.
    #[doc(hidden)]
    pub fn set_hero_skills_for_test(&mut self, skills: Vec<Skill>) {
        self.hero_skills = skills;
    }

    /// Java `SkillTreeData.getNobleSkillTree`.
    pub fn noble_skills(&self) -> &[Skill] {
        &self.noble_skills
    }

    /// The skills a freshly created character of `class_id` starts with
    /// (Java `getAvailableSkills(..., includeAutoGet=true)` at level 1).
    pub fn initial_skills(&self, class_id: i32) -> Vec<Skill> {
        self.trees
            .get(&class_id)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|s| s.auto_get)
                    .map(|s| (s.skill_id, s.skill_level))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Java `SkillTreeData.getAvailableSkills(player, playerClass, false,
    /// false)` — the public overload, so required-item entries stay in. Every
    /// complete-tree entry the learner's level reaches whose `skillLevel` is
    /// one past what they know (1 for an unknown skill), skipping autoGet and
    /// forgotten-scroll entries, the class's `<removeSkill>` ids, levels past
    /// the skill's highest loaded one, and — for an awakened learner — any
    /// entry neither in the class's own tree nor in its awakening-save list.
    /// Ends with [`Self::drop_removed_by_known`].
    ///
    /// `max_skill_level` is `SkillData.getMaxLevel`. `hasReplacedSkill` has no
    /// counterpart: only `ReplaceSkillBySkill` feeds it, and that effect is
    /// not ported.
    pub fn available_skills(
        &self,
        learner: &Learner,
        max_skill_level: impl Fn(i32) -> i32,
    ) -> Vec<&SkillLearn> {
        let class_id = learner.class_id;
        let removed = self.remove_skill_cache.get(&class_id);
        let own = self.trees.get(&class_id);
        let saved = self.awakening_save.get(&class_id);
        let mut out: Vec<&SkillLearn> = self
            .complete_entries(class_id)
            .into_iter()
            .filter(|s| !s.auto_get && !s.learned_by_fs)
            .filter(|s| !removed.is_some_and(|r| r.contains(&s.skill_id)))
            .filter(|s| {
                !learner.awakened
                    || own.is_some_and(|t| {
                        t.iter()
                            .any(|o| o.skill_id == s.skill_id && o.skill_level == s.skill_level)
                    })
                    || saved.is_some_and(|k| k.contains(&s.skill_id))
            })
            .filter(|s| learner.level >= s.get_level)
            .filter(|s| s.skill_level <= max_skill_level(s.skill_id))
            .filter(|s| learner.is_next_level(s))
            .collect();
        self.drop_removed_by_known(class_id, learner.known, &mut out);
        out
    }

    /// Java `SkillTreeData.getNextAvailableSkills(player, playerClass, false,
    /// false)` — what the learner will be offered later. From
    /// `getMinLevelForNewSkill` (the lowest `getLevel` above theirs, over the
    /// whole complete tree) up, every non-autoGet, non-forgotten-scroll entry
    /// one level past what they know, up to `PLAYER_MAXIMUM_LEVEL`. No
    /// remove-skill, awakening or max-level filter, as in Java. Ends with
    /// [`Self::drop_removed_by_known`].
    pub fn next_available_skills(
        &self,
        learner: &Learner,
        max_player_level: i32,
    ) -> Vec<&SkillLearn> {
        let class_id = learner.class_id;
        let entries = self.complete_entries(class_id);
        let min_level = entries
            .iter()
            .map(|s| s.get_level)
            .filter(|&g| learner.level < g)
            .min()
            .unwrap_or(0);
        let mut out: Vec<&SkillLearn> = if min_level > 0 {
            entries
                .into_iter()
                .filter(|s| s.get_level <= max_player_level)
                .filter(|s| !s.auto_get && !s.learned_by_fs)
                .filter(|s| min_level <= s.get_level)
                .filter(|s| learner.is_next_level(s))
                .collect()
        } else {
            Vec::new()
        };
        self.drop_removed_by_known(class_id, learner.known, &mut out);
        out
    }

    /// The "Manage skill unlearn" tail both selections share: a known skill
    /// whose class-tree entry (or, above level 1, its level-1 entry) lists
    /// `<removeSkill>` ids takes the first offered entry of each id out.
    fn drop_removed_by_known(
        &self,
        class_id: i32,
        known: &HashMap<i32, i32>,
        out: &mut Vec<&SkillLearn>,
    ) {
        let entries = self.complete_entries(class_id);
        let class_skill = |id: i32, level: i32| {
            entries
                .iter()
                .find(|s| s.skill_id == id && s.skill_level == level)
                .copied()
        };
        for (&id, &level) in known {
            let Some(mut learn) = class_skill(id, level) else {
                continue;
            };
            if learn.remove_skills.is_empty() {
                if level <= 1 {
                    continue;
                }
                match class_skill(id, 1) {
                    Some(first) if !first.remove_skills.is_empty() => learn = first,
                    _ => continue,
                }
            }
            for &(remove_id, _) in &learn.remove_skills {
                if let Some(i) = out.iter().position(|s| s.skill_id == remove_id) {
                    out.remove(i);
                }
            }
        }
    }

    /// Java `Player.rewardSkills` → `getAvailableAutoGetSkills`: every autoGet
    /// entry reachable at `level`. The caller keeps only levels above what the
    /// player already knows.
    pub fn auto_get_skills(&self, class_id: i32, level: i32) -> Vec<&SkillLearn> {
        self.complete_entries(class_id)
            .into_iter()
            .filter(|s| s.auto_get && s.get_level <= level)
            .collect()
    }

    /// Java `SkillTreeData.getAllAvailableSkills` (the `AutoLearnSkills` path,
    /// `includeAutoGet=true`) narrowed to base-class trees, which carry no
    /// forgotten-scroll / removeSkills / required-item entries. For every skill
    /// in the class tree it returns the highest `skill_level` whose `get_level`
    /// the player has reached — but only when that beats what they already
    /// know. Java grants levels one at a time through a holder until it stops
    /// changing; since `get_level` rises with `skill_level`, taking the max
    /// reachable level per skill lands on the same final state in one pass.
    /// `include_required_items` (Java `AutoLearnSkillsWithoutItems`) keeps
    /// book-gated skills; `include_divine_inspiration` (Java
    /// `AutoLearnDivineInspiration`, or GM) keeps Divine Inspiration — both
    /// otherwise withheld from the free auto-learn grant.
    pub fn all_available_skills(
        &self,
        class_id: i32,
        level: i32,
        known: &HashMap<i32, i32>,
        include_required_items: bool,
        include_divine_inspiration: bool,
    ) -> Vec<Skill> {
        let mut best: HashMap<i32, i32> = HashMap::new();
        for s in self
            .complete_entries(class_id)
            .into_iter()
            .filter(|s| s.get_level <= level)
        {
            if !include_required_items && s.requires_item() {
                continue;
            }
            if s.skill_id == DIVINE_INSPIRATION_SKILL_ID && !include_divine_inspiration {
                continue;
            }
            let slot = best.entry(s.skill_id).or_insert(0);
            *slot = (*slot).max(s.skill_level);
        }
        best.into_iter()
            .filter(|&(id, lvl)| lvl > known.get(&id).copied().unwrap_or(0))
            .collect()
    }

    /// The `SkillLearn` for a specific `(class_id, skill_id, skill_level)`,
    /// used by `RequestAcquireSkill` to re-validate the client's request.
    pub fn skill_learn(
        &self,
        class_id: i32,
        skill_id: i32,
        skill_level: i32,
    ) -> Option<&SkillLearn> {
        self.complete_entries(class_id)
            .into_iter()
            .find(|s| s.skill_id == skill_id && s.skill_level == skill_level)
    }

    /// Java `Player.checkPlayerSkills` + `deacreaseSkillLevel`: given the
    /// player's current `level` and the skills they `known` (id → level),
    /// decide the corrective action for every skill whose learn level the
    /// player has fallen below. Returns, sorted by skill id for determinism:
    /// `(skill_id, Some(new_level))` to downgrade to the highest still-reachable
    /// level, or `(skill_id, None)` to remove the skill (no reachable level
    /// remains). Skills absent from this class tree are left untouched (Java
    /// `getClassSkill` → null → skip).
    ///
    /// A 9-level grace applies to every skill but Expertise, which has none.
    pub fn delevel_skill_changes(
        &self,
        class_id: i32,
        level: i32,
        known: &HashMap<i32, i32>,
    ) -> Vec<(i32, Option<i32>)> {
        let entries = self.complete_entries(class_id);
        if entries.is_empty() {
            return Vec::new();
        }
        let level_diff_of = |skill_id: i32| {
            if skill_id == EXPERTISE_SKILL_ID { 0 } else { 9 }
        };
        let mut out = Vec::new();
        for (&skill_id, &skill_level) in known {
            // Java keys the lookup on `getLevel() % 100` — enchanted skills
            // carry the enchant route in the hundreds digit.
            let base_level = skill_level % 100;
            let Some(learn) = entries
                .iter()
                .find(|s| s.skill_id == skill_id && s.skill_level == base_level)
            else {
                continue;
            };
            let level_diff = level_diff_of(skill_id);
            if level >= (learn.get_level - level_diff) {
                continue; // still within range — keep as is
            }
            // deacreaseSkillLevel: highest level of this skill still reachable.
            let mut next = -1;
            for s in entries.iter() {
                if s.skill_id == skill_id
                    && s.skill_level > next
                    && level >= (s.get_level - level_diff)
                {
                    next = s.skill_level;
                }
            }
            out.push((skill_id, (next != -1).then_some(next)));
        }
        out.sort_by_key(|&(id, _)| id);
        out
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            trees: HashMap::new(),
            parents: HashMap::new(),
            common: Vec::new(),
            hero_skills: Vec::new(),
            gm_skills: Vec::new(),
            gm_aura_skills: Vec::new(),
            noble_skills: Vec::new(),
            check_only_entries: Vec::new(),
            transfer: HashMap::new(),
            race_trees: HashMap::new(),
            revelation: HashMap::new(),
            collect: Vec::new(),
            sub_class: Vec::new(),
            ability: Vec::new(),
            alchemy: Vec::new(),
            dual_class: Vec::new(),
            awakening_save: HashMap::new(),
            remove_skill_cache: HashMap::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, class_id: i32, learn: SkillLearn) {
        self.trees.entry(class_id).or_default().push(learn);
    }

    #[doc(hidden)]
    pub fn set_parent_for_test(&mut self, class_id: i32, parent_class_id: i32) {
        self.parents.insert(class_id, parent_class_id);
    }
}

/// One `<skillTree>` block as Java's `parseDocument` sees it: the `type`, the
/// header attributes, and its `<skill>` entries with their children read.
struct TreeBlock {
    kind: String,
    class_id: Option<i32>,
    parent_class_id: Option<i32>,
    race: Option<crate::enums::Race>,
    sub_type: Option<String>,
    skills: Vec<SkillLearn>,
}

/// `new SkillLearn(StatSet)` over the `<skill>` attributes.
fn new_learn(e: &quick_xml::events::BytesStart) -> SkillLearn {
    let flag = |key: &str| attr_str(e, key).as_deref() == Some("true");
    // Java reads `_column` from the `row` attribute — upstream's typo, kept.
    let row = attr_i32(e, "row").unwrap_or(0);
    SkillLearn {
        skill_id: attr_i32(e, "skillId").unwrap_or(-1),
        skill_level: attr_i32(e, "skillLevel").unwrap_or(0),
        name: attr_str(e, "skillName").unwrap_or_default(),
        get_level: attr_i32(e, "getLevel").unwrap_or(0),
        get_dual_class_level: attr_i32(e, "getDualClassLevel").unwrap_or(0),
        level_up_sp: attr_i64(e, "levelUpSp").unwrap_or(0),
        auto_get: flag("autoGet"),
        residence_skill: flag("residenceSkill"),
        learned_by_npc: flag("learnedByNpc"),
        learned_by_fs: flag("learnedByFS"),
        tree_id: attr_i32(e, "treeId").unwrap_or(0),
        row,
        column: row,
        points_required: attr_i32(e, "pointsRequired").unwrap_or(0),
        ..SkillLearn::default()
    }
}

/// Every `<skillTree>` block of one file, in document order.
fn parse_file(path: &std::path::Path) -> Vec<TreeBlock> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut blocks: Vec<TreeBlock> = Vec::new();
    // The `<skill>` whose children are still being read: an entry with
    // children is only filed at its `</skill>`, after Java's child loop.
    let mut pending: Option<SkillLearn> = None;
    // The text-bodied child being read (`race`, `residenceId`, `socialClass`).
    let mut text_child: Option<String> = None;
    for event in xml::events(&content) {
        match event {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "skillTree" => {
                blocks.push(TreeBlock {
                    kind: attr_str(&e, "type").unwrap_or_default(),
                    class_id: attr_i32(&e, "classId"),
                    parent_class_id: attr_i32(&e, "parentClassId"),
                    race: attr_str(&e, "race").and_then(|r| crate::enums::Race::from_name(&r)),
                    sub_type: attr_str(&e, "subType"),
                    skills: Vec::new(),
                });
            }
            Event::Empty(e) if e.name().as_ref() == "skill" => {
                if let Some(block) = blocks.last_mut() {
                    block.skills.push(new_learn(&e));
                }
            }
            Event::Start(e) if e.name().as_ref() == "skill" => pending = Some(new_learn(&e)),
            Event::End(e) if e.name().as_ref() == "skill" => {
                if let (Some(learn), Some(block)) = (pending.take(), blocks.last_mut()) {
                    block.skills.push(learn);
                }
            }
            Event::Start(e) | Event::Empty(e) if pending.is_some() => {
                let learn = pending.as_mut().expect("checked");
                match e.name().as_ref() {
                    "item" => {
                        if let Some(id) = attr_i32(&e, "id") {
                            learn
                                .required_items
                                .push((id, attr_i64(&e, "count").unwrap_or(1)));
                        }
                    }
                    "preRequisiteSkill" => {
                        if let Some(id) = attr_i32(&e, "id") {
                            learn
                                .pre_req_skills
                                .push((id, attr_i32(&e, "lvl").unwrap_or(0)));
                        }
                    }
                    "removeSkill" => {
                        if let Some(id) = attr_i32(&e, "id") {
                            let only_replace =
                                attr_str(&e, "onlyReplaceByLearn").as_deref() == Some("true");
                            learn.remove_skills.push((id, only_replace));
                        }
                    }
                    name @ ("race" | "residenceId" | "socialClass") => {
                        text_child = Some(name.to_string());
                    }
                    _ => {}
                }
            }
            Event::Text(t) if text_child.is_some() => {
                let Some(learn) = pending.as_mut() else {
                    continue;
                };
                let text = t.into_inner();
                let text = text.trim();
                match text_child.as_deref() {
                    Some("race") => {
                        if let Some(race) = crate::enums::Race::from_name(text) {
                            learn.races.push(race);
                        }
                    }
                    Some("residenceId") => {
                        if let Ok(id) = text.parse() {
                            learn.residence_ids.push(id);
                        }
                    }
                    Some("socialClass") => learn.social_class = Some(text.to_string()),
                    _ => {}
                }
            }
            Event::End(e)
                if matches!(e.name().as_ref(), "race" | "residenceId" | "socialClass") =>
            {
                text_child = None;
            }
            _ => {}
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dist;
    use crate::enums::Race;

    fn learn(skill_id: i32, skill_level: i32, get_level: i32, level_up_sp: i64) -> SkillLearn {
        SkillLearn {
            skill_id,
            skill_level,
            name: String::new(),
            get_level,
            level_up_sp,
            auto_get: false,
            required_items: Vec::new(),
            ..Default::default()
        }
    }

    /// The boot gap report claims that of everything the skill parser drops,
    /// only a named handful sits on a skill a player can actually learn — the
    /// rest is later-chronicle datapack content. That claim is the whole point
    /// of the report, so it is asserted against the real dist rather than left
    /// to the log to state.
    ///
    /// The one survivor is a recorded G34 out-of-scope decision: **42 Sweeper**
    /// (`OpSweeper` — enforced at *apply* time by `effects::sweep` with the
    /// right per-corpse messages; gating the cast too would double them).
    ///
    /// **173 Acrobatics used to be the second**, on the grounds that
    /// `SafeFallHeight` fed a stat with no consumer. Fall damage landed with
    /// `General.ini`'s `EnableFallingDamage`, so the consumer exists
    /// ([`game_loop::space::falling`](crate::game_loop::space::falling)) and the effect is
    /// registered — a reminder that "the stat has no consumer" is a statement
    /// about today, not a decision.
    ///
    /// A new name appearing here means a parser gap became reachable, which is
    /// exactly the moment someone should look — the count alone would not say
    /// which skill or why.
    #[test]
    #[ignore = "TODO(salvation): S7 — Salvation's trees reach ~95 effect names and OpNotAffectedBySkill that the effect layer has no handler for yet (the `*Attack` → `*Damage` renames and the new effects); re-enable once S7 lands"]
    fn only_the_recorded_gap_is_reachable_from_a_skill_tree() {
        let trees = dist::skill_trees();
        let skills = dist::skills();
        let learnable = trees.all_learnable_skill_ids();
        assert!(
            learnable.len() > 500,
            "sanity: the class trees should yield hundreds of ids, got {}",
            learnable.len()
        );

        let mut reachable: Vec<String> = Vec::new();
        for (label, map) in skills.gaps().categories() {
            for (name, ids) in map {
                let mut hit: Vec<i32> = ids
                    .iter()
                    .copied()
                    .filter(|id| learnable.contains(id))
                    .collect();
                hit.sort_unstable();
                if !hit.is_empty() {
                    reachable.push(format!("{label}/{name} {hit:?}"));
                }
            }
        }
        reachable.sort();
        assert_eq!(
            reachable,
            vec!["condition/conditions/OpSweeper [42]".to_string()],
            "the set of parser gaps reachable from a skill tree changed"
        );
    }

    /// `isSkillAllowed`, against the **real dist trees** rather than a
    /// fixture — the point of the check is what the shipped data does and does
    /// not contain, which a synthetic tree cannot tell you.
    ///
    /// Human Fighter (class 0) is the subject. `Power Strike` (3) is its own,
    /// and 8 is a made-up id no tree carries.
    #[test]
    fn is_skill_allowed_accepts_the_class_tree_and_refuses_what_no_tree_grants() {
        let trees = dist::skill_trees();
        let skills = dist::skills();
        let max = |id: i32| skills.max_level(id);

        assert!(
            trees.is_skill_allowed(0, Some(Race::Human), false, 3, 1, max(3)),
            "Power Strike is a Human Fighter class skill"
        );
        assert!(
            !trees.is_skill_allowed(0, Some(Race::Human), false, 1000, 1, max(1000)),
            "a skill no tree on this dist grants is refused"
        );
    }

    /// The race arm. `Expand Dwarven Craft` (1368) is in the **fishing** tree
    /// — a tree this port never teaches — scoped `<race>DWARF</race>`.
    ///
    /// Two things fail without it: a Dwarf who legitimately holds the skill
    /// has it removed, and a Human who could not have it keeps it. Both are
    /// asserted, because a check that allows everything passes the first.
    #[test]
    fn is_skill_allowed_honours_the_race_scope_of_the_fishing_tree() {
        let trees = dist::skill_trees();
        let max = dist::skills().max_level(1368);
        assert!(
            trees.is_skill_allowed(53, Some(Race::Dwarf), false, 1368, 1, max),
            "a Dwarven Fighter may hold Expand Dwarven Craft"
        );
        assert!(
            !trees.is_skill_allowed(0, Some(Race::Human), false, 1368, 1, max),
            "a Human may not — the entry names DWARF"
        );
    }

    /// The race-**less** arm of the same trees (Java's `_allSkillsHashCodes`):
    /// `Expand Storage`/`Expand Inventory` carry no `<race>`, so every race
    /// keeps them. Pinned separately from the scoped arm because collapsing
    /// the two — treating "no races listed" as "no race matches" — refuses
    /// every one of them, and that is the failure that eats real skills.
    #[test]
    fn is_skill_allowed_accepts_race_less_entries_for_everyone() {
        let trees = dist::skill_trees();
        let max = dist::skills().max_level(1369);
        for (class_id, race) in [(0, Race::Human), (53, Race::Dwarf), (18, Race::Elf)] {
            assert!(
                trees.is_skill_allowed(class_id, Some(race), false, 1369, 1, max),
                "{race:?} keeps the race-less fishing entry"
            );
        }
    }

    /// Java clamps to `SkillData.getMaxLevel(id)` before matching, which is
    /// what lets an **enchanted** skill — stored at level 101+ — match the
    /// level-`max` tree entry it was enchanted from. Without the clamp every
    /// enchanted skill on the server fails the check at once.
    #[test]
    fn is_skill_allowed_clamps_an_enchanted_level_to_the_tree_maximum() {
        let trees = dist::skill_trees();
        let max = dist::skills().max_level(3);
        assert!(max > 1, "sanity: Power Strike has several levels");
        assert!(
            trees.is_skill_allowed(0, Some(Race::Human), false, 3, 101, max),
            "an enchanted Power Strike matches its max-level tree entry"
        );
    }

    /// A GM holding a GM-tree skill is allowed outright (Java's second arm),
    /// and the same skill on a non-GM is not.
    #[test]
    fn is_skill_allowed_lets_a_gm_keep_gm_tree_skills() {
        let trees = dist::skill_trees();
        let Some(&(gm_skill, level)) = trees.gm_skills(false).first() else {
            panic!("the GM tree should not be empty");
        };
        let max = dist::skills().max_level(gm_skill);
        assert!(trees.is_skill_allowed(0, Some(Race::Human), true, gm_skill, level, max));
        assert!(!trees.is_skill_allowed(0, Some(Race::Human), false, gm_skill, level, max));
    }

    /// **Hero and noble skills are in no allow-list arm at all**, and that is
    /// correct rather than an oversight: Java grants both with
    /// `addSkill(skill, false)` and never writes them to `character_skills`,
    /// so a row carrying one is exactly the garbage the check exists to find.
    ///
    /// Asserted because it is the most surprising thing about the function,
    /// and because it is only safe while the port re-derives both on the way
    /// in — see `is_hero_or_noble_skill` and `Player::from_char`.
    #[test]
    fn hero_and_noble_skills_are_deliberately_not_allowed() {
        let trees = dist::skill_trees();
        let skills = dist::skills();
        for &(id, level) in trees.hero_skills().iter().chain(trees.noble_skills()) {
            assert!(
                !trees.is_skill_allowed(
                    0,
                    Some(Race::Human),
                    false,
                    id,
                    level,
                    skills.max_level(id)
                ),
                "{id}/{level} must not pass the check as a stored row"
            );
            assert!(
                trees.is_hero_or_noble_skill(id),
                "{id} must be recognised by the persistence filter instead"
            );
        }
    }

    /// `available_skills` for class 0 with every skill capped at level 99,
    /// as sorted `(id, level)` pairs.
    fn avail(data: &SkillTreeData, level: i32, known: &HashMap<i32, i32>) -> Vec<(i32, i32)> {
        let learner = Learner {
            class_id: 0,
            level,
            known,
            awakened: false,
        };
        let mut v: Vec<(i32, i32)> = data
            .available_skills(&learner, |_| 99)
            .iter()
            .map(|s| (s.skill_id, s.skill_level))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn available_skills_gates_by_level_and_next_skill_level_only() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 100));
        data.insert_for_test(0, learn(91, 2, 10, 200));
        data.insert_for_test(0, learn(3, 1, 5, 50));

        // Below the level gate: nothing learnable.
        assert!(avail(&data, 4, &HashMap::new()).is_empty());

        // At the gate: both level-1 entries, not the level-2 (still gated by
        // its own getLevel *and* by not knowing level 1 yet).
        assert_eq!(avail(&data, 5, &HashMap::new()), [(3, 1), (91, 1)]);

        // Already knowing skill 91 at level 1: level 5 no longer offers 91
        // level 1 again (would need level 10 for the next level); skill 3
        // (still unknown) stays available regardless.
        let known = HashMap::from([(91, 1)]);
        assert_eq!(avail(&data, 5, &known), [(3, 1)]);
        assert_eq!(
            avail(&data, 10, &known),
            [(3, 1), (91, 2)],
            "91's next level (getLevel=10) now shows up alongside still-unlearned 3"
        );
    }

    /// `getAvailableSkills`' skips: autoGet and forgotten-scroll entries, the
    /// class's `<removeSkill>` cache, and a level above the skill's highest.
    #[test]
    fn available_skills_skips_auto_get_fs_removed_and_over_max() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(1, 1, 1, 0));
        data.insert_for_test(
            0,
            SkillLearn {
                auto_get: true,
                ..learn(2, 1, 1, 0)
            },
        );
        data.insert_for_test(
            0,
            SkillLearn {
                learned_by_fs: true,
                ..learn(3, 1, 1, 0)
            },
        );
        data.insert_for_test(0, learn(4, 1, 1, 0));
        data.insert_for_test(0, learn(5, 1, 1, 0));
        data.remove_skill_cache.insert(0, HashSet::from([4]));
        let learner = Learner {
            class_id: 0,
            level: 10,
            known: &HashMap::new(),
            awakened: false,
        };
        let ids: Vec<i32> = data
            .available_skills(&learner, |id| if id == 5 { 0 } else { 99 })
            .iter()
            .map(|s| s.skill_id)
            .collect();
        assert_eq!(ids, [1]);
    }

    /// An awakened learner is offered only the class's own entries and its
    /// awakening-save skills — not the parent or common trees'.
    #[test]
    fn awakened_learners_see_only_their_own_tree_and_saved_skills() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(10, learn(100, 1, 1, 0));
        data.insert_for_test(1, learn(200, 1, 1, 0));
        data.insert_for_test(1, learn(300, 1, 1, 0));
        data.set_parent_for_test(10, 1);
        data.awakening_save.insert(10, HashSet::from([300]));
        let known = HashMap::new();
        let ids = |awakened| {
            let learner = Learner {
                class_id: 10,
                level: 10,
                known: &known,
                awakened,
            };
            let mut v: Vec<i32> = data
                .available_skills(&learner, |_| 99)
                .iter()
                .map(|s| s.skill_id)
                .collect();
            v.sort();
            v
        };
        assert_eq!(ids(false), [100, 200, 300]);
        assert_eq!(ids(true), [100, 300]);
    }

    /// `getNextAvailableSkills`: from the lowest `getLevel` above the
    /// learner's, every next level, but nothing past `PLAYER_MAXIMUM_LEVEL`
    /// and nothing at all once no entry lies above them.
    #[test]
    fn next_available_skills_lists_the_levels_still_ahead() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 0));
        data.insert_for_test(0, learn(91, 2, 20, 0));
        data.insert_for_test(0, learn(92, 1, 40, 0));
        data.insert_for_test(0, learn(93, 1, 90, 0));
        let next = |level, known: &HashMap<i32, i32>| {
            let learner = Learner {
                class_id: 0,
                level,
                known,
                awakened: false,
            };
            let mut v: Vec<(i32, i32)> = data
                .next_available_skills(&learner, 86)
                .iter()
                .map(|s| (s.skill_id, s.skill_level))
                .collect();
            v.sort();
            v
        };
        // 91/2 needs 91/1 first; 93 is past the level cap.
        assert_eq!(next(10, &HashMap::new()), [(92, 1)]);
        assert_eq!(next(10, &HashMap::from([(91, 1)])), [(91, 2), (92, 1)]);
        assert!(next(95, &HashMap::new()).is_empty());
    }

    /// The unlearn tail: a known skill whose entry (or level-1 entry) lists a
    /// `<removeSkill>` takes that id out of the offer.
    #[test]
    fn a_known_skills_remove_list_drops_the_offered_entry() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(
            0,
            SkillLearn {
                remove_skills: vec![(7, true)],
                ..learn(50, 1, 1, 0)
            },
        );
        data.insert_for_test(0, learn(50, 2, 1, 0));
        data.insert_for_test(0, learn(7, 1, 1, 0));
        data.insert_for_test(0, learn(8, 1, 1, 0));
        assert_eq!(avail(&data, 10, &HashMap::new()), [(7, 1), (8, 1), (50, 1)]);
        // Knowing 50 at level 2: its own entry has no list, so level 1's counts.
        assert_eq!(avail(&data, 10, &HashMap::from([(50, 2)])), [(8, 1)]);
    }

    #[test]
    fn all_available_skills_returns_highest_reachable_level_per_skill() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 100));
        data.insert_for_test(0, learn(91, 2, 10, 200));
        data.insert_for_test(0, learn(91, 3, 20, 300));
        data.insert_for_test(0, learn(3, 1, 5, 50));
        let auto = SkillLearn {
            auto_get: true,
            ..learn(1000, 1, 1, 0)
        };
        data.insert_for_test(0, auto);

        // Level below any gate: nothing but the level-1 autoGet skill.
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 1, &HashMap::new(), true, true);
        got.sort();
        assert_eq!(got, vec![(1000, 1)]);

        // At level 10, skill 91 jumps straight to its level-2 max (not 1), plus
        // skill 3 and the autoGet — all in one pass (Java's holder loop).
        let mut got: Vec<(i32, i32)> =
            data.all_available_skills(0, 10, &HashMap::new(), true, true);
        got.sort();
        assert_eq!(got, vec![(3, 1), (91, 2), (1000, 1)]);

        // Already knowing 91 at level 2 and the autoGet: only the still-missing
        // skill 3, and — at level 20 — 91's level-3 upgrade.
        let mut known = HashMap::new();
        known.insert(91, 2);
        known.insert(1000, 1);
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 20, &known, true, true);
        got.sort();
        assert_eq!(got, vec![(3, 1), (91, 3)]);
    }

    #[test]
    fn delevel_skill_changes_downgrades_or_removes_past_the_grace() {
        let mut data = SkillTreeData::empty();
        // Skill 91: level 1 @ getLevel 20, level 2 @ getLevel 40.
        data.insert_for_test(0, learn(91, 1, 20, 100));
        data.insert_for_test(0, learn(91, 2, 40, 200));
        // Expertise (239): level 1 @ getLevel 20 — checked with no 9-lvl grace.
        data.insert_for_test(0, learn(EXPERTISE_SKILL_ID, 1, 20, 0));

        let known = |pairs: &[(i32, i32)]| pairs.iter().copied().collect::<HashMap<i32, i32>>();

        // Within the grace (skill-2 getLevel 40, level 31 ≥ 40-9): keep.
        assert!(
            data.delevel_skill_changes(0, 31, &known(&[(91, 2)]))
                .is_empty()
        );

        // Below grace for level 2 (30 < 40-9) but still fine for level 1
        // (30 ≥ 20-9): downgrade 91 to level 1.
        assert_eq!(
            data.delevel_skill_changes(0, 30, &known(&[(91, 2)])),
            vec![(91, Some(1))]
        );

        // Below grace even for level 1 (10 < 20-9): remove 91 entirely.
        assert_eq!(
            data.delevel_skill_changes(0, 10, &known(&[(91, 2)])),
            vec![(91, None)]
        );

        // Expertise has no grace: at level 19 (< 20)
        // it's removed, even though a 9-level grace would have kept it.
        assert_eq!(
            data.delevel_skill_changes(0, 19, &known(&[(EXPERTISE_SKILL_ID, 1)])),
            vec![(EXPERTISE_SKILL_ID, None)]
        );
        assert!(
            data.delevel_skill_changes(0, 20, &known(&[(EXPERTISE_SKILL_ID, 1)]))
                .is_empty()
        );

        // A skill not in this class tree is left untouched.
        assert!(
            data.delevel_skill_changes(0, 1, &known(&[(777, 5)]))
                .is_empty()
        );
    }

    /// `complete_entries` (Java `getCompleteClassSkillTree`) unions the class's
    /// own tree with every ancestor's (via `parentClassId`) and the common tree,
    /// so a query on a derived class reaches inherited + common skills.
    #[test]
    fn complete_entries_walk_parents_and_common() {
        let mut data = SkillTreeData::empty();
        // 3 (child) → 1 (parent) → 0 (grandparent); plus a common skill.
        data.insert_for_test(0, learn(10, 1, 5, 100)); // grandparent skill
        data.insert_for_test(1, learn(20, 1, 20, 100)); // parent skill
        data.insert_for_test(3, learn(30, 1, 40, 100)); // child skill
        data.set_parent_for_test(3, 1);
        data.set_parent_for_test(1, 0);
        data.common.push(learn(999, 1, 1, 0));

        let mut ids: Vec<i32> = data
            .all_available_skills(3, 40, &HashMap::new(), true, true)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![10, 20, 30, 999],
            "child reaches parent + grandparent + common skills"
        );

        // The parent (class 1) reaches only its own + grandparent + common, not
        // the child's skill 30.
        let mut pids: Vec<i32> = data
            .all_available_skills(1, 40, &HashMap::new(), true, true)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        pids.sort();
        assert_eq!(pids, vec![10, 20, 999]);
    }

    /// The auto-learn path (`all_available_skills`) withholds book-gated skills
    /// unless `include_required_items`, and Divine Inspiration (1405) unless
    /// `include_divine_inspiration` — matching Java's `giveAvailableSkills` gates.
    #[test]
    fn all_available_skills_gates_item_and_divine_inspiration() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(50, 1, 5, 100)); // ordinary skill
        data.insert_for_test(
            0,
            SkillLearn {
                required_items: vec![(8618, 1)],
                ..learn(60, 1, 5, 100)
            },
        ); // book-gated
        data.insert_for_test(0, learn(DIVINE_INSPIRATION_SKILL_ID, 1, 5, 100)); // Divine Inspiration

        let ids = |items: bool, di: bool| {
            let mut v: Vec<i32> = data
                .all_available_skills(0, 40, &HashMap::new(), items, di)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            v.sort();
            v
        };
        // Dist config (both on): everything.
        assert_eq!(ids(true, true), vec![50, 60, DIVINE_INSPIRATION_SKILL_ID]);
        // No required items: drop the book-gated 60 (DI is also item-gated in the
        // real data, but here 1405 has no item child, so its own flag governs).
        assert_eq!(ids(false, true), vec![50, DIVINE_INSPIRATION_SKILL_ID]);
        // No Divine Inspiration: drop 1405.
        assert_eq!(ids(true, false), vec![50, 60]);
        // Neither: only the ordinary skill.
        assert_eq!(ids(false, false), vec![50]);
    }

    /// Against the real datapack: a 2nd-class Warlord (3) inherits its Warrior
    /// (1) / Human Fighter (0) ancestors and the common tree, so it can learn
    /// its own Whirlwind (36), an ancestor skill, and the common Expertise (239).
    #[test]
    fn dist_warlord_inherits_ancestor_and_common_skills() {
        let data = dist::skill_trees();
        let warlord: HashMap<i32, i32> = data
            .all_available_skills(3, 80, &HashMap::new(), true, true)
            .into_iter()
            .collect();
        assert!(warlord.contains_key(&36), "Warlord's own Whirlwind (36)");
        assert!(
            warlord.contains_key(&239),
            "common Expertise (239) inherited by every class"
        );
        // Inheritance: the Human Fighter (0) ancestor tree is a subset reachable
        // through the parent chain, so Warlord strictly out-reaches it.
        let base: HashMap<i32, i32> = data
            .all_available_skills(0, 80, &HashMap::new(), true, true)
            .into_iter()
            .collect();
        assert!(
            base.keys().all(|id| warlord.contains_key(id)),
            "Warlord inherits every Human Fighter skill"
        );
        assert!(
            warlord.len() > base.len(),
            "Warlord adds its own tier on top ({} vs {})",
            warlord.len(),
            base.len()
        );
    }

    /// Against the real datapack: the `<item>` children parse into
    /// `required_items`. In Salvation's class trees every one is the same cost
    /// — Rune Stone (39738) ×4, on the level-99 4th-class entries — and Divine
    /// Inspiration lost its Ancient Books: levels 1–4 are free, and only the
    /// awakened levels 5 and 6 (Wynn Spectral Master, 178) take Rune Stones.
    #[test]
    fn dist_class_tree_items_are_rune_stones() {
        const RUNE_STONE: i32 = 39738;
        let data = dist::skill_trees();
        let warlord_di: Vec<&SkillLearn> = data
            .complete_entries(3)
            .into_iter()
            .filter(|s| s.skill_id == DIVINE_INSPIRATION_SKILL_ID)
            .collect();
        assert!(
            !warlord_di.is_empty(),
            "Divine Inspiration is in the Warlord tree"
        );
        assert!(
            warlord_di.iter().all(|s| !s.requires_item()),
            "no Ancient Book on any Warlord level"
        );

        let wynn_di: Vec<(i32, Vec<(i32, i64)>)> = data
            .complete_entries(178)
            .into_iter()
            .filter(|s| s.skill_id == DIVINE_INSPIRATION_SKILL_ID && s.requires_item())
            .map(|s| (s.skill_level, s.required_items.clone()))
            .collect();
        let mut wynn_di = wynn_di;
        wynn_di.sort();
        assert_eq!(
            wynn_di,
            vec![(5, vec![(RUNE_STONE, 4)]), (6, vec![(RUNE_STONE, 4)])]
        );

        for class_id in data.trees.keys().copied().collect::<Vec<_>>() {
            for entry in data.complete_entries(class_id) {
                assert!(
                    entry.required_items.iter().all(|&i| i == (RUNE_STONE, 4)),
                    "class {class_id} skill {} ({}) costs {:?}",
                    entry.skill_id,
                    entry.name,
                    entry.required_items
                );
            }
        }
    }

    /// The Salvation-only trees load by `type`, wherever their file sits:
    /// `raceSkillTree race="HUMAN"`, the `transferSkillTree classId="97"`
    /// block, `revelationSkillTree subType="BASECLASS"`, the ability tree's
    /// layout attributes, and the 4th-class parent chain (Wynn Spectral Master
    /// 178 → Spectral Master 111, `parentClassId="111"`).
    #[test]
    fn dist_salvation_trees_load_by_type() {
        let data = dist::skill_trees();
        assert!(!data.race_trees[&crate::enums::Race::Human.ordinal()].is_empty());
        assert!(data.transfer.contains_key(&97));
        assert!(data.revelation.contains_key("BASECLASS"));
        assert!(data.revelation.contains_key("DUALCLASS"));
        assert_eq!(data.ability.len(), 123);
        assert!(data.ability.iter().all(|l| l.tree_id > 0));
        assert!(!data.alchemy.is_empty());
        assert!(!data.dual_class.is_empty());
        assert!(!data.awakening_save.is_empty());
        assert_eq!(data.parent_class(178), Some(111));
    }
}
