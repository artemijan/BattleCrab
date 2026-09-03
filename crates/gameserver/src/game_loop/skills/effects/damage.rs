use super::calc_general_trait_bonus;
use super::calc_weakness_bonus;
use super::calc_weapon_trait_bonus;
use super::caster_display_name;
use super::caster_str_bonus;
use super::creature_name;
use super::player_or_npc_level;
use super::pvp_pve_bonus;
use super::record_overhit;
use super::skill_power_mul;
use crate::game_loop::{helpers, npc, skills};
use crate::model::components;

use crate::model::formulas;
use crate::model::skill::active_buff::ActiveBuff;
use crate::model::skill::Skill;
use crate::model::skill::effects::SkillEffect;
use crate::network::server_packets;
use crate::scheduler::ScheduledTask;
use crate::world::World;

/// `calcShldUse` applied to a **skill's** defence term (Java's
/// `PhysicalAttack`/`EnergyAttack`/`calcBlowDamage` all share this shape).
///
/// Returns `None` on a **perfect block**, which the callers turn into a flat
/// **1** damage — Java expresses it as `defence = -1` and then skips the whole
/// damage branch, or `return 1` in `calcBlowDamage`. Otherwise the (possibly
/// shield-augmented) defence.
///
/// The two rolls are consumed even when the target has no shield, matching
/// `calc_shield_use`'s own early return — it is the *rate* that is zero, not
/// the roll that is skipped.
pub(crate) fn defence_after_shield(
    world: &mut World,
    attacker_oid: i32,
    target_oid: i32,
    base_defence: f64,
    ignore_shield_defence: bool,
) -> Option<f64> {
    if ignore_shield_defence {
        return Some(base_defence);
    }
    let (shield_def, shield_rate, con_bonus) =
        crate::game_loop::combat::shield_stats(world, target_oid);
    // `calcShldUse` reads the *attacker's* weapon: a bow raises the block rate
    // by 30 %, on skills exactly as on plain swings — Java takes the flag off
    // `attacker.getAttackType()` with no skill involved either way.
    let ranged = crate::game_loop::combat::ranged::attacker_is_ranged(world, attacker_oid);
    let (rate_roll, perfect_roll) = (world.roll(100), world.roll(100));
    match formulas::calc_shield_use(
        shield_rate,
        con_bonus,
        ranged,
        false,
        rate_roll,
        perfect_roll,
    ) {
        formulas::SHIELD_PERFECT => None,
        formulas::SHIELD_SUCCEED => Some(base_defence + shield_def),
        _ => Some(base_defence),
    }
}

/// The target-side `mDef` for the magic damage formula — players through
/// their stat pipeline, NPCs through the `MDefenseFinalizer` shape
/// (base × MEN bonus × level mod).
pub(crate) fn target_p_def(world: &World, target_oid: i32) -> f64 {
    if let Some(cs) = world
        .objects
        .get_component::<components::CombatStats>(&target_oid)
    {
        return cs.p_def;
    }
    world
        .objects
        .get_component::<crate::model::door::Door>(&target_oid)
        .and_then(|d| world.data.door_data.get(d.door_id))
        .map(|t| (t.p_def as f64).max(1.0))
        .unwrap_or(1.0)
}

/// The trait term every damage formula multiplies in, as one call.
///
/// Java spells it out per handler as three separate factors —
/// `weaponTraitMod · (generalTraitMod == 0 ? 1 : generalTraitMod) · weaknessMod`
/// — and the **`== 0 ? 1`** guard is not decoration: an invulnerable trait
/// zeroes `calcGeneralTraitBonus`, and the damage formulas deliberately treat
/// that as "no modifier" rather than "no damage" (the landing roll is where
/// invulnerability actually bites). `physical` picks whether the weapon term
/// applies: the magic formulas (`calcMagicDam`) have no weapon trait at all.
pub(crate) fn skill_trait_mod(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    physical: bool,
) -> f64 {
    let weapon = if physical {
        calc_weapon_trait_bonus(world, caster_oid, target_oid)
    } else {
        1.0
    };
    let general = calc_general_trait_bonus(world, caster_oid, target_oid, skill.trait_type, true);
    let general = if general == 0.0 { 1.0 } else { general };
    weapon * general * calc_weakness_bonus(world, caster_oid, target_oid, skill.trait_type)
}

