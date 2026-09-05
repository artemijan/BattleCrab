//! Q00230 Test of the Summoner, and the arcana duel it runs.

use super::super::*;

/// The Test-of-the-Summoner (230) arcana-duel primitive, end to end: a
/// servitor's blow reaches `on_attack` marked `is_summon`, the quest sends the
/// rival NPC back at the servitor (`make_npc_attack`), and the servitor's kill
/// is credited to the owner in `on_kill` (VICTORY). Proves the pieces the
/// deferred Q230 needs — `attack_is_summon`, `owner_servitor`, `make_npc_attack`,
/// `is_oid_dead` — cooperate over real servitor combat.
#[test]
fn servitor_arcana_duel_round_trip() {
    const OPPONENT: i32 = 27102; // Pako the Cat
    const SERVITOR_NPC: i32 = 14100; // a Cat servitor template
    const STARTING: i32 = 3360;
    const INPROGRESS: i32 = 3361;
    const VICTORY: i32 = 3364;

    struct ArcanaBattleTest;
    impl quests::QuestScript for ArcanaBattleTest {
        fn id(&self) -> i32 {
            -30
        }
        fn name(&self) -> &'static str {
            "ArcanaBattleTest"
        }
        fn html_dir(&self) -> &'static str {
            ""
        }
        fn start_npcs(&self) -> &[i32] {
            &[]
        }
        fn talk_npcs(&self) -> &[i32] {
            &[]
        }
        fn kill_npcs(&self) -> &[i32] {
            &[OPPONENT]
        }
        fn attack_npcs(&self) -> &[i32] {
            &[OPPONENT]
        }
        fn on_talk(&self, _ctx: &mut quests::QuestCtx) -> Option<String> {
            None
        }
        fn on_kill(&self, ctx: &mut quests::QuestCtx) {
            if ctx.quest_items_count(INPROGRESS) > 0 {
                ctx.take_items(INPROGRESS, -1);
                ctx.give_items(VICTORY, 1);
            }
        }
        fn on_attack(&self, ctx: &mut quests::QuestCtx) {
            match ctx.npc_script_value() {
                0 if ctx.attack_is_summon()
                    && let Some(servitor) = ctx.owner_servitor() =>
                {
                    ctx.set_npc_var_int("ATTACKER", servitor);
                    ctx.set_npc_script_value(1);
                    ctx.start_quest_timer("KILLED_ATTACKER", 5000);
                    if ctx.quest_items_count(STARTING) > 0 {
                        ctx.take_items(STARTING, -1);
                        ctx.give_items(INPROGRESS, 1);
                        ctx.make_npc_attack(servitor); // the rival strikes back
                    }
                }
                1 if !ctx.attack_is_summon()
                    || ctx.owner_servitor() != Some(ctx.npc_var_int("ATTACKER")) =>
                {
                    // A foul: the player, or a different summon, interfered.
                    ctx.set_npc_script_value(2);
                    ctx.delete_npc();
                }
                _ => {}
            }
        }
        fn on_timer(&self, ctx: &mut quests::QuestCtx, name: &str) {
            if name == "KILLED_ATTACKER" && ctx.is_oid_dead(ctx.npc_var_int("ATTACKER")) {
                ctx.delete_npc();
            }
        }
    }

    let (mut world, _db, _l) = quest_test_world();
    world.quests = Arc::new(quests::QuestRegistry::new(vec![Arc::new(ArcanaBattleTest)]));
    add_quest_items(
        &mut world,
        &[
            (STARTING, "start", true),
            (INPROGRESS, "prog", true),
            (VICTORY, "win", true),
        ],
    );
    // A Servitor template for the owner and a quest-monster for the rival.
    let mut st = crate::data::npc_data::default_template(SERVITOR_NPC);
    st.type_name = "Servitor".into();
    st.base_hp_max = 400.0;
    st.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(st);
    let mut ot = crate::data::npc_data::default_template(OPPONENT);
    ot.type_name = "Monster".into();
    ot.level = 40;
    ot.base_hp_max = 100_000.0;
    world.data.npc_data.insert_for_test(ot);

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    inject(&mut world, 3001, 0x0230_0000, STARTING, 1);
    let servitor = crate::game_loop::servitor::summon_servitor(
        &mut world,
        3001,
        SERVITOR_NPC,
        283,
        1200,
        0,
        0,
    )
    .expect("servitor summoned");
    let opponent = NPC_OID + 5;
    add_test_npc(&mut world, opponent, OPPONENT, "Monster", 40, 120, 200, 0);

    // The servitor lands the first blow: reaches on_attack marked is_summon.
    combat::npc_receive_damage(&mut world, opponent, servitor, 10.0, false);
    assert_eq!(
        item_count(&world, 3001, STARTING),
        0,
        "Starting crystal consumed"
    );
    assert_eq!(
        item_count(&world, 3001, INPROGRESS),
        1,
        "In-Progress crystal granted"
    );
    // The rival was set on the servitor (make_npc_attack seeded its aggro).
    let seeded = world
        .objects
        .get_component::<AggroList>(&opponent)
        .is_some_and(|a| a.0.contains_key(&servitor));
    assert!(seeded, "the rival strikes back at the servitor");

    // A foul: the owner (not their summon) hits the rival → it quits (deleted).
    add_test_npc(
        &mut world,
        NPC_OID + 6,
        OPPONENT,
        "Monster",
        40,
        120,
        200,
        0,
    );
    combat::npc_receive_damage(&mut world, NPC_OID + 6, servitor, 1.0, false); // servitor engages it
    combat::npc_receive_damage(&mut world, NPC_OID + 6, 3001, 1.0, false); // the OWNER interferes
    assert!(
        world
            .objects
            .get_component::<Vitals>(&(NPC_OID + 6))
            .is_none_or(|v| v.dead),
        "a player-struck rival fouls out and despawns"
    );

    // The servitor finishes the real duel: its kill is credited to the owner.
    npc::npc_do_die(&mut world, opponent, servitor);
    assert_eq!(
        item_count(&world, 3001, INPROGRESS),
        0,
        "In-Progress consumed on victory"
    );
    assert_eq!(
        item_count(&world, 3001, VICTORY),
        1,
        "Victory crystal awarded to the owner"
    );
}

