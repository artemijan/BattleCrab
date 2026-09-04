//! The third-class saga chains (Q00070-Q00100) and the mechanics they share:
//! the finale boss, the companion taunts and the progression FX.

use super::*;

/// Saga of the Phoenix Knight (70): the 3rd-class engine end to end. Drives the
/// full 20-condition ladder (item hand-ins, the Guardian-Angel kill count, the
/// scripted mob kills, and the Archon reward) through to the class transfer
/// (Paladin 5 → Phoenix Knight 90) — proving the shared Saga engine.
#[test]
fn quest_q00070_saga_of_the_phoenix_knight() {
    const START_NPC: i32 = 30849;
    // items[]: 0..11
    const I0: i32 = 7080;
    const I1: i32 = 7534;
    const I2: i32 = 7081;
    const MARK: i32 = 7485; // items[3] Halisha mark
    const I4: i32 = 7268;
    const I5: i32 = 7299;
    const I6: i32 = 7330;
    const I7: i32 = 7361;
    const I8: i32 = 7392;
    const I9: i32 = 7423;
    const I10: i32 = 7093; // starter
    const I11: i32 = 6482;
    const REWARD_MARK: i32 = 6622;
    // mobs
    const MOB0: i32 = 27286;
    const MOB1: i32 = 27219;
    const MOB2: i32 = 27278;
    const GUARDIAN: i32 = 27214;
    const ARCHON_MINION: i32 = 21646;
    const ARCHON_HALISHA: i32 = 18212;

    let (mut world, _db, _l) = quest_test_world();
    let ids = [
        I0,
        I1,
        I2,
        MARK,
        I4,
        I5,
        I6,
        I7,
        I8,
        I9,
        I10,
        I11,
        REWARD_MARK,
    ];
    let items: Vec<(i32, &str, bool)> = ids.iter().map(|&i| (i, "q", true)).collect();
    add_quest_items(&mut world, &items);
    for id in [MOB0, MOB1, MOB2, GUARDIAN, ARCHON_MINION, ARCHON_HALISHA] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 78;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    let start = NPC_OID;
    add_test_npc(&mut world, start, START_NPC, "Folk", 78, 100, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 76;
        p.class_id = 5; // Paladin (the required 2nd class)
    }
    let q = "Q00070_SagaOfThePhoenixKnight";
    let ev = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{start}_Quest {q} {e}")));
    };
    let cond = |w: &World| quest_cond(w, 3001, q);
    let mut mob = NPC_OID + 20;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 78, 110, 200, 0);
        npc::npc_do_die(w, mob, 3001);
    };

    // Accept.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{start}_Quest {q}")),
    );
    ev(&mut world, "accept");
    assert_eq!(cond(&world), Some(1), "started");
    assert_eq!(item_count(&world, 3001, I10), 1, "starter item");

    // The early intermediary ladder (cond 1 → 5).
    ev(&mut world, "2-1");
    assert_eq!(cond(&world), Some(2));
    ev(&mut world, "1-3");
    assert_eq!(cond(&world), Some(3));
    inject(&mut world, 3001, 0x0070_0000, I0, 1);
    inject(&mut world, 3001, 0x0070_0100, I11, 1);
    ev(&mut world, "1-4");
    assert_eq!(cond(&world), Some(4));
    assert_eq!(item_count(&world, 3001, I0), 0, "items[0] taken");
    assert_eq!(item_count(&world, 3001, I1), 1, "items[1] given");
    ev(&mut world, "2-2");
    assert_eq!(cond(&world), Some(5));
    assert_eq!(item_count(&world, 3001, I4), 1, "items[4] given");

    // Tablet 5-1 → cond 6, then 10 Guardian Angels → cond 7.
    ev(&mut world, "5-1");
    assert_eq!(cond(&world), Some(6));
    for _ in 0..10 {
        kill(&mut world, GUARDIAN);
    }
    assert_eq!(cond(&world), Some(7), "10 angels → cond 7");
    assert_eq!(item_count(&world, 3001, I5), 1, "items[5] from the angels");

    // 6-1 → cond 8, spawn+kill mob[0] → cond 9.
    ev(&mut world, "6-1");
    assert_eq!(cond(&world), Some(8));
    ev(&mut world, "7-1"); // spawns mob[0] (ignored; we kill a synthetic one)
    kill(&mut world, MOB0);
    assert_eq!(cond(&world), Some(9), "mob[0] slain → cond 9");
    assert_eq!(item_count(&world, 3001, I6), 1, "items[6] from mob[0]");

    // 7-2 → cond 10, checker 3-6 → cond 11, Divine Stone → cond 13.
    ev(&mut world, "7-2");
    assert_eq!(cond(&world), Some(10));
    ev(&mut world, "3-6");
    assert_eq!(cond(&world), Some(11));
    inject(&mut world, 3001, 0x0070_0200, I2, 1);
    ev(&mut world, "3-8");
    assert_eq!(cond(&world), Some(13));
    assert_eq!(item_count(&world, 3001, I7), 1, "items[7] given");

    // 8-1 → cond 14, 11-9 → cond 15, Archon farm → cond 16.
    ev(&mut world, "8-1");
    assert_eq!(cond(&world), Some(14));
    ev(&mut world, "11-9");
    assert_eq!(cond(&world), Some(15));
    kill(&mut world, ARCHON_MINION); // → a Halisha mark
    assert_eq!(item_count(&world, 3001, MARK), 1, "Halisha mark farmed");
    kill(&mut world, ARCHON_HALISHA); // → items[8], take marks, cond 16
    assert_eq!(cond(&world), Some(16), "Archon Halisha → cond 16");
    assert_eq!(item_count(&world, 3001, I8), 1, "items[8] archon reward");

    // 9-1 → cond 17, finale spawn 10-1, 4-2 → cond 18, 10-2 → cond 19.
    ev(&mut world, "9-1");
    assert_eq!(cond(&world), Some(17));
    ev(&mut world, "10-1");
    ev(&mut world, "4-2");
    assert_eq!(cond(&world), Some(18));
    assert_eq!(item_count(&world, 3001, I9), 1, "items[9] battle reward");
    ev(&mut world, "10-2");
    assert_eq!(cond(&world), Some(19));

    // The quest-giver performs the class transfer.
    drain(&mut rx);
    ev(&mut world, "0-2");
    assert_eq!(
        item_count(&world, 3001, REWARD_MARK),
        1,
        "Mark of the class transfer"
    );
    // Two 5103 casts land here, and both are Java's:
    //   * `Player.setClassId` broadcasts its own class-change flash as a
    //     *self*-cast (`MagicSkillUse(this, 5103, 1, 0, 0)`), and
    //   * the saga quest then casts npc->player
    //     (`MagicSkillUse(npc, player, 5103, 1, 1000, 0)`).
    // In that order, as in Java's `setClassId(...)` then `broadcastPacket(...)`.
    // The port had the first but sent 4339 — quest 235's elixir flash — for
    // the second, as two self-casts. Assert the caster/target pairs, since the
    // skill id alone cannot tell the two apart.
    let transfer_cast = drain(&mut rx)
        .into_iter()
        .filter(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .map(|p| {
            let mut r = commons::network::PacketReader::new(&p[1..]);
            r.read_i32().unwrap(); // cast bar
            let caster = r.read_i32().unwrap();
            let target = r.read_i32().unwrap();
            (caster, target, r.read_i32().unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        transfer_cast,
        vec![(3001, 3001, 5103), (NPC_OID, 3001, 5103)],
        "setClassId's self-cast, then the saga's npc->player cast"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        90,
        "transferred to Phoenix Knight (90)"
    );
    let quests = world
        .objects
        .get_component::<model::components::social::Quests>(&3001)
        .unwrap();
    assert!(quests.0[q].is_completed(), "Saga completes on the transfer");
}

/// Drive one Saga through the full 20-cond ladder to the class transfer, keyed
/// only on its per-quest data (`items`, `mob[0]`, class ids) — the shared
/// engine is proven by [`quest_q00070_saga_of_the_phoenix_knight`]; this
/// validates each fighter Saga's data table end to end.
fn run_fighter_saga(
    name: &str,
    start_npc: i32,
    prev_class: i32,
    class_id: i32,
    items: [i32; 12],
    mob0: i32,
) {
    const GUARDIAN: i32 = 27214;
    const ARCHON_MINION: i32 = 21646;
    const ARCHON_HALISHA: i32 = 18212;
    const REWARD_MARK: i32 = 6622;

    let (mut world, _db, _l) = quest_test_world();
    let mut ids: Vec<(i32, &str, bool)> = items
        .iter()
        .filter(|&&i| i != 0)
        .map(|&i| (i, "q", true))
        .collect();
    ids.push((REWARD_MARK, "mark", false));
    add_quest_items(&mut world, &ids);
    for id in [mob0, GUARDIAN, ARCHON_MINION, ARCHON_HALISHA] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = "Monster".into();
        t.level = 78;
        t.base_hp_max = 100.0;
        world.data.npc_data.insert_for_test(t);
    }
    let start = NPC_OID;
    add_test_npc(&mut world, start, start_npc, "Folk", 78, 100, 200, 0);
    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 76;
        p.class_id = prev_class;
    }
    let q = name;
    let ev = |w: &mut World, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{start}_Quest {q} {e}")));
    };
    let cond = |w: &World| quest_cond(w, 3001, q);
    let mut mob = NPC_OID + 20;
    let mut kill = |w: &mut World, npc_id: i32| {
        mob += 1;
        add_test_npc(w, mob, npc_id, "Monster", 78, 110, 200, 0);
        npc::npc_do_die(w, mob, 3001);
    };

    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{start}_Quest {q}")),
    );
    ev(&mut world, "accept");
    assert_eq!(cond(&world), Some(1), "{name}: started");
    ev(&mut world, "2-1");
    ev(&mut world, "1-3");
    assert_eq!(cond(&world), Some(3), "{name}: cond 3");
    inject(&mut world, 3001, 0x0071_0000, items[0], 1);
    if items[11] != 0 {
        inject(&mut world, 3001, 0x0071_0100, items[11], 1);
    }
    ev(&mut world, "1-4");
    assert_eq!(
        item_count(&world, 3001, items[1]),
        1,
        "{name}: items[1] given"
    );
    ev(&mut world, "2-2");
    ev(&mut world, "5-1");
    assert_eq!(cond(&world), Some(6), "{name}: cond 6");
    for _ in 0..10 {
        kill(&mut world, GUARDIAN);
    }
    assert_eq!(cond(&world), Some(7), "{name}: angels → cond 7");
    ev(&mut world, "6-1");
    ev(&mut world, "7-1");
    kill(&mut world, mob0);
    assert_eq!(cond(&world), Some(9), "{name}: mob[0] → cond 9");
    ev(&mut world, "7-2");
    ev(&mut world, "3-6");
    inject(&mut world, 3001, 0x0071_0200, items[2], 1);
    ev(&mut world, "3-8");
    assert_eq!(cond(&world), Some(13), "{name}: cond 13");
    ev(&mut world, "8-1");
    ev(&mut world, "11-9");
    assert_eq!(cond(&world), Some(15), "{name}: cond 15");
    kill(&mut world, ARCHON_MINION);
    kill(&mut world, ARCHON_HALISHA);
    assert_eq!(cond(&world), Some(16), "{name}: archon → cond 16");
    ev(&mut world, "9-1");
    ev(&mut world, "10-1");
    ev(&mut world, "4-2");
    assert_eq!(cond(&world), Some(18), "{name}: cond 18");
    ev(&mut world, "10-2");
    assert_eq!(cond(&world), Some(19), "{name}: cond 19");
    ev(&mut world, "0-2");
    assert_eq!(
        item_count(&world, 3001, REWARD_MARK),
        1,
        "{name}: Mark of transfer"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&3001)
            .unwrap()
            .class_id,
        class_id,
        "{name}: transferred to class {class_id}"
    );
}

