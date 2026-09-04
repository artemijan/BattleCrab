//! Fishing: casting, the win rolls, the shot and premium modifiers, and the
//! zone gate on auto-fish.

use super::*;

/// Fishing (G32) — the gate line: **cast, hook, and land a fish.** With a rod
/// equipped and a bait hooked, toggling auto-fish casts the line; after the
/// bait's reel time the cast lands (forced win), consuming one bait and awarding
/// a fish + XP.
#[test]
fn fishing_cast_hook_and_land_a_fish() {
    use crate::data::fishing_data::{FishingBait, FishingCatch, FishingRod};
    use crate::data::item_data::kinds::WeaponType;
    use crate::model::inventory::{Inventory, PaperdollSlot};

    const ROD: i32 = 45492;
    const BAIT: i32 = 47547;
    const FISH: i32 = 47550;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (ROD, "Fishing Rod", false),
            (BAIT, "Bait", true),
            (FISH, "Ugly Fish", true),
        ],
    );
    world
        .data
        .item_data
        .set_weapon_type_for_test(ROD, WeaponType::FishingRod);
    world
        .data
        .fishing_data
        .insert_rod_for_test(ROD, FishingRod::default());
    world.data.fishing_data.insert_bait_for_test(
        BAIT,
        FishingBait {
            min_player_level: 1,
            max_player_level: 100,
            chance: 40,
            time_min: 1000,
            time_max: 1000,
            wait_min: 1000,
            wait_max: 1000,
            premium_only: false,
            catches: vec![FishingCatch {
                item_id: FISH,
                chance: 100,
                multiplier: 1,
            }],
        },
    );

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 20;
    // Equip the rod (right hand) and hook the bait (left hand), 5 baits held.
    let inv = Inventory::from_rows(&[
        item_row(0x4700_0001, ROD, 1, PaperdollSlot::RHand),
        item_row(0x4700_0002, BAIT, 5, PaperdollSlot::LHand),
    ]);
    world.objects.add_components(&3001, inv);
    // The shore fishing zone holds the player (100,200); the adjacent water zone
    // holds the bob (the heading-0 cast lands 90 units east, at ~190,200). They
    // must not overlap — a player standing *in* water can't fish.
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Fishing,
        0,
        160,
        0,
        1000,
    );
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Water,
        160,
        1000,
        0,
        1000,
    );

    // Cast: the reel is scheduled for the bait's time (1000 ms → tick +10).
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001);
    assert_eq!(item_count(&world, 3001, FISH), 0, "no catch mid-cast");

    // Force a win (roll ≤ 40) then the first catch (roll → Ugly Fish).
    world.force_roll(0); // reel win roll
    world.force_roll(0); // catch-table roll
    advance_ticks(&mut world, 12); // past the 10-tick reel time

    assert_eq!(item_count(&world, 3001, FISH), 1, "landed a fish");
    assert_eq!(item_count(&world, 3001, BAIT), 4, "one bait consumed");

    // --- Away from any fishing zone, the cast can't start. ---
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 50_000; // out of the synthetic zones
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001); // stop (was still fishing)
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001); // try to start
    world.force_roll(0);
    world.force_roll(0);
    advance_ticks(&mut world, 12);
    assert_eq!(
        item_count(&world, 3001, FISH),
        1,
        "no fishing outside a fishing zone"
    );
}

/// Build a `PAPERDOLL`-located `ItemRow` for a fishing-fixture inventory.
fn item_row(
    object_id: i32,
    item_id: i32,
    count: i64,
    slot: model::inventory::PaperdollSlot,
) -> crate::db::ItemRow {
    crate::db::ItemRow {
        object_id,
        item_id,
        count,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: slot as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    }
}

