use super::attribute_mod;
use super::broadcast_change_wait_type;
use super::calc_general_trait_bonus;
use super::expire_buffs_where;
use super::player_or_npc_level;
use crate::game_loop::net::broadcast;
use crate::game_loop::npc::ai::force_attack_target;
use crate::game_loop::space::position;
use crate::game_loop::space::position::maybe_position;
use crate::game_loop::{helpers, npc};
use crate::model::formulas;
use crate::model::skill::Skill;
use crate::network::server_packets;
use crate::world::World;

/// `Formulas.calcProbability` against the *effected* creature's level — the
/// shared chance gate on `Confuse` and `RandomizeHate`.
pub(crate) fn confuse_chance_passes(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) -> bool {
    let level = player_or_npc_level(world, target_oid);
    let attribute = attribute_mod(world, caster_oid, target_oid, skill);
    let trait_mod =
        calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false);
    let abnormal_resist = crate::game_loop::stats::basic_property::abnormal_resist(
        world,
        target_oid,
        skill.basic_property,
    );
    let roll = world.roll(100);
    formulas::magic::calc_probability(
        skill.magic_level,
        chance,
        level,
        abnormal_resist,
        attribute,
        trait_mod,
        roll,
    )
}

/// Java's `forEachVisibleObject(effected, Creature.class, …)` plus each
/// handler's own exclusions, then `targetList.get(Rnd.get(size))`.
///
/// `Confuse` excludes only the victim themselves (which the query already
/// does). `RandomizeHate` additionally excludes the caster and any attackable
/// **of the victim's own faction** — "aggro cannot be transfered to a mob of
/// the same faction" — which `exclude_caster_and_clan` selects.
pub(crate) fn random_bystander(
    world: &mut World,
    victim_oid: i32,
    caster_oid: i32,
    exclude_caster_and_clan: bool,
) -> Option<i32> {
    let mut candidates = crate::game_loop::space::visibility::visible_creatures(world, victim_oid);
    if exclude_caster_and_clan {
        candidates.retain(|&oid| oid != caster_oid && !same_npc_faction(world, victim_oid, oid));
    }
    if candidates.is_empty() {
        return None;
    }
    let idx = world.roll(candidates.len() as i32) as usize;
    candidates.get(idx).copied()
}

/// Java `((Attackable) cha).isInMyClan(effectedMob)` — two NPCs sharing a clan
/// tag. A player is never in an NPC's faction.
fn same_npc_faction(world: &World, a_oid: i32, b_oid: i32) -> bool {
    let clan_of = |oid: i32| npc::npc_template(world, oid).map(|t| t.clans.clone());
    match (clan_of(a_oid), clan_of(b_oid)) {
        (Some(a), Some(b)) => a.iter().any(|c| b.contains(c)),
        _ => false,
    }
}

/// `effected.setTarget(target)` + `setIntention(AI_INTENTION_ATTACK, target)`,
/// in the two shapes this port has: hate for an NPC, a plain target swap for a
/// player.
pub(crate) fn retarget_onto(world: &mut World, victim_oid: i32, new_target_oid: i32) {
    if crate::game_loop::combat::is_npc_oid(victim_oid) {
        force_attack_target(world, victim_oid, new_target_oid);
    } else if let Some(client_id) = helpers::client_for_player(world, victim_oid) {
        crate::game_loop::combat::target::set_target(
            world,
            client_id,
            victim_oid,
            Some(new_target_oid),
        );
    }
}

/// `Creature.reduceCurrentHp`'s fake-death branch: any real damage taken while
/// playing dead ends the act (`stopFakeDeath(true)` — note the `true`, which
/// *removes the effect*, not just the pose). Finds whichever active buff
/// carries the `FAKE_DEATH` flag and expires it, which routes through
/// `handle_buff_expire` → [`stop_fake_death`] for the client-side stand-up.
pub(crate) fn break_fake_death_on_damage(world: &mut World, object_id: i32) {
    use crate::model::skill::effect_flag;
    if crate::game_loop::abnormal::flags_of(world, object_id) & effect_flag::FAKE_DEATH == 0 {
        return;
    }
    expire_buffs_where(world, object_id, |_, buff| {
        buff.effect_flags & effect_flag::FAKE_DEATH != 0
    });
}

