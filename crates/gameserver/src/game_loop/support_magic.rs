//! Port of `handlers/bypasshandlers/SupportMagic.java` and
//! `handlers/bypasshandlers/SupportBlessing.java`: the Newbie Helper / Newbie
//! Guide / Gatekeeper buffs. The htm windows (`default/SupportMagic.htm`,
//! `default/BlessingOfProtection.htm`) are served through the `Link` whitelist;
//! their buttons post the `SupportMagic` / `SupportMagicServitor` / `GiveBlessing`
//! bypass verbs handled here.
//!
//! Narrowing vs. Java:
//! - No servitors exist yet, so `SupportMagicServitor` always answers the
//!   "no summon" page (Java's `!player.hasServitors()` branch).
//! - `player.isCursedWeaponEquipped()` is modeled (`cursed_weapon_equipped_id`),
//!   so the Java guard is kept.
//! - A few buff effects aren't modeled yet and silently drop when cast (they
//!   fall through the skill-effect parser): Vampiric Rage (`VampiricAttack`),
//!   Concentration (`ReduceCancel`), and Life Cubic (`SummonCubic`, cubics
//!   unported). The remaining buffs and the animation land normally.

use tracing::warn;

use crate::game_loop::skills::effects::apply_skill_effects;
use crate::model::Player;
use crate::model::components::Position;
use crate::network::server_packets;
use crate::world::World;

// --- SupportMagic buff sets (Java `SkillHolder` tables). ---
const HASTE_1: (i32, i32) = (4327, 1); // Haste
/// Java `HASTE_2` = skill 5632, cast only at `level >= MAX_NEWBIE_BUFF_LEVEL+1`,
/// i.e. above the cap — effectively disabled. Kept for reference.
const _HASTE_2: (i32, i32) = (5632, 1);
const CUBIC: (i32, i32) = (4338, 1); // Life Cubic (unmodeled effect: SummonCubic)

const FIGHTER_BUFFS: &[(i32, i32)] = &[
    (4322, 1), // Wind Walk
    (4323, 1), // Shield
    (5637, 1), // Magic Barrier
    (4324, 1), // Bless the Body
    (4325, 1), // Vampiric Rage
    (4326, 1), // Regeneration
];
const MAGE_BUFFS: &[(i32, i32)] = &[
    (4322, 1), // Wind Walk
    (4323, 1), // Shield
    (5637, 1), // Magic Barrier
    (4328, 1), // Bless the Soul
    (4329, 1), // Acumen
    (4330, 1), // Concentration
    (4331, 1), // Empower
];
/// Java `SUMMON_BUFFS` — applied to a servitor by the `SupportMagicServitor`
/// verb. No servitors exist yet (that verb always answers "no summon"), so this
/// table is unused until summons land.
const _SUMMON_BUFFS: &[(i32, i32)] = &[
    (4322, 1),
    (4323, 1),
    (5637, 1),
    (4324, 1),
    (4325, 1),
    (4326, 1),
    (4328, 1),
    (4329, 1),
    (4330, 1),
    (4331, 1),
];

// --- Levels (Java constants). ---
const LOWEST_LEVEL: i32 = 6;
const CUBIC_LOWEST: i32 = 16;
const CUBIC_HIGHEST: i32 = 34;
/// Java `Config.MAX_NEWBIE_BUFF_LEVEL` — `dist/game/config/Character.ini`'s
/// `MaxNewbieBuffLevel = 40` (the authoritative datapack value).
const MAX_NEWBIE_BUFF_LEVEL: i32 = 40;

const BLESSING_OF_PROTECTION: (i32, i32) = (5182, 1); // CommonSkill.BLESSING_OF_PROTECTION

/// `bypasshandlers/SupportBlessing.useBypass`: Blessing of Protection, gated on
/// level ≤ 39 and not-yet-2nd-class.
pub(crate) fn give_blessing(world: &mut World, client_id: u32, object_id: i32, npc_object_id: i32) {
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    let level = player.level;
    let class_id = player.class_id;
    // Java `player.getClassId().level() >= 2` — completed the 2nd (or 3rd)
    // class transfer. THIRD/FOURTH_CLASS_GROUP == classId.level() 2/3.
    let second_class_done = world
        .data
        .categories
        .contains("THIRD_CLASS_GROUP", class_id)
        || world
            .data
            .categories
            .contains("FOURTH_CLASS_GROUP", class_id);
    if level > 39 || second_class_done {
        send_default_html(
            world,
            client_id,
            npc_object_id,
            "SupportBlessingHighLevel.htm",
        );
        return;
    }
    cast_from_npc(world, npc_object_id, object_id, BLESSING_OF_PROTECTION);
}