#[test]
fn quest_fighter_sagas_q71_q78() {
    // (name, start npc[0], prev class, target class, items[12], mob[0])
    run_fighter_saga(
        "Q00071_SagaOfEvasTemplar",
        30852,
        20,
        99,
        [
            7080, 7535, 7081, 7486, 7269, 7300, 7331, 7362, 7393, 7424, 7094, 6482,
        ],
        27287,
    );
    run_fighter_saga(
        "Q00072_SagaOfTheSwordMuse",
        30853,
        21,
        100,
        [
            7080, 7536, 7081, 7487, 7270, 7301, 7332, 7363, 7394, 7425, 7095, 6482,
        ],
        27288,
    );
    run_fighter_saga(
        "Q00073_SagaOfTheDuelist",
        30849,
        2,
        88,
        [
            7080, 7537, 7081, 7488, 7271, 7302, 7333, 7364, 7395, 7426, 7096, 7546,
        ],
        27289,
    );
    run_fighter_saga(
        "Q00074_SagaOfTheDreadnought",
        30850,
        3,
        89,
        [
            7080, 7538, 7081, 7489, 7272, 7303, 7334, 7365, 7396, 7427, 7097, 6480,
        ],
        27290,
    );
    run_fighter_saga(
        "Q00075_SagaOfTheTitan",
        31327,
        46,
        113,
        [
            7080, 7539, 7081, 7490, 7273, 7304, 7335, 7366, 7397, 7428, 7098, 0,
        ],
        27292,
    );
    run_fighter_saga(
        "Q00076_SagaOfTheGrandKhavatari",
        31339,
        48,
        114,
        [
            7080, 7539, 7081, 7491, 7274, 7305, 7336, 7367, 7398, 7429, 7099, 0,
        ],
        27293,
    );
    run_fighter_saga(
        "Q00077_SagaOfTheDominator",
        31336,
        51,
        115,
        [
            7080, 7539, 7081, 7492, 7275, 7306, 7337, 7368, 7399, 7430, 7100, 0,
        ],
        27294,
    );
    run_fighter_saga(
        "Q00078_SagaOfTheDoomcryer",
        31336,
        52,
        116,
        [
            7080, 7539, 7081, 7493, 7276, 7307, 7338, 7369, 7400, 7431, 7101, 0,
        ],
        27295,
    );
}

