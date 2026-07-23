//! Skill-cast packets: magic-skill use/launch/cancel, the cast gauge, and the
//! reuse cooldown list.

use commons::network::PacketWriter;

use super::opcodes;
use crate::model::Player;

/// Port of `serverpackets/MagicSkillUse` (no ground-targeted skills, no
/// `RequestActionUse` action id yet). `casting_bar_id` is
/// `SkillCastingType.NORMAL`'s client bar id (0). `hit_time` is the
/// client-displayed cast time (`_hitTime + _cancelTime`). Self-casts pass the
/// caster as `target`. `reuse_group` is the skill's `reuseDelayGroup` — -1
/// when ungrouped (the client greys *every* icon on 0, Java's constructor
/// default is -1).
pub fn magic_skill_use(
    caster: &Player,
    caster_pos: &crate::model::components::Position,
    target: (i32, i32, i32, i32), // (object_id, x, y, z) — player or NPC
    skill_id: i32,
    skill_level: i32,
    hit_time: i32,
    reuse_group: i32,
    reuse_delay: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_USE);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster.object_id);
    w.write_i32(target.0);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(hit_time);
    w.write_i32(reuse_group);
    w.write_i32(reuse_delay);
    w.write_i32(caster_pos.x);
    w.write_i32(caster_pos.y);
    w.write_i32(caster_pos.z);
    w.write_i16(0); // isGroundTargetSkill
    w.write_i16(0); // no ground location
    w.write_i32(target.1);
    w.write_i32(target.2);
    w.write_i32(target.3);
    w.write_i32(0); // actionId used
    w.write_i32(0); // actionId
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillLaunched`: the launch flourish, broadcast
/// with the resolved target list (`SkillCaster._targets`).
pub fn magic_skill_launched(
    caster_object_id: i32,
    skill_id: i32,
    skill_level: i32,
    targets: &[i32],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_LAUNCHED);
    w.write_i32(0); // casting bar: NORMAL
    w.write_i32(caster_object_id);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i32(targets.len() as i32);
    for &t in targets {
        w.write_i32(t);
    }
    w.into_bytes()
}

/// Port of `serverpackets/MagicSkillCanceled` — stops the cast animation
/// client-side. Broadcast (self included) by `stopCasting(aborted == true)`;
/// never sent on a quiet stop (finish-phase failures, natural end).
pub fn magic_skill_canceld(object_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::MAGIC_SKILL_CANCELED);
    w.write_i32(object_id);
    w.into_bytes()
}

/// Port of `serverpackets/SetupGauge` (the cast-bar packet). `color`: 0 = blue.
pub fn setup_gauge(object_id: i32, color: i32, time_ms: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SETUP_GAUGE);
    w.write_i32(object_id);
    w.write_i32(color);
    w.write_i32(time_ms);
    w.write_i32(time_ms);
    w.into_bytes()
}

/// Port of `serverpackets/SkillCoolTime`: every skill still on reuse
/// (`Player.reuses` entries with time remaining), total and remaining in
/// whole seconds. The id written is the map key — the shared reuse group when
/// the skill has one, else the skill id (Java writes
/// `sharedReuseGroup > 0 ? group : skillId`). Sent on enter-world and on
/// `RequestSkillCoolTime`.
pub fn skill_cool_time(reuses: &crate::model::components::Reuses, now_tick: u64) -> Vec<u8> {
    let entries: Vec<(i32, i32, i32, i32)> = reuses
        .0
        .iter()
        .filter_map(|(&reuse_key, r)| {
            let remaining_ticks = r.until_tick.checked_sub(now_tick)?;
            if remaining_ticks == 0 {
                return None;
            }
            Some((
                reuse_key,
                r.skill_level,
                r.total_ms / 1000,
                (remaining_ticks / 10) as i32,
            ))
        })
        .collect();
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SKILL_COOL_TIME);
    w.write_i32(entries.len() as i32);
    for (skill_id, level, total_secs, remaining_secs) in entries {
        w.write_i32(skill_id);
        w.write_i32(level);
        w.write_i32(total_secs);
        w.write_i32(remaining_secs);
    }
    w.into_bytes()
}

/// Port of `serverpackets/AcquireSkillDone` — no body.
pub fn acquire_skill_done() -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACQUIRE_SKILL_DONE);
    w.into_bytes()
}

/// Port of `serverpackets/ExAcquirableSkillListByClass` — the learnable-skill
/// window for the non-class trees; G18 uses it for the village master's
/// pledge-skill list (`AcquireSkillType.PLEDGE` = 2). Entries are
/// `(skill_id, skill_level, get_level, level_up_sp)`; the required-items
/// count is always 0 (no pledge entry on this dist carries items).
pub fn ex_acquirable_skill_list_by_class(type_id: i16, skills: &[(i32, i32, i32, i64)]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::EX);
    w.write_i16(opcodes::EX_ACQUIRABLE_SKILL_LIST_BY_CLASS);
    w.write_i16(type_id);
    w.write_i16(skills.len() as i16);
    for &(id, level, get_level, sp) in skills {
        w.write_i32(id);
        w.write_i16(level as i16);
        w.write_i16(level as i16);
        w.write_u8(get_level as u8);
        w.write_i64(sp);
        w.write_u8(0); // required items
    }
    w.into_bytes()
}

/// Port of `serverpackets/AcquireSkillInfo` — the cost detail the client asks
/// for (`RequestAcquireSkillInfo`) before confirming a learn. For pledge
/// skills `sp_cost` is the clan-reputation price; no required items on this
/// dist's pledge tree.
pub fn acquire_skill_info(skill_id: i32, skill_level: i32, sp_cost: i64, type_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::ACQUIRE_SKILL_INFO);
    w.write_i32(skill_id);
    w.write_i32(skill_level);
    w.write_i64(sp_cost);
    w.write_i32(type_id);
    w.write_i32(0); // requirements
    w.into_bytes()
}