/// `bypasshandlers/SupportMagic.makeSupportMagic`. `is_summon` = the
/// `SupportMagicServitor` verb.
pub(crate) fn support_magic(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    npc_object_id: i32,
    is_summon: bool,
) {
    let Some(player) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    // Java: `player.isCursedWeaponEquipped()` blocks the whole bypass.
    if player.cursed_weapon_equipped_id != 0 {
        return;
    }
    let level = player.level;
    let class_id = player.class_id;

    // No servitors exist yet — Java's `!player.hasServitors()` branch is always
    // taken for the servitor verb.
    if is_summon {
        send_default_html(world, client_id, npc_object_id, "SupportMagicNoSummon.htm");
        return;
    }
    if level < LOWEST_LEVEL {
        send_default_html(world, client_id, npc_object_id, "SupportMagicLowLevel.htm");
        return;
    }
    if level > MAX_NEWBIE_BUFF_LEVEL {
        send_default_html(world, client_id, npc_object_id, "SupportMagicHighLevel.htm");
        return;
    }
    // Java `player.getClassId().level() == 3` (custom message). FOURTH_CLASS_GROUP
    // == classId.level() 3; unreachable in Interlude but ported faithfully.
    if world
        .data
        .categories
        .contains("FOURTH_CLASS_GROUP", class_id)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                server_packets::sm_ids::S1_TEXT,
                &[server_packets::SmParam::Text(
                    "Only adventurers who have not completed their 3rd class transfer may receive these buffs.".to_string(),
                )],
            ));
        }
        return;
    }

    // `player.isInCategory(CategoryType.BEGINNER_MAGE)`.
    let is_mage = world.data.categories.contains("BEGINNER_MAGE", class_id);
    let buffs: &[(i32, i32)] = if is_mage { MAGE_BUFFS } else { FIGHTER_BUFFS };
    for &skill in buffs {
        cast_from_npc(world, npc_object_id, object_id, skill);
    }
    // Fighters also get Haste (Java's HASTE_2 tier is above the cap → never).
    if !is_mage {
        cast_from_npc(world, npc_object_id, object_id, HASTE_1);
    }
    // Life Cubic for levels 16-34 (effect unmodeled — SummonCubic — so the cast
    // animation plays but no cubic spawns yet).
    if (CUBIC_LOWEST..=CUBIC_HIGHEST).contains(&level) {
        cast_from_npc(world, npc_object_id, object_id, CUBIC);
    }
}

/// Java `npc.setTarget(target); SkillCaster.triggerCast(npc, target, skill)`:
/// broadcast the NPC's cast animation, then apply the skill's effects on the
/// target (no cast bar, no MP cost — the trigger-cast path).
pub(crate) fn cast_from_npc(
    world: &mut World,
    npc_object_id: i32,
    target_oid: i32,
    (skill_id, skill_level): (i32, i32),
) {
    let Some(skill) = world.data.skill_data.get(skill_id, skill_level).cloned() else {
        warn!("SupportMagic: skill {skill_id}/{skill_level} missing from skill data.");
        return;
    };
    // Animation: MagicSkillUse from the NPC onto the target (Java's
    // `broadcastPacket(new MagicSkillUse(...))` inside triggerCast).
    let npc_pos = world
        .objects
        .get_component::<Position>(&npc_object_id)
        .copied();
    let target_pos = world
        .objects
        .get_component::<Position>(&target_oid)
        .copied();
    if let (Some(np), Some(tp)) = (npc_pos, target_pos) {
        let pkt = server_packets::magic_skill_use_raw(
            (npc_object_id, np.x, np.y, np.z),
            (target_oid, tp.x, tp.y, tp.z),
            skill_id,
            skill_level,
            skill.hit_time,
        );
        crate::game_loop::helpers::broadcast_including_self(world, target_oid, &pkt);
    }
    apply_skill_effects(world, npc_object_id, target_oid, &skill);
}

/// Serve a `data/html/default/<file>` window through the clicked NPC (Java
/// `npc.showChatWindow(player, "data/html/default/...")`).
fn send_default_html(world: &World, client_id: u32, npc_object_id: i32, file: &str) {
    let html =
        crate::data::htm_cache::read_htm(format!("{}data/html/default/{file}", world.data.root))
            .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
            .replace("%objectId%", &npc_object_id.to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_object_id, &html));
    }
}
