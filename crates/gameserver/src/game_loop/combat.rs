//! The auto-attack pipeline (G9): `AttackRequest` handling, the player
//! intent think loops (`PlayerAI.thinkAttack`/`thinkCast` +
//! `CreatureFollowTask` — chase into attack or cast range, then act), the
//! shared swing/hit mechanics (`Creature.doAutoAttack` →
//! `CreatureAttackTaskManager` → `onHitTimeNotDual` → `onHitTarget`), and the
//! combat-stance tracker (`AttackStanceTaskManager`).
//!
//! Scope (see PROGRESS G9): melee swings only — bows/crossbows, dual-weapon
//! split hits, polearm sweeps, soulshots, and shield blocks are all deferred
//! (their formula terms are identity for the actors that exist). PvP
//! auto-attack (force-attacking players) is deferred with the PvP-flag
//! system.

use crate::game_loop::common::maybe_distance_too_far;
use crate::model::components::{
    AttackState, Casting, Collision, CombatStats, Intent, Movement, PlayerVitals, Position,
    RegionCell, Speeds, Vitals,
};
use crate::model::formulas;
use crate::model::movement::{self, get_position, MoveData};
use crate::model::npc::{AggroList, NpcAi, NpcIntention};
use crate::model::stats::BaseStat;
use crate::model::PlayerIntent;
use crate::network::client_packets as cp;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::scheduler::ScheduledTask;
use crate::session::ClientSession;
use crate::world::World;

use super::helpers::{
    broadcast_including_self, broadcast_near_region, client_for_player, ms_to_ticks,
};
use super::skills::cast::break_cast;

/// `AttackStanceTaskManager.COMBAT_TIME` (15 s) in ticks.
pub(crate) const COMBAT_STANCE_TICKS: u64 = 150;

/// NPC object ids live above this base (`model::npc::FIRST_NPC_OBJECT_ID`);
/// everything below is a persistent id (players, items).
pub(crate) fn is_npc_oid(object_id: i32) -> bool {
    object_id >= crate::model::npc::FIRST_NPC_OBJECT_ID
}

/// `Vitals` of any combat actor (one store since the world merge).
pub(crate) fn vitals_of(world: &World, object_id: i32) -> Option<&Vitals> {
    world.objects.get_component::<Vitals>(&object_id)
}

/// Whether an attack target is dead/gone across creatures and doors: a
/// breached siege gate (0 HP) counts as dead, like a corpse, so the attack
/// loop ends on it. A vanished object (no `Vitals`, no `Door`) is also "dead".
pub(crate) fn target_is_dead(world: &World, object_id: i32) -> bool {
    if let Some(door) = world.objects.get_component::<crate::model::door::Door>(&object_id) {
        return door.current_hp <= 0;
    }
    vitals_of(world, object_id).is_none_or(|v| v.dead)
}

/// The combat-relevant view of a player or NPC — the stat finalizer outputs
/// both kinds of combatant feed into the shared `Formulas` ports. NPC values
/// are derived on demand from the template (same finalizer math the player's
/// `recalculate_stats` runs: base × stat bonus × level mod), since NPCs have
/// no buff state yet.
pub(crate) struct Combatant {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub collision_radius: f64,
    pub dead: bool,
    pub p_atk: f64,
    pub p_def: f64,
    pub crit_stat: f64,
    pub accuracy: i32,
    pub evasion: i32,
    pub p_atk_spd: i32,
    pub random_dmg: i32,
    pub atk_range: i32,
    /// Shield block defence (`getShldDef`, added to pDef on a normal block) —
    /// 0 when no shield is equipped in the left hand.
    pub shield_def: f64,
    /// Shield block *rate* already multiplied by this actor's CON bonus
    /// (`SHIELD_DEFENCE_RATE × CON.calcBonus`); 0 when no shield.
    pub shield_rate: f64,
    /// This actor's CON bonus (for the perfect-block roll).
    pub con_bonus: f64,
}

/// Stand-in collision radius for a siege door's extent (the gate carries no
/// `Collision` component). Added to the attacker's reach so a swing/chase
/// lands at roughly the gate face rather than its polygon centre.
pub(crate) const DOOR_COLLISION_RADIUS: f64 = 80.0;

pub(crate) fn combatant(world: &World, object_id: i32) -> Option<Combatant> {
    // A siege door is a valid attack *target* but carries no
    // Vitals/Collision/CombatStats — synthesize a stationary combatant from
    // its Position + template pDef so the shared chase/reach geometry
    // (`distance_2d`/`attack_reach`/`pawn_destination`) works uniformly.
    // `dead` = breached (0 HP); the combat-stat fields are unused for a door
    // target (`do_door_swing` reads the template directly).
    if let Some(door) = world.objects.get_component::<crate::model::door::Door>(&object_id) {
        let pos = world.objects.get_component::<Position>(&object_id)?;
        let p_def = world
            .data
            .door_data
            .get(door.door_id)
            .map(|t| t.p_def as f64)
            .unwrap_or(0.0);
        return Some(Combatant {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            heading: pos.heading,
            collision_radius: DOOR_COLLISION_RADIUS,
            dead: door.current_hp <= 0,
            p_atk: 0.0,
            p_def,
            crit_stat: 0.0,
            accuracy: 0,
            evasion: 0,
            p_atk_spd: 0,
            random_dmg: 0,
            atk_range: 0,
            shield_def: 0.0,
            shield_rate: 0.0,
            con_bonus: 1.0,
        });
    }
    // One component-shaped path for both kinds — NPC stats are memoized
    // into `CombatStats` at spawn (`npc::npc_combat_stats`), so the old
    // per-call template derivation is gone.
    let pos = world.objects.get_component::<Position>(&object_id)?;
    let collision = world.objects.get_component::<Collision>(&object_id)?;
    let vitals = world.objects.get_component::<Vitals>(&object_id)?;
    let cs = world.objects.get_component::<CombatStats>(&object_id)?;
    let (shield_def, shield_rate, con_bonus) = shield_stats(world, object_id);
    Some(Combatant {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading: pos.heading,
        collision_radius: collision.radius,
        dead: vitals.dead,
        p_atk: cs.p_atk,
        p_def: cs.p_def,
        crit_stat: cs.crit_hit,
        accuracy: cs.accuracy,
        // `EvasionRateFinalizer` ends in `Stat.defaultValue`, whose move-type
        // term is read against the creature's *live* move type — so it belongs
        // on this per-attack snapshot rather than the cached `CombatStats`
        // (Acrobatic Move 225 grants evasion only while running, and a cached
        // value would need invalidating on every start and stop of movement).
        evasion: cs.evasion + move_type_evasion_bonus(world, object_id),
        p_atk_spd: cs.p_atk_spd,
        random_dmg: cs.random_dmg,
        atk_range: cs.atk_range,
        shield_def,
        shield_rate,
        con_bonus,
    })
}

/// The defender's `DEFENCE_CRITICAL_RATE` multiplier and `_ADD` term, both at
/// Java's identity defaults when nothing grants them.
fn defence_crit_rate(world: &World, target_oid: i32) -> (f64, f64) {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;
    let Some(m) = world.objects.get_component::<StatModifiers>(&target_oid) else {
        return (1.0, 0.0);
    };
    (
        m.mul.get(&Stat::DefenceCriticalRate).copied().unwrap_or(1.0),
        m.add.get(&Stat::DefenceCriticalRateAdd).copied().unwrap_or(0.0),
    )
}

