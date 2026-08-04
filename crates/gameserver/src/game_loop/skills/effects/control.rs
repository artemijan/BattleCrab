use super::*;

/// `Formulas.calcProbability` against the *effected* creature's level — the
/// shared chance gate on `Confuse` and `RandomizeHate`.
pub(crate) fn confuse_chance_passes(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    chance: i32,
) -> bool {
    let level = target_level(world, target_oid);
    let attribute = attribute_mod(world, caster_oid, target_oid, skill);
    let trait_mod =
        calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, false);
    let roll = world.roll(100);
    formulas::calc_probability(skill.magic_level, chance, level, attribute, trait_mod, roll)
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
    let mut candidates = crate::game_loop::helpers::visible_creatures(world, victim_oid);
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
    let clan_of = |oid: i32| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.clans.clone())
    };
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
        let max_hate = world
            .objects
            .get_component::<crate::model::npc::AggroList>(&victim_oid)
            .map(|a| a.0.values().map(|i| i.hate).fold(0.0_f64, f64::max))
            .unwrap_or(0.0);
        if let Some(aggro) = world
            .objects
            .get_component_mut::<crate::model::npc::AggroList>(&victim_oid)
        {
            aggro.0.entry(new_target_oid).or_default().hate = max_hate + 1.0;
        }
        if let Some(ai) = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&victim_oid)
        {
            ai.intention = crate::model::npc::NpcIntention::Attack;
            ai.attack_timeout_tick = world.tick + crate::game_loop::combat::ATTACK_TIMEOUT_TICKS;
        }
    } else if let Some(client_id) = client_for_player(world, victim_oid) {
        crate::game_loop::target::set_target(world, client_id, victim_oid, Some(new_target_oid));
    }
}

/// `effected.getStat().getValue(Stat.MANA_CHARGE, amount)` — the recipient's
/// recharge bonus. Java's two-arg `getValue` is `mul * baseValue + add`, so
/// Higher Mana Gain 285 (`mode=DIFF`, +22..81 by level) is a flat addition.
pub(crate) fn mana_charge_of(world: &World, target_oid: i32, amount: f64) -> f64 {
    use crate::model::stats::Stat;
    let Some(mods) = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&target_oid)
    else {
        return amount;
    };
    let mul = mods.mul.get(&Stat::ManaCharge).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::ManaCharge).copied().unwrap_or(0.0);
    (mul * amount) + add
}

/// `ManaHealByLevel`'s recharge penalty: a target more than 5 levels above the
/// skill's `magicLevel` gets progressively less, and 15+ levels above gets
/// **nothing at all**.
///
/// Java writes it as an `if/else if` ladder from `levelDiff == 6` (×0.9) down
/// to `== 14` (×0.1) with `>= 15` → 0; that is exactly `1 - (diff - 5)/10`
/// over the ladder's range, so it collapses to arithmetic here rather than
/// nine branches. A gap of 5 or less is unpenalised.
pub(crate) fn recharge_level_penalty(target_level: i32, skill_magic_level: i32) -> f64 {
    let diff = target_level - skill_magic_level;
    if diff <= 5 {
        return 1.0;
    }
    if diff >= 15 {
        return 0.0;
    }
    1.0 - ((diff - 5) as f64 / 10.0)
}

pub(crate) fn target_level(world: &World, oid: i32) -> i32 {
    if let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) {
        return p.level;
    }
    world
        .objects
        .get_component::<crate::model::npc::Npc>(&oid)
        .and_then(|n| n.template(world))
        .map(|t| t.level)
        .unwrap_or(1)
}

