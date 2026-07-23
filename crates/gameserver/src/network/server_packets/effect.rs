//! Environment / GM-effect broadcast packets: the atmosphere and screen-shake
//! effects the `AdminEffects` handler fires. Ported 1:1 from
//! `serverpackets/{Earthquake,SunRise,SunSet,ExRedSky,PlaySound}`.

use commons::network::PacketWriter;

use super::opcodes;

/// Port of `serverpackets/Earthquake` — a localised screen-shake centred on
/// `(x, y, z)`. Broadcast to the caster's surrounding regions.
pub fn earthquake(x: i32, y: i32, z: i32, intensity: i32, duration: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EARTHQUAKE);
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(intensity);
    w.write_i32(duration);
    w.write_i32(0); // Unknown
    w.into_bytes()
}

/// Port of `serverpackets/SunRise` (day) — a bodyless static packet.
pub fn sun_rise() -> Vec<u8> {
    vec![opcodes::SUN_RISE]
}

/// Port of `serverpackets/SunSet` (night) — a bodyless static packet.
pub fn sun_set() -> Vec<u8> {
    vec![opcodes::SUN_SET]
}

/// Port of `serverpackets/ExRedSky` — the blood-red sky, `duration` seconds.
pub fn ex_red_sky(duration: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_RED_SKY);
    w.write_i32(duration);
    w.into_bytes()
}

/// `ExShowScreenMessage(String text, int position, int time)` — an on-screen
/// banner of free text for `time` ms at `position` (`TOP_CENTER = 2`).
///
/// The `MULTILANG_ENABLE` localisation branch is skipped (off by default), so
/// this is the plain `writeImpl`: the fixed field block then, since
/// `npcString == -1`, the raw text. `type = 2`, `sysMessageId = -1`, `size`,
/// `effect`, `fade`, and the three `unk`s are the `(text, time)` constructor's
/// defaults (all 0/false). The NpcString / parameterised variants are a later
/// addition when a caller needs them.
pub fn ex_show_screen_message(text: &str, position: i32, time: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_SHOW_SCREEN_MESSAGE);
    w.write_i32(2); // type
    w.write_i32(-1); // sysMessageId
    w.write_i32(position);
    w.write_i32(0); // unk1
    w.write_i32(0); // size
    w.write_i32(0); // unk2
    w.write_i32(0); // unk3
    w.write_i32(0); // effect (false)
    w.write_i32(time);
    w.write_i32(0); // fade (false)
    w.write_i32(-1); // npcString
    w.write_string(text);
    w.into_bytes()
}

// `PlaySound` for the GM `//play_sound` reuses the quest-sound builder
// (`server_packets::play_sound`) — identical wire form.

/// A raw `MagicSkillUse` for the `//effect` / `//npc_use_skill` admin command,
/// where the animation source (`caster`) may be an NPC — so the
/// [`super::magic_skill_use`] builder, which takes a `&Player`, does not apply.
/// Java calls `new MagicSkillUse(target, activeChar, skill, level, hittime, 0)`:
/// the GM's *target* creature plays the animation and the *GM* is the packet's
/// target. `reuse_group = -1`, `reuse_delay = 0`, `actionId = -1` are the
/// 6-arg constructor's defaults (no reuse-icon greying, no action id).
pub fn magic_skill_use_raw(
    caster: (i32, i32, i32, i32), // animation source: (object_id, x, y, z)
    target: (i32, i32, i32, i32), // packet target: (object_id, x, y, z)
    skill_id: i32,
    skill_level: i32,
    hit_time: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_USE);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster.0);
    w.write_i32(target.0);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(hit_time);
    w.write_i32(-1); // reuse group (ungrouped)
    w.write_i32(0); // reuse delay
    w.write_i32(caster.1);
    w.write_i32(caster.2);
    w.write_i32(caster.3);
    w.write_i16(0); // isGroundTargetSkill
    w.write_i16(0); // no ground location
    w.write_i32(target.1);
    w.write_i32(target.2);
    w.write_i32(target.3);
    w.write_i32(0); // actionId used (actionId < 0 → 0)
    w.write_i32(0); // actionId
    w.into_bytes()
}