/// `Formulas.calcCritDamage` / `calcCritDamageAdd`, **autoattack branch**
/// (`skill == null`) — the crit-damage stats for one attacker/target pair at a
/// given attack position:
///
/// ```java
/// criticalDamage = getValue(CRITICAL_DAMAGE, 1) * getPositionTypeValue(CRITICAL_DAMAGE, position);
/// defenceCriticalDamage = target.getValue(DEFENCE_CRITICAL_DAMAGE, 1);
/// return 2 * criticalDamage * defenceCriticalDamage * balanceMod;   // balanceMod 1
/// ```
///
/// This is where Death Whisper 1242, Focus Attack 317, Vicious Stance 312,
/// Frenzy 176, Dance of Fire 274 and the rest of the 18 learnable
/// `CriticalDamage` skills finally land — every one was inert before, pumping
/// a stat with no reader anywhere. The position term is
/// `CriticalDamagePosition` (Focus Death 355, Focus Power 357), read *only*
/// here, matching Java.
fn crit_damage_auto(
    world: &World,
    attacker_oid: i32,
    target_oid: i32,
    position: crate::model::movement::Position,
) -> formulas::CritDamage {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;
    let attacker = world.objects.get_component::<StatModifiers>(&attacker_oid);
    let target = world.objects.get_component::<StatModifiers>(&target_oid);
    // `getValue(stat, 1)` / `getValue(stat, 0)`: the mul map defaults to 1.0
    // and the add map to 0.0, so an actor with no `StatModifiers` at all (most
    // NPCs) yields Java's stat-free `2.0` / `0.0` — what the whole port
    // hard-coded before this slice.
    let mul_of = |m: Option<&StatModifiers>, s: Stat| m.and_then(|m| m.mul.get(&s).copied()).unwrap_or(1.0);
    let add_of = |m: Option<&StatModifiers>, s: Stat| m.and_then(|m| m.add.get(&s).copied()).unwrap_or(0.0);
    let position_mul = attacker.map(|m| m.position_value(Stat::CriticalDamage, position)).unwrap_or(1.0);
    formulas::CritDamage {
        mul: 2.0 * mul_of(attacker, Stat::CriticalDamage) * position_mul * mul_of(target, Stat::DefenceCriticalDamage),
        add: add_of(attacker, Stat::CriticalDamageAdd) + add_of(target, Stat::DefenceCriticalDamageAdd),
    }
}

/// `calcCritDamage`'s **skill** branches, which take neither the position term
/// nor any additive one (`PhysicalAttack` and `calcMagicDam` apply only
/// `critMod`).
///
/// The physical half reads `PHYSICAL_SKILL_CRITICAL_DAMAGE`, which **no
/// learnable skill on this dist grants** (40 non-learnable ones do), so it
/// stays the stat-free 2.0 — the established `BLOW_RATE_DEFENCE`/`MP_BLOCK`
/// precedent of not inventing plumbing for a stat nothing reachable sets.
/// The magic half is real: Prophecy of Wind 1357 and Victories of Pa'agrio
/// 1414 grant `MAGIC_CRITICAL_DAMAGE`.
pub(crate) fn crit_damage_skill(world: &World, attacker_oid: i32, target_oid: i32, magic: bool) -> f64 {
    use crate::model::components::StatModifiers;
    use crate::model::stats::Stat;
    if !magic {
        return 2.0;
    }
    let mul_of = |oid: i32, s: Stat| {
        world.objects.get_component::<StatModifiers>(&oid).and_then(|m| m.mul.get(&s).copied()).unwrap_or(1.0)
    };
    2.0 * mul_of(attacker_oid, Stat::MagicCriticalDamage) * mul_of(target_oid, Stat::DefenceMagicCriticalDamage)
}

/// The `StatByMoveType` contribution to evasion for whoever is being snapshot
/// — Acrobatic Move 225's `+4..6 EVASION_RATE` while `RUNNING`, the only
/// non-regen use of the effect among learnable skills. Truncated to an `i32`
/// like every other evasion term on this port; zero for anyone without the
/// passive or standing still.
fn move_type_evasion_bonus(world: &World, object_id: i32) -> i32 {
    let Some(mods) = world.objects.get_component::<crate::model::components::StatModifiers>(&object_id) else {
        return 0;
    };
    let move_type = crate::game_loop::regen::move_type_of(world, object_id);
    mods.move_type_value(crate::model::stats::Stat::EvasionRate, move_type) as i32
}

/// A creature's shield block stats: `(shieldDef, shieldRate×CON, conBonus)`.
/// Only players carry an inventory/shield here; NPCs return no shield with a
/// neutral CON bonus.
pub(crate) fn shield_stats(world: &World, object_id: i32) -> (f64, f64, f64) {
    use crate::model::components::{BaseStats, StatModifiers};
    use crate::model::inventory::{Inventory, PaperdollSlot};
    use crate::model::stats::Stat;
    let Some(base) = world.objects.get_component::<BaseStats>(&object_id) else {
        return (0.0, 0.0, 1.0);
    };
    let con_bonus = world.data.stat_bonus.bonus(crate::model::stats::BaseStat::Con, base.con);
    let shield = world
        .objects
        .get_component::<Inventory>(&object_id)
        .and_then(|inv| inv.paperdoll_item(PaperdollSlot::LHand).map(|it| it.item_id))
        .and_then(|id| world.data.item_data.item_stats(id));
    // Java `Formulas.calcShldUse` bails on `!(secondaryWeaponItem instanceof
    // Armor)` *before* ever reading `Stat.SHIELD_DEFENCE`/`_RATE` — so a buff
    // like Residence Shield Defense (+225 DIFF) contributes nothing without an
    // actual shield equipped, matching the early return here.
    let Some(shield) = shield else { return (0.0, 0.0, con_bonus) };
    let (def, rate) = (shield.shield_def.unwrap_or(0) as f64, shield.shield_rate.unwrap_or(0) as f64);
    // `ShieldDefenceFinalizer`/`ShieldDefenceRateFinalizer`: `Stat.defaultValue`
    // (`base * mul + add`) over `calcWeaponPlusBaseValue` — the shield's own
    // sDef/rShld *is* that base value (no other item contributes to either
    // stat), so folding the buff mods here reproduces `getShldDef()`/
    // `getValue(SHIELD_DEFENCE_RATE)` exactly. The CON multiply on the rate
    // happens after, in `calcShldUse` itself — not baked into the stat.
    let (def, rate) = match world.objects.get_component::<StatModifiers>(&object_id) {
        Some(mods) => (
            crate::model::finalize(mods, Stat::ShieldDefence, def),
            crate::model::finalize(mods, Stat::ShieldDefenceRate, rate),
        ),
        None => (def, rate),
    };
    (def, rate * con_bonus, con_bonus)
}

/// 2D center-to-center distance between two combat actors.
fn distance_2d(a: &Combatant, b: &Combatant) -> f64 {
    (((b.x - a.x) as f64).powi(2) + ((b.y - a.y) as f64).powi(2)).sqrt()
}

/// Melee reach: attack range + both collision radii (Java adds collision
/// radii via `Util.checkIfInRange(range, a, b, true)` in the AI range gates).
fn attack_reach(a: &Combatant, b: &Combatant) -> f64 {
    a.atk_range as f64 + a.collision_radius + b.collision_radius
}

// ---------------------------------------------------------------------------
// Combat stance (`AttackStanceTaskManager`)
// ---------------------------------------------------------------------------

/// Put a player into (or refresh) combat stance — `addAttackStanceTask`.
/// Broadcasts `AutoAttackStart` only on the not-in-stance → in-stance edge.
pub(crate) fn refresh_attack_stance(world: &mut World, player_object_id: i32) {
    let now = world.tick;
    let Some(st) = world
        .objects
        .get_component_mut::<AttackState>(&player_object_id)
    else {
        return;
    };
    let was_in_stance = st.stance_until_tick > now;
    st.stance_until_tick = now + COMBAT_STANCE_TICKS;
    if !was_in_stance {
        broadcast_including_self(
            world,
            player_object_id,
            &server_packets::auto_attack_start(player_object_id),
        );
    }
}

/// The 1 s stance sweep: players whose 15 s ran out leave combat stance
/// (`AutoAttackStop` broadcast).
pub(crate) fn stance_tick(world: &mut World) {
    let now = world.tick;
    let mut expired: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &AttackState)>(|(p, st)| {
            if st.stance_until_tick != 0 && st.stance_until_tick <= now {
                expired.push(p.object_id);
            }
        });
    for object_id in expired {
        if let Some(st) = world.objects.get_component_mut::<AttackState>(&object_id) {
            st.stance_until_tick = 0;
        }
        broadcast_including_self(
            world,
            object_id,
            &server_packets::auto_attack_stop(object_id),
        );
    }
}

/// Port of `AttackStanceTaskManager.hasAttackStanceTask` — the actor is in
/// combat stance (sword drawn), i.e. within 15 s of its last swing/hit. This is
/// the state `Player.canLogout` uses to refuse a restart/logout while fighting.
pub(crate) fn has_attack_stance(world: &World, object_id: i32) -> bool {
    world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.stance_until_tick > world.tick)
}

