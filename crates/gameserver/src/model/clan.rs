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
    pub members: Vec<ClanMember>,
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
