//! G15.5 — the augment options' **skill** half: the active skills they grant
//! and the activation ("proc") skills they register.
//!
//! The stat half has worked since G15; these are the two things `Options.apply`
//! does that this port carried on the entry and never granted. On this dist
//! that is 1793 `<active_skill>` rows and 688 triggers (240 attack, 240
//! critical, 208 magic).

use super::*;

use crate::data::option_data::{OptionEntry, OptionSkillType, OptionTrigger};
use crate::model::components::{Buffs, OptionSkills, OptionTriggers, SkillBook};
use crate::model::skill::{AffectObject, AffectScope, OperateType, Skill, SkillEffect, TargetType};

const PLAYER: i32 = 8001;
const CID: u32 = 1;
const MOB_ID: i32 = 48000;
const MOB_OID: i32 = NPC_OID;
/// The skill an option grants as an *active* (Mana Burn's shape).
const ACTIVE: i32 = 9910;
/// The skill an option's proc fires.
const PROC: i32 = 9911;

fn augment_world() -> (
    World,
    db::CmdRx,
    tokio::sync::mpsc::UnboundedReceiver<LoginLinkCommand>,
) {
    let (mut world, db, l) = combat_test_world();
    let mut t = crate::data::npc_data::default_template(MOB_ID);
    t.type_name = "Monster".into();
    t.name = "Dummy".into();
    t.level = 5;
    t.base_hp_max = 5000.0;
    t.collision_radius = 10.0;
    world.data.npc_data.insert_for_test(t);

    // Both skills are plain 60s self-buffs, so "did it land" is "is it up".
    for id in [ACTIVE, PROC] {
        world.data.skill_data.insert_for_test(Skill {
            id,
            level: 1,
            name: format!("Aug{id}"),
            operate_type: OperateType::Active,
            target_type: TargetType::Self_,
            affect_scope: AffectScope::Single,
            affect_object: AffectObject::All,
            is_continuous: true,
            abnormal_time: 60,
            abnormal_type: format!("AUG{id}"),
            effects: vec![SkillEffect::StatModifier(Default::default())],
            ..Default::default()
        });
    }
    (world, db, l)
}

fn has_buff(world: &World, oid: i32, skill_id: i32) -> bool {
    world
        .objects
        .get_component::<Buffs>(&oid)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == skill_id && !x.passive))
}

/// An option granting one active skill.
fn active_option(id: i32) -> OptionEntry {
    OptionEntry {
        id,
        active_skills: vec![(ACTIVE, 1)],
        ..Default::default()
    }
}

/// An option registering one proc of `kind`. `chance` is Java's `Rnd.get(100) <
/// chance`, so 100.0 always fires and 0.0 never does — no forced rolls needed,
/// and the tests stay about the *routing* rather than the RNG.
fn proc_option(id: i32, kind: OptionSkillType, chance: f64) -> OptionEntry {
    OptionEntry {
        id,
        triggers: vec![OptionTrigger {
            skill_id: PROC,
            skill_level: 1,
            chance,
            kind,
        }],
        ..Default::default()
    }
}

/// Equip an augmented weapon carrying `options`, through the real item path.
fn equip_augmented(
    world: &mut World,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
    options: [i32; 2],
) -> i32 {
    use crate::data::item_data::{
        CrystalType, ItemHandler, ItemKind, ItemStats, ItemTemplate, SLOT_R_HAND,
    };
    use crate::model::inventory::Inventory;
    use crate::model::stats::Stat;

    const ITEM_ID: i32 = 601;
    const ITEM_OID: i32 = 9501;
    if world.data.item_data.get(ITEM_ID).is_none() {
        world.data.item_data.insert_for_test(ItemTemplate {
            item_id: ITEM_ID,
            name: "Augmented Blade".into(),
            kind: ItemKind::Weapon,
            body_part: SLOT_R_HAND,
            time: -1,
            duration: -1,
            crystal_type: CrystalType::None,
            handler: ItemHandler::None,
            default_action: crate::data::item_data::ActionType::Other,
            attack_radius: 40,
            is_sellable: true,
            ..Default::default()
        });
        world.data.item_data.set_item_stats_for_test(
            ITEM_ID,
            ItemStats {
                bonuses: vec![(Stat::PhysicalAttack, 10.0)],
                ..Default::default()
            },
        );
    }
    {
        let World { objects, data, .. } = world;
        let inv = objects.get_component_mut::<Inventory>(&PLAYER).unwrap();
        inv.add_item(&data.item_data, ITEM_OID, ITEM_ID, 1);
        inv.set_augmentation(ITEM_OID, 8723, options[0], options[1]);
    }
    drain(rx);
    items::handle_use_item(world, CID, &use_item_body(ITEM_OID));
    ITEM_OID
}