/// Test of the Summoner (230) end to end: the class/level gate, Grocer Lara's
/// list + token farm (with its list gating), the Beginner's Arcana turn-in, a
/// full arcana duel driven through real servitor combat (foul path and victory
/// path), redeeming Victory crystals for all six Summoner arcanas, and Galatea's
/// completion reward.
#[test]
fn quest_q00230_test_of_the_summoner() {
    // Item ids (see q00230_test_of_the_summoner.rs).
    const GALATEAS_LETTER: i32 = 3352;
    const LARAS_1ST_LIST: i32 = 3347;
    const LETO_AMULET: i32 = 3337;
    const SAC_OF_REDSPORES: i32 = 3338;
    const KARUL_TOTEM: i32 = 3339;
    const BEGINNERS_ARCANA: i32 = 3353;
    const STARTING_1ST: i32 = 3360;
    const INPROGRESS_1ST: i32 = 3361;
    const FOUL_1ST: i32 = 3362;
    const VICTORY_1ST: i32 = 3364;
    const ALMORS_ARCANA: i32 = 3354;
    const MARK_OF_SUMMONER: i32 = 3336;
    // NPCs
    const GALATEA: i32 = 30634;
    const LARA: i32 = 30063;
    const ALMORS: i32 = 30635;
    const CAMONIELL: i32 = 30636;
    const BELTHUS: i32 = 30637;
    const BASILLA: i32 = 30638;
    const CELESTIEL: i32 = 30639;
    const BRYNTHEA: i32 = 30640;
    // Monsters
    const PAKO: i32 = 27102;
    const LETO: i32 = 20577;
    const KARUL: i32 = 20600;
    const SERVITOR_NPC: i32 = 14100;

    let (mut world, _db, _l) = quest_test_world();
    // Every quest item this test moves, plus the (tradeable) reward.
    let mut items: Vec<(i32, &str, bool)> = [
        GALATEAS_LETTER,
        LARAS_1ST_LIST,
        3348,
        3349,
        3350,
        3351, // lists 2..5
        LETO_AMULET,
        SAC_OF_REDSPORES,
        KARUL_TOTEM,
        BEGINNERS_ARCANA,
        STARTING_1ST,
        INPROGRESS_1ST,
        FOUL_1ST,
        3363, // DEFEAT_1ST
        VICTORY_1ST,
        ALMORS_ARCANA,
        3355,
        3356,
        3357,
        3358,
        3359, // other 5 arcanas
        3369,
        3374,
        3379,
        3384,
        3389, // VICTORY 2nd..6th
    ]
    .iter()
    .map(|&id| (id, "Q230", true))
    .collect();
    items.push((MARK_OF_SUMMONER, "Mark of Summoner", false));
    add_quest_items(&mut world, &items);

    // A Cat servitor template for the owner; a durable Pako for the duel.
    let mut sv = crate::data::npc_data::default_template(SERVITOR_NPC);
    sv.type_name = "Servitor".into();
    sv.base_hp_max = 400.0;
    sv.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(sv);
    for id in [PAKO, LETO, KARUL] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 40;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }

    let galatea = NPC_OID;
    let lara = NPC_OID + 1;
    let almors = NPC_OID + 2;
    let camoniell = NPC_OID + 3;
    let belthus = NPC_OID + 4;
    let basilla = NPC_OID + 5;
    let celestiel = NPC_OID + 6;
    let brynthea = NPC_OID + 7;
    for (oid, npc) in [
        (galatea, GALATEA),
        (lara, LARA),
        (almors, ALMORS),
        (camoniell, CAMONIELL),
        (belthus, BELTHUS),
        (basilla, BASILLA),
        (celestiel, CELESTIEL),
        (brynthea, BRYNTHEA),
    ] {
        add_test_npc(&mut world, oid, npc, "Folk", 40, 100, 200, 0);
    }

    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 40;
        p.class_id = 11; // Wizard
    }
    let q = "Q00230_TestOfTheSummoner";
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };
    let talk = |w: &mut World, npc: i32| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q}")));
    };

    // Grab the first HTML from a talk, whether it went out as a `.html`
    // (`NpcHtmlMessage`) or a `.htm` (`ExNpcQuestHtmlMessage`).
    let grab_html = |rx: &mut UnboundedReceiver<bytes::Bytes>| -> Option<String> {
        drain(rx).iter().find_map(|p| {
            if p[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                decode_npc_html(p)
            } else if p[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                r.read_i16()?; // ex opcode
                r.read_i32()?; // npc oid
                r.read_string()
            } else {
                None
            }
        })
    };

    // --- Class / level gate on the start NPC. ---
    talk(&mut world, galatea);
    let html = grab_html(&mut rx).expect("Galatea greets a Wizard");
    // The 30634-03 offer page carries the "accept the trial" button (→30634-04).
    assert!(
        html.contains("30634-04.htm"),
        "level-39 Wizard is offered the trial: {html}"
    );
    // A non-caster is turned away — the refusal page has no accept button.
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .class_id = 10; // Human Fighter
    talk(&mut world, galatea);
    let html = grab_html(&mut rx).unwrap();
    assert!(
        !html.contains("30634-04.htm"),
        "a fighter is refused (no accept button): {html}"
    );
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .class_id = 11;

    // --- Accept: Galatea's Letter, quest started. ---
    ev(&mut world, galatea, "ACCEPT");
    assert_eq!(quest_cond(&world, 3001, q), Some(1));
    assert_eq!(
        item_count(&world, 3001, GALATEAS_LETTER),
        1,
        "Galatea's Letter"
    );

    // --- Lara hands out a hunting list (forced to the 1st), takes the Letter. ---
    world.force_roll(0); // getRandom(5) → LARAS_1ST_LIST
    ev(&mut world, lara, "30063-02.html");
    assert_eq!(quest_cond(&world, 3001, q), Some(2));
    assert_eq!(
        item_count(&world, 3001, LARAS_1ST_LIST),
        1,
        "1st list granted"
    );
    assert_eq!(
        item_count(&world, 3001, GALATEAS_LETTER),
        0,
        "Letter surrendered"
    );

    // --- Token farm: a matching kill drops, a mismatched one does not. ---
    let mut mob = NPC_OID + 30;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 40, 110, 200, 0);
        w.force_roll(0); // give_item_randomly roll_f64 → 0.0 ≤ chance
        npc::npc_do_die(w, mob, 3001);
    };
    kill(&mut world, LETO); // list1 held → Leto Lizardman Amulet drops
    assert!(
        item_count(&world, 3001, LETO_AMULET) >= 1,
        "amulet dropped while holding 1st list"
    );
    kill(&mut world, KARUL); // Karul needs the 2nd list → nothing
    assert_eq!(
        item_count(&world, 3001, KARUL_TOTEM),
        0,
        "no drop for a mismatched list"
    );

    // `Util.checkIfInRange(ALT_PARTY_RANGE, npc, killer, true)` gates every
    // branch: a party member who was nowhere near the kill collects nothing.
    // 1500 is the configured range, so 5000 units out is comfortably outside.
    {
        let held = item_count(&world, 3001, LETO_AMULET);
        let far = NPC_OID + 90;
        add_test_npc(&mut world, far, LETO, "Monster", 40, 5_000, 5_000, 0);
        world.force_roll(0);
        npc::npc_do_die(&mut world, far, 3001);
        assert_eq!(
            item_count(&world, 3001, LETO_AMULET),
            held,
            "a kill outside AltPartyRange drops nothing"
        );
    }

    // --- Turn in the 1st list: 30 + 30 tokens → two Beginner's Arcana, cond 3. ---
    inject(&mut world, 3001, 0x0230_1000, LETO_AMULET, 30);
    inject(&mut world, 3001, 0x0230_2000, SAC_OF_REDSPORES, 30);
    talk(&mut world, lara);
    assert_eq!(quest_cond(&world, 3001, q), Some(3));
    assert_eq!(
        item_count(&world, 3001, BEGINNERS_ARCANA),
        2,
        "two Beginner's Arcana"
    );
    assert_eq!(
        item_count(&world, 3001, LARAS_1ST_LIST),
        0,
        "list consumed on turn-in"
    );

    // --- Summoner Almors: the offer needs an arcana; buying starts the duel. ---
    let _ = drain(&mut rx); // clear queued html from the prior turn-in
    ev(&mut world, almors, "30635-03.html"); // gated: shows the offer (arcana in hand)
    let html = grab_html(&mut rx).expect("offer page");
    assert!(
        html.contains("30635-04.htm") || html.contains("Almors"),
        "offer shown with an arcana in hand: {html}"
    );
    ev(&mut world, almors, "30635-04.html"); // Arcana → Crystal of Starting (1st)
    assert_eq!(
        item_count(&world, 3001, STARTING_1ST),
        1,
        "Crystal of Starting granted"
    );
    assert_eq!(
        item_count(&world, 3001, BEGINNERS_ARCANA),
        1,
        "one arcana spent"
    );

    // Summon the servitor that will fight the duels.
    let servitor = crate::game_loop::servitor::summon_servitor(
        &mut world,
        3001,
        SERVITOR_NPC,
        283,
        1200,
        0,
        0,
    )
    .expect("servitor summoned");

    // --- Foul path: servitor engages, then the *player* interferes. ---
    let pako1 = NPC_OID + 60;
    add_test_npc(&mut world, pako1, PAKO, "Monster", 40, 120, 200, 0);
    combat::npc_receive_damage(&mut world, pako1, servitor, 10.0, false); // servitor engages
    assert_eq!(
        item_count(&world, 3001, INPROGRESS_1ST),
        1,
        "duel engaged: In-Progress"
    );
    assert_eq!(
        item_count(&world, 3001, STARTING_1ST),
        0,
        "Starting consumed on engage"
    );
    combat::npc_receive_damage(&mut world, pako1, 3001, 10.0, false); // the OWNER fouls it
    assert_eq!(
        item_count(&world, 3001, FOUL_1ST),
        1,
        "a player strike fouls the duel"
    );
    assert_eq!(
        item_count(&world, 3001, INPROGRESS_1ST),
        0,
        "In-Progress lost on foul"
    );

    // --- Victory path: buy a fresh Starting (clears the Foul), win by servitor. ---
    ev(&mut world, almors, "30635-04.html");
    assert_eq!(
        item_count(&world, 3001, FOUL_1ST),
        0,
        "Foul cleared by a fresh Starting"
    );
    assert_eq!(item_count(&world, 3001, STARTING_1ST), 1);
    let pako2 = NPC_OID + 61;
    add_test_npc(&mut world, pako2, PAKO, "Monster", 40, 120, 200, 0);
    combat::npc_receive_damage(&mut world, pako2, servitor, 10.0, false); // engage
    assert_eq!(item_count(&world, 3001, INPROGRESS_1ST), 1);
    npc::npc_do_die(&mut world, pako2, servitor); // servitor kill → owner-credited
    assert_eq!(
        item_count(&world, 3001, VICTORY_1ST),
        1,
        "Victory on a servitor kill"
    );
    assert_eq!(
        item_count(&world, 3001, INPROGRESS_1ST),
        0,
        "In-Progress consumed on victory"
    );

    // --- Redeem Victory for the Almors Arcana. ---
    talk(&mut world, almors);
    assert_eq!(
        item_count(&world, 3001, ALMORS_ARCANA),
        1,
        "Almors Arcana earned"
    );
    assert_eq!(item_count(&world, 3001, VICTORY_1ST), 0, "Victory redeemed");

    // --- The other five duels: inject Victory crystals and redeem each. The
    // final redemption (all six arcanas held) advances to cond 4. ---
    for (obj, victory, summoner) in [
        (0x0230_3000, 3369, basilla),   // 2nd → Basillia
        (0x0230_4000, 3374, camoniell), // 3rd → Camoniell
        (0x0230_5000, 3379, celestiel), // 4th → Celestiel
        (0x0230_6000, 3384, belthus),   // 5th → Belthus
        (0x0230_7000, 3389, brynthea),  // 6th → Brynthea
    ] {
        inject(&mut world, 3001, obj, victory, 1);
        talk(&mut world, summoner);
    }
    assert_eq!(
        quest_cond(&world, 3001, q),
        Some(4),
        "all six arcanas → cond 4"
    );
    for arcana in [3355, 3356, 3357, 3358, 3359] {
        assert_eq!(
            item_count(&world, 3001, arcana),
            1,
            "arcana {arcana} earned"
        );
    }

    // --- Galatea completes the test: Mark of Summoner, exit. ---
    talk(&mut world, galatea);
    assert_eq!(
        item_count(&world, 3001, MARK_OF_SUMMONER),
        1,
        "Mark of Summoner awarded"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "one-time quest stays COMPLETED");
}
