//! HP/MP/CP regeneration (`CreatureStatus.doRegeneration`), run every
//! `REGEN_TICK_PERIOD` ticks from the game loop.

use crate::data::GameData;
use crate::model::components::{BaseStats, PlayerVitals, StatModifiers, Vitals};
use crate::model::stats::{BaseStat, MoveType, Stat};
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
        // Java reads the move type live inside the finalizer
        // (`creature.getMoveType()`), so it is resolved per regen tick here
        // rather than cached anywhere — a player who starts running between
        // ticks regenerates at the running rate on the very next one.
        let move_type = move_type_of(world, object_id);
        let Some((player, mut vitals, mut pvitals, base, mods)) =
            world.objects.get_many_mut::<(
                &Player,
                &mut Vitals,
                &mut PlayerVitals,
                &BaseStats,
                &StatModifiers,
            )>(&object_id)
        else {
            continue;
        };
        let Some(updates) = regen_player(
            &player,
            &mut vitals,
            &mut pvitals,
            &base,
            &mods,
            move_type,
            &world.data,
        ) else {
            continue;
        };
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::status_update(object_id, &updates));
        }
        super::party::notify_party_vitals(world, object_id);
    }
}

/// `Formulas.getRegeneratePeriod`'s standing-still multiplier (1.1×).
/// Superseded as a blanket constant by [`movement_regen_multiplier`]; kept as
/// the name for the standing case it still is.
pub(crate) const STANDING_STILL_REGEN_MULTIPLIER: f64 = 1.1;

/// The "Calculate Movement bonus" block shared verbatim by all three regen
/// finalizers (`RegenHPFinalizer`/`RegenMPFinalizer`/`RegenCPFinalizer`):
///
/// ```java
/// if (player.isSitting())        baseValue *= 1.5; // Sitting
/// else if (!player.isMoving())   baseValue *= 1.1; // Staying
/// else if (player.isRunning())   baseValue *= 0.7; // Running
/// ```
///
/// Note the **walking case falls through all three branches** and so gets no
/// multiplier at all — walking regen (×1.0) is *worse* than standing still
/// (×1.1). That is Java as written, not an omission here.
///
/// Before this slice the port hard-coded the standing 1.1 for every state, so
/// running players regenerated ~57% faster than they should have.
pub(crate) fn movement_regen_multiplier(move_type: MoveType) -> f64 {
    match move_type {
        MoveType::Sitting => 1.5,
        MoveType::Standing => STANDING_STILL_REGEN_MULTIPLIER,
        MoveType::Running => 0.7,
        MoveType::Walking => 1.0,
    }
}

/// Java `Creature.getMoveType`, with `Player`'s sitting override folded in.
/// Sitting is not modeled on this port (`TODO(G29)`), so the seated branch has
/// no source yet and the result is one of walking/running/standing.
pub(crate) fn move_type_of(world: &World, object_id: i32) -> MoveType {
    let moving = world
        .objects
        .has_component::<crate::model::components::Movement>(&object_id);
    if !moving {
        return MoveType::Standing;
    }
    let running = world
        .objects
        .get_component::<crate::model::components::Speeds>(&object_id)
        .is_some_and(|s| s.running);
    if running {
        MoveType::Running
    } else {
        MoveType::Walking
    }
}