/// The tail every MP-restore handler shares: the dead / `isMpBlocked` gate, the
/// overheal clamp, the write, `broadcastStatusUpdate`, and the self-vs-other
/// system message.
///
/// Java clamps against `getMaxRecoverableMp()` (`MAX_RECOVERABLE_MP` over
/// `maxMp`). Two skills declare `LimitMp` — Seal of Limit (1509) and Mass
/// Restriction (11603) — but **neither is reachable**: 1509 appears on no
/// skill tree, NPC or item, and 11603 is post-Interlude. So the stat is
/// identity and the ceiling is plain `maxMp` here.
pub(crate) fn restore_mp(world: &mut World, caster_oid: i32, target_oid: i32, amount: f64) {
    use server_packets::{SmParam, sm_ids};
    // `effected.isDead() || effected.isDoor() || effected.isMpBlocked()`.
    if world
        .objects
        .get_component::<Vitals>(&target_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    if crate::game_loop::abnormal::is_mp_blocked(world, target_oid) {
        return;
    }
    // "Prevents overheal and negative amount".
    let restored = {
        let Some(v) = world.objects.get_component_mut::<Vitals>(&target_oid) else {
            return;
        };
        let headroom = (v.max_mp as f64 - v.cur_mp).max(0.0);
        let restored = amount.min(headroom).max(0.0);
        if restored != 0.0 {
            v.cur_mp += restored;
        }
        restored
    };
    if restored != 0.0 {
        broadcast_vitals(world, target_oid);
    }
    // Java sends the message even when the amount rounded to nothing.
    if let Some(cid) = client_for_player(world, target_oid) {
        let pkt = if caster_oid != target_oid {
            server_packets::system_message_with(
                sm_ids::S2_MP_HAS_BEEN_RESTORED_BY_C1,
                &[
                    SmParam::Text(caster_display_name(world, caster_oid)),
                    SmParam::Int(restored as i32),
                ],
            )
        } else {
            server_packets::system_message_with(
                sm_ids::S1_MP_HAS_BEEN_RESTORED,
                &[SmParam::Int(restored as i32)],
            )
        };
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(pkt);
        }
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
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .filter(|x| x.effect_flags & effect_flag::FAKE_DEATH != 0)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default();
    for skill_id in skill_ids {
        handle_buff_expire(world, object_id, skill_id);
    }
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
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .filter(|x| {
                    world
                        .data
                        .skill_data
                        .get(x.skill_id, x.skill_level)
                        .is_some_and(|s| s.removed_on_damage)
                })
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default();
    for skill_id in skill_ids {
        handle_buff_expire(world, object_id, skill_id);
    }
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
        .has_component::<crate::model::components::ServitorOf>(&target_oid)
        || world
            .objects
            .has_component::<crate::model::components::PetOf>(&target_oid)
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
    use crate::model::components::Position;
    // `Creature.moveToLocation`'s own bail — a rooted or stunned victim can't
    // be driven anywhere, though the fear's timer keeps running.
    if crate::game_loop::abnormal::is_movement_disabled(world, effected)
        || crate::game_loop::abnormal::is_blocked_from_actions(world, effected)
    {
        return;
    }
    let Some(pos) = world.objects.get_component::<Position>(&effected).copied() else {
        return;
    };
    let radians = match effector.and_then(|e| world.objects.get_component::<Position>(&e).copied())
    {
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
    if let Some(client_id) = client_for_player(world, effected) {
        crate::game_loop::position::intention_move_to(
            world,
            client_id,
            effected,
            pos,
            (vx, vy, vz),
        );
    } else {
        // Set before the move: `move_npc_to` can bail (no speed, no path), and
        // Java changes the intention regardless of whether the walk starts.
        if let Some(ai) = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&effected)
        {
            ai.intention = crate::model::npc::NpcIntention::MoveTo;
        }
        crate::game_loop::npc_ai::move_npc_to(world, effected, vx, vy, vz);
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
    let is_raid = world
        .objects
        .get_component::<crate::model::npc::Npc>(&target_oid)
        .and_then(|n| n.template(world))
        .is_some_and(|t| t.is_raid());
    if is_raid {
        return;
    }
    // Java's is `abortCast()` → `stopCasting(true)`, so the same
    // `MagicSkillCanceled` applies here: a silenced caster's animation has to
    // stop with the cast.
    crate::game_loop::skills::cast::abort_all_skill_casters(world, target_oid);
}

/// Java `AttackableStatus.reduceHp` + `Attackable.setOverhitValues`: bank the
/// *excess* damage of a killing `<overHit>` blow, so the kill reward can pay a
/// bonus for it.
///
/// `excess = damage - currentHp`. A blow that fails to kill (negative excess)
/// **disarms** the state — as does any damage from a non-overhit skill — so the
/// record only ever survives on a corpse, and only from the blow that made it
/// one.
pub(crate) fn record_overhit(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    damage: f64,
    over_hit: bool,
) {
    use crate::model::components::Overhit;
    if damage <= 0.0 {
        return;
    }
    let cur_hp = world
        .objects
        .get_component::<Vitals>(&target_oid)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0);
    let excess = damage - cur_hp;
    if !over_hit || excess < 0.0 {
        world.objects.remove_component::<Overhit>(&target_oid);
        return;
    }
    world.objects.add_components(
        &target_oid,
        Overhit {
            damage: excess,
            attacker: caster_oid,
        },
    );
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
/// TODO(G34): Java's `startParalyze` also calls `abortAttack()`, which drops the
/// swing already in flight (`CreatureAttackTaskManager.abortAttack`). This port
/// has no cancel handle on a scheduled `AttackHit`, so a stun landing between a
/// swing's start and its hit tick still lets that hit land.
pub(crate) fn apply_block_actions_interrupt(world: &mut World, target_oid: i32) {
    // Order matters: abort the cast *first*. `stop_casting` resumes the move
    // the cast interrupted (`start_casting` stashes it), so clearing movement
    // before the cast would see it immediately restored — the victim would keep
    // walking while stunned.
    crate::game_loop::skills::cast::abort_all_skill_casters(world, target_oid);
    // Then freeze them where they stand and tell everyone who can see them.
    if world
        .objects
        .has_component::<crate::model::components::Movement>(&target_oid)
    {
        world
            .objects
            .remove_component::<crate::model::components::Movement>(&target_oid);
        if let Some(pos) = world
            .objects
            .get_component::<crate::model::components::Position>(&target_oid)
            .copied()
            && let Some(region) = world
                .objects
                .get_component::<crate::model::components::RegionCell>(&target_oid)
                .map(|r| r.0)
        {
            crate::game_loop::helpers::broadcast_near_region(
                world,
                region,
                &server_packets::stop_move(target_oid, pos.x, pos.y, pos.z, pos.heading),
            );
        }
    }
    // Monsters additionally lose their chase leg; `think` will no-op while the
    // flag is up, and the AI resumes on its own once it expires.
}

/// A target creature's level (Java `Creature.getLevel()`) for the debuff
/// landing-rate math — an NPC reads its template, a player its record. Defaults
/// to 1, matching the Spoil landing-level fallback.
pub(crate) fn creature_level(world: &World, oid: i32) -> i32 {
    // Java `Cubic.getLevel()` → `_owner.getLevel()`. Checked before the NPC/
    // player split because a cubic's caster entity is neither.
    if let Some(c) = world
        .objects
        .get_component::<crate::model::components::CubicOf>(&oid)
    {
        return world
            .objects
            .get_component::<crate::model::Player>(&c.owner_object_id)
            .map(|p| p.level)
            .unwrap_or(1);
    }
    if crate::game_loop::combat::is_npc_oid(oid) {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.level)
            .unwrap_or(1)
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.level)
            .unwrap_or(1)
    }
}

