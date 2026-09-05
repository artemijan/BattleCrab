//! Taking a castle: the ownership transfer and endsiege, the reputation cap,
//! blood alliance, the ticket reset, the noble diary entry, and the crest a
//! tax-zone NPC then wears.

use super::*;

/// Castle 3 with clan 500 owning it and clan 700 registered as an attacker —
/// the shape both the capture test and the reputation-settlement tests want.
fn siege_world_for_capture() -> (World, UnboundedReceiver<db::DbCommand>, ()) {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::{Siege, SiegeClanType};
    let (mut world, _db_tx, db_rx, _link) = test_world();
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
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    let clan = |id: i32, name: &str, leader: i32, castle: i32| Clan {
        id,
        name: name.into(),
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
    };
    world.clans.insert(500, clan(500, "Defenders", 8002, 3));
    world.clans.insert(700, clan(700, "Attackers", 8003, 0));
    (world, db_rx, ())
}

/// Mid-siege capture transfers castle ownership to the attacker and reshuffles
/// siege roles; endSiege then declares the new owner victorious. Port of Java
/// Siege capture (midVictory) + endSiege victory determination.
#[test]
fn siege_capture_transfers_ownership_and_endsiege_declares_victor() {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::{Siege, SiegeClanType};
    let (mut world, _db_tx, mut db_rx, _link) = test_world();
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
    siege.add_clan(500, SiegeClanType::Owner); // defender/owner
    siege.add_clan(700, SiegeClanType::Attacker); // attacker
    world.sieges.insert(3, siege);
    let clan = |id: i32, name: &str, leader: i32, castle: i32| Clan {
        id,
        name: name.into(),
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
    };
    world.clans.insert(500, clan(500, "Defenders", 8002, 3)); // owns castle 3
    world.clans.insert(700, clan(700, "Attackers", 8003, 0));
    let mut rx = ingame_player(&mut world, 1, 8002, 0, 0, 0); // hears the announcements
    drain(&mut rx);

    crate::game_loop::siege::start_siege(&mut world, 3);
    assert_eq!(
        world.sieges[&3].first_owner_clan_id, 500,
        "first owner captured at start"
    );
    drain(&mut rx);
    drain_db(&mut db_rx);

    // Capture by attacker clan 700.
    crate::game_loop::siege::capture(&mut world, 3, 700);
    assert_eq!(world.clans[&700].castle_id, 3, "captor now owns the castle");
    assert_eq!(world.clans[&500].castle_id, 0, "old owner lost the castle");
    let role = |cid: i32| {
        world.sieges[&3]
            .clans
            .iter()
            .find(|c| c.clan_id == cid)
            .map(|c| c.kind)
    };
    assert_eq!(
        role(700),
        Some(SiegeClanType::Owner),
        "captor is the new owner side"
    );
    assert_eq!(
        role(500),
        Some(SiegeClanType::Attacker),
        "old owner becomes an attacker"
    );
    let cmds = drain_db(&mut db_rx);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::UpdateClanCastle {
                clan_id: 700,
                castle_id: 3
            }
        )),
        "captor persisted"
    );
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            db::DbCommand::UpdateClanCastle {
                clan_id: 500,
                castle_id: 0
            }
        )),
        "old owner cleared"
    );

    // Give the former owner some reputation, so the settlement below has
    // something to take and to cap against.
    world.clans.get_mut(&500).unwrap().reputation_score = 2000;

    // endSiege → the captor (owner changed) is declared victorious.
    crate::game_loop::siege::end_siege(&mut world, 3);
    assert!(!world.sieges[&3].in_progress, "siege ended");
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::CLAN_S1_IS_VICTORIOUS_OVER_S2_S_CASTLE_SIEGE),
        "victor announced"
    );

    // `Castle.updateClansReputation`: the former owner is docked
    // `LooseCastlePoints` (3000) and the captor gains
    // `min(TakeCastlePoints, what the loser had)` — 1500 vs 2000, so the full
    // 1500. **Clan reputation goes negative**: Java's `setReputationScore`
    // has an explicit arm for crossing below zero (it strips the clan skills),
    // so 2000 - 3000 = -1000 rather than a floor at 0.
    assert_eq!(
        world.clans[&500].reputation_score, -1000,
        "former owner docked LooseCastlePoints, into the negative"
    );
    assert_eq!(
        world.clans[&700].reputation_score, 1500,
        "captor gains TakeCastlePoints, capped by the loser's balance"
    );
}