#[test]
fn quest_rogue_archer_sagas_q79_q84() {
    run_fighter_saga(
        "Q00079_SagaOfTheAdventurer",
        31603,
        8,
        93,
        [
            7080, 7516, 7081, 7494, 7277, 7308, 7339, 7370, 7401, 7432, 7102, 0,
        ],
        27299,
    );
    run_fighter_saga(
        "Q00080_SagaOfTheWindRider",
        31603,
        23,
        101,
        [
            7080, 7517, 7081, 7495, 7278, 7309, 7340, 7371, 7402, 7433, 7103, 0,
        ],
        27300,
    );
    run_fighter_saga(
        "Q00081_SagaOfTheGhostHunter",
        31603,
        36,
        108,
        [
            7080, 7518, 7081, 7496, 7279, 7310, 7341, 7372, 7403, 7434, 7104, 0,
        ],
        27301,
    );
    run_fighter_saga(
        "Q00082_SagaOfTheSagittarius",
        30702,
        9,
        92,
        [
            7080, 7519, 7081, 7497, 7280, 7311, 7342, 7373, 7404, 7435, 7105, 0,
        ],
        27296,
    );
    run_fighter_saga(
        "Q00083_SagaOfTheMoonlightSentinel",
        30702,
        24,
        102,
        [
            7080, 7520, 7081, 7498, 7281, 7312, 7343, 7374, 7405, 7436, 7106, 0,
        ],
        27297,
    );
    run_fighter_saga(
        "Q00084_SagaOfTheGhostSentinel",
        30702,
        37,
        109,
        [
            7080, 7521, 7081, 7499, 7282, 7313, 7344, 7375, 7406, 7437, 7107, 0,
        ],
        27298,
    );
}

