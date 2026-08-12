//! HP/MP/CP regeneration (`CreatureStatus.doRegeneration`), run every
//! `REGEN_TICK_PERIOD` ticks from the game loop.

use crate::data::GameData;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::model::components::{BaseStats, PlayerVitals, StatModifiers, Vitals};
use crate::model::stats::{BaseStat, MoveType, Stat};
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
    // Latched once per tick: the three `Character.ini` regen multipliers Java
    // applies to every creature (`RegenHPFinalizer` line 61 and its siblings).
    let cfg_mult = (
        world.cfg.npc.hp_regen_multiplier,
        world.cfg.npc.mp_regen_multiplier,
        world.cfg.npc.cp_regen_multiplier,
    );
    for (client_id, object_id) in targets {
        // Dead-or-full players skip the whole tick up front: the move-type
        // and clan-hall probes below (the latter a zone polygon query) are
        // the expensive part, and an idle town of topped-up players would
        // otherwise pay them every 3 s for nothing. `regen_player` re-checks
        // the same condition — this is purely the cheap pre-filter.
        let skip = world
            .objects
            .get_component::<Vitals>(&object_id)
            .zip(world.objects.get_component::<PlayerVitals>(&object_id))
            .is_none_or(|(v, pv)| {
                v.dead
                    || (v.cur_hp >= v.max_hp as f64
                        && v.cur_mp >= v.max_mp as f64
                        && pv.cur_cp >= pv.max_cp as f64)
            });
        if skip {
            continue;
        }
        // Java reads the move type live inside the finalizer
        // (`creature.getMoveType()`), so it is resolved per regen tick here
        // rather than cached anywhere — a player who starts running between
        // ticks regenerates at the running rate on the very next one.
        let move_type = move_type_of(world, object_id);
        // The clan-hall regen boost (read before the mutable borrow below),
        // composed with the castle-function boost — a player is in at most one
        // residence zone, so at most one term differs from 1.
        let (hall_hp_mult, hall_mp_mult) = clan_hall_regen_mult(world, object_id);
        let (castle_hp_mult, castle_mp_mult) = castle_regen_mult(world, object_id);
        let (hall_hp_mult, hall_mp_mult) =
            (hall_hp_mult * castle_hp_mult, hall_mp_mult * castle_mp_mult);
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
            player,
            &mut vitals,
            &mut pvitals,
            base,
            mods,
            move_type,
            &world.data,
            hall_hp_mult,
            hall_mp_mult,
            cfg_mult,
        ) else {
            continue;
        };
        send_to_client(
            world,
            client_id,
            server_packets::status_update(object_id, &updates),
        );
        super::party::notify_party_vitals(world, object_id);
    }
}

/// The clan-hall HP/MP regen multipliers for a player: `(1.0, 1.0)` unless the
/// player is a clan member standing in **their own** hall that has bought the
/// `HP_REGEN` / `MP_REGEN` function (Java `RegenHPFinalizer`/`RegenMPFinalizer`,
/// whose `clanHallIndex == posChIndex` check is "you are in the hall your clan
/// owns" — derived here from the hall's `owner_id`).
pub(crate) fn clan_hall_regen_mult(world: &World, object_id: i32) -> (f64, f64) {
    let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
    else {
        return (1.0, 1.0);
    };
    let clan_id = clan_of_or_zero(world, object_id);
    if clan_id == 0 {
        return (1.0, 1.0);
    }
    let Some(hall_id) = world.data.zone_data.clan_hall_at(pos.x, pos.y, pos.z) else {
        return (1.0, 1.0);
    };
    // Only your *own* hall boosts your regen.
    if world.clan_halls.get(&hall_id).map(|h| h.owner_id) != Some(clan_id) {
        return (1.0, 1.0);
    }
    let hp =
        super::clan_hall_function::active_function_value(world, hall_id, "HP_REGEN").unwrap_or(1.0);
    let mp =
        super::clan_hall_function::active_function_value(world, hall_id, "MP_REGEN").unwrap_or(1.0);
    (hp, mp)
}

