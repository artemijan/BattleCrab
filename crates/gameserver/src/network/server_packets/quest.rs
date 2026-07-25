//! Quest packets and quest sounds.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/PlaySound`'s quest-sound shape (`new
/// PlaySound(soundFile)`): every non-string field 0.
pub fn play_sound(sound_file: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLAY_SOUND);
    w.write_i32(0); // 0 for quest sounds
    w.write_string(sound_file);
    w.write_i32(0); // 1 for ship sounds
    w.write_i32(0); // ship object id
    w.write_i32(0); // x
    w.write_i32(0); // y
    w.write_i32(0); // z
    w.write_i32(0);
    w.into_bytes()
}

/// The `QuestSound` file names this slice plays.
pub mod quest_sounds {
    pub const ACCEPT: &str = "ItemSound.quest_accept";
    pub const MIDDLE: &str = "ItemSound.quest_middle";
    pub const FINISH: &str = "ItemSound.quest_finish";
    pub const ITEMGET: &str = "ItemSound.quest_itemget";
    pub const JACKPOT: &str = "ItemSound.quest_jackpot";
    /// `QuestSound.ITEMSOUND_QUEST_BEFORE_BATTLE` — played when a quest
    /// conjures something hostile (quest 416's Durka Spirit).
    pub const BEFORE_BATTLE: &str = "ItemSound.quest_before_battle";
    /// `QuestSound.ETCSOUND_ELROKI_SONG_FULL` — the Elroki flute cue (quest
    /// 111). The client sound name has the "elcroki" spelling; keep it.
    pub const ELROKI_SONG_FULL: &str = "EtcSound.elcroki_song_full";
}

/// Port of `serverpackets/ExShowQuestMark` — the on-screen quest marker,
/// sent after every cond change (Java `QuestState.setCond`).
pub fn ex_show_quest_mark(quest_id: i32, quest_state: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_QUEST_MARK);
    w.write_i32(quest_id);
    w.write_i32(quest_state);
    w.into_bytes()
}

/// Port of `serverpackets/NpcQuestHtmlMessage` — the quest-window variant of
/// `NpcHtmlMessage`, used for `.htm` results of quests with `0 < id < 20000`
/// (`Quest.showHtmlFile`'s `questwindow` branch).
pub fn ex_npc_quest_html_message(npc_object_id: i32, html: &str, quest_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_NPC_QUEST_HTML_MESSAGE);
    w.write_i32(npc_object_id);
    w.write_string(super::npc::clip_html(html));
    w.write_i32(quest_id);
    w.into_bytes()
}