/// `RebalanceHP.instant` — Balance Life (1043).
///
/// Two passes over the same set: sum `maxHp` and `curHp` across every living
/// party member in `affect_range` (plus their pet and servitors), then set each
/// of them to `maxHp * (sumCur / sumMax)`. Java bails outright when the caster
/// is not a player, and does nothing at all when there is no party — an
/// unpartied cast is wasted, which is *not* the "party of one" reading every
/// other party-scoped effect uses.
///
/// The heal direction matters: only a member whose HP goes **up** is clamped by
/// `MAX_RECOVERABLE_HP` (and a member already above that ceiling keeps what
/// they have rather than being pulled down to it). A member who loses HP is
/// written unconditionally — the ceiling guards heals, not the redistribution.
pub(crate) fn rebalance_party_hp(world: &mut World, caster_oid: i32, skill: &Skill) {
    if !world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    let Some(members) = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&caster_oid)
        .and_then(|r| world.parties.get(&r.0))
        .map(|p| p.members.clone())
    else {
        // No party: Java's `if (party != null)` guard skips the whole effect.
        return;
    };
    let Some(origin) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
        .copied()
    else {
        return;
    };
    let range = skill.affect_range;
    let in_range = |world: &World, oid: i32| -> bool {
        // `Util.checkIfInRange(range, effector, target, true)` — 3D, and a
        // range of 0 means "no distance filter" the same way the affect
        // helpers read it.
        if range <= 0 {
            return true;
        }
        world
            .objects
            .get_component::<crate::model::components::Position>(&oid)
            .is_some_and(|p| {
                let (dx, dy, dz) = (
                    (origin.x - p.x) as f64,
                    (origin.y - p.y) as f64,
                    (origin.z - p.z) as f64,
                );
                dx * dx + dy * dy + dz * dz <= (range as f64) * (range as f64)
            })
    };

    // Every creature the effect touches: each member, then their pet and
    // servitor. Java walks all three lists twice; collecting once keeps the
    // two passes over exactly the same set.
    let mut touched: Vec<i32> = Vec::new();
    for member in &members {
        for oid in std::iter::once(*member)
            .chain(crate::game_loop::servitor::pet_of(world, *member))
            .chain(crate::game_loop::servitor::servitor_of(world, *member))
        {
            let alive = world
                .objects
                .get_component::<Vitals>(&oid)
                .is_some_and(|v| !v.dead);
            if alive && in_range(world, oid) {
                touched.push(oid);
            }
        }
    }

    let (mut full_hp, mut current_hp) = (0.0f64, 0.0f64);
    for &oid in &touched {
        if let Some(v) = world.objects.get_component::<Vitals>(&oid) {
            full_hp += v.max_hp as f64;
            current_hp += v.cur_hp;
        }
    }
    if full_hp <= 0.0 {
        return;
    }
    let percent = current_hp / full_hp;

    for &oid in &touched {
        let Some(v) = world.objects.get_component::<Vitals>(&oid).copied() else {
            continue;
        };
        let mut new_hp = v.max_hp as f64 * percent;
        if new_hp > v.cur_hp {
            let ceiling = max_recoverable(
                world,
                oid,
                crate::model::stats::Stat::MaxRecoverableHp,
                v.max_hp as f64,
            );
            if v.cur_hp > ceiling {
                new_hp = v.cur_hp;
            } else if new_hp > ceiling {
                new_hp = ceiling;
            }
        }
        if let Some(vit) = world.objects.get_component_mut::<Vitals>(&oid) {
            vit.cur_hp = new_hp.clamp(0.0, vit.max_hp as f64);
        }
        broadcast_vitals(world, oid);
    }
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
        .get_component::<crate::model::components::Casting>(&target_oid)
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

