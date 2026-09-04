//! What a summon's actions do to its owner's PvP state: flagging, karma,
//! duels, and the clan-war credit a summon kill earns.

use super::*;

/// Java `Creature.doAttack` flags `getActingPlayer()`, and `Summon`'s is its
/// **owner** — so setting your pet on another player flags *you*.
///
/// Without this a player can attack through their summon and never go purple:
/// the victim can't retaliate without taking the karma, which is the shape of
/// an exploit rather than a cosmetic gap.
#[test]
fn a_summon_attacking_a_player_flags_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    crate::game_loop::combat::pvp::update_pvp_status_target(&mut world, servitor, victim);

    let flagged = world
        .objects
        .get_component::<model::components::combat::PvpState>(&OWNER)
        .is_some_and(|s| s.flag > 0);
    assert!(flagged, "the owner is flagged for their summon's attack");
}

/// End-to-end: a real summon swing must flag the owner, not just the helper
/// called directly. The unit test above proves `update_pvp_status_target`
/// resolves the owner; this proves the attack path actually reaches it.
#[test]
fn a_real_summon_swing_flags_the_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    combat::do_auto_attack(&mut world, servitor, victim);

    let flagged = world
        .objects
        .get_component::<model::components::combat::PvpState>(&OWNER)
        .is_some_and(|s| s.flag > 0);
    assert!(
        flagged,
        "the owner is flagged by their summon's actual swing"
    );
}

/// The counterpart guard: a **plain monster** hitting a player must still flag
/// nobody. Moving the flag/stance block out of the player-only branch is only
/// safe because `acting_player` resolves a mob to itself, and a mob is not a
/// player.
#[test]
fn a_monster_attacking_a_player_flags_nobody() {
    let (mut world, _db, _l) = servitor_world();
    let victim = OWNER + 7;
    let _rx = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 40, 0, 0);

    combat::do_auto_attack(&mut world, FOE, victim);

    assert!(
        world
            .objects
            .get_component::<model::components::combat::PvpState>(&victim)
            .is_none_or(|s| s.flag == 0),
        "the victim is not flagged by being attacked"
    );
    assert!(
        world
            .objects
            .get_component::<model::components::combat::PvpState>(&FOE)
            .is_none_or(|s| s.flag == 0),
        "and neither is the monster"
    );
}

// ---------------------------------------------------------------------------
// Summon kill credit (slice 22)
// ---------------------------------------------------------------------------

/// Java resolves every damage dealer to `getActingPlayer()` when handing out
/// rewards, so a **summon's killing blow credits its owner**. Without that a
/// player whose pet lands the last hit gets no exp — the core summoner loop.
#[test]
fn a_summon_killing_blow_credits_its_owner() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
    // `default_template` awards nothing, so without this the assertion would
    // be vacuous — it would read 0 exp whether or not the credit worked.
    {
        let mut t = world.data.npc_data.get(PANTHER + 1).unwrap().clone();
        t.exp = 1000.0;
        t.sp = 100.0;
        world.data.npc_data.insert_for_test(t);
    }

    // Rewards are shares of the aggro list's recorded damage. Seeded directly
    // rather than by swinging, because a real swing lands on a *scheduled*
    // tick — this test is about who the damage is credited to, not about
    // attack timing.
    world
        .objects
        .get_component_mut::<AggroList>(&FOE)
        .unwrap()
        .0
        .entry(servitor)
        .or_default()
        .damage = 500.0;
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .exp = 0;
    crate::game_loop::npc::npc_do_die(&mut world, FOE, servitor);

    let exp = world.objects.get_component::<Player>(&OWNER).unwrap().exp;
    assert!(
        exp > 0,
        "the owner was credited for their summon's kill (exp {exp})"
    );
}

/// A player who fights *alongside* their summon appears twice in the aggro
/// list once both resolve to them. Their shares must merge, not double-count —
/// otherwise fighting with a pet would inflate the owner's slice of a
/// contested kill against everyone else.
#[test]
fn an_owner_and_their_summon_share_one_slice() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let rival = OWNER + 9;
    let _rx2 = ingame_caster(&mut world, CID + 9, rival, 20, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();
    add_test_npc(&mut world, FOE, PANTHER + 1, "Monster", 20, 60, 0, 0);
    {
        let mut t = world.data.npc_data.get(PANTHER + 1).unwrap().clone();
        t.exp = 1000.0;
        world.data.npc_data.insert_for_test(t);
    }

    // Owner 100 + their summon 100 = 200; the rival also does 200.
    {
        let aggro = &mut world
            .objects
            .get_component_mut::<AggroList>(&FOE)
            .unwrap()
            .0;
        aggro.entry(OWNER).or_default().damage = 100.0;
        aggro.entry(servitor).or_default().damage = 100.0;
        aggro.entry(rival).or_default().damage = 200.0;
    }
    for oid in [OWNER, rival] {
        world.objects.get_component_mut::<Player>(&oid).unwrap().exp = 0;
    }

    crate::game_loop::npc::npc_do_die(&mut world, FOE, servitor);

    let owner_exp = world.objects.get_component::<Player>(&OWNER).unwrap().exp;
    let rival_exp = world.objects.get_component::<Player>(&rival).unwrap().exp;
    assert!(
        owner_exp > 0 && rival_exp > 0,
        "both earned ({owner_exp} / {rival_exp})"
    );
    assert_eq!(
        owner_exp, rival_exp,
        "equal damage earns equal exp — the pair merged into one slice"
    );
}

