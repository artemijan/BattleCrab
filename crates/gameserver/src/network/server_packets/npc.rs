//! NPC packets: `NpcInfo`, `NpcHtmlMessage`, and the `NpcSay` chat bubble.

use commons::network::PacketWriter;

use super::opcodes;
use crate::data::npc_data::NpcTemplate;
use crate::enums::NpcInfoType;
use crate::network::masks;

/// Port of `serverpackets/NpcSay`'s npc-string shape (`new NpcSay(npc,
/// NPC_GENERAL, npcStringId)`): chat bubble over an NPC with a client-side
/// localized string (no parameters).
pub fn npc_say(npc_object_id: i32, npc_id: i32, npc_string_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_SAY);
    w.write_i32(npc_object_id);
    w.write_i32(22); // ChatType.NPC_GENERAL client id
    w.write_i32(1_000_000 + npc_id);
    w.write_i32(npc_string_id);
    w.into_bytes()
}

/// The npc-string shape **with one `$s1` parameter** (Java `NpcSay` +
/// `addStringParameter`) — "$s1! How dare you interrupt our fight!" with the
/// attacker's name filled in client-side.
pub fn npc_say_param(npc_object_id: i32, npc_id: i32, npc_string_id: i32, param: &str) -> Vec<u8> {
    npc_say_param_typed(npc_object_id, npc_id, 22, npc_string_id, param)
}

/// [`npc_say_param`] with an explicit `ChatType` client id — the castle mass
/// gatekeeper shouts (`NPC_SHOUT`, 23) rather than talking.
pub fn npc_say_param_typed(
    npc_object_id: i32,
    npc_id: i32,
    chat_type: i32,
    npc_string_id: i32,
    param: &str,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_SAY);
    w.write_i32(npc_object_id);
    w.write_i32(chat_type);
    w.write_i32(1_000_000 + npc_id);
    w.write_i32(npc_string_id);
    w.write_string(param);
    w.into_bytes()
}

/// `NpcSay(npc, NPC_GENERAL, String)` — the **literal-text** variant: Java
/// writes `_npcString = -1` then the string, which is what
/// `broadcastSay(type, "some English line")` sends (Dr. Chaos's paranoid
/// barks are literal, not client-localized string ids).
pub fn npc_say_text(npc_object_id: i32, npc_id: i32, text: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_SAY);
    w.write_i32(npc_object_id);
    w.write_i32(22); // ChatType.NPC_GENERAL
    w.write_i32(1_000_000 + npc_id);
    w.write_i32(-1); // literal string follows
    w.write_string(text);
    w.into_bytes()
}

/// Port of `Creature.getTitle()`'s monster branch: with `ShowNpcLevel` /
/// `ShowNpcAggression` on, a monster's title becomes `Lv <level>` plus `[A]`
/// (template aggressive) / `[G]` (has a clan list and a clan-help range, i.e.
/// calls faction help), with the template title appended after. The champion
/// and trap branches are skipped — neither is modeled yet.
pub fn npc_title(t: &NpcTemplate, cfg: &crate::config::NpcConfig) -> String {
    if !t.is_monster() || (!cfg.show_npc_level && !cfg.show_npc_aggression) {
        return t.title.clone();
    }
    let mut title = String::new();
    if cfg.show_npc_level {
        title.push_str("Lv ");
        title.push_str(&t.level.to_string());
    }
    if cfg.show_npc_aggression {
        // Java appends the separator space before checking the flags, so a
        // calm loner still gets "Lv 20 " — kept for byte parity.
        if !title.is_empty() {
            title.push(' ');
        }
        if t.is_aggressive {
            title.push_str("[A]");
        }
        if !t.clans.is_empty() && t.clan_help_range > 0 {
            title.push_str("[G]");
        }
    }
    if !t.title.is_empty() {
        title.push(' ');
        title.push_str(&t.title);
    }
    title
}