// ---------------------------------------------------------------------------
// AttackRequest + player attack think
// ---------------------------------------------------------------------------

/// Port of `clientpackets/AttackRequest` + `Player.onActionRequest` →
/// `NpcAction`'s monster branch: clicking your already-selected monster
/// starts the attack loop. A click on something that isn't the current
/// target re-selects instead (Java falls back to `onAction`).
pub(crate) fn handle_attack_request(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(pkt) = cp::AttackRequest::read(body) else {
        return;
    };
    let Some(ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let object_id = session.player_object_id();
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };

    if world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead)
    {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }

    let _ = player;
    // `Creature.isAttackDisabled()` → `isDisabled()` → `hasBlockActions()`: a
    // stunned/asleep/paralyzed attacker is refused outright.
    if super::abnormal::is_blocked_from_actions(world, object_id) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::action_failed());
        }
        return;
    }
    // A Ctrl-click (force attack) both selects *and* engages the target. When
    // switching target the client may send only this packet — no preceding
    // `Action` — so selecting without engaging drops the "attack this next"
    // order (Java gets the select via a separate `Action` first, then
    // `onForcedAttack`; we can't rely on that ordering). While casting or
    // mid-swing, `start_attack_intent` parks the attack as the intention that
    // fires when the cast/swing ends — Java's `onForcedAttack` →
    // `setIntention(ATTACK)`, deferred to `_nextIntention` while busy.
    let current = world
        .objects
        .get_component::<crate::model::components::TargetRef>(&object_id)
        .copied()
        .unwrap_or_default()
        .0;
    if current != Some(pkt.object_id) {
        super::target::set_target(world, client_id, object_id, Some(pkt.object_id));
    }

    start_attack_intent(world, client_id, object_id, pkt.object_id, pkt.shift);
}

/// Shared entry for "the player wants to auto-attack this target" (from
/// `AttackRequest` or the second `Action` click): monsters, siege
/// towers/flags/guards and siege gates, plus flagged players. Clean players
/// need Ctrl (enforced client-side) and plain folk aren't attackable without
/// the karma system. Out of reach, a non-shift click starts a chase
/// (`player_attack_think` → `chase_target`); shift-click (`dontMove`) refuses.
pub(crate) fn start_attack_intent(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    target_object_id: i32,
    shift: bool,
) {
    let target_is_player = world
        .objects
        .has_component::<crate::model::Player>(&target_object_id);
    let target_dead = target_is_dead(world, target_object_id);
    if target_is_player {
        // `Creature.onForcedAttack` (the Ctrl/force melee path): the client only
        // sends an AttackRequest against a player when it means to — either
        // Ctrl-forced or a target it already knows is attackable (from our
        // RelationChanged). The server just refuses inside a peace zone; the
        // clean-player "needs Ctrl" gate is enforced client-side.
        if target_dead {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
        if super::zones::is_inside_peace_zone(world, object_id, target_object_id) {
            if let Some(client_id) = client_for_player(world, object_id) {
                super::helpers::send_sm_and_action_failed(
                    world,
                    client_id,
                    server_packets::sm_ids::YOU_MAY_NOT_ATTACK_THIS_TARGET_IN_A_PEACEFUL_ZONE,
                    &[],
                );
            }
            return;
        }
    } else {
        // NPCs: monsters (auto-attackable template) — plus siege towers/flags,
        // which combatants tear down during a siege, and the stationed guards,
        // which attackers (anyone but a defender) may attack. Other folk aren't
        // attackable without the karma system.
        let attackable = super::target::is_auto_attackable(world, object_id, target_object_id);
        if !attackable || target_dead {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::action_failed());
            }
            return;
        }
    }
    // Shift-click is Java's `dontMove`: refuse to walk into reach. If the
    // target is beyond melee reach, fail with "out of range" instead of
    // starting a chase (Java discards the flag; we honour it). The player is
    // stationary here — a chase leg or manual move ends before this — so the
    // current position is the right thing to range-check.
    if shift {
        if let (Some(attacker), Some(target)) =
            (combatant(world, object_id), combatant(world, target_object_id))
        {
            if distance_2d(&attacker, &target) > attack_reach(&attacker, &target) {
                super::helpers::send_sm_and_action_failed(
                    world,
                    client_id,
                    server_packets::sm_ids::YOUR_TARGET_IS_OUT_OF_RANGE,
                    &[],
                );
                return;
            }
        }
    }
    world.objects.add_components(
        &object_id,
        Intent(PlayerIntent::Attack { target_object_id }),
    );
    // Think immediately — first swing shouldn't wait for the next tick.
    player_attack_think(world, object_id);
}

/// A player's melee swing against a targeted siege door — the `DoorAction`
/// attack path. Only castle doors during an active siege take damage; the swing
/// lands immediately (per `AttackRequest`; the chase + auto-repeat loop and the
/// scheduled hit-time delay are a refinement, TODO(G24)).
/// A player's melee swing against a targeted siege gate — the in-reach half of
/// the `DoorAction` attack path, called from `player_attack_think` once the
/// chase (`chase_target`) has closed the distance. Doors don't roll
/// miss/crit/shield and have no AI, so this is a straight pAtk-vs-pDef hit
/// (front, no shot); paced by the attacker's swing period so the loop
/// auto-repeats until the gate breaches.
fn do_door_swing(world: &mut World, attacker_oid: i32, door_oid: i32) {
    // Re-check the siege gate (the loop can outlive the siege ending).
    if !super::siege::attackable_door(world, door_oid) {
        world.objects.remove_component::<Intent>(&attacker_oid);
        return;
    }
    let Some(attacker) = combatant(world, attacker_oid) else { return };
    let Some(dpos) = world.objects.get_component::<Position>(&door_oid).copied() else { return };

    // Damage: pAtk vs the door's pDef (front, no crit, no shot).
    let door_pdef = world
        .objects
        .get_component::<crate::model::door::Door>(&door_oid)
        .and_then(|d| world.data.door_data.get(d.door_id))
        .map(|t| (t.p_def as f64).max(1.0))
        .unwrap_or(1.0);
    let damage = formulas::calc_auto_attack_damage(
        attacker.p_atk,
        1.0,
        crate::model::movement::Position::Front,
        door_pdef,
        false,
        // A door swing never crits, so the crit stats are never read.
        formulas::CritDamage::default(),
        false,
    ) as i32;

    // Face the gate (Java `doAttack` `setHeading`).
    let heading = movement::calculate_heading((dpos.x - attacker.x) as f64, (dpos.y - attacker.y) as f64);
    if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    }

    // Pace the loop: hold the next swing for the attacker's attack period and
    // fire the swing-end hook (queued action), exactly like `do_auto_attack`.
    let time_atk = formulas::calculate_time_between_attacks(attacker.p_atk_spd);
    let now = world.tick;
    if let Some(st) = world.objects.get_component_mut::<AttackState>(&attacker_oid) {
        st.attack_end_tick = now + ms_to_ticks(time_atk);
    }
    world.scheduler.schedule(
        now + ms_to_ticks(time_atk),
        ScheduledTask::AttackFinish { object_id: attacker_oid },
    );

    // Broadcast the swing.
    let hit = server_packets::AttackHit {
        target_object_id: door_oid,
        damage,
        miss: false,
        crit: false,
        soulshot: false,
        ss_grade: 0,
    };
    let pkt =
        server_packets::attack(attacker_oid, std::slice::from_ref(&hit), attacker.x, attacker.y, attacker.z, dpos.x, dpos.y, dpos.z);
    broadcast_including_self(world, attacker_oid, &pkt);
    refresh_attack_stance(world, attacker_oid);

    // Apply the damage; on a breach the gate opens and nearby clients see the
    // new HP bar.
    apply_door_damage(world, door_oid, damage);
}