// ---------------------------------------------------------------------------
// Active skills
// ---------------------------------------------------------------------------

/// **The headline.** An augment's active skill is granted on equip, is
/// castable, and is taken back on unequip — and at no point does it enter the
/// persisted [`SkillBook`], because Java grants it with `store = false` and
/// this port writes the whole book to `character_skills`.
#[test]
fn an_augment_active_is_granted_castable_and_never_persisted() {
    let (mut world, ..) = augment_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.id_pool = 0x4200_0000..0x4200_0100;
    world.data.options.insert_for_test(active_option(4001));
    world.data.options.insert_for_test(OptionEntry {
        id: 4002,
        ..Default::default()
    });

    let item_oid = equip_augmented(&mut world, &mut rx, [4001, 4002]);

    assert_eq!(
        world
            .objects
            .get_component::<OptionSkills>(&PLAYER)
            .and_then(|s| s.0.get(&ACTIVE).copied()),
        Some(1),
        "granted into the transient set"
    );
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&PLAYER)
            .is_some_and(|b| b.0.contains_key(&ACTIVE)),
        "must never reach the persisted book — Java's addSkill(…, false)"
    );
    // Castable: the cast path's known-skill lookup has to see it, or the skill
    // sits on the bar and answers every click with ActionFailed.
    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        Some(1),
        "the cast path resolves it"
    );

    // Unequip: gone again.
    items::handle_use_item(&mut world, CID, &use_item_body(item_oid));
    assert!(
        !world
            .objects
            .get_component::<OptionSkills>(&PLAYER)
            .is_some_and(|s| s.0.contains_key(&ACTIVE)),
        "taken back with the item"
    );
    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        None
    );
}

/// A player's **own** trained level wins over an option granting the same
/// skill, so an item can never downgrade what they learned.
#[test]
fn the_skill_book_wins_over_an_option_granting_the_same_skill() {
    let (mut world, ..) = augment_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<SkillBook>(&PLAYER)
        .unwrap()
        .0
        .insert(ACTIVE, 7);
    world
        .objects
        .get_component_mut::<OptionSkills>(&PLAYER)
        .unwrap()
        .0
        .insert(ACTIVE, 1);
    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        Some(7)
    );
}

/// Java gets its augment bonuses back through `restoreCharData` re-running the
/// equip listeners. This port had no such replay, so an augmented weapon worn
/// *through* a relog contributed nothing — stats included — until it was
/// manually re-equipped.
#[test]
fn equipped_augments_are_reapplied_at_enter_world() {
    use crate::data::item_data::{CrystalType, ItemHandler, ItemKind, ItemTemplate, SLOT_R_HAND};
    use crate::model::inventory::Inventory;

    let (mut world, ..) = augment_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.data.options.insert_for_test(active_option(4001));
    world.data.item_data.insert_for_test(ItemTemplate {
        item_id: 601,
        name: "Augmented Blade".into(),
        kind: ItemKind::Weapon,
        body_part: SLOT_R_HAND,
        time: -1,
        duration: -1,
        crystal_type: CrystalType::None,
        handler: ItemHandler::None,
        default_action: crate::data::item_data::ActionType::Other,
        attack_radius: 40,
        is_sellable: true,
        ..Default::default()
    });
    // Straight into the paperdoll, as a restored character arrives: no equip
    // packet is ever sent, so only the enter-world replay can grant this.
    {
        let World { objects, data, .. } = &mut world;
        let inv = objects.get_component_mut::<Inventory>(&PLAYER).unwrap();
        inv.add_item(&data.item_data, 9501, 601, 1);
        inv.set_augmentation(9501, 8723, 4001, 0);
        inv.equip_item(&data.item_data, 9501);
    }
    assert!(
        !world
            .objects
            .get_component::<OptionSkills>(&PLAYER)
            .is_some_and(|s| s.0.contains_key(&ACTIVE)),
        "nothing granted yet"
    );

    crate::game_loop::options::apply_equipped_item_options(&mut world, PLAYER);

    assert_eq!(
        world
            .objects
            .get_component::<OptionSkills>(&PLAYER)
            .and_then(|s| s.0.get(&ACTIVE).copied()),
        Some(1),
        "the worn augment's active is back after login"
    );
}

