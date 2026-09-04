//! The collar: summoning a pet from it, using it as an item, refusing a
//! second pet, the pet-window orders, and when the collar may be sold.

use super::*;

/// The owner is sent `PetInfo` (0xB2) when the servitor appears — without it
/// nothing renders client-side.
#[test]
fn the_owner_is_sent_pet_info() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let _ = drain(&mut rx);

    summon_servitor(&mut world, OWNER, PANTHER, 283, 1200, 0, 0).unwrap();

    let opcodes: Vec<u8> = drain(&mut rx)
        .iter()
        .filter_map(|p| p.first().copied())
        .collect();
    assert!(
        opcodes.contains(&server_packets::opcodes::PET_INFO),
        "PetInfo sent, got {opcodes:?}"
    );
}

// ---------------------------------------------------------------------------
// Real dist data
// ---------------------------------------------------------------------------

/// The collar summons its pet, bound to that **collar's object id** — the
/// identity two collars of the same kind are distinguished by.
#[test]
fn a_collar_summons_its_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);

    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    let link = world.objects.get_component::<PetOf>(&pet).unwrap();
    assert_eq!(
        link.collar_object_id, collar,
        "bound to this collar, not the item type"
    );
    assert_eq!(link.fed, 248, "starts on a full food bar from PetData");
    assert_eq!(pet_of(&world, OWNER), Some(pet));
}

/// A pet reuses the servitor owner-link, so it inherits follow for free.
#[test]
fn a_pet_follows_like_a_servitor() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).unwrap();

    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following
    );
    world
        .objects
        .get_component_mut::<Position>(&OWNER)
        .unwrap()
        .x = 900;
    servitor_follow_tick(&mut world, pet);
    assert!(world.objects.get_component::<Movement>(&pet).is_some());
}

/// The collar is **taken**, not copied — Java's `removeScript`. A second
/// summon with nothing parked must not produce a second pet.
#[test]
fn the_parked_collar_is_consumed_by_the_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    assert!(
        world
            .objects
            .get_component::<Player>(&OWNER)
            .unwrap()
            .pending_pet_collar
            .is_none(),
        "the holder was taken"
    );
    assert_eq!(
        summon_pet(&mut world, OWNER),
        None,
        "nothing parked, nothing summoned"
    );
}

/// Reaching the effect without going through the item handler summons nothing
/// — Java logs a warning and bails.
#[test]
fn summoning_without_a_parked_collar_does_nothing() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    give_collar(&mut world);
    assert_eq!(summon_pet(&mut world, OWNER), None);
}

/// "You already have a pet." — a second collar does not stack.
#[test]
fn a_second_pet_is_refused() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let first = summon_pet(&mut world, OWNER).unwrap();

    park_collar(&mut world, collar);
    assert_eq!(summon_pet(&mut world, OWNER), None, "refused");
    assert_eq!(
        pet_of(&world, OWNER),
        Some(first),
        "the first one is untouched"
    );
}

/// A collar the owner no longer holds cannot summon — Java re-checks the
/// inventory, which is what stops a traded/dropped collar working.
#[test]
fn a_collar_not_in_the_inventory_cannot_summon() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    world
        .objects
        .get_component_mut::<Inventory>(&OWNER)
        .unwrap()
        .remove_item(WOLF_COLLAR, 1);

    assert_eq!(summon_pet(&mut world, OWNER), None, "no collar, no pet");
}

/// A pet's `PetInfo` declares `summonType` **1**, where a servitor's is 2 —
/// that byte is how the client decides to offer the pet inventory and food bar.
#[test]
fn a_pet_declares_the_pet_summon_type() {
    let (mut world, _db, _l) = servitor_world();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let _ = drain(&mut rx);
    summon_pet(&mut world, OWNER).unwrap();

    let pkt = drain(&mut rx)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::PET_INFO))
        .expect("PetInfo sent");
    assert_eq!(pkt[1], 1, "summonType 1 = pet");

    // And the servitor path still says 2.
    let mut rx2 = ingame_caster(&mut world, 3, OWNER + 2, 0, 0);
    let _ = drain(&mut rx2);
    summon_servitor(&mut world, OWNER + 2, PANTHER, 283, 0, 0, 0).unwrap();
    let s_pkt = drain(&mut rx2)
        .into_iter()
        .find(|p| p.first() == Some(&server_packets::opcodes::PET_INFO))
        .expect("PetInfo sent");
    assert_eq!(s_pkt[1], 2, "summonType 2 = servitor");
}

