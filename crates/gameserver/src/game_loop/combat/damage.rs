use super::*;

/// Damage application shared by auto-attacks (and reusable by future physical
/// skills): route to the right victim kind, waking NPC AI / breaking player
/// casts / killing at 0 HP. `is_dot` is Java `CreatureStatus.reduceHp`'s
/// `isDOT` — the one exemption from `HP_BLOCK` (Celestial Shield, …) besides
/// a skill's own HP cost, which never reaches this shared path at all.
/// Java `Creature.doAttack`'s tail — the on-hit reactions that wrap the plain
/// HP reduction: the attacker's **vampiric absorb** before the damage lands,
/// and the target's **damage reflect** after.
///
/// Separate from [`apply_physical_damage`] because that is the
/// `reduceCurrentHp` analog, and Java's own non-attack damage sources (a
/// `DamageZone` tick) call *that* directly — a lava pool neither feeds a
/// vampire nor gets reflected.
///
/// `skill_magic` is `None` for an auto-attack and `Some(is_magic)` for a skill
/// hit; both gates need it (`skill == null` for the absorb,
/// `skill.isMagic()` for the reflect's defence cap).
pub(crate) fn apply_attack_damage(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
    is_dot: bool,
    skill_magic: Option<bool>,
) {
    absorb_damage_to_hp(world, attacker, target, damage, is_dot, skill_magic);
    absorb_damage_to_mp(world, attacker, target, damage, is_dot, skill_magic);
    apply_physical_damage(
        world,
        attacker,
        target,
        damage,
        is_dot,
        skill_magic.is_some(),
    );
    reflect_damage(world, attacker, target, damage, is_dot, skill_magic);
    // Java `OnCreatureDamageReceived`, fired from `reduceCurrentHp` alongside
    // `OnCreatureDamageDealt` — so unlike the attack-side twin (which the
    // autoattack path fires, since `allowSkillAttack` defaults false) this one
    // sees **skill damage too**. `TriggerSkillByDamage` listens on it.
    crate::game_loop::skills::effects::fire_damage_received_triggers(
        world,
        target,
        attacker,
        damage as i32,
        is_dot,
    );
}

/// `Creature.doAttack`'s "Absorb HP from the damage inflicted" block.
///
/// Java's gates, in order: **not with a ranged weapon** ("Do not absorb if
/// weapon is ranged"), not in PvP unless `VampiricAttackAffectsPvP`, and not on
/// a skill unless `VampiricAttackWorkWithSkills` — **False** on this dist, so
/// Vampiric Rage feeds off auto-attacks only.
///
/// The absorbed amount is clamped three times: by the healer's missing HP, by
/// the victim's *current* HP (you cannot drain more than is there), and by the
/// victim's `ABSORB_DAMAGE_DEFENCE` multiplier.
fn absorb_damage_to_hp(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
    is_dot: bool,
    skill_magic: Option<bool>,
) {
    use crate::model::components::{StatModifiers, Vitals};
    use crate::model::stats::Stat;
    if is_dot || damage <= 0.0 {
        return;
    }
    if skill_magic.is_some() && !world.cfg.character.vampiric_attack_works_with_skills {
        return;
    }
    // "Do not absorb if weapon is ranged" — a bow drains nothing.
    if crate::game_loop::ranged::is_ranged(
        crate::game_loop::ranged::equipped_weapon_type(world, attacker).unwrap_or_default(),
    ) {
        return;
    }
    // `isPvP` here is Java's narrow one: a *playable* hitting a *playable*.
    let is_pvp = !is_npc_oid(attacker) && !is_npc_oid(target);
    if is_pvp && !world.cfg.character.vampiric_attack_affects_pvp {
        return;
    }

    let Some(mods) = world.objects.get_component::<StatModifiers>(&attacker) else {
        return;
    };
    let absorb_percent = crate::model::finalize(mods, Stat::AbsorbDamagePercent, 0.0);
    if absorb_percent <= 0.0 {
        return;
    }
    let vampiric_sum = crate::model::finalize(mods, Stat::VampiricSum, 0.0);
    // `VampiricChanceFinalizer`: `min(1, vampiricSum / (absorbPercent·100) / 100)`.
    let chance = (vampiric_sum / (absorb_percent * 100.0) / 100.0).min(1.0);
    if world.roll_f64() >= chance {
        return;
    }

    let Some(healer) = world.objects.get_component::<Vitals>(&attacker) else {
        return;
    };
    let missing = healer.max_hp as f64 - healer.cur_hp;
    let victim_hp = world
        .objects
        .get_component::<Vitals>(&target)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0);
    // Java truncates to `int` at each step, so the two `min`s are integer ones.
    let mut absorbed = (absorb_percent * damage).min(missing).trunc();
    absorbed = absorbed.min(victim_hp.trunc());
    // Java also multiplies by the victim's `ABSORB_DAMAGE_DEFENCE`; no skill on
    // this dist grants that stat, so it is its 1.0 identity and is not folded.
    if absorbed <= 0.0 {
        return;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&attacker) {
        v.cur_hp = (v.cur_hp + absorbed).min(v.max_hp as f64);
    }
    crate::game_loop::skills::effects::broadcast_vitals_for(world, attacker);
}