// ---------------------------------------------------------------------------
// Activation (proc) skills
// ---------------------------------------------------------------------------

fn arm_trigger(world: &mut World, kind: OptionSkillType, chance: f64) {
    world
        .objects
        .get_component_mut::<OptionTriggers>(&PLAYER)
        .unwrap()
        .0
        .insert(
            PROC,
            OptionTrigger {
                skill_id: PROC,
                skill_level: 1,
                chance,
                kind,
            },
        );
}

/// `ATTACK` fires on an ordinary hit and **not** on a critical; `CRITICAL` is
/// the exact mirror. Java writes them as one `if` with that split, so getting it
/// wrong makes every crit fire the wrong proc.
#[test]
fn attack_and_critical_procs_split_on_criticality() {
    for (kind, crit, expect) in [
        (OptionSkillType::Attack, false, true),
        (OptionSkillType::Attack, true, false),
        (OptionSkillType::Critical, true, true),
        (OptionSkillType::Critical, false, false),
    ] {
        let (mut world, ..) = augment_world();
        let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
        world.id_pool = 0x4200_0000..0x4200_0100;
        add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 60, 0, 0);
        // End-to-end: the proc is registered by equipping a real augmented
        // weapon, so this covers `Options.apply`'s trigger half and the
        // auto-attack call site as well as the routing.
        world
            .data
            .options
            .insert_for_test(proc_option(4003, kind, 100.0));
        world.data.options.insert_for_test(OptionEntry {
            id: 4004,
            ..Default::default()
        });
        equip_augmented(&mut world, &mut rx, [4003, 4004]);
        assert!(
            world
                .objects
                .get_component::<OptionTriggers>(&PLAYER)
                .is_some_and(|r| r.0.contains_key(&PROC)),
            "the option registered its proc on equip"
        );

        crate::game_loop::combat::handle_attack_hit(
            &mut world, PLAYER, MOB_OID, 50, false, crit, 0,
        );

        assert_eq!(
            has_buff(&world, MOB_OID, PROC),
            expect,
            "{kind:?} on crit={crit}"
        );
    }
}

/// A 0-chance proc never fires — the roll is real, not bypassed.
#[test]
fn a_zero_chance_proc_never_fires() {
    let (mut world, ..) = augment_world();
    let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 60, 0, 0);
    arm_trigger(&mut world, OptionSkillType::Attack, 0.0);

    for _ in 0..20 {
        crate::game_loop::combat::handle_attack_hit(
            &mut world, PLAYER, MOB_OID, 50, false, false, 0,
        );
    }
    assert!(!has_buff(&world, MOB_OID, PROC));
}

/// The cast half: `MAGIC` fires on a magic cast, `ATTACK` on a physical one,
/// and a **static** skill (`magic_type = 2`) fires nothing — Java wraps the
/// whole block in `!skill.isStatic()`.
#[test]
fn cast_procs_split_magic_from_physical_and_skip_static() {
    for (kind, magic_type, expect) in [
        (OptionSkillType::Magic, 1, true),
        (OptionSkillType::Magic, 0, false),
        (OptionSkillType::Attack, 0, true),
        (OptionSkillType::Attack, 1, false),
        // Static: neither kind may fire.
        (OptionSkillType::Magic, 2, false),
        (OptionSkillType::Attack, 2, false),
    ] {
        let (mut world, ..) = augment_world();
        let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
        add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 60, 0, 0);
        arm_trigger(&mut world, kind, 100.0);

        crate::game_loop::skills::effects::fire_option_cast_triggers(
            &mut world, PLAYER, MOB_OID, magic_type,
        );

        assert_eq!(
            has_buff(&world, MOB_OID, PROC),
            expect,
            "{kind:?} on magic_type={magic_type}"
        );
    }
}