/// Apply damage to a siege door's HP and push its refreshed HP bar to nearby
/// clients (`StatusUpdate`); a breach opens the gate (`siege::damage_door`).
/// Shared by the melee swing (`do_door_swing`) and offensive skills
/// (`skills::effects::apply_skill_damage`).
pub(crate) fn apply_door_damage(world: &mut World, door_oid: i32, damage: i32) {
    super::siege::damage_door(world, door_oid, damage);
    let (cur_hp, max_hp) = {
        let d = world.objects.get_component::<crate::model::door::Door>(&door_oid);
        (
            d.map(|d| d.current_hp).unwrap_or(0),
            d.and_then(|d| world.data.door_data.get(d.door_id)).map(|t| t.hp_max).unwrap_or(1),
        )
    };
    if let Some(region) = world.objects.get_component::<RegionCell>(&door_oid).map(|r| r.0) {
        broadcast_near_region(
            world,
            region,
            &server_packets::status_update(
                door_oid,
                &[
                    (server_packets::status_update_type::MAX_HP, max_hp),
                    (server_packets::status_update_type::CUR_HP, cur_hp),
                ],
            ),
        );
    }
}

/// Per-tick player combat system: drive every attack/cast intent one step.
/// The sweep is presence-filtered — only intent-holders are visited.
pub(crate) fn player_combat_tick(world: &mut World) {
    let mut ids: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &Intent)>(|(p, _)| ids.push(p.object_id));
    for object_id in ids {
        match world.objects.get_component::<Intent>(&object_id).copied() {
            Some(Intent(PlayerIntent::Attack { .. })) => player_attack_think(world, object_id),
            Some(Intent(PlayerIntent::Cast { .. })) => player_cast_think(world, object_id),
            Some(Intent(PlayerIntent::Interact { .. })) => player_interact_think(world, object_id),
            None => {}
        }
    }
}

/// `PlayerAI.thinkAttack`: chase into reach, swing when ready. Runs every
/// tick per intent-holding player; chase re-pathing is throttled to the
/// follow cadence inside `chase_target`.
fn player_attack_think(world: &mut World, object_id: i32) {
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let Some(Intent(PlayerIntent::Attack { target_object_id })) =
        world.objects.get_component::<Intent>(&object_id).copied()
    else {
        return;
    };

    let dead = world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead);
    if dead || world.objects.has_component::<Casting>(&object_id) {
        return; // casting pauses the loop (Java: CAST intention), death ends it via do_die.
    }
    // Target gone or dead → drop the intent (Java `checkTargetLostOrDead` →
    // ACTIVE intention). A breached siege gate counts as dead.
    if target_is_dead(world, target_object_id) {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    }
    // Mid-swing: wait for the attack period to pass (`isAttackingNow`).
    let _ = player;
    if world
        .objects
        .get_component::<AttackState>(&object_id)
        .is_some_and(|st| st.attack_end_tick > world.tick)
    {
        return;
    }
    // A skill queued during the swing fires before the next swing (Java
    // `thinkAttack`'s queued-skill check). Normally the `AttackFinish` task
    // consumed it already this tick — this is the in-loop backstop. A cast
    // takes over the turn; anything else interleaves with the loop.
    if world
        .objects
        .has_component::<crate::model::components::QueuedAction>(&object_id)
    {
        super::helpers::run_queued_action(world, object_id);
        if world.objects.has_component::<Casting>(&object_id) {
            return;
        }
    }

    let Some(attacker) = combatant(world, object_id) else {
        return;
    };
    let Some(target) = combatant(world, target_object_id) else {
        return;
    };
    if distance_2d(&attacker, &target) > attack_reach(&attacker, &target) {
        chase_target(world, object_id, target_object_id, attacker.atk_range);
        return;
    }
    // In reach: stop the chase and swing.
    if world.objects.has_component::<Movement>(&object_id) {
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(
                world,
                object_id,
                &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }
    // A siege door takes damage through the gate path (no miss/crit/shield/AI);
    // everything else goes through the shared creature swing.
    if world.objects.has_component::<crate::model::door::Door>(&target_object_id) {
        do_door_swing(world, object_id, target_object_id);
    } else {
        do_auto_attack(world, object_id, target_object_id);
    }
}

/// `Creature.moveToPawn` for a player chasing a pawn (attack target or cast
/// target): walk to `range` + collision radii of it, re-pathed every 5 ticks
/// (Java's 500 ms `CreatureFollowTaskManager.ATTACK_FOLLOW_WEIGHT` cadence).
fn chase_target(world: &mut World, object_id: i32, target_object_id: i32, range: i32) {
    if !world.tick.is_multiple_of(5) {
        // Keep walking on the current path between re-paths.
        if world.objects.has_component::<Movement>(&object_id) {
            return;
        }
    }
    let Some(attacker) = combatant(world, object_id) else {
        return;
    };
    let Some(target) = combatant(world, target_object_id) else {
        return;
    };
    let reach = range as f64 + attacker.collision_radius + target.collision_radius;
    let Some((dest_x, dest_y, dest_z, heading)) = pawn_destination(&attacker, &target, reach)
    else {
        return;
    };

    let (speed, start) = {
        let Some(speeds) = world.objects.get_component::<Speeds>(&object_id) else {
            return;
        };
        let pos = world
            .objects
            .get_component::<Position>(&object_id)
            .copied()
            .unwrap_or(Position {
                x: 0,
                y: 0,
                z: 0,
                heading: 0,
            });
        (speeds.move_speed(), (pos.x, pos.y, pos.z))
    };
    if speed <= 0.0 {
        return;
    }
    let distance =
        (((dest_x - start.0) as f64).powi(2) + ((dest_y - start.1) as f64).powi(2)).sqrt();
    let total_ticks = ((10.0 * distance / speed).round() as u64).max(1);
    let start_tick = world.tick;
    if let Some(pos) = world.objects.get_component_mut::<Position>(&object_id) {
        pos.heading = heading;
    }
    world.objects.add_components(
        &object_id,
        Movement(MoveData {
            start_x: start.0,
            start_y: start.1,
            start_z: start.2,
            dest_x,
            dest_y,
            dest_z,
            start_tick,
            total_ticks,
            geo_path: None,
        }),
    );
    let pkt = server_packets::move_to_pawn(
        object_id,
        target_object_id,
        reach as i32,
        start.0,
        start.1,
        start.2,
        target.x,
        target.y,
        target.z,
    );
    broadcast_including_self(world, object_id, &pkt);
}

/// The point at `reach` distance from the target on the mover→target line
/// (`Creature.moveToLocation`'s `offset` handling), plus the facing heading.
/// `None` when already inside reach.
pub(crate) fn pawn_destination(
    mover: &Combatant,
    target: &Combatant,
    reach: f64,
) -> Option<(i32, i32, i32, i32)> {
    let dx = (target.x - mover.x) as f64;
    let dy = (target.y - mover.y) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= reach {
        return None;
    }
    // Land 5 units inside reach (Java `moveToLocation`: "due to rounding
    // error, we have to move a bit closer to be in range") — aiming at the
    // exact boundary can round to a point just outside it, wedging the
    // chase in an arrive/re-path loop that never satisfies the range gate.
    let frac = (distance - (reach - 5.0).max(0.0)) / distance;
    let dest_x = mover.x + (dx * frac).round() as i32;
    let dest_y = mover.y + (dy * frac).round() as i32;
    let heading = movement::calculate_heading(dx, dy);
    Some((dest_x, dest_y, target.z, heading))
}

/// `PlayerAI.thinkCast` for the walk-to-cast leg: chase into the skill's cast
/// range (`maybeMoveToPawn(target, getMagicalAttackRange(skill))`), then hand
/// back to `use_magic_on` for a fully re-validated cast (LOS from the arrival
/// spot, MP, reuse) at the target snapshotted in the intent — Java casts at
/// the intention's cast target even if the player re-targeted mid-walk.
pub(crate) fn player_cast_think(world: &mut World, object_id: i32) {
    let Some(Intent(PlayerIntent::Cast {
        skill_id,
        ctrl,
        shift,
        target_object_id,
    })) = world.objects.get_component::<Intent>(&object_id).copied()
    else {
        return;
    };
    if world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead)
        || world.objects.has_component::<Casting>(&object_id)
    {
        return;
    }
    // `checkTargetLost`: a dead or vanished target drops the intention. A
    // siege door carries no `Vitals` (its HP lives on the `Door` component), so
    // use the door-aware `target_is_dead` — otherwise `vitals_of` reads `None`
    // for a door and the walk-to-cast is abandoned before it starts.
    if target_is_dead(world, target_object_id) {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    }
    let cast_range = world
        .objects
        .get_component::<crate::model::components::SkillBook>(&object_id)
        .and_then(|book| book.0.get(&skill_id))
        .and_then(|&level| world.data.skill_data.get(skill_id, level))
        .map(|s| s.cast_range);
    let Some(cast_range) = cast_range else {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    };
    let Some(caster_pos) = world.objects.get_component::<Position>(&object_id).copied() else {
        return;
    };
    if !super::skills::cast::in_cast_range(
        world,
        object_id,
        &caster_pos,
        target_object_id,
        cast_range,
        false,
    ) {
        chase_target(world, object_id, target_object_id, cast_range);
        return;
    }
    // Arrived: consume the intention, stop the chase leg (`clientStopMoving`
    // in `thinkCast`), and cast.
    world.objects.remove_component::<Intent>(&object_id);
    if world.objects.has_component::<Movement>(&object_id) {
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(
                world,
                object_id,
                &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }
    let Some(client_id) = client_for_player(world, object_id) else {
        return;
    };
    super::skills::cast::use_magic_on(
        world,
        client_id,
        object_id,
        skill_id,
        ctrl,
        shift,
        Some(target_object_id),
    );
}

