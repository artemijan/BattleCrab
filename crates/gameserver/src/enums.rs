//! Small ports of `gameserver/enums` needed so far.

use crate::model::inventory::PaperdollSlot;

/// Port of `enums/Race` (ordinals matter — sent on the wire). The six
/// playable races (plus Ertheia, unused by this dist's char-creation but
/// present in the shared Java enum) were the only ones this port needed
/// until G19's `AttackTrait` — Java's `Race` is one enum shared by players
/// *and* the creature-category values every NPC's `<race>` also carries
/// (`Npc.getRace()`/`TraitType`'s `*_WEAKNESS` family checks `target.
/// getRace() == Race.X`) — so the monster-flavor variants join here rather
/// than in a separate type, matching Java's actual design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
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
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// Inverse of [`Race::ordinal`] — the stored `Player.race` byte back to the
    /// enum (`None` on an out-of-range value).
    pub fn from_ordinal(o: i32) -> Option<Race> {
        Some(match o {
            0 => Race::Human,
            1 => Race::Elf,
            2 => Race::DarkElf,
            3 => Race::Orc,
            4 => Race::Dwarf,
            5 => Race::Kamael,
            6 => Race::Ertheia,
            7 => Race::Animal,
            8 => Race::Beast,
            9 => Race::Bug,
            10 => Race::CastleGuard,
            11 => Race::Construct,
            12 => Race::Demonic,
            13 => Race::Divine,
            14 => Race::Dragon,
            15 => Race::Elemental,
            16 => Race::Etc,
            17 => Race::Fairy,
            18 => Race::Giant,
            19 => Race::Humanoid,
            20 => Race::Mercenary,
            21 => Race::None_,
            22 => Race::Plant,
            23 => Race::SiegeWeapon,
            24 => Race::Undead,
            25 => Race::Friend,
            _ => return None,
        })
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
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
            6 => Some(Self::PetitionPlayer),
            7 => Some(Self::PetitionGm),
            8 => Some(Self::Trade),
            9 => Some(Self::Alliance),
            10 => Some(Self::Announcement),
            11 => Some(Self::Boat),
            14 => Some(Self::PartyMatchRoom),
            15 => Some(Self::PartyroomCommander),
            16 => Some(Self::PartyroomAll),
            17 => Some(Self::HeroVoice),
            _ => None,
        }
    }
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