/// Test hook for [`creature_level`], which is private to this module.
#[cfg(test)]
pub(crate) fn creature_level_for_test(world: &World, oid: i32) -> i32 {
    creature_level(world, oid)
}

/// A target creature's display name (Java `Creature.getName()`) for the debuff
/// landed/resisted caster line — an NPC's template name or the player's name.
pub(crate) fn creature_name(world: &World, oid: i32) -> String {
    if crate::game_loop::combat::is_npc_oid(oid) {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&oid)
            .and_then(|n| n.template(world))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&oid)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }
}

/// `CallParty.instant` — Chant of Gate (1429).
///
/// Every *other* party member is pulled to the caster. There is deliberately no
/// `ConfirmDlg` here: unlike Summon Friend, Java calls `teleToLocation`
/// outright, so a party member gets no say in it.
///
/// Each member is gated by `CallPc.checkSummonTargetStatus`, whose refusals are
/// **messaged to the caster**, not the member — the ported subset is dead, in a
/// private store, and in combat (Java also checks rooted, olympiad, observer,
/// flying mount, combat flag, the `NO_SUMMON_FRIEND`/`JAIL` zones and instance
/// permissions; none of those states are modelled for this path yet).
/// TODO(G34): extend the gate list as those states land.
pub(crate) fn call_party(world: &mut World, caster_oid: i32) {
    use server_packets::{SmParam, sm_ids};

    let Some(members) = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&caster_oid)
        .and_then(|r| world.parties.get(&r.0))
        .map(|p| p.members.clone())
    else {
        // `if (party == null) return` — solo, the cast is simply wasted.
        return;
    };
    let Some(dest) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
        .copied()
    else {
        return;
    };

    for member in members {
        // `effector != partyMember` — the caster is not recalled to itself.
        if member == caster_oid {
            continue;
        }
        let name = world
            .objects
            .get_component::<crate::model::Player>(&member)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let refusal = if world
            .objects
            .get_component::<Vitals>(&member)
            .is_none_or(|v| v.dead)
        {
            Some(sm_ids::C1_IS_DEAD_AT_THE_MOMENT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED)
        } else if world
            .objects
            .get_component::<crate::model::Player>(&member)
            .is_some_and(|p| p.store_type != 0)
        {
            Some(
                sm_ids::C1_IS_CURRENTLY_TRADING_OR_OPERATING_A_PRIVATE_STORE_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED,
            )
        } else if crate::game_loop::combat::has_attack_stance(world, member) {
            // `isInCombat()` — the attack stance is exactly Java's flag.
            Some(sm_ids::C1_IS_ENGAGED_IN_COMBAT_AND_CANNOT_BE_SUMMONED_OR_TELEPORTED)
        } else {
            None
        };
        if let Some(sm) = refusal {
            send_sm_with(world, caster_oid, sm, &[SmParam::PlayerName(name)]);
            continue;
        }
        crate::game_loop::death::teleport_player(world, member, dest.x, dest.y, dest.z);
    }
}

