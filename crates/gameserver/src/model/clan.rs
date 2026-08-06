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
    /// Java `ClanMember._pledgeType` (`characters.subpledge`): 0 = main pledge,
    /// -1 = academy, 100/200 = royal guard units, 1001/1002/2001/2002 = knight
    /// units. Mirrors the live `Player.pledge_type` for online members.
    pub pledge_type: i32,
    /// Java `ClanMember._apprentice`/`_sponsor` (`characters.apprentice`/
    /// `.sponsor`) — the academy mentorship pair, mirrored on the roster so an
    /// offline member's pane still shows their partner.
    pub apprentice: i32,
    pub sponsor: i32,
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
    /// Java `Clan._bloodAllianceCount` (`clan_data.blood_alliance_count`):
    /// incremented by `SiegeManager.getBloodAllianceReward()` each time the clan
    /// holds its castle through a siege. The Interlude reward is 0 (Siege.ini
    /// `BloodAllianceReward = 0`), so it stays 0 unless an admin raises it.
    pub blood_alliance_count: i32,
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
    /// at the Wednesday daily reset (Java `DailyTaskManager.clanLeaderApply`).
    pub new_leader_id: i32,
    /// Java `_subPledges` (`clan_subpledges` table): the academy + up to 2
    /// royal-guard + up to 4 knight-order sub-units this clan has founded,
    /// keyed by pledge-type id.
    pub sub_pledges: std::collections::HashMap<i32, SubPledge>,
    /// Java `_allyId` (`clan_data.ally_id`): the alliance this clan belongs to
    /// — the *leader clan's own id* doubles as the alliance id (0 = none).
    pub ally_id: i32,
    /// Java `_allyName` (`clan_data.ally_name`).
    pub ally_name: String,
    /// Java `_allyPenaltyExpiryTime`/`_allyPenaltyType` — the 1-day ally
    /// penalties ([`ALLY_PENALTY_TYPE_CLAN_LEAVED`] …).
    pub ally_penalty_expiry_time: i64,
    pub ally_penalty_type: i32,
    /// Java `_crestId`/`_crestLargeId`/`_allyCrestId` — ids into
    /// [`crate::world::World::crests`]; 0 = none.
    pub crest_id: i32,
    pub crest_large_id: i32,
    pub ally_crest_id: i32,
}

impl Clan {
    pub fn leader_name(&self) -> &str {
        self.members
            .iter()
            .find(|m| m.char_id == self.leader_id)
            .map(|m| m.name.as_str())
            .unwrap_or("")
    }

    pub fn member(&self, char_id: i32) -> Option<&ClanMember> {
        self.members.iter().find(|m| m.char_id == char_id)
    }

    /// The pledge class a member of this clan holds — Java
    /// `ClanMember.calculatePledgeClass`, a direct port of its per-level nested
    /// switch on `player.getPledgeType()` (own sub-unit membership) and
    /// `clan.getLeaderSubPledge` (which sub-unit, if any, they captain). Clan
    /// levels run to 11 on this dist; a clan below level 4 yields 0 for
    /// everyone, so the on-head rank crown (which the client draws from this
    /// value, sent in UserInfo/CharInfo) only appears once the clan is
    /// developed enough — matching retail.
    pub fn pledge_class_of(&self, char_id: i32) -> u8 {
        let is_leader = char_id == self.leader_id;
        let pledge_type = self.member(char_id).map(|m| m.pledge_type).unwrap_or(0);
        let level = self.level;
        if level < 4 {
            return 0;
        }
        if level == 4 {
            return if is_leader { 3 } else { 0 };
        }
        if level == 5 {
            return if is_leader { 4 } else { 2 };
        }
        // level 6..=11: verified against ClanMember.calculatePledgeClass's
        // per-level switch — `default_member`/`royal_member` are uniform
        // across this range; only the plain leader value (6→5, not 6) and the
        // sub-unit-captain bonus (+1 at level 6 where knights don't exist yet,
        // +2 from level 7 on) are level-6-irregular.
        let leader_val: i32 = if level == 6 { 5 } else { level };
        let default_member: i32 = level - 3; // 6→3, 7→4, …, 11→8
        let royal_member: i32 = level - 4; // 6→2, …, 11→7
        let knight_member: i32 = level - 5; // only reachable level 7..=11: 7→2 … 11→6
        let class: i32 = match pledge_type {
            SUBUNIT_ACADEMY => 1,
            SUBUNIT_ROYAL1 | SUBUNIT_ROYAL2 => royal_member,
            SUBUNIT_KNIGHT1 | SUBUNIT_KNIGHT2 | SUBUNIT_KNIGHT3 | SUBUNIT_KNIGHT4 => knight_member,
            _ if is_leader => leader_val,
            _ => match self.leader_sub_pledge_of(char_id) {
                SUBUNIT_ROYAL1 | SUBUNIT_ROYAL2 => default_member + if level == 6 { 1 } else { 2 },
                SUBUNIT_KNIGHT1 | SUBUNIT_KNIGHT2 | SUBUNIT_KNIGHT3 | SUBUNIT_KNIGHT4 => {
                    default_member + 1
                }
                _ => default_member,
            },
        };
        class as u8
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
        self.reputation_score = self
            .reputation_score
            .saturating_add(value)
            .clamp(-MAX_REPUTATION, MAX_REPUTATION);
        self.reputation_score
    }
}

