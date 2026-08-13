//! Small ports of `gameserver/enums` needed so far.

use crate::model::inventory::PaperdollSlot;
use enum_ordinalize::Ordinalize;

/// Port of `enums/Race` (ordinals matter — sent on the wire). The six
/// playable races (plus Ertheia, unused by this dist's char-creation but
/// present in the shared Java enum) were the only ones this port needed
/// until G19's `AttackTrait` — Java's `Race` is one enum shared by players
/// *and* the creature-category values every NPC's `<race>` also carries
/// (`Npc.getRace()`/`TraitType`'s `*_WEAKNESS` family checks `target.
/// getRace() == Race.X`) — so the monster-flavor variants join here rather
/// than in a separate type, matching Java's actual design.
///
/// `#[repr(i32)]` fixes the ordinal type the wire and the stored `Player.race`
/// byte use; `Ordinalize` derives both directions from the declaration, so a
/// variant's number lives in exactly one place.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Ordinalize,
)]
#[repr(i32)]
#[ordinalize(ordinal(pub const fn ordinal, doc = "Java `ordinal()` — the wire value and the stored `Player.race` byte."))]
#[ordinalize(from_ordinal(pub const fn from_ordinal, doc = "Inverse of [`Race::ordinal`] — the stored `Player.race` byte back to the enum (`None` on an out-of-range value)."))]
pub enum Race {
    Human = 0,
    Elf = 1,
    DarkElf = 2,
    Orc = 3,
    Dwarf = 4,
    Kamael = 5,
    Ertheia = 6,
    Animal = 7,
    Beast = 8,
    Bug = 9,
    CastleGuard = 10,
    Construct = 11,
    Demonic = 12,
    Divine = 13,
    Dragon = 14,
    Elemental = 15,
    Etc = 16,
    Fairy = 17,
    Giant = 18,
    Humanoid = 19,
    Mercenary = 20,
    None_ = 21,
    Plant = 22,
    SiegeWeapon = 23,
    Undead = 24,
    Friend = 25,
}

impl Race {
    /// The XML/`Race.valueOf` name (`"DARK_ELF"`, …) → enum, as used by the
    /// `<race name=…>` attributes in `respawn.xml` and every NPC template's
    /// `<race>` (`data/npc_data.rs`'s `parse_race`).
    pub fn from_name(name: &str) -> Option<Race> {
        Some(match name {
            "HUMAN" => Race::Human,
            "ELF" => Race::Elf,
            "DARK_ELF" => Race::DarkElf,
            "ORC" => Race::Orc,
            "DWARF" => Race::Dwarf,
            "KAMAEL" => Race::Kamael,
            "ERTHEIA" => Race::Ertheia,
            "ANIMAL" => Race::Animal,
            "BEAST" => Race::Beast,
            "BUG" => Race::Bug,
            "CASTLE_GUARD" => Race::CastleGuard,
            "CONSTRUCT" => Race::Construct,
            "DEMONIC" => Race::Demonic,
            "DIVINE" => Race::Divine,
            "DRAGON" => Race::Dragon,
            "ELEMENTAL" => Race::Elemental,
            "ETC" => Race::Etc,
            "FAIRY" => Race::Fairy,
            "GIANT" => Race::Giant,
            "HUMANOID" => Race::Humanoid,
            "MERCENARY" => Race::Mercenary,
            "NONE" => Race::None_,
            "PLANT" => Race::Plant,
            "SIEGE_WEAPON" => Race::SiegeWeapon,
            "UNDEAD" => Race::Undead,
            "FRIEND" => Race::Friend,
            _ => return None,
        })
    }
}

