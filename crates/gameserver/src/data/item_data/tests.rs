//! Item catalogue tests — the dist-loading assertions and the guard that
//! keeps `ItemTemplate::for_test`'s reduced fixture honest.

use super::kinds::{ActionType, CrystalType, EtcItemType, ItemHandler, ItemKind};
use super::template::{ItemTemplate, TradeFlags};
use super::*;

use crate::data::dist;

#[test]
fn loads_short_sword_and_adena() {
    let data = dist::items();
    let sword = data.get(1).expect("item 1 (Short Sword)");
    assert_eq!(sword.name, "Short Sword");
    assert_eq!(sword.kind, ItemKind::Weapon);
    assert_eq!(sword.body_part, SLOT_R_HAND);
    assert!(sword.is_equipable());
    // No-grade weapon → CrystalType::None (level 0), so it never penalizes.
    assert_eq!(sword.crystal_type, CrystalType::None);
    assert_eq!(sword.crystal_type.level(), 0);

    // Ranged weapons (G20): Short Bow 13 costs MP per shot and reaches 500;
    // Wooden Arrow 17 is ARROW ammunition. Both are no-grade, so they match.
    let bow = data.get(13).expect("Short Bow 13");
    assert_eq!(bow.mp_consume, 1, "a bow spends MP per shot");
    assert_eq!(bow.crystal_type, CrystalType::None);
    let arrow = data.get(17).expect("Wooden Arrow 17");
    assert_eq!(arrow.etc_item_type, EtcItemType::Arrow);
    assert_eq!(
        arrow.crystal_type,
        CrystalType::None,
        "matches the no-grade bow"
    );
    // A melee weapon spends nothing.
    assert_eq!(data.get(2).map(|t| t.mp_consume), Some(0));

    // Melee sweep geometry (G20): a polearm reaches further than a sword,
    // both with a 120-degree arc (`damage_range` = a;b;radius;angle).
    let polearm = data.get(15).expect("polearm 15");
    assert_eq!((polearm.attack_radius, polearm.attack_angle), (66, 120));
    assert_eq!((sword.attack_radius, sword.attack_angle), (40, 120));

    // G15 item-cast slice: the flags `ItemSkillsTemplate` branches on.
    // A Scroll of Escape is *not* immediate — it casts its 20 s skill —
    // while a Healing Potion is, so it fires instantly. Both are
    // SKILL_REDUCE, which is what makes `checkConsume` spend them.
    let soe = data.get(736).expect("Scroll of Escape 736");
    assert!(!soe.immediate_effect, "SoE must take the cast branch");
    assert!(!soe.ex_immediate_effect);
    assert_eq!(soe.default_action, ActionType::SkillReduce);
    assert_eq!(soe.item_skills, vec![(2013, 1)]);
    let potion = data.get(1060).expect("Healing Potion 1060");
    assert!(potion.immediate_effect, "potions stay instant");
    assert_eq!(potion.default_action, ActionType::SkillReduce);
    // Packs are CAPSULE + immediate.
    let pack = data.get(22599).expect("spiritshot pack 22599");
    assert!(pack.immediate_effect);
    assert_eq!(pack.default_action, ActionType::Capsule);

    // A graded item parses its <set name="crystal_type"/>.
    let boots = data.get(40).expect("item 40 (Leather Boots)");
    assert_eq!(boots.crystal_type, CrystalType::D);
    assert_eq!(boots.crystal_type.level(), 1);

    let adena = data.get(ADENA_ID).expect("adena");
    assert!(adena.is_stackable);
    assert_eq!(adena.type2, TYPE2_MONEY);
    assert!(!adena.is_equipable());

    // Shots: the D-grade soulshot (1463) resolves to the SoulShots handler
    // and carries its NORMAL visual skill; a graded weapon declares a shot
    // count so it can charge (Java `Weapon._soulShotCount`).
    let soulshot = data.get(1463).expect("item 1463 (Soulshot D)");
    assert_eq!(soulshot.handler, ItemHandler::SoulShots);
    assert!(soulshot.item_skills.iter().any(|&(id, _)| id == 2150));
    assert_eq!(soulshot.crystal_type, CrystalType::D);
    // Some real weapon must declare soulshots/spiritshots.
    assert!(
        data.weapon_shots.values().any(|&(ss, _)| ss > 0),
        "a weapon declares a soulshot count"
    );
    assert!(
        data.weapon_shots.values().any(|&(_, sps)| sps > 0),
        "a weapon declares a spiritshot count"
    );

    assert!(data.by_id.len() > 5000);
}

#[test]
fn parses_extractable_pack_handler_and_capsules() {
    let data = dist::items();
    let pack = data
        .get(15195)
        .expect("item 15195 (Mage Class Equipment Set, 10-day)");
    assert_eq!(pack.handler, ItemHandler::ExtractableItems);
    assert_eq!(pack.extractable_count_min, 0);
    assert_eq!(pack.extractable_count_max, 0);
    assert_eq!(pack.capsuled_items.len(), 9);
    let robe = pack
        .capsuled_items
        .iter()
        .find(|c| c.item_id == 15230)
        .expect("Dark Crystal Robe pack entry");
    assert_eq!(robe.min, 1);
    assert_eq!(robe.max, 1);
    assert_eq!(robe.chance, 100_000); // chance="100" -> (100.0 * 1000) as i32

    let box_item = data
        .get(23762)
        .expect("item 23762 (High-grade Elixir Pack)");
    assert_eq!(box_item.extractable_count_min, 1);
    assert_eq!(box_item.extractable_count_max, 1);
}

