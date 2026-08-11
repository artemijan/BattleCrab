use super::*;

use crate::data::item_data::{
    CrystalType, EtcItemType, ItemHandler, ItemKind, ItemTemplate, SLOT_R_HAND,
};
use crate::data::recipe_data::{RareProduction, RecipeList};
use crate::model::components::{RecipeBook, SkillBook};
use crate::model::inventory::Inventory;

const MATERIAL: i32 = 1869; // Iron Ore (stackable)
const PRODUCT: i32 = 3; // Broadsword (weapon)
const RECIPE_ITEM: i32 = 1786; // "Recipe: Broadsword" etc item
const RECIPE_LIST: i32 = 2; // dwarven, 100% success
const RECIPE_LIST_FAIL: i32 = 20; // dwarven, 0% success
const RECIPE_LIST_RARE: i32 = 30; // dwarven, has a masterwork rare
const RARE_PRODUCT: i32 = 4;

fn etc_template(item_id: i32, name: &str, stackable: bool, handler: ItemHandler) -> ItemTemplate {
    ItemTemplate {
        trade_flags: Default::default(),
        time: -1,
        duration: -1,
        immediate_effect: false,
        ex_immediate_effect: false,
        default_action: crate::data::item_data::ActionType::Other,
        item_id,
        name: name.into(),
        kind: ItemKind::Etc,
        body_part: 0,
        weight: 0,
        is_stackable: stackable,
        is_infinite: false,
        type1: 0,
        type2: 0,
        is_quest_item: false,
        is_sellable: true,
        is_freightable: false,
        price: 10,
        handler,
        crystal_type: CrystalType::None,
        crystal_count: 0,
        attack_radius: 40,
        attack_angle: 0,
        mp_consume: 0,
        reduced_mp_consume: 0,
        reduced_mp_consume_chance: 0,
        capsuled_items: Vec::new(),
        extractable_count_min: 0,
        extractable_count_max: 0,
        item_skills: Vec::new(),
        etc_item_type: EtcItemType::Other,
        enchant_enabled: false,
        enchant_limit: 0,
        is_magic_weapon: false,
    }
}

fn weapon_template(item_id: i32, name: &str) -> ItemTemplate {
    ItemTemplate {
        kind: ItemKind::Weapon,
        body_part: SLOT_R_HAND,
        ..etc_template(item_id, name, false, ItemHandler::None)
    }
}

fn recipe(id: i32, success_rate: i32, rare: Option<RareProduction>) -> RecipeList {
    RecipeList {
        id,
        level: 1,
        recipe_item_id: RECIPE_ITEM,
        name: format!("mk_{id}"),
        success_rate,
        item_id: PRODUCT,
        count: 1,
        rare,
        is_dwarven: true,
        ingredients: vec![(MATERIAL, 2)],
        mp_use: 30,
        hp_use: 0,
        alt_exp: None,
        alt_sp: None,
        alt_gim: 1,
    }
}
/// Insert the item + recipe fixtures into a `cast_test_world`, and give the
/// world a live object-id pool so crafted items can be allocated.
fn install_fixtures(world: &mut World) {
    world.id_pool = 0x4000_0000..0x4000_0100;
    world
        .data
        .item_data
        .insert_for_test(etc_template(57, "Adena", true, ItemHandler::None));
    world.data.item_data.insert_for_test(etc_template(
        MATERIAL,
        "Iron Ore",
        true,
        ItemHandler::None,
    ));
    world.data.item_data.insert_for_test(etc_template(
        RECIPE_ITEM,
        "Recipe: Broadsword",
        false,
        ItemHandler::Recipes,
    ));
    world
        .data
        .item_data
        .insert_for_test(weapon_template(PRODUCT, "Broadsword"));
    world
        .data
        .item_data
        .insert_for_test(weapon_template(RARE_PRODUCT, "Broadsword*"));
    world
        .data
        .recipes
        .insert_for_test(recipe(RECIPE_LIST, 100, None));
    world
        .data
        .recipes
        .insert_for_test(recipe(RECIPE_LIST_FAIL, 0, None));
    world.data.recipes.insert_for_test(recipe(
        RECIPE_LIST_RARE,
        100,
        Some(RareProduction {
            item_id: RARE_PRODUCT,
            count: 1,
            rarity: 100,
        }),
    ));
    // The recipe item (1786) teaches whichever list `by_recipe_item_id` resolves;
    // insert_for_test keys `by_item` by the last recipe with that recipe_item_id.
    // Re-insert the canonical teaching target so learning maps to RECIPE_LIST.
    world
        .data
        .recipes
        .insert_for_test(recipe(RECIPE_LIST, 100, None));
}

