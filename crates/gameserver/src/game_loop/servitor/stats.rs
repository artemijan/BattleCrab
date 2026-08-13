//! Pet stats: per-level template substitution and stat recalculation.

use super::*;

/// A pet's NPC template with its **per-level pet stats substituted in**.
///
/// Java does this at the finalizer level: `MaxHpFinalizer`, `PDefenseFinalizer`,
/// `MDefenseFinalizer`, `MaxMpFinalizer` and
/// `IStatFunction.calcWeaponBaseValue` each check `isPet()` and read
/// `getPetLevelData()` **instead of** the template base, then run the *same*
/// bonus math. Substituting the bases up front reproduces that exactly while
/// reusing the whole existing NPC stat pipeline (STR/INT/CON/MEN bonuses,
/// levelMod, the m.atk 2.2072 power, passive skills, buffs).
///
/// The **level is substituted too** — a pet's `levelMod` follows its own level,
/// not the NPC template's, which is most of why a levelled pet gets stronger.
fn pet_template_at_level(
    t: &crate::data::npc_data::NpcTemplate,
    row: &crate::data::pet_data::PetLevel,
    level: i32,
) -> crate::data::npc_data::NpcTemplate {
    let mut t = t.clone();
    t.level = level;
    // A row that does not carry a given stat keeps the NPC template's value
    // rather than substituting a zero. Java reads the pet row unconditionally
    // and every shipped species populates all of these, so this never fires on
    // real data — but a single missing `org_hp` would otherwise give the pet
    // **0 max HP**, and it is not worth losing a pet to a datapack typo.
    let or_template = |v: f64, fallback: f64| if v > 0.0 { v } else { fallback };
    t.base_p_atk = or_template(row.p_atk, t.base_p_atk);
    t.base_m_atk = or_template(row.m_atk, t.base_m_atk);
    t.base_p_def = or_template(row.p_def, t.base_p_def);
    t.base_m_def = or_template(row.m_def, t.base_m_def);
    t.base_hp_max = or_template(row.max_hp, t.base_hp_max);
    t.base_mp_max = or_template(row.max_mp, t.base_mp_max);
    t
}

/// Recompute a live pet's stats for its current level, preserving the HP/MP
/// *fraction* across a max-HP change so levelling neither heals nor wounds it.
pub(crate) fn recalculate_pet_stats(world: &mut World, pet_oid: i32) {
    let Some(level) = world
        .objects
        .get_component::<crate::model::components::PetOf>(&pet_oid)
        .map(|p| p.level)
    else {
        return;
    };
    let Some(npc_id) = npc_template_id(world, pet_oid) else {
        return;
    };
    let Some(row) = world
        .data
        .pet_data
        .get(npc_id)
        .and_then(|t| t.levels.get(&level).cloned())
    else {
        return;
    };
    let Some(template) = world.data.npc_data.get(npc_id).cloned() else {
        return;
    };
    let petted = pet_template_at_level(&template, &row, level);

    let buffs = world
        .objects
        .get_component::<crate::model::components::Buffs>(&pet_oid)
        .cloned()
        .unwrap_or_default();
    let (mut combat, speeds, max_hp, max_mp) =
        // A pet is a `Summon`, not an `Attackable`, so it can never be a
        // champion — neutral mods.
        crate::model::npc_finalized_stats(
            &world.data,
            &petted,
            &buffs,
            crate::model::ChampionStatMods::default(),
        );

    // A pet's worn armour adds to its defences. Java runs pets through the same
    // finalizers as everyone else, which sum the paperdoll; the port's NPC
    // pipeline has no inventory step, so the sum is done here against the
    // **pet's own** paperdoll (`PetInventory`, held on the owner).
    //
    // Only the defensive stats are folded: the 96 pet-armour items on this dist
    // are armour, and a pet has no weapon slot to speak of.
    let owner = world
        .objects
        .get_component::<ServitorOf>(&pet_oid)
        .map(|s| s.owner_object_id);
    if let Some(owner) = owner
        && let Some(pi) = world
            .objects
            .get_component::<crate::model::inventory::PetInventory>(&owner)
    {
        for item in pi.0.equipped_items() {
            let Some(stats) = world.data.item_data.item_stats(item.item_id) else {
                continue;
            };
            for &(stat, val) in &stats.bonuses {
                match stat {
                    crate::model::stats::Stat::PhysicalDefence => combat.p_def += val,
                    crate::model::stats::Stat::MagicalDefence => combat.m_def += val,
                    _ => {}
                }
            }
        }
    }

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid) {
        // Keep the bar where it was proportionally — Java's stat recompute does
        // not refill a pet on level-up.
        let hp_frac = if v.max_hp > 0 {
            v.cur_hp / v.max_hp as f64
        } else {
            1.0
        };
        let mp_frac = if v.max_mp > 0 {
            v.cur_mp / v.max_mp as f64
        } else {
            1.0
        };
        v.max_hp = max_hp.round() as i32;
        v.max_mp = max_mp.round() as i32;
        v.cur_hp = (v.max_hp as f64 * hp_frac).min(v.max_hp as f64);
        v.cur_mp = (v.max_mp as f64 * mp_frac).min(v.max_mp as f64);
    }
    world.objects.add_components(&pet_oid, combat);
    world.objects.add_components(&pet_oid, speeds);
}