/// `RegenHPFinalizer`/`RegenMPFinalizer`/`RegenCPFinalizer`, config-multiplier
/// terms omitted (`HpRegenMultiplier`/… default to 1.0 — see the `MAX_*`
/// stat-cap TODO in `model/mod.rs`). Returns the `StatusUpdate` entries for
/// whichever of HP/MP/CP actually changed, or `None` if all are already full.
///
/// Each rate ends in Java's `Stat.defaultValue(creature, stat, baseValue)` —
/// `mul * baseValue + add + getMoveTypeValue(stat, getMoveType())` — which is
/// what makes the regen *stats* mean anything. Until this slice `regen_player`
/// never looked at `StatModifiers` at all: every `HpRegen`/`MpRegen`/`CpRegen`
/// effect in the datapack (21 learnable skills — Regeneration 1044, Song of
/// Life 265, Focus Mind 191, Mana Regeneration 1045, …) parsed to a modifier
/// that was pumped and then read by nobody.
pub(crate) fn regen_player(
    p: &Player,
    vitals: &mut Vitals,
    pvitals: &mut PlayerVitals,
    base: &BaseStats,
    mods: &StatModifiers,
    move_type: MoveType,
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

    let movement = movement_regen_multiplier(move_type);
    // `Stat.defaultValue(creature, stat, baseValue)`: the buff `mul`/`add` and
    // the move-type term wrap the finalizer's own base computation.
    let finalize = |stat: Stat, base_value: f64| -> f64 {
        let mul = mods.mul.get(&stat).copied().unwrap_or(1.0);
        let add = mods.add.get(&stat).copied().unwrap_or(0.0);
        (mul * base_value) + add + mods.move_type_value(stat, move_type)
    };

    let mut updates = Vec::new();
    if vitals.cur_hp < vitals.max_hp as f64 {
        let regen = finalize(
            Stat::RegenerateHpRate,
            t.base_hp_regen(p.level) * movement * level_mod * con_bonus,
        );
        vitals.cur_hp = (vitals.cur_hp + regen).min(vitals.max_hp as f64);
        updates.push((
            server_packets::status_update_type::CUR_HP,
            vitals.cur_hp as i32,
        ));
    }
    if vitals.cur_mp < vitals.max_mp as f64 {
        let regen = finalize(
            Stat::RegenerateMpRate,
            t.base_mp_regen(p.level) * movement * level_mod * men_bonus,
        );
        vitals.cur_mp = (vitals.cur_mp + regen).min(vitals.max_mp as f64);
        updates.push((
            server_packets::status_update_type::CUR_MP,
            vitals.cur_mp as i32,
        ));
    }
    if pvitals.cur_cp < pvitals.max_cp as f64 {
        let regen = finalize(
            Stat::RegenerateCpRate,
            t.base_cp_regen(p.level) * level_mod * con_bonus * movement,
        );
        pvitals.cur_cp = (pvitals.cur_cp + regen).min(pvitals.max_cp as f64);
        updates.push((
            server_packets::status_update_type::CUR_CP,
            pvitals.cur_cp as i32,
        ));
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
        let Some(t) = world.data.npc_data.get(npc_id) else {
            continue;
        };
        let is_raid = matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss");
        let (hp_mul, mp_mul) = if is_raid {
            (
                world.cfg.npc.raid_hp_regen_multiplier,
                world.cfg.npc.raid_mp_regen_multiplier,
            )
        } else {
            (
                world.cfg.npc.hp_regen_multiplier,
                world.cfg.npc.mp_regen_multiplier,
            )
        };
        // Java `RegenHPFinalizer`/`RegenMPFinalizer` pet branch: a pet regens
        // from its **per-level pet row**, under its own multipliers — the same
        // "substitute the base, keep the pipeline" shape as every other pet
        // stat (slice 13). Read live here because regen re-reads the template
        // each tick rather than caching onto a component.
        let pet_row = world
            .objects
            .get_component::<crate::model::components::PetOf>(&oid)
            .and_then(|p| {
                world
                    .data
                    .pet_data
                    .get(npc_id)
                    .and_then(|pt| pt.levels.get(&p.level))
            });
        let (hp_regen, mp_regen) = match pet_row {
            Some(row) => (
                row.regen_hp * world.cfg.npc.pet_hp_regen_multiplier,
                row.regen_mp * world.cfg.npc.pet_mp_regen_multiplier,
            ),
            None => (t.base_hp_reg * hp_mul, t.base_mp_reg * mp_mul),
        };

        // Scoped so the mutable borrow ends before the broadcast reads the
        // store again.
        let (cur_hp, max_hp, changed) = {
            let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) else {
                continue;
            };
            let before_hp = v.cur_hp;
            v.cur_hp = (v.cur_hp + hp_regen).min(v.max_hp as f64);
            v.cur_mp = (v.cur_mp + mp_regen).min(v.max_mp as f64);
            (v.cur_hp as i32, v.max_hp, v.cur_hp != before_hp)
        };

        // `broadcastStatusUpdate` — refresh the HP bar for anyone targeting it.
        // Only on an actual HP change, so a full-HP/low-MP mob doesn't spam.
        if changed {
            if let Some(region) = world
                .objects
                .get_component::<crate::model::components::RegionCell>(&oid)
                .map(|r| r.0)
            {
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
