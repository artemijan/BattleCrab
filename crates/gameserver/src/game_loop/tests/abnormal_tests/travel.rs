//! Escape scrolls and chant of gate.

use super::*;

/// **`CallParty`** (Chant of Gate 1429) — recall every *other* party member to
/// the caster. Two halves matter: it is **not** Summon Friend, so there is no
/// `ConfirmDlg` and the members get no say; and each one is gated by CallPc's
/// shared `checkSummonTargetStatus`, whose refusals are messaged to the
/// **caster**, not the member left behind.
#[test]
fn chant_of_gate_recalls_the_party_but_not_someone_in_combat() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 1000, 1000);
    let willing = CASTER + 1;
    let fighting = CASTER + 2;
    let _w = ingame_player(&mut world, CID + 1, willing, 0, 0, 0);
    let _f = ingame_player(&mut world, CID + 2, fighting, 50, 50, 0);
    let mut skill = cc_skill(9421, SkillEffect::CallParty, "NONE");
    skill.target_type = TargetType::Self_;
    world.data.skill_data.insert_for_test(skill);
    make_party(&mut world, &[CASTER, willing, fighting], LootRule::Random);

    // One member is in combat — `isInCombat()` is the attack stance.
    combat::refresh_attack_stance(&mut world, fighting);

    land(&mut world, 9421, CASTER);

    let pos = |world: &World, oid: i32| {
        world
            .objects
            .get_component::<Position>(&oid)
            .map(|p| (p.x, p.y))
            .unwrap()
    };
    assert_eq!(
        pos(&world, willing),
        (1000, 1000),
        "the willing member is pulled to the caster, no dialog asked"
    );
    assert_eq!(
        pos(&world, fighting),
        (50, 50),
        "the one in combat stays put"
    );
    assert_eq!(
        pos(&world, CASTER),
        (1000, 1000),
        "and the caster does not recall themselves"
    );
}

/// **`Teleport`** — the destination Scrolls of Escape. 107 reachable skills
/// carried this effect and the parser did not know the name, so every one of
/// them loaded with an **empty effect list**: the scroll was consumed, the cast
/// animated, and nothing happened. Note the destination is per skill *level* —
/// skill 2213 alone carries 22 towns, one per level.
#[test]
fn every_destination_escape_scroll_now_carries_a_teleport() {
    use crate::model::skill::effects::SkillEffect as E;

    let skills = dist::skills();
    // Two levels of the same scroll must give two *different* destinations.
    let lv1 = skills.get(2213, 1).expect("SoE lv1");
    let lv2 = skills.get(2213, 2).expect("SoE lv2");
    let coords = |s: &Skill| {
        s.effects.iter().find_map(|e| match e {
            E::Teleport { x, y, z } => Some((*x, *y, *z)),
            _ => None,
        })
    };
    let (a, b) = (coords(lv1), coords(lv2));
    assert!(a.is_some(), "the scroll carries a Teleport at all");
    assert_ne!(
        a, b,
        "and the destination is keyed on the skill level, not shared"
    );
    assert_eq!(
        a,
        Some((-114558, 253605, -1536)),
        "Talking Island, straight out of the datapack"
    );
}

/// And it has to actually move the player — an effect that parses and is never
/// applied is the failure this epic keeps finding.
#[test]
fn a_scroll_of_escape_moves_the_caster() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.data.skill_data.insert_for_test(cc_skill(
        9440,
        SkillEffect::Teleport {
            x: 12_345,
            y: -6_789,
            z: -1_000,
        },
        "NONE",
    ));

    land(&mut world, 9440, CASTER);

    let pos = pos_of(&world, CASTER).unwrap();
    assert_eq!(
        (pos.0, pos.1),
        (12_345, -6_789),
        "the scroll actually moves you"
    );
    // `teleport_player` settles z onto the ground, so the destination z is a
    // request rather than a literal — assert the neighbourhood, not the value.
    assert!(
        (pos.2 - (-1_000)).abs() <= 64,
        "and lands near the requested height, got {}",
        pos.2
    );
}