#[test]
fn quest_caster_sagas_q85_q90() {
    run_fighter_saga(
        "Q00085_SagaOfTheCardinal",
        30191,
        16,
        97,
        [
            7080, 7522, 7081, 7500, 7283, 7314, 7345, 7376, 7407, 7438, 7087, 0,
        ],
        27267,
    );
    run_fighter_saga(
        "Q00086_SagaOfTheHierophant",
        30191,
        17,
        98,
        [
            7080, 7523, 7081, 7501, 7284, 7315, 7346, 7377, 7408, 7439, 7089, 0,
        ],
        27269,
    );
    run_fighter_saga(
        "Q00087_SagaOfEvasSaint",
        30191,
        30,
        105,
        [
            7080, 7524, 7081, 7502, 7285, 7316, 7347, 7378, 7409, 7440, 7088, 0,
        ],
        27266,
    );
    run_fighter_saga(
        "Q00088_SagaOfTheArchmage",
        30176,
        12,
        94,
        [
            7080, 7529, 7081, 7503, 7286, 7317, 7348, 7379, 7410, 7441, 7082, 0,
        ],
        27250,
    );
    run_fighter_saga(
        "Q00089_SagaOfTheMysticMuse",
        30174,
        27,
        103,
        [
            7080, 7530, 7081, 7504, 7287, 7318, 7349, 7380, 7411, 7442, 7083, 0,
        ],
        27251,
    );
    run_fighter_saga(
        "Q00090_SagaOfTheStormScreamer",
        30175,
        40,
        110,
        [
            7080, 7531, 7081, 7505, 7288, 7319, 7350, 7381, 7412, 7443, 7084, 0,
        ],
        27252,
    );
}