/// The all-bits leader privilege mask: Java `new EnumIntBitmask<>(
/// ClanPrivilege.class, true)` over the 24-entry enum (ordinal = bit index,
/// DUMMY included) = bits 0..24.
pub const ALL_CLAN_PRIVILEGES: i32 = (1 << 24) - 1;

/// One `clan_subpledges` row — a founded sub-unit (Java `Clan.SubPledge`).
#[derive(Debug, Clone)]
pub struct SubPledge {
    pub id: i32,
    pub name: String,
    /// 0 while vacant (Java: a departed leader's slot, or the academy, which
    /// never has a sub-pledge leader of its own).
    pub leader_id: i32,
}

/// Java `Clan.SUBUNIT_*` pledge-type ids.
pub const SUBUNIT_ROYAL1: i32 = 100;
pub const SUBUNIT_ROYAL2: i32 = 200;
pub const SUBUNIT_KNIGHT1: i32 = 1001;
pub const SUBUNIT_KNIGHT2: i32 = 1002;
pub const SUBUNIT_KNIGHT3: i32 = 2001;
pub const SUBUNIT_KNIGHT4: i32 = 2002;

impl Clan {
    /// Java `Clan.getLeaderSubPledge(leaderId)` — the pledge-type id of the
    /// sub-unit `leaderId` captains, or 0 if they don't lead one.
    pub fn leader_sub_pledge_of(&self, leader_id: i32) -> i32 {
        self.sub_pledges
            .values()
            .find(|sp| sp.leader_id != 0 && sp.leader_id == leader_id)
            .map(|sp| sp.id)
            .unwrap_or(0)
    }

    /// Java `Clan.getAvailablePledgeTypes(pledgeType)`: 0 when every slot of
    /// that family is taken, else the next open id in the chain (Royal 1→2,
    /// Knight 1→2→3→4).
    pub fn available_pledge_type(&self, requested: i32) -> i32 {
        if !self.sub_pledges.contains_key(&requested) {
            return requested;
        }
        match requested {
            SUBUNIT_ROYAL1 => self.available_pledge_type(SUBUNIT_ROYAL2),
            SUBUNIT_KNIGHT1 => self.available_pledge_type(SUBUNIT_KNIGHT2),
            SUBUNIT_KNIGHT2 => self.available_pledge_type(SUBUNIT_KNIGHT3),
            SUBUNIT_KNIGHT3 => self.available_pledge_type(SUBUNIT_KNIGHT4),
            _ => 0, // SUBUNIT_ACADEMY, SUBUNIT_ROYAL2, SUBUNIT_KNIGHT4: no fallback
        }
    }
}

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

/// `ClanPrivilege.CL_APPRENTICE` (ordinal 8) — required to pair or unpair an
/// academy member with a sponsor (`RequestPledgeSetAcademyMaster`).
pub const CL_APPRENTICE: i32 = 1 << 8;

/// `ClanPrivilege.CL_REGISTER_CREST` (ordinal 7) — required to register/
/// delete a clan or large crest.
pub const CL_REGISTER_CREST: i32 = 1 << 7;

/// `ClanPrivilege.CS_MANOR_ADMIN` (ordinal 17 — second of the CS_ block, right
/// before [`CS_MANAGE_SIEGE`]) — required to manage the castle manor
/// (seed/crop setup) through the chamberlain.
pub const CS_MANOR_ADMIN: i32 = 1 << 17;

