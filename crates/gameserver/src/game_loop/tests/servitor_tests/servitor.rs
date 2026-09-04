//! The servitor itself: summoning and dismissing, following and attack
//! orders, summon info, lifetime and upkeep, the leash, the passive/defending
//! modes, betrayal, and the siege weapon's faster upkeep.

use super::*;

/// A servitor spawns at its owner, is linked back to them, and starts at full
/// HP/MP (Java's `setCurrentHp(getMaxHp())`).
#[test]
fn summoning_spawns_a_servitor_owned_by_the_caster() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 100, 200);

    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).expect("summoned");

    let link = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .expect("linked to its owner");
    assert_eq!(link.owner_object_id, OWNER);
    assert_eq!(
        link.reference_skill, 283,
        "remembers the skill that summoned it"
    );

    let pos = world.objects.get_component::<Position>(&oid).unwrap();
    assert_eq!((pos.x, pos.y), (100, 200), "spawns on its owner");

    let v = world.objects.get_component::<Vitals>(&oid).unwrap();
    assert_eq!(v.cur_hp, v.max_hp as f64, "full HP");
    assert_eq!(v.cur_mp, v.max_mp as f64, "full MP");

    assert_eq!(
        servitor_of(&world, OWNER),
        Some(oid),
        "found by owner lookup"
    );
}

/// Java unsummons any existing servitor before spawning the new one, so
/// re-casting **swaps** rather than stacking.
#[test]
fn resummoning_replaces_rather_than_stacks() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let first = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    let second = summon_servitor(&mut world, OWNER, PANTHER + 1, 283, 1200, 0, 0).unwrap();

    assert_ne!(first, second, "a genuinely new entity");
    assert!(
        world.objects.get_component::<ServitorOf>(&first).is_none(),
        "the first one is gone"
    );
    assert_eq!(
        servitor_of(&world, OWNER),
        Some(second),
        "only the newest remains"
    );
}

/// Unsummoning removes the servitor from the world entirely.
#[test]
fn unsummoning_removes_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    assert_eq!(unsummon_servitor(&mut world, OWNER), Some(oid));
    assert_eq!(servitor_of(&world, OWNER), None, "no servitor left");
    assert!(
        world.objects.get_component::<Vitals>(&oid).is_none(),
        "and the entity is despawned"
    );
}

/// Unsummoning with nothing out is a no-op rather than an error.
#[test]
fn unsummoning_nothing_is_harmless() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    assert_eq!(unsummon_servitor(&mut world, OWNER), None);
}

/// Only players summon (Java's `if (!effected.isPlayer()) return`).
#[test]
fn an_npc_cannot_summon() {
    let (mut world, _db, _l) = servitor_world();
    add_test_npc(&mut world, NPC_OID, PANTHER, "Monster", 20, 0, 0, 0);
    assert_eq!(
        summon_servitor(&mut world, NPC_OID, PANTHER, 283, 1200, 0, 0),
        None
    );
}

// ---------------------------------------------------------------------------
// The owner's view
// ---------------------------------------------------------------------------

/// A fresh servitor follows (Java's `getFollowStatus()` defaults true) and
/// closes the gap when its owner walks away.
#[test]
fn an_idle_servitor_trails_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "follows by default"
    );

    // Owner walks well beyond the follow range.
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 900;
    servitor_follow_tick(&mut world, oid);

    let m = world.objects.get_component::<Movement>(&oid);
    assert!(m.is_some(), "the servitor set off after its owner");
}

/// Inside the follow range it stays put rather than jittering.
#[test]
fn a_servitor_already_close_does_not_move() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 100; // < FOLLOW_RANGE
    servitor_follow_tick(&mut world, oid);
    assert!(
        world.objects.get_component::<Movement>(&oid).is_none(),
        "no pointless walk"
    );
}

/// "Hold your ground" stops the following, and toggling again resumes it.
#[test]
fn hold_toggles_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    assert_eq!(
        servitor_toggle_follow(&mut world, OWNER),
        Some(false),
        "now holding"
    );
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 900;
    servitor_follow_tick(&mut world, oid);
    assert!(
        world.objects.get_component::<Movement>(&oid).is_none(),
        "a holding servitor ignores its owner walking off"
    );

    assert_eq!(
        servitor_toggle_follow(&mut world, OWNER),
        Some(true),
        "and back to following"
    );
    servitor_follow_tick(&mut world, oid);
    assert!(world.objects.get_component::<Movement>(&oid).is_some());
}