/// Shared entry for "the player wants to talk to this NPC but
/// `Npc.canInteract` failed" (out of `target::INTERACTION_DISTANCE`): Java's
/// `NpcAction` sets `AI_INTENTION_INTERACT`, which `CreatureAI.onIntentionInteract`
/// turns into an immediate `moveToPawn` — mirrored here by setting the intent
/// and thinking it once synchronously, same as `start_attack_intent`.
pub(crate) fn start_interact_intent(world: &mut World, object_id: i32, target_object_id: i32) {
    world.objects.add_components(
        &object_id,
        Intent(PlayerIntent::Interact { target_object_id }),
    );
    player_interact_think(world, object_id);
}

/// `PlayerAI.thinkInteract`: chase to `maybeMoveToPawn(target, 36)` range,
/// then hand back to `interact_with_npc` for a fully re-validated interaction
/// — Java's `Player.doInteract` re-dispatches `target.onAction(this)`, which
/// re-runs the same click handler now that `canInteract` (250 units) passes
/// comfortably inside this 36-unit arrival range.
fn player_interact_think(world: &mut World, object_id: i32) {
    let Some(Intent(PlayerIntent::Interact { target_object_id })) =
        world.objects.get_component::<Intent>(&object_id).copied()
    else {
        return;
    };
    if world
        .objects
        .get_component::<Vitals>(&object_id)
        .is_none_or(|v| v.dead)
        || world.objects.has_component::<Casting>(&object_id)
    {
        return;
    }
    let Some(attacker) = combatant(world, object_id) else {
        return;
    };
    // Target gone → drop the intention (Java `checkTargetLost`).
    let Some(target) = combatant(world, target_object_id) else {
        world.objects.remove_component::<Intent>(&object_id);
        return;
    };
    const INTERACT_APPROACH_RANGE: i32 = 36;
    let reach =
        INTERACT_APPROACH_RANGE as f64 + attacker.collision_radius + target.collision_radius;
    if distance_2d(&attacker, &target) > reach {
        chase_target(world, object_id, target_object_id, INTERACT_APPROACH_RANGE);
        return;
    }
    // Arrived: stop the chase leg and re-run the interact click.
    world.objects.remove_component::<Intent>(&object_id);
    if world.objects.has_component::<Movement>(&object_id) {
        world.objects.remove_component::<Movement>(&object_id);
        if let Some(pos) = world.objects.get_component::<Position>(&object_id).copied() {
            broadcast_including_self(
                world,
                object_id,
                &server_packets::stop_move(object_id, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }
    let Some(client_id) = client_for_player(world, object_id) else {
        return;
    };
    // Re-entry after walking into interaction range: only the chat/interact
    // branch reaches here (attackable targets chase via the attack loop, not
    // this walk-to-interact path), so the dontMove flag is moot.
    super::target::interact_with_npc(world, client_id, object_id, target_object_id, false);
}

// ---------------------------------------------------------------------------
// The swing (`Creature.doAutoAttack` → scheduled hit)
// ---------------------------------------------------------------------------

/// Port of `Creature.doAutoAttack` + `generateAttackTargetData`/`generateHit`
/// for the melee single-hit case, shared by players and NPCs: roll the hit
/// now, broadcast `Attack`, land it at `timeToHit` via the scheduler.
pub(crate) fn do_auto_attack(world: &mut World, attacker_oid: i32, target_oid: i32) {
    let Some(attacker) = combatant(world, attacker_oid) else {
        return;
    };
    let Some(target) = combatant(world, target_oid) else {
        return;
    };
    if attacker.dead || target.dead {
        return;
    }

    // GeoData LOS check (`doAutoAttack`).
    if !world.geo.can_see_target(
        attacker.x, attacker.y, attacker.z, target.x, target.y, target.z,
    ) {
        if let Some(client_id) = client_for_player(world, attacker_oid) {
            super::helpers::send_sm_and_action_failed(
                world,
                client_id,
                sm_ids::CANNOT_SEE_TARGET,
                &[],
            );
        }
        world.objects.remove_component::<Intent>(&attacker_oid);
        return;
    }

    // Ranged weapons run Java's `doAttack` BOW/CROSSBOW gate before the swing:
    // reload delay, ammunition, MP. Only players carry ammunition — an NPC
    // archer shoots freely, as in Java (the whole block is `isPlayer()`-gated
    // apart from the reuse timer).
    let weapon_type = super::ranged::equipped_weapon_type(world, attacker_oid).unwrap_or_default();
    if super::ranged::is_ranged(weapon_type) && world.objects.has_component::<crate::model::Player>(&attacker_oid) {
        if let Err(why) = super::ranged::prepare_ranged_shot(world, attacker_oid, weapon_type) {
            super::ranged::report_refusal(world, attacker_oid, why);
            return;
        }
    }

    let time_atk = formulas::calculate_time_between_attacks(attacker.p_atk_spd);
    // Two-handed timing needs the weapon's body part — item kinds are parsed
    // (G5), so check the equipped right hand for SLOT_LR_HAND.
    let two_handed = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
        .is_some_and(|inv| {
            let rhand = inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand);
            rhand != 0
                && world
                    .data
                    .item_data
                    .get(rhand)
                    .is_some_and(|t| t.body_part == crate::data::item_data::SLOT_LR_HAND)
        });
    let time_to_hit = formulas::calculate_time_to_hit(time_atk, two_handed);

    // Face the target (Java `setHeading(calculateHeadingFrom(...))`).
    let heading = movement::calculate_heading(
        (target.x - attacker.x) as f64,
        (target.y - attacker.y) as f64,
    );
    if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    } else if let Some(pos) = world.objects.get_component_mut::<Position>(&attacker_oid) {
        pos.heading = heading;
    }

    // Java `doAttack`: "Always try to charge soulshots" before the swing —
    // the auto-use toggle re-charges the physical shot if the player set one.
    if is_npc_oid(attacker_oid) {
        // A **summon** charges from its owner's Beast shots (Java
        // `Summon.rechargeShots`); a plain monster has no owner and no-ops.
        super::servitor::recharge_shots(world, attacker_oid, true);
    } else if !world
        .objects
        .get_component::<crate::model::Player>(&attacker_oid)
        .is_some_and(|p| p.is_charged_shot(crate::model::ShotType::Soulshots))
    {
        super::items::recharge_shots(world, attacker_oid, true, false);
    }

    // Roll the hit (`generateHit`): miss → everything else skipped.
    let position = get_position(attacker.x, attacker.y, target.x, target.y, target.heading);
    let condition = world
        .data
        .hit_condition_bonus
        .condition_bonus(attacker.z, target.z, position);
    let miss_roll = world.roll(1000);
    let miss = formulas::calc_hit_miss(attacker.accuracy, target.evasion, condition, miss_roll);
    let (crit, damage, ss, shield) = if miss {
        (false, 0, false, formulas::SHIELD_NONE)
    } else {
        // `generateHit`: a charged soulshot is spent on a non-miss and doubles
        // the swing (`unchargeShot(SOULSHOTS)` → `ss` into `calcAutoAttackDamage`).
        let ss = if is_npc_oid(attacker_oid) {
            super::servitor::uncharge_soulshot(world, attacker_oid)
        } else {
            world
                .objects
                .get_component_mut::<crate::model::Player>(&attacker_oid)
                .is_some_and(|p| p.uncharge_shot(crate::model::ShotType::Soulshots))
        };
        // Shield block (`calcShldUse`): a back attack (attacker outside the 120°
        // front arc) can't be blocked; melee only until bows land (G20).
        let from_behind = matches!(position, crate::model::movement::Position::Back);
        let shield = formulas::calc_shield_use(
            target.shield_rate,
            target.con_bonus,
            false,
            from_behind,
            world.roll(100),
            world.roll(100),
        );
        let crit_roll = world.roll(100);
        // `DEFENCE_CRITICAL_RATE`/`_ADD` are read off the **defender** — Light
        // Armor Mastery 233 makes its wearer harder to crit, it does not make
        // its wearer crit less.
        let (def_crit_mul, def_crit_add) = defence_crit_rate(world, target_oid);
        let crit = formulas::calc_auto_attack_crit(
            attacker.crit_stat,
            def_crit_mul,
            def_crit_add,
            position,
            attacker.z,
            target.z,
            crit_roll,
        );
        let r = attacker.random_dmg;
        let rand_roll = if r > 0 { world.roll(2 * r + 1) - r } else { 0 };
        // A normal block adds the shield's defence to pDef; a perfect block
        // reduces the hit to 1 (Java `SHIELD_DEFENSE_PERFECT_BLOCK`).
        let eff_pdef = target.p_def + if shield == formulas::SHIELD_SUCCEED { target.shield_def } else { 0.0 };
        let dmg = if shield == formulas::SHIELD_PERFECT {
            1.0
        } else {
            formulas::calc_auto_attack_damage(
                attacker.p_atk,
                formulas::random_damage_multiplier(rand_roll),
                position,
                eff_pdef,
                crit,
                crit_damage_auto(world, attacker_oid, target_oid, position),
                ss,
            )
        };
        (crit, dmg as i32, ss, shield)
    };
    // Notify a shielding player their block landed (Interlude has only the
    // "succeeded" message; the perfect block reuses it).
    if shield != formulas::SHIELD_NONE {
        if let Some(cid) = client_for_player(world, target_oid) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(server_packets::system_message_with(sm_ids::SHIELD_DEFENSE_SUCCEEDED, &[]));
            }
        }
    }
    // `Hit.getGrade()`: the equipped weapon's crystal-grade ordinal, only when
    // a soulshot was actually spent.
    let ss_grade = if ss {
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
            .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
            .and_then(|w| world.data.item_data.get(w))
            .map(|t| t.crystal_type.level())
            .unwrap_or(0)
    } else {
        0
    };

    // `generateAttackTargetData` — one swing can carry several hits.
    //
    // * A **dual** weapon strikes the main target twice, each at half damage
    //   (Java's `halfDamage` in `generateHit`).
    // * A **polearm sweep** adds one simple hit per extra target, gated on
    //   `ATTACK_COUNT_MAX > 1` (Polearm Mastery 216 sets it to 5). Extra
    //   targets must be alive, auto-attackable, inside the weapon's attack
    //   radius, and within its attack angle of the attacker's heading.
    let weapon_id = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&attacker_oid)
        .map(|inv| inv.paperdoll_item_id(crate::model::inventory::PaperdollSlot::RHand))
        .unwrap_or(0);
    let is_dual = matches!(
        world.data.item_data.weapon_type(weapon_id),
        crate::data::item_data::WeaponType::Dual
            | crate::data::item_data::WeaponType::DualBlunt
            | crate::data::item_data::WeaponType::DualDagger
            | crate::data::item_data::WeaponType::DualFist
    );

    let mut hits: Vec<server_packets::AttackHit> = Vec::new();
    let main_damage = if is_dual { damage / 2 } else { damage };
    hits.push(server_packets::AttackHit {
        target_object_id: target_oid,
        damage: main_damage,
        miss,
        crit,
        soulshot: ss,
        ss_grade,
    });
    if is_dual {
        // Java rolls the second hit independently; this port reuses the first
        // roll's outcome for it, so a dual swing is two halves of one roll
        // rather than two separate rolls. TODO(G20): independent second roll
        // once the roll is factored out of `do_auto_attack`.
        hits.push(server_packets::AttackHit {
            target_object_id: target_oid,
            damage: main_damage,
            miss,
            crit,
            soulshot: ss,
            ss_grade,
        });
    }
    for extra in sweep_targets(world, attacker_oid, target_oid, weapon_id) {
        hits.push(server_packets::AttackHit {
            target_object_id: extra,
            damage,
            miss,
            crit,
            soulshot: ss,
            ss_grade,
        });
    }

    let now = world.tick;
    if let Some(st) = world
        .objects
        .get_component_mut::<AttackState>(&attacker_oid)
    {
        st.attack_end_tick = now + ms_to_ticks(time_atk);
    }
    for hit in &hits {
        world.scheduler.schedule(
            now + ms_to_ticks(time_to_hit),
            ScheduledTask::AttackHit {
                attacker: attacker_oid,
                target: hit.target_object_id,
                damage: hit.damage,
                miss: hit.miss,
                crit: hit.crit,
            },
        );
    }
    // Swing-end hook (Java's `EVT_READY_TO_ACT` schedule in `doAttack`).
    // Players: fire the action the swing held back, if any. NPCs: re-run the AI
    // think at the swing's end so it re-swings at the weapon's attack rate
    // instead of stalling until the coarse 1 s `AttackableAI` tick — the fix for
    // the "attack, pause a second, attack again" stutter (siege guards & mobs).
    if !is_npc_oid(attacker_oid) {
        world.scheduler.schedule(
            now + ms_to_ticks(time_atk),
            ScheduledTask::AttackFinish {
                object_id: attacker_oid,
            },
        );
    } else {
        world.scheduler.schedule(
            now + ms_to_ticks(time_atk),
            ScheduledTask::NpcAttackReady {
                npc_oid: attacker_oid,
            },
        );
    }

    // Broadcast the swing (all of its hits in one packet).
    let pkt = server_packets::attack(
        attacker_oid,
        &hits,
        attacker.x,
        attacker.y,
        attacker.z,
        target.x,
        target.y,
        target.z,
    );
    if is_npc_oid(attacker_oid) {
        let Some(region) = world
            .objects
            .get_component::<RegionCell>(&attacker_oid)
            .map(|r| r.0)
        else {
            return;
        };
        broadcast_near_region(world, region, &pkt);
    } else {
        broadcast_including_self(world, attacker_oid, &pkt);
    }

    // `Creature.doAttack` tail: outside a PVP zone, and not self-targeting, the
    // attacker enters stance and flags against a player target.
    //
    // Java hangs both off `getActingPlayer()`, which is **not** the same as
    // "the attacker is a player": a summon's acting player is its owner, so a
    // pet's swing flags and stances the *owner*. This block used to live in the
    // player-only `else` above, so a summon attacking a player flagged nobody —
    // a player could attack through their pet and never go purple, leaving the
    // victim unable to retaliate without taking the karma.
    //
    // A plain monster resolves to itself, is not a player, and so still flags
    // nobody.
    let actor = super::pvp::acting_player(world, attacker_oid);
    if world.objects.has_component::<crate::model::Player>(&actor) {
        refresh_attack_stance(world, actor);
        super::pvp::update_pvp_status_target(world, actor, target_oid);
    }
}