#[test]
fn quest_summoner_dark_dwarf_sagas_q91_q100() {
    run_fighter_saga(
        "Q00091_SagaOfTheArcanaLord",
        31605,
        14,
        96,
        [
            7080, 7604, 7081, 7506, 7289, 7320, 7351, 7382, 7413, 7444, 7110, 0,
        ],
        27313,
    );
    run_fighter_saga(
        "Q00092_SagaOfTheElementalMaster",
        30174,
        28,
        104,
        [
            7080, 7605, 7081, 7507, 7290, 7321, 7352, 7383, 7414, 7445, 7111, 0,
        ],
        27314,
    );
    run_fighter_saga(
        "Q00093_SagaOfTheSpectralMaster",
        30175,
        41,
        111,
        [
            7080, 7606, 7081, 7508, 7291, 7322, 7353, 7384, 7415, 7446, 7112, 0,
        ],
        27315,
    );
    run_fighter_saga(
        "Q00094_SagaOfTheSoultaker",
        30832,
        13,
        95,
        [
            7080, 7533, 7081, 7509, 7292, 7323, 7354, 7385, 7416, 7447, 7085, 0,
        ],
        27257,
    );
    run_fighter_saga(
        "Q00095_SagaOfTheHellKnight",
        31582,
        6,
        91,
        [
            7080, 7532, 7081, 7510, 7293, 7324, 7355, 7386, 7417, 7448, 7086, 0,
        ],
        27258,
    );
    run_fighter_saga(
        "Q00096_SagaOfTheSpectralDancer",
        31582,
        34,
        107,
        [
            7080, 7527, 7081, 7511, 7294, 7325, 7356, 7387, 7418, 7449, 7092, 0,
        ],
        27272,
    );
    run_fighter_saga(
        "Q00097_SagaOfTheShillienTemplar",
        31580,
        33,
        106,
        [
            7080, 7526, 7081, 7512, 7295, 7326, 7357, 7388, 7419, 7450, 7091, 0,
        ],
        27271,
    );
    run_fighter_saga(
        "Q00098_SagaOfTheShillienSaint",
        31581,
        43,
        112,
        [
            7080, 7525, 7081, 7513, 7296, 7327, 7358, 7389, 7420, 7451, 7090, 0,
        ],
        27270,
    );
    run_fighter_saga(
        "Q00099_SagaOfTheFortuneSeeker",
        31594,
        55,
        117,
        [
            7080, 7608, 7081, 7514, 7297, 7328, 7359, 7390, 7421, 7452, 7109, 0,
        ],
        27259,
    );
    run_fighter_saga(
        "Q00100_SagaOfTheMaestro",
        31592,
        57,
        118,
        [
            7080, 7607, 7081, 7515, 7298, 7329, 7360, 7391, 7422, 7453, 7108, 0,
        ],
        27260,
    );
}

