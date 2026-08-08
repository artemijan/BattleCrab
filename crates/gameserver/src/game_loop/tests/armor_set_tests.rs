//! `ArmorSetData` — the set bonuses a player gets for wearing matching armor.
//!
//! Driven against the **real dist data** rather than fixtures, because the
//! whole point of the port is the 37 sets a player can actually assemble here,
//! and a synthetic set would not prove the parser reads them.
//!
//! Set 1 (Mithril) is the workhorse: `minimumPieces=3`, required
//! Helmet 47 (head) / Mithril Breastplate 58 (chest) / Mithril Gaiters 59
//! (legs), optional shield Hoplon 628, and four skills that between them
//! exercise every gate — two plain (`3006`, `3502`), one `optional="true"`
//! (`3543`), and one `minimumEnchant="6"` (`3612`).

use super::*;

use crate::model::components::{BaseStats, SkillBook};
use crate::model::inventory::Inventory;

const PLAYER: i32 = 8001;
const CID: u32 = 1;

// Set 1 — Mithril.
const HELMET: i32 = 47;
const BREASTPLATE: i32 = 58;
const GAITERS: i32 = 59;
const HOPLON: i32 = 628;
const SKILL_PLAIN_A: i32 = 3006;
const SKILL_PLAIN_B: i32 = 3502;
const SKILL_OPTIONAL: i32 = 3543;
const SKILL_PLUS_SIX: i32 = 3612;

/// Object ids for the worn pieces; arbitrary but stable per item.
fn oid_of(item_id: i32) -> i32 {
    9000 + item_id
}

fn armor_set_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game/");
    world.data.item_data = crate::data::ItemData::load_from(root);
    world.data.armor_sets = crate::data::armor_set_data::ArmorSetData::load_from(root);
    world.data.skill_data = crate::data::SkillData::load_from(root);
    assert!(
        !world.data.armor_sets.is_empty(),
        "the dist armor sets loaded"
    );
    (world, db, l)
}

/// Put `item_id` in the bag and equip it through the real item path.
fn equip(
    world: &mut World,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    item_id: i32,
) {
    let oid = oid_of(item_id);
    {
        let World { objects, data, .. } = world;
        let inv = objects.get_component_mut::<Inventory>(&PLAYER).unwrap();
        inv.add_item(&data.item_data, oid, item_id, 1);
    }
    drain(rx);
    items::handle_use_item(world, CID, &use_item_body(oid));
}

/// Unequip a worn piece through the real item path (`UseItem` toggles).
fn unequip(
    world: &mut World,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    item_id: i32,
) {
    drain(rx);
    items::handle_use_item(world, CID, &use_item_body(oid_of(item_id)));
}

fn has_skill(world: &World, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<SkillBook>(&PLAYER)
        .is_some_and(|b| b.0.contains_key(&skill_id))
}

/// Set the enchant level of an already-owned instance.
fn set_enchant(world: &mut World, item_id: i32, level: i32) {
    let inv = world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .unwrap();
    assert!(
        inv.set_item_enchant_level(item_id, level),
        "item {item_id} is owned"
    );
}

/// **The headline.** A set's passives arrive only once `minimumPieces` are
/// worn, and leave the moment the set breaks.
#[test]
fn set_passives_arrive_on_the_last_piece_and_leave_with_the_first() {
    let (mut world, _db, _l) = armor_set_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    equip(&mut world, &mut rx, HELMET);
    equip(&mut world, &mut rx, BREASTPLATE);
    assert!(
        !has_skill(&world, SKILL_PLAIN_A),
        "two of three pieces is not a set"
    );

    equip(&mut world, &mut rx, GAITERS);
    assert!(
        has_skill(&world, SKILL_PLAIN_A) && has_skill(&world, SKILL_PLAIN_B),
        "the third piece completes the set and grants both plain skills"
    );

    unequip(&mut world, &mut rx, GAITERS);
    assert!(
        !has_skill(&world, SKILL_PLAIN_A) && !has_skill(&world, SKILL_PLAIN_B),
        "breaking the set takes the passives back"
    );
}