/// An ordered attack seeds hate on the target and switches the servitor to the
/// attack intention, which is what the ordinary NPC attack think drives from.
#[test]
fn an_ordered_attack_targets_the_owners_target() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);

    assert!(
        servitor_attack(&mut world, OWNER, FOE),
        "the order was accepted"
    );

    let hate = world
        .objects
        .get_component::<AggroList>(&oid)
        .and_then(|a| a.0.get(&FOE))
        .map(|i| i.hate)
        .unwrap_or(0.0);
    assert!(hate > 0.0, "the target is now hated");
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&oid)
            .unwrap()
            .intention,
        NpcIntention::Attack
    );
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "and it stops trailing, or it would drift home between swings"
    );
}

/// Java refuses an order at a target more than 3000 units from the owner and
/// falls back to following, so a stray click doesn't send the summon across the
/// map.
#[test]
fn a_far_target_is_refused_and_the_servitor_keeps_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 9_000, 0, 0);

    assert!(!servitor_attack(&mut world, OWNER, FOE), "refused");
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "falls back to following"
    );
    assert_eq!(
        world
            .objects
            .get_component::<AggroList>(&oid)
            .map(|a| a.0.len()),
        Some(0),
        "and never took the target"
    );
}

/// "Stop" clears the target, halts movement and resumes following.
#[test]
fn stop_cancels_the_attack_and_resumes_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);
    servitor_attack(&mut world, OWNER, FOE);

    assert!(servitor_stop(&mut world, OWNER));
    assert_eq!(
        world
            .objects
            .get_component::<AggroList>(&oid)
            .map(|a| a.0.len()),
        Some(0)
    );
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&oid)
            .unwrap()
            .intention,
        NpcIntention::Active
    );
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "back to trailing its owner"
    );
}

/// A servitor does **not** hunt on its own — unlike a monster it never seeds
/// hate from an aggro scan, only from its owner's order.
#[test]
fn a_servitor_does_not_pick_its_own_fights() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    // A monster stands right next to it.
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 50, 0, 0);

    advance_world(&mut world, 200);

    assert_eq!(
        world
            .objects
            .get_component::<AggroList>(&oid)
            .map(|a| a.0.len()),
        Some(0),
        "no unbidden aggro"
    );
}

// ---------------------------------------------------------------------------
// Visibility to other players (slice 3)
// ---------------------------------------------------------------------------

/// The owner sees `PetInfo`; **everyone else** sees `SummonInfo` (0x8B). Before
/// this slice a servitor was invisible to every player but its summoner.
#[test]
fn other_players_are_sent_summon_info_and_the_owner_is_not() {
    let (mut world, _db, _l) = servitor_world();
    let mut owner_rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let mut other_rx = ingame_caster(&mut world, 2, OWNER + 1, 60, 0);
    let _ = drain(&mut owner_rx);
    let _ = drain(&mut other_rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    let owner_ops: Vec<u8> = drain(&mut owner_rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    let other_ops: Vec<u8> = drain(&mut other_rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();

    assert!(
        owner_ops.contains(&server_packets::opcodes::PET_INFO),
        "owner gets PetInfo: {owner_ops:?}"
    );
    assert!(
        !owner_ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "and not the bystander packet as well"
    );
    assert!(
        other_ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "others get SummonInfo: {other_ops:?}"
    );
    assert!(
        !other_ops.contains(&server_packets::opcodes::PET_INFO),
        "and never the owner-only one"
    );
}

/// The packet carries the **owner's name** in its title slot — that is what
/// draws the "of X" label under a summon, and it is the field most likely to be
/// wired to the wrong string.
#[test]
fn summon_info_carries_the_owners_name() {
    let (mut world, _db, _l) = servitor_world();
    let _owner_rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let mut other_rx = ingame_caster(&mut world, 2, OWNER + 1, 60, 0);
    let _ = drain(&mut other_rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    let owner_name = world
        .objects
        .get_component::<Player>(&OWNER)
        .unwrap()
        .name
        .clone();
    let pkt = drain(&mut other_rx)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::SUMMON_INFO))
        .expect("SummonInfo sent");
    // The name is UTF-16LE in the packet body.
    let wide: Vec<u8> = owner_name
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    assert!(
        pkt.windows(wide.len()).any(|w| w == wide),
        "the owner's name appears in the packet"
    );
}

/// A servitor that walks into view is introduced with `SummonInfo` too, not
/// `NpcInfo` — the visibility delta path has to make the same choice as the
/// summon path.
#[test]
fn a_servitor_entering_view_is_introduced_as_a_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _owner_rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    // A second player logs in nearby *after* the summon.
    let mut late_rx = ingame_caster(&mut world, 2, OWNER + 1, 60, 0);
    let _ = drain(&mut late_rx);
    visibility::on_enter_world(&world, 2, OWNER + 1);

    let ops: Vec<u8> = drain(&mut late_rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    assert!(
        ops.contains(&server_packets::opcodes::SUMMON_INFO),
        "introduced as a summon: {ops:?}"
    );
}

// ---------------------------------------------------------------------------
// Lifecycle (slice 4)
// ---------------------------------------------------------------------------

/// A no-expiry servitor (`lifeTime <= 0`) is never reaped by the tick.
#[test]
fn a_permanent_servitor_is_never_reaped() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    world.tick += 10_000_000;
    handle_life_tick(&mut world, oid);
    assert_eq!(
        servitor_of(&world, OWNER),
        Some(oid),
        "no deadline, no expiry"
    );
}

/// The upkeep item is taken from the owner when it falls due, and the servitor
/// carries on.
#[test]
fn the_upkeep_item_is_consumed_when_due() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let gemstone = 2131;
    {
        // Split borrow: the catalog is read while the inventory is written.
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&OWNER)
            .unwrap()
            .add_item(&data.item_data, 7_000_001, gemstone, 5);
    }
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, gemstone, 1).unwrap();

    world.tick = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .unwrap()
        .next_consume_tick;
    handle_life_tick(&mut world, oid);

    assert_eq!(
        count_of_item(&world, OWNER, gemstone),
        4,
        "one gemstone paid"
    );
    assert_eq!(servitor_of(&world, OWNER), Some(oid), "and it stays out");
}

