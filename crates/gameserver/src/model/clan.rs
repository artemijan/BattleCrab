//! Clans — the G11 creation/display slice of Java's `model/clan/Clan` +
//! `ClanTable`. A clan is a name + leader + member roster; levels/skills/
//! crests/wars/sub-pledges are later milestones (fields exist where the
//! packets need a zero to write). Loaded once at boot (`DbEvent::
//! ClansLoaded`), mutated only by `game_loop/clans.rs`.

/// One `clan_members`-equivalent row — Java reads members from `characters
/// WHERE clanid=?`; this snapshot carries what the pledge packets show.
/// Online status is always resolved live against `World.objects`.
#[derive(Debug, Clone)]
pub struct ClanMember {
    pub char_id: i32,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub sex: i32,
    pub race: i32,
    /// Java `ClanMember._powerGrade` (`characters.power_grade`): the member's
    /// rank (1 = leader, 5 = new member, 9 = academy). Privileges derive from
    /// the clan's rank table at login and on rank edits.
    pub power_grade: i32,
    /// `characters.title` — shown in the member-detail pledge window.
    pub title: String,
}

/// Java `Clan`, narrowed to the creation/display slice.
#[derive(Debug, Clone)]
pub struct Clan {
    pub id: i32,
    pub name: String,
    pub leader_id: i32,
    pub level: i32,
    /// Java `Clan._reputationScore`. Above level 5 a clan accumulates it;
    /// clamped to ±[`MAX_REPUTATION`]. Reputation-gated clan skills are a later
    /// milestone, so nothing consumes it yet beyond the pledge windows.
    pub reputation_score: i32,
    /// Java `clan_data.hasCastle` — the residence id of the castle this clan
    /// owns (0 = none). Source of truth for `Castle` ownership.
    pub castle_id: i32,
    pub members: Vec<ClanMember>,
    /// The clan's learned skills (Java `Clan._skills`, `skillId → level`).
    /// Populated by `//give_clan_skills` and the boot-time `clan_skills` load;
    /// re-applied to each member on login via the pledge social-class gate.
    /// Sub-pledge skills (Java `_subPledgeSkills`) fold in here too — the port
    /// has no sub-units, and `getMaxPledgeSkills` routes squad skills into the
    /// main clan skill set anyway.
    pub skills: std::collections::HashMap<i32, i32>,
    /// The shared clan warehouse (Java `Clan._warehouse`, a `ClanWarehouse`
    /// container). Persisted with `owner_id = clan id`, `loc = "CLANWH"`.
    pub warehouse: crate::model::inventory::Warehouse,
    /// Java `_charPenaltyExpiryTime` (`clan_data.char_penalty_expiry_time`):
    /// after ousting a member the clan cannot invite anyone for a day (SM 231
    /// on the inviter).
    pub char_penalty_expiry_time: i64,
    /// Java `_dissolvingExpiryTime` (`clan_data.dissolving_expiry_time`):
    /// non-zero while a leader-requested dissolution is pending; the clan is
    /// destroyed when it comes due (`ClanTable.scheduleRemoveClan`, re-armed
    /// from this stamp at boot).
    pub dissolving_expiry_time: i64,
    /// Java `_privs` (`clan_privs` table): rank (power grade 1–9) → privilege
    /// bitmask. Ranks without a row hold no privileges (Java initializes all 9
    /// to an empty mask). Member privileges are derived from this at login and
    /// refreshed when the leader edits a rank.
    pub rank_privs: std::collections::HashMap<i32, i32>,
    /// Java `_newLeaderId` (`clan_data.new_leader_id`): a pending delegated
    /// leader transfer (`AltClanLeaderInstantActivation = False` flow). Applied
    /// at the daily reset — TODO(G33): `DailyTaskManager.onClanLeaderChange`.
    pub new_leader_id: i32,
    /// Java `_allyId` (`clan_data.ally_id`): the alliance this clan belongs to
    /// — the *leader clan's own id* doubles as the alliance id (0 = none).
    pub ally_id: i32,
    /// Java `_allyName` (`clan_data.ally_name`).
    pub ally_name: String,
    /// Java `_allyPenaltyExpiryTime`/`_allyPenaltyType` — the 1-day ally
    /// penalties ([`ALLY_PENALTY_TYPE_CLAN_LEAVED`] …).
    pub ally_penalty_expiry_time: i64,
    pub ally_penalty_type: i32,
}

impl Clan {
    pub fn leader_name(&self) -> &str {
        self.members.iter().find(|m| m.char_id == self.leader_id).map(|m| m.name.as_str()).unwrap_or("")
    }