// ---------------------------------------------------------------------------
// Pet persistence (slice 7)
// ---------------------------------------------------------------------------

/// The collar can't be stored inside the pet it summons — it would become
/// unreachable the moment the pet is unsummoned.
#[test]
fn the_collar_cannot_be_given_to_its_own_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    register_food(&mut world, 100);
    park_collar(&mut world, collar);
    let _ = summon_pet(&mut world, OWNER).unwrap();

    let mut body = Vec::new();
    body.extend_from_slice(&collar.to_le_bytes());
    body.extend_from_slice(&1i64.to_le_bytes());
    crate::game_loop::servitor::handle_give_item_to_pet(&mut world, CID, &body);

    assert_eq!(
        world
            .objects
            .get_component::<PetInventory>(&OWNER)
            .unwrap()
            .0
            .count_of(WOLF_COLLAR),
        0
    );
}

/// A pet is reported with the pet discriminator, not the servitor one — the
/// client uses it to decide what the party window row looks like.
#[test]
fn a_pet_reports_the_pet_summon_type_in_the_party_window() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).unwrap();

    let view = crate::game_loop::party::member_view(&world, OWNER).unwrap();
    assert_eq!(view.summons.len(), 1);
    assert_eq!(view.summons[0].summon_type, 1, "1 = pet");
}

/// **A pet is not a servitor.** Java's handler returns `getAnyServitor()`,
/// which is null for a pet-only owner — so "Servitor Heal" does nothing for
/// someone with a Wolf. It reads like a bug and is thematically right: this is
/// the Summoner's kit. Pinned so a later "fix" has to be deliberate.
#[test]
fn a_pet_is_not_a_summon_target() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let pet_oid = summoned_pet(&mut world);

    assert!(pet_of(&world, OWNER).is_some(), "the pet is out");
    assert!(
        servitor_of(&world, OWNER).is_none(),
        "but it is not what a SUMMON-target skill resolves to (pet {pet_oid})"
    );
}

/// The exchange counter: a ticket becomes a collar, and without the ticket
/// nothing is handed out.
#[test]
fn a_pet_ticket_exchanges_for_a_collar() {
    let (mut world, ..) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    world.data.item_data = dist::items_owned();
    world.id_pool = 0x4500_0000..0x4500_0100;

    // No ticket → nothing.
    evolve::handle_exchange(&mut world, CID, OWNER, 0, "exchange 1");
    let count_of = |w: &World, id: i32| {
        w.objects
            .get_component::<Inventory>(&OWNER)
            .unwrap()
            .count_of(id)
    };
    assert_eq!(count_of(&world, 6650), 0);

    // Kookaburra ticket 7585 → collar 6650.
    inventory::add_inventory_item(&mut world, OWNER, 7585, 1).unwrap();
    evolve::handle_exchange(&mut world, CID, OWNER, 0, "exchange 1");
    assert_eq!(count_of(&world, 7585), 0, "the ticket is taken");
    assert_eq!(count_of(&world, 6650), 1, "the collar is given");
}

