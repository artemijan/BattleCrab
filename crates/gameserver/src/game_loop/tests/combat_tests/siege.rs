//! The siege itself: the zone, the eviction when it starts, where a defender
//! respawns, and the side each member is stamped with.

use super::*;

/// A `SiegeZone` makes the players inside it mutually auto-attackable — but only
/// while that castle's siege is in progress (Java `SiegeZone` active state).
#[test]
fn siege_zone_makes_participants_attackable_only_during_siege() {
    let (mut world, ..) = test_world();
    // Siege zone for castle 3 covering (0,0)..(1000,1000).
    insert_siege_zone(&mut world, 3, 0, 1000, 0, 1000);
    world.sieges.insert(3, model::siege::Siege::new(3));
    let _a = ingame_player(&mut world, 1, 4001, 500, 500, 0);
    let _b = ingame_player(&mut world, 2, 4002, 510, 510, 0);
    let attackable =
        |w: &World| crate::game_loop::combat::pvp::is_player_auto_attackable(w, 4001, 4002);

    // Zone loaded but siege idle → two unflagged players aren't attackable.
    assert!(!attackable(&world), "no siege PvP while the siege is idle");

    // Siege in progress → both stand in the battlefield → freely attackable.
    world.sieges.get_mut(&3).unwrap().in_progress = true;
    assert!(attackable(&world), "siege PvP once the siege starts");

    // A player outside the siege zone is not part of it (position-based check).
    world
        .objects
        .get_component_mut::<Position>(&4002)
        .unwrap()
        .x = 5000;
    assert!(
        !attackable(&world),
        "outside the siege zone → not attackable"
    );
}