/// Java `EffectList.stopEffectsOnDamage()` — drop every live buff whose skill
/// declares `<removedOnDamage>`, called from `CreatureStatus.reduceHp` /
/// `PlayerStatus.reduceHp` the moment the holder takes a hit.
///
/// This is what wakes a slept character: `Sleep` (1069, 1072, 1394, the mob
/// casts 4046/4185/4201/4660-4662, …) applies `BlockActions`, and the tag is
/// the *only* thing that takes it back off before the timer. Same tag breaks
/// `Hide` (922) and `Force Meditation` (441).
///
/// Java reads the flag off the `BuffInfo`'s skill (`info.getSkill()
/// .isRemovedOnDamage()`) rather than off a cached copy, so the buff's
/// `(skill_id, skill_level)` is resolved back through the skill table here for
/// the same reason — nothing to keep in sync, and buffs restored from the DB on
/// relog behave identically to freshly-cast ones.
pub(crate) fn stop_effects_on_damage(world: &mut World, object_id: i32) {
    expire_buffs_where(world, object_id, |world, buff| {
        world
            .data
            .skill_data
            .get(buff.skill_id, buff.skill_level)
            .is_some_and(|s| s.removed_on_damage)
    });
}

/// How far one fear shove throws the victim — Java `Fear.FEAR_RANGE`.
const FEAR_RANGE: f64 = 500.0;

/// `Fear.canStart` — who can be feared at all. Raid bosses are immune (the
/// same `isRaid()` bail `Mute` has), and on the NPC side only the `Attackable`
/// subtree qualifies, minus the siege-defence family: a fear must not scatter
/// stationed defenders off a castle wall or push a siege golem around.
/// A player is always fearable. Java's `isSummon()` leg folds into the same
/// case, and servitors landed with G29 — so nothing is missing here; the two
/// legs are simply one branch in this port.
pub(crate) fn fear_can_start(world: &World, target_oid: i32) -> bool {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
    else {
        return true;
    };
    let Some(t) = npc.template(world) else {
        return false;
    };
    if t.is_raid() {
        return false;
    }
    // Java's `isSummon()` leg: a pet/servitor is fearable like a player. (In
    // this port a summon is an NPC entity, so it reaches the NPC branch below —
    // which its non-Attackable "Servitor" type would otherwise reject.)
    if world
        .objects
        .has_component::<crate::model::components::summons::ServitorOf>(&target_oid)
        || world
            .objects
            .has_component::<crate::model::components::summons::PetOf>(&target_oid)
    {
        return true;
    }
    t.is_attackable_class()
        && !matches!(
            t.type_name.as_str(),
            "Defender" | "FortCommander" | "SiegeFlag"
        )
        && t.race != Some(crate::enums::Race::SiegeWeapon as i32)
}

/// `Fear.fearAction` — one shove: pick a flight direction, project
/// [`FEAR_RANGE`] units along it, clamp the destination to walkable geodata and
/// walk there.
///
/// The direction is `Util.calculateAngleFrom(effector, effected)` on the first
/// shove — the angle *from the caster to the victim*, so the victim runs
/// directly away — and the victim's own heading (`convertHeadingToDegree`) on
/// every later tick, which keeps them fleeing the way they were first thrown
/// rather than re-deriving a bearing from a caster who may be dead or gone.
/// Java's `toRadians(atan2-in-degrees)` round-trip collapses to the raw
/// `atan2`, so the first case is computed directly in radians here.
pub(crate) fn fear_action(world: &mut World, effector: Option<i32>, effected: i32) {
    // `Creature.moveToLocation`'s own bail — a rooted or stunned victim can't
    // be driven anywhere, though the fear's timer keeps running.
    if crate::game_loop::abnormal::is_movement_disabled(world, effected)
        || crate::game_loop::abnormal::is_blocked_from_actions(world, effected)
    {
        return;
    }
    let Some(pos) = maybe_position(world, effected) else {
        return;
    };
    let radians = match effector.and_then(|e| maybe_position(world, e)) {
        Some(src) => ((pos.y - src.y) as f64).atan2((pos.x - src.x) as f64),
        // `Util.convertHeadingToDegree`: heading / 182.044444444, in degrees.
        None => (pos.heading as f64 / 182.044_444_444).to_radians(),
    };
    let dest_x = (pos.x as f64 + FEAR_RANGE * radians.cos()) as i32;
    let dest_y = (pos.y as f64 + FEAR_RANGE * radians.sin()) as i32;
    // Java projects at the victim's *own* z and lets geodata correct it.
    let (vx, vy, vz) = world
        .geo
        .get_valid_location(pos.x, pos.y, pos.z, dest_x, dest_y, pos.z);

    // `getAI().setIntention(AI_INTENTION_MOVE_TO, destination)` — the player and
    // NPC halves of Java's shared `Creature.moveToLocation` (each already does
    // its own geodata/pathfinding pass on top of the clamp above).
    if let Some(client_id) = helpers::client_for_player(world, effected) {
        crate::game_loop::space::position::intention_move_to(
            world,
            client_id,
            effected,
            pos,
            (vx, vy, vz),
        );
    } else {
        crate::game_loop::npc::ai::set_move_to_intention(world, effected, vx, vy, vz);
    }
}