/// Fishing (G32) `canFish` gates: premium-only bait needs a premium account, and
/// a player standing in water can't fish.
#[test]
fn fishing_premium_and_underwater_gates() {
    use crate::data::fishing_data::{FishingBait, FishingCatch, FishingRod};
    use crate::data::item_data::kinds::WeaponType;
    use crate::data::zone_data::ZoneKind;
    use crate::model::components::space::Position;
    use crate::model::inventory::{Inventory, PaperdollSlot};

    const ROD: i32 = 45492;
    const BAIT: i32 = 47547;
    const FISH: i32 = 47550;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (ROD, "Fishing Rod", false),
            (BAIT, "Bait", true),
            (FISH, "Ugly Fish", true),
        ],
    );
    world
        .data
        .item_data
        .set_weapon_type_for_test(ROD, WeaponType::FishingRod);
    world
        .data
        .fishing_data
        .insert_rod_for_test(ROD, FishingRod::default());
    let bait = |premium: bool| FishingBait {
        min_player_level: 1,
        max_player_level: 100,
        chance: 40,
        time_min: 1000,
        time_max: 1000,
        wait_min: 1000,
        wait_max: 1000,
        premium_only: premium,
        catches: vec![FishingCatch {
            item_id: FISH,
            chance: 100,
            multiplier: 1,
        }],
    };

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 20;
    let inv = Inventory::from_rows(&[
        item_row(0x4700_0011, ROD, 1, PaperdollSlot::RHand),
        item_row(0x4700_0012, BAIT, 10, PaperdollSlot::LHand),
    ]);
    world.objects.add_components(&3001, inv);
    insert_zone(&mut world, ZoneKind::Fishing, 0, 160, 0, 1000);
    insert_zone(&mut world, ZoneKind::Water, 160, 1000, 0, 1000);

    // --- Gate 1: premium-only bait, no premium account → the cast never starts. ---
    world
        .data
        .fishing_data
        .insert_bait_for_test(BAIT, bait(true));
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001);
    advance_ticks(&mut world, 12);
    assert_eq!(item_count(&world, 3001, FISH), 0, "premium bait blocked");
    assert_eq!(item_count(&world, 3001, BAIT), 10, "no bait consumed");

    // --- Control: the same bait, non-premium, lands a fish. ---
    world
        .data
        .fishing_data
        .insert_bait_for_test(BAIT, bait(false));
    world.force_roll(0); // win
    world.force_roll(0); // catch
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001);
    advance_ticks(&mut world, 12);
    assert_eq!(
        item_count(&world, 3001, FISH),
        1,
        "non-premium bait catches"
    );
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001); // stop the auto-recast

    // --- Gate 2: standing in the water zone → can't fish. ---
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 500; // inside the water zone (160..1000)
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001);
    advance_ticks(&mut world, 12);
    assert_eq!(
        item_count(&world, 3001, FISH),
        1,
        "no fishing while in water"
    );
}

/// Fishing (G32) — fishing shots double the win chance. The *same* reel roll (41)
/// loses at the bare 40% chance but wins at the shot-doubled 80%.
#[test]
fn fishing_shots_double_the_win_chance() {
    use crate::data::fishing_data::{FishingBait, FishingCatch, FishingRod};
    use crate::data::item_data::kinds::{ActionType, ItemHandler, WeaponType};
    use crate::data::zone_data::ZoneKind;
    use crate::model::inventory::{Inventory, PaperdollSlot};

    const ROD: i32 = 45492;
    const BAIT: i32 = 47547;
    const FISH: i32 = 47550;
    const FISH_SHOT: i32 = 6535; // Corroded Fishing Shot

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[
            (ROD, "Fishing Rod", false),
            (BAIT, "Bait", true),
            (FISH, "Ugly Fish", true),
        ],
    );
    add_shot_item(
        &mut world,
        FISH_SHOT,
        "Corroded Fishing Shot",
        ItemHandler::FishShots,
        ActionType::Other,
    );
    world
        .data
        .item_data
        .set_weapon_type_for_test(ROD, WeaponType::FishingRod);
    world
        .data
        .fishing_data
        .insert_rod_for_test(ROD, FishingRod::default());
    world.data.fishing_data.insert_bait_for_test(
        BAIT,
        FishingBait {
            min_player_level: 1,
            max_player_level: 100,
            chance: 40,
            time_min: 1000,
            time_max: 1000,
            wait_min: 1000,
            wait_max: 1000,
            premium_only: false,
            catches: vec![FishingCatch {
                item_id: FISH,
                chance: 100,
                multiplier: 1,
            }],
        },
    );

    let _rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 20;
    let inv = Inventory::from_rows(&[
        item_row(0x4700_0021, ROD, 1, PaperdollSlot::RHand),
        item_row(0x4700_0022, BAIT, 5, PaperdollSlot::LHand),
    ]);
    world.objects.add_components(&3001, inv);
    inject(&mut world, 3001, 0x4700_0023, FISH_SHOT, 10);
    insert_zone(&mut world, ZoneKind::Fishing, 0, 160, 0, 1000);
    insert_zone(&mut world, ZoneKind::Water, 160, 1000, 0, 1000);

    // --- No shots: the 40% chance loses on a roll of 41. ---
    world.force_roll(41); // reel win roll: 41 > 40 → lose
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001);
    advance_ticks(&mut world, 12);
    assert_eq!(
        item_count(&world, 3001, FISH),
        0,
        "bare 40%: 41 > 40 → lose"
    );
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001); // stop the auto-recast

    // --- Fishing shots on: the chance doubles to 80%, so the same 41 wins. ---
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .auto_shots = vec![FISH_SHOT];
    world.force_roll(41); // reel win roll: 41 ≤ 80 → win
    world.force_roll(0); // catch-table roll
    crate::game_loop::activities::fishing::toggle_fishing(&mut world, 3001);
    advance_ticks(&mut world, 12);
    assert_eq!(
        item_count(&world, 3001, FISH),
        1,
        "shot-doubled 80%: 41 ≤ 80 → win"
    );
    assert!(
        item_count(&world, 3001, FISH_SHOT) < 10,
        "the fishing shot was spent"
    );
}

