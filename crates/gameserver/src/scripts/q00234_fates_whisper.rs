//! Fate's Whisper (234) — `quests/Q00234_FatesWhisper`. The level-75 A-grade
//! weapon quest: assemble the Infernium materials from four raid bosses, have
//! Maestro Reorin forge them into a Star of Destiny, then trade a B-grade weapon
//! up to its A-grade counterpart.
//!
//! Two mechanics beyond the usual cond ladder:
//!  - **Boss chests** (`onKill`). Each of four raid bosses drops a chest NPC on
//!    death; the player talks to the chest for a Soul Orb or Infernium Scepter.
//!  - **Pipette on Baium** (`onAttack`). While on cond 7 and wielding the Pipette
//!    Knife, striking Baium fills it (Red Pipette Knife) — the Infernium Varnish
//!    ingredient.
//!  - **The weapon UI** (`onEvent`). Reorin's B-/A-grade selection stores the
//!    chosen weapon in the quest vars `weaponId`/`bypass` and renders templated
//!    pages (`%weaponname%`), exactly like Java's `getHtm(...).replace(...)`.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::network::server_packets::quest_sounds;

// NPCs
const MAESTRO_REORIN: i32 = 31002; // start
const NPC_30182: i32 = 30182;
const NPC_30847: i32 = 30847;
const NPC_30178: i32 = 30178;
const NPC_30833: i32 = 30833;
const CHEST_31027: i32 = 31027;
const CHEST_31028: i32 = 31028;
const CHEST_31029: i32 = 31029;
const CHEST_31030: i32 = 31030;
// Raid bosses that drop the chests
const BOSS_25035: i32 = 25035;
const BOSS_25054: i32 = 25054;
const BOSS_25126: i32 = 25126;
const BOSS_25220: i32 = 25220;
const BAIUM: i32 = 29020;
// Items
const REIRIA_SOUL_ORB: i32 = 4666;
const KERMON_INFERNIUM_SCEPTER: i32 = 4667;
const GOLKONDA_INFERNIUM_SCEPTER: i32 = 4668;
const HALLATE_INFERNIUM_SCEPTER: i32 = 4669;
const INFERNIUM_VARNISH: i32 = 4672;
const REORIN_HAMMER: i32 = 4670;
const REORIN_MOLD: i32 = 4671;
const PIPETTE_KNIFE: i32 = 4665;
const RED_PIPETTE_KNIFE: i32 = 4673;
const CRYSTAL_B: i32 = 1460;
// Reward
const STAR_OF_DESTINY: i32 = 5011;
// Misc
/// Java `addSpawn(…, 120000)` — the chest lives two minutes.
const CHEST_DESPAWN_MS: u64 = 120_000;
const MIN_LEVEL: i32 = 75;

/// The thirteen B-grade weapons Reorin can upgrade, id → name (Java's `WEAPONS`
/// map). The name is only used to fill `%weaponname%` in the dialog pages.
fn weapon_name(id: i32) -> &'static str {
    match id {
        79 => "Sword of Damascus",
        97 => "Lance",
        171 => "Deadman's Glory",
        175 => "Art of Battle Axe",
        210 => "Staff of Evil Spirits",
        234 => "Demon Dagger",
        268 => "Bellion Cestus",
        287 => "Bow of Peril",
        2626 => "Samurai Dual-sword",
        7883 => "Guardian Sword",
        7889 => "Wizard's Tear",
        7893 => "Kaim Vanul's Bones",
        7901 => "Star Buster",
        _ => "",
    }
}

/// Boss id → the chest NPC it spawns on death (Java's `CHEST_SPAWN`).
fn chest_for_boss(boss_id: i32) -> Option<i32> {
    Some(match boss_id {
        BOSS_25035 => CHEST_31027,
        BOSS_25054 => CHEST_31028,
        BOSS_25126 => CHEST_31029,
        BOSS_25220 => CHEST_31030,
        _ => return None,
    })
}

fn has(ctx: &QuestCtx, item: i32) -> bool {
    ctx.quest_items_count(item) > 0
}

pub struct Q00234FatesWhisper;

