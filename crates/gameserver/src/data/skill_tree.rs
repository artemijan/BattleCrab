//! Port of `data/xml/SkillTreeData` — the class skill trees driving character
//! creation (`initNewChar`), `RequestAcquireSkill`'s `CLASS` learn path, and
//! `Player.rewardSkills` / `checkPlayerSkills` on level and class change.
//!
//! The `classSkillTree` entries live in four directories keyed by class tier
//! (`StartingClass/`, `1stClass/`, `2ndClass/`, `3rdClass/`), each `<skillTree>`
//! carrying its `classId` and (for non-base classes) a `parentClassId`. Java's
//! `getCompleteClassSkillTree` walks that parent chain and unions in the
//! class-agnostic `Commons.xml` tree, so a Warlord (3 → Warrior 1 → Human
//! Fighter 0) reaches its own skills *and* every ancestor's plus the common
//! ones (Lucky, Expertise, …). [`SkillTreeData::complete_entries`] reproduces
//! that union; all the per-class queries below run through it.
//!
//! The only `<skill>` child present is `<item>` (a book required to learn some
//! 2nd/3rd-class skills, e.g. Divine Inspiration) — flagged as `requires_item`
//! and honored by the auto-learn gates (`AutoLearnSkillsWithoutItems`,
//! `AutoLearnDivineInspiration`). Parsing the item id/count for the manual
//! learn path (cost display + consumption) is still TODO(G6); no `preReqSkill`,
//! `learnedByFS`, or `removeSkill` entries exist in these trees, so those Java
//! code paths stay out of scope — every entry is gated by `getLevel`/`levelUpSp`
//! (plus the optional book) alone.

use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::info;

pub const SKILL_TREES_DIR: &str = "data/skillTrees";
/// The per-tier subdirectories of `data/skillTrees` holding `classSkillTree`
/// files, most-derived last (order is irrelevant — each entry carries its own
/// `classId`/`parentClassId`).
const CLASS_TREE_SUBDIRS: [&str; 4] = ["StartingClass", "1stClass", "2ndClass", "3rdClass"];
/// The class-agnostic tree unioned into every class (Java `_commonSkillTree`).
const COMMON_TREE_FILE: &str = "Commons.xml";
pub const HERO_SKILL_TREE_FILE: &str = "data/skillTrees/heroSkillTree.xml";
pub const NOBLE_SKILL_TREE_FILE: &str = "data/skillTrees/nobleSkillTree.xml";

/// Java `CommonSkill.EXPERTISE` (239): the one skill `checkPlayerSkills`
/// verifies with no level grace — its level *is* the wearable grade, so it may
/// not outrank the character even by the usual 9 levels.
const EXPERTISE_SKILL_ID: i32 = 239;

/// Java `CommonSkill.DIVINE_INSPIRATION` (1405): an item-gated class skill the
/// auto-learn path withholds unless `AutoLearnDivineInspiration` is set.
const DIVINE_INSPIRATION_SKILL_ID: i32 = 1405;

/// A skill a character knows: `(skill_id, skill_level)`.
pub type Skill = (i32, i32);

/// One `<skill>` entry from a class tree (Java `SkillLearn`, trimmed to the
/// fields the `CLASS` learn path needs).
#[derive(Debug, Clone)]
pub struct SkillLearn {
    pub skill_id: i32,
    pub skill_level: i32,
    pub name: String,
    /// Minimum character level to learn this entry (`getLevel` attribute).
    pub get_level: i32,
    /// SP cost (0 for autoGet skills, which are granted free at creation).
    pub level_up_sp: i64,
    pub auto_get: bool,
    /// True when the `<skill>` carries an `<item>` child, i.e. learning it also
    /// consumes a book (Java `SkillLearn.getRequiredItems`). The `AutoLearnSkills`
    /// path skips these unless `AutoLearnSkillsWithoutItems` is set; the manual
    /// `RequestAcquireSkill` path still lists them (item cost enforced there).
    pub requires_item: bool,
}

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
}