/// `Formulas.calcAttributeBonus(attacker, target, skill)` — the elemental
/// damage/land-rate multiplier (PLAN_G19_ATTRIBUTES.md). With a skill element
/// (Volcano FIRE 20): attacker's matching POWER stat + the skill's value vs
/// the target's matching RES. Without one, the attacker's **strongest POWER
/// stat elects the element** (Java `CreatureStat.getAttackElement`'s "temp
/// fix" scan) — which is how Holy Weapon colors an attribute-less skill holy.
/// Nothing elected (both sides bare) → 1.0.
pub(crate) fn attribute_mod(world: &World, caster_oid: i32, target_oid: i32, skill: &Skill) -> f64 {
    attribute_mod_of(
        world,
        caster_oid,
        target_oid,
        skill
            .attribute_type
            .map(|el| (el, skill.attribute_value as f64)),
    )
}

/// `calcAttributeBonus(attacker, target, **null**)` — the same election with
/// no skill to name an element, which is what an **auto-attack** passes. It is
/// not a degenerate case: with no skill element the attacker's strongest
/// POWER stat elects one, so a Holy Weapon buff colours plain swings too.
pub(crate) fn attribute_mod_no_skill(world: &World, caster_oid: i32, target_oid: i32) -> f64 {
    attribute_mod_of(world, caster_oid, target_oid, None)
}

fn attribute_mod_of(
    world: &World,
    caster_oid: i32,
    target_oid: i32,
    skill_element: Option<(crate::model::stats::Element, f64)>,
) -> f64 {
    use crate::model::stats::Element;
    let (attack, element) = match skill_element {
        Some((el, value)) => (element_stat(world, caster_oid, el, false) + value, el),
        None => {
            let mut best: Option<(Element, f64)> = None;
            for el in Element::ALL {
                let v = element_stat(world, caster_oid, el, false);
                if v > best.map_or(0.0, |(_, b)| b) {
                    best = Some((el, v));
                }
            }
            match best {
                Some((el, v)) => (v, el),
                None => return 1.0,
            }
        }
    };
    let defence = element_stat(world, target_oid, element, true);
    formulas::calc_attribute_bonus(attack, defence)
}

/// One element stat (`*_POWER` / `*_RES`) read the `AttributeFinalizer` way:
/// template base (NPCs — players have none), then the merged modifiers.
/// Players read their rebuilt `StatModifiers`; NPCs keep none, so their
/// active buffs are folded on read (the abnormal-flags pattern) — which is
/// what lets Day of Doom's −50s bite a mob.
fn element_stat(
    world: &World,
    oid: i32,
    element: crate::model::stats::Element,
    defence: bool,
) -> f64 {
    let stat = element.attribute_stat(defence);
    let base = npc::npc_template(world, oid)
        .map(|t| {
            if defence {
                t.base_element_res[element.index()] as f64
            } else {
                match t.base_attack_element {
                    Some((el, v)) if el == element => v as f64,
                    _ => 0.0,
                }
            }
        })
        .unwrap_or(0.0);
    if let Some(mods) = world
        .objects
        .get_component::<components::StatModifiers>(&oid)
    {
        return base * mods.mul.get(&stat).copied().unwrap_or(1.0)
            + mods.add.get(&stat).copied().unwrap_or(0.0);
    }
    // NPC: fold the active buffs' stat modifiers for this stat.
    let (mut add, mut mul) = (0.0, 1.0);
    if let Some(buffs) = world.objects.get_component::<components::Buffs>(&oid) {
        for b in &buffs.0 {
            for m in &b.effects {
                if m.stat == stat {
                    match m.mode {
                        crate::model::stats::StatModifierType::Diff => add += m.amount,
                        crate::model::stats::StatModifierType::Per => mul *= 1.0 + m.amount / 100.0,
                    }
                }
            }
        }
    }
    base * mul + add
}