/// `Creature.reduceCurrentHp`'s "Absorb MP from the damage inflicted" block —
/// `MpVampiricAttack` (Weapon Mastery 250), the MP twin of
/// [`absorb_damage_to_hp`].
///
/// **The two gates are shaped opposite ways and that is not a typo.** HP
/// vampirism asks `skill == null || VAMPIRIC_ATTACK_WORKS_WITH_SKILLS` — it
/// works with *melee* and needs a config to reach skills. MP vampirism asks
/// `skill != null || MP_VAMPIRIC_ATTACK_WORKS_WITH_MELEE` — it works with
/// *skills* and needs a config to reach melee. Both configs are off on this
/// dist, so Weapon Mastery drains MP on skill hits only.
///
/// Unlike the HP twin there is **no ranged-weapon exclusion**: Java's "Do not
/// absorb if weapon is ranged" guard wraps only the HP block.
fn absorb_damage_to_mp(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
    is_dot: bool,
    skill_magic: Option<bool>,
) {
    use crate::model::components::{StatModifiers, Vitals};
    use crate::model::stats::Stat;
    if is_dot || damage <= 0.0 {
        return;
    }
    if skill_magic.is_none() && !world.cfg.character.mp_vampiric_attack_work_with_melee {
        return;
    }
    let is_pvp = !is_npc_oid(attacker) && !is_npc_oid(target);
    if is_pvp && !world.cfg.character.mp_vampiric_attack_affects_pvp {
        return;
    }
    let Some(mods) = world.objects.get_component::<StatModifiers>(&attacker) else {
        return;
    };
    let absorb_percent = crate::model::finalize(mods, Stat::AbsorbManaDamagePercent, 0.0);
    if absorb_percent <= 0.0 {
        return;
    }
    let vampiric_sum = crate::model::finalize(mods, Stat::MpVampiricSum, 0.0);
    // `MpVampiricChanceFinalizer`: `min(1, sum / (percent·100) / 100)`.
    let chance = (vampiric_sum / (absorb_percent * 100.0) / 100.0).min(1.0);
    if world.roll_f64() >= chance {
        return;
    }
    let Some(drainer) = world.objects.get_component::<Vitals>(&attacker) else {
        return;
    };
    // Java caps at `getMaxRecoverableMp() - getCurrentMp()`. Two skills declare
    // `LimitMp` (Seal of Limit 1509, Mass Restriction 11603) but **neither is
    // reachable** — 1509 is on no skill tree, NPC or item, and 11603 is
    // post-Interlude — so `MAX_RECOVERABLE_MP` is identity and this is
    // `getMaxMp()`.
    let missing = drainer.max_mp as f64 - drainer.cur_mp;
    let victim_mp = world
        .objects
        .get_component::<Vitals>(&target)
        .map(|v| v.cur_mp)
        .unwrap_or(0.0);
    let mut absorbed = (absorb_percent * damage).min(missing).trunc();
    absorbed = absorbed.min(victim_mp.trunc());
    if absorbed <= 0.0 {
        return;
    }
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&attacker) {
        v.cur_mp = (v.cur_mp + absorbed).min(v.max_mp as f64);
    }
    crate::game_loop::skills::effects::broadcast_vitals_for(world, attacker);
}

