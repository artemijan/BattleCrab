//! Residential skills: what each castle grants, and how capturing or losing
//! one moves them.

use super::*;

const DIST_RES: &str = crate::data::DIST_GAME;

/// Build a clan that owns `castle` with a single online member `leader`.
#[cfg(test)]
fn owner_clan(id: i32, leader: i32, castle: i32) -> Clan {
    use crate::model::clan::{Clan, ClanMember};
    Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: leader,
        level: 5,
        reputation_score: 0,
        castle_id: castle,
        members: vec![ClanMember {
            char_id: leader,
            name: format!("P{leader}"),
            level: 40,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 1,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        }],
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
        blood_alliance_count: 0,
    }
}

/// A residence skill 593 gated to residence 3, with no social-class gate.
#[cfg(test)]
fn residence_learn() -> crate::data::pledge_skill_tree::PledgeSkillLearn {
    crate::data::pledge_skill_tree::PledgeSkillLearn {
        skill_id: 593,
        skill_level: 1,
        get_level: 4,
        social_class: None,
        residencial: true,
        residence_ids: vec![3],
        level_up_sp: 0,
    }
}

#[cfg(test)]
fn has_clan_skill(world: &World, oid: i32, id: i32) -> bool {
    world
        .objects
        .get_component::<model::components::skills::ClanSkills>(&oid)
        .is_some_and(|c| c.0.contains_key(&id))
}

/// **Residential skills load per residence** — castle 1 grants Residence Health
/// (593); an unknown residence grants nothing (Java `getAvailableResidentialSkills`).
#[test]
fn residential_skills_load_per_castle() {
    let trees = crate::data::pledge_skill_tree::PledgeSkillTreeData::load_from(DIST_RES);
    let ids: Vec<i32> = trees
        .available_residential_skills(1)
        .iter()
        .map(|l| l.skill_id)
        .collect();
    assert!(
        ids.contains(&593),
        "castle 1 grants Residence Health: {ids:?}"
    );
    assert!(
        trees.available_residential_skills(999).is_empty(),
        "an unknown residence grants nothing"
    );
}

/// **A castle-owning clan's member gets the castle's residential skills on
/// login, and loses them when the castle is gone** (Java `Player.enterWorld` +
/// `AbstractResidence.removeResidentialSkills`).
#[test]
fn residential_skills_granted_on_login_and_stripped() {
    let (mut world, mut db_rx, _link) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(593));
    world
        .data
        .pledge_skill_trees
        .insert_for_test(residence_learn(), false);
    let _a = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    drain_db(&mut db_rx);
    let clan_id = 0x3000_0056;
    world.clans.insert(clan_id, owner_clan(clan_id, 3001, 3));
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = clan_id;

    clans::apply_clan_skills_to_member(&mut world, clan_id, 3001);
    assert!(
        has_clan_skill(&world, 3001, 593),
        "a castle-owning clan member gets the residential skill on login"
    );

    clans::remove_residential_skills(&mut world, 3001, 3);
    assert!(
        !has_clan_skill(&world, 3001, 593),
        "losing the castle strips the residential skill"
    );
}

/// **Capturing a castle moves its residential skills** — the former owner's
/// online member loses them, the captor's gains them (Java `Castle.setOwner`).
#[test]
fn capturing_a_castle_moves_residential_skills() {
    use crate::model::siege::{Siege, SiegeClanType};
    let (mut world, mut db_rx, _link) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(593));
    world
        .data
        .pledge_skill_trees
        .insert_for_test(residence_learn(), false);
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    // Defender clan 500 owns castle 3; attacker clan 700 owns nothing. Both
    // leaders online.
    let _def = ingame_player(&mut world, 1, 8002, 0, 0, 0);
    let _atk = ingame_player(&mut world, 2, 8003, 0, 0, 0);
    world.clans.insert(500, owner_clan(500, 8002, 3));
    world.clans.insert(700, owner_clan(700, 8003, 0));
    for (oid, cid) in [(8002, 500), (8003, 700)] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .unwrap()
            .clan_id = cid;
    }
    // The defender already holds the skill (granted while owning).
    clans::give_residential_skills(&mut world, 8002, 3, 500);
    assert!(
        has_clan_skill(&world, 8002, 593),
        "defender holds it pre-capture"
    );
    drain_db(&mut db_rx);

    crate::game_loop::siege::capture(&mut world, 3, 700);

    assert!(
        !has_clan_skill(&world, 8002, 593),
        "the former owner's member loses the residential skill"
    );
    assert!(
        has_clan_skill(&world, 8003, 593),
        "the captor's member gains it"
    );
}

// --- G18.6: academy graduation, restrictions and mentorship ----------------

/// **Residential skills follow clan membership, not just login.** A member who
/// joins a castle-owning clan gets them at once (Java `addClanMember` →
/// `addSkillEffects`), and a member who leaves loses them with the clan
/// (`setClan(null)` → `removeResidentialSkills`) — otherwise a one-day
/// membership would leave the buff on them for good.
#[test]
fn residential_skills_follow_joining_and_leaving() {
    let (mut world, mut db_rx, _link) = quest_test_world();
    world
        .data
        .skill_data
        .insert_for_test(passive_clan_test_skill(593));
    world
        .data
        .pledge_skill_trees
        .insert_for_test(residence_learn(), false);
    let _leader = ingame_player(&mut world, 1, 3001, 0, 0, 0);
    let _recruit = ingame_player(&mut world, 2, 3003, 0, 0, 0);
    install_clan(&mut world, 5000, &[3001]);
    // The clan owns castle 3.
    world.clans.get_mut(&5000).unwrap().castle_id = 3;
    drain_db(&mut db_rx);

    assert!(
        !has_clan_skill(&world, 3003, 593),
        "an outsider has nothing"
    );

    // Join.
    clans::handle_request_join_pledge(&mut world, 1, &invite_body(3003, 0));
    clans::handle_request_answer_join_pledge(&mut world, 2, &answer_body(1));
    assert!(
        has_clan_skill(&world, 3003, 593),
        "joining a castle-owning clan grants the residential skill immediately"
    );

    // Leave.
    clans::handle_request_withdrawal_pledge(&mut world, 2);
    assert!(
        !has_clan_skill(&world, 3003, 593),
        "and leaving takes it back"
    );
}