/// The caster's magic attack — the `mAtk` term of every magic formula
/// (`calcMagicDam`, `calcMagicAffected`, `calcHeal`).
///
/// `0.0` for an object with no `CombatStats`, which is what the formulas want:
/// a caster that has left the world contributes nothing rather than panicking
/// mid-cast. The [`target_m_def`] counterpart.
pub(crate) fn caster_m_atk(world: &World, caster_oid: i32) -> f64 {
    world
        .objects
        .get_component::<components::CombatStats>(&caster_oid)
        .map(|c| c.m_atk)
        .unwrap_or(0.0)
}

pub(crate) fn target_m_def(world: &World, target_oid: i32) -> f64 {
    if let Some(cs) = world
        .objects
        .get_component::<components::CombatStats>(&target_oid)
    {
        // Players + NPCs: memoized at spawn through the MDefenseFinalizer shape.
        return cs.m_def;
    }
    // Siege doors carry no `CombatStats` — their mDef is a flat template value.
    if let Some(m_def) = world
        .objects
        .get_component::<crate::model::door::Door>(&target_oid)
        .and_then(|d| world.data.door_data.get(d.door_id))
        .map(|t| (t.m_def as f64).max(1.0))
    {
        return m_def;
    }
    1.0
}

/// `Player.sendDamageMessage`'s crit line: magic skills show `M_CRITICAL`,
/// physical skills `C1_LANDED_A_CRITICAL_HIT` (named after the attacker).
fn crit_message(is_magic: bool, caster_name: &str) -> Vec<u8> {
    use server_packets::{SmParam, sm_ids};
    if is_magic {
        server_packets::system_message_with(sm_ids::M_CRITICAL, &[])
    } else {
        server_packets::system_message_with(
            sm_ids::C1_LANDED_A_CRITICAL_HIT,
            &[SmParam::PlayerName(caster_name.to_string())],
        )
    }
}

/// Port of `Creature.doAttack` → `reduceCurrentHp` for instant skill damage
/// (magic and physical): the caster-side messages here, the victim-side
/// application (CP soak, death, NPC hate/AI wake) shared with the auto-attack
/// path in `combat::apply_physical_damage`'s per-kind receivers. `is_magic`
/// picks the crit line (`Player.sendDamageMessage`: `M_CRITICAL` for magic,
/// `C1_LANDED_A_CRITICAL_HIT` for physical skills).
/// `Formulas.calcCounterAttack` — Shield of Revenge (439) and Counterattack
/// (447), whose `CounterPhysicalSkill` effect grants a **chance** (20 % / 90 %),
/// not a multiplier.
///
/// Two guards decide whether it can fire at all, and both are easy to drop:
/// **only melee skills are counterable** (`skill.isMagic() ||
/// skill.getCastRange() > 40` bails), and the counter is skipped for a dead
/// target and for DoT ticks. The counter damage itself is
/// `target.pAtk * 873 / attacker.pDef`, scaled by the weapon/general trait and
/// attribute bonuses.
///
/// **The bonuses are read in Java's orientation, which is not the damage's.**
/// Java passes `(attacker, target)` to all three — the *attacker* being the one
/// about to take the counter — so the weapon term reads the attacker's weapon
/// against the counter-attacker's resistances even though the damage flows the
/// other way. It is written that way in `Formulas.calcCounterAttack` and the
/// port follows it rather than the orientation that would make physical sense.
/// Note too that this path multiplies **only** the weapon and general trait
/// terms: no `calcWeaknessBonus`, and no `generalTraitMod == 0 ? 1` guard —
/// both of those belong to the `PhysicalAttack` handler family, so the shared
/// `skill_trait_mod` helper is deliberately not used here.
pub(crate) fn calc_counter_attack(
    world: &mut World,
    attacker_oid: i32,
    target_oid: i32,
    skill_id: i32,
    is_dot: bool,
) {
    /// Java `Formulas.MELEE_ATTACK_RANGE`.
    const MELEE_ATTACK_RANGE: i32 = 40;
    if is_dot {
        return;
    }
    let Some(skill) = skills::skill_by_id(world, skill_id, 1) else {
        return;
    };
    if skill.magic_type == 1 || skill.cast_range > MELEE_ATTACK_RANGE {
        return;
    }
    if helpers::is_dead(world, target_oid) {
        return;
    }
    let chance = helpers::stat_add(
        world,
        target_oid,
        crate::model::stats::Stat::VengeanceSkillPhysicalDamage,
    );
    if chance <= 0.0 || (world.roll(100) as f64) >= chance {
        return;
    }
    let (target_p_atk, attacker_p_def) = (
        world
            .objects
            .get_component::<components::CombatStats>(&target_oid)
            .map(|c| c.p_atk)
            .unwrap_or(0.0),
        world
            .objects
            .get_component::<components::CombatStats>(&attacker_oid)
            .map(|c| c.p_def)
            .unwrap_or(0.0)
            .max(1.0),
    );
    let counter = (target_p_atk * 873.0 / attacker_p_def)
        * calc_weapon_trait_bonus(world, attacker_oid, target_oid)
        * calc_general_trait_bonus(world, attacker_oid, target_oid, skill.trait_type, true)
        * attribute_mod(world, attacker_oid, target_oid, &skill);
    if counter <= 0.0 {
        return;
    }
    let (attacker_name, target_name) = (
        creature_name(world, attacker_oid),
        creature_name(world, target_oid),
    );
    helpers::send_sm_to_player(
        world,
        target_oid,
        server_packets::sm_ids::YOU_COUNTERED_C1_S_ATTACK,
        &[server_packets::SmParam::Text(attacker_name)],
    );
    helpers::send_sm_to_player(
        world,
        attacker_oid,
        server_packets::sm_ids::C1_IS_PERFORMING_A_COUNTERATTACK,
        &[server_packets::SmParam::Text(target_name)],
    );
    crate::game_loop::combat::apply_physical_damage(
        world,
        target_oid,
        attacker_oid,
        counter,
        false,
        true,
    );
}