/// Port of `enums/InventorySlot` — the client's 33 equip-slot components, in
/// wire order (`LRHand` is a display slot backed by the `RHand` paperdoll
/// entry). The declaration order **is** the mask: `getMask()` = ordinal.
///
/// Both `values()` and `getMask()` therefore *are* the declaration, so
/// `Ordinalize` derives them from it rather than restating the order in a
/// parallel array a reordered variant could drift out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ordinalize)]
#[repr(usize)]
#[ordinalize(variants(pub const VALUES, doc = "Java `InventorySlot.values()`."))]
#[ordinalize(ordinal(pub const fn mask, doc = "Java `getMask()` — the component bit for `AbstractMaskPacket` masks."))]
pub enum InventorySlot {
    Under,
    REar,
    LEar,
    Neck,
    RFinger,
    LFinger,
    Head,
    RHand,
    LHand,
    Gloves,
    Chest,
    Legs,
    Feet,
    Cloak,
    LRHand,
    Hair,
    Hair2,
    RBracelet,
    LBracelet,
    Deco1,
    Deco2,
    Deco3,
    Deco4,
    Deco5,
    Deco6,
    Belt,
    Brooch,
    BroochJewel1,
    BroochJewel2,
    BroochJewel3,
    BroochJewel4,
    BroochJewel5,
    BroochJewel6,
}

impl InventorySlot {
    /// Java `getSlot()` — the backing paperdoll slot.
    pub fn slot(self) -> PaperdollSlot {
        match self {
            Self::Under => PaperdollSlot::Under,
            Self::REar => PaperdollSlot::REar,
            Self::LEar => PaperdollSlot::LEar,
            Self::Neck => PaperdollSlot::Neck,
            Self::RFinger => PaperdollSlot::RFinger,
            Self::LFinger => PaperdollSlot::LFinger,
            Self::Head => PaperdollSlot::Head,
            Self::RHand | Self::LRHand => PaperdollSlot::RHand,
            Self::LHand => PaperdollSlot::LHand,
            Self::Gloves => PaperdollSlot::Gloves,
            Self::Chest => PaperdollSlot::Chest,
            Self::Legs => PaperdollSlot::Legs,
            Self::Feet => PaperdollSlot::Feet,
            Self::Cloak => PaperdollSlot::Cloak,
            Self::Hair => PaperdollSlot::Hair,
            Self::Hair2 => PaperdollSlot::Hair2,
            Self::RBracelet => PaperdollSlot::RBracelet,
            Self::LBracelet => PaperdollSlot::LBracelet,
            Self::Deco1 => PaperdollSlot::Deco1,
            Self::Deco2 => PaperdollSlot::Deco2,
            Self::Deco3 => PaperdollSlot::Deco3,
            Self::Deco4 => PaperdollSlot::Deco4,
            Self::Deco5 => PaperdollSlot::Deco5,
            Self::Deco6 => PaperdollSlot::Deco6,
            Self::Belt => PaperdollSlot::Belt,
            Self::Brooch => PaperdollSlot::Brooch,
            Self::BroochJewel1 => PaperdollSlot::BroochJewel1,
            Self::BroochJewel2 => PaperdollSlot::BroochJewel2,
            Self::BroochJewel3 => PaperdollSlot::BroochJewel3,
            Self::BroochJewel4 => PaperdollSlot::BroochJewel4,
            Self::BroochJewel5 => PaperdollSlot::BroochJewel5,
            Self::BroochJewel6 => PaperdollSlot::BroochJewel6,
        }
    }
}

/// Port of `enums/UserInfoType` — the 23 `UserInfo` blocks: component bit
/// (declaration order = `getMask()`) and fixed block length. `BasicInfo` and
/// `Clan` additionally carry `name.len()*2` / `title.len()*2` bytes.
///
/// As with [`InventorySlot`], `values()` and `getMask()` are derived from the
/// declaration — the block order is the mask and must not be restated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ordinalize)]
#[repr(usize)]
#[ordinalize(variants(pub const VALUES, doc = "Java `UserInfoType.values()`."))]
#[ordinalize(ordinal(pub const fn mask, doc = "Java `getMask()` — the component bit for `AbstractMaskPacket` masks."))]
pub enum UserInfoType {
    Relation,
    BasicInfo,
    BaseStats,
    MaxHpCpMp,
    CurrentHpMpCpExpSp,
    EnchantLevel,
    Appearance,
    Status,
    Stats,
    Elementals,
    Position,
    Speed,
    Multiplier,
    ColRadiusHeight,
    AtkElemental,
    Clan,
    Social,
    VitaFame,
    Slots,
    Movements,
    Color,
    InventoryLimit,
    TrueHero,
}