/// A `CRITICAL` proc belongs to the attack path only — a cast never fires it,
/// whatever the skill's type.
#[test]
fn a_critical_proc_never_fires_from_a_cast() {
    for magic_type in [0, 1] {
        let (mut world, ..) = augment_world();
        let _rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
        add_test_npc(&mut world, MOB_OID, MOB_ID, "Monster", 5, 60, 0, 0);
        arm_trigger(&mut world, OptionSkillType::Critical, 100.0);

        crate::game_loop::skills::effects::fire_option_cast_triggers(
            &mut world, PLAYER, MOB_OID, magic_type,
        );
        assert!(!has_buff(&world, MOB_OID, PROC), "magic_type={magic_type}");
    }
}

/// Destroying a *worn* augmented item must take its options back with it.
///
/// This drives the documented destroy protocol exactly as `remove_item`'s doc
/// comment prescribes — snapshot `equipped_object_ids`, remove, intersect via
/// `unequipped_by_removal`, hand the result to `finish_equipped_item_destroyed`
/// — so a failure here is a hole in the protocol itself, not in a call site
/// that skipped it.
#[test]
fn destroying_a_worn_augmented_item_takes_its_option_back() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = augment_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.id_pool = 0x4200_0000..0x4200_0100;
    world.data.options.insert_for_test(active_option(4001));

    let item_oid = equip_augmented(&mut world, &mut rx, [4001, 0]);
    let item_id = world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.object_id == item_oid)
                .map(|i| i.item_id)
        })
        .expect("the augmented weapon is in the bag");
    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        Some(1),
        "granted while worn"
    );

    // The destroy protocol, by the book.
    let before = world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .map(|inv| inv.equipped_object_ids())
        .unwrap_or_default();
    let changes = world
        .objects
        .get_component_mut::<Inventory>(&PLAYER)
        .map(|inv| inv.remove_item(item_id, 1))
        .unwrap_or_default();
    let unequipped = crate::game_loop::items::unequipped_by_removal(&before, &changes);
    assert_eq!(
        unequipped.iter().map(|i| i.object_id).collect::<Vec<_>>(),
        vec![item_oid],
        "the protocol correctly identifies the destroyed worn item"
    );
    crate::game_loop::items::finish_equipped_item_destroyed(&mut world, CID, PLAYER, &unequipped);

    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        None,
        "the option's skill must go with the destroyed item"
    );
}

/// A GM destroying gear off a player's back takes the augment options with it.
///
/// `//destroy` ran a bare `remove_item` — none of the destroy protocol — so the
/// target kept the option's stats and granted skills. This drives the real
/// admin command, so it covers the wiring, not just the helper.
#[test]
fn admin_destroy_takes_the_options_off_a_worn_item() {
    use crate::model::inventory::Inventory;
    let (mut world, ..) = augment_world();
    let mut rx = ingame_player(&mut world, CID, PLAYER, 0, 0, 0);
    world.id_pool = 0x4200_0000..0x4200_0100;
    world.data.options.insert_for_test(active_option(4001));

    let item_oid = equip_augmented(&mut world, &mut rx, [4001, 0]);
    let item_id = world
        .objects
        .get_component::<Inventory>(&PLAYER)
        .and_then(|inv| {
            inv.items()
                .iter()
                .find(|i| i.object_id == item_oid)
                .map(|i| i.item_id)
        })
        .expect("worn augmented weapon");
    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        Some(1),
        "granted while worn"
    );

    crate::game_loop::items::destroy_item_by_id(&mut world, PLAYER, item_id, 1);

    assert!(
        world
            .objects
            .get_component::<Inventory>(&PLAYER)
            .is_some_and(|inv| inv.first_of_item(item_id).is_none()),
        "the item is gone"
    );
    assert_eq!(
        crate::game_loop::skills::cast::known_skill_level(&world, PLAYER, ACTIVE),
        None,
        "and so is the option it granted"
    );
}