/// `Creature.doAttack`'s reflect block: the target bounces
/// `REFLECT_DAMAGE_PERCENT`% of what it just took back at the attacker.
///
/// **"When killing blow is made, the target doesn't reflect"** — Java skips the
/// whole block when the target died, and skips DoT ticks and reflected damage
/// itself (no infinite ping-pong). The bounced amount is capped by the target's
/// max HP and then by its own defence: `mDef · 1.5` for a magic skill, `pDef`
/// otherwise.
fn reflect_damage(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
    is_dot: bool,
    skill_magic: Option<bool>,
) {
    use crate::model::components::{CombatStats, StatModifiers, Vitals};
    use crate::model::stats::Stat;
    if is_dot || damage <= 0.0 {
        return;
    }
    if world
        .objects
        .get_component::<Vitals>(&target)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    let target_is_player = !is_npc_oid(target);
    let limit = if target_is_player {
        world.cfg.character.player_reflect_percent_limit
    } else {
        world.cfg.character.non_player_reflect_percent_limit
    };
    let Some(mods) = world.objects.get_component::<StatModifiers>(&target) else {
        return;
    };
    let percent = crate::model::finalize(mods, Stat::ReflectDamagePercent, 0.0);
    // Java subtracts the *attacker's* `REFLECT_DAMAGE_PERCENT_DEFENSE` before
    // the clamp; no skill on this dist grants it, so it is the 0 default.
    let percent = percent.min(limit);
    if percent <= 0.0 {
        return;
    }
    let mut reflected = ((percent / 100.0) * damage).trunc();
    let (max_hp, p_def, m_def) = {
        let v = world.objects.get_component::<Vitals>(&target);
        let cs = world.objects.get_component::<CombatStats>(&target);
        (
            v.map(|v| v.max_hp as f64).unwrap_or(0.0),
            cs.map(|c| c.p_def).unwrap_or(0.0),
            cs.map(|c| c.m_def).unwrap_or(0.0),
        )
    };
    reflected = reflected.min(max_hp);
    // Java holds `reflectedDamage` in an `int`, so the magic cap's
    // `(int) Math.min(reflectedDamage, mDef * 1.5)` truncates the fractional
    // half — a 25.06 mDef caps at 37, not 37.59.
    reflected = if skill_magic == Some(true) {
        reflected.min(m_def * 1.5)
    } else {
        reflected.min(p_def)
    }
    .trunc();
    if reflected <= 0.0 {
        return;
    }
    // `target.doAttack(reflectedDamage, this, …, reflect = true)` — the
    // `reflect` flag is what stops this from bouncing back again.
    apply_physical_damage(world, target, attacker, reflected, false, false);
}