fn learn_skill(world: &mut World, oid: i32, skill_id: i32) {
    world
        .objects
        .get_component_mut::<SkillBook>(&oid)
        .unwrap()
        .0
        .insert(skill_id, 1);
}

/// Using a recipe item registers it and consumes the item.
#[test]
fn using_recipe_item_registers_recipe() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172); // Create Dwarven
    give_item(&mut world, 3001, 9001, RECIPE_ITEM, 1);
    drain(&mut rx);

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    assert!(
        world
            .objects
            .get_component::<RecipeBook>(&3001)
            .unwrap()
            .dwarven
            .contains(&RECIPE_LIST),
        "recipe registered"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(RECIPE_ITEM),
        0,
        "recipe item consumed"
    );
    let out = drain(&mut rx);
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE
                && sm_id(p) == server_packets::sm_ids::S1_HAS_BEEN_ADDED),
        "S1_HAS_BEEN_ADDED sent"
    );
}

/// Learning without the craft skill is refused (no book change).
#[test]
fn learn_refused_without_craft_ability() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    give_item(&mut world, 3001, 9001, RECIPE_ITEM, 1);
    drain(&mut rx);

    items::handle_use_item(&mut world, 1, &use_item_body(9001));

    assert!(
        world
            .objects
            .get_component::<RecipeBook>(&3001)
            .unwrap()
            .dwarven
            .is_empty(),
        "not registered"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .count_of(RECIPE_ITEM),
        1,
        "recipe item kept"
    );
    let out = drain(&mut rx);
    assert!(has_sm(&out, server_packets::sm_ids::THE_RECIPE_CANNOT_BE_REGISTERED_YOU_DO_NOT_HAVE_THE_ABILITY_TO_CREATE_ITEMS));
}

/// Self-craft success: materials + MP consumed, product created, make-info +
/// earned message sent.
#[test]
fn self_craft_success_consumes_and_rewards() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172);
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven
        .push(RECIPE_LIST);
    give_item(&mut world, 3001, 9001, MATERIAL, 10);
    let mp0 = pvit(&world, 3001).cur_mp;
    drain(&mut rx);

    crafting::handle_make_self(&mut world, 1, RECIPE_LIST);

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(PRODUCT), 1, "product crafted");
    assert_eq!(inv.count_of(MATERIAL), 8, "2 materials consumed");
    assert_eq!(pvit(&world, 3001).cur_mp, mp0 - 30.0, "MP consumed");
    let out = drain(&mut rx);
    assert!(
        out.iter()
            .any(|p| p[0] == server_packets::opcodes::RECIPE_ITEM_MAKE_INFO && p[17] == 1),
        "RecipeItemMakeInfo success=1 sent"
    );
    assert!(
        has_sm(&out, server_packets::sm_ids::YOU_HAVE_EARNED_S1),
        "earned message"
    );
}

/// A missing material aborts before any consumption.
#[test]
fn self_craft_missing_material_aborts() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172);
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven
        .push(RECIPE_LIST);
    give_item(&mut world, 3001, 9001, MATERIAL, 1); // need 2, only 1
    let mp0 = pvit(&world, 3001).cur_mp;
    drain(&mut rx);

    crafting::handle_make_self(&mut world, 1, RECIPE_LIST);

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(PRODUCT), 0, "nothing crafted");
    assert_eq!(inv.count_of(MATERIAL), 1, "material not consumed");
    assert_eq!(pvit(&world, 3001).cur_mp, mp0, "MP not consumed");
    let out = drain(&mut rx);
    assert!(
        has_sm(&out, server_packets::sm_ids::YOU_NEED_S2_MORE_S1_S),
        "need-more message"
    );
}

