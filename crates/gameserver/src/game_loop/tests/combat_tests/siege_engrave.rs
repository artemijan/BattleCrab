//! Taking a castle the way Java does: the attacking clan's leader casts an
//! Imprint on the throne-room Holy Artifact (`HOLYTHING` target →
//! `PossessHolything` → `TakeCastle` → `Castle.engrave`). Plus the other
//! selection-testing target types that share its resolver.

use super::*;
use crate::game_loop::skills::cast::{resolve_cast_target, selection_fits, use_magic_on};
use crate::model::castle::CastleSide;
use crate::model::siege::{Siege, SiegeClanType};
use crate::model::skill::target::TargetType;
use crate::network::server_packets::sm_ids;

const LEADER: i32 = 8003;
const CID: u32 = 1;
const ARTIFACT: i32 = NPC_OID + 20;
const IMPRINT_OF_LIGHT: i32 = 19034;
/// `hitTime` 120 000 ms, and a little over.
const IMPRINT_CAST_TICKS: u64 = 1300;

/// Castle 3 under siege by clan 700, whose leader stands 50 units from the
/// Holy Artifact with Imprint of Light — the real dist skill — in their book.
fn engrave_world() -> (World, UnboundedReceiver<bytes::Bytes>) {
    let (mut world, _db, _l) = cast_test_world();
    insert_siege_zone(&mut world, 3, -1000, 1000, -1000, 1000);
    world.castles = vec![castle_row(3, "Giran")];
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    world.clans.insert(
        500,
        Clan {
            castle_id: 3,
            ..test_clan(500, 8002)
        },
    );
    world.clans.insert(
        700,
        Clan {
            name: "Attackers".into(),
            members: vec![clan_leader(LEADER)],
            ..test_clan(700, LEADER)
        },
    );
    add_test_npc(&mut world, ARTIFACT, 35147, "Artefact", 20, 50, 0, 0);

    let rx = ingame_caster(&mut world, CID, LEADER, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = 700;
    let imprint = dist::skills()
        .get(IMPRINT_OF_LIGHT, 1)
        .cloned()
        .expect("Imprint of Light");
    world.data.skill_data.insert_for_test(imprint);
    world
        .objects
        .get_component_mut::<SkillBook>(&LEADER)
        .unwrap()
        .0
        .insert(IMPRINT_OF_LIGHT, 1);
    (world, rx)
}

fn cast_imprint(world: &mut World) {
    use_magic_on(
        world,
        CID,
        LEADER,
        IMPRINT_OF_LIGHT,
        false,
        false,
        Some(ARTIFACT),
    );
    advance_ticks(world, IMPRINT_CAST_TICKS);
}

/// The skill data itself: the three castle-taking pieces all parse.
#[test]
fn imprint_of_light_parses_as_an_engraving() {
    use crate::model::skill::condition::SkillCondition;
    use crate::model::skill::effects::SkillEffect;

    let skills = dist::skills();
    let imprint = skills.get(IMPRINT_OF_LIGHT, 1).expect("Imprint of Light");
    assert_eq!(imprint.target_type, TargetType::HolyThing);
    assert!(
        imprint
            .conditions
            .iter()
            .any(|c| matches!(c, SkillCondition::PossessHolything))
    );
    assert!(imprint.effects.iter().any(|e| matches!(
        e,
        SkillEffect::TakeCastle {
            side: CastleSide::Light
        }
    )));
    assert_eq!(
        skills.get(5110, 1).expect("Cannon Fodder").target_type,
        TargetType::Artillery
    );
}

/// The whole path: the leader's finished cast hands the castle to their clan
/// on the Imprint's side, and the siege roles flip.
#[test]
fn engraving_the_holy_artifact_takes_the_castle() {
    let (mut world, mut rx) = engrave_world();
    drain(&mut rx);

    cast_imprint(&mut world);

    assert_eq!(
        world.clans[&700].castle_id, 3,
        "the attacker holds the castle"
    );
    assert_eq!(world.clans[&500].castle_id, 0, "the former owner does not");
    assert_eq!(world.castle(3).unwrap().side, CastleSide::Light);
    let kind_of = |clan: i32| {
        world.sieges[&3]
            .clans
            .iter()
            .find(|c| c.clan_id == clan)
            .map(|c| c.kind)
    };
    assert_eq!(kind_of(700), Some(SiegeClanType::Owner));
    assert_eq!(kind_of(500), Some(SiegeClanType::Attacker));
    assert!(
        has_system_message(&drain(&mut rx), sm_ids::CLAN_S1_HAS_SUCCEEDED_IN_S2),
        "the siege hears who took it"
    );
}

/// Clicking the artifact is just a click — Java's `Artefact` has no
/// `onAction` of its own, and the castle used to change hands right here.
#[test]
fn touching_the_holy_artifact_takes_nothing() {
    let (mut world, _rx) = engrave_world();
    interact_with_npc(&mut world, CID, LEADER, ARTIFACT);
    assert_eq!(world.clans[&700].castle_id, 0);
    assert_eq!(world.clans[&500].castle_id, 3);
}

/// `PossessHolything` turns away everyone but an attacking clan's leader
/// standing at the artifact.
#[test]
fn only_an_attacking_leader_at_the_artifact_may_engrave() {
    // A clan member who is not the leader.
    let (mut world, _rx) = engrave_world();
    world.clans.get_mut(&700).unwrap().leader_id = 9999;
    cast_imprint(&mut world);
    assert_eq!(world.clans[&700].castle_id, 0, "not the leader");

    // A leader whose clan is not attacking this castle.
    let (mut world, _rx) = engrave_world();
    world
        .sieges
        .get_mut(&3)
        .unwrap()
        .clans
        .retain(|c| c.clan_id != 700);
    cast_imprint(&mut world);
    assert_eq!(world.clans[&700].castle_id, 0, "not an attacker");

    // No siege running.
    let (mut world, _rx) = engrave_world();
    world.sieges.get_mut(&3).unwrap().in_progress = false;
    cast_imprint(&mut world);
    assert_eq!(world.clans[&700].castle_id, 0, "no siege in progress");

    // Too far from the artifact (`castRange` 85).
    let (mut world, mut rx) = engrave_world();
    world
        .objects
        .get_component_mut::<Position>(&ARTIFACT)
        .unwrap()
        .x = 600;
    drain(&mut rx);
    cast_imprint(&mut world);
    assert_eq!(world.clans[&700].castle_id, 0, "out of reach");
    assert!(has_system_message(
        &drain(&mut rx),
        sm_ids::THE_DISTANCE_IS_TOO_FAR_AND_SO_THE_CASTING_HAS_BEEN_CANCELLED
    ));
}

/// `HOLYTHING` accepts a Holy Artifact and nothing else, with Java's
/// "incorrect target" rather than the generic invalid-target line.
#[test]
fn holything_resolves_only_to_an_artifact() {
    let (mut world, _rx) = engrave_world();
    let mob = NPC_OID + 21;
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 60, 0, 0);
    let imprint = world
        .data
        .skill_data
        .get(IMPRINT_OF_LIGHT, 1)
        .unwrap()
        .clone();
    let caster = world
        .objects
        .get_component::<Player>(&LEADER)
        .unwrap()
        .clone();
    let pos = *world.objects.get_component::<Position>(&LEADER).unwrap();
    let resolve = |target: Option<i32>| {
        resolve_cast_target(&world, &caster, &pos, target, &imprint, false, false)
    };

    assert_eq!(resolve(Some(ARTIFACT)), Ok(ARTIFACT));
    assert_eq!(resolve(Some(mob)), Err(sm_ids::THAT_IS_AN_INCORRECT_TARGET));
    assert_eq!(resolve(None), Err(sm_ids::THAT_IS_AN_INCORRECT_TARGET));
}