/// Port of `serverpackets/NpcInfo` (masked, 5 mask bytes / "mask_bits_37").
/// Component selection follows the Java constructor with the not-yet-modeled
/// state at its defaults: no summon animation, no water/fly/clone/transform,
/// no clan, reputation 0, pvp flag 0. The localisation
/// pass (`MULTILANG_ENABLE`) is skipped.
/// `abnormal_visuals` are the mob's live `AbnormalVisualEffect` client ids
/// (`abnormal::visual_effects`). Java adds the `ABNORMALS` component whenever
/// that list is non-empty, and the block is a count plus one short each — the
/// same shape `CharInfo` needed before a stunned *player* was visible.
pub fn npc_info(
    v: &crate::model::npc::NpcView,
    t: &NpcTemplate,
    cfg: &crate::config::NpcConfig,
    abnormal_visuals: &[i16],
) -> Vec<u8> {
    let crate::model::npc::NpcView {
        npc,
        pos,
        vitals,
        speeds,
    } = v;
    use NpcInfoType as T;

    // A per-instance `setTitle` (an EffectPoint seal naming its caster) wins
    // over the template + level/aggression decoration.
    let title = match &npc.title_override {
        Some(custom) => custom.clone(),
        None => npc_title(t, cfg),
    };

    // Java `NpcInfo._masks` starts with the two unnamed always-on component
    // pairs (0x0C/0x0D and 0x14/0x15) pre-set.
    let mut mask_bytes: [u8; 5] = [0x00, 0x0C, 0x0C, 0x00, 0x00];
    let mut init_size: i32 = 0;
    let mut block_size: i32 = 0;
    let mut add = |mask_bytes: &mut [u8; 5], ty: T| {
        masks::add_mask(mask_bytes, ty.mask());
        // `calcBlockSize`: ATTACKABLE/RELATIONS/TITLE go in block 1, the rest
        // in block 2; the string components add their chars on top.
        match ty {
            T::Attackable | T::Relations => init_size += ty.block_length(),
            T::Title => init_size += ty.block_length() + title.len() as i32 * 2,
            T::Name => block_size += ty.block_length() + t.name.len() as i32 * 2,
            _ => block_size += ty.block_length(),
        }
    };

    add(&mut mask_bytes, T::Attackable);
    add(&mut mask_bytes, T::Relations);
    add(&mut mask_bytes, T::Id);
    add(&mut mask_bytes, T::Position);
    add(&mut mask_bytes, T::StopMode);
    add(&mut mask_bytes, T::MoveMode);
    if pos.heading > 0 {
        add(&mut mask_bytes, T::Heading);
    }
    if t.base_p_atk_spd > 0 || t.base_m_atk_spd > 0 {
        add(&mut mask_bytes, T::AtkCastSpeed);
    }
    if t.base_run_spd > 0.0 {
        add(&mut mask_bytes, T::SpeedMultiplier);
    }
    if t.rhand > 0 || t.lhand > 0 {
        add(&mut mask_bytes, T::Equipped);
    }
    if vitals.max_hp > 0 {
        add(&mut mask_bytes, T::MaxHp);
    }
    if vitals.max_mp > 0 {
        add(&mut mask_bytes, T::MaxMp);
    }
    if vitals.cur_hp <= vitals.max_hp as f64 {
        add(&mut mask_bytes, T::CurrentHp);
    }
    if vitals.cur_mp <= vitals.max_mp as f64 {
        add(&mut mask_bytes, T::CurrentMp);
    }
    if t.server_side_name {
        add(&mut mask_bytes, T::Name);
    }
    if t.server_side_title || (t.is_monster() && (cfg.show_npc_level || cfg.show_npc_aggression)) {
        add(&mut mask_bytes, T::Title);
    }
    // Java `if (npc.getEnchantEffect() > 0)` — the weapon glow. Rolled per
    // instance at spawn (`EnableRandomEnchantEffect`), so it rides along on
    // every `NpcInfo` for this NPC's whole life.
    if npc.enchant_effect > 0 {
        add(&mut mask_bytes, T::Enchant);
    }
    // Java `if (npc.getTeam() != Team.NONE)` / `if (npc.getDisplayEffect() > 0)`
    // — the blue/red aura and the model's visual state. Both are *stored* on the
    // NPC, so an observer who arrives later still sees them; broadcasting only
    // the change packet would leave them out of sync.
    if npc.team != 0 {
        add(&mut mask_bytes, T::Team);
    }
    if npc.display_effect > 0 {
        add(&mut mask_bytes, T::DisplayEffect);
    }
    add(&mut mask_bytes, T::PetEvolutionId);
    // Status mask: 0x01 in combat, 0x02 dead, 0x04 targetable, 0x08 show name.
    let mut status_mask = 0u8;
    if t.targetable {
        status_mask |= 0x04;
    }
    if t.show_name {
        status_mask |= 0x08;
    }
    // Java: `if (!_abnormalVisualEffects.isEmpty() || npc.isInvisible())`.
    // Declared before VISUAL_STATE because that is the order the blocks are
    // written in below, and the client reads them positionally.
    if !abnormal_visuals.is_empty() {
        add(&mut mask_bytes, T::Abnormals);
    }
    if status_mask != 0 {
        add(&mut mask_bytes, T::VisualState);
    }

    let contains = |ty: T| masks::contains_mask(&mask_bytes, ty.mask());

    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_INFO);
    w.write_i32(npc.object_id);
    w.write_u8(0); // 0=teleported 1=default 2=summoned
    w.write_i16(37); // mask_bits_37
    w.write_bytes(&mask_bytes);

    // Block 1.
    w.write_u8(init_size as u8);
    w.write_u8(u8::from(t.is_attackable_class() && t.type_name != "Guard"));
    w.write_i32(0); // relations
    if contains(T::Title) {
        w.write_string(&title);
    }

    // Block 2.
    w.write_i16(block_size as i16);
    w.write_i32(t.display_id + 1_000_000);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    if contains(T::Heading) {
        w.write_i32(pos.heading);
    }
    if contains(T::AtkCastSpeed) {
        w.write_i32(t.base_p_atk_spd);
        w.write_i32(t.base_m_atk_spd);
    }
    if contains(T::SpeedMultiplier) {
        // Current speed / template base speed — 1.0 until buffs/AI exist.
        w.write_f32(1.0); // movement speed multiplier
        w.write_f32(1.0); // attack speed multiplier
    }
    if contains(T::Equipped) {
        w.write_i32(t.rhand);
        w.write_i32(0); // armor id (Java writes 0)
        w.write_i32(t.lhand);
    }
    w.write_u8(1); // STOP_MODE: !isDead
    w.write_u8(speeds.running as u8); // MOVE_MODE
    // Java's write order through here: SWIM_OR_FLY (never set — no NPC water/
    // fly state), TEAM, ENCHANT, FLYING, CLONE, PET_EVOLUTION_ID,
    // DISPLAY_EFFECT. The client reads them positionally, so the order matters
    // more than the individual fields.
    if contains(T::Team) {
        w.write_u8(npc.team);
    }
    if contains(T::Enchant) {
        w.write_i32(npc.enchant_effect);
    }
    w.write_i32(0); // PET_EVOLUTION_ID
    if contains(T::DisplayEffect) {
        w.write_i32(npc.display_effect);
    }
    if contains(T::CurrentHp) {
        w.write_i32(vitals.cur_hp as i32);
    }
    if contains(T::CurrentMp) {
        w.write_i32(vitals.cur_mp as i32);
    }
    if contains(T::MaxHp) {
        w.write_i32(vitals.max_hp);
    }
    if contains(T::MaxMp) {
        w.write_i32(vitals.max_mp);
    }
    if contains(T::Name) {
        w.write_string(&t.name);
    }
    if contains(T::Abnormals) {
        w.write_i16(abnormal_visuals.len() as i16);
        for id in abnormal_visuals {
            w.write_i16(*id);
        }
    }
    if contains(T::VisualState) {
        w.write_u8(status_mask);
    }
    w.into_bytes()
}

