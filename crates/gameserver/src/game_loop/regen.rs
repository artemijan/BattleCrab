//! HP/MP/CP regeneration (`CreatureStatus.doRegeneration`), run every
//! `REGEN_TICK_PERIOD` ticks from the game loop.

use crate::data::GameData;
use crate::model::stats::BaseStat;
use crate::model::Player;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

/// `Formulas.getRegeneratePeriod`: 3000 ms for player characters (30 × the
/// 100 ms base tick), matching Java's `CreatureStatus.startHpMpRegeneration`.
pub(crate) const REGEN_TICK_PERIOD: u64 = 30;


/// `CreatureStatus.doRegeneration`, run every `REGEN_TICK_PERIOD` ticks for
/// every in-game player. Iterates connected clients (not `world.players`
/// directly) so each player's `StatusUpdate` reaches its own connection.
pub(crate) fn run_regen_tick(world: &mut World) {
    let targets: Vec<(u32, i32)> = world
        .clients
        .iter()
        .filter_map(|(&client_id, cs)| match cs {
            ClientSession::InGame(s) => Some((client_id, s.player_object_id())),
            _ => None,
        })
        .collect();
    for (client_id, object_id) in targets {
        let Some(player) = world.players.get_mut(&object_id) else { continue };
        let Some(updates) = regen_player(player, &world.data) else { continue };
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::status_update(object_id, &updates));
        }
    }
}

/// `Formulas.getRegeneratePeriod`'s standing-still multiplier (1.1×) — the
/// only movement state a player can be in until G7 adds sitting/moving.
/// TODO(G7): sitting (×1.5) / running (×0.7) once those states exist.
pub(crate) const STANDING_STILL_REGEN_MULTIPLIER: f64 = 1.1;

/// `RegenHPFinalizer`/`RegenMPFinalizer`/`RegenCPFinalizer`, config-multiplier
/// terms omitted (`HpRegenMultiplier`/… default to 1.0 — see the `MAX_*`
/// stat-cap TODO in `model/mod.rs`). Returns the `StatusUpdate` entries for
/// whichever of HP/MP/CP actually changed, or `None` if all are already full.
pub(crate) fn regen_player(p: &mut Player, data: &GameData) -> Option<Vec<(u8, i32)>> {
    // The dead don't regenerate (`CreatureStatus.stopHpMpRegeneration` on death).
    if p.dead {
        return None;
    }
    if p.cur_hp >= p.max_hp as f64 && p.cur_mp >= p.max_mp as f64 && p.cur_cp >= p.max_cp as f64 {
        return None;
    }
    let t = data
        .player_templates
        .get(p.class_id)
        .or_else(|| data.player_templates.get(p.base_class_id))
        .cloned()
        .unwrap_or_default();
    let level_mod = (p.level as f64 + 89.0) / 100.0;
    let con_bonus = data.stat_bonus.bonus(BaseStat::Con, p.con);
    let men_bonus = data.stat_bonus.bonus(BaseStat::Men, p.men);

    let mut updates = Vec::new();
    if p.cur_hp < p.max_hp as f64 {
        let regen = t.base_hp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        p.cur_hp = (p.cur_hp + regen).min(p.max_hp as f64);
        updates.push((server_packets::status_update_type::CUR_HP, p.cur_hp as i32));
    }
    if p.cur_mp < p.max_mp as f64 {
        let regen = t.base_mp_regen(p.level) * level_mod * men_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        p.cur_mp = (p.cur_mp + regen).min(p.max_mp as f64);
        updates.push((server_packets::status_update_type::CUR_MP, p.cur_mp as i32));
    }
    if p.cur_cp < p.max_cp as f64 {
        let regen = t.base_cp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        p.cur_cp = (p.cur_cp + regen).min(p.max_cp as f64);
        updates.push((server_packets::status_update_type::CUR_CP, p.cur_cp as i32));
    }
    if updates.is_empty() {
        None
    } else {
        Some(updates)
    }
}
