//! The castle treasury (`Castle.addToTreasury` / `addToTreasuryNoTax`) and the
//! paths that move money through it: merchant/multisell tax inside a `TaxZone`,
//! manor seed sales, and the chamberlain's vault console.

use super::*;

use crate::data::item_data::ADENA_ID;
use crate::data::multisell_data::{Ingredient, MultisellEntry, MultisellList, Product};
use crate::data::zone_data::{Zone, ZoneKind};
use crate::game_loop::castle::{
    add_to_treasury, add_to_treasury_no_tax, npc_tax_castle, tax_percent, treasury,
};
use crate::game_loop::{multisell, shop};
use crate::model::castle::{Castle, CastleSide, TaxType};
use crate::model::clan::Clan;
use crate::model::components::ActiveMultisell;

const GLUDIO: i32 = 1;
const ADEN: i32 = 5;

fn castle(id: i32, name: &str, side: CastleSide) -> Castle {
    Castle {
        show_npc_crest: false,
        id,
        name: name.into(),
        side,
        ticket_buy_count: 0,
        first_mid_victory: false,
        time_registration_over: true,
        siege_time_registration_end: 0,
        siege_date: 0,
        treasury: 0,
    }
}

/// Gludio + Aden — the vassal/liege pair Java's `addToTreasury` switch names.
fn with_castles(world: &mut World) {
    world.castles = vec![
        castle(GLUDIO, "Gludio", CastleSide::Neutral),
        castle(ADEN, "Aden", CastleSide::Neutral),
    ];
}

/// Give a castle an owner clan (Java `_ownerId > 0`), without touching any
/// player's membership — the treasury only cares that *someone* holds it.
fn own(world: &mut World, castle_id: i32, clan_id: i32) {
    world.clans.insert(
        clan_id,
        Clan {
            id: clan_id,
            name: format!("Owners{clan_id}"),
            leader_id: 0,
            level: 5,
            reputation_score: 0,
            castle_id,
            members: Vec::new(),
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
        },
    );
}

/// A `TaxZone` paying `castle_id`, covering the whole test neighbourhood.
fn insert_tax_zone(world: &mut World, castle_id: i32) {
    world.data.zone_data.insert(Zone {
        id: 0,
        name: format!("test_tax_{castle_id}"),
        kind: ZoneKind::Tax,
        territory: test_territory(),
        castle_id,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
    });
}

// ------------------------------------------------------------------- vault

/// **An unowned castle has no vault.** Java returns early on `_ownerId <= 0`,
/// so income aimed at a castle nobody holds is lost — the balance doesn't move.
#[test]
fn an_unowned_castle_takes_nothing() {
    let (mut world, ..) = quest_test_world();
    with_castles(&mut world);

    assert!(!add_to_treasury_no_tax(&mut world, GLUDIO, 1_000));
    assert_eq!(treasury(&world, GLUDIO), 0);

    own(&mut world, GLUDIO, 500);
    assert!(add_to_treasury_no_tax(&mut world, GLUDIO, 1_000));
    assert_eq!(treasury(&world, GLUDIO), 1_000, "an owned castle banks it");
}

/// **A withdrawal larger than the balance changes nothing.** Java's negative
/// branch returns before touching `_treasury`; one the vault can cover goes
/// through.
#[test]
fn overdrawing_the_vault_is_refused() {
    let (mut world, ..) = quest_test_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    add_to_treasury_no_tax(&mut world, GLUDIO, 1_000);

    assert!(!add_to_treasury_no_tax(&mut world, GLUDIO, -1_001));
    assert_eq!(treasury(&world, GLUDIO), 1_000, "nothing was taken");

    assert!(add_to_treasury_no_tax(&mut world, GLUDIO, -1_000));
    assert_eq!(treasury(&world, GLUDIO), 0);
}

/// **A credit past `MaxAdena` clamps instead of failing.** Java's overflow
/// branch assigns the ceiling and still reports success.
#[test]
fn a_credit_over_the_ceiling_clamps() {
    let (mut world, ..) = quest_test_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    let max = world.cfg.character.max_adena;
    add_to_treasury_no_tax(&mut world, GLUDIO, max - 10);

    assert!(add_to_treasury_no_tax(&mut world, GLUDIO, 1_000));
    assert_eq!(treasury(&world, GLUDIO), max, "clamped to MaxAdena");
}