/// Running out of the upkeep item dismisses the servitor — Java's "since you do
/// not have enough items to maintain the servitor's stay".
#[test]
fn running_out_of_the_upkeep_item_dismisses_the_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let gemstone = 2131;
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, gemstone, 1).unwrap();
    // The owner has none.

    world.tick = world
        .objects
        .get_component::<ServitorOf>(&oid)
        .unwrap()
        .next_consume_tick;
    handle_life_tick(&mut world, oid);

    assert_eq!(
        servitor_of(&world, OWNER),
        None,
        "dismissed for non-payment"
    );
}

/// The leash: a servitor stranded far from its owner is pulled back into
/// following, whatever it was doing — an ordered attack cannot leave it
/// abandoned across the map.
#[test]
fn a_stranded_servitor_is_leashed_back_to_following() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);
    servitor_attack(&mut world, OWNER, FOE);
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "off following, mid-order"
    );

    // The owner runs far away.
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 50_000;
    handle_life_tick(&mut world, oid);

    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .following,
        "leashed back into follow"
    );
}

/// A servitor does not outlive its owner's session.
#[test]
fn logging_out_takes_the_servitor_with_you() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();

    on_owner_leave_world(&mut world, OWNER);

    assert_eq!(
        servitor_of(&world, OWNER),
        None,
        "no ownerless NPC left behind"
    );
    assert!(
        world.objects.get_component::<Vitals>(&oid).is_none(),
        "despawned"
    );
}

/// A dead servitor ends the tick chain rather than rescheduling forever.
#[test]
fn a_dead_servitor_stops_ticking() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 60, 0, 0).unwrap();
    world
        .objects
        .get_component_mut::<Vitals>(&oid)
        .unwrap()
        .dead = true;

    // Well past the deadline: a live tick would have unsummoned it and sent
    // the "passed away" notice. A dead one just stops.
    world.tick += 100_000;
    handle_life_tick(&mut world, oid);
    assert!(
        world.objects.get_component::<ServitorOf>(&oid).is_some(),
        "left for the death path to clean up"
    );
}

// ---------------------------------------------------------------------------
// Pets (slice 6)
// ---------------------------------------------------------------------------

/// A party member's summon must appear in everyone else's party window. The
/// count was hard-coded to 0, so it never did — the third hard-coded-zero
/// count found by the sweep that started with `CharInfo`'s cubics.
#[test]
fn the_party_window_carries_a_members_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    let before = crate::game_loop::party::member_view(&world, OWNER).unwrap();
    assert!(before.summons.is_empty(), "no summon, no rows");

    summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    let after = crate::game_loop::party::member_view(&world, OWNER).unwrap();
    assert_eq!(
        after.summons.len(),
        1,
        "the servitor shows up in the party window"
    );
    assert_eq!(after.summons[0].summon_type, 2, "2 = servitor");
    assert!(
        after.summons[0].max_hp > 0,
        "and carries real vitals for the HP bar"
    );
}

