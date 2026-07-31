use gameserver::config::CharacterConfig;
use gameserver::data::GameData;
use gameserver::model::Player;
use gameserver::model::PlayerView;
use gameserver::model::components::{
    BaseStats, Collision, CombatStats, PlayerVitals, Position, Speeds, StatModifiers, Vitals,
};
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

        access_level: 0,
        name_color: gameserver::model::DEFAULT_NAME_COLOR,
        title_color: gameserver::model::DEFAULT_TITLE_COLOR,

        hero_aura: false,

        is_noble: false,

        class_index: 0,

        subclasses: Vec::new(),
        skills_by_index: Default::default(),
        team: 0,
        on_event: false,
        registered_on_event: false,
        hennas_by_index: Default::default(),
        shortcuts_by_index: Default::default(),

        base_level: 1,

        base_exp: 0,

        base_sp: 0,
        is_hero: false,
        level: 1,
        class_id: 10,
        base_class_id: 10, // Defaulting to class_id
        race: 0,           // Maps to m.race_id
        is_female: true,

        exp: 0,
        sp: 0,
        reputation: 0,
        pk_kills: 0,
        pvp_kills: 0,
        raidboss_points: 0,
        cursed_weapon_equipped_id: 0,
        charges: 0,
        vitality_points: 0,
        pccafe_points: 0,
        prime_points: 0,
        fame: 0,
        // The golden packet below was captured with recom-left/have = 0.
        rec_have: 0,
        rec_left: 0,
        reco_two_hours_given: false,
        reco_give_seq: 0,

        // Extracted from your m.variables JSON string / sample fields
        clan_id: 0,
        clan_privs: 0,
        clan_leader: false,
        pledge_class: 0,
        clan_create_expiry_time: 0,
        clan_join_expiry_time: 0,
        create_date: String::new(),
        power_grade: 0,
        ally_id: 0,
        siege_state: 0,
        siege_side: 0,
        pledge_type: 0,
        lvl_joined_academy: 0,
        apprentice: 0,
        sponsor: 0,
        clan_crest_id: 0,
        ally_crest_id: 0,
        face: 1,       // m.face / visualFaceId
        hair_style: 3, // visualHairStyleId
        hair_color: 2, // visualHairColorId

        cast_seq: 0,
        pending_revive: false,
        teleporting: false,
        jailed: false,
        sitting: false,
        last_petition_gm_name: None,
        snoop_listeners: Vec::new(),
        snooped: Vec::new(),
        quest_zone_id: 0,
        charged_shots: 0,
        auto_shots: Vec::new(),
        mount_type: 0,
        mount_npc_id: 0,
        mount_level: 0,
        trade_refusal: false,
        cond_overrides: 0,
        transform_id: 0,
        transform_display_id: 0,
        store_type: 0,
        lost_exp_on_death: 0,
        revive_request: None,
        pending_pet_collar: None,
    };
    let position = Position {
        x: -90939,
        y: 248_138,
        z: -3563,
        heading: 0,
    };
    let vitals = Vitals {
        max_hp: 98,
        cur_hp: 98.0,
        max_mp: 59,
        cur_mp: 59.0,
        dead: false,
    };
    let pvitals = PlayerVitals {
        max_cp: 49,
        cur_cp: 49.0,
    };
    let base = BaseStats {
        str_: 22,
        dex: 21,
        con: 27,
        int_: 41,
        wit: 20,
        men: 39,
    };
    let speeds = Speeds {
        run_spd: 0.0,
        walk_spd: 0.0,
        swim_run_spd: 0.0,
        swim_walk_spd: 0.0,
        move_multiplier: 1.0,
        base_run_spd: 0.0,
        running: true,
        swimming: false,
        swamp_multiplier: 1.0,
    };
    let collision = Collision {
        radius: 0.0,
        height: 0.0,
    };
    let combat = CombatStats {
        accuracy: 31,
        magic_accuracy: 31,
        ..Default::default()
    };
    let inventory = gameserver::model::inventory::Inventory::default();
    let mods = StatModifiers::default();
    let view = PlayerView {
        p: &player,
        pos: &position,
        vitals: &vitals,
        pvitals: &pvitals,
        base: &base,
        speeds: &speeds,
        collision: &collision,
        combat: &combat,
        inventory: &inventory,
        pvp_flag: 0,
        in_matching_room: false,
        mods: &mods,
    };
    let gd = GameData::load_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/"));
    let cfg = CharacterConfig::default();
    // clan_id 0 + empty inventory → relation 0 and weapon-enchant 0, so the
    // golden bytes below are unchanged by the ENCHANTLEVEL/RELATION fills.
    let packet = user_info(&view, &gd, &cfg, 0);
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
            0, 0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 32,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 1, 10, 0, 255,
            255, 255, 0, 119, 255, 255, 0, 9, 0, 0, 0, 0, 0, 80, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0
        ],
        packet
    );
}