/// Failure roll (0% success) still consumes materials but produces nothing.
#[test]
fn self_craft_failure_consumes_materials_no_product() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172);
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven
        .push(RECIPE_LIST_FAIL);
    give_item(&mut world, 3001, 9001, MATERIAL, 10);
    drain(&mut rx);

    crafting::handle_make_self(&mut world, 1, RECIPE_LIST_FAIL);

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(PRODUCT), 0, "no product on failure");
    assert_eq!(inv.count_of(MATERIAL), 8, "materials still consumed");
    let out = drain(&mut rx);
    assert!(
        has_sm(&out, server_packets::sm_ids::YOU_FAILED_AT_MIXING_THE_ITEM),
        "mixing-failed message"
    );
}

/// Masterwork: a recipe with a rare production and a rolled hit yields the rare item.
#[test]
fn self_craft_rolls_masterwork_rare() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172);
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven
        .push(RECIPE_LIST_RARE);
    give_item(&mut world, 3001, 9001, MATERIAL, 10);
    // roll order: success (< 100) then masterwork (<= 100). Force both to hit.
    world.forced_rolls.extend([0, 0]);
    drain(&mut rx);

    crafting::handle_make_self(&mut world, 1, RECIPE_LIST_RARE);

    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(RARE_PRODUCT), 1, "rare masterwork crafted");
    assert_eq!(inv.count_of(PRODUCT), 0, "normal product not also given");
}

/// Manufacture store end-to-end: a customer buys a craft from another player's
/// store — product + materials on the customer, adena to the manufacturer, MP
/// off the manufacturer.
#[test]
fn manufacture_store_craft_for_customer() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    let mut m_rx = ingame_caster(&mut world, 1, 3001, 0, 0); // manufacturer
    let mut c_rx = ingame_caster(&mut world, 2, 3002, 20, 0); // customer nearby
    learn_skill(&mut world, 3001, 172);
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven
        .push(RECIPE_LIST);
    give_item(&mut world, 3002, 9101, MATERIAL, 10); // customer holds materials
    give_item(&mut world, 3002, 9102, 57, 5000); // customer adena
    let mp0 = pvit(&world, 3001).cur_mp;

    // Manufacturer opens a store selling the craft at 1000 adena.
    crafting::handle_list_set(
        &mut world,
        1,
        vec![cp::ManufactureLine {
            recipe_id: RECIPE_LIST,
            cost: 1000,
        }],
    );
    assert!(crafting::is_manufacture_owner(&world, 3001), "store active");
    drain(&mut m_rx);
    drain(&mut c_rx);

    crafting::handle_shop_make_item(&mut world, 2, 3001, RECIPE_LIST);

    let cust = world.objects.get_component::<Inventory>(&3002).unwrap();
    assert_eq!(cust.count_of(PRODUCT), 1, "product delivered to customer");
    assert_eq!(cust.count_of(MATERIAL), 8, "customer materials consumed");
    assert_eq!(cust.adena(), 4000, "customer paid 1000 adena");
    assert_eq!(
        world
            .objects
            .get_component::<Inventory>(&3001)
            .unwrap()
            .adena(),
        1000,
        "manufacturer received fee"
    );
    assert_eq!(
        pvit(&world, 3001).cur_mp,
        mp0 - 30.0,
        "manufacturer MP consumed"
    );
    // Customer sees the earned message.
    assert!(has_sm(
        &drain(&mut c_rx),
        server_packets::sm_ids::YOU_HAVE_EARNED_S1
    ));
}