/// The owner→summon link is what makes the lookup readable from `&World`.
/// Unsummoning must clear it, or the party window would keep showing a
/// creature that no longer exists.
#[test]
fn unsummoning_clears_the_owner_link() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    assert!(servitor_of(&world, OWNER).is_some());

    unsummon_servitor(&mut world, OWNER);
    assert!(servitor_of(&world, OWNER).is_none(), "link cleared");
    assert!(
        crate::game_loop::party::member_view(&world, OWNER)
            .unwrap()
            .summons
            .is_empty(),
        "and the party window row goes with it"
    );
}

// ---------------------------------------------------------------------------
// Pet experience (slice 12)
// ---------------------------------------------------------------------------

/// Too few shots left for one hit: nothing is spent and the pet stays
/// uncharged, rather than a partial charge on a partial payment.
#[test]
fn a_partial_stack_buys_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    register_beast_soulshot(&mut world);
    let pet_oid = summoned_pet(&mut world);
    give_owner_shots(&mut world, 1); // level 1 costs 2

    assert!(!crate::game_loop::servitor::recharge_shots(
        &mut world, pet_oid, true
    ));
    assert_eq!(owner_shot_count(&world), 1, "the odd shot is not consumed");
}

/// The owner also enters combat stance — Java hands the stance to
/// `getActingPlayer()`, and it is the owner's stance that blocks their own
/// sit/logout, not the summon's.
#[test]
fn a_summon_swing_puts_its_owner_in_combat_stance() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    combat::do_auto_attack(&mut world, servitor, victim);

    let now = world.tick;
    assert!(
        world
            .objects
            .get_component::<model::components::combat::AttackState>(&OWNER)
            .is_some_and(|s| s.stance_until_tick > now),
        "the owner is in combat stance"
    );
}

fn action_use_body(action_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(action_id);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i32(0);
    w.write_u8(0);
    w.into_bytes()
}

/// **Betray (1380)** turns somebody's servitor against them. Three things have
/// to happen and a port that only did the first would look plausible: the AI
/// points at the **owner**, the servitor stops taking orders ("your servitor is
/// unresponsive"), and `SummonInfo`'s status bit `0x01` marks it
/// auto-attackable so the owner can kill their own pet.
#[test]
fn betray_turns_a_servitor_against_its_owner_and_it_stops_obeying() {
    let (mut world, _db, _l) = servitor_world();
    let mut out = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, 20001, "Monster", 20, 80, 0, 0);

    // Before: the servitor obeys an attack order.
    assert!(
        servitor_attack(&mut world, OWNER, FOE),
        "an unbetrayed servitor takes orders"
    );

    let caster = OWNER + 1;
    let _c = ingame_player(&mut world, CID + 1, caster, 30, 0, 0);
    let betray = Skill {
        self_continuous: false,
        id: 9420,
        level: 1,
        target_type: TargetType::EnemyOnly,
        abnormal_time: 1200,
        abnormal_type: "BETRAY".into(),
        effects: vec![SkillEffect::Betray],
        ..Default::default()
    };
    world.data.skill_data.insert_for_test(betray.clone());
    drain(&mut out);
    effects::apply_skill_effects(&mut world, caster, servitor, &betray);

    // 1. The flag is up.
    assert_ne!(
        abnormal::flags_of(&world, servitor) & model::skill::effect_flag::BETRAYED,
        0,
        "the BETRAYED flag lands"
    );
    // 2. It is attacking its owner.
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&servitor)
            .map(|ai| ai.intention),
        Some(NpcIntention::Attack),
        "and it has turned on someone"
    );
    assert!(
        world
            .objects
            .get_component::<AggroList>(&servitor)
            .and_then(|a| a.0.get(&OWNER).map(|i| i.hate))
            .unwrap_or(0.0)
            > 0.0,
        "specifically its own owner"
    );
    // 3. It no longer obeys.
    drain(&mut out);
    // The order dispatches through `ActionData.xml`'s handler table, which the
    // fixture world ships empty — without the row the refusal below would be
    // "no handler found" rather than "the servitor is unresponsive".
    world.data.action_data.insert_row_for_test(
        crate::game_loop::client::actions::action::SERVITOR_STOP,
        "ServitorStop",
        0,
    );
    crate::game_loop::client::actions::handle_request_action_use(
        &mut world,
        CID,
        &action_use_body(crate::game_loop::client::actions::action::SERVITOR_STOP),
    );
    let pkts = drain(&mut out);
    assert!(
        has_system_message(
            &pkts,
            server_packets::sm_ids::YOUR_SERVITOR_IS_UNRESPONSIVE_AND_WILL_NOT_OBEY_ANY_ORDERS
        ),
        "a betrayed servitor refuses its owner's commands"
    );
}

