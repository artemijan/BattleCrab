//! Lobby / enter-world lifecycle packets: the protocol handshake reply, the
//! character-selection screen, character create/delete acks, and the
//! select/restart/logout transitions.

use commons::network::PacketWriter;

use super::opcodes;
use crate::model::inventory::PaperdollSlot;

/// Port of `serverpackets/KeyPacket` — the reply to `ProtocolVersion`. Hands the
/// client the first 8 bytes of the cipher key and the crypt/server flags.
///
/// * `key8` — first 8 bytes of the 16-byte cipher key (the static tail is
///   hard-coded in the client).
/// * `result` — 1 = protocol ok, 0 = wrong protocol.
pub fn key_packet(
    key8: &[u8; 8],
    result: u8,
    packet_encryption: bool,
    server_id: i32,
    is_classic: bool,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::VERSION_CHECK);
    w.write_u8(result); // 0 - wrong protocol, 1 - protocol ok
    for b in key8 {
        w.write_u8(*b);
    }
    w.write_i32(packet_encryption as i32); // use blowfish encryption
    w.write_i32(server_id);
    w.write_u8(1);
    w.write_i32(0); // obfuscation key
    w.write_u8(is_classic as u8);
    w.into_bytes()
}

/// Port of `serverpackets/LoginFail`. `LoginFail.LOGIN_SUCCESS` = `(-1, 0)`.
pub fn login_fail(success: i32, reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LOGIN_FAIL);
    w.write_i32(success);
    w.write_i32(reason);
    w.into_bytes()
}

/// `LoginFail.LOGIN_SUCCESS`.
pub fn login_success() -> Vec<u8> {
    login_fail(-1, 0)
}

/// `CharSelectionInfo.PAPERDOLL_ORDER` — this packet overrides the base order
/// with its own 33 slots, which leave out the five agathions (the five
/// "Salvation" ints written after it stand where they would go).
const CHAR_SELECT_PAPERDOLL_ORDER: [PaperdollSlot; 33] = [
    PaperdollSlot::Under,
    PaperdollSlot::REar,
    PaperdollSlot::LEar,
    PaperdollSlot::Neck,
    PaperdollSlot::RFinger,
    PaperdollSlot::LFinger,
    PaperdollSlot::Head,
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::Cloak,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
    PaperdollSlot::RBracelet,
    PaperdollSlot::LBracelet,
    PaperdollSlot::Deco1,
    PaperdollSlot::Deco2,
    PaperdollSlot::Deco3,
    PaperdollSlot::Deco4,
    PaperdollSlot::Deco5,
    PaperdollSlot::Deco6,
    PaperdollSlot::Belt,
    PaperdollSlot::Brooch,
    PaperdollSlot::BroochJewel1,
    PaperdollSlot::BroochJewel2,
    PaperdollSlot::BroochJewel3,
    PaperdollSlot::BroochJewel4,
    PaperdollSlot::BroochJewel5,
    PaperdollSlot::BroochJewel6,
];

/// `CharSelectionInfo.PAPERDOLL_ORDER_VISUAL_ID` — this packet overrides the
/// `ServerPacket` default with its own 9-slot visual-id order.
const CHAR_SELECT_PAPERDOLL_VISUAL_ORDER: [PaperdollSlot; 9] = [
    PaperdollSlot::RHand,
    PaperdollSlot::LHand,
    PaperdollSlot::Gloves,
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Feet,
    PaperdollSlot::RHand,
    PaperdollSlot::Hair,
    PaperdollSlot::Hair2,
];

/// The five armor slots whose enchant effect the lobby shows, in write order.
const CHAR_SELECT_ENCHANT_ORDER: [PaperdollSlot; 5] = [
    PaperdollSlot::Chest,
    PaperdollSlot::Legs,
    PaperdollSlot::Head,
    PaperdollSlot::Gloves,
    PaperdollSlot::Feet,
];

/// The lobby slot to highlight when the caller gave none: Java's scan for the
/// highest `lastAccess`, where the first of equal values wins and a list whose
/// every `lastAccess` is 0 highlights nothing (-1).
fn lobby_active_id(chars: &[crate::db::CharData]) -> i32 {
    let mut last_access = 0;
    let mut active_id = -1;
    for (i, c) in chars.iter().enumerate() {
        if last_access < c.last_access {
            last_access = c.last_access;
            active_id = i as i32;
        }
    }
    active_id
}

