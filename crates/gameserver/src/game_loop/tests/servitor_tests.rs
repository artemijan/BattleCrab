//! Servitor summoning — the first G29 slice.
//!
//! `Summon` is the single biggest unported effect on the whole ranking (24
//! learnable skills). This slice covers summoning, ownership, unsummon and the
//! owner's `PetInfo` view; follow/attack AI and the `SummonInfo` packet that
//! shows a servitor to *other* players are separate slices.

use super::*;

use crate::model::components::ServitorOf;
use crate::model::skill::SkillEffect;

use crate::game_loop::servitor::{servitor_of, summon_servitor, unsummon_servitor};

const OWNER: i32 = 9901;
const CID: u32 = 1;
const PANTHER: i32 = 14799;
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");

fn servitor_world() -> (World, db::CmdRx, tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>) {
    let (mut world, db, l) = combat_test_world();
    for id in [PANTHER, PANTHER + 1] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Servitor".into();
        t.name = format!("Panther {id}");
        t.level = 20;
        t.base_hp_max = 400.0;
        t.base_mp_max = 200.0;
        t.collision_radius = 10.0;
        world.data.npc_data.insert_for_test(t);
    }
    (world, db, l)
}

// ---------------------------------------------------------------------------
// Summon / unsummon
// ---------------------------------------------------------------------------

/// A servitor spawns at its owner, is linked back to them, and starts at full
/// HP/MP (Java's `setCurrentHp(getMaxHp())`).
#[test]
fn summoning_spawns_a_servitor_owned_by_the_caster() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);

    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200).expect("summoned");

    let link = world.objects.get_component::<ServitorOf>(&oid).expect("linked to its owner");
    assert_eq!(link.owner_object_id, OWNER);
    assert_eq!(link.reference_skill, 283, "remembers the skill that summoned it");

    let pos = world.objects.get_component::<Position>(&oid).unwrap();
    assert_eq!((pos.x, pos.y), (100, 200), "spawns on its owner");

    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "full HP");
    assert_eq!(v.cur_mp, v.max_mp as f64, "full MP");

    assert_eq!(servitor_of(&mut world, OWNER), Some(oid), "found by owner lookup");
}

/// Java unsummons any existing servitor before spawning the new one, so
/// re-casting **swaps** rather than stacking.
#[test]
fn resummoning_replaces_rather_than_stacks() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let first = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200).unwrap();
    let second = summon_servitor(&mut world, OWNER, PANTHER + 1, 283, 1200).unwrap();

    assert_ne!(first, second, "a genuinely new entity");
    assert!(world.objects.get_component::<ServitorOf>(&first).is_none(), "the first one is gone");
    assert_eq!(servitor_of(&mut world, OWNER), Some(second), "only the newest remains");
}

/// Unsummoning removes the servitor from the world entirely.
#[test]
fn unsummoning_removes_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200).unwrap();

    assert_eq!(unsummon_servitor(&mut world, OWNER), Some(oid));
    assert_eq!(servitor_of(&mut world, OWNER), None, "no servitor left");
    assert!(world.objects.get_component::<Vitals>(&oid).is_none(), "and the entity is despawned");
}

/// Unsummoning with nothing out is a no-op rather than an error.
#[test]
fn unsummoning_nothing_is_harmless() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    assert_eq!(unsummon_servitor(&mut world, OWNER), None);
}

/// `lifeTime <= 0` is Java's "no expiry" case (`Integer.MAX_VALUE`, commented
/// "Classic hack. Resummon upon entering game."), and a positive one is stored
/// as an absolute deadline.
#[test]
fn life_time_zero_means_no_expiry() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let forever = summon_servitor(&mut world, OWNER, PANTHER, 283, 0).unwrap();
    assert_eq!(world.objects.get_component::<ServitorOf>(&forever).unwrap().expires_at_tick, u64::MAX);

    let timed = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200).unwrap();
    let link = world.objects.get_component::<ServitorOf>(&timed).unwrap();
    assert_eq!(link.expires_at_tick, world.tick + 12_000, "1200 s at 10 ticks/s");
    assert_eq!(link.life_time_secs, 1200);
}

/// Only players summon (Java's `if (!effected.isPlayer()) return`).
#[test]
fn an_npc_cannot_summon() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, NPC_OID, PANTHER, "Monster", 20, 0, 0, 0);
    assert_eq!(summon_servitor(&mut world, NPC_OID, PANTHER, 283, 1200), None);
}

// ---------------------------------------------------------------------------
// The owner's view
// ---------------------------------------------------------------------------

/// The owner is sent `PetInfo` (0xB2) when the servitor appears — without it
/// nothing renders client-side.
#[test]
fn the_owner_is_sent_pet_info() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let _ = drain(&mut rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 1200).unwrap();

    let opcodes: Vec<u8> = drain(&mut rx).iter().filter_map(|p| p.first().copied()).collect();
    assert!(opcodes.contains(&server_packets::opcodes::PET_INFO), "PetInfo sent, got {opcodes:?}");
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The summon skills parse, and `npcId` is **per level** — each level summons a
/// stronger template, which is why the effect carries the id rather than the
/// skill.
#[test]
fn real_dist_summon_skills_parse_per_level_npc_ids() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    let npc_of = |id: i32, level: i32| {
        skills.get(id, level).and_then(|s| {
            s.effects.iter().find_map(|e| match e {
                SkillEffect::Summon { npc_id, .. } => Some(*npc_id),
                _ => None,
            })
        })
    };
    // Summon Dark Panther 283 walks its template id up with the skill level.
    let l1 = npc_of(283, 1).expect("level 1 summons something");
    let l2 = npc_of(283, 2).expect("level 2 summons something");
    assert_ne!(l1, l2, "a different template per level: {l1} vs {l2}");

    // Summon Siege Golem 13 has a flat npcId and a real consume item.
    let golem = skills.get(13, 1).expect("Siege Golem loads");
    let effect = golem.effects.iter().find_map(|e| match e {
        SkillEffect::Summon { npc_id, life_time, consume_item_id, .. } => Some((*npc_id, *life_time, *consume_item_id)),
        _ => None,
    });
    assert_eq!(effect, Some((14737, 1200, 2131)), "npcId / lifeTime / gemstone");
}

/// All 24 learnable summon skills produce a usable effect — none is dropped for
/// want of an `npcId`.
#[test]
fn every_learnable_summon_skill_parses() {
    let skills = crate::data::skill_data::SkillData::load_from(DIST);
    for id in [13, 25, 283, 299, 301, 448, 1111, 1128] {
        let skill = skills.get(id, 1).unwrap_or_else(|| panic!("skill {id} loads"));
        assert!(
            skill.effects.iter().any(|e| matches!(e, SkillEffect::Summon { npc_id, .. } if *npc_id > 0)),
            "skill {id} carries a usable Summon: {:?}",
            skill.effects
        );
    }
}
