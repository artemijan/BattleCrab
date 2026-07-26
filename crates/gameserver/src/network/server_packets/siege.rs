//! `serverpackets/SiegeInfo` (`CASTLE_SIEGE_INFO`, 0xC9) — the castle-siege
//! registration/roster window shown to a clan leader.

use commons::network::PacketWriter;

use super::opcodes;

/// Build the `SiegeInfo` window. `can_set_time` is Java's
/// `(ownerId == player.getClanId()) && player.isClanLeader()`. When
/// `hour_options` is non-empty (the owner-leader may still pick the hour —
/// `!isTimeRegistrationOver()`), the packet sends the selectable siege-time list
/// instead of a fixed date; otherwise it sends `siege_date_secs`.
#[allow(clippy::too_many_arguments)]
pub fn siege_info(
    castle_id: i32,
    can_set_time: bool,
    owner_id: i32,
    owner_name: &str,
    owner_leader: &str,
    owner_ally_id: i32,
    owner_ally_name: &str,
    now_secs: i32,
    siege_date_secs: i32,
    hour_options: &[i32],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CASTLE_SIEGE_INFO);
    w.write_i32(castle_id);
    w.write_i32(can_set_time as i32);
    w.write_i32(owner_id);
    if owner_id > 0 {
        w.write_string(owner_name);
        w.write_string(owner_leader);
        w.write_i32(owner_ally_id);
        w.write_string(owner_ally_name);
    } else {
        w.write_string("");
        w.write_string("");
        w.write_i32(0);
        w.write_string("");
    }
    w.write_i32(now_secs);
    if hour_options.is_empty() {
        // Registration is over (or the viewer can't set time): the fixed date.
        w.write_i32(siege_date_secs);
        w.write_i32(0);
    } else {
        // `Config.SIEGE_HOUR_LIST`: the owner picks from these siege times.
        w.write_i32(0);
        w.write_i32(hour_options.len() as i32);
        for &secs in hour_options {
            w.write_i32(secs);
        }
    }
    w.into_bytes()
}

/// One row of the attacker roster.
pub struct AttackerEntry {
    pub clan_id: i32,
    pub name: String,
    pub leader_name: String,
    pub crest_id: i32,
    pub ally_id: i32,
    pub ally_name: String,
    pub ally_crest_id: i32,
}

/// `serverpackets/SiegeAttackerList` (`CASTLE_SIEGE_ATTACKER_LIST`, 0xCA) — the
/// castle's registered attacker clans.
pub fn siege_attacker_list(castle_id: i32, attackers: &[AttackerEntry]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CASTLE_SIEGE_ATTACKER_LIST);
    w.write_i32(castle_id);
    w.write_i32(0);
    w.write_i32(1);
    w.write_i32(0);
    w.write_i32(attackers.len() as i32);
    w.write_i32(attackers.len() as i32);
    for a in attackers {
        w.write_i32(a.clan_id);
        w.write_string(&a.name);
        w.write_string(&a.leader_name);
        w.write_i32(a.crest_id);
        w.write_i32(0); // signed time (seconds) — not stored
        w.write_i32(a.ally_id);
        w.write_string(&a.ally_name);
        w.write_string(""); // ally leader name (Java writes "")
        w.write_i32(a.ally_crest_id);
    }
    w.into_bytes()
}

/// One row of the defender roster.
pub struct DefenderEntry {
    pub clan_id: i32,
    pub name: String,
    pub leader_name: String,
    pub crest_id: i32,
    /// Java `SiegeClanType.ordinal() + 1`: owner 1, defender-pending 2, defender 3.
    pub type_value: i32,
    pub ally_id: i32,
    pub ally_name: String,
    pub ally_leader_name: String,
    pub ally_crest_id: i32,
}

/// `serverpackets/SiegeDefenderList` (`CASTLE_SIEGE_DEFENDER_LIST`, 0xCB) — the
/// owner's defender-management list (owner + confirmed + pending defenders).
pub fn siege_defender_list(
    castle_id: i32,
    valid_registration: bool,
    defenders: &[DefenderEntry],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CASTLE_SIEGE_DEFENDER_LIST);
    w.write_i32(castle_id);
    w.write_i32(0); // unknown
    w.write_i32(valid_registration as i32);
    w.write_i32(0); // unknown
    w.write_i32(defenders.len() as i32);
    w.write_i32(defenders.len() as i32);
    for d in defenders {
        w.write_i32(d.clan_id);
        w.write_string(&d.name);
        w.write_string(&d.leader_name);
        w.write_i32(d.crest_id);
        w.write_i32(0); // signed time (seconds)
        w.write_i32(d.type_value);
        w.write_i32(d.ally_id);
        if d.ally_id != 0 {
            w.write_string(&d.ally_name);
            w.write_string(&d.ally_leader_name);
            w.write_i32(d.ally_crest_id);
        } else {
            w.write_string("");
            w.write_string("");
            w.write_i32(0);
        }
    }
    w.into_bytes()
}