/// The polearm sweep's extra targets — Java's `ATTACK_COUNT_MAX` loop in
/// `generateAttackTargetData`.
///
/// Returns nothing at all unless the attacker's `ATTACK_COUNT_MAX` exceeds 1,
/// which on this dist means Polearm Mastery 216 (`HitNumber` 5). Candidates
/// must be alive, auto-attackable, inside the **weapon's** attack radius (66
/// for a polearm, 40 for most others) and within its attack angle of the
/// attacker's heading (120° for both — a weapon that declares no
/// `damage_range` falls back to angle 0, which selects nothing).
///
/// Java also skips the sweep when `PHYSICAL_POLEARM_TARGET_SINGLE > 0`; no
/// ported effect sets that stat, so the check is omitted. TODO(G20).
fn sweep_targets(world: &World, attacker_oid: i32, main_target: i32, weapon_id: i32) -> Vec<i32> {
    let max_targets = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&attacker_oid)
        .and_then(|m| m.add.get(&crate::model::stats::Stat::AttackCountMax).copied())
        .unwrap_or(0.0) as i32
        + 1; // the base 1 target
    if max_targets <= 1 {
        return Vec::new();
    }
    let Some(template) = world.data.item_data.get(weapon_id) else { return Vec::new() };
    let (radius, angle) = (template.attack_radius, template.attack_angle);
    if radius <= 0 || angle <= 0 {
        return Vec::new();
    }
    let Some(origin) = world.objects.get_component::<Position>(&attacker_oid).copied() else {
        return Vec::new();
    };
    let Some(region) = world.objects.get_component::<RegionCell>(&attacker_oid).map(|r| r.0) else {
        return Vec::new();
    };
    let heading_deg = origin.heading as f64 * 360.0 / 65536.0;

    let mut out = Vec::new();
    let mut room = max_targets - 1; // the main target already used one
    for candidate in world.npcs_visible_from(region) {
        if room <= 0 {
            break;
        }
        if candidate == main_target || candidate == attacker_oid {
            continue;
        }
        if vitals_of(world, candidate).map(|v| v.dead).unwrap_or(true) {
            continue;
        }
        // Only auto-attackable creatures are swept up (Java `isAutoAttackable`).
        let attackable = world
            .objects
            .get_component::<crate::model::npc::Npc>(&candidate)
            .and_then(|n| n.template(world))
            .is_some_and(|t| t.is_auto_attackable());
        if !attackable {
            continue;
        }
        let Some(pos) = world.objects.get_component::<Position>(&candidate).copied() else { continue };
        let (dx, dy) = ((pos.x - origin.x) as f64, (pos.y - origin.y) as f64);
        if (dx * dx + dy * dy).sqrt() > radius as f64 {
            continue;
        }
        // `Math.abs(calculateDirectionTo(obj) - headingAngle) > angle` → skip.
        let direction = dy.atan2(dx).to_degrees();
        let mut delta = (direction - heading_deg).abs() % 360.0;
        if delta > 180.0 {
            delta = 360.0 - delta;
        }
        if delta > angle as f64 {
            continue;
        }
        out.push(candidate);
        room -= 1;
    }
    out
}