/// The per-hit half of [`apply_skill_damage`]'s arguments.
///
/// Four of them are booleans standing next to each other, which at a call site
/// reads `true, true, &caster_name, skill.over_hit, false` — impossible to
/// check by eye and easy to transpose. Naming them costs nothing, and
/// `..Default::default()` lets each site mention only what it actually varies.
#[derive(Default)]
pub(crate) struct SkillHit<'a> {
    /// Damage before the receiver's own reductions.
    pub damage: f64,
    /// The hit rolled a critical — `mcrit` for magic, `crit` for physical.
    pub crit: bool,
    pub is_magic: bool,
    pub caster_name: &'a str,
    /// Java `AttackableStatus.reduceHp` consults the skill's `<overHit>` here.
    /// Passed explicitly rather than re-read, because the damage value this
    /// needs only exists at the call site.
    pub over_hit: bool,
    /// `CreatureStatus.reduceHp`'s `isDOT` — a DoT tick (and only a DoT tick)
    /// still applies through `HP_BLOCK` (`isHpBlocked() && !(isDOT || …)`).
    /// Every instant-effect call site leaves this `false`; only
    /// `handle_dam_over_time_tick` sets it.
    pub is_dot: bool,
    /// The skill driving this hit, surfaced to quest `onAttack` handlers so they
    /// can distinguish a skill from a melee swing (Java's `onAttack(..., Skill)`).
    pub skill_id: i32,
}