/// **A summon does not fight back on its own.** Java's `Summon` is not an
/// `Attackable`: `SummonAI.onEvtAttacked` retaliates only in the `ServitorMode`
/// defending stance, and a fresh summon starts passive. The port used to run
/// every summon through the ordinary monster reaction, so it always retaliated
/// — this is the gate that stops it.
#[test]
fn a_passive_summon_does_not_fight_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);

    hit(&mut world, oid, FOE);

    assert_eq!(hate_for(&world, oid, FOE), 0.0, "no hate is taken");
    assert_ne!(
        world
            .objects
            .get_component::<NpcAi>(&oid)
            .unwrap()
            .intention,
        NpcIntention::Attack,
        "and it does not switch to attacking"
    );
    // The damage tally is still kept — it is what decides kill credit.
    assert!(
        world
            .objects
            .get_component::<AggroList>(&oid)
            .and_then(|a| a.0.get(&FOE))
            .map(|i| i.damage)
            .unwrap_or(0.0)
            > 0.0,
        "but the damage is recorded"
    );
}

/// `ServitorMode` option 2 — the defending stance. Now the same hit produces
/// the retaliation Java's `defendAttack` does.
#[test]
fn servitor_mode_defending_makes_it_fight_back() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);

    handle_servitor_action(&mut world, CID, OWNER, "ServitorMode", 2);
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .defending,
        "the stance is set"
    );

    hit(&mut world, oid, FOE);

    assert!(hate_for(&world, oid, FOE) > 0.0, "it takes the attacker");
    assert_eq!(
        world
            .objects
            .get_component::<NpcAi>(&oid)
            .unwrap()
            .intention,
        NpcIntention::Attack
    );

    // Option 1 puts it back to passive.
    handle_servitor_action(&mut world, CID, OWNER, "ServitorMode", 1);
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&oid)
            .unwrap()
            .defending
    );
}

/// `defendAttack`'s `owner != attacker` guard: a defending summon does not turn
/// on its own master, however it got hit by them.
#[test]
fn a_defending_summon_never_turns_on_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let oid = summon_servitor(&mut world, OWNER, PANTHER, 283, 0, 0, 0).unwrap();
    handle_servitor_action(&mut world, CID, OWNER, "ServitorMode", 2);

    hit(&mut world, oid, OWNER);

    assert_eq!(hate_for(&world, oid, OWNER), 0.0, "its master is exempt");
}

/// `Summon.instant`'s upkeep period:
///
/// ```java
/// final int consumeItemInterval = (_consumeItemInterval > 0 ? _consumeItemInterval
///     : (template.getRace() != Race.SIEGE_WEAPON ? 240 : 60)) * 1000;
/// ```
///
/// **A siege weapon pays four times as often.** Summon Siege Golem (13) is
/// learnable and costs 40 C-grade gemstones a go, so running the golem on the
/// ordinary 240 s interval quartered the price of the most expensive summon in
/// the game. No skill on this dist declares a `consumeItemInterval` of its own,
/// so the race arm is the whole of it.
#[test]
fn a_siege_weapon_pays_its_upkeep_four_times_as_often() {
    use crate::enums::Race;
    use crate::model::components::summons::ServitorOf;

    const GOLEM: i32 = 14737;
    const GEMSTONE: i32 = 2131;

    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);

    // Two templates that differ only in `<race>`.
    for (npc_id, race) in [(GOLEM, Some(Race::SiegeWeapon as i32)), (PANTHER, None)] {
        let mut t = world
            .data
            .npc_data
            .get(npc_id)
            .cloned()
            .unwrap_or_else(|| crate::data::npc_data::default_template(npc_id));
        t.race = race;
        world.data.npc_data.insert_for_test(t);
    }

    let period = |world: &mut World, npc_id: i32| -> u64 {
        let oid = summon_servitor(world, OWNER, npc_id, 1, 1200, GEMSTONE, 1).expect("summoned");
        let next = world
            .objects
            .get_component::<ServitorOf>(&oid)
            .map(|l| l.next_consume_tick - world.tick)
            .expect("linked");
        crate::game_loop::servitor::unsummon_servitor(world, OWNER);
        next
    };

    // 10 game ticks a second.
    assert_eq!(period(&mut world, GOLEM), 60 * 10, "siege weapon: 60 s");
    assert_eq!(
        period(&mut world, PANTHER),
        240 * 10,
        "everything else: 240 s"
    );
}
