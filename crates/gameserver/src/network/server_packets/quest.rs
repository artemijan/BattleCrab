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

/// Port of the positioned `PlaySound` shape (`new PlaySound(1, soundFile, 1,
/// objectId, x, y, z)`) — the grand-boss roars are anchored to the boss so the
/// client attenuates them by distance instead of playing them flat.
pub fn play_sound_at(sound_file: &str, object_id: i32, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLAY_SOUND);
    w.write_i32(1);
    w.write_string(sound_file);
    w.write_i32(1);
    w.write_i32(object_id);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
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

/// Port of the tutorial-voice `PlaySound` shape (`new PlaySound(2, voice, 0,
/// 0, x, y, z)` — Java `AbstractScript.playTutorialVoice` anchors the voice
/// line at the player's own position).
pub fn play_tutorial_voice(voice: &str, x: i32, y: i32, z: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::PLAY_SOUND);
    w.write_i32(2);
    w.write_string(voice);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(0);
    w.into_bytes()
}

/// Port of `serverpackets/TutorialShowHtml` (0xA6): the tutorial window.
/// `type` is 1 (NORMAL_WINDOW) — the only shape Q255 uses.
pub fn tutorial_show_html(html: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TUTORIAL_SHOW_HTML);
    w.write_i32(1); // NORMAL_WINDOW
    w.write_string(super::npc::clip_html(html));
    w.into_bytes()
}

/// Port of `serverpackets/TutorialShowQuestionMark` (0xA7). This build writes
/// a leading mark-type byte before the mark id (a later-chronicle field the
/// Mobius build keeps; the paired client packet 0x87 reads the same layout) —
/// Q255 always sends type 0.
pub fn tutorial_show_question_mark(mark_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TUTORIAL_SHOW_QUESTION_MARK);
    w.write_u8(0); // mark type
    w.write_i32(mark_id);
    w.into_bytes()
}

/// Port of `serverpackets/TutorialCloseHtml` (0xA9). No body.
pub fn tutorial_close_html() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::TUTORIAL_CLOSE_HTML);
    w.into_bytes()
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