/// G19 `EnlargeSlot`: Expand Dwarven Craft (1368, real dist data) raises
/// `dwarf_recipe_limit` via `Stat::RecipeDwarven`. Before this slice the
/// effect fell through (unregistered `<effect name>`) and the skill did
/// nothing — a book capped at the config base stayed capped forever.
/// Learning it goes through the same `recompute_conditioned_passives` path
/// `handle_request_acquire_skill` now calls, so this also exercises that a
/// freshly learned passive contributes its stat immediately.
#[test]
fn enlarge_slot_expand_dwarven_craft_raises_recipe_limit() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    world.data.skill_data = crate::data::skill_data::SkillData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172); // Create Dwarven
    give_item(&mut world, 3001, 9001, RECIPE_ITEM, 1);

    // Fill the book to the config base limit with unrelated ids.
    let base = world.cfg.character.dwarf_recipe_limit;
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven = (100..100 + base).collect();
    drain(&mut rx);

    items::handle_use_item(&mut world, 1, &use_item_body(9001));
    assert!(
        !world
            .objects
            .get_component::<RecipeBook>(&3001)
            .unwrap()
            .dwarven
            .contains(&RECIPE_LIST),
        "refused at the base limit"
    );
    assert!(has_sm(
        &drain(&mut rx),
        server_packets::sm_ids::UP_TO_S1_RECIPES_CAN_BE_REGISTERED
    ));

    // Learn Expand Dwarven Craft lvl1 (+6, real dist data) and retry.
    learn_skill(&mut world, 3001, 1368);
    crate::game_loop::passive_skills::recompute_conditioned_passives(&mut world, 3001);
    give_item(&mut world, 3001, 9002, RECIPE_ITEM, 1);
    drain(&mut rx);

    items::handle_use_item(&mut world, 1, &use_item_body(9002));
    assert!(
        world
            .objects
            .get_component::<RecipeBook>(&3001)
            .unwrap()
            .dwarven
            .contains(&RECIPE_LIST),
        "registered once the passive raises the limit past the book's current size"
    );
}

/// **The `UserInfo` STATUS block's craft byte opens the client's create-item
/// window**, and it was hard-coded 0 — so the whole (ported) crafting subsystem
/// was unreachable from the UI. Java's condition is
/// `hasDwarvenCraft() || getSkillLevel(248) > 0`: Create Item (172), or
/// Crystallize (248) for a non-Dwarf.
///
/// Rather than hard-code a byte offset into a block-structured packet, this
/// builds the same `UserInfo` twice and asserts the *only* difference is one
/// byte flipping 0 → 1.
#[test]
fn user_info_advertises_the_craft_window_to_a_crafter() {
    const OID: i32 = 7401;

    let build = |world: &World| {
        let v = crate::model::PlayerView::of(&world.objects, OID).expect("a live player");
        crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, 0)
    };

    for skill_id in [172, 248] {
        let (mut world, ..) = cast_test_world();
        let _rx = ingame_caster(&mut world, 1, OID, 0, 0);
        let before = build(&world);

        world
            .objects
            .get_component_mut::<SkillBook>(&OID)
            .unwrap()
            .0
            .insert(skill_id, 1);
        let after = build(&world);

        assert_eq!(before.len(), after.len(), "same packet shape");
        let diffs: Vec<usize> = (0..before.len())
            .filter(|&i| before[i] != after[i])
            .collect();
        assert_eq!(
            diffs.len(),
            1,
            "skill {skill_id} flips exactly one byte, got {diffs:?}"
        );
        assert_eq!((before[diffs[0]], after[diffs[0]]), (0, 1));
    }
}

/// Three more `UserInfo` fields that were hard-coded 0 while the state backing
/// them existed: the **large clan crest** (mirrored onto the player like the
/// small one, because the builder cannot reach `World.clans`), the **raid-boss
/// points**, and the **cursed-weapon stage** that colours a wielder's name.
///
/// Same diff technique as the craft byte — build the packet twice and compare —
/// so no byte offsets are baked in.
#[test]
fn user_info_carries_the_crest_raid_points_and_cursed_stage() {
    const OID: i32 = 7402;

    let build = |world: &World| {
        let v = crate::model::PlayerView::of_world(world, OID).expect("a live player");
        crate::network::user_info::user_info(&v, &world.data, &world.cfg.character, 0)
    };
    let one_byte_diff = |before: &[u8], after: &[u8]| -> Vec<usize> {
        assert_eq!(before.len(), after.len(), "same packet shape");
        (0..before.len())
            .filter(|&i| before[i] != after[i])
            .collect()
    };

    // --- the large clan crest ---
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, OID, 0, 0);
    let before = build(&world);
    world
        .objects
        .get_component_mut::<Player>(&OID)
        .unwrap()
        .clan_crest_large_id = 0x0A0B_0C0D;
    assert_eq!(
        one_byte_diff(&before, &build(&world)).len(),
        4,
        "the large crest is a dword of its own"
    );

    // --- raid-boss points ---
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, OID, 0, 0);
    let before = build(&world);
    world
        .objects
        .get_component_mut::<Player>(&OID)
        .unwrap()
        .raidboss_points = 0x1112_1314;
    assert_eq!(one_byte_diff(&before, &build(&world)).len(), 4);

    // --- the cursed-weapon stage ---
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, 1, OID, 0, 0);
    let before = build(&world);
    world
        .objects
        .get_component_mut::<Player>(&OID)
        .unwrap()
        .cursed_weapon_equipped_id = 8190;
    // Equipped but the weapon is not in the world's list yet → still stage 0.
    assert!(
        one_byte_diff(&before, &build(&world)).is_empty(),
        "an unknown cursed weapon reports no stage"
    );
    // Load the real Zariche/Akamanah definitions, then mark one wielded.
    world.data.cursed_weapons = crate::data::CursedWeaponData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.cursed_weapons = world.data.cursed_weapons.weapons.clone();
    let cw = world
        .cursed_weapons
        .iter_mut()
        .find(|c| c.item_id == 8190)
        .expect("Zariche 8190 is in the dist");
    cw.is_activated = true;
    cw.nb_kills = 12;
    cw.stage_kills = 3;
    cw.skill_max_level = 10;
    let diffs = one_byte_diff(&before, &build(&world));
    assert_eq!(diffs.len(), 1, "the stage is a single byte");
    let after = build(&world);
    assert_eq!(after[diffs[0]], 5, "1 + 12/3 = stage 5");
}