/// `from_skill` is Java's `skill != null` at the `reduceCurrentHp` call — the
/// discriminator `Attackable.reduceCurrentHp` uses to decide whether the
/// attacker's `HATE_ATTACK` scales the hate generated. Reflect and zone damage
/// pass `false`: Java hands `null` for the skill on both paths.
pub(crate) fn apply_physical_damage(
    world: &mut World,
    attacker: i32,
    target: i32,
    damage: f64,
    is_dot: bool,
    from_skill: bool,
) {
    if !is_dot && crate::game_loop::abnormal::is_hp_blocked(world, target) {
        return;
    }
    if is_npc_oid(target) {
        // `CreatureStatus.reduceHp`: `if (!isDOT && !isHPConsumption) { if
        // (awake) creature.stopEffectsOnDamage(); … }` — a slept mob wakes on
        // the first blow, but a DoT tick alone will not rouse it. (The player
        // twin below is deliberately *not* DoT-gated; see there.)
        //
        // Java's `awake` is `(skill == null) || !skill.isToggle()`. No ported
        // damage source is a toggle — the toggles that cost HP drain `Vitals`
        // directly as `isHPConsumption`, which never reaches this path — so it
        // is always true here and is not threaded through.
        if !is_dot {
            crate::game_loop::skills::effects::stop_effects_on_damage(world, target);
        }
        // `skill_magic.is_none()` is Java's `skill == null` — the *auto-attack*
        // discriminator `Attackable.reduceCurrentHp` uses to decide whether
        // `HATE_ATTACK` applies (G34 S4).
        npc_receive_damage(world, target, attacker, damage, !from_skill);
    } else {
        // `Creature.reduceCurrentHp`: `if (isPlayer() && isFakeDeath() &&
        // Config.FAKE_DEATH_DAMAGE_STAND && amount > 0) stopFakeDeath(true)`.
        // `FakeDeathDamageStand = True` on this dist, so taking a hit while
        // playing dead stands you back up — otherwise a rogue could feign
        // death and soak a whole fight from the floor.
        if damage > 0.0 {
            crate::game_loop::skills::effects::break_fake_death_on_damage(world, target);
        }
        player_receive_damage(world, target, attacker, damage);
    }
}

