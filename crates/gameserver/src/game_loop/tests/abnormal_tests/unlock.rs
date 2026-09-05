//! Unlock: which doors and chests it opens, the level bands, and the drop
//! tables a smashed and an unlocked chest do not share.

use super::*;

/// **`OpenDoor`** — the lock-picking half of Unlock (27). Three outcomes, and
/// the two refusals are different messages for different reasons: a door that
/// is not `openMethod="BY_SKILL"` cannot be picked *at all* ("this door cannot
/// be unlocked"), while a `BY_SKILL` door that fails its roll gets the softer
/// "you have failed to unlock the door" and can be tried again.
#[test]
fn unlock_picks_a_by_skill_door_and_refuses_the_rest() {
    use crate::data::door_data::DoorOpenMethod;

    let (mut world, _db, _l) = cc2_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9403,
        SkillEffect::OpenDoor {
            chance: 50,
            is_item: false,
        },
        "NONE",
    ));
    let pickable =
        model::door::spawn_door_for_test(&mut world, test_door(9901, DoorOpenMethod::BySkill));
    let mut plain = test_door(9902, DoorOpenMethod::ByClick);
    plain.x = 400;
    let plain_oid = model::door::spawn_door_for_test(&mut world, plain);

    // A `BY_CLICK` door: refused outright, with its own message, and the roll
    // is never reached.
    drain(&mut out);
    world.clear_forced_rolls();
    world.force_rolls([0; 4]);
    land(&mut world, 9403, plain_oid);
    let pkts = drain(&mut out);
    assert!(
        has_system_message(&pkts, server_packets::sm_ids::THIS_DOOR_CANNOT_BE_UNLOCKED),
        "a door that is not BY_SKILL cannot be picked at all"
    );
    assert!(!world.geo.doors.is_open(9902), "and it stays shut");

    // A `BY_SKILL` door with a failing roll: the softer message, still shut.
    world.clear_forced_rolls();
    world.force_rolls([50; 4]);
    land(&mut world, 9403, pickable);
    let pkts = drain(&mut out);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::YOU_HAVE_FAILED_TO_UNLOCK_THE_DOOR
        ),
        "a missed roll says so"
    );
    assert!(!world.geo.doors.is_open(9901), "and the door is still shut");

    // Same door, a roll under the chance: it opens.
    world.clear_forced_rolls();
    world.force_rolls([10; 4]);
    land(&mut world, 9403, pickable);
    assert!(world.geo.doors.is_open(9901), "a passing roll opens it");
}

/// **`OpenChest`** — the treasure-box half of the same skill, gated by a
/// *level band* rather than a roll. Inside the band the box pops open: it dies
/// without paying exp/sp and is flagged `specialDrop` so it rolls its own list
/// rather than the smashed-box one. Outside it, the box turns on you.
///
/// Note the reachability: **no `type="Chest"` NPC is spawned anywhere on this
/// dist**, so the only way to meet one today is `//spawn`. The effect is
/// ported anyway — a datapack with chest spawns is a data change, not a code
/// change.
#[test]
fn unlocking_a_chest_depends_on_the_level_band() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9404, SkillEffect::OpenChest, "NONE"));

    let in_band = NPC_OID;
    let out_of_band = NPC_OID + 1;
    // 18265 is a real `type="Chest"` template on this dist, so its **level
    // comes from the datapack** — `add_test_npc`'s level argument only applies
    // to ids it has to invent. The caster's level is therefore the variable
    // here, which is also the honest reading: the band is a gap, not a floor.
    add_test_npc(&mut world, in_band, 18265, "Chest", 25, 100, 0, 0);
    add_test_npc(&mut world, out_of_band, 18265, "Chest", 25, 150, 0, 0);
    let chest_level = effects::creature_level_for_test(&world, in_band);
    let set_caster_level = |world: &mut World, level: i32| {
        if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
            p.level = level;
        }
    };

    // Five levels below the chest: inside the 6-level band, so it opens.
    set_caster_level(&mut world, chest_level - 5);
    land(&mut world, 9404, in_band);
    assert!(
        world
            .objects
            .get_component::<Vitals>(&in_band)
            .is_some_and(|v| v.dead),
        "a box within the band is opened — which kills it"
    );
    let npc = world
        .objects
        .get_component::<model::npc::Npc>(&in_band)
        .expect("chest");
    assert!(npc.special_drop, "and it rolls its own drop list");
    assert!(!npc.must_reward_exp_sp, "but pays no exp/sp");

    // Twenty levels below: outside the band, so it refuses and aggroes.
    set_caster_level(&mut world, chest_level - 20);
    land(&mut world, 9404, out_of_band);
    assert!(
        world
            .objects
            .get_component::<Vitals>(&out_of_band)
            .is_some_and(|v| !v.dead),
        "a box outside the band is not opened"
    );
    assert!(
        world
            .objects
            .get_component::<AggroList>(&out_of_band)
            .and_then(|a| a.0.get(&CASTER).map(|i| i.hate))
            .unwrap_or(0.0)
            > 0.0,
        "it turns on the caster instead"
    );
}