pub(crate) fn apply_skill_damage(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    hit: SkillHit<'_>,
) {
    let SkillHit {
        damage,
        crit,
        is_magic,
        caster_name,
        over_hit,
        is_dot,
        skill_id,
    } = hit;
    record_overhit(world, caster_oid, target_oid, damage, over_hit);
    use server_packets::{SmParam, sm_ids};

    // `Formulas.calcCounterAttack`, which Java runs from `reduceCurrentHp`
    // *before* the damage lands ("Counterattacks happen before damage
    // received") whenever a skill is involved (G34 S4).
    calc_counter_attack(world, caster_oid, target_oid, skill_id, is_dot);

    // A siege door: route the hit straight to the gate's HP (no CP/hate/AI
    // receivers) and refresh its HP bar, then report the damage to the caster.
    if world
        .objects
        .has_component::<crate::model::door::Door>(&target_oid)
    {
        let door_name = world
            .objects
            .get_component::<crate::model::door::Door>(&target_oid)
            .and_then(|d| world.data.door_data.get(d.door_id))
            .map(|t| t.name.clone())
            .unwrap_or_default();
        if crit {
            helpers::send_to_player(world, caster_oid, crit_message(is_magic, caster_name));
        }
        helpers::send_sm_to_player(
            world,
            caster_oid,
            sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
            &[
                SmParam::PlayerName(caster_name.to_string()),
                SmParam::Text(door_name),
                SmParam::Int(damage as i32),
            ],
        );
        crate::game_loop::combat::apply_door_damage(world, target_oid, damage as i32);
        return;
    }

    let target_param = if let Some(p) = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
    {
        SmParam::PlayerName(p.name.clone())
    } else if let Some(t) = npc::npc_template(world, target_oid) {
        SmParam::NpcName(t.id)
    } else {
        return;
    };
    let dmg_int = damage as i32;

    if crit {
        helpers::send_to_player(world, caster_oid, crit_message(is_magic, caster_name));
    }
    helpers::send_sm_to_player(
        world,
        caster_oid,
        sm_ids::C1_HAS_INFLICTED_S3_DAMAGE_ON_C2,
        &[
            SmParam::PlayerName(caster_name.to_string()),
            target_param,
            SmParam::Int(dmg_int),
            // `sendDamageMessage`'s `addPopup(target, attacker, -damage)`
            // — the on-screen floating damage number over the target.
            SmParam::Popup {
                target: target_oid,
                attacker: caster_oid,
                damage: -dmg_int,
            },
        ],
    );

    // Victim-side application: CP soak/HP/death/cast-break for players
    // (including the C1_HAS_RECEIVED message), hate + AI wake + death for
    // NPCs — the same receivers the auto-attack hits go through. The skill id
    // rides on the world for the duration of the hit so quest `onAttack` can
    // read it (Java threads `Skill` straight into the notification).
    world.quest_attack_skill = Some(skill_id);
    crate::game_loop::combat::apply_attack_damage(
        world,
        caster_oid,
        target_oid,
        damage,
        is_dot,
        Some(is_magic),
    );
    world.quest_attack_skill = None;
}

/// Land a buff on an NPC: store it (a re-cast of the same skill replaces the
/// old instance, like `EffectList`'s per-skill slot), recompute its stats, and
/// refresh the buff row in the target window of anyone watching it.
pub(crate) fn apply_buff_to_npc(
    world: &mut World,
    target_oid: i32,
    buff: ActiveBuff,
    skill_id: i32,
) {
    match world
        .objects
        .get_component_mut::<components::Buffs>(&target_oid)
    {
        Some(b) => {
            b.0.retain(|x| x.skill_id != skill_id);
            b.0.push(buff);
        }
        None => return,
    }
    recompute_npc_buffed_stats(world, target_oid);
    broadcast_target_buffs(world, target_oid);
    refresh_summon_info(world, target_oid);
}

/// A **summon** whose stats just changed has to tell the client, or a buff the
/// player cast deliberately appears to do nothing.
///
/// A generic mob doesn't get this: the port never re-broadcasts `NpcInfo` on a
/// buff, so a buffed mob's speed change only shows after respawn. That is
/// tolerable for a mob nobody is watching closely and wrong for a servitor —
/// Servitor Haste (attack speed) and Servitor Wind Walk (movement speed) both
/// land in fields `PetInfo`/`SummonInfo` carry, and both are cast by the owner
/// *expecting* to see the difference.
pub(crate) fn refresh_summon_info(world: &mut World, target_oid: i32) {
    let Some(owner) = world
        .objects
        .get_component::<crate::model::components::ServitorOf>(&target_oid)
        .map(|s| s.owner_object_id)
    else {
        return;
    };
    crate::game_loop::servitor::send_pet_info(
        world,
        owner,
        target_oid,
        crate::game_loop::servitor::PetInfoKind::Default,
    );
    crate::game_loop::servitor::broadcast_summon_info(world, target_oid, false);
}