/// **Taking a castle off a bankrupt clan pays nothing.** Java caps the
/// captor's gain at `maxreward` — the former owner's score *before* it is
/// docked — so `min(TakeCastlePoints, 0)` is 0. Without the cap the captor
/// would be paid 1500 out of thin air.
#[test]
fn castle_capture_reputation_is_capped_by_what_the_loser_had() {
    let (mut world, _db_rx, _l) = siege_world_for_capture();
    world.clans.get_mut(&500).unwrap().reputation_score = 0;
    crate::game_loop::siege::start_siege(&mut world, 3);
    crate::game_loop::siege::capture(&mut world, 3, 700);
    crate::game_loop::siege::end_siege(&mut world, 3);
    assert_eq!(
        world.clans[&700].reputation_score, 0,
        "nothing to take, so nothing paid"
    );
}

/// **Holding your own castle pays `CastleDefendedPoints`.** The other arm of
/// the settlement — Java's `else` when the owner has not changed.
#[test]
fn a_successful_defence_pays_castle_defended_points() {
    let (mut world, _db_rx, _l) = siege_world_for_capture();
    world.clans.get_mut(&500).unwrap().reputation_score = 100;
    crate::game_loop::siege::start_siege(&mut world, 3);
    // No capture: the defenders hold.
    crate::game_loop::siege::end_siege(&mut world, 3);
    assert_eq!(
        world.clans[&500].reputation_score,
        100 + 750,
        "defenders paid CastleDefendedPoints"
    );
}

/// A siege-end helper world: castle 3 (with `tickets` placed) owned by clan
/// 500, attacker clan 700, siege started so `first_owner_clan_id == 500`.
#[cfg(test)]
fn siege_end_world(tickets: i32) -> (World, UnboundedReceiver<db::DbCommand>) {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::{Siege, SiegeClanType};
    let (mut world, _db_tx, db_rx, _link) = test_world();
    world.castles = vec![Castle {
        show_npc_crest: false,
        id: 3,
        name: "Giran".into(),
        side: CastleSide::Neutral,
        ticket_buy_count: tickets,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }];
    let mut siege = Siege::new(3);
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    let clan = |id: i32, castle: i32| Clan {
        id,
        name: format!("Clan{id}"),
        leader_id: id * 10,
        level: 5,
        reputation_score: 0,
        castle_id: castle,
        members: vec![ClanMember {
            char_id: id * 10,
            name: format!("P{id}"),
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
    world.clans.insert(500, clan(500, 3));
    world.clans.insert(700, clan(700, 0));
    crate::game_loop::siege::start_siege(&mut world, 3);
    assert_eq!(world.sieges[&3].first_owner_clan_id, 500);
    (world, db_rx)
}

/// **When the defenders hold their castle, the owner gets the blood-alliance
/// reward and the ticket count is left alone** (Java `endSiege`'s
/// owner-unchanged branch). The reward is 0 on this dist, so the count stays 0;
/// the untouched ticket count is what distinguishes this from the capture path.
#[test]
fn siege_defenders_hold_awards_blood_alliance() {
    let (mut world, mut db_rx) = siege_end_world(5);
    // No capture — clan 500 still owns castle 3 at the end.
    crate::game_loop::siege::end_siege(&mut world, 3);

    assert_eq!(
        world.clans[&500].blood_alliance_count,
        crate::game_loop::siege::BLOOD_ALLIANCE_REWARD,
        "the defender was awarded the blood-alliance reward"
    );
    assert_eq!(
        world.castles[0].ticket_buy_count, 5,
        "the ticket count is untouched when the owner is unchanged"
    );
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            db::DbCommand::UpdateClanBloodAlliance { clan_id: 500, .. }
        )),
        "the blood-alliance count was persisted"
    );
}

/// **When an attacker captures the castle, its mercenary ticket count is reset
/// to 0** (Java `endSiege`'s owner-changed branch → `setTicketBuyCount(0)`), and
/// the captor gets no blood-alliance reward.
#[test]
fn siege_capture_resets_ticket_count() {
    let (mut world, mut db_rx) = siege_end_world(5);
    crate::game_loop::siege::capture(&mut world, 3, 700);
    drain_db(&mut db_rx);
    crate::game_loop::siege::end_siege(&mut world, 3);

    assert_eq!(
        world.castles[0].ticket_buy_count, 0,
        "the captured castle's ticket count is cleared"
    );
    assert_eq!(
        world.clans[&700].blood_alliance_count, 0,
        "the captor gets no blood-alliance reward"
    );
    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            db::DbCommand::UpdateCastleTicketCount {
                castle_id: 3,
                count: 0
            }
        )),
        "the reset ticket count was persisted"
    );
}