    pub fn member(&self, char_id: i32) -> Option<&ClanMember> {
        self.members.iter().find(|m| m.char_id == char_id)
    }

    /// The pledge class a member of this clan holds — Java
    /// `ClanMember.calculatePledgeClass`, narrowed to the main clan (`pledgeType
    /// == 0`) with the default sub-pledge, the only pledge shape the port models
    /// (no academy/royal/order sub-pledges yet). Clan levels run to 11 on this
    /// dist; a clan below level 4 yields 0 for everyone, so the on-head rank
    /// crown (which the client draws from this value, sent in UserInfo/CharInfo)
    /// only appears once the clan is developed enough — matching retail. A clan
    /// leader's value climbs with the clan level (…7→7, 8→8, …, 11→11), which is
    /// what puts the crown over a high-level leader's head.
    pub fn pledge_class_of(&self, char_id: i32) -> u8 {
        let is_leader = char_id == self.leader_id;
        match self.level {
            4 if is_leader => 3,
            5 => if is_leader { 4 } else { 2 },
            6 => if is_leader { 5 } else { 3 },
            7 => if is_leader { 7 } else { 4 },
            8 => if is_leader { 8 } else { 5 },
            9 => if is_leader { 9 } else { 6 },
            10 => if is_leader { 10 } else { 7 },
            11 => if is_leader { 11 } else { 8 },
            _ => 0,
        }
    }
}

/// Java `setReputationScore` clamp bound (±100M).
pub const MAX_REPUTATION: i32 = 100_000_000;

impl Clan {
    /// Java `Clan.addReputationScore` → `setReputationScore`: add `value`
    /// (signed) and clamp to ±[`MAX_REPUTATION`]; returns the new score. The
    /// zero-crossing clan-skill (de)activation and the `PledgeShowInfoUpdate`
    /// broadcast are the caller's job (clan skills are a later milestone).
    pub fn add_reputation_score(&mut self, value: i32) -> i32 {
        self.reputation_score = self.reputation_score.saturating_add(value).clamp(-MAX_REPUTATION, MAX_REPUTATION);
        self.reputation_score
    }
}

/// The all-bits leader privilege mask: Java `new EnumIntBitmask<>(
/// ClanPrivilege.class, true)` over the 24-entry enum (ordinal = bit index,
/// DUMMY included) = bits 0..24.
pub const ALL_CLAN_PRIVILEGES: i32 = (1 << 24) - 1;

/// `ClanPrivilege.CL_JOIN_CLAN` (ordinal 1) — required to invite into the clan.
pub const CL_JOIN_CLAN: i32 = 1 << 1;

/// `ClanPrivilege.CL_VIEW_WAREHOUSE` (ordinal 3) — required to withdraw from
/// the clan warehouse.
pub const CL_VIEW_WAREHOUSE: i32 = 1 << 3;

/// `ClanPrivilege.CL_DISMISS` (ordinal 6) — required to oust a member.
pub const CL_DISMISS: i32 = 1 << 6;

/// The academy pledge type (Java `Clan.SUBUNIT_ACADEMY`).
pub const SUBUNIT_ACADEMY: i32 = -1;

/// `ClanPrivilege.CL_MANAGE_RANKS` (ordinal 4) — required to edit member ranks.
pub const CL_MANAGE_RANKS: i32 = 1 << 4;

/// `ClanPrivilege.CL_PLEDGE_WAR` (ordinal 5) — required to declare/stop wars.
pub const CL_PLEDGE_WAR: i32 = 1 << 5;

/// Java `Clan.PENALTY_TYPE_*` — what the running ally penalty forbids.
pub const ALLY_PENALTY_TYPE_CLAN_LEAVED: i32 = 1;
pub const ALLY_PENALTY_TYPE_CLAN_DISMISSED: i32 = 2;
pub const ALLY_PENALTY_TYPE_DISMISS_CLAN: i32 = 3;
pub const ALLY_PENALTY_TYPE_DISSOLVE_ALLY: i32 = 4;

/// The only rights bestowable on rank 9 (academy): CL_VIEW_WAREHOUSE (3),
/// CH_OPEN_DOOR (11), CS_OPEN_DOOR (15) — Java `RequestPledgePower`'s mask.
pub const RANK9_PRIVS_MASK: i32 = (1 << 3) | (1 << 11) | (1 << 15);