/// Push a creature's current buffs to every player who has it targeted (Java
/// `EffectList.updateEffectIcons` → `ExAbnormalStatusUpdateFromTarget` to the
/// status listeners) — this is what draws the buff icons under a target's HP
/// bar. Used for NPC targets; players get their own self bar separately.
pub(crate) fn broadcast_target_buffs(world: &mut World, target_oid: i32) {
    let now = world.tick;
    let pkt = match world
        .objects
        .get_component::<components::Buffs>(&target_oid)
    {
        Some(buffs) => crate::network::enter_world::ex_abnormal_status_update_from_target(
            target_oid, buffs, now,
        ),
        None => return,
    };
    let mut observers: Vec<i32> = Vec::new();
    world
        .objects
        .for_each_mut::<(&crate::model::Player, &crate::model::components::TargetRef)>(|(p, t)| {
            if t.0 == Some(target_oid) {
                observers.push(p.object_id);
            }
        });
    for oid in observers {
        helpers::send_to_player(world, oid, pkt.clone());
    }
}

/// Rebuild an NPC's combat stats from its template + current buffs (see
/// `model::recompute_npc_stats_from_buffs`). `world.data` and `world.objects`
/// are disjoint fields, so the template ref and the mutable component borrow
/// coexist.
pub(crate) fn recompute_npc_buffed_stats(world: &mut World, target_oid: i32) {
    let Some(npc_id) = npc::npc_id_of(world, target_oid) else {
        return;
    };
    let Some(t) = world.data.npc_data.get(npc_id) else {
        return;
    };
    // Read the champion flag out before the multi-borrow below: a champion's
    // recomputed stats must keep their multipliers, or the first buff cast on
    // one would quietly strip them back to the ordinary template values.
    // Same for the raid flag: a raid boss (or a raid boss's minion) recomputing
    // after a buff must keep its raid multipliers.
    let champion_mods = crate::model::NpcStatMods::of(
        &world.cfg,
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&target_oid)
            .is_some_and(|n| n.champion),
        t.is_raid() || crate::game_loop::npc::minions::is_raid_minion(world, target_oid),
    );
    if let Some((buffs, mut combat, mut speeds, mut vitals)) = world.objects.get_many_mut::<(
        &components::Buffs,
        &mut components::CombatStats,
        &mut components::Speeds,
        &mut components::Vitals,
    )>(&target_oid)
    {
        crate::model::recompute_npc_stats_from_buffs(
            &world.data,
            t,
            buffs,
            champion_mods,
            &mut combat,
            &mut speeds,
            &mut vitals,
        );
    }
}

/// Recompute a player's max HP/MP/CP from base + CON/MEN + gear + the current
/// buff modifier maps — Java's `Max{Hp,Mp,Cp}Finalizer`, which run inside the
/// same `recalculateStats`. The player's `recalculate_stats` only covers
/// combat/speed stats, so this must be called alongside any buff apply/remove
/// (clan skills, Clan Advent, GM buffs, …) or the HP/MP/CP stat modifiers those
/// carry never move the bar. Current values are only clamped *down* (Java
/// doesn't heal on a max increase). Callers already broadcast UserInfo.
pub(crate) fn recompute_max_vitals(world: &mut World, oid: i32) {
    use crate::model::components::{PlayerVitals, StatModifiers, Vitals};
    use crate::model::inventory::Inventory;
    let Some(p) = world.objects.get_component::<crate::model::Player>(&oid) else {
        return;
    };
    let (level, class_id, base_class_id) = (p.level, p.class_id, p.base_class_id);
    let t = world
        .data
        .player_templates
        .get_or_base(class_id, base_class_id)
        .cloned()
        .unwrap_or_default();
    let (max_hp, max_mp, max_cp) = {
        let Some(mods) = world.objects.get_component::<StatModifiers>(&oid) else {
            return;
        };
        let inv = world.objects.get_component::<Inventory>(&oid);
        (
            crate::model::calc_max_hp(&world.data, &t, level, inv, mods),
            crate::model::calc_max_mp(&world.data, &t, level, inv, mods),
            crate::model::calc_max_cp(&world.data, &t, level, mods),
        )
    };
    if let Some(v) = world.objects.get_component_mut::<Vitals>(&oid) {
        v.max_hp = max_hp as i32;
        v.max_mp = max_mp as i32;
        if v.cur_hp > max_hp {
            v.cur_hp = max_hp;
        }
        if v.cur_mp > max_mp {
            v.cur_mp = max_mp;
        }
    }
    if let Some(pv) = world.objects.get_component_mut::<PlayerVitals>(&oid) {
        pv.max_cp = max_cp as i32;
        if pv.cur_cp > max_cp {
            pv.cur_cp = max_cp;
        }
    }
}