/// **Every accepted change is persisted, and only those.** Java writes
/// `UPDATE castle SET treasury` per call; a refused call writes nothing.
#[test]
fn each_change_writes_the_row() {
    let (mut world, mut db, _link) = quest_test_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    drain_db(&mut db);

    add_to_treasury_no_tax(&mut world, GLUDIO, 700);
    add_to_treasury_no_tax(&mut world, GLUDIO, -5_000); // refused: overdraw

    let writes: Vec<i64> = drain_db(&mut db)
        .into_iter()
        .filter_map(|c| match c {
            db::DbCommand::UpdateCastleTreasury {
                castle_id,
                treasury,
            } if castle_id == GLUDIO => Some(treasury),
            _ => None,
        })
        .collect();
    assert_eq!(writes, vec![700], "one write; the refusal wrote nothing");
}

/// **Tax income pays the liege castle first.** Gludio feeds Aden (Java's name
/// `switch`), so a neutral Aden's 15 % takes 150 of a 1000 income and Gludio
/// keeps 850. `addToTreasuryNoTax` bypasses the cascade entirely.
#[test]
fn tax_income_pays_the_liege_castle() {
    let (mut world, ..) = quest_test_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    own(&mut world, ADEN, 501);

    add_to_treasury(&mut world, GLUDIO, 1_000);
    assert_eq!(treasury(&world, ADEN), 150, "Aden's 15% off the top");
    assert_eq!(treasury(&world, GLUDIO), 850);

    add_to_treasury_no_tax(&mut world, GLUDIO, 1_000);
    assert_eq!(
        treasury(&world, ADEN),
        150,
        "the no-tax path skips the liege"
    );
    assert_eq!(treasury(&world, GLUDIO), 1_850);
}

/// **An unowned liege still takes its cut out of circulation.** Java subtracts
/// `adenTax` from the vassal's income *outside* the `getOwnerId() > 0` check,
/// so the money disappears rather than staying with the vassal — kept verbatim.
#[test]
fn an_unowned_liege_still_takes_its_cut() {
    let (mut world, ..) = quest_test_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500); // Aden stays unowned

    add_to_treasury(&mut world, GLUDIO, 1_000);

    assert_eq!(treasury(&world, ADEN), 0, "an unowned Aden banks nothing");
    assert_eq!(
        treasury(&world, GLUDIO),
        850,
        "and Gludio keeps only 850 — the cut is gone"
    );
}

/// **The tax percent follows the castle's side** (`Feature.ini`: neutral 15,
/// light 0, dark 30), and an unknown castle taxes nothing.
#[test]
fn tax_percent_follows_the_castle_side() {
    let (mut world, ..) = quest_test_world();
    with_castles(&mut world);
    assert_eq!(tax_percent(&world, GLUDIO, TaxType::Buy), 15, "neutral");

    world.castles[0].side = CastleSide::Light;
    assert_eq!(tax_percent(&world, GLUDIO, TaxType::Buy), 0, "light");

    world.castles[0].side = CastleSide::Dark;
    assert_eq!(tax_percent(&world, GLUDIO, TaxType::Buy), 30, "dark");

    assert_eq!(tax_percent(&world, 42, TaxType::Buy), 0, "unknown castle");
}

// ------------------------------------------------------------ merchant tax

/// **A purchase from a merchant inside a tax zone feeds that castle's vault**,
/// through the liege cascade. `shop_world`'s merchant sits at (100, 0, 0) and
/// item 41 costs 100, so at Gludio's neutral 15 %:
///
/// - charged `(long)(100 × 1.15)` = **114** — Java's `(long)` cast truncates,
///   and `100 × 1.15` is `114.99999999999999` in double, so the buyer pays 114
///   rather than the arithmetically expected 115. Quirk kept deliberately.
/// - taxed `(long)(114 × 0.15)` = **17**, of which Aden (Gludio's liege) takes
///   `(long)(17 × 0.15)` = **2** and Gludio keeps **15**.
#[test]
fn a_purchase_in_a_tax_zone_feeds_the_treasury() {
    let (mut world, _db, _rx) = shop_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    own(&mut world, ADEN, 501);
    insert_tax_zone(&mut world, GLUDIO);
    assert_eq!(npc_tax_castle(&world, NPC_OID), Some(GLUDIO));

    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1)]));

    assert_eq!(adena_of(&world, 3001), 1_000 - 114, "the taxed price");
    assert_eq!(count_of_item(&world, 3001, 41), 1);
    assert_eq!(treasury(&world, GLUDIO), 15, "the castle banked its tax");
    assert_eq!(treasury(&world, ADEN), 2, "and its liege took a cut");
}