impl Clan {
    /// Java `getRankPrivs(rank).getBitmask()` — an unset rank is an empty mask.
    pub fn rank_privs_of(&self, rank: i32) -> i32 {
        self.rank_privs.get(&rank).copied().unwrap_or(0)
    }
}

impl Clan {
    /// Java `Clan.getMaxNrOfMembers(pledgeType)` — the member cap per pledge
    /// type at this clan's level. The full retail table is ported even though
    /// only the main pledge (0) can be joined until sub-units land (G18
    /// slice 6).
    pub fn max_members_of(&self, pledge_type: i32) -> usize {
        match pledge_type {
            0 => match self.level {
                0 => 10,
                1 => 15,
                2 => 20,
                3 => 30,
                _ => 40,
            },
            -1 => 20,
            100 | 200 => {
                if self.level == 11 {
                    30
                } else {
                    20
                }
            }
            1001 | 1002 | 2001 | 2002 => {
                if self.level >= 9 {
                    25
                } else {
                    10
                }
            }
            _ => 0,
        }
    }

    /// Java `getSubPledgeMembersCount(pledgeType)`, narrowed: every member the
    /// port models is in the main pledge (type 0) until sub-units land.
    pub fn sub_pledge_members_count(&self, pledge_type: i32) -> usize {
        if pledge_type == 0 {
            self.members.len()
        } else {
            0
        }
    }
}

impl Clan {
    /// Whether `char_id` holds `privilege` (a `CL_*` bit): the leader always
    /// does (Java `Player.hasClanPrivilege` short-circuits for the leader),
    /// otherwise it's tested against the member's stored `clan_privs`.
    pub fn has_privilege(&self, char_id: i32, member_privs: i32, privilege: i32) -> bool {
        char_id == self.leader_id || (member_privs & privilege) != 0
    }
}

/// Java `ClanWarState` (ordinal = the wire/DB value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanWarState {
    Declaration = 0,
    BloodDeclaration = 1,
    Mutual = 2,
    Win = 3,
    Loss = 4,
    Tie = 5,
}

impl ClanWarState {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::BloodDeclaration,
            2 => Self::Mutual,
            3 => Self::Win,
            4 => Self::Loss,
            5 => Self::Tie,
            _ => Self::Declaration,
        }
    }
}

/// Java `ClanWar` — one war between two clans (`clan_wars` row). The declarer
/// is the *attacker*; the war turns MUTUAL when the attacked side declares
/// back or kills 5 attackers during BLOOD_DECLARATION.
#[derive(Debug, Clone)]
pub struct ClanWar {
    pub attacker_id: i32,
    pub attacked_id: i32,
    pub state: ClanWarState,
    /// Set by a surrender (`cancel`): the other side won.
    pub winner_id: i32,
    pub start_time: i64,
    pub end_time: i64,
    pub attacker_kills: i32,
    pub attacked_kills: i32,
}

/// `ClanWar.TIME_TO_CANCEL_NON_MUTUAL_CLAN_WAR` — 7 days.
pub const WAR_TIMEOUT_MS: i64 = 7 * 86_400_000;

impl ClanWar {
    pub fn involves(&self, clan_id: i32) -> bool {
        self.attacker_id == clan_id || self.attacked_id == clan_id
    }

    pub fn opposing(&self, clan_id: i32) -> i32 {
        if self.attacker_id == clan_id {
            self.attacked_id
        } else {
            self.attacker_id
        }
    }

    /// Java `getClanWarState(clan)` — a set winner turns the shared state into
    /// this side's WIN/LOSS view.
    pub fn state_for(&self, clan_id: i32) -> ClanWarState {
        if self.winner_id > 0 {
            if self.winner_id == clan_id {
                ClanWarState::Win
            } else {
                ClanWarState::Loss
            }
        } else {
            self.state
        }
    }

    /// Java `getKillDifference(clan)` — this side's score in the war list.
    pub fn kill_difference(&self, clan_id: i32) -> i32 {
        if self.attacker_id == clan_id {
            self.attacker_kills - self.attacked_kills
        } else {
            self.attacked_kills - self.attacker_kills
        }
    }

    /// Java `getKillToStart` — attacker kills still needed to force MUTUAL.
    pub fn kill_to_start(&self) -> i32 {
        if self.state == ClanWarState::BloodDeclaration {
            5 - self.attacked_kills
        } else {
            0
        }
    }

    /// Java `getRemainingTime` — the (whole-seconds) stamp the war list shows.
    pub fn remaining_time(&self) -> i32 {
        ((self.start_time + WAR_TIMEOUT_MS) / 1000) as i32
    }
}
