//! Auto potions — port of `taskmanager/AutoPotionTaskManager` and its
//! `.apon`/`.apoff` voiced commands, gated on `Custom/AutoPotions.ini`
//! (`AutoPotionsEnabled = True` on this dist).
//!
//! Once a second, each opted-in player is topped up from their own potions:
//! CP, HP and MP each have a threshold, an enable flag, and an ordered list of
//! acceptable items. The potion is *used* through the normal item-skill path,
//! so its cast, cooldown and item consumption are the same as drinking it by
//! hand.
//!
//! **Java's "out of potions" message is noisier than it looks.** Its `success`
//! flag is set when a configured potion is merely *present* in the bag — not
//! when one is drunk — so a player at full health with potions stays quiet,
//! while a player carrying none is told once a second, forever. Ported
//! verbatim, because the alternative silently changes what an operator sees.

use crate::game_loop::helpers::hp_pair;
use crate::model::Player;
use crate::model::components::{PlayerVitals, Vitals};
use crate::world::World;

use super::helpers::client_for_player;

/// Java `schedulePriorityTaskAtFixedRate(this, 0, 1000)` — one second, which is
/// ten game-loop ticks.
pub(crate) const TICK_PERIOD: u64 = 10;

/// `.apon` / `.potionon` and `.apoff` / `.potionoff`.
pub(crate) fn handle_voiced(world: &mut World, client_id: u32, player_oid: i32, command: &str) {
    let cfg = &world.cfg.auto_potions;
    if !cfg.enabled {
        return;
    }
    let level = world
        .objects
        .get_component::<Player>(&player_oid)
        .map_or(0, |p| p.level);
    if level < cfg.minimum_level {
        let text = format!(
            "You need to be at least {} to use auto potions.",
            cfg.minimum_level
        );
        message(world, client_id, &text);
        return;
    }
    match command {
        "apon" | "potionon" => {
            world.auto_potion_players.insert(player_oid);
            message(world, client_id, "Auto potions is enabled.");
        }
        _ => {
            world.auto_potion_players.remove(&player_oid);
            message(world, client_id, "Auto potions is disabled.");
        }
    }
}

/// `AutoPotionTaskManager.run` — the one-second sweep.
pub(crate) fn tick(world: &mut World) {
    if !world.cfg.auto_potions.enabled || world.auto_potion_players.is_empty() {
        return;
    }
    // One clone per sweep, not per player — the pool id lists ride along.
    let cfg = world.cfg.auto_potions.clone();
    let players: Vec<i32> = world.auto_potion_players.iter().copied().collect();
    for player_oid in players {
        // Java drops (not skips) anyone dead, offline, or in the Olympiad
        // without `AutoPotionsInOlympiad` — so re-enabling means typing `.apon`
        // again.
        let alive = world
            .objects
            .get_component::<Vitals>(&player_oid)
            .is_some_and(|v| !v.dead);
        let online = client_for_player(world, player_oid).is_some();
        let in_oly = world.olympiad.in_competition.contains(&player_oid);
        if !alive || !online || (!world.cfg.auto_potions.in_olympiad && in_oly) {
            world.auto_potion_players.remove(&player_oid);
            continue;
        }
        run_for_player(world, player_oid, &cfg);
    }
}

/// The three pools, in Java's order: HP, then CP, then MP.
fn run_for_player(world: &mut World, player_oid: i32, cfg: &crate::config::AutoPotionsConfig) {
    let hp = hp_pair(world, player_oid);
    let mp = world
        .objects
        .get_component::<Vitals>(&player_oid)
        .map(|v| (v.cur_mp, v.max_mp as f64));
    let cp = world
        .objects
        .get_component::<PlayerVitals>(&player_oid)
        .map(|v| (v.cur_cp, v.max_cp as f64));

    let mut carries_any = false;
    for (pool, label, pools) in [
        (&cfg.hp, "HP", hp),
        (&cfg.cp, "CP", cp),
        (&cfg.mp, "MP", mp),
    ] {
        let (Some((current, max)), true) = (pools, pool.enabled) else {
            continue;
        };
        let below = max > 0.0 && (current / max) * 100.0 < pool.percentage as f64;
        // Java walks the id list and sets `success` on the first one the player
        // *carries*, drinking only if the pool is actually low.
        for &item_id in &pool.item_ids {
            let Some(item_object_id) =
                crate::game_loop::helpers::carried_item(world, player_oid, item_id)
            else {
                continue;
            };
            carries_any = true;
            if below {
                super::items::use_item_by_object_id(world, player_oid, item_object_id);
                if let Some(cid) = client_for_player(world, player_oid) {
                    message(world, cid, &format!("Auto potion: Restored {label}."));
                }
                break;
            }
        }
    }
    if !carries_any && let Some(cid) = client_for_player(world, player_oid) {
        message(world, cid, "Auto potion: You are out of potions!");
    }
}

/// The object id of the first instance of `item_id` the player carries, if any
/// (Java `getInventory().getItemByItemId`).
fn message(world: &World, client_id: u32, text: &str) {
    crate::game_loop::admin::send_message(world, client_id, text);
}

/// Drop a player from the loop (logout, and anything else that ends a session).
pub(crate) fn remove(world: &mut World, player_oid: i32) {
    world.auto_potion_players.remove(&player_oid);
}
