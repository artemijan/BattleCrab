use gameserver::data::GameData;
use gameserver::model::Player;
use gameserver::network::user_info::user_info;

#[tokio::test]
async fn user_info_test() {
    // Assuming you have your imports and a mock user object:
    // let user = User { id: "some_account_name".to_string() };
    // let object_id = 268_476_204;

    let player = Player {
        object_id: 268_476_204,
        name: "Adelante".to_string(),
        account: "Adelante".to_string(), // Maps to m.user_id
        title: "".to_string(),

        level: 1,
        class_id: 10,
        base_class_id: 10, // Defaulting to class_id
        race: 0,           // Maps to m.race_id
        is_female: true,

        x: -90939,
        y: 248_138,
        z: -3563,
        heading: 0,

        // Base primary stats (Placeholders until template lookup is hooked up)
        str_: 22,
        dex: 21,
        con: 27,
        int_: 41,
        wit: 20,
        men: 39,

        // Max values are cast to i32 to match your Player struct definition
        max_hp: 98, // 98.00 -> i32
        cur_hp: 98.00,
        max_mp: 59, // 59.00 -> i32
        cur_mp: 59.00,
        max_cp: 49, // 49.00 -> i32
        cur_cp: 49.00,
        exp: 0,
        sp: 0,
        reputation: 0,
        pk_kills: 0,
        pvp_kills: 0,
        vitality_points: 0,
        fame: 0,

        // Extracted from your m.variables JSON string / sample fields
        face: 1,       // m.face / visualFaceId
        hair_style: 3, // visualHairStyleId
        hair_color: 2, // visualHairColorId

        // Combat stats (Placeholders)
        p_atk: 0,
        p_atk_spd: 0,
        p_def: 0,
        m_atk: 0,
        m_atk_spd: 0,
        m_def: 0,
        crit_hit: 0,
        m_crit_hit: 0,
        evasion: 0,
        accuracy: 31,
        magic_evasion: 0,
        magic_accuracy: 31,
        atk_range: 0,

        // Movement & Collision (Placeholders)
        run_spd: 0,
        walk_spd: 0,
        swim_run_spd: 0,
        swim_walk_spd: 0,
        move_multiplier: 1.0,
        collision_radius: 0.0,
        collision_height: 0.0,
        running: true,

        inventory: Default::default(),
    };
    let gd = GameData::load_from("../../dist/game/");
    let packet = user_info(&player, &gd);
    assert_eq!(
        vec![
            50, 44, 159, 0, 16, 137, 1, 0, 0, 23, 0, 255, 255, 254, 0, 0, 0, 0, 32, 0, 8, 0, 65, 0,
            100, 0, 101, 0, 108, 0, 97, 0, 110, 0, 116, 0, 101, 0, 0, 0, 1, 10, 0, 0, 0, 10, 0, 0,
            0, 1, 18, 0, 22, 0, 21, 0, 27, 0, 41, 0, 20, 0, 39, 0, 0, 0, 0, 0, 14, 0, 98, 0, 0, 0,
            59, 0, 0, 0, 49, 0, 0, 0, 38, 0, 98, 0, 0, 0, 59, 0, 0, 0, 49, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 15, 0, 3, 0, 0, 0,
            2, 0, 0, 0, 1, 0, 0, 0, 1, 6, 0, 0, 0, 0, 0, 56, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 31, 0, 0, 0, 0, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            18, 0, 197, 156, 254, 255, 74, 201, 3, 0, 21, 242, 255, 255, 0, 0, 0, 0, 18, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 240, 63, 0, 0, 0, 0,
            0, 0, 240, 63, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0,
            32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 1, 10, 0, 255,
            255, 255, 0, 119, 255, 255, 0, 9, 0, 0, 0, 0, 0, 80, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0
        ],
        packet
    );
}