impl UserInfoType {
    /// Java `getBlockLength()` — the fixed byte size of this block (including
    /// its own 2-byte length prefix where the packet writes one).
    pub fn block_length(self) -> i32 {
        match self {
            Self::Relation => 4,
            Self::BasicInfo => 16,
            Self::BaseStats => 18,
            Self::MaxHpCpMp => 14,
            Self::CurrentHpMpCpExpSp => 38,
            Self::EnchantLevel => 4,
            Self::Appearance => 15,
            Self::Status => 6,
            Self::Stats => 56,
            Self::Elementals => 14,
            Self::Position => 18,
            Self::Speed => 18,
            Self::Multiplier => 18,
            Self::ColRadiusHeight => 18,
            Self::AtkElemental => 5,
            Self::Clan => 32,
            Self::Social => 22,
            Self::VitaFame => 15,
            Self::Slots => 9,
            Self::Movements => 4,
            Self::Color => 10,
            Self::InventoryLimit => 9,
            Self::TrueHero => 9,
        }
    }
}

/// Port of `enums/NpcInfoType` — the `NpcInfo` component blocks. Unlike
/// `UserInfoType` the component bits are *not* contiguous (0x0C/0x0D and
/// 0x14/0x15 are unnamed always-on gaps, pre-set in the packet's initial mask
/// bytes), so the discriminants are explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcInfoType {
    Id = 0x00,
    Attackable = 0x01,
    Relations = 0x02,
    Name = 0x03,
    Position = 0x04,
    Heading = 0x05,
    VehicleId = 0x06,
    AtkCastSpeed = 0x07,
    SpeedMultiplier = 0x08,
    Equipped = 0x09,
    StopMode = 0x0A,
    MoveMode = 0x0B,
    SwimOrFly = 0x0E,
    Team = 0x0F,
    Enchant = 0x10,
    Flying = 0x11,
    CloneObj = 0x12,
    PetEvolutionId = 0x13,
    DisplayEffect = 0x16,
    Transformation = 0x17,
    CurrentHp = 0x18,
    CurrentMp = 0x19,
    MaxHp = 0x1A,
    MaxMp = 0x1B,
    Summoned = 0x1C,
    FollowInfo = 0x1D,
    Title = 0x1E,
    NameNpcStringId = 0x1F,
    TitleNpcStringId = 0x20,
    PvpFlag = 0x21,
    Reputation = 0x22,
    Clan = 0x23,
    Abnormals = 0x24,
    VisualState = 0x25,
}

impl NpcInfoType {
    /// Java `getMask()`.
    pub fn mask(self) -> usize {
        self as usize
    }

    /// Java `getBlockLength()` (strings add `chars * 2` on top — see
    /// `NpcInfo.calcBlockSize`).
    pub fn block_length(self) -> i32 {
        match self {
            Self::Id => 4,
            Self::Attackable => 1,
            Self::Relations => 4,
            Self::Name => 2,
            Self::Position => 12,
            Self::Heading => 4,
            Self::VehicleId => 4,
            Self::AtkCastSpeed => 8,
            Self::SpeedMultiplier => 8,
            Self::Equipped => 12,
            Self::StopMode => 1,
            Self::MoveMode => 1,
            Self::SwimOrFly => 1,
            Self::Team => 1,
            Self::Enchant => 4,
            Self::Flying => 4,
            Self::CloneObj => 4,
            Self::PetEvolutionId => 4,
            Self::DisplayEffect => 4,
            Self::Transformation => 4,
            Self::CurrentHp => 4,
            Self::CurrentMp => 4,
            Self::MaxHp => 4,
            Self::MaxMp => 4,
            Self::Summoned => 1,
            Self::FollowInfo => 8,
            Self::Title => 2,
            Self::NameNpcStringId => 4,
            Self::TitleNpcStringId => 4,
            Self::PvpFlag => 1,
            Self::Reputation => 4,
            Self::Clan => 20,
            Self::Abnormals => 0,
            Self::VisualState => 1,
        }
    }
}