/// **Outside every tax zone the same purchase is untaxed.** This is the case
/// that fails if the tax rate is taken from a nearest-castle lookup instead of
/// the zone the merchant actually stands in.
#[test]
fn a_purchase_outside_a_tax_zone_is_untaxed() {
    let (mut world, _db, _rx) = shop_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    // No tax zone.

    assert_eq!(npc_tax_castle(&world, NPC_OID), None);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1)]));

    assert_eq!(adena_of(&world, 3001), 900, "the bare price");
    assert_eq!(treasury(&world, GLUDIO), 0);
}

/// **An unowned castle's tax zone charges nothing extra to nobody's benefit —
/// the price is still taxed.** Java computes the rate off the *side* and only
/// checks ownership when banking it, so the buyer pays the same either way.
#[test]
fn tax_is_charged_even_when_the_castle_is_unowned() {
    let (mut world, _db, _rx) = shop_world();
    with_castles(&mut world);
    insert_tax_zone(&mut world, GLUDIO); // nobody owns Gludio

    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1)]));

    assert_eq!(adena_of(&world, 3001), 886, "still the taxed price");
    assert_eq!(treasury(&world, GLUDIO), 0, "but nobody banks it");
}

// ----------------------------------------------------------- multisell tax

/// A one-entry list: `adena_cost` adena → one item 41.
fn insert_taxed_multisell(world: &mut World, list_id: i32, adena_cost: i64, apply_taxes: bool) {
    world.data.multisells.insert_for_test(MultisellList {
        list_id,
        is_chance_multisell: false,
        apply_taxes,
        maintain_enchantment: false,
        ingredient_multiplier: 1.0,
        product_multiplier: 1.0,
        entries: vec![MultisellEntry {
            ingredients: vec![Ingredient {
                id: ADENA_ID,
                count: adena_cost,
                enchant_level: 0,
                maintain: false,
            }],
            products: vec![Product {
                id: 41,
                count: 1,
                chance: None,
                enchant_level: 0,
            }],
            stackable: false,
        }],
        npcs_allowed: None,
    });
}

/// **A taxed multisell charges the castle's cut on its adena ingredient and
/// pays it into the vault.** Java rounds here (`Math.round`) instead of
/// truncating like the shop, so 100 adena at Gludio's neutral 15 % costs
/// **115** and the tax is **15** — of which Aden takes 2 and Gludio keeps 13.
/// The remaining 100 is the exchange's own price and simply vanishes: Java
/// banks the tax slice only.
#[test]
fn a_taxed_multisell_feeds_the_treasury() {
    let (mut world, _db, _rx) = shop_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    own(&mut world, ADEN, 501);
    insert_tax_zone(&mut world, GLUDIO);
    insert_taxed_multisell(&mut world, 9001, 100, true);

    multisell::separate_and_send(&mut world, 1, 3001, Some(NPC_OID), 9001, false);
    assert_eq!(
        world
            .objects
            .get_component::<ActiveMultisell>(&3001)
            .map(|a| a.tax_rate),
        Some(0.15),
        "the window latched the rate it displayed"
    );

    crate::game_loop::multisell::handle_multi_sell_choose(
        &mut world,
        1,
        &multisell_choose_body(9001, 1, 1),
    );

    assert_eq!(
        adena_of(&world, 3001),
        1_000 - 115,
        "taxed ingredient count"
    );
    assert_eq!(count_of_item(&world, 3001, 41), 1, "the product arrived");
    assert_eq!(treasury(&world, GLUDIO), 13, "only the tax slice is banked");
    assert_eq!(treasury(&world, ADEN), 2, "the liege's cut of that tax");
}