/// The sell tab hides the collar of a pet that is currently out — Java's
/// `(pet == null) || (item.getObjectId() != pet.getControlObjectId())`.
///
/// Keyed on the **object** id, so a second collar of the same kind sitting in
/// the bag stays sellable. That distinction is the whole point of the guard and
/// is what an item-id comparison would get wrong, so both are asserted.
#[test]
fn the_summoned_pets_collar_is_not_offered_for_sale() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    // `give_collar` registers the *pet* and NPC templates but not the collar's
    // own `ItemTemplate`, without which the sell filter drops it for want of a
    // template and the test below passes vacuously.
    let mut tmpl = crate::data::item_data::template::ItemTemplate::default();
    tmpl.item_id = WOLF_COLLAR;
    tmpl.name = "Wolf Collar".into();
    tmpl.is_sellable = true;
    tmpl.price = 100;
    world.data.item_data.insert_for_test(tmpl);

    let collar = give_collar(&mut world);
    // A *second* collar of the same kind. `give_collar` hard-codes one object
    // id, so the spare is added by hand — the point of this test is that the
    // guard compares object ids, which needs two of them.
    let spare = collar + 1;
    {
        let World { data, objects, .. } = &mut world;
        objects
            .get_component_mut::<Inventory>(&OWNER)
            .unwrap()
            .add_item(&data.item_data, spare, WOLF_COLLAR, 1);
    }
    assert_ne!(collar, spare, "two distinct collar instances");
    park_collar(&mut world, collar);
    summon_pet(&mut world, OWNER).expect("summoned");

    // Build the **real** `ExBuySellList` and count its sell entries. An earlier
    // draft re-implemented the filter inline here, which meant deleting the
    // production filter changed nothing — the test only proved it agreed with
    // itself. Sabotage caught it.
    let sell_entry_count = |world: &World, active: Option<i32>| -> i16 {
        let inv = world.objects.get_component::<Inventory>(&OWNER).unwrap();
        let pkt =
            crate::network::trade::ex_buy_sell_list_sell(inv, &[], &world.data, false, active);
        // u8 opcode + i16 ex-opcode + i32 type + i32 slots, then the i16 count.
        i16::from_le_bytes([pkt[11], pkt[12]])
    };

    let active = crate::game_loop::servitor::active_pet_collar(&world, OWNER);
    assert_eq!(active, Some(collar), "the summoned pet's own collar");

    // Baseline: unguarded, both collars are offered. Without it the assertion
    // below would also pass on a list empty for an unrelated reason — and it
    // nearly was: `give_collar` never registers the collar's own `ItemTemplate`,
    // so before one was added above, *neither* collar appeared at all.
    let unguarded = sell_entry_count(&world, None);
    let guarded = sell_entry_count(&world, active);
    assert!(
        unguarded >= 2,
        "baseline: both collars are offered when nothing is excluded (got {unguarded})"
    );
    assert_eq!(
        guarded,
        unguarded - 1,
        "exactly one entry — the summoned pet's collar — is withheld, so a \
         spare of the same item id stays sellable"
    );
}

/// Unsummoning releases the collar: it is sellable again. Pinned because the
/// guard reads through the live `SummonRef` link, so a despawn path that forgot
/// to clear it would silently keep the item locked forever.
#[test]
fn unsummoning_releases_the_collar_for_sale() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    assert_eq!(
        crate::game_loop::servitor::active_pet_collar(&world, OWNER),
        Some(collar)
    );

    // Takes the owner, not the summon — one path retires either kind.
    let _ = pet;
    unsummon_servitor(&mut world, OWNER);

    assert_eq!(
        crate::game_loop::servitor::active_pet_collar(&world, OWNER),
        None,
        "no pet out — nothing is locked"
    );
}

/// The pet window's orders reach the **pet**, not the servitor slot — the two
/// live in different halves of `SummonRef` and the old dispatcher could reach
/// neither.
#[test]
fn the_pet_window_orders_reach_the_pet() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");
    add_test_npc(&mut world, FOE, PANTHER, "Monster", 20, 200, 0, 0);
    world.objects.add_components(&OWNER, TargetRef(Some(FOE)));

    handle_pet_action(&mut world, CID, OWNER, "PetAttack", 0);
    assert!(
        hate_for(&world, pet, FOE) > 0.0,
        "PetAttack takes the target"
    );
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following,
        "and stops trailing"
    );

    handle_pet_action(&mut world, CID, OWNER, "PetStop", 0);
    assert_eq!(hate_for(&world, pet, FOE), 0.0, "PetStop drops the target");
    assert!(
        world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following,
        "and resumes following"
    );

    handle_pet_action(&mut world, CID, OWNER, "PetHold", 0);
    assert!(
        !world
            .objects
            .get_component::<ServitorOf>(&pet)
            .unwrap()
            .following,
        "PetHold toggles the follow off"
    );
}