/// The world state `CharSelectionInfo` reads besides the character rows.
pub struct CharSelectCtx<'a> {
    /// `ServerConfig.MAX_CHARACTERS_NUMBER_PER_ACCOUNT`.
    pub max_characters: i32,
    pub exp: &'a crate::data::ExperienceData,
    /// For the hero glow (Java `Hero.isHero`).
    pub olympiad: &'a crate::model::olympiad::OlympiadState,
    /// `RatesConfig.RATE_VITALITY_EXP_MULTIPLIER`.
    pub rate_vitality_exp_multiplier: f64,
}

/// `AbstractVariables.getBoolean`: `Boolean.parseBoolean` of the stored value.
fn variable_bool(c: &crate::db::CharData, key: &str, default: bool) -> bool {
    c.variables
        .iter()
        .find(|(k, _)| k == key)
        .map_or(default, |(_, v)| v.eq_ignore_ascii_case("true"))
}

/// `AbstractVariables.getInt`.
fn variable_int(c: &crate::db::CharData, key: &str, default: i32) -> i32 {
    c.variables
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(default)
}

/// Port of `serverpackets/CharSelectionInfo` (protocol 140). Augmentation and
/// visual ids are zero until those systems are ported.
pub fn char_selection_info(
    login_name: &str,
    session_id: i32,
    chars: &[crate::db::CharData],
    active_id: i32,
    ctx: &CharSelectCtx<'_>,
) -> Vec<u8> {
    let now = commons::util::now_millis();
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHARACTER_SELECTION_INFO);
    let size = chars.len() as i32;
    w.write_i32(size); // created character count
    w.write_i32(ctx.max_characters);
    w.write_u8((size == ctx.max_characters) as u8); // 1 = can't create new char
    w.write_u8(1); // 1 = can play free until level 85
    w.write_i32(2); // client region flag
    w.write_u8(0); // gift message for inactive accounts
    // Balthus Knights: `BalthusKnightsEnabled` ships 0, which writes 0. The
    // premium branches arrive with the Balthus start (S12).
    w.write_u8(0);

    let active_id = if active_id == -1 {
        lobby_active_id(chars)
    } else {
        active_id
    };

    for (i, c) in chars.iter().enumerate() {
        w.write_string(&c.name);
        w.write_i32(c.object_id);
        w.write_string(login_name);
        w.write_i32(session_id);
        w.write_i32(0); // clan id
        w.write_i32(0); // builder level
        w.write_i32(c.sex);
        w.write_i32(c.race);
        w.write_i32(c.base_class_id);
        w.write_i32(1); // game server name
        w.write_i32(c.x);
        w.write_i32(c.y);
        w.write_i32(c.z);
        w.write_f64(c.cur_hp);
        w.write_f64(c.cur_mp);
        w.write_i64(c.sp);
        w.write_i64(c.exp);
        w.write_f64(exp_percent(ctx.exp, c.exp, c.level));
        w.write_i32(c.level);
        w.write_i32(c.reputation);
        w.write_i32(c.pk_kills);
        w.write_i32(c.pvp_kills);
        for _ in 0..9 {
            w.write_i32(0); // 7 reserved + 2 Ertheia
        }
        // Per-character paperdoll (Java `CharacterInfoHolder`), read from the
        // `items` rows loaded alongside the character.
        let inv = crate::model::inventory::Inventory::from_rows(&c.items);
        for slot in CHAR_SELECT_PAPERDOLL_ORDER {
            let item_id = inv.paperdoll_item_id(slot);
            w.write_i32(item_id);
        }
        for _ in 0..5 {
            w.write_i32(0); // Salvation
        }
        for slot in CHAR_SELECT_PAPERDOLL_VISUAL_ORDER {
            let visual_id = inv.paperdoll_visual_id(slot); // always 0 (appearance: later milestone)
            w.write_i32(visual_id);
        }
        for slot in CHAR_SELECT_ENCHANT_ORDER {
            let enchant = inv.paperdoll_enchant_level(slot);
            w.write_i16(enchant as i16);
        }
        w.write_i32(c.hair_style);
        w.write_i32(c.hair_color);
        w.write_i32(c.face);
        w.write_f64(c.max_hp as f64);
        w.write_f64(c.max_mp as f64);
        w.write_i32(if c.access_level < 0 {
            -1 // banned
        } else if c.delete_time > 0 {
            ((c.delete_time - now) / 1000) as i32
        } else {
            0
        });
        w.write_i32(c.class_id);
        w.write_i32((i as i32 == active_id) as i32);
        w.write_u8(inv.paperdoll_enchant_level(PaperdollSlot::RHand).min(127) as u8); // rhand weapon enchant (capped 127)
        w.write_i32(0); // augmentation option 1
        w.write_i32(0); // augmentation option 2
        w.write_i32(0); // transform: retail shows none at character select
        w.write_i32(0); // pet npc id
        w.write_i32(0); // pet level
        w.write_i32(0); // pet food
        w.write_i32(0); // pet food level
        w.write_f64(0.0); // pet hp
        w.write_f64(0.0); // pet mp
        w.write_i32(c.vitality_points);
        // Java's `(int) RATE_VITALITY_EXP_MULTIPLIER * 100`: the cast binds
        // to the multiplier, so 2.5 shows as 200.
        w.write_i32((ctx.rate_vitality_exp_multiplier as i32).wrapping_mul(100));
        w.write_i32(variable_int(
            c,
            crate::model::components::player::VITALITY_ITEMS_USED,
            0,
        ));
        w.write_i32((c.access_level != -100) as i32); // char active
        w.write_u8(c.noble as u8);
        w.write_u8(ctx.olympiad.is_hero(c.object_id) as u8); // hero glow
        w.write_u8(variable_bool(
            c,
            crate::model::components::player::HAIR_ACCESSORY_ENABLED,
            true,
        ) as u8);
    }
    w.into_bytes()
}