/// `Attackable.reduceCurrentHp` → `addDamage`/`addDamageHate` + the
/// `onEvtAttacked` AI reaction, then the HP cut and `doDie`.
pub(crate) fn npc_receive_damage(
    world: &mut World,
    npc_oid: i32,
    attacker_oid: i32,
    damage: f64,
    auto_attack: bool,
) {
    if world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    // `ai/others/Servitors/SinEater`'s `ON_CREATURE_ATTACKED` bark (a no-op for
    // every other NPC).
    crate::scripts::sin_eater::on_attacked(world, npc_oid);
    let level = match world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    {
        Some(npc) => npc.template(world).map(|t| t.level).unwrap_or(1),
        None => return,
    };
    let now = world.tick;
    // Resolved before the component borrow below. `CHAMPION_HP == 0` disables
    // the division in Java (the `!= 0` guard), so it maps to a ×1 divisor.
    let champion_divisor = {
        let cfg = &world.cfg.champion;
        let is_champion = cfg.enable
            && world
                .objects
                .get_component::<crate::model::npc::Npc>(&npc_oid)
                .is_some_and(|n| n.champion);
        if is_champion && cfg.hp != 0 {
            cfg.hp as f64
        } else {
            1.0
        }
    };

    let hate_attack_mul = if auto_attack {
        world
            .objects
            .get_component::<crate::model::components::StatModifiers>(&attacker_oid)
            .and_then(|m| m.mul.get(&crate::model::stats::Stat::HateAttack).copied())
            .unwrap_or(1.0)
    } else {
        1.0
    };
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
        //
        // `Attackable.reduceCurrentHp` then scales it by the attacker's
        // `HATE_ATTACK` — but **only when `skill == null`**, i.e. for an
        // auto-attack. A skill's hate is deliberately not amplified, which is
        // why Sword/Blunt Weapon Mastery (217) helps a tank hold aggro through
        // ordinary swings and does nothing for their taunts.
        let hate = damage * 100.0 / (level + 7) as f64 * hate_attack_mul;
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

        // `Creature.reduceCurrentHp`'s champion arm: the hit is divided by
        // `ChampionHp` — Java models a champion's bulk as damage reduction,
        // **not** as a bigger HP pool, so the health bar still reads 100 % and
        // the mob simply takes ten times as many swings to fall. Hate above is
        // deliberately computed from the *undivided* damage, as in Java, where
        // `Attackable.reduceCurrentHp` calls `addDamageHate` before delegating
        // the HP cut to `super`.
        vitals.cur_hp -= damage / champion_divisor;
        if vitals.cur_hp <= 0.0 {
            vitals.cur_hp = 0.0;
            died = true;
        }
        (vitals.cur_hp as i32, vitals.max_hp)
    };
    // Orfen's `onAttack`: the half-HP relocation and the mid-range drag. Both
    // react to a hit that has already landed, so they sit alongside the raid
    // curse below. No-op for every other NPC.
    if let Some(npc_id) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .map(|n| n.npc_id)
    {
        if npc_id == crate::game_loop::core_boss::CORE {
            crate::game_loop::core_boss::on_core_attacked(world, npc_oid);
        }
        if npc_id == crate::game_loop::baium::BAIUM {
            crate::game_loop::baium::on_baium_attacked(world, npc_oid, attacker_oid);
            // A physical swing is Java's `skill == null` branch — the ×1000
            // melee weighting.
            crate::game_loop::baium::on_baium_damage(
                world,
                npc_oid,
                attacker_oid,
                damage as i32,
                true,
            );
        }
        if npc_id == crate::game_loop::antharas::ANTHARAS {
            // A physical swing is Java's `skill == null` branch — the ×1000
            // melee weighting. Same table and same order as Baium's.
            crate::game_loop::antharas::on_antharas_damage(
                world,
                npc_oid,
                attacker_oid,
                damage as i32,
                true,
            );
        }
        if npc_id == crate::game_loop::valakas::VALAKAS {
            crate::game_loop::valakas::on_valakas_attacked(world, npc_oid, attacker_oid);
        }
        if npc_id == crate::game_loop::dr_chaos::CHAOS_GOLEM {
            crate::game_loop::dr_chaos::on_golem_attacked(world, npc_oid);
        }
        if npc_id == crate::game_loop::orfen::ORFEN {
            crate::game_loop::orfen::on_orfen_attacked(world, npc_oid, attacker_oid);
        } else if npc_id == crate::game_loop::orfen::RIBA_IREN {
            crate::game_loop::orfen::on_riba_iren_attacked(world, npc_oid);
        }
    }

    // `Attackable.reduceCurrentHp`'s raid-curse check, and Java's own comment
    // is the reason it sits **here** rather than before the damage block:
    // "In retail you deal damage to raid before curse." The hit that earns the
    // curse still lands.
    crate::game_loop::raid_curse::on_raid_attacked(world, npc_oid, attacker_oid);

    // Same method's loot-privilege block: a big-enough command channel claims
    // (or refreshes) raid looting rights with this hit.
    crate::game_loop::command_channel::on_raid_attacked_loot_rights(world, npc_oid, attacker_oid);

    // Quest `onAttack` (Java `addAttackId` scripts, notified from
    // `Attackable.reduceCurrentHp` before any death processing). The acting
    // player is the attacker itself, or — for a servitor/pet blow, Java's
    // `isSummon` branch — its owner.
    let quest_attacker = if world
        .objects
        .has_component::<crate::model::Player>(&attacker_oid)
    {
        Some((attacker_oid, false))
    } else {
        world
            .objects
            .get_component::<crate::model::components::ServitorOf>(&attacker_oid)
            .map(|s| (s.owner_object_id, true))
    };
    if let Some((player_oid, is_summon)) = quest_attacker {
        let npc_id = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .map(|n| n.npc_id)
            .unwrap_or(0);
        let skill_id = world.quest_attack_skill;
        crate::game_loop::quests::notify_attack(
            world, player_oid, npc_oid, npc_id, skill_id, is_summon,
        );
    }
    let Some(region) = world
        .objects
        .get_component::<RegionCell>(&npc_oid)
        .map(|r| r.0)
    else {
        return;
    };

    if became_running {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::change_move_type(npc_oid, true),
        );
    }
    if died {
        crate::game_loop::death::npc_do_die(world, npc_oid, attacker_oid);
        return;
    }
    // `broadcastStatusUpdate` — the HP bar for everyone targeting it.
    broadcast_near_region_in(
        world,
        region,
        instance_of(world, npc_oid),
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
    if world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .is_none_or(|v| v.dead)
    {
        return;
    }
    // `Attackable.addDamageHate` → `MinionList.onAssist`: hitting one member of
    // a pack pulls in the leader and the rest of the escort.
    crate::game_loop::minions::on_assist(world, npc_oid, attacker_oid);
    let now = world.tick;
    let became_running = {
        let Some((mut aggro, mut ai, mut speeds)) =
            world
                .objects
                .get_many_mut::<(&mut AggroList, &mut NpcAi, &mut Speeds)>(&npc_oid)
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
    if became_running
        && let Some(region) = world
            .objects
            .get_component::<RegionCell>(&npc_oid)
            .map(|r| r.0)
    {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::change_move_type(npc_oid, true),
        );
    }
}