/// **When a castle is captured, the captor's online noble members get a
/// `heroes_diary` "castle taken" entry** (Java `endSiege` → `Hero.setCastleTaken`).
/// A non-noble member gets nothing.
#[test]
fn capturing_a_castle_diaries_the_captors_nobles() {
    let (mut world, mut db_rx) = siege_end_world(0);
    // Clan 700's member (char id 7000) is online; make it a noble.
    let _rx = ingame_player(&mut world, 9, 7000, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&7000)
        .unwrap()
        .is_noble = true;

    crate::game_loop::siege::capture(&mut world, 3, 700);
    drain_db(&mut db_rx);
    crate::game_loop::siege::end_siege(&mut world, 3);

    assert!(
        drain_db(&mut db_rx).iter().any(|c| matches!(
            c,
            db::DbCommand::SaveHeroDiary {
                char_id: 7000,
                action: 3, // ACTION_CASTLE_TAKEN
                param: 3,  // castle id
                ..
            }
        )),
        "the captor's noble got a castle-taken diary entry"
    );
}

/// A non-noble captor gets no diary entry (the `isNoble` gate).
#[test]
fn a_non_noble_captor_gets_no_diary_entry() {
    let (mut world, mut db_rx) = siege_end_world(0);
    let _rx = ingame_player(&mut world, 9, 7000, 0, 0, 0); // online but not noble
    crate::game_loop::siege::capture(&mut world, 3, 700);
    drain_db(&mut db_rx);
    crate::game_loop::siege::end_siege(&mut world, 3);

    assert!(
        !drain_db(&mut db_rx)
            .iter()
            .any(|c| matches!(c, db::DbCommand::SaveHeroDiary { .. })),
        "a non-noble captor is not diaried"
    );
}