impl SkillTreeData {
    pub fn load() -> Self {
        Self::load_from("")
    }
    pub fn load_from(file_path: &str) -> Self {
        let mut trees: HashMap<i32, Vec<SkillLearn>> = HashMap::new();
        let mut parents: HashMap<i32, i32> = HashMap::new();
        let mut common: Vec<SkillLearn> = Vec::new();
        for sub in CLASS_TREE_SUBDIRS {
            let Ok(dir) = std::fs::read_dir(format!("{file_path}{SKILL_TREES_DIR}/{sub}")) else { continue };
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                parse_tree(&path, &mut trees, &mut parents, &mut common);
            }
        }
        // `Commons.xml` is a `classSkillTree` with no `classId` → the common tree.
        parse_tree(
            std::path::Path::new(&format!("{file_path}{SKILL_TREES_DIR}/{COMMON_TREE_FILE}")),
            &mut trees,
            &mut parents,
            &mut common,
        );
        let hero_skills = parse_hero_tree(&format!("{file_path}{HERO_SKILL_TREE_FILE}"));
        // Same flat `<skill id level/>` shape as the hero tree.
        let noble_skills = parse_hero_tree(&format!("{file_path}{NOBLE_SKILL_TREE_FILE}"));
        let total: usize = trees.values().map(|v| v.len()).sum();
        info!(
            "SkillTreeData: Loaded skill trees for {} classes ({total} skill levels), {} common + {} hero skills.",
            trees.len(),
            common.len(),
            hero_skills.len()
        );
        info!("SkillTreeData: Loaded {} noble skills.", noble_skills.len());
        Self { trees, parents, common, hero_skills, noble_skills }
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
        for _ in 0..CLASS_TREE_SUBDIRS.len() + 1 {
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

    /// Java `SkillTreeData.getNobleSkillTree`.
    pub fn noble_skills(&self) -> &[Skill] {
        &self.noble_skills
    }

    /// The skills a freshly created character of `class_id` starts with
    /// (Java `getAvailableSkills(..., includeAutoGet=true)` at level 1).
    pub fn initial_skills(&self, class_id: i32) -> Vec<Skill> {
        self.trees
            .get(&class_id)
            .map(|entries| entries.iter().filter(|s| s.auto_get).map(|s| (s.skill_id, s.skill_level)).collect())
            .unwrap_or_default()
    }

    /// Java `SkillTreeData.getAvailableSkills` narrowed to `AcquireSkillType.CLASS`:
    /// every entry whose `getLevel` the player has reached and whose
    /// `skillLevel` is exactly one past what they currently know (0 for an
    /// unknown skill, so `skillLevel == 1` is "new"). autoGet entries are
    /// already granted at creation, so they never reappear here once known.
    pub fn available_skills(&self, class_id: i32, level: i32, known: &HashMap<i32, i32>) -> Vec<&SkillLearn> {
        self.complete_entries(class_id)
            .into_iter()
            .filter(|s| !s.auto_get && s.get_level <= level)
            .filter(|s| s.skill_level == known.get(&s.skill_id).copied().unwrap_or(0) + 1)
            .collect()
    }

    /// Java `Player.rewardSkills` → `getAvailableAutoGetSkills`: every autoGet
    /// entry reachable at `level`. The caller keeps only levels above what the
    /// player already knows.
    pub fn auto_get_skills(&self, class_id: i32, level: i32) -> Vec<&SkillLearn> {
        self.complete_entries(class_id).into_iter().filter(|s| s.auto_get && s.get_level <= level).collect()
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
        for s in self.complete_entries(class_id).into_iter().filter(|s| s.get_level <= level) {
            if !include_required_items && s.requires_item {
                continue;
            }
            if s.skill_id == DIVINE_INSPIRATION_SKILL_ID && !include_divine_inspiration {
                continue;
            }
            let slot = best.entry(s.skill_id).or_insert(0);
            *slot = (*slot).max(s.skill_level);
        }
        best.into_iter().filter(|&(id, lvl)| lvl > known.get(&id).copied().unwrap_or(0)).collect()
    }

    /// The `SkillLearn` for a specific `(class_id, skill_id, skill_level)`,
    /// used by `RequestAcquireSkill` to re-validate the client's request.
    pub fn skill_learn(&self, class_id: i32, skill_id: i32, skill_level: i32) -> Option<&SkillLearn> {
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
    /// `strict` drops the 9-level grace Java normally applies (`StrictDelevel-
    /// SkillRemoval`): when true, every skill is matched level-exactly (the same
    /// no-grace rule Java always uses for Expertise); when false, the ordinary
    /// 9-level buffer applies (0 for Expertise).
    pub fn delevel_skill_changes(&self, class_id: i32, level: i32, known: &HashMap<i32, i32>, strict: bool) -> Vec<(i32, Option<i32>)> {
        let entries = self.complete_entries(class_id);
        if entries.is_empty() {
            return Vec::new();
        }
        let level_diff_of = |skill_id: i32| if strict || skill_id == EXPERTISE_SKILL_ID { 0 } else { 9 };
        let mut out = Vec::new();
        for (&skill_id, &skill_level) in known {
            // Java keys the lookup on `getLevel() % 100` — enchanted skills
            // carry the enchant route in the hundreds digit.
            let base_level = skill_level % 100;
            let Some(learn) = entries.iter().find(|s| s.skill_id == skill_id && s.skill_level == base_level) else {
                continue;
            };
            let level_diff = level_diff_of(skill_id);
            if level >= (learn.get_level - level_diff) {
                continue; // still within range — keep as is
            }
            // deacreaseSkillLevel: highest level of this skill still reachable.
            let mut next = -1;
            for s in entries.iter() {
                if s.skill_id == skill_id && s.skill_level > next && level >= (s.get_level - level_diff) {
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
        Self { trees: HashMap::new(), parents: HashMap::new(), common: Vec::new(), hero_skills: Vec::new(), noble_skills: Vec::new() }
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

/// Parse one `classSkillTree` file into `out` (per class), `parents`
/// (`classId → parentClassId`), and `common` (entries with no `classId`, i.e.
/// `Commons.xml`). Non-`classSkillTree` blocks (hero/noble/pledge/…) are
/// skipped — those live in their own Java maps outside the class tree.
fn parse_tree(
    path: &std::path::Path,
    out: &mut HashMap<i32, Vec<SkillLearn>>,
    parents: &mut HashMap<i32, i32>,
    common: &mut Vec<SkillLearn>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let mut reader = Reader::from_str(&content);
    // The class the current `<skillTree>` applies to: `Some(id)` for a per-class
    // tree, `None` for the common tree; `is_class_tree` gates non-class blocks.
    let mut current_class: Option<i32> = None;
    let mut is_class_tree = false;
    loop {
        // `has_children` distinguishes `<skill …>…</skill>` (an `<item>`
        // requirement, the only child these trees carry) from self-closing
        // `<skill …/>` — see the required-item note on `SkillLearn`.
        let (e, has_children) = match reader.read_event() {
            Ok(Event::Start(e)) => (e, true),
            Ok(Event::Empty(e)) => (e, false),
            Ok(Event::Eof) | Err(_) => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"skillTree" => {
                is_class_tree = attr_str(&e, b"type").as_deref() == Some("classSkillTree");
                current_class = attr_i32(&e, b"classId");
                // Java `_parentClassMap`: record a real, distinct parent.
                if let (Some(cid), Some(parent)) = (current_class, attr_i32(&e, b"parentClassId")) {
                    if cid > -1 && parent > -1 && parent != cid {
                        parents.entry(cid).or_insert(parent);
                    }
                }
            }
            b"skill" if is_class_tree => {
                let skill_id = attr_i32(&e, b"skillId").unwrap_or(-1);
                let skill_level = attr_i32(&e, b"skillLevel").unwrap_or(0);
                let get_level = attr_i32(&e, b"getLevel").unwrap_or(99);
                let level_up_sp = attr_i64(&e, b"levelUpSp").unwrap_or(0);
                let name = attr_str(&e, b"skillName").unwrap_or_default();
                let auto_get = attr_str(&e, b"autoGet").as_deref() == Some("true") || (get_level <= 1 && skill_level == 1);
                if skill_id > 0 {
                    let learn = SkillLearn {
                        skill_id,
                        skill_level,
                        name,
                        get_level,
                        level_up_sp,
                        auto_get,
                        requires_item: has_children,
                    };
                    match current_class {
                        Some(class_id) => out.entry(class_id).or_default().push(learn),
                        None => common.push(learn),
                    }
                }
            }
            _ => {}
        }
    }
}

/// Parse `heroSkillTree.xml` (Java `getHeroSkillTree`): a flat `<skill>` list.
fn parse_hero_tree(path: &str) -> Vec<Skill> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut reader = Reader::from_str(&content);
    let mut out = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"skill" => {
                let id = attr_i32(&e, b"skillId").unwrap_or(-1);
                let level = attr_i32(&e, b"skillLevel").unwrap_or(1);
                if id > 0 {
                    out.push((id, level));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    out
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn attr_i32(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i32> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

fn attr_i64(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<i64> {
    attr_str(e, key).and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learn(skill_id: i32, skill_level: i32, get_level: i32, level_up_sp: i64) -> SkillLearn {
        SkillLearn { skill_id, skill_level, name: String::new(), get_level, level_up_sp, auto_get: false, requires_item: false }
    }

    #[test]
    fn available_skills_gates_by_level_and_next_skill_level_only() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 100));
        data.insert_for_test(0, learn(91, 2, 10, 200));
        data.insert_for_test(0, learn(3, 1, 5, 50));

        // Below the level gate: nothing learnable.
        assert!(data.available_skills(0, 4, &HashMap::new()).is_empty());

        // At the gate: both level-1 entries, not the level-2 (still gated by
        // its own getLevel *and* by not knowing level 1 yet).
        let known = HashMap::new();
        let mut ids: Vec<i32> = data.available_skills(0, 5, &known).iter().map(|s| s.skill_id).collect();
        ids.sort();
        assert_eq!(ids, vec![3, 91]);

        // Already knowing skill 91 at level 1: level 5 no longer offers 91
        // level 1 again (would need level 10 for the next level); skill 3
        // (still unknown) stays available regardless.
        let mut known = HashMap::new();
        known.insert(91, 1);
        let ids_at_5: Vec<i32> = data.available_skills(0, 5, &known).iter().map(|s| s.skill_id).collect();
        assert_eq!(ids_at_5, vec![3]);
        let mut ids_at_10: Vec<i32> = data.available_skills(0, 10, &known).iter().map(|s| s.skill_id).collect();
        ids_at_10.sort();
        assert_eq!(ids_at_10, vec![3, 91], "91's next level (getLevel=10) now shows up alongside still-unlearned 3");
    }

    #[test]
    fn all_available_skills_returns_highest_reachable_level_per_skill() {
        let mut data = SkillTreeData::empty();
        data.insert_for_test(0, learn(91, 1, 5, 100));
        data.insert_for_test(0, learn(91, 2, 10, 200));
        data.insert_for_test(0, learn(91, 3, 20, 300));
        data.insert_for_test(0, learn(3, 1, 5, 50));
        let auto = SkillLearn { auto_get: true, ..learn(1000, 1, 1, 0) };
        data.insert_for_test(0, auto);

        // Level below any gate: nothing but the level-1 autoGet skill.
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 1, &HashMap::new(), true, true);
        got.sort();
        assert_eq!(got, vec![(1000, 1)]);

        // At level 10, skill 91 jumps straight to its level-2 max (not 1), plus
        // skill 3 and the autoGet — all in one pass (Java's holder loop).
        let mut got: Vec<(i32, i32)> = data.all_available_skills(0, 10, &HashMap::new(), true, true);
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

        // Non-strict (Java-faithful) 9-level grace below.
        // Within the grace (skill-2 getLevel 40, level 31 ≥ 40-9): keep.
        assert!(data.delevel_skill_changes(0, 31, &known(&[(91, 2)]), false).is_empty());

        // Below grace for level 2 (30 < 40-9) but still fine for level 1
        // (30 ≥ 20-9): downgrade 91 to level 1.
        assert_eq!(data.delevel_skill_changes(0, 30, &known(&[(91, 2)]), false), vec![(91, Some(1))]);

        // Below grace even for level 1 (10 < 20-9): remove 91 entirely.
        assert_eq!(data.delevel_skill_changes(0, 10, &known(&[(91, 2)]), false), vec![(91, None)]);

        // Expertise has no grace even in non-strict mode: at level 19 (< 20)
        // it's removed, even though a 9-level grace would have kept it.
        assert_eq!(
            data.delevel_skill_changes(0, 19, &known(&[(EXPERTISE_SKILL_ID, 1)]), false),
            vec![(EXPERTISE_SKILL_ID, None)]
        );
        assert!(data.delevel_skill_changes(0, 20, &known(&[(EXPERTISE_SKILL_ID, 1)]), false).is_empty());

        // A skill not in this class tree is left untouched.
        assert!(data.delevel_skill_changes(0, 1, &known(&[(777, 5)]), false).is_empty());
    }

    #[test]
    fn delevel_skill_changes_strict_matches_level_exactly() {
        let mut data = SkillTreeData::empty();
        // Skill 91: level 1 @ getLevel 20, level 2 @ getLevel 40.
        data.insert_for_test(0, learn(91, 1, 20, 100));
        data.insert_for_test(0, learn(91, 2, 40, 200));
        let known = |pairs: &[(i32, i32)]| pairs.iter().copied().collect::<HashMap<i32, i32>>();

        // Strict mode drops the 9-level grace: at level 31, skill 91 @ level 2
        // (getLevel 40) is out of range (31 < 40), so it downgrades to level 1
        // (31 ≥ 20) — where non-strict keeps it (31 ≥ 40 − 9).
        assert!(data.delevel_skill_changes(0, 31, &known(&[(91, 2)]), false).is_empty());
        assert_eq!(data.delevel_skill_changes(0, 31, &known(&[(91, 2)]), true), vec![(91, Some(1))]);

        // At level 19 (< 20) even level 1 is out of range → removed.
        assert_eq!(data.delevel_skill_changes(0, 19, &known(&[(91, 2)]), true), vec![(91, None)]);

        // At level 40 the skill is exactly in range → no change.
        assert!(data.delevel_skill_changes(0, 40, &known(&[(91, 2)]), true).is_empty());

        // Real HumanMystic case that non-strict keeps but strict strips: Wind
        // Strike @ level 3 has getLevel 7; a level-1 char in strict mode
        // downgrades it to the highest reachable level (its autoGet level 1).
        data.insert_for_test(10, SkillLearn { auto_get: true, ..learn(1177, 1, 1, 0) });
        data.insert_for_test(10, learn(1177, 2, 7, 240));
        data.insert_for_test(10, learn(1177, 3, 7, 240));
        assert!(data.delevel_skill_changes(10, 1, &known(&[(1177, 3)]), false).is_empty());
        assert_eq!(data.delevel_skill_changes(10, 1, &known(&[(1177, 3)]), true), vec![(1177, Some(1))]);
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

        let mut ids: Vec<i32> = data.all_available_skills(3, 40, &HashMap::new(), true, true).into_iter().map(|(id, _)| id).collect();
        ids.sort();
        assert_eq!(ids, vec![10, 20, 30, 999], "child reaches parent + grandparent + common skills");

        // The parent (class 1) reaches only its own + grandparent + common, not
        // the child's skill 30.
        let mut pids: Vec<i32> = data.all_available_skills(1, 40, &HashMap::new(), true, true).into_iter().map(|(id, _)| id).collect();
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
        data.insert_for_test(0, SkillLearn { requires_item: true, ..learn(60, 1, 5, 100) }); // book-gated
        data.insert_for_test(0, learn(DIVINE_INSPIRATION_SKILL_ID, 1, 5, 100)); // Divine Inspiration

        let ids = |items: bool, di: bool| {
            let mut v: Vec<i32> =
                data.all_available_skills(0, 40, &HashMap::new(), items, di).into_iter().map(|(id, _)| id).collect();
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
        const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
        let data = SkillTreeData::load_from(DIST);
        let warlord: HashMap<i32, i32> =
            data.all_available_skills(3, 80, &HashMap::new(), true, true).into_iter().collect();
        assert!(warlord.contains_key(&36), "Warlord's own Whirlwind (36)");
        assert!(warlord.contains_key(&239), "common Expertise (239) inherited by every class");
        // Inheritance: the Human Fighter (0) ancestor tree is a subset reachable
        // through the parent chain, so Warlord strictly out-reaches it.
        let base: HashMap<i32, i32> = data.all_available_skills(0, 80, &HashMap::new(), true, true).into_iter().collect();
        assert!(base.keys().all(|id| warlord.contains_key(id)), "Warlord inherits every Human Fighter skill");
        assert!(warlord.len() > base.len(), "Warlord adds its own tier on top ({} vs {})", warlord.len(), base.len());
    }
}