/// The castle-function regen boost (`RegenHPFinalizer`/`RegenMPFinalizer`'s
/// castle branch): a clan member standing in their own clan's castle zone
/// with an active HP/MP-regen function gets `baseValue *= (lvl / 100)` —
/// **integer division, as Java wrote it**. The HP levels (300/400) come out
/// ×3/×4; the MP levels (40/55) come out ×0, i.e. Java's shipped code zeroes
/// MP regen inside the castle while the MP function is rented. Ported as
/// behaviour, documented as the bug it is.
pub(crate) fn castle_regen_mult(world: &World, object_id: i32) -> (f64, f64) {
    let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&object_id)
    else {
        return (1.0, 1.0);
    };
    let clan_id = clan_of_or_zero(world, object_id);
    if clan_id == 0 {
        return (1.0, 1.0);
    }
    let Some(castle_id) = world.data.zone_data.castle_zone_at(pos.x, pos.y, pos.z) else {
        return (1.0, 1.0);
    };
    // Only your own clan's castle boosts you (Java compares the clan's castle
    // id against the zone's residence id).
    if world.clans.get(&clan_id).map(|c| c.castle_id) != Some(castle_id) {
        return (1.0, 1.0);
    }
    let mult = |func_type: i32| {
        super::castle::castle_function(world, castle_id, func_type)
            .map(|f| f64::from(f.level / 100))
            .unwrap_or(1.0)
    };
    (
        mult(crate::model::castle::FUNC_RESTORE_HP),
        mult(crate::model::castle::FUNC_RESTORE_MP),
    )
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
///
/// The sitting branch wins over everything: Java's `Player.getMoveType`
/// short-circuits on `_waitTypeSitting` before it looks at movement at all,
/// which is what makes the seated regen bonus the largest one.
pub(crate) fn move_type_of(world: &World, object_id: i32) -> MoveType {
    if super::sit_stand::is_sitting(world, object_id) {
        return MoveType::Sitting;
    }
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

/// `RegenHPFinalizer`/`RegenMPFinalizer`/`RegenCPFinalizer`, **including the
/// `Hp`/`Mp`/`CpRegenMultiplier` config terms**. Java applies those to every
/// creature — `baseValue *= isRaid ? RAID_… : HP_REGEN_MULTIPLIER` sits above
/// the `isPlayer()` branch — and this path used to skip them for players while
/// the NPC and pet paths beside it applied them. All three are 100 (×1.0) on
/// this dist, so the omission was inert, which is exactly how it survived.
/// Returns the `StatusUpdate` entries for
/// whichever of HP/MP/CP actually changed, or `None` if all are already full.
///
/// Each rate ends in Java's `Stat.defaultValue(creature, stat, baseValue)` —
/// `mul * baseValue + add + getMoveTypeValue(stat, getMoveType())` — which is
/// what makes the regen *stats* mean anything. Until this slice `regen_player`
/// never looked at `StatModifiers` at all: every `HpRegen`/`MpRegen`/`CpRegen`
/// effect in the datapack (21 learnable skills — Regeneration 1044, Song of
/// Life 265, Focus Mind 191, Mana Regeneration 1045, …) parsed to a modifier
/// that was pumped and then read by nobody.
#[allow(clippy::too_many_arguments)]
pub(crate) fn regen_player(
    p: &Player,
    vitals: &mut Vitals,
    pvitals: &mut PlayerVitals,
    base: &BaseStats,
    mods: &StatModifiers,
    move_type: MoveType,
    data: &GameData,
    // Clan-hall HP/MP-regen function multipliers (1.0 = no boost) — Java's
    // `baseValue *= func.getValue()` inside the regen finalizer, applied while a
    // clan member stands in their own hall (`RegenHPFinalizer`/`RegenMPFinalizer`).
    hall_hp_mult: f64,
    hall_mp_mult: f64,
    // `Hp`/`Mp`/`CpRegenMultiplier` from `Character.ini`, as fractions. Passed
    // in rather than read from `World` because this function takes only the
    // components it touches; all three are ×1.0 on this dist.
    cfg_mult: (f64, f64, f64),
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
    // Borrow, don't clone — the template carries six level tables and a
    // stat map, and this runs per wounded player per regen tick. The static
    // default only exists to keep the `None` arm allocation-free too.
    static DEFAULT_TEMPLATE: std::sync::LazyLock<crate::data::player_template::PlayerTemplate> =
        std::sync::LazyLock::new(Default::default);
    let t = data
        .player_templates
        .get_or_base(p.class_id, p.base_class_id)
        .unwrap_or(&DEFAULT_TEMPLATE);
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
            t.base_hp_regen(p.level) * cfg_mult.0 * movement * level_mod * con_bonus * hall_hp_mult,
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
            t.base_mp_regen(p.level) * cfg_mult.1 * movement * level_mod * men_bonus * hall_mp_mult,
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
            t.base_cp_regen(p.level) * cfg_mult.2 * level_mod * con_bonus * movement,
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
        let (mut hp_mul, mp_mul) = if is_raid {
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
        // `RegenHPFinalizer`: `baseValue *= CHAMPION_HP_REGEN` right after the
        // raid/normal multiplier and before everything else. There is no
        // champion **MP** regen key — Java's `RegenMPFinalizer` has no champion
        // arm at all, so `mp_mul` is deliberately left alone.
        if world.cfg.champion.enable
            && world
                .objects
                .get_component::<Npc>(&oid)
                .is_some_and(|n| n.champion)
        {
            hp_mul *= world.cfg.champion.hp_regen;
        }
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
        if changed && let Some(region) = region_cell_of(world, oid) {
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
