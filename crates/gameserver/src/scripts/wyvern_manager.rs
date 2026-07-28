//! Wyvern Manager NPC — port of
//! `dist/game/data/scripts/ai/others/WyvernManager/WyvernManager.java`.
//!
//! The castle/clan-hall official who exchanges a level-55+ strider (which the
//! *clan leader of the owning clan* must already be riding) plus 25 B-grade
//! crystals for a wyvern mount. On this dist's config
//! (`AllowRideWyvernAlways = False`) every *castle* manager answers with the
//! Seal of Strife block page (`wyvernmanager-dusk.html`) and never mounts —
//! Java behaves identically (Seven Signs is gone from the codebase but the
//! flag gates on) — so only the clan-hall manager (35419, unspawned hall) and
//! GM `//ride_wyvern` produce actual wyverns until the flag is flipped.
//!
//! Java's `FORT` manager type has no registered NPC id and is dead code; not
//! ported.

use crate::game_loop::quests::{QuestCtx, QuestScript};
use crate::model::Player;
use crate::model::components::Position;
use crate::network::server_packets::{SmParam, sm_ids, system_message_with};

pub struct WyvernManager;

const CRYSTAL_B_GRADE: i32 = 1460;
const WYVERN: i32 = 12621;
const WYVERN_FEE: i64 = 25;
const STRIDER_LEVEL: i32 = 55;
/// The strider species (incl. event ones) a wyvern can be exchanged for.
const STRIDERS: &[i32] = &[12526, 12527, 12528, 16038, 16039, 16040, 16068, 13197];

/// Castle managers + the clan-hall manager (35419). `MANAGERS` in Java.
const MANAGER_IDS: &[i32] = &[
    35101, 35143, 35185, 35227, 35275, 35317, 35364, 35510, 35536, 35556, 35419,
];

#[derive(PartialEq)]
enum ManagerType {
    Castle,
    ClanHall,
}

fn manager_type(npc_id: i32) -> ManagerType {
    if npc_id == 35419 {
        ManagerType::ClanHall
    } else {
        ManagerType::Castle
    }
}

/// Java resolves `npc.getCastle()` by residence zone; every castle manager id
/// serves exactly one castle, so the mapping is static (same approach as
/// `chamberlain_castle_id`). 35536 is registered in Java but unspawned on
/// this dist — no castle zone would resolve for it, hence `None`.
fn manager_castle_id(npc_id: i32) -> Option<i32> {
    Some(match npc_id {
        35101 => 1, // Gludio
        35143 => 2, // Dion
        35185 => 3, // Giran
        35227 => 4, // Oren
        35275 => 5, // Aden
        35317 => 6, // Innadril
        35364 => 7, // Goddard
        35510 => 8, // Rune
        35556 => 9, // Schuttgart
        _ => return None,
    })
}

/// `npc.getCastle().getName()` for the static map above.
fn castle_name(castle_id: i32) -> &'static str {
    match castle_id {
        1 => "Gludio",
        2 => "Dion",
        3 => "Giran",
        4 => "Oren",
        5 => "Aden",
        6 => "Innadril",
        7 => "Goddard",
        8 => "Rune",
        9 => "Schuttgart",
        _ => "",
    }
}

impl QuestScript for WyvernManager {
    fn id(&self) -> i32 {
        -1
    }
    fn name(&self) -> &'static str {
        "WyvernManager"
    }
    fn html_dir(&self) -> &'static str {
        "ai/others/WyvernManager"
    }
    fn start_npcs(&self) -> &[i32] {
        MANAGER_IDS
    }
    fn talk_npcs(&self) -> &[i32] {
        MANAGER_IDS
    }
    fn first_talk_npcs(&self) -> &[i32] {
        MANAGER_IDS
    }

    fn on_first_talk(&self, ctx: &mut QuestCtx) -> Option<String> {
        Some(main_page(ctx))
    }

    fn on_talk(&self, _ctx: &mut QuestCtx) -> Option<String> {
        None
    }

    fn on_event(&self, ctx: &mut QuestCtx, event: &str) -> Option<String> {
        match event {
            // Java `case "Return"` — identical to onFirstTalk.
            "Return" => Some(main_page(ctx)),
            "Help" => Some(match manager_type(ctx.npc_id) {
                ManagerType::Castle => replace_part(ctx, "wyvernmanager-03.html"),
                ManagerType::ClanHall => replace_part(ctx, "wyvernmanager-03b.html"),
            }),
            "RideWyvern" => {
                let cfg = &ctx.world.cfg.feature;
                if !cfg.allow_ride_wyvern_always {
                    if !cfg.allow_ride_wyvern_during_siege
                        && (manager_in_siege(ctx) || player_in_siege(ctx))
                    {
                        // Java `player.sendMessage(...)` — the plain-text SM.
                        if let Some(cs) = ctx.world.clients.get(&ctx.client_id) {
                            cs.send(system_message_with(
                                sm_ids::S1_TEXT,
                                &[SmParam::Text(
                                    "You cannot summon wyvern while in siege.".to_string(),
                                )],
                            ));
                        }
                        return None;
                    }
                    if manager_type(ctx.npc_id) == ManagerType::Castle {
                        return Some("wyvernmanager-dusk.html".to_string());
                    }
                }
                Some(mount_wyvern(ctx))
            }
            _ => None,
        }
    }
}