impl QuestScript for Q00234FatesWhisper {
    fn id(&self) -> i32 {
        234
    }
    fn name(&self) -> &'static str {
        "Q00234_FatesWhisper"
    }
    fn html_dir(&self) -> &'static str {
        "quests/Q00234_FatesWhisper"
    }
    fn start_npcs(&self) -> &[i32] {
        &[MAESTRO_REORIN]
    }
    fn talk_npcs(&self) -> &[i32] {
        &[
            MAESTRO_REORIN,
            NPC_30182,
            NPC_30847,
            NPC_30178,
            NPC_30833,
            CHEST_31027,
            CHEST_31028,
            CHEST_31029,
            CHEST_31030,
        ]
    }
    fn kill_npcs(&self) -> &[i32] {
        &[BOSS_25035, BOSS_25054, BOSS_25126, BOSS_25220]
    }
    fn attack_npcs(&self) -> &[i32] {
        &[BAIUM]
    }
    fn quest_items(&self) -> &[i32] {
        // Java's registerQuestItems — only the two pipettes are auto-cleared.
        &[PIPETTE_KNIFE, RED_PIPETTE_KNIFE]
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        if !ctx.has_qs() {
            return None;
        }
        match event {
            "31002-03.htm" => {
                ctx.start_quest();
                return Some(event.to_string());
            }
            "30182-01c.htm" => {
                ctx.play_sound(quest_sounds::ITEMGET);
                ctx.give_items(INFERNIUM_VARNISH, 1);
                return Some(event.to_string());
            }
            "30178-01a.htm" => {
                ctx.set_cond(6, true);
                return Some(event.to_string());
            }
            "30833-01b.htm" => {
                ctx.set_cond(7, true);
                ctx.give_items(PIPETTE_KNIFE, 1);
                return Some(event.to_string());
            }
            _ => {}
        }

        // Reorin's weapon UI.
        if let Some(id) = event.strip_prefix("selectBGrade_") {
            if ctx.get_int("bypass") == 1 {
                return None;
            }
            ctx.set_var("weaponId", id);
            let name = weapon_name(id.parse().unwrap_or(0));
            return Some(ctx.get_htm("31002-13.htm").replace("%weaponname%", name));
        }
        if event.starts_with("confirmWeapon") {
            ctx.set_var("bypass", "1");
            let name = weapon_name(ctx.get_int("weaponId"));
            return Some(ctx.get_htm("31002-14.htm").replace("%weaponname%", name));
        }
        if let Some(a_grade) = event.strip_prefix("selectAGrade_") {
            if ctx.get_int("bypass") != 1 {
                return Some("31002-16.htm".to_string());
            }
            let b_grade = ctx.get_int("weaponId");
            if has(ctx, b_grade) {
                let a_id: i32 = a_grade.parse().unwrap_or(0);
                let a_name = ctx.item_name(a_id);
                ctx.take_items(b_grade, 1);
                ctx.give_items(a_id, 1);
                ctx.give_items(STAR_OF_DESTINY, 1);
                ctx.social_action(3);
                ctx.exit_quest(false, true);
                return Some(ctx.get_htm("31002-12.htm").replace("%weaponname%", &a_name));
            }
            let name = weapon_name(b_grade);
            return Some(ctx.get_htm("31002-15.htm").replace("%weaponname%", name));
        }

        // Any other `.htm`/`.html` bypass is a navigation link — return it so the
        // page loads, mirroring Java's `htmltext = event` default.
        if event.ends_with(".htm") || event.ends_with(".html") {
            return Some(event.to_string());
        }
        None
    }

    fn on_attack(&self, ctx: &mut QuestCtx) {
        // Baium: dipping the Pipette Knife in its blood fills it (cond 7).
        if ctx.npc_id != BAIUM || !ctx.has_qs() || ctx.cond() != 7 {
            return;
        }
        if ctx.equipped_weapon_id() == PIPETTE_KNIFE && !has(ctx, RED_PIPETTE_KNIFE) {
            ctx.play_sound(quest_sounds::ITEMGET);
            ctx.take_items(PIPETTE_KNIFE, 1);
            ctx.give_items(RED_PIPETTE_KNIFE, 1);
        }
    }

    fn on_kill(&self, ctx: &mut QuestCtx) {
        // Each boss drops a chest beside its corpse, which Java despawns after
        // two minutes (`addSpawn(…, true, 120000)`) whether or not anyone
        // opened it. Without that the chest lingered until talked-to or
        // restart, so a missed drop stayed on the field indefinitely.
        if let Some(chest) = chest_for_boss(ctx.npc_id)
            && let Some(oid) = ctx.spawn_near_npc(chest, true)
        {
            ctx.schedule_despawn(oid, CHEST_DESPAWN_MS);
        }
    }

    fn on_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        ctx.ensure_qs();
        if ctx.is_created() {
            return Some(if ctx.player_level() < MIN_LEVEL {
                "31002-01.htm".to_string()
            } else {
                "31002-02.htm".to_string()
            });
        }
        if ctx.is_completed() {
            return Some(ctx.already_completed_html());
        }
        // Started.
        let cond = ctx.cond();
        let html = match ctx.npc_id {
            MAESTRO_REORIN => reorin_talk(ctx, cond),
            NPC_30182 => {
                if cond == 3 {
                    if has(ctx, INFERNIUM_VARNISH) {
                        "30182-02.htm".to_string()
                    } else {
                        "30182-01.htm".to_string()
                    }
                } else {
                    ctx.no_quest_html()
                }
            }
            NPC_30847 => {
                if cond == 4 && !has(ctx, REORIN_HAMMER) {
                    ctx.play_sound(quest_sounds::ITEMGET);
                    ctx.give_items(REORIN_HAMMER, 1);
                    "30847-01.htm".to_string()
                } else if cond >= 4 && has(ctx, REORIN_HAMMER) {
                    "30847-02.htm".to_string()
                } else {
                    ctx.no_quest_html()
                }
            }
            NPC_30178 => {
                if cond == 5 {
                    "30178-01.htm".to_string()
                } else if cond > 5 {
                    "30178-02.htm".to_string()
                } else {
                    ctx.no_quest_html()
                }
            }
            NPC_30833 => npc_30833_talk(ctx, cond),
            CHEST_31027 => {
                if cond == 1 && !has(ctx, REIRIA_SOUL_ORB) {
                    ctx.play_sound(quest_sounds::ITEMGET);
                    ctx.give_items(REIRIA_SOUL_ORB, 1);
                    "31027-01.htm".to_string()
                } else {
                    "31027-02.htm".to_string()
                }
            }
            CHEST_31028 | CHEST_31029 | CHEST_31030 => {
                // itemId = npcId - 26361 → the matching Infernium Scepter.
                let item_id = ctx.npc_id - 26361;
                if cond == 2 && !has(ctx, item_id) {
                    ctx.play_sound(quest_sounds::ITEMGET);
                    ctx.give_items(item_id, 1);
                    format!("{}-01.htm", ctx.npc_id)
                } else {
                    format!("{}-02.htm", ctx.npc_id)
                }
            }
            _ => ctx.no_quest_html(),
        };
        Some(html)
    }
}