/// Port of `serverpackets/NpcHtmlMessage` — the NPC dialog window. `item_id`
/// stays 0: the window is replaced by the next html (or closed) on click.
pub fn npc_html_message(npc_object_id: i32, html: &str) -> Vec<u8> {
    npc_html_message_item(npc_object_id, 0, html)
}

/// `NpcHtmlMessage(npcObjId, itemId)`: a non-zero `item_id` marks the dialog
/// as item-bound, which the client does NOT close when a bypass button is
/// clicked — Java's `AdminHtml` sends every admin page as
/// `NpcHtmlMessage(0, 1)` so the GM menu survives its own buttons.
pub fn npc_html_message_item(npc_object_id: i32, item_id: i32, html: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::NPC_HTML_MESSAGE);
    w.write_i32(npc_object_id);
    w.write_string(clip_html(html));
    w.write_i32(item_id);
    w.write_i32(0); // show common board
    w.into_bytes()
}

/// `AbstractHtmlPacket.setHtml`'s length guard: the client *crashes* on an
/// oversized dialog, so Java clips anything past 17 200 chars with a warning.
/// Counted in chars like Java's `substring` (never mid-UTF-8).
pub(super) fn clip_html(html: &str) -> &str {
    const HTML_MAX_CHARS: usize = 17200;
    match html.char_indices().nth(HTML_MAX_CHARS) {
        Some((cut, _)) => {
            tracing::warn!(
                "NpcHtmlMessage: html is too long ({} chars)! this will crash the client!",
                html.chars().count()
            );
            &html[..cut]
        }
        None => html,
    }
}

