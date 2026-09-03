//! Combat and skill packets: skill use, action and attack requests, the
//! target/dispel/restart follow-ups, and the duel invites.

use commons::network::PacketReader;

/// Port of `clientpackets/RequestMagicSkillUse` (`cdc`). `shift_pressed` is
/// Java's `dontMove`: an out-of-range shift-cast is cancelled (SM 748)
/// instead of walking into range. Ground targeting still waits on a later
/// milestone.
pub struct RequestMagicSkillUse {
    pub magic_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestMagicSkillUse {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let magic_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        let shift_pressed = r.read_u8().is_some_and(|b| b != 0);
        Some(Self {
            magic_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}

/// Port of `clientpackets/RequestExMagicSkillUseGround` (ex 0x41) — a
/// `targetType GROUND` cast aimed at a world position (format `dddddc`).
pub struct RequestExMagicSkillUseGround {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub skill_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestExMagicSkillUseGround {
    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let x = r.read_i32()?;
        let y = r.read_i32()?;
        let z = r.read_i32()?;
        let skill_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? != 0;
        let shift_pressed = r.read_u8().is_some_and(|b| b != 0);
        Some(Self {
            x,
            y,
            z,
            skill_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}

/// Port of `clientpackets/RequestAcquireSkill`. `sub_type` is only meaningful
/// for `AcquireSkillType::Subpledge` (id `3`) — out of scope here (see the G6
/// plan's "only `CLASS`" note), read anyway to keep the reader positioned
/// correctly if the client ever sends it.
pub struct RequestAcquireSkill {
    pub skill_id: i32,
    pub skill_level: i32,
    pub acquire_type: i32,
}

impl RequestAcquireSkill {
    pub const CLASS: i32 = 0;
    pub const PLEDGE: i32 = 2;
    pub const SUBPLEDGE: i32 = 3;

    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let skill_id = r.read_i32()?;
        let skill_level = r.read_i32()?;
        let acquire_type = r.read_i32()?;
        if acquire_type == Self::SUBPLEDGE {
            r.read_i32()?; // sub_type — unused (see doc comment)
        }
        Some(Self {
            skill_id,
            skill_level,
            acquire_type,
        })
    }
}

/// Port of `clientpackets/Action` (`cdddc`). Origin x/y/z are the client's own
/// echoed position — Java reads them but never uses them (`@SuppressWarnings
/// ("unused")` on all three), so they're dropped here too.
pub struct Action {
    pub object_id: i32,
    pub action_id: u8,
}

impl Action {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        r.read_i32()?; // origin_x — unused
        r.read_i32()?; // origin_y — unused
        r.read_i32()?; // origin_z — unused
        let action_id = r.read_u8()?;
        Some(Self {
            object_id,
            action_id,
        })
    }
}

/// Port of `clientpackets/AttackRequest` (`cddddc`) — the client clicking an
/// attackable creature. The origin coordinates are read and discarded like
/// Java's unused fields; the trailing `attackId` byte (`0` = simple click, `1`
/// = shift-click) is Java's `dontMove` flag — Java ignores it, but we honour it
/// so a shift-attack refuses to chase (see `start_attack_intent`).
pub struct AttackRequest {
    pub object_id: i32,
    pub shift: bool,
}

impl AttackRequest {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let object_id = r.read_i32()?;
        r.read_i32()?; // origin_x — unused
        r.read_i32()?; // origin_y — unused
        r.read_i32()?; // origin_z — unused
        let shift = r.read_u8()? == 1;
        Some(Self { object_id, shift })
    }
}

/// Port of `clientpackets/RequestRestartPoint` (`cd`) — the death dialog's
/// revive choice (0 = to village; the clan-hall/castle/fixed variants need
/// systems that don't exist yet).
pub struct RequestRestartPoint {
    pub point_type: i32,
}

impl RequestRestartPoint {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let point_type = r.read_i32()?;
        Some(Self { point_type })
    }
}

/// Port of `clientpackets/RequestTargetCanceld` (`ch`): a single flag, nonzero
/// meaning "the client wants its target cleared".
pub struct RequestTargetCanceld {
    pub target_lost: bool,
}

impl RequestTargetCanceld {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let target_lost = r.read_i16()? != 0;
        Some(Self { target_lost })
    }
}

/// Port of `clientpackets/RequestDispel` (ex `dddhh`): the alt+click buff-cancel
/// on a buff icon. `object_id` is whose buff (self, pet, or servitor);
/// `skill_id`/`skill_level`/`skill_sub_level` identify the buff to strip.
pub struct RequestDispel {
    pub object_id: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    pub skill_sub_level: i32,
}

impl RequestDispel {
    /// `readImpl`: readInt objectId, readInt skillId, readShort skillLevel,
    /// readShort skillSubLevel. Called with the body after the 2-byte sub-opcode.
    pub fn read(ex_body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(ex_body);
        let object_id = r.read_i32()?;
        let skill_id = r.read_i32()?;
        let skill_level = r.read_i16()? as i32;
        let skill_sub_level = r.read_i16()? as i32;
        Some(Self {
            object_id,
            skill_id,
            skill_level,
            skill_sub_level,
        })
    }
}

/// The single-int henna packets (`RequestHennaEquip`/`Remove`/`ItemInfo`/
/// `ItemRemoveInfo`): one int `symbolId`.
pub fn read_symbol_id(body: &[u8]) -> Option<i32> {
    PacketReader::new(body).read_i32()
}

/// `RequestDuelStart` — the challenged player's name and the party-duel flag.
pub fn read_duel_start(body: &[u8]) -> Option<(String, i32)> {
    let mut r = PacketReader::new(body);
    let name = r.read_string()?;
    let party_duel = r.read_i32().unwrap_or(0);
    Some((name, party_duel))
}

/// `RequestDuelAnswerStart` — reads `partyDuel`, an unused field, then the
/// response (1 accepts, anything else declines).
pub fn read_duel_answer(body: &[u8]) -> Option<i32> {
    let mut r = PacketReader::new(body);
    let _party_duel = r.read_i32()?;
    let _unused = r.read_i32().unwrap_or(0);
    Some(r.read_i32().unwrap_or(0))
}

/// Port of `clientpackets/RequestActionUse` — the action bar's non-skill
/// buttons (sit/stand, socials, and the servitor commands).
#[derive(Debug, Clone, Copy)]
pub struct RequestActionUse {
    pub action_id: i32,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl RequestActionUse {
    pub fn read(body: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body);
        let action_id = r.read_i32()?;
        let ctrl_pressed = r.read_i32()? == 1;
        let shift_pressed = r.read_u8()? == 1;
        Some(Self {
            action_id,
            ctrl_pressed,
            shift_pressed,
        })
    }
}