fn reorin_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        1 => {
            if has(ctx, REIRIA_SOUL_ORB) {
                ctx.set_cond(2, true);
                ctx.take_items(REIRIA_SOUL_ORB, 1);
                "31002-05.htm".to_string()
            } else {
                "31002-04b.htm".to_string()
            }
        }
        2 => {
            if has(ctx, KERMON_INFERNIUM_SCEPTER)
                && has(ctx, GOLKONDA_INFERNIUM_SCEPTER)
                && has(ctx, HALLATE_INFERNIUM_SCEPTER)
            {
                ctx.set_cond(3, true);
                ctx.take_items(GOLKONDA_INFERNIUM_SCEPTER, 1);
                ctx.take_items(HALLATE_INFERNIUM_SCEPTER, 1);
                ctx.take_items(KERMON_INFERNIUM_SCEPTER, 1);
                "31002-06.htm".to_string()
            } else {
                "31002-05c.htm".to_string()
            }
        }
        3 => {
            if has(ctx, INFERNIUM_VARNISH) {
                ctx.set_cond(4, true);
                ctx.take_items(INFERNIUM_VARNISH, 1);
                "31002-07.htm".to_string()
            } else {
                "31002-06b.htm".to_string()
            }
        }
        4 => {
            if has(ctx, REORIN_HAMMER) {
                ctx.set_cond(5, true);
                ctx.take_items(REORIN_HAMMER, 1);
                "31002-08.htm".to_string()
            } else {
                "31002-07b.htm".to_string()
            }
        }
        5..=7 => "31002-08b.htm".to_string(),
        8 => {
            ctx.set_cond(9, true);
            ctx.take_items(REORIN_MOLD, 1);
            "31002-09.htm".to_string()
        }
        9 => {
            if ctx.quest_items_count(CRYSTAL_B) < 984 {
                "31002-09b.htm".to_string()
            } else {
                ctx.set_cond(10, true);
                ctx.take_items(CRYSTAL_B, 984);
                "31002-BGradeList.htm".to_string()
            }
        }
        10 => {
            if ctx.get_int("bypass") == 1 {
                let item_id = ctx.get_int("weaponId");
                if has(ctx, item_id) {
                    ctx.get_htm("31002-AGradeList.htm")
                        .replace("%weaponname%", weapon_name(item_id))
                } else {
                    ctx.get_htm("31002-15.htm")
                        .replace("%weaponname%", weapon_name(item_id))
                }
            } else {
                "31002-BGradeList.htm".to_string()
            }
        }
        _ => ctx.no_quest_html(),
    }
}

fn npc_30833_talk(ctx: &mut QuestCtx, cond: i32) -> String {
    match cond {
        6 => "30833-01.htm".to_string(),
        7 => {
            if has(ctx, PIPETTE_KNIFE) && !has(ctx, RED_PIPETTE_KNIFE) {
                "30833-02.htm".to_string()
            } else {
                ctx.set_cond(8, true);
                ctx.take_items(RED_PIPETTE_KNIFE, 1);
                ctx.give_items(REORIN_MOLD, 1);
                "30833-03.htm".to_string()
            }
        }
        c if c > 7 => "30833-04.htm".to_string(),
        _ => ctx.no_quest_html(),
    }
}