/// `UnsummonPet` refuses mid-fight, and on the way out stores the pet — the
/// session's fed/exp deltas ride out on the row, not with the entity.
#[test]
fn unsummoning_a_pet_refuses_in_combat_and_otherwise_stores_it() {
    let (mut world, _db, _l) = servitor_world();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    park_collar(&mut world, collar);
    let pet = summon_pet(&mut world, OWNER).expect("summoned");

    // In combat stance → refused, pet still out.
    world
        .objects
        .get_component_mut::<model::components::combat::AttackState>(&pet)
        .unwrap()
        .stance_until_tick = world.tick + 100;
    handle_pet_action(&mut world, CID, OWNER, "UnsummonPet", 0);
    assert!(pet_of(&world, OWNER).is_some(), "refused mid-fight");

    // Out of combat → gone, and the fed value reached the owner's saved row.
    world
        .objects
        .get_component_mut::<model::components::combat::AttackState>(&pet)
        .unwrap()
        .stance_until_tick = 0;
    // Above the wolf's 55 % hunger limit — below it the *hungry* refusal fires
    // first, which is a different branch and a different message.
    world.objects.get_component_mut::<PetOf>(&pet).unwrap().fed = 200;
    handle_pet_action(&mut world, CID, OWNER, "UnsummonPet", 0);

    assert!(pet_of(&world, OWNER).is_none(), "the pet is put away");
    assert_eq!(
        world
            .objects
            .get_component::<PlayerPets>(&OWNER)
            .and_then(|p| p.0.get(&collar))
            .map(|r| r.fed),
        Some(200),
        "and its state was stored on the way out"
    );
}

// ---------------------------------------------------------------------------
// The collar as an *item* (`handlers/itemhandlers/SummonItems`)
// ---------------------------------------------------------------------------

/// **The pet system's way in.** Every pet collar on this dist declares
/// `handler="SummonItems"` — the Wolf Collar included — and that name used to
/// fall through `ItemHandler`'s match to `None`, so using a collar consumed the
/// click and did nothing. Summoning worked in every test because the tests set
/// `pending_pet_collar` by hand; no client could reach it.
///
/// This drives the real path: a real collar item, the real `UseItem` dispatch,
/// the real `Summon Pet` (2046) skill off the item's own `<skills>`.
#[test]
fn using_a_collar_item_summons_the_pet() {
    let (mut world, _db, _l) = servitor_world();
    world.data.item_data = dist::items_owned();
    world.data.skill_data = dist::skills_owned();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);

    assert!(pet_of(&world, OWNER).is_none(), "no pet before the click");
    items::use_equipable_item(&mut world, CID, OWNER, collar);
    // Summon Pet (2046) has `hitTime 5000`, so the effect lands 50 ticks out.
    advance_ticks(&mut world, 60);

    assert!(
        pet_of(&world, OWNER).is_some(),
        "using the collar brings the wolf out"
    );
}

/// `SummonItems`' own guards, which `ItemSkillsTemplate` has no reason to
/// carry: one summon at a time.
#[test]
fn a_second_collar_click_is_refused_while_a_pet_is_out() {
    let (mut world, _db, _l) = servitor_world();
    world.data.item_data = dist::items_owned();
    world.data.skill_data = dist::skills_owned();
    let mut rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    items::use_equipable_item(&mut world, CID, OWNER, collar);
    advance_ticks(&mut world, 60);
    let first = pet_of(&world, OWNER).expect("summoned");
    drain(&mut rx);

    items::use_equipable_item(&mut world, CID, OWNER, collar);
    advance_ticks(&mut world, 60);

    assert_eq!(
        pet_of(&world, OWNER),
        Some(first),
        "the same pet is still out, not a second one"
    );
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SYSTEM_MESSAGE),
        "and the owner is told they already have one"
    );
}

/// A seated player cannot summon (`YOU_CANNOT_USE_ACTIONS_AND_SKILLS_WHILE_THE_CHARACTER_IS_SITTING`).
#[test]
fn a_seated_player_cannot_use_a_collar() {
    let (mut world, _db, _l) = servitor_world();
    world.data.item_data = dist::items_owned();
    world.data.skill_data = dist::skills_owned();
    let _rx = ingame_caster(&mut world, CID, OWNER, 0, 0);
    let collar = give_collar(&mut world);
    world
        .objects
        .get_component_mut::<Player>(&OWNER)
        .unwrap()
        .sitting = true;

    items::use_equipable_item(&mut world, CID, OWNER, collar);
    advance_ticks(&mut world, 60);

    assert!(pet_of(&world, OWNER).is_none(), "refused while seated");
}