/// `Mute.onStart` — silencing someone also drops the cast they were already
/// mid-way through, otherwise a mute landing during a cast would let that cast
/// finish. **Raid bosses are immune** (Java's `effected.isRaid()` bail), which
/// is what stops a single silence from neutering a raid.
///
/// Unlike a stun this does not touch movement — a silenced character walks
/// normally.
pub(crate) fn apply_mute_interrupt(world: &mut World, target_oid: i32, skill: &Skill) {
    let mutes = skill.effect_flags()
        & (crate::model::skill::effect_flag::MUTED
            | crate::model::skill::effect_flag::PHYSICAL_MUTED)
        != 0;
    if !mutes {
        return;
    }
    let is_raid = npc::is_raid_npc(world, target_oid);
    if is_raid {
        return;
    }
    // Java's is `abortCast()` → `stopCasting(true)`, so the same
    // `MagicSkillCanceled` applies here: a silenced caster's animation has to
    // stop with the cast.
    crate::game_loop::skills::cast::abort_all_skill_casters(world, target_oid);
    // `startPhysicalAttackMuted()` is `abortAttack()` and nothing else, so the
    // swing in flight dies with a **physical** mute only. A plain silence
    // stops the cast and leaves the swing alone — Java's `Mute.onStart` never
    // calls `abortAttack`.
    if skill.effect_flags() & crate::model::skill::effect_flag::PHYSICAL_MUTED != 0 {
        crate::game_loop::combat::abort_attack(world, target_oid);
    }
}

