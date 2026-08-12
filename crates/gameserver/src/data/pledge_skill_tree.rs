//! Port of `SkillTreeData`'s pledge (clan) skill trees — `pledgeSkillTree.xml`
//! and `subPledgeSkillTree.xml`. Drives the `//give_clan_skills` /
//! `//give_all_clan_skills` admin path ([`PledgeSkillTreeData::max_pledge_skills`],
//! Java `getMaxPledgeSkills`) and the per-member social-class gate applied when
//! a clan skill is granted / a member logs in ([`PledgeSkillTreeData::
//! social_class_of`], Java `getPledgeSkill`/`getSubPledgeSkill`).

use crate::data::xml;
use crate::data::xml::{attr_i32, attr_str};
use std::collections::HashMap;

use quick_xml::events::Event;
use tracing::info;

pub const PLEDGE_SKILL_TREE_FILE: &str = "data/skillTrees/pledgeSkillTree.xml";
pub const SUB_PLEDGE_SKILL_TREE_FILE: &str = "data/skillTrees/subPledgeSkillTree.xml";

/// One `<skill>` entry in a pledge/sub-pledge tree (Java `SkillLearn`, narrowed
/// to the clan-skill fields).
#[derive(Debug, Clone)]
pub struct PledgeSkillLearn {
    pub skill_id: i32,
    pub skill_level: i32,
    /// Clan level required to learn it (Java `getGetLevel`).
    pub get_level: i32,
    /// The `<socialClass>` ordinal gating which members receive the skill (Java
    /// `SocialClass.ordinal()`); `None` when the entry carries no socialClass —
    /// then every member gets it (`skillLearn.getSocialClass() == null`).
    pub social_class: Option<u8>,
    /// Java `isResidencialSkill` — castle/clan-hall skills, excluded from
    /// `getMaxPledgeSkills` and granted through residence ownership instead.
    pub residencial: bool,
    /// Java `SkillLearn._residenceIds` — the residences (castle/clan-hall ids)
    /// whose owner-clan members receive this residential skill.
    pub residence_ids: Vec<i32>,
    /// Java `getLevelUpSp` — for pledge skills this is the **clan reputation**
    /// cost the leader pays at the village master (`levelUpSp` attribute).
    pub level_up_sp: i64,
}

#[derive(Clone)]
pub struct PledgeSkillTreeData {
    /// Java `_pledgeSkillTree`.
    pledge: Vec<PledgeSkillLearn>,
    /// Java `_subPledgeSkillTree`.
    sub_pledge: Vec<PledgeSkillLearn>,
}

impl PledgeSkillTreeData {
    pub fn load() -> Self {
        Self::load_from("")
    }

    pub fn load_from(file_path: &str) -> Self {
        let pledge = parse(
            &format!("{file_path}{PLEDGE_SKILL_TREE_FILE}"),
            "pledgeSkillTree",
        );
        let sub_pledge = parse(
            &format!("{file_path}{SUB_PLEDGE_SKILL_TREE_FILE}"),
            "subPledgeSkillTree",
        );
        info!(
            "PledgeSkillTreeData: Loaded {} pledge + {} sub-pledge skill levels.",
            pledge.len(),
            sub_pledge.len()
        );
        Self { pledge, sub_pledge }
    }