/// `ScheduledTask::AttackHit` — `onHitTimeNotDual` + `onHitTarget`: the swing
/// lands (or misses).
pub(crate) fn handle_attack_hit(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: i32,
    miss: bool,
    crit: bool,
) {
    // Attacker died mid-swing → EVT_CANCEL (nothing lands).
    let attacker_alive = vitals_of(world, attacker).map(|v| !v.dead);
    if attacker_alive != Some(true) {
        return;
    }
    // Target dead/gone → skipped (Java's per-hit target check).
    let target_alive = vitals_of(world, target).map(|v| !v.dead);
    if target_alive != Some(true) {
        return;
    }

    if miss {
        // `sendDamageMessage(miss)` + `notifyAttackAvoid`.
        if let Some(client_id) = client_for_player(world, attacker) {
            let name = world
                .objects
                .get_component::<crate::model::Player>(&attacker)
                .expect("player")
                .name
                .clone();
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::C1_S_ATTACK_WENT_ASTRAY,
                    &[SmParam::PlayerName(name)],
                ));
            }
        }
        if let Some(client_id) = client_for_player(world, target) {
            let attacker_name = attacker_display_name(world, attacker);
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::system_message_with(
                    sm_ids::C1_HAS_EVADED_C2_S_ATTACK,
                    &[
                        SmParam::PlayerName(
                            world
                                .objects
                                .get_component::<crate::model::Player>(&target)
                                .expect("player")
                                .name
                                .clone(),
                        ),
                        attacker_name,
                    ],
                ));
            }
            refresh_attack_stance(world, target);
        }
        return;
    }

    // Crit + damage messages (`Player.sendDamageMessage`).
    if let Some(client_id) = client_for_player(world, attacker) {
        let attacker_name = world
            .objects
            .get_component::<crate::model::Player>(&attacker)
            .expect("player")
            .name
            .clone();
        let target_name = target_display_param(world, target);
        // `Player.sendDamageMessage`: an invul / HP-blocked target silently
        // absorbs the hit — the attacker is told "The attack has been blocked",
        // NOT the damage line (which would falsely claim damage that never
        // lands). Matches Java's `target.isInvul()` branch ahead of the
        // `C1_HAS_INFLICTED` line.
        let target_blocked = world
            .objects
            .get_component::<crate::model::components::AdminFlags>(&target)
            .is_some_and(|f| f.invul);
        if let Some(cs) = world.clients.get(&client_id) {
            if crit {
                cs.send(server_packets::system_message_with(
                    sm_ids::C1_LANDED_A_CRITICAL_HIT,
                    &[SmParam::PlayerName(attacker_name.clone())],
                ));
            }
            if target_blocked {
                cs.send(server_packets::system_message_with(
                    sm_ids::THE_ATTACK_HAS_BEEN_BLOCKED,
                    &[],
                ));
            } else {
                cs.send(server_packets::system_message_with(
                    sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
                    &[
                        SmParam::PlayerName(attacker_name),
                        target_name,
                        SmParam::Int(damage),
                        // `sendDamageMessage`'s `addPopup(target, attacker,
                        // -damage)` — the on-screen floating damage number.
                        SmParam::Popup { target, attacker, damage: -damage },
                    ],
                ));
            }
        }
    }

    apply_physical_damage(world, attacker, target, damage as f64, false);

    // Java `OnCreatureDamageDealt` — the event `TriggerSkillByAttack` listens
    // on. Fired after the damage lands, and only for a *normal* attack (this
    // is the autoattack path; `allowSkillAttack` defaults to false, so skill
    // hits would be rejected anyway).
    super::skills::effects::fire_attack_triggers(world, attacker, target, damage, crit);
}

/// How an attacker shows up in the *victim's* damage messages ($c2).
fn attacker_display_name(world: &World, attacker: i32) -> SmParam {
    if let Some(p) = world
        .objects
        .get_component::<crate::model::Player>(&attacker)
    {
        SmParam::PlayerName(p.name.clone())
    } else if let Some(t) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&attacker)
        .and_then(|n| n.template(world))
    {
        SmParam::NpcName(t.id)
    } else {
        SmParam::Text(String::new())
    }
}

/// How a target shows up in the *attacker's* damage messages ($c2).
fn target_display_param(world: &World, target: i32) -> SmParam {
    attacker_display_name(world, target)
}

/// Damage application shared by auto-attacks (and reusable by future physical
/// skills): route to the right victim kind, waking NPC AI / breaking player
/// casts / killing at 0 HP. `is_dot` is Java `CreatureStatus.reduceHp`'s
/// `isDOT` — the one exemption from `HP_BLOCK` (Celestial Shield, …) besides
/// a skill's own HP cost, which never reaches this shared path at all.
pub(crate) fn apply_physical_damage(world: &mut World, attacker: i32, target: i32, damage: f64, is_dot: bool) {
    if !is_dot && super::abnormal::is_hp_blocked(world, target) {
        return;
    }
    if is_npc_oid(target) {
        npc_receive_damage(world, target, attacker, damage);
    } else {
        // `Creature.reduceCurrentHp`: `if (isPlayer() && isFakeDeath() &&
        // Config.FAKE_DEATH_DAMAGE_STAND && amount > 0) stopFakeDeath(true)`.
        // `FakeDeathDamageStand = True` on this dist, so taking a hit while
        // playing dead stands you back up — otherwise a rogue could feign
        // death and soak a whole fight from the floor.
        if damage > 0.0 {
            super::skills::effects::break_fake_death_on_damage(world, target);
        }
        player_receive_damage(world, target, attacker, damage);
    }
}

