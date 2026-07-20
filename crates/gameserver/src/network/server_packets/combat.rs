//! Attack, death/revive, combat-stance and social-action packets.

use commons::network::PacketWriter;

use super::opcodes;

/// One rolled hit inside an `Attack` packet (Java `model/Hit`): flag bits
/// from `enums/AttackType` (miss 0x01 within flags... see `hit_flags`).
#[derive(Debug, Clone, Copy)]
pub struct AttackHit {
    pub target_object_id: i32,
    pub damage: i32,
    pub miss: bool,
    pub crit: bool,
    /// A charged soulshot was spent on this hit (`SHOT_USED` flag + `getGrade`):
    /// the client plays the soulshot swing animation.
    pub soulshot: bool,
    /// `Hit.getGrade()` — the weapon's crystal-grade ordinal, sent only with a
    /// soulshot hit (0 otherwise).
    pub ss_grade: i32,
}

/// Java `enums/AttackType` masks folded by `Hit`'s constructor: `MISSED` =
/// 0x01, `BLOCKED` = 0x02 (never set — no shield defence), `CRITICAL` = 0x04,
/// `SHOT_USED` = 0x08 (a charged soulshot was spent).
fn hit_flags(hit: &AttackHit) -> i32 {
    if hit.miss {
        return 0x01;
    }
    let mut flags = if hit.crit { 0x04 } else { 0 };
    if hit.soulshot {
        flags |= 0x08;
    }
    flags
}

/// Port of `serverpackets/Attack` (single-hit melee shape — the trailing
/// extra-hit list is empty, matching non-dual weapons).
/// One swing, which may land **several hits**: a dual weapon strikes twice, and
/// a polearm sweep adds one hit per extra target (Java `Attack.addHit`). `hits`
/// must be non-empty; the first is written inline and the rest follow the
/// `writeShort(size - 1)` count.
pub fn attack(attacker_object_id: i32, hits: &[AttackHit], ax: i32, ay: i32, az: i32, tx: i32, ty: i32, tz: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    let Some(first) = hits.first() else { return Vec::new() };
    w.write_u8(opcodes::ATTACK);
    w.write_i32(attacker_object_id);
    w.write_i32(first.target_object_id);
    w.write_i32(0); // soulshot visual substitute (brooch jewels)
    w.write_i32(first.damage);
    w.write_i32(hit_flags(first));
    w.write_i32(first.ss_grade); // Hit.getGrade() — weapon crystal grade on a soulshot hit
    w.write_i32(ax);
    w.write_i32(ay);
    w.write_i32(az);
    w.write_i16((hits.len() - 1) as i16);
    for hit in &hits[1..] {
        w.write_i32(hit.target_object_id);
        w.write_i32(hit.damage);
        w.write_i32(hit_flags(hit));
        w.write_i32(hit.ss_grade);
    }
    w.write_i32(tx);
    w.write_i32(ty);
    w.write_i32(tz);
    w.into_bytes()
}

/// Port of `serverpackets/Die` — broadcast on any creature's death. Every
/// revive-destination flag is written explicitly; for NPCs they are all
/// false. `to_village` = `canRevive() && !isPendingRevive()` for players.
pub fn die(object_id: i32, to_village: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::DIE);
    w.write_i32(object_id);
    w.write_i32(to_village as i32); // to village
    w.write_i32(0); // to clan hall
    w.write_i32(0); // to castle
    w.write_i32(0); // to outpost / siege HQ
    w.write_i32(0); // sweepable
    w.write_i32(0); // use feather
    w.write_i32(0); // to fortress
    w.write_i32(0); // disables feather button timer
    w.write_i32(0); // adventure's song
    w.write_u8(0); // hide die animation
    w.write_i32(0); // items enabled
    w.write_i32(0); // item count
    w.into_bytes()
}

/// `ChangeWaitType`'s `newMoveType` constants. Only the fake-death pair is
/// used here; sitting/standing wait for the sitting state (`TODO(G29)`).
pub mod wait_type {
    pub const _SITTING: i32 = 0;
    pub const _STANDING: i32 = 1;
    pub const START_FAKEDEATH: i32 = 2;
    pub const STOP_FAKEDEATH: i32 = 3;
}

/// Port of `serverpackets/ChangeWaitType` (0x29) — tells every observing
/// client to drop the character into (or out of) the fake-death pose. Carries
/// the position so late-arriving observers place the body correctly.
pub fn change_wait_type(object_id: i32, move_type: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHANGE_WAIT_TYPE);
    w.write_i32(object_id);
    w.write_i32(move_type);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.into_bytes()
}

/// Port of `serverpackets/Revive`.
pub fn revive(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::REVIVE);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/AutoAttackStart` — combat stance begins.
pub fn auto_attack_start(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::AUTO_ATTACK_START);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/AutoAttackStop` — combat stance ends (15 s after
/// the last swing, `AttackStanceTaskManager.COMBAT_TIME`).
pub fn auto_attack_stop(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::AUTO_ATTACK_STOP);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/SocialAction` (also carries the level-up effect,
/// `SocialAction.LEVEL_UP` = 2122).
pub const SOCIAL_ACTION_LEVEL_UP: i32 = 2122;
pub fn social_action(object_id: i32, action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SOCIAL_ACTION);
    w.write_i32(object_id);
    w.write_i32(action_id);
    w.write_i32(0);
    w.into_bytes()
}

// ---------------------------------------------------------------------------
// Duels (G20)
// ---------------------------------------------------------------------------

fn ex(opcode: i16) -> PacketWriter {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcode);
    w
}

/// `ExDuelAskStart` — "X challenges you to a duel" prompt.
pub fn ex_duel_ask_start(requestor_name: &str, party_duel: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_DUEL_ASK_START);
    w.write_string(requestor_name);
    w.write_i32(party_duel);
    w.into_bytes()
}

/// `ExDuelReady` — the countdown finished; the client shows "ready".
pub fn ex_duel_ready(party_duel: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_DUEL_READY);
    w.write_i32(party_duel);
    w.into_bytes()
}

/// `ExDuelStart` — the duel is live.
pub fn ex_duel_start(party_duel: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_DUEL_START);
    w.write_i32(party_duel);
    w.into_bytes()
}

/// `ExDuelEnd` — closes the duel UI.
pub fn ex_duel_end(party_duel: i32) -> Vec<u8> {
    let mut w = ex(opcodes::EX_DUEL_END);
    w.write_i32(party_duel);
    w.into_bytes()
}

/// `ExDuelUpdateUserInfo` — the opponent's bars in the duel panel.
#[allow(clippy::too_many_arguments)]
pub fn ex_duel_update_user_info(
    name: &str,
    object_id: i32,
    class_id: i32,
    cur_hp: i32,
    max_hp: i32,
    cur_mp: i32,
    max_mp: i32,
    cur_cp: i32,
    max_cp: i32,
    level: i32,
) -> Vec<u8> {
    let mut w = ex(opcodes::EX_DUEL_UPDATE_USER_INFO);
    w.write_string(name);
    w.write_i32(object_id);
    w.write_i32(class_id);
    w.write_i32(level);
    w.write_i32(cur_hp);
    w.write_i32(max_hp);
    w.write_i32(cur_mp);
    w.write_i32(max_mp);
    w.write_i32(cur_cp);
    w.write_i32(max_cp);
    w.into_bytes()
}