/// Port of `serverpackets/SummonInfo` (masked, same 37-bit `NpcInfoType`
/// format as [`npc_info`]) — a servitor as seen by players **other than** its
/// owner. The owner gets `PetSummonInfo` (`PET_INFO`) instead, which is a flat
/// packet with the fed/lifetime bar.
///
/// The differences from `npc_info` are all summon-specific:
/// * `TITLE` is always present and carries the **owner's name**, which is what
///   draws "Servitor of X" under the model;
/// * `PVP_FLAG` is always present (Java adds it unconditionally);
/// * `NAME` rides along when the template's display id differs from its own id;
/// * `SUMMONED` marks the spawn animation.
///
/// Not modelled and left at Java's defaults: clan crests (the owner's), team,
/// reputation, water/fly and transformation. The weapon glow reads the
/// **template's** `weaponEnchant` here — Java's `SummonInfo` does not use the
/// random per-instance roll `NpcInfo` does, so a servitor only glows if its
/// template says so (nothing in this dist does).
#[allow(clippy::too_many_arguments)]
pub fn summon_info(
    object_id: i32,
    t: &NpcTemplate,
    pos: &crate::model::components::Position,
    vitals: &crate::model::components::Vitals,
    speeds: &crate::model::components::Speeds,
    combat: &crate::model::components::CombatStats,
    owner_name: &str,
    relation: i32,
    summoned: bool,
) -> Vec<u8> {
    use NpcInfoType as T;

    let mut mask_bytes: [u8; 5] = [0x00, 0x0C, 0x0C, 0x00, 0x00];
    let mut init_size: i32 = 0;
    let mut block_size: i32 = 0;
    let mut add = |mask_bytes: &mut [u8; 5], ty: T| {
        masks::add_mask(mask_bytes, ty.mask());
        match ty {
            T::Attackable | T::Relations => init_size += ty.block_length(),
            T::Title => init_size += ty.block_length() + owner_name.len() as i32 * 2,
            T::Name => block_size += ty.block_length() + t.name.len() as i32 * 2,
            _ => block_size += ty.block_length(),
        }
    };

    // Java's unconditional set for a summon.
    add(&mut mask_bytes, T::Attackable);
    add(&mut mask_bytes, T::Relations);
    add(&mut mask_bytes, T::Title);
    add(&mut mask_bytes, T::Id);
    add(&mut mask_bytes, T::Position);
    add(&mut mask_bytes, T::StopMode);
    add(&mut mask_bytes, T::MoveMode);
    add(&mut mask_bytes, T::PvpFlag);
    if t.display_id != t.id {
        add(&mut mask_bytes, T::Name);
    }
    if pos.heading > 0 {
        add(&mut mask_bytes, T::Heading);
    }
    if combat.p_atk_spd > 0 || combat.m_atk_spd > 0 {
        add(&mut mask_bytes, T::AtkCastSpeed);
    }
    if speeds.run_spd > 0.0 {
        add(&mut mask_bytes, T::SpeedMultiplier);
    }
    if vitals.max_hp > 0 {
        add(&mut mask_bytes, T::MaxHp);
    }
    if vitals.max_mp > 0 {
        add(&mut mask_bytes, T::MaxMp);
    }
    if vitals.cur_hp <= vitals.max_hp as f64 {
        add(&mut mask_bytes, T::CurrentHp);
    }
    if vitals.cur_mp <= vitals.max_mp as f64 {
        add(&mut mask_bytes, T::CurrentMp);
    }
    if summoned {
        add(&mut mask_bytes, T::Summoned);
    }
    if t.weapon_enchant > 0 {
        add(&mut mask_bytes, T::Enchant);
    }
    add(&mut mask_bytes, T::PetEvolutionId);
    // 0x01 in combat, 0x02 dead, 0x04 targetable, 0x08 always.
    let mut status_mask = 0x08u8;
    if vitals.dead {
        status_mask |= 0x02;
    }
    if t.targetable {
        status_mask |= 0x04;
    }
    add(&mut mask_bytes, T::VisualState);

    let contains = |ty: T| masks::contains_mask(&mask_bytes, ty.mask());

    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SUMMON_INFO);
    w.write_i32(object_id);
    w.write_u8(if summoned { 2 } else { 1 }); // 0=teleported 1=default 2=summoned
    w.write_i16(37); // mask_bits_37
    w.write_bytes(&mask_bytes);

    // Block 1.
    w.write_u8(init_size as u8);
    // `isAutoAttackable(attacker)` — a servitor is only auto-attackable in a
    // PvP state this port doesn't resolve per-viewer yet, so 0.
    w.write_u8(0);
    w.write_i32(relation);
    w.write_string(owner_name);

    // Block 2.
    w.write_i16(block_size as i16);
    w.write_i32(t.display_id + 1_000_000);
    w.write_i32(pos.x);
    w.write_i32(pos.y);
    w.write_i32(pos.z);
    if contains(T::Heading) {
        w.write_i32(pos.heading);
    }
    if contains(T::AtkCastSpeed) {
        w.write_i32(combat.p_atk_spd);
        w.write_i32(combat.m_atk_spd);
    }
    if contains(T::SpeedMultiplier) {
        w.write_f32(speeds.move_multiplier as f32);
        w.write_f32(1.0);
    }
    w.write_u8(u8::from(!vitals.dead)); // STOP_MODE
    w.write_u8(speeds.running as u8); // MOVE_MODE
    if contains(T::Enchant) {
        w.write_i32(t.weapon_enchant);
    }
    w.write_i32(0); // PET_EVOLUTION_ID
    if contains(T::CurrentHp) {
        w.write_i32(vitals.cur_hp as i32);
    }
    if contains(T::CurrentMp) {
        w.write_i32(vitals.cur_mp as i32);
    }
    if contains(T::MaxHp) {
        w.write_i32(vitals.max_hp);
    }
    if contains(T::MaxMp) {
        w.write_i32(vitals.max_mp);
    }
    if contains(T::Summoned) {
        w.write_u8(2); // "do some animation on spawn"
    }
    if contains(T::Name) {
        w.write_string(&t.name);
    }
    w.write_u8(0); // PVP_FLAG
    w.write_u8(status_mask); // VISUAL_STATE
    w.into_bytes()
}