/// `effect.getTicks() * EFFECT_TICK_RATIO` expressed in whole game ticks
/// (`game_loop::TICK` = 100 ms): both the delay to the first DoT tick and the
/// interval between ticks (Java `scheduleAtFixedRate(task, period, period)`).
/// `0` when `ticks <= 0`, which suppresses scheduling.
pub(crate) fn dot_interval_ticks(ticks: i32, ratio_ms: i64) -> u64 {
    if ticks <= 0 || ratio_ms <= 0 {
        return 0;
    }
    (ticks as u64 * ratio_ms as u64) / crate::game_loop::TICK.as_millis() as u64
}

/// Damage per DoT tick: `power * getTicksMultiplier()`, where
/// `getTicksMultiplier() = ticks * EFFECT_TICK_RATIO / 1000`
/// (`AbstractEffect`). Curse Poison lvl 1 (power 11, ticks 5) → `11 * 5 * 666 /
/// 1000 ≈ 36.6` every `5 * 666 = 3330 ms`.
pub(crate) fn dot_tick_damage(power: f64, ticks: i32, ratio_ms: i64) -> f64 {
    power * (ticks as f64 * ratio_ms as f64) / 1000.0
}

/// Arm the first `DamOverTimeTick` for a skill carrying a `DamOverTime` effect
/// (Java `BuffInfo.scheduleEffects`). One recurring task per skill drives all
/// its DoT effects; the cadence comes from the first such effect (Interlude
/// poison/bleed skills carry exactly one). A no-op for skills without a DoT.
pub(crate) fn schedule_dam_over_time(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
) {
    let interval = skill
        .effects
        .iter()
        .find_map(|e| match e {
            SkillEffect::DamOverTime { ticks, .. }
            | SkillEffect::HealOverTime { ticks, .. }
            | SkillEffect::ManaDamOverTime { ticks, .. }
            | SkillEffect::MpConsumePerLevel { ticks, .. }
            | SkillEffect::Relax { ticks, .. }
            | SkillEffect::ChameleonRest { ticks, .. }
            | SkillEffect::ManaHealOverTime { ticks, .. }
            | SkillEffect::Fear { ticks }
            | SkillEffect::FakeDeath { ticks, .. }
                if *ticks > 0 =>
            {
                Some(dot_interval_ticks(
                    *ticks,
                    world.cfg.character.effect_tick_ratio_ms,
                ))
            }
            _ => None,
        })
        .unwrap_or(0);
    if interval == 0 {
        return;
    }
    world.scheduler.schedule(
        world.tick + interval,
        ScheduledTask::DamOverTimeTick {
            caster: caster_oid,
            target: target_oid,
            skill_id: skill.id,
            skill_level: skill.level,
        },
    );
}

pub(crate) fn broadcast_vitals(world: &World, target_oid: i32) {
    if let Some(v) = world
        .objects
        .get_component::<components::Vitals>(&target_oid)
        .copied()
    {
        helpers::send_to_player(
            world,
            target_oid,
            server_packets::status_update(
                target_oid,
                &[
                    (server_packets::status_update_type::CUR_HP, v.cur_hp as i32),
                    (server_packets::status_update_type::CUR_MP, v.cur_mp as i32),
                ],
            ),
        );
    }
    crate::game_loop::party::notify_party_vitals(world, target_oid);
}

