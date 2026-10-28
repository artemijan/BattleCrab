//! Port of `SkillTreeData`'s pledge (clan) skill trees — the
//! `pledgeSkillTree` and `subPledgeSkillTree` blocks under
//! `data/stats/players/skillTrees/`. Salvation files a block by its `type`
//! attribute, whatever file it is in; it ships `pledgeSkillTree.xml` and no
//! sub-pledge (squad) tree at all, so `_subPledgeSkillTree` stays empty and
//! `includeSquad` adds nothing. Drives the `//give_clan_skills` /
//! `//give_all_clan_skills` admin path ([`PledgeSkillTreeData::max_pledge_skills`],
//! Java `getMaxPledgeSkills`) and the per-member social-class gate applied when
//! a clan skill is granted / a member logs in ([`PledgeSkillTreeData::
//! social_class_of`], Java `getPledgeSkill`/`getSubPledgeSkill`).

use crate::data::xml;
use crate::data::xml::{attr_i32, attr_str};
use std::collections::HashMap;

use quick_xml::events::Event;
use tracing::info;

/// One `<skill>` entry in a pledge/sub-pledge tree (Java `SkillLearn`, narrowed
/// to the clan-skill fields).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
        let mut pledge = Vec::new();
        let mut sub_pledge = Vec::new();
        let dir = format!("{file_path}{}", crate::data::skill_tree::SKILL_TREES_DIR);
        for path in xml::xml_files_under(dir) {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Most of the directory is class trees; skip a file that names
            // neither type rather than walk it event by event.
            if !content.contains("ledgeSkillTree\"") {
                continue;
            }
            pledge.extend(parse(&content, "pledgeSkillTree"));
            sub_pledge.extend(parse(&content, "subPledgeSkillTree"));
        }
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
        let squad: &[PledgeSkillLearn] = if include_squad { &self.sub_pledge } else { &[] };
        for learn in self
            .pledge
            .iter()
            .filter(|l| !l.residencial)
            .chain(squad.iter())
        {
            if clan_level < learn.get_level
                || current.get(&learn.skill_id).copied().unwrap_or(0) >= learn.skill_level
            {
                continue;
            }
            result
                .entry(learn.skill_id)
                .and_modify(|lvl| *lvl = (*lvl).max(learn.skill_level))
                .or_insert(learn.skill_level);
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
        skill_id: attr_i32(e, "skillId").unwrap_or(-1),
        skill_level: attr_i32(e, "skillLevel").unwrap_or(0),
        get_level: attr_i32(e, "getLevel").unwrap_or(99),
        social_class: None,
        residencial: attr_str(e, "residenceSkill").as_deref() == Some("true"),
        residence_ids: Vec::new(),
        level_up_sp: attr_str(e, "levelUpSp")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    }
}

/// Parse one skill-tree file, keeping only `<skillTree>` blocks whose `type`
/// matches `tree_type`. `<socialClass>` is a child element, so a skill spanning
/// a Start…End pair is buffered in `cur` until its `</skill>`.
fn parse(content: &str, tree_type: &str) -> Vec<PledgeSkillLearn> {
    let mut out = Vec::new();
    let mut in_tree = false;
    let mut cur: Option<PledgeSkillLearn> = None;
    let mut in_social = false;
    let mut in_residence = false;
    for event in xml::events(content) {
        match event {
            Event::Start(e) => match e.name().as_ref() {
                "skillTree" => in_tree = attr_str(&e, "type").as_deref() == Some(tree_type),
                "skill" if in_tree => cur = Some(new_learn(&e)),
                "socialClass" if cur.is_some() => in_social = true,
                "residenceId" if cur.is_some() => in_residence = true,
                _ => {}
            },
            Event::Empty(e) if in_tree && e.name().as_ref() == "skill" => {
                let learn = new_learn(&e);
                if learn.skill_id > 0 {
                    out.push(learn);
                }
            }
            Event::Text(t) if in_social => {
                if let Some(c) = cur.as_mut() {
                    c.social_class = social_class_ordinal(&t.into_inner());
                }
            }
            Event::Text(t) if in_residence => {
                if let (Some(c), Ok(id)) = (cur.as_mut(), t.into_inner().trim().parse()) {
                    c.residence_ids.push(id);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "socialClass" => in_social = false,
                "residenceId" => in_residence = false,
                "skill" => {
                    if let Some(c) = cur.take()
                        && c.skill_id > 0
                    {
                        out.push(c);
                    }
                }
                "skillTree" => in_tree = false,
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

    /// Salvation's `pledgeSkillTree.xml` holds only the 11 residence skills —
    /// the clan skills moved to `ClanMasteryData.xml` — so every entry is
    /// residential and `getMaxPledgeSkills` grants nothing, squad or not.
    #[test]
    fn loads_real_pledge_tree() {
        let t = PledgeSkillTreeData::load_from(dist_root());
        assert_eq!(t.pledge.len(), 11);
        assert!(t.pledge.iter().all(|l| l.residencial));
        assert!(t.sub_pledge.is_empty(), "no sub-pledge tree ships");
        let none: HashMap<i32, i32> = HashMap::new();
        assert!(t.max_pledge_skills(11, &none, true).is_empty());
        assert!(t.available_pledge_skills(11, &none).is_empty());
    }

    /// Gludio Territory Benefaction (848): `getLevel="5"`,
    /// `<socialClass>APPRENTICE</socialClass>` (ordinal 2), residence 81.
    /// Residence Guard (32123) is `DISTINGUISHED_KING` (12) at the fortresses
    /// 112 and 114–121.
    #[test]
    fn residence_skills_carry_their_gates() {
        let t = PledgeSkillTreeData::load_from(dist_root());
        assert_eq!(t.social_class_of(848, 1), Some(2));
        assert!(t.is_residence_skill(848));
        let gludio: Vec<i32> = t
            .available_residential_skills(81)
            .iter()
            .map(|l| l.skill_id)
            .collect();
        assert_eq!(gludio, vec![848]);
        assert_eq!(t.social_class_of(32123, 1), Some(12));
        let guard = t.pledge_skill(32123, 1).expect("Residence Guard");
        assert_eq!(
            guard.residence_ids,
            vec![112, 114, 115, 116, 117, 118, 119, 120, 121]
        );
    }
}