/// **Mid-victory's tail** (Java `Siege.midVictory`): the capture does not just
/// swap ownership. The new attackers are thrown out of the castle, the captor's
/// own base camp comes down (`removeDefenderFlags` runs *after* the reshuffle,
/// so the flags it strips are the new defenders' — i.e. the captor's), and the
/// control/flame towers are torn down and rebuilt with the count reset to 0.
#[test]
fn siege_capture_evicts_the_new_attackers_and_rebuilds_the_towers() {
    use model::castle::{Castle, CastleSide};
    use model::clan::{Clan, ClanMember};
    use model::siege::{Siege, SiegeClanType, SiegeSpawn};
    const ROOT: &str = crate::data::DIST_GAME;

    let (mut world, _db_tx, mut db_rx, _link) = test_world();
    // The eviction lands the player in a town, so the real region table has to
    // be there — an empty one resolves no respawn point and nobody moves.
    world.data.map_region = crate::data::MapRegionData::load_from(ROOT);
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
    let clan = |id: i32, name: &str, leader: i32, castle: i32| Clan {
        id,
        name: name.into(),
        leader_id: leader,
        level: 5,
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
        reputation_score: 0,
        castle_id: castle,
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
    world.clans.insert(500, clan(500, "Defenders", 8002, 3));
    world.clans.insert(700, clan(700, "Attackers", 8003, 0));

    // One control tower and one flame tower for castle 3.
    let tower = |npc_id: i32| SiegeSpawn {
        npc_id,
        x: 100,
        y: 100,
        z: 0,
        heading: 0,
    };
    for (npc_id, ty) in [(13007, "ControlTower"), (13004, "FlameTower")] {
        let mut t = crate::data::npc_data::default_template(npc_id);
        t.type_name = ty.into();
        t.base_hp_max = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }
    world
        .data
        .siege_towers
        .insert(3, vec![tower(13007), tower(13004)]);

    let mut siege = Siege::new(3);
    siege.in_progress = true;
    siege.add_clan(500, SiegeClanType::Owner);
    siege.add_clan(700, SiegeClanType::Attacker);
    world.sieges.insert(3, siege);
    // A tower set is standing, and the attacker has a base camp planted.
    crate::game_loop::siege::spawn_towers_for_test(&mut world, 3);
    let before: Vec<i32> = world.sieges[&3].spawned_npcs.clone();
    assert_eq!(before.len(), 2, "two towers stand");
    assert_eq!(world.sieges[&3].control_tower_count, 1);
    // Leave a *stale, non-zero* count going in: only an explicit reset to 0
    // before the respawn brings it back to one tower rather than adding to it.
    world.sieges.get_mut(&3).unwrap().control_tower_count = 5;
    world.sieges.get_mut(&3).unwrap().flags.push((700, 91_001));

    let mut rx = ingame_player(&mut world, 1, 8003, 0, 0, 0); // the captor's leader
    world
        .objects
        .get_component_mut::<Player>(&8003)
        .unwrap()
        .clan_id = 700;
    let mut def_rx = ingame_player(&mut world, 2, 8002, 0, 0, 0); // a defender, inside
    world
        .objects
        .get_component_mut::<Player>(&8002)
        .unwrap()
        .clan_id = 500;
    drain(&mut rx);
    drain(&mut def_rx);
    drain_db(&mut db_rx);

    crate::game_loop::siege::capture(&mut world, 3, 700);

    // The captor's base camp is gone — you don't keep an HQ once you own the
    // castle.
    assert!(
        world.sieges[&3].flags.is_empty(),
        "removeDefenderFlags stripped the captor's flag"
    );
    // Towers rebuilt, and the count restarted from 0 rather than accumulating.
    let after: Vec<i32> = world.sieges[&3].spawned_npcs.clone();
    assert_eq!(after.len(), 2, "a fresh tower set stands");
    assert!(
        after.iter().all(|oid| !before.contains(oid)),
        "they are new objects, not the old ones: {before:?} vs {after:?}"
    );
    assert_eq!(
        world.sieges[&3].control_tower_count, 1,
        "the count was reset to 0 before the respawn, not added to"
    );
    // The ex-defender (now an attacker) was evicted.
    assert!(
        drain(&mut def_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::TELEPORT_TO_LOCATION),
        "the new attackers are teleported out"
    );
}

/// The castle-crest chain (`Npc.onSpawn` → `NpcInfo` CLAN): a Folk NPC
/// spawning in an owned castle's TAX zone records the owner clan when the
/// display is on (`castle.show_npc_crest` here), and `npc_clan_block`
/// resolves the crest ids only inside a peace zone. Off = the dist default:
/// nothing recorded at all. A capture resets the flag like
/// `Castle.setOwner`.
#[test]
fn tax_zone_npc_wears_owner_crest_when_display_is_on() {
    use crate::data::zone_data::{Zone, ZoneKind};
    use crate::game_loop;
    use model::castle::{Castle, CastleSide};

    let (mut world, _db_rx, _link_rx) = combat_test_world();
    let tax_zone = |castle_id: i32| Zone {
        id: 0,
        name: format!("test_tax_{castle_id}"),
        kind: ZoneKind::Tax,
        territory: Territory {
            form: crate::data::spawn_data::ZoneForm::Cuboid {
                x1: -500,
                x2: 500,
                y1: -500,
                y2: 500,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
        mother_tree: None,
    };
    world.data.zone_data.insert(tax_zone(3));
    insert_zone(&mut world, ZoneKind::Peace, -500, 500, -500, 500);
    world.castles = vec![Castle {
        show_npc_crest: true,
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
    let clan = Clan {
        id: 500,
        name: "Owners".into(),
        leader_id: 5000,
        level: 5,
        reputation_score: 0,
        castle_id: 3,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 77,
        ally_name: "Ally".into(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 11,
        crest_large_id: 12,
        ally_crest_id: 13,
        blood_alliance_count: 0,
    };
    world.clans.insert(500, clan);
    let mut t = crate::data::npc_data::default_template(30099);
    t.type_name = "Folk".into();
    world.data.npc_data.insert_for_test(t);

    let npc = game_loop::npc::spawn_npc_at(&mut world, 30099, 0, 0, 0, 0).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<model::npc::Npc>(&npc)
            .unwrap()
            .crest_clan_id,
        500,
        "the tax-zone spawn recorded the owner clan"
    );
    assert_eq!(
        visibility::npc_clan_block(&world, npc),
        Some([500, 11, 12, 77, 13]),
        "NpcInfo's CLAN block resolves the crest ids"
    );

    // The dist default: display off → nothing recorded at spawn.
    world.castles[0].show_npc_crest = false;
    let plain = game_loop::npc::spawn_npc_at(&mut world, 30099, 10, 10, 0, 0).unwrap();
    assert_eq!(
        world
            .objects
            .get_component::<model::npc::Npc>(&plain)
            .unwrap()
            .crest_clan_id,
        0,
        "with both gate halves off (the dist default) no crest is recorded"
    );

    // A change of hands resets the flag, like `Castle.setOwner`.
    world.castles[0].show_npc_crest = true;
    let mut siege = model::siege::Siege::new(3);
    siege.in_progress = true;
    world.sieges.insert(3, siege);
    crate::game_loop::siege::capture(&mut world, 3, 500);
    assert!(
        !world.castles[0].show_npc_crest,
        "capture ran Castle.setOwner's setShowNpcCrest(false)"
    );
}
