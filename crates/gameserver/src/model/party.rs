//! Port of `model/Party` (the state; the packet flows live in
//! `game_loop/party.rs`) and the pure party XP-split math from
//! `Party.distributeXpAndSp`. Command channels, matching rooms, tactical
//! signs, duels and pets are out of scope — see PLAN_G10_SOCIAL.md.

use std::collections::HashSet;

/// Java `PartyDistributionType` (client id + the sysstring-e.dat name id the
/// loot-change SMs display).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootRule {
    FindersKeepers,
    Random,
    RandomIncludingSpoil,
    ByTurn,
    ByTurnIncludingSpoil,
}

impl LootRule {
    pub fn id(self) -> i32 {
        match self {
            Self::FindersKeepers => 0,
            Self::Random => 1,
            Self::RandomIncludingSpoil => 2,
            Self::ByTurn => 3,
            Self::ByTurnIncludingSpoil => 4,
        }
    }

    pub fn from_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::FindersKeepers),
            1 => Some(Self::Random),
            2 => Some(Self::RandomIncludingSpoil),
            3 => Some(Self::ByTurn),
            4 => Some(Self::ByTurnIncludingSpoil),
            _ => None,
        }
    }

    /// `getSysStringId` — for `REQUESTING_APPROVAL…`/`PARTY_LOOT_WAS_CHANGED…`.
    pub fn sys_string_id(self) -> i32 {
        match self {
            Self::FindersKeepers => 487,
            Self::Random => 488,
            Self::RandomIncludingSpoil => 798,
            Self::ByTurn => 799,
            Self::ByTurnIncludingSpoil => 800,
        }
    }

    /// Random/by-turn looter selection; the `*_INCLUDING_SPOIL` variants share
    /// the same selection for normal drops and additionally spread spoil loot
    /// (see `includes_spoil`).
    pub fn is_random(self) -> bool {
        matches!(self, Self::Random | Self::RandomIncludingSpoil)
    }
    pub fn is_by_turn(self) -> bool {
        matches!(self, Self::ByTurn | Self::ByTurnIncludingSpoil)
    }

    /// Whether Sweeper (spoil) loot is distributed among the party rather than
    /// kept by the sweeper. Java `getActualLooter(spoil=true)` only assigns a
    /// party looter for the `*_INCLUDING_SPOIL` types; every other rule leaves
    /// spoil loot with the caster.
    pub fn includes_spoil(self) -> bool {
        matches!(self, Self::RandomIncludingSpoil | Self::ByTurnIncludingSpoil)
    }
}

/// A leader's pending loot-rule change (`Party._changeRequestDistributionType`
/// + `_changeDistributionTypeAnswers`): every other member must agree within
/// 15 s.
#[derive(Debug, Clone)]
pub struct LootChangeRequest {
    pub rule: LootRule,
    pub answers: HashSet<i32>,
}

/// One party. Keyed by `World.parties` id; members are player object ids,
/// **leader first** (Java's `_members.get(0)`).
#[derive(Debug, Clone)]
pub struct Party {
    pub members: Vec<i32>,
    pub distribution: LootRule,
    /// An `AskJoinParty` is outstanding (`_pendingInvitation`); expires
    /// `REQUEST_TIMEOUT` (15 s) after it was sent without an answer.
    pub pending_invitation: bool,
    pub pending_invite_expiry_tick: u64,
    /// `_itemLastLoot` — the by-turn loot cursor.
    pub item_last_loot: usize,
    pub loot_change: Option<LootChangeRequest>,
    /// Generation counter for this party's scheduled tasks (position
    /// broadcast, loot-change timeout): a task with a stale seq no-ops, and
    /// bumping it on disband kills every outstanding task.
    pub seq: u64,
}

impl Party {
    pub fn new(leader: i32, distribution: LootRule, seq: u64) -> Self {
        Self {
            members: vec![leader],
            distribution,
            pending_invitation: false,
            pending_invite_expiry_tick: 0,
            item_last_loot: 0,
            loot_change: None,
            seq,
        }
    }

    pub fn leader(&self) -> i32 {
        self.members[0]
    }

    pub fn is_leader(&self, object_id: i32) -> bool {
        self.leader() == object_id
    }

    pub fn contains(&self, object_id: i32) -> bool {
        self.members.contains(&object_id)
    }
}

/// `Party.BONUS_EXP_SP` — the party-size XP/SP bonus ladder (index =
/// member count − 1, capped).
pub const BONUS_EXP_SP: [f64; 9] = [1.0, 1.3, 1.35, 1.4, 1.55, 1.6, 1.7, 1.8, 2.0];