/// `BlockActions.onStart` — `startParalyze()` (`abortCast` + `stopMove`) plus
/// `abortAllSkillCasters()` on the freshly-stunned victim: a skill that lands
/// `BLOCK_ACTIONS` interrupts whatever the target was doing, rather than only
/// preventing the *next* action. Without this a stun landing mid-cast would let
/// the cast finish.
///
/// The abort goes through [`crate::game_loop::skills::cast::abort_all_skill_casters`], i.e. Java's
/// `stopCasting(true)` — an *aborted* stop, which broadcasts
/// `MagicSkillCanceled`. Dropping the cast quietly is not enough: that packet is
/// what stops the cast animation client-side, so a silent stop leaves a slept
/// mob (or player) visibly finishing its channel — and its skill FX playing —
/// for the rest of the client-side cast time after the sleep already landed.
///
/// A root deliberately does not do this — it stops movement (the movement
/// primitives refuse it from the next tick) but leaves a cast running.
///
/// Java's `startParalyze`/`startStunning` also call `abortAttack()`, so the
/// swing already in flight never lands either — a stun arriving between a
/// swing's start and its hit tick eats that hit.
pub(crate) fn apply_block_actions_interrupt(world: &mut World, target_oid: i32) {
    // ```java
    // public void onStart(Creature effector, Creature effected, Skill skill, Item item)
    // {
    //     if ((effected == null) || effected.isRaid())
    //     {
    //         return;
    //     }
    //     …
    //     effected.startParalyze();
    //     effected.abortAllSkillCasters();
    // }
    // ```
    //
    // **A raid boss is never interrupted.** The buff still lands and its
    // `BLOCK_ACTIONS` flag still counts — `onStart` is the only thing Java
    // skips — so a stun on a raid still gates its *next* action while leaving
    // the cast and the swing already in flight alone. That asymmetry is the
    // point: without it a chain of stuns would cancel a boss's every cast, and
    // the fight would be decided by stun uptime rather than by the encounter.
    //
    // `isRaid()` is the RaidBoss/GrandBoss subtree only — a raid *minion* is
    // `isRaidMinion()`, a separate predicate Java does not consult here, so a
    // minion is interrupted like any other monster.
    if npc::is_raid_npc(world, target_oid) {
        return;
    }
    // Order matters: abort the cast *first*. `stop_casting` resumes the move
    // the cast interrupted (`start_casting` stashes it), so clearing movement
    // before the cast would see it immediately restored — the victim would keep
    // walking while stunned.
    crate::game_loop::skills::cast::abort_all_skill_casters(world, target_oid);
    // `abortAttack()` — the swing already in flight is dropped too, so a stun
    // arriving between a swing's start and its hit tick eats that hit.
    crate::game_loop::combat::abort_attack(world, target_oid);
    // Then freeze them where they stand and tell everyone who can see them.
    if world
        .objects
        .has_component::<crate::model::components::space::Movement>(&target_oid)
    {
        world
            .objects
            .remove_component::<crate::model::components::space::Movement>(&target_oid);
        if let Some(pos) = maybe_position(world, target_oid)
            && let Some(region) = position::region_cell_of(world, target_oid)
        {
            broadcast::broadcast_near_region(
                world,
                region,
                &server_packets::stop_move(target_oid, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }
    // Monsters additionally lose their chase leg; `think` will no-op while the
    // flag is up, and the AI resumes on its own once it expires.
}

/// `target.isCastingNow(s -> s.getSkill().getAbnormalResists().contains(
/// skill.getAbnormalType()))` — is the target part-way through a cast that
/// declares immunity to this abnormal type?
///
/// Empty `abnormal_type` never matches: Java compares against an
/// `AbnormalType` enum whose `NONE` is not in any resist list.
pub(crate) fn casting_resists_abnormal(
    world: &World,
    target_oid: i32,
    abnormal_type: &str,
) -> bool {
    if abnormal_type.is_empty() {
        return false;
    }
    let Some(casting) = world
        .objects
        .get_component::<crate::model::components::combat::Casting>(&target_oid)
    else {
        return false;
    };
    world
        .data
        .skill_data
        .get(casting.0.skill_id, casting.0.skill_level)
        .is_some_and(|s| {
            s.abnormal_resists
                .iter()
                .any(|t| t.eq_ignore_ascii_case(abnormal_type))
        })
}

/// `Bluff.instant` — spin the target to face the caster's heading. Raid
/// bosses and their minions are immune (Java also names NPC 35062, a siege
/// headquarters, explicitly); the pair of rotation packets is what the client
/// animates, and the server-side heading change is what makes a subsequent
/// Backstab land.
pub(crate) fn bluff(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) {
    let is_raid = npc::is_raid_npc(world, target_oid)
        // Java's `isRaidMinion()` is `Monster.onSpawn`'s
        // `setIsRaidMinion(_master.isRaid())` — a minion inherits its master's
        // raid immunity. The port tracks the link as `MinionOf`, so ask the
        // master's template.
        || crate::game_loop::npc::minions::is_raid_minion(world, target_oid);
    if is_raid || !confuse_chance_passes(world, caster_oid, target_oid, skill, chance) {
        return;
    }
    let Some(caster_heading) = world
        .objects
        .get_component::<crate::model::components::space::Position>(&caster_oid)
        .map(|p| p.heading)
    else {
        return;
    };
    let target_heading = world
        .objects
        .get_component::<crate::model::components::space::Position>(&target_oid)
        .map(|p| p.heading)
        .unwrap_or(0);
    if let Some(region) = position::region_cell_of(world, target_oid) {
        for pkt in [
            server_packets::start_rotation(target_oid, target_heading, 1, 65535),
            server_packets::stop_rotation(target_oid, caster_heading, 65535),
        ] {
            broadcast::broadcast_near_region(world, region, &pkt);
        }
    }
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::components::space::Position>(&target_oid)
    {
        p.heading = caster_heading;
    }
}

/// `FakeDeath.onStart` → `Creature.startFakeDeath()`: drop whatever you were
/// doing and hit the deck. `isAlikeDead()` then covers the rest (no aggro, no
/// being targeted), and the client is told with
/// `ChangeWaitType(WT_START_FAKEDEATH)`.
///
/// Java's `FAKE_DEATH_UNTARGET` block (clearing the fake-dead player off
/// everyone else's target) is **False** on this dist's `Character.ini`, so it
/// is deliberately not ported.
pub(crate) fn fake_death(world: &mut World, target_oid: i32) {
    // Players only — Java's `startFakeDeath` returns immediately for anything
    // else.
    if helpers::client_for_player(world, target_oid).is_none() {
        return;
    }
    world
        .objects
        .remove_component::<crate::model::components::combat::Intent>(&target_oid);
    if world
        .objects
        .has_component::<crate::model::components::combat::Casting>(&target_oid)
    {
        crate::game_loop::skills::cast::stop_casting(world, target_oid);
    }
    // `startFakeDeath` calls `abortAttack()` too: you cannot play dead and
    // still land the swing you were mid-way through.
    crate::game_loop::combat::abort_attack(world, target_oid);
    world
        .objects
        .remove_component::<crate::model::components::space::Movement>(&target_oid);
    broadcast_change_wait_type(
        world,
        target_oid,
        server_packets::wait_type::START_FAKEDEATH,
    );
}

/// `SkillTurning.instant` — Spell Turning (1412). Offensive despite the name:
/// it breaks the *target's* cast. Java bails on a self-cast and on raid
/// bosses, and rolls `Rnd.get(100) < chance` unless `staticChance`, which
/// routes through `calcProbability` (level-aware) instead. No dist skill sets
/// `staticChance`.
pub(crate) fn skill_turning(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
    static_chance: bool,
) {
    let is_raid = npc::is_raid_npc(world, target_oid);
    if caster_oid == target_oid || is_raid {
        return;
    }
    let passes = if static_chance {
        confuse_chance_passes(world, caster_oid, target_oid, skill, chance)
    } else {
        world.roll(100) < chance
    };
    if passes {
        crate::game_loop::skills::cast::break_cast(world, target_oid);
    }
}

/// `Formulas.calcStunBreak` + `Creature.stopStunning` — a hit has a 1-in-14
/// chance to shake off a stun (`BreakStun`, **True** on this dist and `false`
/// in Java's own default, so the dist opts *into* it).
///
/// Java's removal is narrower than "every BLOCK_ACTIONS effect": it takes the
/// ones whose `AbnormalType` is `STUN`, and only while
/// `info.getTime() <= info.getSkill().getAbnormalTime()` — the guard that
/// spares a stun whose duration was doubled by skill mastery until it has
/// burned back down to the normal length. The port models no such doubling, so
/// that second half is always true here; it is written down rather than
/// silently dropped, because it becomes load-bearing the moment mastery
/// durations land.
///
/// Sleep and paralyze also carry `BLOCK_ACTIONS` and are deliberately **not**
/// removed — only `STUN` is, which is why this filters on the abnormal type
/// rather than the flag.
pub(crate) fn try_break_stun(world: &mut World, object_id: i32) {
    if !world.cfg.character.alt_game_stun_break {
        return;
    }
    if crate::game_loop::abnormal::flags_of(world, object_id)
        & crate::model::skill::effect_flag::BLOCK_ACTIONS
        == 0
    {
        return;
    }
    // `Rnd.get(14) == 0`.
    if world.roll(14) != 0 {
        return;
    }
    expire_buffs_where(world, object_id, |_, buff| {
        buff.abnormal_type.eq_ignore_ascii_case("STUN")
    });
}