/// `ClanPrivilege.CS_MANAGE_SIEGE` (ordinal 18 — after the 10 CL_ and 5 CH_
/// entries, third of the CS_ block; `ALL_CLAN_PRIVILEGES = (1<<24)-1` confirms
/// the 24-entry layout) — required to register the clan for a castle siege.
pub const CS_MANAGE_SIEGE: i32 = 1 << 18;

/// `ClanPrivilege.CS_OPEN_DOOR` (ordinal 16 — first of the CS_ block) —
/// open/close the castle gates and use the doormen's post teleports.
pub const CS_OPEN_DOOR: i32 = 1 << 16;

/// `ClanPrivilege.CS_USE_FUNCTIONS` (ordinal 19) — use the castle's rented
/// functions: the chamberlain's teleport, buffer, products and function list.
pub const CS_USE_FUNCTIONS: i32 = 1 << 19;

/// `ClanPrivilege.CS_DISMISS` (ordinal 20) — banish foreigners from the
/// castle grounds.
pub const CS_DISMISS: i32 = 1 << 20;

/// `ClanPrivilege.CS_SET_FUNCTIONS` (ordinal 23, the last) — buy/upgrade the
/// castle's functions, doors and traps.
pub const CS_SET_FUNCTIONS: i32 = 1 << 23;

/// `ClanPrivilege.CS_TAXES` (ordinal 21) — the chamberlain's castle vault:
/// view the balance, deposit and withdraw.
pub const CS_TAXES: i32 = 1 << 21;

/// `ClanPrivilege.CS_MERCENARIES` (ordinal 22) — the castle Mercenary
/// Manager's console (ticket buy lists).
pub const CS_MERCENARIES: i32 = 1 << 22;

/// `ClanPrivilege.CH_OPEN_DOOR` (ordinal 11) — open/close clan-hall doors.
pub const CH_OPEN_DOOR: i32 = 1 << 11;
/// `ClanPrivilege.CH_OTHER_RIGHTS` (ordinal 12) — use hall functions
/// (teleport / buffs / item creation).
pub const CH_OTHER_RIGHTS: i32 = 1 << 12;
/// `ClanPrivilege.CH_DISMISS` (ordinal 14) — banish non-members from the hall.
pub const CH_DISMISS: i32 = 1 << 14;
/// `ClanPrivilege.CH_SET_FUNCTIONS` (ordinal 15) — buy/remove hall functions.
pub const CH_SET_FUNCTIONS: i32 = 1 << 15;

/// Java `CrestType` ordinals (`crests.type` column).
pub const CREST_TYPE_PLEDGE: i32 = 1;
pub const CREST_TYPE_PLEDGE_LARGE: i32 = 2;
pub const CREST_TYPE_ALLY: i32 = 3;

/// One `crests` row — a stored bitmap (Java `Crest`).
#[derive(Debug, Clone)]
pub struct Crest {
    pub id: i32,
    pub data: Vec<u8>,
    pub kind: i32,
}

/// Java `Clan.PENALTY_TYPE_*` — what the running ally penalty forbids.
pub const ALLY_PENALTY_TYPE_CLAN_LEAVED: i32 = 1;
pub const ALLY_PENALTY_TYPE_CLAN_DISMISSED: i32 = 2;
pub const ALLY_PENALTY_TYPE_DISMISS_CLAN: i32 = 3;
pub const ALLY_PENALTY_TYPE_DISSOLVE_ALLY: i32 = 4;