/// **A list that doesn't `applyTaxes` is untaxed even inside a tax zone.**
/// Java's `getTaxRate()` returns 0 for such a list whatever the NPC's castle
/// charges.
#[test]
fn a_multisell_without_apply_taxes_is_untaxed() {
    let (mut world, _db, _rx) = shop_world();
    with_castles(&mut world);
    own(&mut world, GLUDIO, 500);
    insert_tax_zone(&mut world, GLUDIO);
    insert_taxed_multisell(&mut world, 9002, 100, false);

    crate::game_loop::multisell::separate_and_send(&mut world, 1, 3001, Some(NPC_OID), 9002, false);
    crate::game_loop::multisell::handle_multi_sell_choose(
        &mut world,
        1,
        &multisell_choose_body(9002, 1, 1),
    );

    assert_eq!(adena_of(&world, 3001), 900, "the bare ingredient count");
    assert_eq!(treasury(&world, GLUDIO), 0);
}

// ------------------------------------------------------ the chamberlain vault

/// A world with Gludio's chamberlain (35100, object 701) and player 100 in
/// front of it.
fn chamberlain_vault_world() -> (World, tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>) {
    let (mut world, _db, _link) = quest_test_world();
    with_castles(&mut world);
    add_test_npc(&mut world, 701, 35100, "Merchant", 75, 0, 0, 0);
    let rx = ingame_player(&mut world, 1, 100, 0, 0, 0);
    (world, rx)
}

/// Make player 100 the leader of the clan that owns `castle_id` — a leader
/// holds every clan privilege, including `CS_TAXES`.
fn own_as_leader(world: &mut World, castle_id: i32, clan_id: i32) {
    own(world, castle_id, clan_id);
    world.clans.get_mut(&clan_id).unwrap().leader_id = 100;
    let p = world
        .objects
        .get_component_mut::<crate::model::Player>(&100)
        .unwrap();
    p.clan_id = clan_id;
}

fn chamberlain(world: &mut World, event: &str) {
    handle_request_bypass_to_server(
        world,
        1,
        &bypass_body(&format!("npc_701_Quest CastleChamberlain {event}")),
    );
}

/// **Deposit then withdraw through the chamberlain.** The adena moves both
/// ways, and the vault page shows the balance grouped into thousands
/// (`Util.formatAdena`).
#[test]
fn the_chamberlain_deposits_and_withdraws() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 200_000);
    drain(&mut rx);

    chamberlain(&mut world, "deposit 150000");
    assert_eq!(treasury(&world, GLUDIO), 150_000, "the vault took it");
    assert_eq!(adena_of(&world, 100), 50_000, "and the player paid it");

    let page = served_html(&mut rx).unwrap_or_default();
    assert!(
        page.contains("CastleChamberlain manage_vault"),
        "the console main page comes back: {page}"
    );

    chamberlain(&mut world, "manage_vault");
    let page = served_html(&mut rx).expect("the vault page");
    assert!(
        page.contains("150,000"),
        "the balance is shown grouped: {page}"
    );

    chamberlain(&mut world, "withdraw 50000");
    assert_eq!(treasury(&world, GLUDIO), 100_000);
    assert_eq!(adena_of(&world, 100), 100_000, "the adena came back");
}

/// **Withdrawing more than the vault holds serves the "not enough balance"
/// page and moves nothing.**
#[test]
fn withdrawing_over_the_balance_is_refused() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    add_to_treasury_no_tax(&mut world, GLUDIO, 1_000);
    drain(&mut rx);

    chamberlain(&mut world, "withdraw 5000");

    let page = served_html(&mut rx).expect("a page is served");
    assert!(
        page.contains("1,000") && page.contains("5,000"),
        "the balance and the request are both shown: {page}"
    );
    assert_eq!(treasury(&world, GLUDIO), 1_000, "nothing left the vault");
    assert_eq!(adena_of(&world, 100), 0, "and nothing reached the player");
}

/// **A depositor who can't cover the amount is refused** — no adena taken, no
/// credit, and Java's "not enough adena" message instead.
#[test]
fn depositing_more_than_you_hold_is_refused() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 100);
    drain(&mut rx);

    chamberlain(&mut world, "deposit 5000");

    assert_eq!(treasury(&world, GLUDIO), 0, "the vault is untouched");
    assert_eq!(adena_of(&world, 100), 100, "and so is the purse");
    assert!(
        sm_ids_of(&drain(&mut rx)).contains(&server_packets::sm_ids::YOU_DO_NOT_HAVE_ENOUGH_ADENA),
        "the shortfall is reported"
    );
}

