//! `calc_max_hp` / `_mp` / `_cp` — the class-template curve plus equipment and
//! modifiers, and the vitals helpers that read the result.

use crate::data::GameData;
use crate::data::player_template::PlayerTemplate;

use super::components::StatModifiers;
use super::inventory::Inventory;
use super::stats::Stat;

/// Java `Creature.getCurrentHpPercent()` — `(int) ((currentHp * 100) / maxHp)`.
///
/// The integer truncation is Java's and is kept: at 30.9 % HP this answers 30,
/// so a `<hpPercent>30</hpPercent>` effect is already up. A max of 0 answers 0
/// rather than dividing by zero, which keeps a not-yet-initialised creature on
/// the "hurt" side — the same side Java's `0/0 = NaN` comparison would fail to.
pub(crate) fn hp_percent_of(cur_hp: f64, max_hp: i32) -> i32 {
    if max_hp <= 0 {
        return 0;
    }
    ((cur_hp * 100.0) / max_hp as f64) as i32
}

/// Sum of one `<stat>` across every equipped piece — the flat additive item
/// term the `MaxHp`/`MaxMp` finalizers apply *after* the CON/MEN multiply
/// (Java's `for (Item item : inv.getPaperdollItems()) maxHp += getStats(...)`).
fn equipped_stat_sum(inventory: &Inventory, data: &GameData, stat: Stat) -> f64 {
    inventory
        .equipped_items()
        .iter()
        .filter_map(|item| data.item_data.item_stats(item.item_id))
        .flat_map(|s| s.bonuses.iter())
        .filter(|(st, _)| *st == stat)
        .map(|(_, v)| *v)
        .sum()
}

/// `MaxHpFinalizer`: `mul·(baseHpMax(level)·CON bonus) + add`, plus each
/// equipped item's flat `maxHp` bonus (added *after* the buff `mul`, per Java —
/// items aren't scaled by the buff). `inventory = None` for the pre-equip
/// char-creation preview. The `mul`/`add` come from the buff modifier maps —
/// HP-boosting clan skills / buffs move the stat through here.
pub fn calc_max_hp(
    data: &GameData,
    t: &PlayerTemplate,
    level: i32,
    inventory: Option<&Inventory>,
    mods: &StatModifiers,
) -> f64 {
    let base = t.base_hp_max(level) * data.stat_bonus.con_bonus(t.base_con);
    let mul = mods.mul.get(&Stat::MaxHp).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MaxHp).copied().unwrap_or(0.0);
    let item = inventory
        .map(|inv| equipped_stat_sum(inv, data, Stat::MaxHp))
        .unwrap_or(0.0);
    let enchant = inventory
        .map(|inv| enchanted_armour_hp(inv, data))
        .unwrap_or(0.0);
    let total = mul * base + add + item + enchant;
    // `MaxHpFinalizer`'s HP_LIMIT arm: `min(maxHp, MAX_HP * mul + add)`. No
    // skill on this dist grants `hpLimit`, so the mul/add stay at 1/0 and the
    // ceiling is the flat config figure. Java lifts it outright for a
    // cursed-weapon wielder — Zariche and Akamanah, both Interlude weapons —
    // and for a dragon weapon, which is post-Interlude and unequippable here.
    let cursed = inventory.is_some_and(|inv| {
        inv.equipped_items().iter().any(|it| {
            data.cursed_weapons
                .weapons
                .iter()
                .any(|cw| cw.item_id == it.item_id)
        })
    });
    if cursed {
        total
    } else {
        total.min(data.combat_caps.max_hp)
    }
}

/// `MaxHpFinalizer`'s "Apply enchanted item bonus HP" arm: every equipped
/// **armour** piece that is enchanted adds a flat figure from
/// `enchantHPBonus.xml`, on top of its own `maxHp` stat.
///
/// Java excludes three slots by body part — necklace, earrings and rings —
/// which is why the test is on the slot rather than on "is it a jewel":
/// `ItemKind::Armor` covers jewellery too.
fn enchanted_armour_hp(inventory: &Inventory, data: &GameData) -> f64 {
    use crate::data::item_data::{ItemKind, SLOT_LR_EAR, SLOT_LR_FINGER, SLOT_NECK};
    inventory
        .equipped_items()
        .iter()
        .filter(|item| item.enchant_level > 0)
        .filter_map(|item| data.item_data.get(item.item_id).map(|t| (item, t)))
        .filter(|(_, t)| t.kind == ItemKind::Armor)
        .filter(|(_, t)| !matches!(t.body_part, SLOT_NECK | SLOT_LR_EAR | SLOT_LR_FINGER))
        .map(|(item, t)| {
            data.enchant_hp_bonus
                .bonus(t.crystal_type, item.enchant_level, t.body_part)
        })
        .sum()
}

/// `MaxMpFinalizer`: `mul·(baseMpMax(level)·MEN bonus) + add` + equipped `maxMp`.
pub fn calc_max_mp(
    data: &GameData,
    t: &PlayerTemplate,
    level: i32,
    inventory: Option<&Inventory>,
    mods: &StatModifiers,
) -> f64 {
    let base = t.base_mp_max(level) * data.stat_bonus.men_bonus(t.base_men);
    let mul = mods.mul.get(&Stat::MaxMp).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MaxMp).copied().unwrap_or(0.0);
    let item = inventory
        .map(|inv| equipped_stat_sum(inv, data, Stat::MaxMp))
        .unwrap_or(0.0);
    mul * base + add + item
}

/// `MaxCpFinalizer`: `mul·(baseCpMax(level)·CON bonus) + add`. No item bonus —
/// no item in this dist carries `maxCp`, and Java's `MaxCpFinalizer` has no
/// paperdoll loop.
pub fn calc_max_cp(data: &GameData, t: &PlayerTemplate, level: i32, mods: &StatModifiers) -> f64 {
    let base = t.base_cp_max(level) * data.stat_bonus.con_bonus(t.base_con);
    let mul = mods.mul.get(&Stat::MaxCp).copied().unwrap_or(1.0);
    let add = mods.add.get(&Stat::MaxCp).copied().unwrap_or(0.0);
    mul * base + add
}