/// Fishing (G32) — entering a fishing zone (rod + bait ready) lights the client's
/// auto-fish button (`ExAutoFishAvailable` YES); leaving dims it (NO).
#[test]
fn fishing_zone_toggles_auto_fish_available() {
    use crate::data::fishing_data::{FishingBait, FishingCatch, FishingRod};
    use crate::data::item_data::kinds::WeaponType;
    use crate::data::zone_data::ZoneKind;
    use crate::model::components::space::{Position, ZoneFlags};
    use crate::model::inventory::{Inventory, PaperdollSlot};

    const ROD: i32 = 45492;
    const BAIT: i32 = 47547;

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(
        &mut world,
        &[(ROD, "Fishing Rod", false), (BAIT, "Bait", true)],
    );
    world
        .data
        .item_data
        .set_weapon_type_for_test(ROD, WeaponType::FishingRod);
    world
        .data
        .fishing_data
        .insert_rod_for_test(ROD, FishingRod::default());
    world.data.fishing_data.insert_bait_for_test(
        BAIT,
        FishingBait {
            min_player_level: 1,
            max_player_level: 100,
            chance: 40,
            time_min: 1000,
            time_max: 1000,
            wait_min: 1000,
            wait_max: 1000,
            premium_only: false,
            catches: vec![FishingCatch {
                item_id: 47550,
                chance: 100,
                multiplier: 1,
            }],
        },
    );

    let mut rx = ingame_player(&mut world, 1, 3001, 100, 200, 0);
    world
        .objects
        .get_component_mut::<Player>(&3001)
        .unwrap()
        .level = 20;
    let inv = Inventory::from_rows(&[
        item_row(0x4700_0031, ROD, 1, PaperdollSlot::RHand),
        item_row(0x4700_0032, BAIT, 5, PaperdollSlot::LHand),
    ]);
    world.objects.add_components(&3001, inv);
    world.objects.add_components(&3001, ZoneFlags::default());
    // The fishing zone covers x 400..600 — the player starts outside it (100,200).
    insert_zone(&mut world, ZoneKind::Fishing, 400, 600, 0, 1000);

    // Read the latest `ExAutoFishAvailable` flag from the outbound packets.
    let read_avail = |rx: &mut _| -> Option<bool> {
        drain(rx).iter().rev().find_map(|p| {
            (p.first() == Some(&server_packets::opcodes::EX)
                && i16::from_le_bytes(p[1..3].try_into().ok()?)
                    == server_packets::opcodes::EX_AUTO_FISH_AVAILABLE)
                .then(|| p[3] != 0)
        })
    };

    // Outside the zone: no availability packet (it was never lit).
    zones::revalidate_zone(&mut world, 3001, true);
    assert_eq!(read_avail(&mut rx), None, "no packet while outside");

    // Move into the fishing zone → YES.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 500;
    zones::revalidate_zone(&mut world, 3001, true);
    assert_eq!(
        read_avail(&mut rx),
        Some(true),
        "entering lights the button"
    );

    // Move back out → NO.
    world
        .objects
        .get_component_mut::<Position>(&3001)
        .unwrap()
        .x = 100;
    zones::revalidate_zone(&mut world, 3001, true);
    assert_eq!(read_avail(&mut rx), Some(false), "leaving dims it");
}