/// `ARTILLERY` — the Swoop Cannon's shots: a standing, HP-showing door of a
/// castle under siege, attacked by someone outside the owning clan.
#[test]
fn artillery_accepts_only_a_besieged_enemy_gate() {
    use crate::data::door_data::DoorOpenMethod;

    let (mut world, _rx) = engrave_world();
    // Before the doors: a door takes the next runtime object id, and a
    // fixture NPC placed on that id afterwards would replace it.
    let mob = NPC_OID + 21;
    add_test_npc(&mut world, mob, 20001, "Monster", 5, 60, 0, 0);
    let gate_template = |id: i32, y: i32, show_hp: bool| {
        let mut t = test_door(id, DoorOpenMethod::None);
        t.y = y;
        t.show_hp = show_hp;
        t
    };
    let gate = crate::model::door::spawn_door_for_test(&mut world, gate_template(9001, 0, true));
    let plain = crate::model::door::spawn_door_for_test(&mut world, gate_template(9002, 50, false));
    let breached =
        crate::model::door::spawn_door_for_test(&mut world, gate_template(9003, 100, true));
    world
        .objects
        .get_component_mut::<crate::model::door::Door>(&breached)
        .unwrap()
        .current_hp = 0;
    let mut outside = gate_template(9004, 0, true);
    outside.x = 5000;
    let outside = crate::model::door::spawn_door_for_test(&mut world, outside);

    let fits = |world: &World, caster: i32, target: i32| {
        selection_fits(world, caster, target, TargetType::Artillery)
    };
    assert!(fits(&world, LEADER, gate));
    assert!(!fits(&world, LEADER, plain), "a door that shows no HP");
    assert!(!fits(&world, LEADER, breached), "a breached gate");
    assert!(!fits(&world, LEADER, outside), "a door outside the siege");
    assert!(!fits(&world, LEADER, mob), "not a door at all");
    assert!(!fits(&world, LEADER, ARTIFACT));
    assert!(!fits(&world, mob, gate), "doors can't be attacked by NPCs");

    // The owning clan's own cannon does not fire on its own gate.
    world
        .objects
        .get_component_mut::<Player>(&LEADER)
        .unwrap()
        .clan_id = 500;
    assert!(!fits(&world, LEADER, gate));
}

/// The rest of the family: an outpost for `ADVANCE_BASE`, nothing at all for
/// `FORTRESS_FLAGPOLE` (no FORT zones here), and the three handlers that
/// resolve to null.
#[test]
fn the_remaining_narrow_target_types() {
    let (mut world, _rx) = engrave_world();
    let outpost = NPC_OID + 22;
    add_test_npc(&mut world, outpost, 36590, "Monster", 5, 60, 0, 0);

    assert!(selection_fits(
        &world,
        LEADER,
        outpost,
        TargetType::AdvanceBase
    ));
    assert!(!selection_fits(
        &world,
        LEADER,
        ARTIFACT,
        TargetType::AdvanceBase
    ));
    assert!(!selection_fits(
        &world,
        LEADER,
        outpost,
        TargetType::FortressFlagpole
    ));
    world
        .objects
        .get_component_mut::<Vitals>(&outpost)
        .unwrap()
        .dead = true;
    assert!(
        !selection_fits(&world, LEADER, outpost, TargetType::AdvanceBase),
        "a destroyed outpost"
    );

    for t in [
        TargetType::Item,
        TargetType::MyMentor,
        TargetType::WyvernTarget,
    ] {
        assert!(t.resolves_to_nothing());
    }
    assert!(!TargetType::HolyThing.resolves_to_nothing());
}