/// The only rights bestowable on rank 9 (academy): CL_VIEW_WAREHOUSE (3),
/// CH_OPEN_DOOR (11), CS_OPEN_DOOR (**16**) — Java `RequestPledgePower`'s mask.
/// (The bit was 15 here until the G22 castle-staff slice named the CS_ block:
/// 15 is `CH_SET_FUNCTIONS`, so academy members kept hall-function rights and
/// lost the castle-door right the mask is supposed to grant.)
pub const RANK9_PRIVS_MASK: i32 = CL_VIEW_WAREHOUSE | CH_OPEN_DOOR | CS_OPEN_DOOR;

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

    /// Java `getSubPledgeMembersCount(pledgeType)`.
    pub fn sub_pledge_members_count(&self, pledge_type: i32) -> usize {
        self.members
            .iter()
            .filter(|m| m.pledge_type == pledge_type)
            .count()
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

#[cfg(test)]
mod pledge_class_tests {
    use super::*;

    fn clan_at(level: i32, leader: i32, members: Vec<(i32, i32)>) -> Clan {
        Clan {
            id: 1,
            name: "T".into(),
            leader_id: leader,
            level,
            reputation_score: 0,
            castle_id: 0,
            members: members
                .iter()
                .map(|&(id, pt)| ClanMember {
                    char_id: id,
                    name: format!("P{id}"),
                    level: 1,
                    class_id: 0,
                    sex: 0,
                    race: 0,
                    power_grade: 5,
                    title: String::new(),
                    pledge_type: pt,
                    apprentice: 0,
                    sponsor: 0,
                })
                .collect(),
            skills: Default::default(),
            warehouse: Default::default(),
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            rank_privs: Default::default(),
            new_leader_id: 0,
            sub_pledges: Default::default(),
            ally_id: 0,
            ally_name: String::new(),
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            blood_alliance_count: 0,
        }
    }

    /// Every `(level, leader_class, plain_member_class, academy, royal_member,
    /// knight_member, royal_captain, knight_captain)` row hand-transcribed from
    /// `ClanMember.calculatePledgeClass`'s per-level switch (levels 4..=11).
    #[test]
    fn matches_java_calculate_pledge_class_table() {
        // level 4: leader 3, member 0 (no sub-units possible).
        {
            let c = clan_at(4, 1, vec![(1, 0), (2, 0)]);
            assert_eq!(c.pledge_class_of(1), 3);
            assert_eq!(c.pledge_class_of(2), 0);
        }
        // level 5: leader 4, member 2 (still no per-pledge-type split).
        {
            let c = clan_at(5, 1, vec![(1, 0), (2, 0), (2, -1)]);
            assert_eq!(c.pledge_class_of(1), 4);
            assert_eq!(c.pledge_class_of(2), 2);
        }
        // (leader, default_member, academy, royal_member, royal_captain,
        //  knight_member, knight_captain) per level 6..=11.
        let table: &[(i32, u8, u8, u8, u8, u8, Option<u8>, Option<u8>)] = &[
            (6, 5, 3, 1, 2, 4, None, None),
            (7, 7, 4, 1, 3, 6, Some(2), Some(5)),
            (8, 8, 5, 1, 4, 7, Some(3), Some(6)),
            (9, 9, 6, 1, 5, 8, Some(4), Some(7)),
            (10, 10, 7, 1, 6, 9, Some(5), Some(8)),
            (11, 11, 8, 1, 7, 10, Some(6), Some(9)),
        ];
        for &(
            level,
            leader,
            default_member,
            academy,
            royal_member,
            royal_captain,
            knight_member,
            knight_captain,
        ) in table
        {
            let mut members = vec![
                (1, 0),   // leader, main pledge
                (2, 0),   // plain main-pledge member
                (3, -1),  // academy member
                (4, 100), // royal-unit member
                (5, 0),   // the royal captain: main-pledge member who leads unit 100
            ];
            let mut c = clan_at(level, 1, members.clone());
            c.sub_pledges.insert(
                100,
                SubPledge {
                    id: 100,
                    name: "Royal".into(),
                    leader_id: 5,
                },
            );
            assert_eq!(c.pledge_class_of(1), leader, "level {level} leader");
            assert_eq!(
                c.pledge_class_of(2),
                default_member,
                "level {level} plain member"
            );
            assert_eq!(c.pledge_class_of(3), academy, "level {level} academy");
            assert_eq!(
                c.pledge_class_of(4),
                royal_member,
                "level {level} royal member"
            );
            assert_eq!(
                c.pledge_class_of(5),
                royal_captain,
                "level {level} royal captain"
            );

            if let (Some(km), Some(kc)) = (knight_member, knight_captain) {
                members.push((6, 1001)); // knight-unit member
                members.push((7, 0)); // the knight captain
                c = clan_at(level, 1, members.clone());
                c.sub_pledges.insert(
                    100,
                    SubPledge {
                        id: 100,
                        name: "Royal".into(),
                        leader_id: 5,
                    },
                );
                c.sub_pledges.insert(
                    1001,
                    SubPledge {
                        id: 1001,
                        name: "Knights".into(),
                        leader_id: 7,
                    },
                );
                assert_eq!(c.pledge_class_of(6), km, "level {level} knight member");
                assert_eq!(c.pledge_class_of(7), kc, "level {level} knight captain");
            }
        }
    }
}