/// `optional="true"` — the skill needs one of the set's `<optionalItems>` (the
/// shield) on top of the required pieces. Getting the two `<item>` lists
/// confused would either grant this with no shield or count the shield toward
/// `minimumPieces`.
#[test]
fn the_optional_shield_gates_its_own_skill() {
    let (mut world, _db, _l) = armor_set_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    for id in [HELMET, BREASTPLATE, GAITERS] {
        equip(&mut world, &mut rx, id);
    }
    assert!(
        has_skill(&world, SKILL_PLAIN_A),
        "the set itself is complete"
    );
    assert!(
        !has_skill(&world, SKILL_OPTIONAL),
        "no shield, no optional skill"
    );

    equip(&mut world, &mut rx, HOPLON);
    assert!(
        has_skill(&world, SKILL_OPTIONAL),
        "the shield unlocks the optional skill"
    );

    unequip(&mut world, &mut rx, HOPLON);
    assert!(
        !has_skill(&world, SKILL_OPTIONAL),
        "and takes it away again"
    );
    assert!(
        has_skill(&world, SKILL_PLAIN_A),
        "while the plain skills, which never needed it, stay"
    );
}

/// **The gap that started this.** `minimumEnchant` is a floor on the set's
/// *lowest* piece, not on any one of them — a +6/+6/+5 set does not qualify.
#[test]
fn the_plus_six_skill_needs_every_piece_at_six() {
    let (mut world, _db, _l) = armor_set_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    for id in [HELMET, BREASTPLATE, GAITERS] {
        equip(&mut world, &mut rx, id);
    }
    assert!(
        !has_skill(&world, SKILL_PLUS_SIX),
        "a +0 set grants only the unenchanted skills"
    );

    // Two at +6, one at +5: the floor is 5, so still no.
    for id in [HELMET, BREASTPLATE] {
        set_enchant(&mut world, id, 6);
    }
    set_enchant(&mut world, GAITERS, 5);
    // Re-run the listener the way an equip would.
    crate::game_loop::armor_sets::refresh_armor_sets(&mut world, PLAYER);
    assert!(
        !has_skill(&world, SKILL_PLUS_SIX),
        "the LOWEST piece decides — +6/+6/+5 is a +5 set"
    );

    set_enchant(&mut world, GAITERS, 6);
    crate::game_loop::armor_sets::refresh_armor_sets(&mut world, PLAYER);
    assert!(
        has_skill(&world, SKILL_PLUS_SIX),
        "every piece at +6 grants the set's enchant skill"
    );
}

/// Java `Inventory.getArmorMinEnchant` — the byte `UserInfo`/`CharInfo` carry,
/// and what draws the +6 glow. It is 0 while the set is incomplete even if the
/// worn pieces are enchanted, because `getLowestSetEnchant` bails on the piece
/// count first.
#[test]
fn the_armor_min_enchant_byte_follows_the_completed_set() {
    let (mut world, _db, _l) = armor_set_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    equip(&mut world, &mut rx, HELMET);
    equip(&mut world, &mut rx, BREASTPLATE);
    for id in [HELMET, BREASTPLATE] {
        set_enchant(&mut world, id, 6);
    }
    assert_eq!(
        crate::game_loop::armor_sets::max_set_enchant(&world, PLAYER),
        0,
        "an incomplete set reports 0 however enchanted its pieces are"
    );

    equip(&mut world, &mut rx, GAITERS);
    set_enchant(&mut world, GAITERS, 4);
    assert_eq!(
        crate::game_loop::armor_sets::max_set_enchant(&world, PLAYER),
        4,
        "a complete set reports its lowest piece"
    );
}

/// `<stats>` — flat base-stat bonuses, folded into `BaseStats` the way a henna
/// is. Set 13 is STR +4 / CON −1 over items 398 / 418 / 2431.
#[test]
fn a_complete_set_adds_its_flat_base_stats() {
    let (mut world, _db, _l) = armor_set_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    let set = world.data.armor_sets.get(13).expect("set 13 loaded");
    assert_eq!(set.stats.str_, 4.0, "set 13 is the STR +4 one");
    assert_eq!(set.stats.con, -1.0);
    let pieces = set.required_items.clone();
    assert_eq!(pieces.len(), 3);

    let before = *world
        .objects
        .get_component::<BaseStats>(&PLAYER)
        .expect("base stats");

    for id in &pieces[..2] {
        equip(&mut world, &mut rx, *id);
    }
    assert_eq!(
        world
            .objects
            .get_component::<BaseStats>(&PLAYER)
            .unwrap()
            .str_,
        before.str_,
        "an incomplete set contributes nothing"
    );

    equip(&mut world, &mut rx, pieces[2]);
    let after = *world.objects.get_component::<BaseStats>(&PLAYER).unwrap();
    assert_eq!(after.str_, before.str_ + 4, "STR +4 on completion");
    assert_eq!(after.con, before.con - 1, "and the CON −1 that pays for it");

    unequip(&mut world, &mut rx, pieces[2]);
    assert_eq!(
        *world.objects.get_component::<BaseStats>(&PLAYER).unwrap(),
        before,
        "breaking the set restores the base"
    );
}