/// Java `onFirstTalk`/`case "Return"`: non-owners get turned away; owners get
/// the console — except castle managers under the Seal of Strife block.
fn main_page(ctx: &mut QuestCtx) -> String {
    if !is_owner_clan(ctx) {
        "wyvernmanager-02.html".to_string()
    } else if ctx.world.cfg.feature.allow_ride_wyvern_always
        || manager_type(ctx.npc_id) != ManagerType::Castle
    {
        replace_all(ctx)
    } else {
        "wyvernmanager-dusk.html".to_string()
    }
}

/// Java `mountWyvern`: must already be riding a level-55+ strider; the owning
/// clan's leader pays 25 B-grade crystals, the strider is swapped for the
/// wyvern.
fn mount_wyvern(ctx: &mut QuestCtx) -> String {
    let riding_strider = ctx
        .world
        .objects
        .get_component::<Player>(&ctx.player)
        .is_some_and(|p| {
            p.is_mounted() && p.mount_level >= STRIDER_LEVEL && STRIDERS.contains(&p.mount_npc_id)
        });
    if !riding_strider {
        return replace_part(ctx, "wyvernmanager-05.html");
    }
    if !(is_owner_clan(ctx) && ctx.quest_items_count(CRYSTAL_B_GRADE) >= WYVERN_FEE) {
        return replace_part(ctx, "wyvernmanager-06.html");
    }
    ctx.take_items(CRYSTAL_B_GRADE, WYVERN_FEE);
    let player = ctx.player;
    crate::game_loop::admin::mounts::dismount(ctx.world, player);
    // Java `player.mount(WYVERN, 0, true)` — MountType.WYVERN ordinal 2.
    crate::game_loop::admin::mounts::mount_player(ctx.world, player, WYVERN, 2);
    "wyvernmanager-04.html".to_string()
}

/// Java `isOwnerClan`: the player must be the *leader* of the clan owning
/// this manager's residence.
fn is_owner_clan(ctx: &QuestCtx) -> bool {
    let Some(p) = ctx.world.objects.get_component::<Player>(&ctx.player) else {
        return false;
    };
    // Java `player.isClanLeader()` — the clan's own leader id, not the cached
    // Player flag.
    let is_leader = p.clan_id != 0
        && ctx
            .world
            .clans
            .get(&p.clan_id)
            .is_some_and(|c| c.leader_id == ctx.player);
    if !is_leader {
        return false;
    }
    match manager_type(ctx.npc_id) {
        ManagerType::Castle => manager_castle_id(ctx.npc_id).is_some_and(|castle_id| {
            crate::game_loop::manor::castle_owner_clan_id(ctx.world, castle_id) == Some(p.clan_id)
        }),
        // `npc.getClanHall()` resolves by zone; on this dist no clan-hall zone
        // covers 35419's spawn (Devastated Castle is not among the 48 halls),
        // so this is `None` → false, matching Java's null-hall branch.
        ManagerType::ClanHall => hall_id_of_npc(ctx).is_some_and(|hall_id| {
            ctx.world
                .clan_halls
                .get(&hall_id)
                .is_some_and(|h| h.owner_id == p.clan_id)
        }),
    }
}

/// The clan hall whose zone contains this manager NPC (Java
/// `npc.getClanHall()`).
fn hall_id_of_npc(ctx: &QuestCtx) -> Option<i32> {
    let pos = ctx.world.objects.get_component::<Position>(&ctx.npc)?;
    ctx.world.data.zone_data.clan_hall_at(pos.x, pos.y, pos.z)
}

/// Java `isInSiege(npc)`: castle managers check their castle's siege zone;
/// the clan-hall type is always false (hall sieges are off-dist).
fn manager_in_siege(ctx: &QuestCtx) -> bool {
    manager_castle_id(ctx.npc_id).is_some_and(|castle_id| {
        ctx.world
            .sieges
            .get(&castle_id)
            .is_some_and(|s| s.in_progress)
    })
}

/// Java `player.isInSiege()` — standing in a siege zone whose siege is
/// running.
fn player_in_siege(ctx: &QuestCtx) -> bool {
    let Some(pos) = ctx.world.objects.get_component::<Position>(&ctx.player) else {
        return false;
    };
    match ctx
        .world
        .data
        .zone_data
        .siege_castle_at(pos.x, pos.y, pos.z)
    {
        Some(castle_id) => ctx
            .world
            .sieges
            .get(&castle_id)
            .is_some_and(|s| s.in_progress),
        None => false,
    }
}

/// Java `replaceAll` — the main console with the residence name filled in.
fn replace_all(ctx: &QuestCtx) -> String {
    let residence = match manager_type(ctx.npc_id) {
        ManagerType::Castle => manager_castle_id(ctx.npc_id)
            .map(castle_name)
            .unwrap_or("")
            .to_string(),
        ManagerType::ClanHall => hall_id_of_npc(ctx)
            .and_then(|id| ctx.world.clan_halls.get(&id))
            .map(|h| h.name.clone())
            .unwrap_or_default(),
    };
    replace_part(ctx, "wyvernmanager-01.html").replace("%residence_name%", &residence)
}

/// Java `replacePart` — read a page and fill the fee/level placeholders.
fn replace_part(ctx: &QuestCtx, file: &str) -> String {
    crate::data::htm_cache::read_htm(format!(
        "{}data/scripts/ai/others/WyvernManager/{file}",
        ctx.world.data.root
    ))
    .unwrap_or_default()
    .replace("%wyvern_fee%", &WYVERN_FEE.to_string())
    .replace("%strider_level%", &STRIDER_LEVEL.to_string())
}