/// `PhysicalAttack.instant()` — crit is rolled here (per-effect in Java), not
/// the once-per-cast magic roll. `hp_link` is `PhysicalAttackHpLink`'s tail:
/// the same formula with one extra multiplier at the end, keyed on the
/// **caster's** missing HP — at full health the multiplier is 0, so Fatal
/// Counter fired by a healthy archer does nothing at all.
#[allow(clippy::too_many_arguments)]
pub(crate) fn physical_attack(
    world: &mut World,
    caster_oid: i32,
    target_oid: i32,
    skill: &Skill,
    ss: bool,
    power: f64,
    p_atk_mod: f64,
    p_def_mod: f64,
    critical_chance: f64,
    ignore_shield_defence: bool,
    hp_link: bool,
) {
    let (p_atk, level, str_bonus, random_dmg, caster_name) = {
        let cs = world
            .objects
            .get_component::<components::CombatStats>(&caster_oid);
        let p_atk = cs.map(|c| c.p_atk).unwrap_or(0.0);
        let random_dmg = cs.map(|c| c.random_dmg).unwrap_or(0);
        let str_bonus = caster_str_bonus(world, caster_oid);
        (
            p_atk,
            player_or_npc_level(world, caster_oid),
            str_bonus,
            random_dmg,
            caster_display_name(world, caster_oid),
        )
    };
    // Java folds `pDefMod` in *before* the shield add, so the shield's own
    // sDef is never scaled by it.
    let base_defence = target_p_def(world, target_oid) * p_def_mod;
    let defence = defence_after_shield(
        world,
        caster_oid,
        target_oid,
        base_defence,
        ignore_shield_defence,
    );
    let crit = formulas::calc_physical_skill_crit(critical_chance, str_bonus, world.roll(100));
    let rand_roll = if random_dmg > 0 {
        world.roll(2 * random_dmg + 1) - random_dmg
    } else {
        0
    };
    // A perfect block is a flat 1, whatever the rest would say.
    let damage = match defence {
        None => 1.0,
        Some(defence) => {
            // `weaponMod` is **70 with a `+pAtk+power` bonus term** for a
            // ranged weapon, 77 for melee — the difference between an
            // archer's skill and a swordsman's.
            let ranged = crate::game_loop::combat::ranged::is_ranged(
                crate::game_loop::combat::ranged::equipped_weapon_type(world, caster_oid)
                    .unwrap_or_default(),
            );
            formulas::calc_physical_skill_damage(
                p_atk,
                p_atk_mod,
                defence,
                1.0, // already folded into `defence` above
                power,
                formulas::level_mod(level),
                formulas::random_damage_multiplier(rand_roll),
                crit,
                crate::game_loop::combat::crit_damage_skill(world, caster_oid, target_oid, false),
                ss,
                // `Stat.SHOTS_BONUS` — the enchant-scaled shot multiplier
                // (`ShotsBonusFinalizer`), read live off the attacker.
                crate::game_loop::combat::shots_bonus_of(world, caster_oid),
                ranged,
            ) * attribute_mod(world, caster_oid, target_oid, skill)
                * skill_trait_mod(world, caster_oid, target_oid, skill, true)
                * skill_power_mul(world, caster_oid, false)
                * pvp_pve_bonus(world, caster_oid, target_oid, Some(skill))
        }
    };
    let damage = if hp_link {
        let v = world
            .objects
            .get_component::<components::Vitals>(&caster_oid)
            .copied();
        match v {
            Some(v) if v.max_hp > 0 => damage * (-((v.cur_hp * 2.0) / v.max_hp as f64) + 2.0),
            _ => damage,
        }
    } else {
        damage
    };
    apply_skill_damage(
        world,
        caster_oid,
        target_oid,
        SkillHit {
            damage,
            crit,
            caster_name: &caster_name,
            over_hit: skill.over_hit,
            skill_id: skill.id,
            ..Default::default()
        },
    );
}
