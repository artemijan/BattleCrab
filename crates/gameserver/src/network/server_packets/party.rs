//! Party packets (G10).

use commons::network::PacketWriter;

use super::opcodes;

/// The per-member fields the party-window packets write, gathered from the
/// member's components by `game_loop/party.rs`.
#[derive(Debug, Clone)]
pub struct PartyMemberView {
    pub object_id: i32,
    pub name: String,
    pub cp: i32,
    pub max_cp: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub vitality: i32,
    pub level: i32,
    pub class_id: i32,
    pub race: i32,
    /// The member's pet and servitors, in Java's order (pet first). Empty
    /// until G29 — the count used to be hard-coded to 0, so a party member's
    /// summon never appeared in anyone else's party window.
    pub summons: Vec<PartySummonView>,
}

/// One summon row inside a party-window member entry.
#[derive(Debug, Clone)]
pub struct PartySummonView {
    pub object_id: i32,
    /// Java writes `getId() + 1000000` — the client's summon-template space.
    pub npc_id: i32,
    /// 1 = pet, 2 = servitor (the same discriminator `PetInfo` carries).
    pub summon_type: u8,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub level: i32,
}

/// Java's offset into the client's summon-template space.
const SUMMON_TEMPLATE_OFFSET: i32 = 1_000_000;

fn write_summons(w: &mut PacketWriter, summons: &[PartySummonView]) {
    w.write_i32(summons.len() as i32);
    for s in summons {
        w.write_i32(s.object_id);
        w.write_i32(s.npc_id + SUMMON_TEMPLATE_OFFSET);
        w.write_u8(s.summon_type);
        w.write_string(&s.name);
        w.write_i32(s.hp);
        w.write_i32(s.max_hp);
        w.write_i32(s.mp);
        w.write_i32(s.max_mp);
        w.write_u8(s.level as u8);
    }
}

/// `serverpackets/AskJoinParty`.
pub fn ask_join_party(requestor_name: &str, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ASK_JOIN_PARTY);
    w.write_string(requestor_name);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

/// `serverpackets/JoinParty` — the answer echoed to the requestor. The
/// trailing int is a field Java labels "Find me!" and does not identify;
/// written 0 there too.
pub fn join_party(response: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::JOIN_PARTY);
    w.write_i32(response);
    w.write_i32(0);
    w.into_bytes()
}

fn write_party_member(w: &mut PacketWriter, m: &PartyMemberView) {
    w.write_i32(m.object_id);
    w.write_string(&m.name);
    w.write_i32(m.cp);
    w.write_i32(m.max_cp);
    w.write_i32(m.hp);
    w.write_i32(m.max_hp);
    w.write_i32(m.mp);
    w.write_i32(m.max_mp);
    w.write_i32(m.vitality);
    w.write_u8(m.level as u8);
    w.write_i16(m.class_id as i16);
}

/// `serverpackets/PartySmallWindowAll` — the receiver's full party window
/// (every member **except the receiver**, leader first).
pub fn party_small_window_all(
    leader_object_id: i32,
    loot_rule_id: i32,
    others: &[PartyMemberView],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_ALL);
    w.write_i32(leader_object_id);
    w.write_u8(loot_rule_id as u8);
    w.write_u8(others.len() as u8);
    for m in others {
        write_party_member(&mut w, m);
        w.write_u8(1); // unk
        w.write_i16(m.race as i16);
        write_summons(&mut w, &m.summons);
    }
    w.into_bytes()
}

/// `serverpackets/PartySmallWindowAdd` — one new member for existing members'
/// windows (note: loot rule is an **int** here, a byte in `…All`).
pub fn party_small_window_add(
    leader_object_id: i32,
    loot_rule_id: i32,
    member: &PartyMemberView,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_ADD);
    w.write_i32(leader_object_id);
    w.write_i32(loot_rule_id);
    write_party_member(&mut w, member);
    w.write_u8(0);
    w.write_i16(member.race as i16);
    w.into_bytes()
}

/// `serverpackets/PartySmallWindowDelete`.
pub fn party_small_window_delete(object_id: i32, name: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_DELETE);
    w.write_i32(object_id);
    w.write_string(name);
    w.into_bytes()
}