/// The shared Saga htmls render with `%questname%` substituted, so one html set
/// serves every Saga: talking the Q70 start NPC shows the intro page whose
/// accept button carries this quest's own name.
#[test]
fn saga_shared_htmls_substitute_questname() {
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(7093, "starter", true)]);
    let start = NPC_OID;
    add_test_npc(&mut world, start, 30849, "Folk", 78, 100, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 76;
        p.class_id = 5; // Paladin — the Q70 prerequisite
    }
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{start}_Quest Q00070_SagaOfThePhoenixKnight")),
    );
    let html = drain(&mut rx)
        .iter()
        .find_map(|p| {
            if p[0] == server_packets::opcodes::NPC_HTML_MESSAGE {
                decode_npc_html(p)
            } else if p[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                r.read_i16()?;
                r.read_i32()?;
                r.read_string()
            } else {
                None
            }
        })
        .expect("Saga intro html");
    // The generic 0-01.htm rendered, with %questname% replaced by Q70's name.
    assert!(
        html.contains("Quest Q00070_SagaOfThePhoenixKnight 0-1"),
        "the accept button carries the substituted quest name: {html}"
    );
    assert!(
        !html.contains("%questname%"),
        "no unsubstituted placeholder left"
    );
}

/// The Saga finale AI: the boss (mob[2]) is driven off after 15 hits (not
/// killed), and only then does the companion offer the reward. Uses Q70.
#[test]
fn saga_finale_boss_retreats_after_15_hits() {
    const BOSS: i32 = 27278; // Q70 mob[2]
    const COMPANION: i32 = 31631; // Q70 npc[4]
    const START: i32 = 30849;
    const I9: i32 = 7423; // items[9], the finale reward
    const I10: i32 = 7093;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(I9, "q", true), (I10, "q", true)]);
    for id in [BOSS, COMPANION] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = if id == BOSS { "Monster" } else { "Folk" }.into();
        t.level = 78;
        t.base_hp_max = 100_000.0; // survives the 15 hits (it retreats, not dies)
        world.data.npc_data.insert_for_test(t);
    }
    let start = NPC_OID;
    add_test_npc(&mut world, start, START, "Folk", 78, 100, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 76;
        p.class_id = 5;
    }
    let q = "Q00070_SagaOfThePhoenixKnight";
    // Seed the quest at cond 17 (the finale).
    {
        let quests = world
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = quests.0.entry(q.to_string()).or_default();
        qs.state = model::quest::state::STARTED;
        qs.vars.insert("cond".to_string(), "17".to_string());
    }
    let ev = |w: &mut World, npc: i32, e: &str| {
        handle_request_bypass_to_server(w, 1, &bypass_body(&format!("npc_{npc}_Quest {q} {e}")));
    };

    // Summon the finale: the boss appears.
    ev(&mut world, start, "10-1");
    let boss = *npcs_of(&mut world, BOSS).first().expect("boss spawned");
    let companion = *npcs_of(&mut world, COMPANION)
        .first()
        .expect("companion spawned");

    // Choreography: the boss and its companion set upon each other.
    let boss_targets_companion = world
        .objects
        .get_component::<AggroList>(&boss)
        .is_some_and(|a| a.0.contains_key(&companion));
    let companion_targets_boss = world
        .objects
        .get_component::<AggroList>(&companion)
        .is_some_and(|a| a.0.contains_key(&boss));
    assert!(boss_targets_companion, "the boss duels the companion");
    assert!(companion_targets_boss, "the companion duels the boss");

    // Before the boss is driven off, the companion refuses the reward.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{companion}_Quest {q}")),
    );
    let before = drain(&mut rx)
        .iter()
        .find_map(|p| {
            if p[0] == server_packets::opcodes::EX {
                let mut r = commons::network::PacketReader::new(&p[1..]);
                r.read_i16()?;
                r.read_i32()?;
                r.read_string()
            } else {
                None
            }
        })
        .unwrap_or_default();
    assert!(
        !before.contains(" 4-2\""),
        "no reward before the boss retreats: {before}"
    );

    // 14 hits leave the boss standing (Quest0 counts from 1); the 15th drives
    // it off.
    for _ in 0..14 {
        combat::npc_receive_damage(&mut world, boss, 3001, 1.0, false);
    }
    assert!(
        !npcs_of(&mut world, BOSS).is_empty(),
        "boss still fighting after 14 hits"
    );
    combat::npc_receive_damage(&mut world, boss, 3001, 1.0, false);
    assert!(
        npcs_of(&mut world, BOSS).is_empty(),
        "boss retreats on the 15th hit"
    );

    // Now the companion offers the reward → items[9], cond 18.
    ev(&mut world, companion, "4-2");
    assert_eq!(item_count(&world, 3001, I9), 1, "finale reward granted");
    assert_eq!(quest_cond(&world, 3001, q), Some(18), "finale → cond 18");
}

