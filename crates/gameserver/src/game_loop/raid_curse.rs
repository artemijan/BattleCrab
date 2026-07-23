//! The raid curse — port of the `CommonSkill.RAID_CURSE`/`RAID_CURSE2` checks
//! in `Attackable.reduceCurrentHp` and `Creature`'s post-cast block.
//!
//! An anti-farming rule, not a difficulty one: a player **more than 8 levels
//! above** a raid boss who attacks it, or casts near it while it is fighting,
//! is punished. It exists to stop a high-level character trivialising a raid
//! for a low-level party.
//!
//! Two skills, both already in the datapack with ported effects:
//!
//! | skill | effects | duration | when |
//! |---|---|---|---|
//! | 4215 `RAID_CURSE`  | `Mute` + `PhysicalMute` | 3600 s | casting a **good** skill nearby |
//! | 4515 `RAID_CURSE2` | `BlockActions`          | 120 s  | attacking it, or casting a **bad** skill nearby |

use crate::model::Player;
use crate::world::World;

/// `CommonSkill.RAID_CURSE` — silence, for helping from a distance.
const RAID_CURSE: i32 = 4215;
/// `CommonSkill.RAID_CURSE2` — petrification, for laying hands on the boss.
const RAID_CURSE2: i32 = 4515;

/// Java's `> 8`, i.e. nine levels above. Written as Java writes it so the
/// off-by-one is not reintroduced by "improving" it to `>= 9`.
const LEVEL_GAP: i32 = 8;

/// `Creature.giveRaidCurse()` — true for a raid boss or grand boss, and for a
/// **raid minion**, which inherits the answer from its master
/// (`Monster.giveRaidCurse`). An ordinary monster never curses.
pub(crate) fn gives_raid_curse(world: &World, npc_oid: i32) -> bool {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return false;
    };
    if is_raid_template(world, npc.npc_id) {
        return true;
    }
    // A raid minion defers to its master, so a boss's adds curse too.
    world
        .objects
        .get_component::<crate::game_loop::minions::MinionOf>(&npc_oid)
        .and_then(|m| world.objects.get_component::<crate::model::npc::Npc>(&m.0))
        .is_some_and(|master| is_raid_template(world, master.npc_id))
}

fn is_raid_template(world: &World, npc_id: i32) -> bool {
    world
        .data
        .npc_data
        .get(npc_id)
        .is_some_and(|t| matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss"))
}

/// The shared gate: curse enabled, the NPC curses, the actor is a player, and
/// they are more than 8 levels above it.
fn should_curse(world: &World, npc_oid: i32, player_oid: i32) -> bool {
    if world.cfg.npc.disable_raid_curse {
        return false;
    }
    if !gives_raid_curse(world, npc_oid) {
        return false;
    }
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return false;
    };
    let boss_level = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .and_then(|n| world.data.npc_data.get(n.npc_id))
        .map(|t| t.level)
        .unwrap_or(0);
    p.level > boss_level + LEVEL_GAP
}

/// `Attackable.reduceCurrentHp` — an over-levelled attacker is petrified.
///
/// Java's own comment: *"In retail you deal damage to raid before curse."* So
/// this is called **after** the damage lands, and the hit that triggers the
/// curse still counts.
pub(crate) fn on_raid_attacked(world: &mut World, npc_oid: i32, attacker_oid: i32) {
    if !should_curse(world, npc_oid, attacker_oid) {
        return;
    }
    apply_curse(world, npc_oid, attacker_oid, RAID_CURSE2);
}

/// The post-cast check: a player casting **near** a raid boss that is in
/// combat is cursed even without touching it — silenced for a good skill,
/// petrified for a bad one.
///
/// Java scans every `Attackable` within `ALT_PARTY_RANGE` of the caster, so
/// helping a low-level party from outside the fight is caught too. The boss
/// must be `isInCombat()`: casting near an idle boss is free.
pub(crate) fn on_skill_cast_near_raid(world: &mut World, caster_oid: i32, skill_is_bad: bool) {
    if world.cfg.npc.disable_raid_curse {
        return;
    }
    if world.objects.get_component::<Player>(&caster_oid).is_none() {
        return;
    }
    // `forEachVisibleObjectInRange(this, Attackable.class, ALT_PARTY_RANGE, …)`
    // — the helper has no radius term (Java's visible-object scan is
    // region-based), so the range is applied here.
    let nearby = crate::game_loop::helpers::visible_creatures(world, caster_oid);
    for npc_oid in nearby {
        if !within(world, caster_oid, npc_oid, PARTY_RANGE) {
            continue;
        }
        if !in_combat(world, npc_oid) || !should_curse(world, npc_oid, caster_oid) {
            continue;
        }
        let curse = if skill_is_bad {
            RAID_CURSE2
        } else {
            RAID_CURSE
        };
        apply_curse(world, npc_oid, caster_oid, curse);
        return; // one curse is enough
    }
}

/// `Config.ALT_PARTY_RANGE`, the radius Java's visible-object scan uses.
const PARTY_RANGE: f64 = 1500.0;

fn within(world: &World, a: i32, b: i32, range: f64) -> bool {
    use crate::model::components::Position;
    let (Some(pa), Some(pb)) = (
        world.objects.get_component::<Position>(&a),
        world.objects.get_component::<Position>(&b),
    ) else {
        return false;
    };
    let (dx, dy) = ((pa.x - pb.x) as f64, (pa.y - pb.y) as f64);
    (dx * dx + dy * dy).sqrt() <= range
}

/// A boss only curses while it is actually fighting (`isInCombat`).
fn in_combat(world: &World, npc_oid: i32) -> bool {
    world
        .objects
        .get_component::<crate::model::npc::AggroList>(&npc_oid)
        .is_some_and(|a| !a.0.is_empty())
}

/// `curse.getSkill().applyEffects(attackable, player)` — the **boss** is the
/// caster, so the debuff cannot be resisted as if the victim had cast it on
/// themselves, and its landing rate reads the boss's level.
fn apply_curse(world: &mut World, npc_oid: i32, player_oid: i32, skill_id: i32) {
    let Some(skill) = world.data.skill_data.get(skill_id, 1).cloned() else {
        return;
    };
    crate::game_loop::skills::effects::apply_continuous_effects(
        world, npc_oid, player_oid, &skill, None,
    );
}