/// **`AltGameCreation = True` stages the craft** (G33): the session parks on
/// the crafter (`isCrafting` — the offline-mode gate observes it), nothing is
/// consumed up front, each pass pays its share of the stat use and "equips" a
/// grab of materials with SM 368, and the finish settles: materials taken,
/// product granted, session gone.
#[test]
fn alt_game_creation_stages_the_craft() {
    let (mut world, ..) = cast_test_world();
    install_fixtures(&mut world);
    world.cfg.character.alt_game_creation = true;
    // Zero the XP/SP knobs: the award path would level the test player against
    // this world's EMPTY exp tables, whose recompute clamps vitals to a
    // missing template (see the empty-fixture-tables trap).
    world.cfg.character.alt_game_creation_xp_rate = 0.0;
    world.cfg.character.alt_game_creation_sp_rate = 0.0;
    let mut rx = ingame_caster(&mut world, 1, 3001, 0, 0);
    learn_skill(&mut world, 3001, 172); // create-dwarven lvl 1 → grab 1/pass → 2 passes
    world
        .objects
        .get_component_mut::<RecipeBook>(&3001)
        .unwrap()
        .dwarven
        .push(RECIPE_LIST);
    give_item(&mut world, 3001, 9001, MATERIAL, 10);
    let mp0 = pvit(&world, 3001).cur_mp;
    drain(&mut rx);

    crafting::handle_make_self(&mut world, 1, RECIPE_LIST);
    assert!(
        crafting::is_crafting(&world, 3001),
        "the staged session parked on the crafter"
    );
    {
        let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
        assert_eq!(inv.count_of(MATERIAL), 10, "nothing consumed up front");
        assert_eq!(inv.count_of(PRODUCT), 0);
    }

    // First pass: half the stat use (30 / 2 passes), one material grabbed.
    advance_ticks(&mut world, 1);
    assert_eq!(pvit(&world, 3001).cur_mp, mp0 - 15.0, "per-pass stat share");
    assert!(crafting::is_crafting(&world, 3001), "still crafting");
    assert!(
        has_sm(&drain(&mut rx), server_packets::sm_ids::EQUIPPED_S1_S2),
        "the grab announced itself"
    );

    // Second pass + the finish settle.
    advance_ticks(&mut world, 6);
    assert!(!crafting::is_crafting(&world, 3001), "session closed");
    let inv = world.objects.get_component::<Inventory>(&3001).unwrap();
    assert_eq!(inv.count_of(PRODUCT), 1, "product crafted");
    assert_eq!(inv.count_of(MATERIAL), 8, "materials settled at the finish");
    assert_eq!(pvit(&world, 3001).cur_mp, mp0 - 30.0, "full stat use paid");
    assert!(
        has_sm(&drain(&mut rx), server_packets::sm_ids::YOU_HAVE_EARNED_S1),
        "earned message"
    );
}