/// The other half of `OpenChest`, on the **death** side: an unlocked box pays
/// no exp/sp and keeps its own drop list, while a box that was merely beaten
/// to death rolls a *different* npc id's list (Java `Chest.doItemDrop`).
///
/// A dist finding recorded by this test: the ids that remap points at —
/// 21801-21822 and the six 216xx/217xx ones — **do not exist** in this
/// datapack, and the chest templates carry no `<drops>` of their own either.
/// In Java that null template reaches `calculateDrops` and throws; here the
/// swap simply falls back to the chest's own (empty) list, which is the only
/// non-crashing reading of the same code.
#[test]
fn a_smashed_chest_and_an_unlocked_one_do_not_share_a_drop_table() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(9405, SkillEffect::OpenChest, "NONE"));
    // A real experience table (the fixture ships an empty one, where the level
    // cap makes *every* award clamp to -1 and an exp assertion would pass no
    // matter what the gate does) and a chest that actually pays exp — 18265
    // declares no `<acquire>` at all, so with the dist's own 0 there would be
    // nothing for the gate to withhold.
    world.data.experience =
        crate::data::ExperienceData::from_table(vec![0, 0, 1000, 2000, 3000, 4000, 5000], 6);
    let chest = NPC_OID;
    add_test_npc(&mut world, chest, 18265, "Chest", 25, 100, 0, 0);
    {
        let mut t = world.data.npc_data.get(18265).cloned().expect("Chest");
        t.exp = 500.0;
        t.sp = 50.0;
        world.data.npc_data.insert_for_test(t);
    }
    let template = world
        .data
        .npc_data
        .get(18265)
        .cloned()
        .expect("18265 is a Chest on this dist");

    // The dist finding: 18265 + 3536 = 21801, and **21801 is not a template
    // on this datapack** — nor is any of the six 216xx/217xx ids the fixed
    // pairs map onto. Java hands that null straight to `calculateDrops` and
    // throws; here the swap finds nothing and the caller falls back to the
    // chest's own (also empty) list.
    assert!(
        world.data.npc_data.get(21801).is_none(),
        "the remap target does not exist on this dist — recorded, not assumed"
    );
    assert!(
        crate::game_loop::death::chest_drop_template_for_test(&world, chest, &template).is_none(),
        "so a smashed chest falls back to its own list"
    );

    // Give 21801 a template and the redirect is visible: the *mechanism* is
    // what this asserts, independently of whether this dist ships the target.
    let mut mimic = crate::data::npc_data::default_template(21801);
    mimic.type_name = "Monster".into();
    world.data.npc_data.insert_for_test(mimic);
    assert_eq!(
        crate::game_loop::death::chest_drop_template_for_test(&world, chest, &template)
            .map(|t| t.id),
        Some(21801),
        "a chest that was not unlocked rolls 21801's drop list, not its own"
    );

    // Unlock it, and the swap stops applying at all.
    let exp_before = world
        .objects
        .get_component::<Player>(&CASTER)
        .map(|p| p.exp)
        .unwrap_or(0);
    let chest_level = effects::creature_level_for_test(&world, chest);
    if let Some(p) = world.objects.get_component_mut::<Player>(&CASTER) {
        p.level = chest_level;
    }
    land(&mut world, 9405, chest);
    assert!(
        crate::game_loop::death::chest_drop_template_for_test(&world, chest, &template).is_none(),
        "an unlocked chest always rolls its own list"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&CASTER)
            .map(|p| p.exp)
            .unwrap_or(0),
        exp_before,
        "and pays no exp — `setMustRewardExpSp(false)`"
    );
}