/// A Saga progression step broadcasts the tablet glow (`MagicSkillUse` 4546) to
/// the player; the class transfer broadcasts the transform flash (4339).
#[test]
fn saga_progression_casts_visual_fx() {
    const START: i32 = 30849;
    const I4: i32 = 7268; // Q70 items[4]
    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(I4, "q", true)]);
    let start = NPC_OID;
    add_test_npc(&mut world, start, START, "Folk", 78, 100, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 76;
        p.class_id = 5;
    }
    let q = "Q00070_SagaOfThePhoenixKnight";
    {
        let quests = world
            .objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap();
        let qs = quests.0.entry(q.to_string()).or_default();
        qs.state = model::quest::state::STARTED;
        qs.vars.insert("cond".to_string(), "5".to_string());
    }
    inject(&mut world, 3001, 0x0070_4000, I4, 1);

    // A tablet step ("5-1") glows with skill 4546.
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{start}_Quest {q} 5-1")),
    );
    let skill_of = |p: &[u8]| -> Option<i32> {
        // MAGIC_SKILL_USE: op, i32 castbar, i32 caster, i32 target, i32 skillId..
        if p.first() != Some(&server_packets::opcodes::MAGIC_SKILL_USE) {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&p[1..]);
        r.read_i32()?; // cast bar
        r.read_i32()?; // caster
        r.read_i32()?; // target
        r.read_i32() // skill id
    };
    let casts: Vec<i32> = drain(&mut rx).iter().filter_map(|p| skill_of(p)).collect();
    assert!(
        casts.contains(&4546),
        "the tablet glow (4546) was cast: {casts:?}"
    );
}

