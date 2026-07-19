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
        super::party::notify_party_vitals(world, object_id);
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

/// `CreatureStatus.doRegeneration` for NPCs — the half that was never ported.
/// 14855 templates on this dist declare an `hpRegen` (only 58 are zero), and
/// none of it did anything: a wounded mob stayed wounded until it despawned,
/// and a raid boss whittled down over several attempts never recovered.
///
/// **The NPC formula is much shorter than the player one, and that's Java, not
/// a narrowing.** In `RegenHPFinalizer` the level-mod, CON/MEN bonus and the
/// sitting/standing/running multipliers all sit *inside* `if (isPlayer())`.
/// An NPC's rate is simply its template value times the config multiplier.
///
/// Java also regenerates **during combat** — the task only checks "not dead"
/// and "not already full", never an in-combat flag. That is deliberate here
/// too: it's what makes a long fight against a high-regen boss a DPS race.
pub(crate) fn run_npc_regen_tick(world: &mut World) {
    use crate::model::npc::Npc;

    // Collect first: applying regen needs the template (a `world.data` read)
    // while the vitals are borrowed, and the broadcast needs `world` again.
    let mut wounded: Vec<(i32, i32)> = Vec::new();
    world.objects.for_each_mut::<(&Npc, &Vitals)>(|(npc, v)| {
        if !v.dead && (v.cur_hp < v.max_hp as f64 || v.cur_mp < v.max_mp as f64) {
            wounded.push((npc.object_id, npc.npc_id));
        }
    });

    for (oid, npc_id) in wounded {
        let Some(t) = world.data.npc_data.get(npc_id) else { continue };
        let is_raid = matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss");
        let (hp_mul, mp_mul) = if is_raid {
            (world.cfg.npc.raid_hp_regen_multiplier, world.cfg.npc.raid_mp_regen_multiplier)
        } else {
            (world.cfg.npc.hp_regen_multiplier, world.cfg.npc.mp_regen_multiplier)
        };
        let hp_regen = t.base_hp_reg * hp_mul;
        let mp_regen = t.base_mp_reg * mp_mul;

        // Scoped so the mutable borrow ends before the broadcast reads the
        // store again.
        let (cur_hp, max_hp, changed) = {
            let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) else { continue };
            let before_hp = v.cur_hp;
            v.cur_hp = (v.cur_hp + hp_regen).min(v.max_hp as f64);
            v.cur_mp = (v.cur_mp + mp_regen).min(v.max_mp as f64);
            (v.cur_hp as i32, v.max_hp, v.cur_hp != before_hp)
        };

        // `broadcastStatusUpdate` — refresh the HP bar for anyone targeting it.
        // Only on an actual HP change, so a full-HP/low-MP mob doesn't spam.
        if changed {
            if let Some(region) = world.objects.get_component::<crate::model::components::RegionCell>(&oid).map(|r| r.0) {
                super::helpers::broadcast_near_region(
                    world,
                    region,
                    &server_packets::status_update(
                        oid,
                        &[
                            (server_packets::status_update_type::MAX_HP, max_hp),
                            (server_packets::status_update_type::CUR_HP, cur_hp),
                        ],
                    ),
                );
            }
        }
    }
}