/// `handlers/effecthandlers/CallPc.java`, the `player == null` branch — a
/// **monster** dragging its victim to itself. This is Porta's (20213) "Summon"
/// (4161), and Java's body is five lines:
///
/// ```text
/// effected.abortCast();
/// effected.abortAttack();
/// effected.stopMove(null);
/// effected.sendPacket(new FlyToLocation(effected, effector, FlyType.DUMMY, …));
/// effected.setLocation(effector.getLocation());
/// ```
///
/// Note `setLocation`, **not** `teleToLocation`: no fade, no decay/respawn, no
/// `Appearing` round trip. The victim slides across on the client and the
/// server just moves the point. The whole hop is bounded by the skill's
/// `castRange` (600 for 4161), so it never crosses more than one world region
/// and the ordinary visibility sweep picks up the new neighbourhood.
///
/// The `TargetType::Enemy` gate is Java's: `CallPc` on any other target type
/// from a non-player effector falls to the `teleToLocation` branch, which is
/// the *player* being recalled — not something a monster does.
pub(crate) fn call_pc(world: &mut World, caster_oid: i32, target_oid: i32, skill: &Skill) {
    // "if (effector == effected) return" — a mob can't summon itself.
    if caster_oid == target_oid {
        return;
    }
    // The ported half is the NPC one; a player effector wants the Summon
    // Friend `ConfirmDlg` round trip, which isn't built (see
    // `SkillEffect::CallPc`).
    if world
        .objects
        .has_component::<crate::model::Player>(&caster_oid)
    {
        return;
    }
    if skill.target_type != crate::model::skill::TargetType::Enemy {
        return;
    }
    // `effected.getActingPlayer()` — the branch is player-only; a servitor
    // caught in the cast is left where it stands, as in Java.
    if !world
        .objects
        .has_component::<crate::model::Player>(&target_oid)
    {
        return;
    }
    let Some(dest) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
        .copied()
    else {
        return;
    };
    let Some(from) = world
        .objects
        .get_component::<crate::model::components::Position>(&target_oid)
        .copied()
    else {
        return;
    };

    // `abortCast()` / `abortAttack()` / `stopMove(null)`.
    //
    // `abortCast()` is `SkillCaster.canAbortCast`-gated — a *target* check, not
    // the phase check its Java comment claims — so it takes the same helper the
    // teleport prologue uses, not [`crate::game_loop::skills::cast::abort_cast`], whose `!launched`
    // guard would swallow the `MagicSkillCanceled` that stops the victim's own
    // cast animation client-side.
    crate::game_loop::skills::cast::abort_cast_when_untargeted(world, target_oid);
    world
        .objects
        .remove_component::<crate::model::components::AttackState>(&target_oid);
    world
        .objects
        .remove_component::<crate::model::components::Movement>(&target_oid);
    world
        .objects
        .remove_component::<crate::model::components::Intent>(&target_oid);
    // Java's `stopMove(null)` ends with `broadcastPacket(new StopMove(this))`.
    // Dropping the `Movement` component only stops the *server* walking the
    // victim; without the packet every client keeps animating the run toward
    // the old destination, so the drag leaves the character sliding. Java
    // broadcasts it before `setLocation`, i.e. at the old point.
    crate::game_loop::helpers::broadcast_including_self(
        world,
        target_oid,
        &server_packets::stop_move(target_oid, from.x, from.y, from.z, from.heading),
    );

    // Java's `FlyToLocation` constructor arms `blinkActive` for a player
    // target, which makes the next `ValidatePosition` skip its out-of-sync
    // snap — otherwise the victim's own stale position report drags it back
    // out of the mob's lap.
    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.blink_active = true;
    }
    // Java sends `FlyToLocation` to the effected player only; everyone else
    // learns the new position from the movement/validate-position stream. The
    // port broadcasts it so bystanders see the yank rather than a silent
    // teleport — the packet is a pure animation and the client ignores it for
    // objects it can't see.
    crate::game_loop::helpers::broadcast_including_self(
        world,
        target_oid,
        &server_packets::fly_to_location(
            target_oid,
            (from.x, from.y, from.z),
            (dest.x, dest.y, dest.z),
            server_packets::FlyType::Dummy,
        ),
    );

    if let Some(pos) = world
        .objects
        .get_component_mut::<crate::model::components::Position>(&target_oid)
    {
        pos.x = dest.x;
        pos.y = dest.y;
        pos.z = dest.z;
    }
    // Same reason as the respawn teleport: the region index has to move with
    // the cell. No-op on the index for a non-player target.
    world.set_player_region(target_oid, crate::world::region_of(dest.x, dest.y));
    // Java sends nothing else here — in particular no `MagicSkillCanceled` for
    // the caster. A cancel would end the summoning FX the client keeps drawing
    // for the skill's own (skillgrp) duration, past the 2 s cast; Java has that
    // leftover too, so the port keeps it rather than inventing a packet.
}