/// `Attackable.reduceCurrentHp` → `addDamage`/`addDamageHate` + the
/// `onEvtAttacked` AI reaction, then the HP cut and `doDie`.
pub(crate) fn npc_receive_damage(world: &mut World, npc_oid: i32, attacker_oid: i32, damage: f64) {
    if world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    let level = match world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    {
        Some(npc) => npc.template(world).map(|t| t.level).unwrap_or(1),
        None => return,
    };
    let now = world.tick;

    let mut became_running = false;
    let mut died = false;
    let (cur_hp, max_hp) = {
        let Some((mut aggro, mut ai, mut vitals, mut speeds)) =
            world
                .objects
                .get_many_mut::<(&mut AggroList, &mut NpcAi, &mut Vitals, &mut Speeds)>(&npc_oid)
        else {
            return;
        };
        // `addDamage`: hate = damage·100 / (level + 7); `onEvtAttacked`:
        // reset the calm-after-spawn counter, arm the attack timeout, run.
        let hate = damage * 100.0 / (level + 7) as f64;
        let entry = aggro.0.entry(attacker_oid).or_default();
        entry.damage += damage;
        entry.hate += hate;
        if ai.global_aggro < 0 {
            ai.global_aggro = 0;
        }
        ai.attack_timeout_tick = now + ATTACK_TIMEOUT_TICKS;
        if !speeds.running {
            speeds.running = true;
            became_running = true;
        }
        ai.intention = NpcIntention::Attack;

        vitals.cur_hp -= damage;
        if vitals.cur_hp <= 0.0 {
            vitals.cur_hp = 0.0;
            died = true;
        }
        (vitals.cur_hp as i32, vitals.max_hp)
    };
    // `Attackable.reduceCurrentHp`'s raid-curse check, and Java's own comment
    // is the reason it sits **here** rather than before the damage block:
    // "In retail you deal damage to raid before curse." The hit that earns the
    // curse still lands.
    super::raid_curse::on_raid_attacked(world, npc_oid, attacker_oid);

    // Quest `onAttack` (Java `addAttackId` scripts, notified from
    // `Attackable.reduceCurrentHp` before any death processing). Only
    // players drive quests.
    if world
        .objects
        .has_component::<crate::model::Player>(&attacker_oid)
    {
        let npc_id = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .map(|n| n.npc_id)
            .unwrap_or(0);
        super::quests::notify_attack(world, attacker_oid, npc_oid, npc_id);
    }
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };

    if became_running {
        broadcast_near_region(
            world,
            region,
            &server_packets::change_move_type(npc_oid, true),
        );
    }
    if died {
        super::death::npc_do_die(world, npc_oid, attacker_oid);
        return;
    }
    // `broadcastStatusUpdate` — the HP bar for everyone targeting it.
    broadcast_near_region(
        world,
        region,
        &server_packets::status_update(
            npc_oid,
            &[
                (server_packets::status_update_type::MAX_HP, max_hp),
                (server_packets::status_update_type::CUR_HP, cur_hp),
            ],
        ),
    );
}

/// `AttackableAI.MAX_ATTACK_TIMEOUT`: 1200 game ticks (120 s) without combat
/// activity against the target ends the chase.
pub(crate) const ATTACK_TIMEOUT_TICKS: u64 = 1200;

/// `AI.notifyEvent(EVT_ATTACKED, attacker)` → `AttackableAI.onEvtAttacked`
/// with no HP change: the aggro/wake half of `npc_receive_damage`, used by
/// non-damaging offensive effects (Spoil). `addDamageHate(attacker, 0, 1)`
/// (hate += 1), reset the calm-after-spawn counter, arm the timeout, run, and
/// switch to the attack intention. No StatusUpdate — HP didn't move.
pub(crate) fn npc_wake_on_attacked(world: &mut World, npc_oid: i32, attacker_oid: i32) {
    if world.objects.get_component::<Vitals>(&npc_oid).is_none_or(|v| v.dead) {
        return;
    }
    // `Attackable.addDamageHate` → `MinionList.onAssist`: hitting one member of
    // a pack pulls in the leader and the rest of the escort.
    super::minions::on_assist(world, npc_oid, attacker_oid);
    let now = world.tick;
    let became_running = {
        let Some((mut aggro, mut ai, mut speeds)) =
            world.objects.get_many_mut::<(&mut AggroList, &mut NpcAi, &mut Speeds)>(&npc_oid)
        else {
            return;
        };
        aggro.0.entry(attacker_oid).or_default().hate += 1.0;
        if ai.global_aggro < 0 {
            ai.global_aggro = 0;
        }
        ai.attack_timeout_tick = now + ATTACK_TIMEOUT_TICKS;
        ai.intention = NpcIntention::Attack;
        let was_running = speeds.running;
        speeds.running = true;
        !was_running
    };
    if became_running {
        if let Some(region) = world.objects.get_component::<RegionCell>(&npc_oid).map(|r| r.0) {
            broadcast_near_region(world, region, &server_packets::change_move_type(npc_oid, true));
        }
    }
}

/// `PlayerStatus.reduceHp` for a physical hit: CP absorbs first only against
/// playable attackers (mobs bite straight into HP), casts can break
/// (`Formulas.calcAtkBreak`), 0 HP → `doDie`.
pub(crate) fn player_receive_damage(
    world: &mut World,
    player_oid: i32,
    attacker_oid: i32,
    damage: f64,
) {
    // A duel is consequence-free: the losing blow stops at 1 HP and ends the
    // duel instead of killing (Java caps it in the duel damage path, which is
    // why a duel loser stands back up rather than dying).
    if super::duel::duel_lethal_guard(world, attacker_oid, player_oid, damage) {
        return;
    }
    let attacker_is_playable = !is_npc_oid(attacker_oid);
    // GM `//invul`/`//undying` (Java `isInvul`/`isUndying`): invul ignores the
    // hit entirely; undying lets damage apply but floors HP at 1.
    let flags = world
        .objects
        .get_component::<crate::model::components::AdminFlags>(&player_oid)
        .copied()
        .unwrap_or_default();
    if flags.invul {
        return;
    }
    let mut died = false;
    let (cp_after, hp_after) = {
        let Some((mut vitals, mut pvitals)) = world
            .objects
            .get_many_mut::<(&mut Vitals, &mut PlayerVitals)>(&player_oid)
        else {
            return;
        };
        if vitals.dead {
            return;
        }
        let mut remaining = damage;
        if attacker_is_playable {
            let cp_absorb = remaining.min(pvitals.cur_cp);
            pvitals.cur_cp -= cp_absorb;
            remaining -= cp_absorb;
        }
        vitals.cur_hp -= remaining;
        if vitals.cur_hp <= 0.0 {
            if flags.undying {
                vitals.cur_hp = 1.0;
            } else {
                vitals.cur_hp = 0.0;
                died = true;
            }
        }
        (pvitals.cur_cp as i32, vitals.cur_hp as i32)
    };

    // Victim-side damage message + stance.
    if let Some(client_id) = client_for_player(world, player_oid) {
        let attacker_name = attacker_display_name(world, attacker_oid);
        let victim_name = world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
            .expect("player")
            .name
            .clone();
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2,
                &[
                    SmParam::PlayerName(victim_name),
                    attacker_name,
                    SmParam::Int(damage as i32),
                ],
            ));
        }
    }
    if !died {
        refresh_attack_stance(world, player_oid);
    }

    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (server_packets::status_update_type::CUR_CP, cp_after),
                (server_packets::status_update_type::CUR_HP, hp_after),
            ],
        ),
    );
    super::party::notify_party_vitals(world, player_oid);

    if died {
        super::death::player_do_die(world, player_oid, attacker_oid);
        return;
    }

    // Cast break on hit (`Formulas.calcAtkBreak`, same roll as the magic
    // damage path).
    let breakable = world
        .objects
        .get_component::<Casting>(&player_oid)
        .is_some_and(|c| !c.0.launched);
    if breakable {
        let men_bonus = {
            let men = world
                .objects
                .get_component::<crate::model::components::BaseStats>(&player_oid)
                .map(|b| b.men)
                .unwrap_or(0);
            world.data.stat_bonus.bonus(BaseStat::Men, men)
        };
        // `Stat.ATTACK_CANCEL` modifiers (Concentration etc.) lower the rate.
        let (cancel_add, cancel_mul) = world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&player_oid)
            .map(|m| {
                use crate::model::stats::Stat::AttackCancel;
                (m.add.get(&AttackCancel).copied().unwrap_or(0.0), m.mul.get(&AttackCancel).copied().unwrap_or(1.0))
            })
            .unwrap_or((0.0, 1.0));
        let break_roll = world.roll(100);
        if formulas::calc_atk_break(damage, men_bonus, break_roll, cancel_add, cancel_mul) {
            break_cast(world, player_oid);
            maybe_distance_too_far(world, player_oid);
        }
    }
}
