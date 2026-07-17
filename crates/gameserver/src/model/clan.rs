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
    /// == 0`), the only pledge type the port models (no academy/royal/order
    /// sub-pledges yet). Interlude clan levels cap at 8; a clan below level 4
    /// yields 0 for everyone, so the on-head rank crown only appears once the
    /// clan is developed enough — matching retail.
    pub fn pledge_class_of(&self, char_id: i32) -> u8 {
        let is_leader = char_id == self.leader_id;
        match self.level {
            4 if is_leader => 3,
            5 => if is_leader { 4 } else { 2 },
            6 => if is_leader { 5 } else { 3 },
            7 => if is_leader { 7 } else { 4 },
            8 => if is_leader { 8 } else { 5 },
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

/// `ClanPrivilege.CL_VIEW_WAREHOUSE` (ordinal 3) — required to withdraw from
/// the clan warehouse.
pub const CL_VIEW_WAREHOUSE: i32 = 1 << 3;

impl Clan {
    /// Whether `char_id` holds `privilege` (a `CL_*` bit): the leader always
    /// does (Java `Player.hasClanPrivilege` short-circuits for the leader),
    /// otherwise it's tested against the member's stored `clan_privs`.
    pub fn has_privilege(&self, char_id: i32, member_privs: i32, privilege: i32) -> bool {
        char_id == self.leader_id || (member_privs & privilege) != 0
    }
}