/// Port of `enums/ChatType` — the `Say2` channel ids the client sends and
/// `CreatureSay` echoes back. Only the channels the chat slice handles are
/// listed; unknown ids are dropped by the handler (Java disconnects — see the
/// G10 plan's deviations).
///
/// The variants are declared out of numeric order (they are grouped by the
/// slice that introduced them), so the discriminants carry the ids and
/// `Ordinalize` derives the lookup from them — a hand-written inverse would
/// have to be re-sorted by hand every time a channel joins the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ordinalize)]
#[repr(i32)]
#[ordinalize(ordinal(pub const fn client_id, doc = "The `Say2`/`CreatureSay` channel id (Java `ChatType.getClientId()`, which on this chronicle is the ordinal)."))]
#[ordinalize(from_ordinal(pub const fn from_client_id, doc = "Inverse of [`ChatType::client_id`] — `None` for a channel this port doesn't handle."))]
pub enum ChatType {
    General = 0,
    Shout = 1,
    Whisper = 2,
    Party = 3,
    Clan = 4,
    /// Petitioner→GM line during a petition consultation (G31).
    PetitionPlayer = 6,
    /// GM→petitioner line during a petition consultation (G31).
    PetitionGm = 7,
    Trade = 8,
    /// `ChatType.PARTYMATCH_ROOM` — chat inside a party matching room (G30).
    PartyMatchRoom = 14,
    /// `ChatType.PARTYROOM_COMMANDER` — the "Command Channel" chat line only
    /// the CC leader may speak on; every channel member hears it.
    PartyroomCommander = 15,
    /// `ChatType.PARTYROOM_ALL` — the CC chat line any party leader in the
    /// channel may speak on; every channel member hears it.
    PartyroomAll = 16,
    Alliance = 9,
    /// `ChatType.HERO_VOICE` — used server-side for the "Petition System"
    /// broadcast to GMs (G31); also hero global chat.
    HeroVoice = 17,
    /// Server-wide announcements (`Broadcast.toAllOnlinePlayers(String)` →
    /// `CreatureSay(ChatType.ANNOUNCEMENT, ...)`) — server-sent only (G28).
    Announcement = 10,
    /// Ferry announcements (`CreatureSay(ChatType.BOAT, ...)`) — server-sent only.
    Boat = 11,
    /// `ChatType.WORLD` — the server-wide channel handled by
    /// `handlers/chathandlers/ChatWorld.java`: a minimum level, a daily point
    /// quota and a per-speaker reuse window, then a broadcast to every online
    /// player. Enabled on this dist (`WorldChatEnabled = True`).
    ///
    /// **Chronicle caveat:** the client half is Grand-Crusade-era —
    /// `ExWorldChatCnt` is ex-opcode `0x175`, and the Interlude client's chat
    /// selector has no World entry — so a stock Interlude client is not
    /// expected to reach this. Ported because the dist ships it enabled and
    /// the server half is chronicle-agnostic; the reachable consumer today is
    /// `BanChatChannels`, which names `WORLD` and could not parse without it.
    World = 25,
}