/// Port of `serverpackets/SetSummonRemainTime` — refreshes the summon's
/// remaining-lifetime bar. Sent on every upkeep tick.
pub fn set_summon_remain_time(max_time: i32, remaining: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SET_SUMMON_REMAIN_TIME);
    w.write_i32(max_time);
    w.write_i32(remaining);
    w.into_bytes()
}

/// `PetItemList` (0xB3) — the contents of the pet's own inventory.
///
/// Just a count and the standard item entries; unlike the player's
/// `ItemList` (0x11) there is no "open window" flag or trailing block.
pub fn pet_item_list(
    inventory: &crate::model::inventory::Inventory,
    data: &crate::data::GameData,
) -> Vec<u8> {
    let entries: Vec<_> = inventory
        .items()
        .iter()
        .filter_map(|item| data.item_data.get(item.item_id).map(|t| (item, t)))
        .collect();

    let mut w = PacketWriter::new();
    w.write_u8(0xB3);
    w.write_i16(entries.len() as i16);
    for (item, template) in &entries {
        crate::network::enter_world::write_item_entry(&mut w, item, template, false);
    }
    w.into_bytes()
}

/// `SpecialCamera` (0xD6) — the cinematic camera, used by the grand-boss
/// entry sequences (Valakas 19 times, Antharas 7).
///
/// **`range` is accepted and dropped.** Java's canonical constructor takes it,
/// stores every other parameter, and never assigns it — the wire carries eleven
/// ints and `range` is not one of them. It is kept in the signature so call
/// sites transcribe the Java argument list literally rather than silently
/// shifting every following parameter by one.
///
/// (Java also ships an 11-arg overload that forwards `duration` and `range` into
/// each other's slots, so a caller's *range* is written as the duration. No
/// boss script uses it — all 26 call sites take the 12-arg form — so the port
/// does not reproduce the swap.)
#[allow(clippy::too_many_arguments)]
pub fn special_camera(
    object_id: i32,
    force: i32,
    angle1: i32,
    angle2: i32,
    time: i32,
    _range: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
    rel_angle: i32,
    unk: i32,
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(0xD6);
    w.write_i32(object_id);
    w.write_i32(force);
    w.write_i32(angle1);
    w.write_i32(angle2);
    w.write_i32(time);
    w.write_i32(duration);
    w.write_i32(rel_yaw);
    w.write_i32(rel_pitch);
    w.write_i32(is_wide);
    w.write_i32(rel_angle);
    w.write_i32(unk);
    w.into_bytes()
}