    #[doc(hidden)]
    pub fn empty() -> Self {
        Self {
            pledge: Vec::new(),
            sub_pledge: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert_for_test(&mut self, learn: PledgeSkillLearn, squad: bool) {
        if squad {
            self.sub_pledge.push(learn);
        } else {
            self.pledge.push(learn);
        }
    }

    /// Java `getMaxPledgeSkills(clan, includeSquad)`: for every pledge (and, when
    /// `include_squad`, sub-pledge) skill the clan qualifies for at its level and
    /// doesn't already have at least at that level, the highest reachable level
    /// per skill id. `current` is the clan's known `(skill_id → level)` set.
    /// Residential skills are excluded from the pledge tree (as in Java).
    pub fn max_pledge_skills(
        &self,
        clan_level: i32,
        current: &HashMap<i32, i32>,
        include_squad: bool,
    ) -> Vec<(i32, i32)> {
        let mut result: HashMap<i32, i32> = HashMap::new();
        for learn in &self.pledge {
            if learn.residencial || clan_level < learn.get_level {
                continue;
            }
            if current.get(&learn.skill_id).copied().unwrap_or(0) < learn.skill_level {
                let slot = result.entry(learn.skill_id).or_insert(0);
                *slot = (*slot).max(learn.skill_level);
            }
        }
        if include_squad {
            for learn in &self.sub_pledge {
                if clan_level < learn.get_level {
                    continue;
                }
                if current.get(&learn.skill_id).copied().unwrap_or(0) < learn.skill_level {
                    let slot = result.entry(learn.skill_id).or_insert(0);
                    *slot = (*slot).max(learn.skill_level);
                }
            }
        }
        result.into_iter().collect()
    }

    /// Java `SkillTreeData.getAvailablePledgeSkills(clan)`: the next learnable
    /// level of every non-residence pledge skill the clan qualifies for at its
    /// level — the known level + 1, or level 1 for an unknown skill.
    pub fn available_pledge_skills(
        &self,
        clan_level: i32,
        current: &HashMap<i32, i32>,
    ) -> Vec<PledgeSkillLearn> {
        self.pledge
            .iter()
            .filter(|l| !l.residencial && clan_level >= l.get_level)
            .filter(|l| current.get(&l.skill_id).copied().unwrap_or(0) + 1 == l.skill_level)
            .cloned()
            .collect()
    }

    /// Java `SkillTreeData.getAvailableResidentialSkills(residenceId)` — every
    /// residential skill (id, level, socialClass gate) a residence grants its
    /// owner-clan members.
    pub fn available_residential_skills(&self, residence_id: i32) -> Vec<&PledgeSkillLearn> {
        self.pledge
            .iter()
            .filter(|l| l.residencial && l.residence_ids.contains(&residence_id))
            .collect()
    }

    /// Java `getPledgeSkill(id, lvl)` — one pledge-tree entry (the learn
    /// request's validation + reputation cost).
    pub fn pledge_skill(&self, skill_id: i32, skill_level: i32) -> Option<&PledgeSkillLearn> {
        self.pledge
            .iter()
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
    }

    /// Java `addSkillEffects`'s per-skill gate: the `<socialClass>` ordinal a
    /// member must reach (as `pledgeClass + 1 >= ordinal`) to receive clan skill
    /// `(id, level)`. `None` = no gate (every member gets it) — covering both an
    /// entry with no `<socialClass>` and a skill absent from the trees (Java's
    /// `skillLearn == null || getSocialClass() == null`). Searched in the pledge
    /// tree first, then the sub-pledge tree.
    pub fn social_class_of(&self, skill_id: i32, skill_level: i32) -> Option<u8> {
        self.pledge
            .iter()
            .chain(self.sub_pledge.iter())
            .find(|l| l.skill_id == skill_id && l.skill_level == skill_level)
            .and_then(|l| l.social_class)
    }

    /// Whether `skill_id` is a residence skill (`residenceSkill="true"` in the
    /// pledge tree) — a castle/clan-hall benefit, never a `//give_clan_skills`
    /// grant. Used to purge/ignore residence skills that a pre-fix grant leaked
    /// into a clan's stored skill set.
    pub fn is_residence_skill(&self, skill_id: i32) -> bool {
        self.pledge
            .iter()
            .any(|l| l.skill_id == skill_id && l.residencial)
    }
}

/// `SocialClass.valueOf(name).ordinal()` — the clan rank ladder (Java
/// `model/SocialClass`).
fn social_class_ordinal(name: &str) -> Option<u8> {
    Some(match name.trim() {
        "VAGABOND" => 0,
        "VASSAL" => 1,
        "APPRENTICE" => 2,
        "HEIR" => 3,
        "KNIGHT" => 4,
        "ELDER" => 5,
        "BARON" => 6,
        "VISCOUNT" => 7,
        "COUNT" => 8,
        "MARQUIS" => 9,
        "DUKE" => 10,
        "GRAND_DUKE" => 11,
        "DISTINGUISHED_KING" => 12,
        "EMPEROR" => 13,
        _ => return None,
    })
}

fn new_learn(e: &quick_xml::events::BytesStart) -> PledgeSkillLearn {
    PledgeSkillLearn {
        skill_id: attr_i32(e, b"skillId").unwrap_or(-1),
        skill_level: attr_i32(e, b"skillLevel").unwrap_or(0),
        get_level: attr_i32(e, b"getLevel").unwrap_or(99),
        social_class: None,
        residencial: attr_str(e, b"residenceSkill").as_deref() == Some("true"),
        residence_ids: Vec::new(),
        level_up_sp: attr_str(e, b"levelUpSp")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    }
}

/// Parse one pledge/sub-pledge tree file, keeping only `<skillTree>` blocks
/// whose `type` matches `tree_type`. `<socialClass>` is a child element, so a
/// skill spanning a Start…End pair is buffered in `cur` until its `</skill>`.
fn parse(path: &str, tree_type: &str) -> Vec<PledgeSkillLearn> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut in_tree = false;
    let mut cur: Option<PledgeSkillLearn> = None;
    let mut in_social = false;
    let mut in_residence = false;
    for event in xml::events(&content) {
        match event {
            Event::Start(e) => match e.name().as_ref() {
                b"skillTree" => in_tree = attr_str(&e, b"type").as_deref() == Some(tree_type),
                b"skill" if in_tree => cur = Some(new_learn(&e)),
                b"socialClass" if cur.is_some() => in_social = true,
                b"residenceId" if cur.is_some() => in_residence = true,
                _ => {}
            },
            Event::Empty(e) if in_tree && e.name().as_ref() == b"skill" => {
                let learn = new_learn(&e);
                if learn.skill_id > 0 {
                    out.push(learn);
                }
            }
            Event::Text(t) if in_social => {
                if let Some(c) = cur.as_mut() {
                    c.social_class = social_class_ordinal(&t.unescape().unwrap_or_default());
                }
            }
            Event::Text(t) if in_residence => {
                if let (Some(c), Ok(id)) = (
                    cur.as_mut(),
                    t.unescape().unwrap_or_default().trim().parse(),
                ) {
                    c.residence_ids.push(id);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"socialClass" => in_social = false,
                b"residenceId" => in_residence = false,
                b"skill" => {
                    if let Some(c) = cur.take()
                        && c.skill_id > 0
                    {
                        out.push(c);
                    }
                }
                b"skillTree" => in_tree = false,
                _ => {}
            },
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist_root() -> &'static str {
        crate::data::DIST_GAME
    }

    #[test]
    fn loads_real_pledge_tree() {
        let t = PledgeSkillTreeData::load_from(dist_root());
        // Clan Body (370) is an HEIR (ordinal 3) pledge skill learnable at clan
        // level 3 — the canonical clan passive.
        assert_eq!(
            t.social_class_of(370, 1),
            Some(3),
            "Clan Body lv1 is HEIR-gated"
        );
        // A brand-new clan of level 4 qualifies for the level-1..4 clan skills.
        let none: HashMap<i32, i32> = HashMap::new();
        let l4 = t.max_pledge_skills(4, &none, false);
        assert!(
            l4.iter().any(|&(id, _)| id == 370),
            "level-4 clan qualifies for Clan Body"
        );
        // A level-1 clan qualifies for nothing gated above level 1.
        let l1 = t.max_pledge_skills(1, &none, false);
        assert!(
            !l1.iter().any(|&(id, _)| id == 370),
            "level-1 clan does not get level-3 Clan Body"
        );
        // includeSquad pulls in sub-pledge (squad) skills at high clan level.
        let squad = t.max_pledge_skills(11, &none, true);
        assert!(
            squad.len() > t.max_pledge_skills(11, &none, false).len(),
            "squad skills add entries"
        );
    }

    #[test]
    fn excludes_residence_skills() {
        // Java `getMaxPledgeSkills` filters `!isResidencialSkill()`: residence
        // skills (`residenceSkill="true"` in the tree, e.g. Residence Body 590)
        // are granted by owning a castle/clan hall, never by //give_clan_skills.
        // Regression: the loader read the wrong attribute name, so all 30
        // residence skills leaked into the grant.
        let t = PledgeSkillTreeData::load_from(dist_root());
        let none: HashMap<i32, i32> = HashMap::new();
        let all = t.max_pledge_skills(11, &none, true);
        assert!(
            !all.iter().any(|&(id, _)| id == 590),
            "Residence Body (590) is a residence skill and must be excluded"
        );
        assert!(
            all.iter().any(|&(id, _)| id == 370),
            "non-residence clan skills are still granted"
        );
    }

    #[test]
    fn keeps_highest_level_and_respects_current() {
        let t = PledgeSkillTreeData::load_from(dist_root());
        let none: HashMap<i32, i32> = HashMap::new();
        // Clan Body has 5 levels but all gate at clan level 3 → a max-level clan
        // gets exactly one entry for it, at its highest reachable level.
        let all = t.max_pledge_skills(11, &none, false);
        let clan_body: Vec<_> = all.iter().filter(|&&(id, _)| id == 370).collect();
        assert_eq!(clan_body.len(), 1, "one entry per skill id");
        let max_lvl = clan_body[0].1;
        // A clan that already has it at max level gets no new entry for it.
        let mut current = HashMap::new();
        current.insert(370, max_lvl);
        let refreshed = t.max_pledge_skills(11, &current, false);
        assert!(
            !refreshed.iter().any(|&(id, _)| id == 370),
            "already-max skill is skipped"
        );
    }
}