/// Port of `enums/AdminTeleportType` — the GM "Additional Movement Options"
/// click-to-move modes (`html/admin/move.htm`). The mode is a *latch* on the
/// GM: once armed, the next `MoveBackwardToLocation` click is consumed by the
/// matching branch instead of starting an ordinary walk. Never persisted
/// (Java: a plain `Player` field defaulting to `NORMAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdminTeleportType {
    /// Ordinary movement — the click starts a walk.
    #[default]
    Normal,
    /// `//instant_move` ("Demonic mode") — the click teleports the GM there.
    /// One-shot: the latch falls back to [`Normal`](Self::Normal) after it fires.
    Demonic,
    /// `//teleto sayune` ("Sayune mode") — the click slides the GM there
    /// without a loading screen. One-shot, like `Demonic`.
    Sayune,
    /// `//teleto charge` ("Charge mode") — the click slides the GM there with
    /// the charge animation. **Sticky**: Java never resets this one, so every
    /// subsequent click charges until `//teleto end`.
    Charge,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ChatType`'s discriminants are the `Say2`/`CreatureSay` channel ids and
    /// the variants are *not* declared in numeric order, so the ids are pinned
    /// against Java here rather than left to be read off the declaration.
    #[test]
    fn chat_type_ids_match_java_and_round_trip() {
        let expected = [
            (ChatType::General, 0),
            (ChatType::Shout, 1),
            (ChatType::Whisper, 2),
            (ChatType::Party, 3),
            (ChatType::Clan, 4),
            (ChatType::PetitionPlayer, 6),
            (ChatType::PetitionGm, 7),
            (ChatType::Trade, 8),
            (ChatType::Alliance, 9),
            (ChatType::Announcement, 10),
            (ChatType::Boat, 11),
            (ChatType::PartyMatchRoom, 14),
            (ChatType::PartyroomCommander, 15),
            (ChatType::PartyroomAll, 16),
            (ChatType::HeroVoice, 17),
            (ChatType::World, 25),
        ];
        for (channel, id) in expected {
            assert_eq!(channel.client_id(), id, "{channel:?}");
            assert_eq!(ChatType::from_client_id(id), Some(channel));
        }
        // The gaps Java fills with channels this port drops, plus either end.
        for id in [i32::MIN, -1, 5, 12, 13, 18, 24, 26, i32::MAX] {
            assert_eq!(ChatType::from_client_id(id), None, "{id}");
        }
    }

    /// For both mask enums the declaration order *is* the component bit, so
    /// `VALUES` must stay index-aligned with `mask()` and keep Java's length —
    /// a reordered or inserted variant shifts every later block's bit.
    #[test]
    fn mask_enums_are_index_aligned() {
        assert_eq!(InventorySlot::VALUES.len(), 33);
        for (i, slot) in InventorySlot::VALUES.iter().enumerate() {
            assert_eq!(slot.mask(), i, "{slot:?}");
        }
        assert_eq!(InventorySlot::Under.mask(), 0);
        assert_eq!(InventorySlot::LRHand.mask(), 14);
        assert_eq!(InventorySlot::BroochJewel6.mask(), 32);

        assert_eq!(UserInfoType::VALUES.len(), 23);
        for (i, block) in UserInfoType::VALUES.iter().enumerate() {
            assert_eq!(block.mask(), i, "{block:?}");
        }
        assert_eq!(UserInfoType::Relation.mask(), 0);
        assert_eq!(UserInfoType::TrueHero.mask(), 22);
    }

    /// `Race`'s ordinal is the wire value and the stored `Player.race` byte;
    /// the monster-flavor variants share the enum, so the playable six must
    /// keep the low slots (see the type's doc comment).
    #[test]
    fn race_ordinals_match_java() {
        let playable = [
            (Race::Human, 0),
            (Race::Elf, 1),
            (Race::DarkElf, 2),
            (Race::Orc, 3),
            (Race::Dwarf, 4),
            (Race::Kamael, 5),
            (Race::Ertheia, 6),
        ];
        for (race, ordinal) in playable {
            assert_eq!(race.ordinal(), ordinal, "{race:?}");
            assert_eq!(Race::from_ordinal(ordinal), Some(race));
        }
        assert_eq!(Race::Friend.ordinal(), 25);
        assert_eq!(Race::from_ordinal(26), None);
        assert_eq!(Race::from_ordinal(-1), None);
    }
}
