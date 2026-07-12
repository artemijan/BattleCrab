//! HP/MP/CP regeneration (`CreatureStatus.doRegeneration`), run every
//! `REGEN_TICK_PERIOD` ticks from the game loop.

use crate::data::GameData;
use crate::model::components::{BaseStats, PlayerVitals, Vitals};
use crate::model::stats::BaseStat;
use crate::model::Player;
use crate::network::server_packets;
use crate::session::ClientSession;
use crate::world::World;

/// `Formulas.getRegeneratePeriod`: 3000 ms for player characters (30 × the
/// 100 ms base tick), matching Java's `CreatureStatus.startHpMpRegeneration`.
pub(crate) const REGEN_TICK_PERIOD: u64 = 30;


/// `CreatureStatus.doRegeneration`, run every `REGEN_TICK_PERIOD` ticks for
/// every in-game player. Iterates connected clients (not `world.objects`
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
        let Some((player, mut vitals, mut pvitals, base)) = world
            .objects
            .get_many_mut::<(&Player, &mut Vitals, &mut PlayerVitals, &BaseStats)>(&object_id)
        else {
            continue;
        };
        let Some(updates) = regen_player(&player, &mut vitals, &mut pvitals, &base, &world.data) else { continue };
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
pub(crate) fn regen_player(
    p: &Player,
    vitals: &mut Vitals,
    pvitals: &mut PlayerVitals,
    base: &BaseStats,
    data: &GameData,
) -> Option<Vec<(u8, i32)>> {
    // The dead don't regenerate (`CreatureStatus.stopHpMpRegeneration` on death).
    if vitals.dead {
        return None;
    }
    if vitals.cur_hp >= vitals.max_hp as f64
        && vitals.cur_mp >= vitals.max_mp as f64
        && pvitals.cur_cp >= pvitals.max_cp as f64
    {
        return None;
    }
    let t = data
        .player_templates
        .get(p.class_id)
        .or_else(|| data.player_templates.get(p.base_class_id))
        .cloned()
        .unwrap_or_default();
    let level_mod = (p.level as f64 + 89.0) / 100.0;
    let con_bonus = data.stat_bonus.bonus(BaseStat::Con, base.con);
    let men_bonus = data.stat_bonus.bonus(BaseStat::Men, base.men);

    let mut updates = Vec::new();
    if vitals.cur_hp < vitals.max_hp as f64 {
        let regen = t.base_hp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        vitals.cur_hp = (vitals.cur_hp + regen).min(vitals.max_hp as f64);
        updates.push((server_packets::status_update_type::CUR_HP, vitals.cur_hp as i32));
    }
    if vitals.cur_mp < vitals.max_mp as f64 {
        let regen = t.base_mp_regen(p.level) * level_mod * men_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        vitals.cur_mp = (vitals.cur_mp + regen).min(vitals.max_mp as f64);
        updates.push((server_packets::status_update_type::CUR_MP, vitals.cur_mp as i32));
    }
    if pvitals.cur_cp < pvitals.max_cp as f64 {
        let regen = t.base_cp_regen(p.level) * level_mod * con_bonus * STANDING_STILL_REGEN_MULTIPLIER;
        pvitals.cur_cp = (pvitals.cur_cp + regen).min(pvitals.max_cp as f64);
        updates.push((server_packets::status_update_type::CUR_CP, pvitals.cur_cp as i32));
    }
    if updates.is_empty() {
        None
    } else {
        Some(updates)
    }
}