/// Java grants set skills with `addSkill(skill, false)` — `store = false`, so
/// they never reach `character_skills`. Here the whole `SkillBook` is
/// persisted, so the filter is what keeps them out; without it a set bonus
/// would become permanent the first time the character was saved.
#[test]
fn set_skills_never_reach_the_persisted_skill_book() {
    let (mut world, _db, _l) = armor_set_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);

    for id in [HELMET, BREASTPLATE, GAITERS] {
        equip(&mut world, &mut rx, id);
    }
    assert!(
        has_skill(&world, SKILL_PLAIN_A),
        "the set is granting in-memory"
    );
    assert!(
        world.data.armor_sets.is_armor_set_skill(SKILL_PLAIN_A),
        "and the persistence filter recognises it"
    );

    let saved = crate::game_loop::net::build_save_data(&world, PLAYER).expect("save data");
    assert!(
        !saved.skills.iter().any(|&(id, _, _)| id == SKILL_PLAIN_A),
        "a set-granted skill is absent from what gets flushed"
    );
}

/// A character who logs out wearing a set must log back in still wearing its
/// bonus. This is the exact shape that regressed for augment options — worn
/// gear contributed nothing through a relog because only the equip listener
/// ever applied it — and set skills are more exposed, because they are
/// *stripped from the loaded rows* on the way in and exist only if something
/// re-derives them.
#[test]
fn a_worn_set_survives_a_relog() {
    use crate::character::ItemRow;
    use crate::model::inventory::PaperdollSlot;

    let (world, _db, _l) = armor_set_world();
    let mut chr = dummy_char(PLAYER, "Setter");
    let paperdoll = |item_id: i32, slot: PaperdollSlot| ItemRow {
        object_id: oid_of(item_id),
        item_id,
        count: 1,
        enchant_level: 6,
        loc: "PAPERDOLL".into(),
        loc_data: slot as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: 0,
        augment_mineral: 0,
        augment_option1: 0,
        augment_option2: 0,
    };
    chr.items = vec![
        paperdoll(HELMET, PaperdollSlot::Head),
        paperdoll(BREASTPLATE, PaperdollSlot::Chest),
        paperdoll(GAITERS, PaperdollSlot::Legs),
    ];
    // As if an older build had flushed a set skill into `character_skills`
    // before the filter existed: it must be dropped on load and then re-granted
    // on its own merit, not inherited.
    chr.skills = vec![(SKILL_PLAIN_A, 1, 0), (SKILL_PLUS_SIX, 1, 0)];

    let bundle = Player::from_char(&world.data, &chr);

    assert!(
        bundle.skills.0.contains_key(&SKILL_PLAIN_A),
        "the worn set's plain passive is re-granted at login"
    );
    assert!(
        bundle.skills.0.contains_key(&SKILL_PLUS_SIX),
        "and the +6 skill, since every piece is at +6"
    );

    // Now the same stored rows with the gear *not* worn: the stale skill rows
    // must not survive, or a sold set would leave a permanent bonus.
    let mut naked = dummy_char(PLAYER, "Setter");
    naked.skills = vec![(SKILL_PLAIN_A, 1, 0), (SKILL_PLUS_SIX, 1, 0)];
    let bundle = Player::from_char(&world.data, &naked);
    assert!(
        !bundle.skills.0.contains_key(&SKILL_PLAIN_A)
            && !bundle.skills.0.contains_key(&SKILL_PLUS_SIX),
        "stale set-skill rows are dropped when the set isn't worn"
    );
}

/// Pin what the dist actually loads. `armor_set_world`'s `is_empty` check would
/// pass on a parser that read one set out of 317; these numbers are the ones
/// the reachability census was computed against, so a change here means the
/// census needs redoing, not just the assertion.
#[test]
fn the_dist_armor_sets_load_as_censused() {
    let (world, _db, _l) = armor_set_world();
    let sets = &world.data.armor_sets;
    assert_eq!(sets.len(), 317, "every <set> in data/stats/armorsets");

    let mithril = sets.get(1).expect("set 1");
    assert_eq!(mithril.minimum_pieces, 3);
    assert_eq!(mithril.required_items, vec![BREASTPLATE, GAITERS, HELMET]);
    assert_eq!(mithril.optional_items, vec![HOPLON]);
    assert_eq!(mithril.skills.len(), 4);

    // The by-item index has to reach a set from any of its pieces, the shield
    // included — Java builds it from `concat(required, optional)`.
    for piece in [HELMET, BREASTPLATE, GAITERS, HOPLON] {
        assert!(
            sets.sets_for_item(piece).contains(&1),
            "item {piece} indexes back to set 1"
        );
    }
}