#[cfg(test)]
mod tests {
    use commons::network::PacketWriter;

    /// `NpcInfo` byte layout, hand-computed against the Java constructor +
    /// `writeImpl` (no client capture available for NPCs yet, unlike the
    /// UserInfo test — the mask math is shared with that byte-verified path).
    #[test]
    fn npc_info_layout_matches_java() {
        let mut t = crate::data::npc_data::default_template(30001);
        t.name = "Gina".into();
        t.server_side_name = true;
        t.level = 5;
        t.base_hp_max = 100.0;
        t.base_mp_max = 50.0;
        // Defaults keep: p_atk_spd 300, m_atk_spd 333, run 120, rhand/lhand 0,
        // targetable + show_name true (→ status mask 0x0C), type Folk.
        let (npc, (mut pos, _region, vitals, speeds, _collision, _attack, _ai, _aggro, _buffs)) =
            crate::model::npc::Npc::for_test(0x4000_0001, 30001, 100, 200, -300, 100, 50);
        pos.heading = 4000;
        let v = crate::model::npc::NpcView {
            npc: &npc,
            pos: &pos,
            vitals: &vitals,
            speeds: &speeds,
        };

        let mut w = PacketWriter::new();
        w.write_u8(0x0C); // NPC_INFO
        w.write_i32(0x4000_0001);
        w.write_u8(0); // no summon animation
        w.write_i16(37);
        // Components: Id, Attackable, Relations, Name, Position, Heading,
        // AtkCastSpeed | SpeedMultiplier, StopMode, MoveMode (+ pre-set
        // 0x0C/0x0D) | PetEvolutionId (+ pre-set 0x14/0x15) | CurrentHp,
        // CurrentMp, MaxHp, MaxMp | VisualState(37).
        w.write_bytes(&[0xFD, 0xBC, 0x1C, 0xF0, 0x04]);
        w.write_u8(5); // init size: attackable(1) + relations(4)
        w.write_u8(0); // Folk is not in the Attackable subtree
        w.write_i32(0); // relations
        w.write_i16(69); // block 2 size
        w.write_i32(1_030_001); // display id + 1000000
        w.write_i32(100);
        w.write_i32(200);
        w.write_i32(-300);
        w.write_i32(4000); // heading
        w.write_i32(300); // p atk spd
        w.write_i32(333); // m atk spd
        w.write_f32(1.0); // movement multiplier
        w.write_f32(1.0); // attack speed multiplier
        w.write_u8(1); // stop mode: alive
        w.write_u8(0); // move mode: walking
        w.write_i32(0); // pet evolution id
        w.write_i32(100); // cur hp
        w.write_i32(50); // cur mp
        w.write_i32(100); // max hp
        w.write_i32(50); // max mp
        w.write_string("Gina");
        w.write_u8(0x0C); // visual state: targetable | show name
        let expected = w.into_bytes();

        assert_eq!(
            super::npc_info(&v, &t, &crate::config::NpcConfig::default(), &[]),
            expected
        );
    }