// ---------------------------------------------------------------------------
// getActingPlayer audit, part 2 (slice 23)
// ---------------------------------------------------------------------------

/// Java's PK/karma block reads `killer.getActingPlayer()`, so killing a player
/// **with your pet** carries the same consequences as killing them yourself.
/// Without it, a pet kill is a free kill: no PK counter, no karma.
#[test]
fn a_summon_killing_a_player_gives_its_owner_the_karma() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    let before = world
        .objects
        .get_component::<Player>(&OWNER)
        .unwrap()
        .pk_kills;
    crate::game_loop::death::player_do_die(&mut world, victim, servitor);
    let after = world
        .objects
        .get_component::<Player>(&OWNER)
        .unwrap()
        .pk_kills;

    assert!(
        after > before,
        "the owner took the PK for their summon's kill ({before} → {after})"
    );
}

/// **A duel never kills** (G20's invariant). The lethal guard resolves the
/// attacker to its acting player, or a summon's blow slips past it and really
/// kills the opponent.
#[test]
fn a_summons_blow_cannot_kill_a_duel_opponent() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let foe_player = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, foe_player, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // Put the two players in a duel with each other.
    world
        .objects
        .add_components(&OWNER, model::components::social::DuelRef(1));
    world
        .objects
        .add_components(&foe_player, model::components::social::DuelRef(1));
    // The snapshot the end-of-duel restore puts back: both at full.
    let snap = |world: &World, oid: i32| {
        let v = world.objects.get_component::<Vitals>(&oid).unwrap();
        (v.max_hp as f64, v.max_mp as f64, 0.0)
    };
    world.duels.insert(
        1,
        crate::game_loop::combat::duel::Duel {
            snapshot: [snap(&world, OWNER), snap(&world, foe_player)],
            id: 1,
            player_a: OWNER,
            player_b: foe_player,
            countdown: 0,
            ends_at_tick: u64::MAX,
            surrender: 0,
            party: false,
            team_a: Vec::new(),
            team_b: Vec::new(),
            member_snapshot: Vec::new(),
            instance_id: 0,
            defeated: Vec::new(),
            winner_team: 0,
        },
    );
    world
        .objects
        .get_component_mut::<Vitals>(&foe_player)
        .unwrap()
        .cur_hp = 50.0;

    let capped =
        crate::game_loop::combat::duel::duel_lethal_guard(&mut world, servitor, foe_player, 9999.0);
    assert!(capped, "the summon's lethal blow was capped");
    // The cap sets 1 HP and ends the duel, and ending it runs
    // `restorePlayerConditions`, which heals both sides — so the observable
    // post-condition is "alive", not "at 1 HP".
    let v = world.objects.get_component::<Vitals>(&foe_player).unwrap();
    assert!(
        !v.dead && v.cur_hp > 0.0,
        "the duel opponent survived ({} HP)",
        v.cur_hp
    );
}

/// The clan-war kill counter also follows the acting player: a kill by the
/// enemy's pet is still a kill for the war score.
#[test]
fn a_summon_kill_counts_for_the_clan_war() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let victim = OWNER + 7;
    let _rx2 = ingame_caster(&mut world, CID + 7, victim, 60, 0);
    let servitor = summon_servitor(&mut world, OWNER, PANTHER, 1, 1200, 0, 0).unwrap();

    // `clan_war_on_kill` returns early unless the *killer* resolves to a
    // player; before the resolution a summon killer fell out immediately.
    // Reaching it at all is what this asserts — the war bookkeeping itself is
    // covered by the clan tests.
    let reached = crate::game_loop::combat::pvp::acting_player(&world, servitor);
    assert_eq!(
        reached, OWNER,
        "the summon resolves to its owner for war credit"
    );
    crate::game_loop::death::player_do_die(&mut world, victim, servitor);
    assert!(
        world
            .objects
            .get_component::<Player>(&OWNER)
            .unwrap()
            .pk_kills
            > 0,
        "the kill was attributed to the owner"
    );
}