/// Starting a siege evicts everyone in the battlefield except the owning clan
/// to their nearest town (Java teleportPlayer(NotOwner, TOWN)).
#[test]
fn siege_start_evicts_non_owners_to_town() {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::Siege;
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = test_world();
    world.data.map_region = crate::data::MapRegionData::load_from(ROOT);
    insert_siege_zone(&mut world, 3, 0, 1000, 0, 1000);
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: 3,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    world.sieges.insert(3, Siege::new(3));
    // Owner clan 500 holds castle 3.
    world.clans.insert(
        500,
        Clan {
            id: 500,
            name: "Owners".into(),
            leader_id: 9002,
            level: 5,
            reputation_score: 0,
            castle_id: 3,
            members: vec![ClanMember {
                char_id: 9002,
                name: "P9002".into(),
                level: 40,
                class_id: 0,
                sex: 0,
                race: 0,
                power_grade: 5,
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
        },
    );
    let _o = ingame_player(&mut world, 1, 9002, 500, 500, 0); // owner-clan member in the zone
    let _n = ingame_player(&mut world, 2, 9003, 600, 600, 0); // non-owner in the zone
    world
        .objects
        .get_component_mut::<Player>(&9002)
        .unwrap()
        .clan_id = 500;

    crate::game_loop::siege::start_siege(&mut world, 3);

    // Owner-clan member stays in the battlefield.
    let op = *world.objects.get_component::<Position>(&9002).unwrap();
    assert_eq!(
        world.data.zone_data.siege_castle_at(op.x, op.y, op.z),
        Some(3),
        "owner clan holds the castle"
    );
    // Non-owner is teleported out of the siege zone.
    let np = *world.objects.get_component::<Position>(&9003).unwrap();
    assert_ne!(
        world.data.zone_data.siege_castle_at(np.x, np.y, np.z),
        Some(3),
        "non-owner evicted to town"
    );
}

/// Starting a siege spawns the castle's stationed guards onto the battlefield;
/// ending it despawns them. Port of Siege.spawnSiegeGuard / removeSiegeGuards.
#[test]
fn siege_spawns_and_despawns_the_stationed_guards() {
    use model::siege::{Siege, SiegeSpawn};
    let (mut world, ..) = test_world();
    // Register a guard NPC template so spawn_npc_at can build it.
    world
        .data
        .npc_data
        .insert_for_test(crate::data::npc_data::default_template(35085));
    world.sieges.insert(3, Siege::new(3));
    world.siege_guards.insert(
        3,
        vec![
            SiegeSpawn {
                npc_id: 35085,
                x: 100,
                y: 100,
                z: 0,
                heading: 0,
            },
            SiegeSpawn {
                npc_id: 35085,
                x: 200,
                y: 100,
                z: 0,
                heading: 0,
            },
        ],
    );

    // start_siege → both guards spawn as live NPCs, tracked on the siege.
    crate::game_loop::siege::start_siege(&mut world, 3);
    let guard_oids = world.sieges[&3].spawned_npcs.clone();
    assert_eq!(guard_oids.len(), 2, "two stationed guards spawned");
    assert!(
        guard_oids
            .iter()
            .all(|oid| world.objects.has_component::<model::npc::Npc>(oid)),
        "guards are live NPCs"
    );

    // end_siege → the guards are despawned and the list cleared.
    crate::game_loop::siege::end_siege(&mut world, 3);
    assert!(
        world.sieges[&3].spawned_npcs.is_empty(),
        "guard list cleared"
    );
    assert!(
        guard_oids
            .iter()
            .all(|oid| !world.objects.has_component::<model::npc::Npc>(oid)),
        "guards despawned"
    );
}

/// A defender killed during a siege respawns *inside* the castle when it picks
/// "to castle" (type 2 → residence `getSpawnLoc`); "to village" (type 0) still
/// sends it to the map-region town. Java `RequestRestartPoint.portPlayer` — the
/// castle respawn is not gated on the control-tower count (that only blocks
/// resurrection, unported).
#[test]
fn siege_defender_respawns_at_castle_on_to_castle() {
    use model::clan::{Clan, ClanMember};
    use model::siege::Siege;
    let (mut world, _db_rx, _link_rx) = combat_test_world();
    // Town fallback: one region covering the death spot, respawn at (1000, 1000).
    world.data.map_region =
        crate::data::MapRegionData::from_regions(vec![crate::data::map_region::MapRegion {
            name: "test_town".into(),
            loc_id: 0,
            bbs: 0,
            respawn_points: vec![(1000, 1000, 7)],
            tiles: vec![(20, 18)],
        }]);
    insert_siege_zone(&mut world, 3, -1000, 1000, -1000, 1000);
    // The castle's owner restart point (from castle_hall.xml).
    world.data.castle_restart_points.insert(
        3,
        crate::data::castle_zone_data::CastleRespawnPoints {
            spawn: vec![(500, 600, 100)],
            ..Default::default()
        },
    );
    // Clan 700 owns castle 3 and is under siege.
    world.clans.insert(
        700,
        Clan {
            id: 700,
            name: "Defenders".into(),
            leader_id: 3001,
            level: 5,
            reputation_score: 0,
            castle_id: 3,
            members: vec![ClanMember {
                char_id: 3001,
                name: "P3001".into(),
                level: 40,
                class_id: 0,
                sex: 0,
                race: 0,
                power_grade: 5,
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
        },
    );
    let mut siege = Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);

    let _rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .clan_id = 700;
    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .dead = true;

    // "To castle" → respawn inside the castle.
    handle_request_restart_point(&mut world, 1, &restart_to(2));
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (500, 600, 105),
        "defender respawns inside the castle (z +5)"
    );

    // "To village" → the ordinary town respawn (siege role doesn't hijack it).
    world
        .objects
        .get_component_mut::<Vitals>(&3001)
        .unwrap()
        .dead = true;
    world.force_roll(0);
    handle_request_restart_point(&mut world, 1, &restart_to(0));
    let pos = world.objects.get_component::<Position>(&3001).unwrap();
    assert_eq!(
        (pos.x, pos.y, pos.z),
        (1000, 1000, 12),
        "to-village goes to town, not the castle"
    );
}

/// Put two clanned players on castle 3's active battlefield, registered with
/// the given siege roles. Returns the world ready for an attackability check.
#[cfg(test)]
fn siege_sides_world(
    a_kind: model::siege::SiegeClanType,
    b_kind: model::siege::SiegeClanType,
) -> World {
    use model::castle::{Castle, CastleSide};
    use model::clan::Clan;
    use model::siege::Siege;

    let (mut world, ..) = test_world();
    insert_siege_zone(&mut world, 3, 0, 1000, 0, 1000);
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: 3,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    let mut siege = Siege::new(3);
    siege.add_clan(500, a_kind);
    siege.add_clan(700, b_kind);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    for (id, leader) in [(500, 4001), (700, 4002)] {
        let clan = Clan {
            id,
            name: format!("Clan{id}"),
            leader_id: leader,
            level: 5,
            reputation_score: 0,
            castle_id: 0,
            members: vec![model::clan::ClanMember {
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
        };
        world.clans.insert(id, clan);
    }
    let _a = ingame_player(&mut world, 1, 4001, 500, 500, 0);
    let _b = ingame_player(&mut world, 2, 4002, 510, 510, 0);
    world
        .objects
        .get_component_mut::<Player>(&4001)
        .unwrap()
        .clan_id = 500;
    world
        .objects
        .get_component_mut::<Player>(&4002)
        .unwrap()
        .clan_id = 700;
    world
}

/// **Same-side clans don't fight.** Java's `isAutoAttackable` siege block:
/// two defender clans are never attackable to each other, and two *attacker*
/// clans only become attackable once the castle has been engraved once
/// (`isFirstMidVictory`) — until then the besiegers are allies.
#[test]
fn siege_sides_decide_who_may_attack_whom() {
    use model::siege::SiegeClanType::{Attacker, Defender, Owner};
    let attackable =
        |w: &World| crate::game_loop::combat::pvp::is_player_auto_attackable(w, 4001, 4002);

    // Attacker vs defender — the ordinary case, hostile.
    assert!(attackable(&siege_sides_world(Attacker, Defender)));
    assert!(attackable(&siege_sides_world(Attacker, Owner)));

    // Two defenders never fight.
    assert!(
        !attackable(&siege_sides_world(Defender, Defender)),
        "two defender clans are on the same side"
    );
    assert!(
        !attackable(&siege_sides_world(Owner, Defender)),
        "the owner counts as a defender"
    );

    // Two attackers: allies until the castle is engraved…
    let mut world = siege_sides_world(Attacker, Attacker);
    assert!(
        !attackable(&world),
        "besiegers are allies before the first mid victory"
    );
    // …and enemies after.
    world
        .castles
        .iter_mut()
        .find(|c| c.id == 3)
        .unwrap()
        .first_mid_victory = true;
    assert!(
        attackable(&world),
        "once someone has engraved the castle, attackers may fight each other"
    );
}

/// **A siege stamps its side on every online member, and takes it back.** The
/// flags drive the relation icon: same state → the blue ALLY bit, different →
/// red ENEMY, and a besieger additionally carries ATTACKER.
#[test]
fn a_siege_stamps_and_clears_each_members_side() {
    use model::siege::SiegeClanType::{Attacker, Owner};
    let mut world = siege_sides_world(Owner, Attacker);
    // The fixture pre-set `in_progress`; run the real start-of-siege update.
    let state = |w: &World, oid: i32| {
        w.objects
            .get_component::<Player>(&oid)
            .map(|p| (p.siege_state, p.siege_side))
            .unwrap()
    };
    assert_eq!(state(&world, 4001), (0, 0), "no side before the update");

    crate::game_loop::siege::update_player_siege_state_flags(&mut world, 3, false);
    assert_eq!(state(&world, 4001), (2, 3), "the owner defends castle 3");
    assert_eq!(state(&world, 4002), (1, 3), "the other clan attacks it");

    crate::game_loop::siege::update_player_siege_state_flags(&mut world, 3, true);
    assert_eq!(state(&world, 4001), (0, 0), "cleared when the siege ends");
    assert_eq!(state(&world, 4002), (0, 0));
}
