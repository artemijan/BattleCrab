//! Small ports of `gameserver/enums` needed so far.

use crate::model::inventory::PaperdollSlot;

/// Port of `enums/Race` (ordinals matter — sent on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Race {
    Human = 0,
    Elf = 1,
    DarkElf = 2,
    Orc = 3,
    Dwarf = 4,
    Kamael = 5,
    Ertheia = 6,
}

impl Race {
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

/// Port of `enums/InventorySlot` — the client's 33 equip-slot components, in
/// wire order (`LRHand` is a display slot backed by the `RHand` paperdoll
/// entry). The declaration order **is** the mask: `getMask()` = ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Java `InventorySlot.values()`.
    pub const VALUES: [InventorySlot; 33] = [
        Self::Under,
        Self::REar,
        Self::LEar,
        Self::Neck,
        Self::RFinger,
        Self::LFinger,
        Self::Head,
        Self::RHand,
        Self::LHand,
        Self::Gloves,
        Self::Chest,
        Self::Legs,
        Self::Feet,
        Self::Cloak,
        Self::LRHand,
        Self::Hair,
        Self::Hair2,
        Self::RBracelet,
        Self::LBracelet,
        Self::Deco1,
        Self::Deco2,
        Self::Deco3,
        Self::Deco4,
        Self::Deco5,
        Self::Deco6,
        Self::Belt,
        Self::Brooch,
        Self::BroochJewel1,
        Self::BroochJewel2,
        Self::BroochJewel3,
        Self::BroochJewel4,
        Self::BroochJewel5,
        Self::BroochJewel6,
    ];

    /// Java `getMask()` — the component bit for `AbstractMaskPacket` masks.
    pub fn mask(self) -> usize {
        self as usize
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Java `UserInfoType.values()`.
    pub const VALUES: [UserInfoType; 23] = [
        Self::Relation,
        Self::BasicInfo,
        Self::BaseStats,
        Self::MaxHpCpMp,
        Self::CurrentHpMpCpExpSp,
        Self::EnchantLevel,
        Self::Appearance,
        Self::Status,
        Self::Stats,
        Self::Elementals,
        Self::Position,
        Self::Speed,
        Self::Multiplier,
        Self::ColRadiusHeight,
        Self::AtkElemental,
        Self::Clan,
        Self::Social,
        Self::VitaFame,
        Self::Slots,
        Self::Movements,
        Self::Color,
        Self::InventoryLimit,
        Self::TrueHero,
    ];

    /// Java `getMask()` — the component bit for `AbstractMaskPacket` masks.
    pub fn mask(self) -> usize {
        self as usize
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ChatType {
    General = 0,
    Shout = 1,
    Whisper = 2,
    Party = 3,
    Clan = 4,
    Trade = 8,
    Alliance = 9,
}

impl ChatType {
    pub fn client_id(self) -> i32 {
        self as i32
    }

    pub fn from_client_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::General),
            1 => Some(Self::Shout),
            2 => Some(Self::Whisper),
            3 => Some(Self::Party),
            4 => Some(Self::Clan),
            8 => Some(Self::Trade),
            9 => Some(Self::Alliance),
            _ => None,
        }
    }
}