/// `getBaseExpSpBonus(membersCount)`.
pub fn base_exp_sp_bonus(members_count: usize) -> f64 {
    if members_count < 2 {
        return 1.0;
    }
    BONUS_EXP_SP[(members_count - 1).min(BONUS_EXP_SP.len() - 1)]
}

/// `getExpBonus`/`getSpBonus` (no instances): base bonus × the party rate for
/// parties of 2+, plain base bonus solo.
pub fn exp_sp_bonus(members_count: usize, party_rate: f64) -> f64 {
    if members_count < 2 {
        base_exp_sp_bonus(members_count)
    } else {
        base_exp_sp_bonus(members_count) * party_rate
    }
}

/// `calculateExpSpPartyCutoff`'s "highfive" gap table: percentage of the
/// reward a member keeps, by their level gap to the top rewarded level.
/// `gaps` are inclusive `(from, to)` ranges paired with `percents`; a gap
/// outside every range keeps **nothing** (Java only rewards inside a range).
pub fn highfive_cutoff_percent(level_diff: i32, gaps: &[(i32, i32)], percents: &[i32]) -> Option<i32> {
    gaps.iter()
        .zip(percents)
        .find(|((from, to), _)| level_diff >= *from && level_diff <= *to)
        .map(|(_, &p)| p)
}

/// `getValidMembers` — which rewarded members share the split, per
/// `PartyXpCutoffMethod`. Takes `(object_id, level)` pairs; returns the
/// valid subset's object ids.
pub fn valid_members(
    members: &[(i32, i32)],
    top_level: i32,
    method: &str,
    cutoff_level: i32,
    cutoff_percent: f64,
) -> Vec<i32> {
    match method {
        "level" => members
            .iter()
            .filter(|(_, lvl)| top_level - lvl <= cutoff_level)
            .map(|&(id, _)| id)
            .collect(),
        "percentage" => {
            let sq_sum: i64 = members.iter().map(|&(_, l)| (l as i64) * (l as i64)).sum();
            members
                .iter()
                .filter(|&&(_, l)| (l as i64) * (l as i64) * 100 >= sq_sum * cutoff_percent as i64)
                .map(|&(id, _)| id)
                .collect()
        }
        "auto" => {
            let n = members.len() as i64;
            if n < 2 {
                return members.iter().map(|&(id, _)| id).collect();
            }
            let sq_sum: i64 = members.iter().map(|&(_, l)| (l as i64) * (l as i64)).sum();
            members
                .iter()
                .filter(|&&(_, l)| (l as i64) * (l as i64) >= sq_sum / (n * n))
                .map(|&(id, _)| id)
                .collect()
        }
        // "highfive" and "none" keep everyone (the highfive gap table is
        // applied per member afterwards).
        _ => members.iter().map(|&(id, _)| id).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonus_ladder_matches_java() {
        assert_eq!(base_exp_sp_bonus(1), 1.0);
        assert_eq!(base_exp_sp_bonus(2), 1.3);
        assert_eq!(base_exp_sp_bonus(9), 2.0);
        assert_eq!(base_exp_sp_bonus(20), 2.0, "capped at the table end");
        assert_eq!(exp_sp_bonus(1, 70.0), 1.0, "solo never gets the party rate");
        assert_eq!(exp_sp_bonus(2, 70.0), 1.3 * 70.0);
    }

    #[test]
    fn highfive_gaps_match_dist_config() {
        let gaps = [(0, 9), (10, 14), (15, 99)];
        let pct = [100, 30, 0];
        assert_eq!(highfive_cutoff_percent(0, &gaps, &pct), Some(100));
        assert_eq!(highfive_cutoff_percent(9, &gaps, &pct), Some(100));
        assert_eq!(highfive_cutoff_percent(10, &gaps, &pct), Some(30));
        assert_eq!(highfive_cutoff_percent(14, &gaps, &pct), Some(30));
        assert_eq!(highfive_cutoff_percent(15, &gaps, &pct), Some(0));
        assert_eq!(highfive_cutoff_percent(-1, &gaps, &pct), None, "below every range");
    }

    #[test]
    fn valid_members_level_method() {
        let members = [(1, 20), (2, 10), (3, 4)];
        assert_eq!(valid_members(&members, 20, "level", 15, 3.0), vec![1, 2]);
        assert_eq!(valid_members(&members, 20, "highfive", 15, 3.0), vec![1, 2, 3]);
        assert_eq!(valid_members(&members, 20, "none", 15, 3.0), vec![1, 2, 3]);
    }
}