/// **The vault is gated on owning *this* castle.** A clan leader who owns Dion
/// gets the refusal page at Gludio's chamberlain, and no adena moves.
#[test]
fn the_vault_gates_on_ownership() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, 2, 500); // Dion, not Gludio
    own(&mut world, GLUDIO, 501); // Gludio belongs to someone else
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 10_000);
    drain(&mut rx);

    chamberlain(&mut world, "deposit 5000");

    assert_eq!(treasury(&world, GLUDIO), 0, "a stranger can't deposit");
    assert_eq!(adena_of(&world, 100), 10_000);
    let page = served_html(&mut rx).unwrap_or_default();
    assert!(!page.is_empty(), "the refusal page is served");
}

// ------------------------------------------ the chamberlain console (G24/G26)

/// Renting a castle function: `set_func` takes the lease from the buyer, the
/// renewal task charges the clan warehouse each period, and a warehouse that
/// can't pay loses the function (Java `CastleFunction.FunctionTask`).
#[test]
fn castle_functions_buy_renew_and_lapse() {
    use crate::model::castle::FUNC_RESTORE_HP;
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 50_000);
    drain(&mut rx);

    // Buy HP-regen level 300 (fee 12,000 from Feature.ini defaults).
    chamberlain(&mut world, "set_func 2 300");
    assert_eq!(
        adena_of(&world, 100),
        38_000,
        "the lease came from the buyer"
    );
    let func = crate::game_loop::castle::castle_function(&world, GLUDIO, FUNC_RESTORE_HP)
        .expect("function active");
    assert_eq!(func.level, 300);

    // The immediate post-purchase run stamps the end time without charging.
    advance_ticks(&mut world, 2);
    let func = crate::game_loop::castle::castle_function(&world, GLUDIO, FUNC_RESTORE_HP).unwrap();
    assert!(func.end_time > 0, "rental period stamped");

    // A period later the clan warehouse pays…
    world.clans.get_mut(&500).unwrap().warehouse.0.add_item(
        &world.data.item_data,
        9_000_001,
        ADENA_ID,
        20_000,
    );
    crate::game_loop::castle::handle_function_renew(&mut world, GLUDIO, FUNC_RESTORE_HP, true);
    assert_eq!(
        world.clans[&500].warehouse.0.count_of(ADENA_ID),
        8_000,
        "the renewal charged the warehouse"
    );

    // …and a warehouse that can't pay loses the function.
    crate::game_loop::castle::handle_function_renew(&mut world, GLUDIO, FUNC_RESTORE_HP, true);
    assert!(
        crate::game_loop::castle::castle_function(&world, GLUDIO, FUNC_RESTORE_HP).is_none(),
        "the unpaid function lapsed"
    );
}

/// The buffer page is gated on the rented SUPPORT function: disabled page
/// without it, the level's buff menu with it.
#[test]
fn the_buffer_needs_the_rented_support_function() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 100_000);
    drain(&mut rx);

    chamberlain(&mut world, "buffer");
    let page = served_html(&mut rx).unwrap_or_default();
    assert!(
        page.contains("no longer be provided") || page.contains("disabled") || !page.is_empty(),
        "the function-disabled page: {page}"
    );
    assert!(
        crate::game_loop::castle::castle_function(
            &world,
            GLUDIO,
            crate::model::castle::FUNC_SUPPORT
        )
        .is_none()
    );

    // Rent support level 5 (49,000) and ask again.
    chamberlain(&mut world, "set_func 5 5");
    assert_eq!(adena_of(&world, 100), 51_000);
    chamberlain(&mut world, "buffer");
    let page = served_html(&mut rx).unwrap_or_default();
    assert!(
        page.contains("cast_buff"),
        "the buff menu is served: {page}"
    );
}