/// During the finale duel the companion keeps up a timed battle-banter cadence:
/// a first line ~4s in, then reschedules every 12s while the boss stands. Once
/// the boss retreats (`Tab` set), the cadence lapses on its next firing.
#[test]
fn saga_finale_companion_taunt_cadence() {
    const BOSS: i32 = 27278; // Q70 mob[2]
    const COMPANION: i32 = 31631; // Q70 npc[4]
    const START: i32 = 30849;

    let (mut world, _db, _l) = quest_test_world();
    for id in [BOSS, COMPANION] {
        let mut t = crate::data::npc_data::default_template(id);
        t.type_name = if id == BOSS { "Monster" } else { "Folk" }.into();
        t.level = 78;
        t.base_hp_max = 100_000.0;
        world.data.npc_data.insert_for_test(t);
    }
    let start = NPC_OID;
    add_test_npc(&mut world, start, START, "Folk", 78, 100, 200, 0);
    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    {
        let p = world.objects.get_component_mut::<Player>(&3001).unwrap();
        p.level = 76;
        p.class_id = 5;
    }
    let q = "Q00070_SagaOfThePhoenixKnight";
    let set_var = |w: &mut World, key: &str, val: &str| {
        w.objects
            .get_component_mut::<model::components::social::Quests>(&3001)
            .unwrap()
            .0
            .entry(q.to_string())
            .or_default()
            .vars
            .insert(key.to_string(), val.to_string());
    };
    set_var(&mut world, "cond", "17");
    world
        .objects
        .get_component_mut::<model::components::social::Quests>(&3001)
        .unwrap()
        .0
        .get_mut(q)
        .unwrap()
        .state = model::quest::state::STARTED;

    // Summon the finale (starts the taunt timer).
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{start}_Quest {q} 10-1")),
    );
    let companion = *npcs_of(&mut world, COMPANION)
        .first()
        .expect("companion spawned");
    // The opening COMPANION_CALL fires immediately; drain it so we only capture
    // the *timed* banter that follows.
    let _ = drain(&mut rx);

    // Decode NpcSay-text (op 0x30): oid, chatType, 1_000_000+npcId, -1, string.
    let taunt_from = |p: &[u8], oid: i32| -> Option<String> {
        if p.first() != Some(&server_packets::opcodes::NPC_SAY) {
            return None;
        }
        let mut r = commons::network::PacketReader::new(&p[1..]);
        (r.read_i32()? == oid).then_some(())?;
        r.read_i32()?; // chat type
        r.read_i32()?; // 1_000_000 + npc id
        r.read_i32()?; // -1 (literal string marker)
        r.read_string()
    };

    // ~4s in: the first timed taunt lands, from the companion.
    advance_ticks(&mut world, 41);
    let first: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| taunt_from(p, companion))
        .collect();
    assert!(
        first
            .iter()
            .any(|s| s == "Hold your ground — its strength wanes!"),
        "companion's first timed taunt: {first:?}"
    );

    // The cadence reschedules: ~12s later a *second*, different line lands.
    advance_ticks(&mut world, 121);
    let second: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| taunt_from(p, companion))
        .collect();
    assert!(
        second.iter().any(|s| s == "Strike now, while it staggers!"),
        "companion's second timed taunt cycles the line: {second:?}"
    );

    // Once the boss retreats (Tab set), the cadence lapses on its next firing.
    set_var(&mut world, "Tab", "1");
    advance_ticks(&mut world, 121);
    let after: Vec<String> = drain(&mut rx)
        .iter()
        .filter_map(|p| taunt_from(p, companion))
        .collect();
    assert!(
        after.is_empty(),
        "cadence stops after the boss retreats: {after:?}"
    );
}