fn exp_percent(exp: &crate::data::ExperienceData, current_exp: i64, level: i32) -> f64 {
    let base = exp.exp_for_level(level);
    let next = exp.exp_for_level(level + 1);
    let denom = next - base;
    if denom <= 0 {
        0.0
    } else {
        (current_exp - base) as f64 / denom as f64
    }
}

/// `serverpackets/NewCharacterSuccess` — the base-stat table for the creation
/// screen (one entry per offered template).
pub fn new_character_success(
    templates: &[(
        i32,
        crate::enums::Race,
        &crate::data::player_template::PlayerTemplate,
    )],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NEW_CHARACTER_SUCCESS);
    w.write_i32(templates.len() as i32);
    for (class_id, race, t) in templates {
        w.write_i32(race.ordinal());
        w.write_i32(*class_id);
        w.write_i32(99);
        w.write_i32(t.base_str);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_dex);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_con);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_int);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_wit);
        w.write_i32(1);
        w.write_i32(99);
        w.write_i32(t.base_men);
        w.write_i32(1);
    }
    w.into_bytes()
}

/// `serverpackets/CharCreateOk`.
pub fn char_create_ok() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_CREATE_SUCCESS);
    w.write_i32(1);
    w.into_bytes()
}

/// `serverpackets/CharCreateFail` (`CharCreateFail.REASON_*`).
pub fn char_create_fail(reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_CREATE_FAIL);
    w.write_i32(reason);
    w.into_bytes()
}

/// `serverpackets/CharDeleteSuccess`.
pub fn char_delete_success() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_DELETE_SUCCESS);
    w.into_bytes()
}

/// `serverpackets/CharDeleteFail` (`CharacterDeleteFailType`).
pub fn char_delete_fail(reason: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_DELETE_FAIL);
    w.write_i32(reason);
    w.into_bytes()
}

/// Port of `serverpackets/ExIsCharNameCreatable`. `allowed` = -1 when the name
/// may be used; 1..5 is a `RequestCharacterNameCreatable` failure reason.
pub fn ex_is_char_name_creatable(allowed: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_IS_CHAR_NAME_CREATABLE);
    w.write_i32(allowed);
    w.into_bytes()
}