/// `PlayerStatus.reduceHp` for a physical hit: CP absorbs first only against
/// playable attackers (mobs bite straight into HP), casts can break
/// (`Formulas.calcAtkBreak`), 0 HP → `doDie`.
/// `PlayerStatus.reduceHp`'s `TRANSFER_DAMAGE_SUMMON_PERCENT` block — returns
/// the damage left for the owner after the servitor's share.
///
/// Java's three guards all matter: there must *be* a first servitor, it must be
/// within 1000 units, and the transfer is clamped to `currentHp - 1` so Transfer
/// Pain can never kill the pet it is protecting you with.
fn transfer_damage_to_servitor(
    world: &mut World,
    player_oid: i32,
    attacker_oid: i32,
    damage: f64,
) -> f64 {
    let percent = world
        .objects
        .get_component::<crate::model::components::StatModifiers>(&player_oid)
        .and_then(|m| {
            m.add
                .get(&crate::model::stats::Stat::TransferDamageSummonPercent)
                .copied()
        })
        .unwrap_or(0.0);
    if percent <= 0.0 {
        return damage;
    }
    let Some(servitor) = crate::game_loop::servitor::servitor_of(world, player_oid) else {
        return damage;
    };
    let in_range = match (
        world
            .objects
            .get_component::<crate::model::components::Position>(&player_oid),
        world
            .objects
            .get_component::<crate::model::components::Position>(&servitor),
    ) {
        (Some(a), Some(b)) => {
            let (dx, dy, dz) = ((a.x - b.x) as f64, (a.y - b.y) as f64, (a.z - b.z) as f64);
            dx * dx + dy * dy + dz * dz <= 1000.0 * 1000.0
        }
        _ => false,
    };
    if !in_range {
        return damage;
    }
    // Java truncates to int on both sides before dividing.
    let mut transferred = ((damage as i32) * (percent as i32)) as f64 / 100.0;
    let servitor_hp = world
        .objects
        .get_component::<Vitals>(&servitor)
        .map(|v| v.cur_hp)
        .unwrap_or(0.0);
    transferred = transferred.min(servitor_hp - 1.0);
    if transferred <= 0.0 {
        return damage;
    }
    npc_receive_damage(world, servitor, attacker_oid, transferred, false);
    damage - transferred
}

pub(crate) fn player_receive_damage(
    world: &mut World,
    player_oid: i32,
    attacker_oid: i32,
    damage: f64,
) {
    player_receive_damage_ex(world, player_oid, attacker_oid, damage, false)
}