/// `serverpackets/PartySmallWindowDeleteAll`.
pub fn party_small_window_delete_all() -> Vec<u8> {
    vec![opcodes::PARTY_SMALL_WINDOW_DELETE_ALL]
}

/// `PartySmallWindowUpdateType` masks — **natural** bit values written as one
/// short (`writeShort(_flags)`), NOT the reversed-array masked-packet scheme.
pub mod party_window_flags {
    pub const CURRENT_CP: u16 = 1;
    pub const MAX_CP: u16 = 2;
    pub const CURRENT_HP: u16 = 4;
    pub const MAX_HP: u16 = 8;
    pub const CURRENT_MP: u16 = 16;
    pub const MAX_MP: u16 = 32;
    pub const LEVEL: u16 = 64;
    pub const CLASS_ID: u16 = 128;
    pub const PARTY_SUBSTITUTE: u16 = 256;
    pub const VITALITY_POINTS: u16 = 512;
    /// The CP/HP/MP block `broadcastStatusUpdate` sends.
    pub const VITALS: u16 = CURRENT_CP | MAX_CP | CURRENT_HP | MAX_HP | CURRENT_MP | MAX_MP;
    /// `PartySmallWindowUpdate(member, true)` — every component.
    pub const ALL: u16 = VITALS | LEVEL | CLASS_ID | PARTY_SUBSTITUTE | VITALITY_POINTS;
}

/// `serverpackets/PartySmallWindowUpdate` — masked member-field refresh for
/// the other members' windows.
pub fn party_small_window_update(m: &PartyMemberView, flags: u16) -> Vec<u8> {
    use party_window_flags as f;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_SMALL_WINDOW_UPDATE);
    w.write_i32(m.object_id);
    w.write_i16(flags as i16);
    for (flag, value) in [
        (f::CURRENT_CP, m.cp),
        (f::MAX_CP, m.max_cp),
        (f::CURRENT_HP, m.hp),
        (f::MAX_HP, m.max_hp),
        (f::CURRENT_MP, m.mp),
        (f::MAX_MP, m.max_mp),
    ] {
        if flags & flag != 0 {
            w.write_i32(value);
        }
    }
    if flags & f::LEVEL != 0 {
        w.write_u8(m.level as u8);
    }
    if flags & f::CLASS_ID != 0 {
        w.write_i16(m.class_id as i16);
    }
    if flags & f::PARTY_SUBSTITUTE != 0 {
        w.write_u8(0);
    }
    if flags & f::VITALITY_POINTS != 0 {
        w.write_i32(m.vitality);
    }
    w.into_bytes()
}

/// `serverpackets/PartyMemberPosition` — the 12 s member-location refresh.
pub fn party_member_position(locations: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PARTY_MEMBER_POSITION);
    w.write_i32(locations.len() as i32);
    for &(object_id, x, y, z) in locations {
        w.write_i32(object_id);
        w.write_i32(x);
        w.write_i32(y);
        w.write_i32(z);
    }
    w.into_bytes()
}

/// `serverpackets/ExAskModifyPartyLooting` — the leader wants a new loot rule.
pub fn ex_ask_modify_party_looting(leader_name: &str, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_ASK_MODIFY_PARTY_LOOTING);
    w.write_string(leader_name);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

/// `serverpackets/ExSetPartyLooting` — the loot-rule change verdict.
pub fn ex_set_party_looting(result: i32, loot_rule_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SET_PARTY_LOOTING);
    w.write_i32(result);
    w.write_i32(loot_rule_id);
    w.into_bytes()
}

/// `ExInzoneWaiting` — the `/instancezone` re-enter window: the template id of
/// the instance the player is standing in (`-1` for the overworld) and one
/// `(templateId, secondsLeft)` pair per instance still on cooldown. Java's
/// leading byte is `!hide`, and the command always passes `hide = false`.
pub fn ex_inzone_waiting(current_template_id: i32, reuse: &[(i32, i32)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_INZONE_WAITING_INFO);
    w.write_u8(1); // !hide
    w.write_i32(current_template_id);
    w.write_i32(reuse.len() as i32);
    for (template_id, seconds) in reuse {
        w.write_i32(*template_id);
        w.write_i32(*seconds);
    }
    w.into_bytes()
}