/// Port of `serverpackets/CharSelected` — the reply to `CharacterSelect` that
/// starts the enter-world loading screen. `game_time` is game-minutes of the
/// in-game day (`game_time::game_time_minutes`, G33; the client self-advances
/// from it).
pub fn char_selected(v: &crate::model::PlayerView, session_id: i32, game_time: i32) -> Vec<u8> {
    let crate::model::PlayerView { p, pos, vitals, .. } = v;
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::CHAR_SELECTED);
    w.write_string(&p.name);
    w.write_i32(p.object_id);
    w.write_string(&p.title);
    w.write_i32(session_id);
    w.write_i32(p.clan_id);
    w.write_i32(0);
    w.write_i32(p.is_female as i32);
    w.write_i32(p.race);
    w.write_i32(p.class_id);
    w.write_i32(1); // active
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    w.write_f64(vitals.cur_hp);
    w.write_f64(vitals.cur_mp);
    w.write_i64(p.sp);
    w.write_i64(p.exp);
    w.write_i32(p.level);
    w.write_i32(p.reputation);
    w.write_i32(p.pk_kills);
    w.write_i32(game_time % (24 * 60));
    w.write_i32(0);
    w.write_i32(p.class_id);
    w.write_bytes(&[0u8; 16]);
    for _ in 0..9 {
        w.write_i32(0);
    }
    w.write_bytes(&[0u8; 28]);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/RestartResponse` (`TRUE`/`FALSE` statics): whether
/// the `RequestRestart` was accepted — `true` sends the client back to the
/// character-selection screen (a `CharSelectionInfo` must follow).
pub fn restart_response(result: bool) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::RESTART_RESPONSE);
    w.write_i32(result as i32);
    w.into_bytes()
}

/// Port of `serverpackets/LeaveWorld` (`LOG_OUT_OK`): the "safe to quit"
/// acknowledgement for the client's `Logout`; the server closes the connection
/// right after it.
pub fn leave_world() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::LOG_OUT_OK);
    w.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::lobby_active_id;
    use crate::db::CharData;

    fn chr(last_access: i64, delete_time: i64) -> CharData {
        CharData {
            last_access,
            delete_time,
            ..Default::default()
        }
    }

    #[test]
    fn active_id_is_the_first_most_recently_accessed() {
        let now = commons::util::now_millis();
        // A character marked for deletion is still eligible, as in Java.
        let chars = [chr(100, 0), chr(300, now), chr(300, 0)];
        assert_eq!(lobby_active_id(&chars), 1);
    }

    #[test]
    fn active_id_is_none_when_nobody_has_logged_in() {
        assert_eq!(lobby_active_id(&[chr(0, 0), chr(0, 0)]), -1);
    }

    #[test]
    fn active_id_is_none_for_empty_list() {
        assert_eq!(lobby_active_id(&[]), -1);
    }

    /// The protocol-140 layout: two header bytes after the region flag, five
    /// zero ints between the paperdoll and the visual ids, `-1` in the delete
    /// timer of a banned character, and the hero glow as a bool.
    #[test]
    fn salvation_layout() {
        use commons::network::PacketReader;

        let banned_hero = CharData {
            object_id: 7,
            name: "Hero".into(),
            access_level: -1,
            delete_time: commons::util::now_millis() + 60_000,
            last_access: 5,
            variables: vec![("VITALITY_ITEMS_USED".into(), "3".into())],
            ..Default::default()
        };
        let mut olympiad = crate::model::olympiad::OlympiadState::default();
        olympiad.heroes.push((7, 88));
        olympiad.claimed_heroes.insert(7);
        let exp = crate::data::ExperienceData::from_table(vec![0, 0, 100], 2);
        let ctx = super::CharSelectCtx {
            max_characters: 7,
            exp: &exp,
            olympiad: &olympiad,
            rate_vitality_exp_multiplier: 2.5,
        };
        let pkt = super::char_selection_info("acct", 9, &[banned_hero], -1, &ctx);

        let mut r = PacketReader::new(&pkt[1..]);
        assert_eq!(r.read_i32(), Some(1));
        assert_eq!(r.read_i32(), Some(7));
        assert_eq!(r.read_u8(), Some(0));
        assert_eq!(r.read_u8(), Some(1));
        assert_eq!(r.read_i32(), Some(2));
        assert_eq!(r.read_u8(), Some(0), "gift message");
        assert_eq!(r.read_u8(), Some(0), "Balthus Knights");

        assert_eq!(r.read_string().as_deref(), Some("Hero"));
        r.read_i32(); // object id
        assert_eq!(r.read_string().as_deref(), Some("acct"));
        r.read_bytes(4 * 10 + 8 * 2 + 8 * 3 + 4 * 4 + 4 * 9); // through the reserved ints
        let paperdoll = super::CHAR_SELECT_PAPERDOLL_ORDER.len();
        r.read_bytes(4 * paperdoll);
        assert_eq!(
            r.read_i32_array::<5>(),
            Some([0; 5]),
            "the five Salvation ints"
        );
        r.read_bytes(4 * 9 + 2 * 5 + 4 * 3 + 8 * 2); // visual ids .. max MP
        assert_eq!(
            r.read_i32(),
            Some(-1),
            "a banned character has no delete timer"
        );
        r.read_bytes(4 * 2 + 1 + 4 * 3 + 4 * 4 + 8 * 2 + 4); // class .. vitality
        assert_eq!(
            r.read_i32(),
            Some(200),
            "the cast binds before the multiply"
        );
        assert_eq!(r.read_i32(), Some(3), "vitality items used");
        r.read_i32(); // active
        r.read_u8(); // noble
        assert_eq!(r.read_u8(), Some(1), "hero glow is a bool");
        assert_eq!(r.read_u8(), Some(1), "hair accessory defaults to shown");
        assert_eq!(r.remaining(), 0);
    }
}