/// [`player_receive_damage`] with Java's `directlyToHp` flag exposed —
/// `reduceCurrentHp`'s fifth argument, which `PlayerStatus.reduceHp` reads as
/// "skip the CP pool entirely". Only environmental damage sets it (drowning is
/// the one ported caller); ordinary hits go through the wrapper above.
pub(crate) fn player_receive_damage_ex(
    world: &mut World,
    player_oid: i32,
    attacker_oid: i32,
    damage: f64,
    directly_to_hp: bool,
) {
    // A duel is consequence-free: the losing blow stops at 1 HP and ends the
    // duel instead of killing (Java caps it in the duel damage path, which is
    // why a duel loser stands back up rather than dying).
    if crate::game_loop::duel::duel_lethal_guard(world, attacker_oid, player_oid, damage) {
        return;
    }
    // `PlayerStatus.reduceHp`'s `OFFLINE_MODE_NO_DAMAGE` gate: an unattended
    // shop cannot be hurt at all. Java's condition also re-checks the store
    // type, which is what `is_damage_immune` folds in.
    if crate::game_loop::offline_trade::is_damage_immune(world, player_oid) {
        return;
    }
    // `PlayerStatus.reduceHp`: `if (!isHPConsumption) { if (awake)
    // stopEffectsOnDamage(); … }` — being hit strips every `<removedOnDamage>`
    // buff, which is what wakes a slept player and un-hides a hidden one.
    //
    // Note this is *not* gated on `isDOT` the way the NPC twin in
    // `apply_physical_damage` is: Java puts the player's `stopEffectsOnDamage`
    // above the `if (!isDOT)` block that guards the stun/real-target breaks, so
    // a poison tick wakes a sleeping player even though it would not wake a
    // sleeping mob. Sits above the stand-up below because Java runs it first.
    crate::game_loop::skills::effects::stop_effects_on_damage(world, player_oid);
    // `PlayerStatus.reduceHp`: being hit stands a seated victim up — and a
    // crafter/shopkeeper loses their store with it. This is why you cannot
    // sit-tank.
    if crate::game_loop::sit_stand::is_sitting(world, player_oid) {
        if world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
            .is_some_and(|p| p.store_type != 0)
        {
            crate::game_loop::private_store::close_any_store(world, player_oid);
        }
        crate::game_loop::sit_stand::stand_up(world, player_oid);
    }
    let attacker_is_playable = !is_npc_oid(attacker_oid);
    // `PlayerStatus.reduceHp` wraps its whole attacker-aware block — the CP
    // absorb *and* the `C1_HAS_RECEIVED_S3_DAMAGE_FROM_C2` line — in
    // `if ((attacker != null) && (attacker != getActiveChar()))`. Damage you
    // deal to yourself is silent and bypasses CP.
    //
    // This is not a corner case: the environmental-damage paths all name the
    // victim as their own attacker (Java's `WaterTask` passes `_player`), so
    // without the guard drowning printed "Bob has received 4 damage from Bob"
    // next to its own "unable to breathe" line, and lava let CP soak the tick.
    let attacker_is_other = attacker_oid != player_oid;
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
    // `PlayerStatus.reduceHp`'s transfer block (Transfer Pain 1262), ahead of
    // the CP pool exactly as Java has it: a share of the incoming damage is
    // redirected to the first servitor **within 1000 units**, capped so it can
    // never kill it (`min(summon.getCurrentHp() - 1, tDmg)`), and the
    // player's damage is reduced by whatever actually landed there.
    let damage = transfer_damage_to_servitor(world, player_oid, attacker_oid, damage);
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
        if attacker_is_other && attacker_is_playable && !directly_to_hp {
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

    // Victim-side damage message + stance. Self-inflicted damage says nothing
    // (see `attacker_is_other`) — the environmental sources send their own
    // line instead, e.g. drowning's "you were unable to breathe".
    if let Some(client_id) = client_for_player(world, player_oid).filter(|_| attacker_is_other) {
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
    crate::game_loop::party::notify_party_vitals(world, player_oid);

    if died {
        crate::game_loop::death::player_do_die(world, player_oid, attacker_oid);
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
                (
                    m.add.get(&AttackCancel).copied().unwrap_or(0.0),
                    m.mul.get(&AttackCancel).copied().unwrap_or(1.0),
                )
            })
            .unwrap_or((0.0, 1.0));
        let break_roll = world.roll(100);
        if formulas::calc_atk_break(damage, men_bonus, break_roll, cancel_add, cancel_mul) {
            break_cast(world, player_oid);
            maybe_distance_too_far(world, player_oid);
        }
    }
}
