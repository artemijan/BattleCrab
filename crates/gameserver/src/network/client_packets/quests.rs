//! Quest and tutorial packets.

use commons::network::PacketReader;

/// Port of `clientpackets/RequestTutorialLinkHtml` (`dS`): a `link` click in
/// the tutorial window — a discarded int, then the bypass string.
pub struct RequestTutorialLinkHtml {
    pub bypass: String,
}

impl RequestTutorialLinkHtml {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let _unused = r.read_i32()?;
        Some(Self {
            bypass: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestTutorialPassCmdToServer` (`S`): a `bypass`
/// press in the tutorial window (no leading int, unlike the link packet).
pub struct RequestTutorialPassCmd {
    pub bypass: String,
}

impl RequestTutorialPassCmd {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            bypass: r.read_string()?,
        })
    }
}

/// Port of `clientpackets/RequestTutorialQuestionMark` (`cd`): the leading
/// byte mirrors the mark-type byte 0xA7 writes; only the mark id matters.
pub struct RequestTutorialQuestionMark {
    pub number: i32,
}

impl RequestTutorialQuestionMark {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        let _mark_type = r.read_u8()?;
        Some(Self {
            number: r.read_i32()?,
        })
    }
}

/// Port of `clientpackets/RequestQuestAbort` — the quest UI's Abandon button.
pub struct RequestQuestAbort {
    pub quest_id: i32,
}

impl RequestQuestAbort {
    pub fn read(body_after_opcode: &[u8]) -> Option<Self> {
        let mut r = PacketReader::new(body_after_opcode);
        Some(Self {
            quest_id: r.read_i32()?,
        })
    }
}