/// The datapack's transfer restrictions: *Mage Class Equipment Set
/// (10-day)* (15195) is bound — untradable, undroppable, unsellable and
/// time-limited — while an ordinary item declares none of the tags and so
/// inherits Java's permissive defaults.
#[test]
fn parses_bound_item_trade_flags() {
    let data = dist::items();
    let bound = data
        .get(15195)
        .expect("item 15195 (Mage Class Equipment Set, 10-day)");
    assert!(!bound.is_dropable(), "is_dropable=false in the XML");
    assert!(!bound.is_tradable(), "is_tradable=false in the XML");
    assert!(!bound.is_sellable, "is_sellable=false in the XML");
    assert!(bound.is_time_limited(), "time=14400 makes it expire");
    // Storing and destroying stay available: a private warehouse takes
    // untradable items, and nothing marks the box undestroyable.
    assert!(bound.is_depositable(true), "a private warehouse takes it");
    assert!(!bound.is_depositable(false), "the clan warehouse does not");
    assert!(bound.is_destroyable(), "it can still be deleted");

    // An ordinary item declares none of the tags → all defaults true.
    let sword = data.get(1).expect("item 1 (Short Sword)");
    assert!(sword.is_dropable());
    assert!(sword.is_tradable());
    assert!(sword.is_destroyable());
    assert!(sword.is_depositable(false));
    assert!(!sword.is_time_limited());

    // Quest items are never depositable (Java forces the flag off).
    let quest = data
        .all()
        .find(|t| t.is_quest_item)
        .expect("at least one quest item");
    assert!(
        !quest.is_depositable(true),
        "quest items stay out of the WH"
    );
}

#[test]
fn parses_item_icons_with_fallback() {
    let data = dist::items();
    // Adena carries an explicit `<set name="icon">`.
    assert_eq!(data.icon(57), "icon.etc_adena_i00");
    // An unknown item falls back to the client question-mark (Java default).
    assert_eq!(data.icon(-1), "icon.etc_question_mark_i00");
    // `all()` yields the loaded catalog (Java `getAllItems`).
    assert!(
        data.all().any(|i| i.item_id == 57),
        "adena is in the catalog"
    );
}

#[test]
fn parses_weapon_and_armor_stats() {
    let data = dist::items();

    // Short Sword (item 1): pAtk/mAtk/rCrit/pAtkSpd + range/random-damage.
    let sword = data.item_stats(1).expect("item 1 <stats>");
    let get = |s: Stat| {
        sword
            .bonuses
            .iter()
            .find(|(st, _)| *st == s)
            .map(|(_, v)| *v)
    };
    assert_eq!(get(Stat::PhysicalAttack), Some(8.0));
    assert_eq!(get(Stat::MagicalAttack), Some(6.0));
    assert_eq!(get(Stat::CriticalRate), Some(8.0));
    assert_eq!(get(Stat::PhysicalAttackSpeed), Some(379.0));
    assert_eq!(sword.atk_range, Some(40)); // pAtkRange (not a Stat)
    assert_eq!(sword.random_damage, Some(10)); // randomDamage (not a Stat)

    // Leather Boots (item 40): a single pDef contribution.
    let boots = data.item_stats(40).expect("item 40 <stats>");
    assert_eq!(boots.bonuses, vec![(Stat::PhysicalDefence, 19.0)]);

    // Hoplon (item 628): a shield — sDef/rShld parsed into the shield fields
    // (not the Stat bonus list), rEvas into the sum-add bonuses.
    let hoplon = data.item_stats(628).expect("item 628 <stats>");
    assert_eq!(hoplon.shield_def, Some(128));
    assert_eq!(hoplon.shield_rate, Some(20));
    assert_eq!(
        hoplon
            .bonuses
            .iter()
            .find(|(s, _)| *s == Stat::EvasionRate)
            .map(|(_, v)| *v),
        Some(-8.0)
    );

    // Stackable/etc items with no <stats> have no side-map entry.
    assert!(data.item_stats(ADENA_ID).is_none());
}

mod for_test_is_faithful {
    use super::*;

    /// Every field the fixture reduction dropped, asserted against what
    /// `for_test()` actually produces. If this drifts, the reduced fixtures
    /// silently change meaning.
    #[test]
    fn dropped_fields_match_the_base() {
        let t = ItemTemplate::for_test();
        assert_eq!(t.trade_flags, TradeFlags::default());
        assert_eq!(t.time, -1);
        assert_eq!(t.duration, -1);
        assert!(!t.immediate_effect);
        assert!(!t.ex_immediate_effect);
        assert_eq!(t.default_action, ActionType::Other);
        assert_eq!(t.weight, 0);
        assert!(!t.is_infinite);
        assert_eq!(t.type1, 0);
        assert_eq!(t.type2, 0);
        assert!(t.is_sellable);
        assert!(!t.is_freightable);
        assert_eq!(t.price, 0);
        assert_eq!(t.handler, ItemHandler::None);
        assert_eq!(t.crystal_type, CrystalType::None);
        assert_eq!(t.crystal_count, 0);
        assert_eq!(t.attack_radius, 40);
        assert_eq!(t.attack_angle, 0);
        assert_eq!(t.mp_consume, 0);
        assert_eq!(t.reduced_mp_consume, 0);
        assert_eq!(t.reduced_mp_consume_chance, 0);
        assert!(t.capsuled_items.is_empty());
        assert_eq!(t.extractable_count_min, 0);
        assert_eq!(t.extractable_count_max, 0);
        assert!(t.item_skills.is_empty());
        assert_eq!(t.etc_item_type, EtcItemType::Other);
        assert!(!t.enchant_enabled);
        assert_eq!(t.enchant_limit, 0);
        assert!(!t.is_magic_weapon);
    }
}