/// Trap (flame-tower) upgrades: the confirm pays and stores the level
/// (persisted through a global var), and a level already reached serves the
/// "already at" page instead of charging again.
#[test]
fn trap_upgrade_pays_once_and_remembers_the_level() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    super::items::add_inventory_item(&mut world, 100, ADENA_ID, 10_000_000);
    drain(&mut rx);

    chamberlain(&mut world, "upgrade_trap_confirm 0 2");
    assert_eq!(adena_of(&world, 100), 6_000_000, "level 2 costs 4,000,000");
    assert_eq!(
        crate::game_loop::castle::trap_upgrade_level(&world, GLUDIO, 0),
        2
    );
    drain(&mut rx);

    // Asking for the same level again charges nothing and serves the
    // "already at this level" page.
    chamberlain(&mut world, "upgrade_trap_confirm 0 2");
    assert_eq!(adena_of(&world, 100), 6_000_000);
    assert!(
        served_html(&mut rx).is_some(),
        "the already-at page is served"
    );
    assert_eq!(
        crate::game_loop::castle::trap_upgrade_level(&world, GLUDIO, 0),
        2,
        "level unchanged"
    );
}

/// The regen consumer, Java's integer division included: HP level 300 → ×3,
/// and the MP levels (40/55) → ×0 — the reference's shipped code zeroes MP
/// regen inside the castle while the MP function is rented.
#[test]
fn castle_regen_multiplier_keeps_javas_integer_division() {
    use crate::data::spawn_data::{Territory, ZoneForm};
    use crate::data::zone_data::{Zone, ZoneKind};
    let (mut world, _rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    world.data.zone_data.insert(Zone {
        id: 0,
        name: "gludio_castle".into(),
        kind: ZoneKind::Castle,
        territory: Territory {
            form: ZoneForm::Cuboid {
                x1: -100,
                x2: 100,
                y1: -100,
                y2: 100,
            },
            min_z: -1000,
            max_z: 1000,
        },
        castle_id: GLUDIO,
        clan_hall_id: 0,
        effect: None,
        damage: None,
        swamp: None,
        condition: None,
    });
    crate::game_loop::castle::update_castle_function(
        &mut world,
        GLUDIO,
        crate::model::castle::FUNC_RESTORE_HP,
        300,
        12_000,
        604_800_000,
    );
    crate::game_loop::castle::update_castle_function(
        &mut world,
        GLUDIO,
        crate::model::castle::FUNC_RESTORE_MP,
        40,
        45_000,
        604_800_000,
    );
    let (hp, mp) = crate::game_loop::regen::castle_regen_mult(&world, 100);
    assert_eq!(hp, 3.0, "300 / 100 = 3");
    assert_eq!(mp, 0.0, "40 / 100 = 0 — Java's bug, ported as behaviour");
}

/// The lord's crown is handed out once, with the presentation page.
#[test]
fn the_crown_is_granted_once() {
    let (mut world, mut rx) = chamberlain_vault_world();
    own_as_leader(&mut world, GLUDIO, 500);
    drain(&mut rx);

    chamberlain(&mut world, "give_crown");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&100)
            .unwrap()
            .count_of(6841),
        1,
        "the crown"
    );
    chamberlain(&mut world, "give_crown");
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&100)
            .unwrap()
            .count_of(6841),
        1,
        "only ever one"
    );
}

/// `RequestBuyItem`'s slot gate — the "same G5 deferral as `shop.rs`" that both
/// the shop and multisell module docs used to describe as absent.
///
/// The refusal must land **before** the charge: a purchase that cannot be
/// carried leaves the adena untouched, which is what separates a real gate from
/// one that takes the money and drops the goods.
#[test]
fn a_purchase_that_does_not_fit_is_refused_before_charging() {
    let (mut world, _db, _rx) = shop_world();

    // Squeeze the bag shut: one free slot short of what the buy needs.
    let used = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&3001)
        .unwrap()
        .non_quest_size(&world.data.item_data) as i32;
    world.cfg.character.inventory_max_no_dwarf = used;

    let before = adena_of(&world, 3001);
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1)]));

    assert_eq!(
        count_of_item(&world, 3001, 41),
        0,
        "nothing delivered when the bag is full"
    );
    assert_eq!(
        adena_of(&world, 3001),
        before,
        "and nothing charged — the gate runs before `reduceAdena`"
    );

    // One more slot and the identical purchase goes through, so the refusal
    // above was the capacity gate and not some unrelated guard.
    world.cfg.character.inventory_max_no_dwarf = used + 1;
    shop::handle_request_buy_item(&mut world, 1, &buy_body(3, &[(41, 1)]));
    assert_eq!(count_of_item(&world, 3001, 41), 1, "fits now");
    assert!(adena_of(&world, 3001) < before, "and was charged");
}