    /// `EnableRandomEnchantEffect`: an NPC whose weapon glows adds Java's
    /// `ENCHANT` block (component 0x10 → high bit of mask byte 2) and writes
    /// the level between `MOVE_MODE` and `PET_EVOLUTION_ID`. Same NPC as
    /// [`npc_info_layout_matches_java`], so the diff is only the glow.
    #[test]
    fn npc_info_carries_the_weapon_enchant_glow() {
        let mut t = crate::data::npc_data::default_template(30001);
        t.name = "Gina".into();
        t.server_side_name = true;
        t.level = 5;
        t.base_hp_max = 100.0;
        t.base_mp_max = 50.0;
        let (mut npc, (mut pos, _region, vitals, speeds, _collision, _attack, _ai, _aggro, _buffs)) =
            crate::model::npc::Npc::for_test(0x4000_0001, 30001, 100, 200, -300, 100, 50);
        pos.heading = 4000;
        npc.enchant_effect = 12;
        let v = crate::model::npc::NpcView {
            npc: &npc,
            pos: &pos,
            vitals: &vitals,
            speeds: &speeds,
        };

        let mut w = PacketWriter::new();
        w.write_u8(0x0C); // NPC_INFO
        w.write_i32(0x4000_0001);
        w.write_u8(0);
        w.write_i16(37);
        // Byte 2 gains 0x80 over the no-glow layout: ENCHANT is component
        // 0x10, i.e. bit 0 of byte 2, and the mask order is reversed.
        w.write_bytes(&[0xFD, 0xBC, 0x9C, 0xF0, 0x04]);
        w.write_u8(5);
        w.write_u8(0);
        w.write_i32(0);
        w.write_i16(73); // block 2 size: 69 + the 4-byte ENCHANT block
        w.write_i32(1_030_001);
        w.write_i32(100);
        w.write_i32(200);
        w.write_i32(-300);
        w.write_i32(4000);
        w.write_i32(300);
        w.write_i32(333);
        w.write_f32(1.0);
        w.write_f32(1.0);
        w.write_u8(1); // stop mode
        w.write_u8(0); // move mode
        w.write_i32(12); // ENCHANT — before pet evolution id, like Java
        w.write_i32(0); // pet evolution id
        w.write_i32(100);
        w.write_i32(50);
        w.write_i32(100);
        w.write_i32(50);
        w.write_string("Gina");
        w.write_u8(0x0C);
        let expected = w.into_bytes();

        assert_eq!(
            super::npc_info(&v, &t, &crate::config::NpcConfig::default(), &[]),
            expected
        );
    }

    /// `Creature.getTitle()` monster branch, per config combination.
    #[test]
    fn npc_title_level_and_aggression_flags() {
        let mut t = crate::data::npc_data::default_template(20001);
        t.type_name = "Monster".into();
        t.level = 20;
        t.is_aggressive = true;
        t.clans = vec!["ORC_CLAN".into()];
        t.clan_help_range = 300;

        let mut cfg = crate::config::NpcConfig::default();
        assert_eq!(super::npc_title(&t, &cfg), ""); // both off → template title
        cfg.show_npc_level = true;
        cfg.show_npc_aggression = true;
        assert_eq!(super::npc_title(&t, &cfg), "Lv 20 [A][G]");

        t.is_aggressive = false;
        t.clan_help_range = 0;
        // Java writes the separator space even when no flag follows.
        assert_eq!(super::npc_title(&t, &cfg), "Lv 20 ");

        cfg.show_npc_level = false;
        t.is_aggressive = true;
        t.title = "Raid Fighter".into();
        assert_eq!(super::npc_title(&t, &cfg), "[A] Raid Fighter");

        // Non-monsters keep their template title untouched.
        t.type_name = "Folk".into();
        assert_eq!(super::npc_title(&t, &cfg), "Raid Fighter");
    }
}
