//! `Player::recalculate_stats` — the per-recalc pass that turns base stats,
//! equipment and active modifiers into the cached `CombatStats`/`Speeds` —
//! plus the equipment bonus table it reads.

use crate::data::GameData;
use crate::data::player_template::PlayerTemplate;

use super::Player;
use super::components::stats::{BaseStats, CombatStats, Speeds, StatModifiers};
use super::inventory::Inventory;
use super::stat_finalize::{finalize, finalize_def, finalize_speed};
use super::stats::{BaseStat, Stat};

/// Equipped-gear contributions to the combat finalizers, summarized from the
/// paperdoll once per recompute — Java re-reads item `getStats(...)` inside
/// each finalizer, but the numbers are the same. Two families, matching the
/// Java stat finalizers (see [`crate::data::item_data::template::ItemStats`]):
///   * **weapon-replace** bases (`None` ⇒ fall back to the class template
///     base) — `calcWeaponBaseValue`, the equipped weapon only;
///   * **sum-add** contributions (0.0 when nothing equipped adds them) —
///     summed across every equipped piece.
struct EquippedBonuses {
    weapon_p_atk: Option<f64>,
    weapon_m_atk: Option<f64>,
    weapon_p_atk_spd: Option<f64>,
    weapon_crit: Option<f64>,
    weapon_m_crit: Option<f64>,
    weapon_atk_range: Option<i32>,
    weapon_random_dmg: Option<i32>,
    p_def: f64,
    m_def: f64,
    accuracy: f64,
    magic_accuracy: f64,
    evasion: f64,
    magic_evasion: f64,
    /// Sum of `getBaseDefBySlot` over the *occupied* pDef/mDef slots — the naked
    /// slot defenses the P/MDefenseFinalizer subtracts so worn gear replaces
    /// (not stacks on) the class base. See the finalizer loops in Java's
    /// `PDefenseFinalizer`/`MDefenseFinalizer`.
    p_def_slot_sub: f64,
    m_def_slot_sub: f64,
    /// `calcEnchantedItemBonus` per stat — the extra attack/defence an
    /// **enchanted** piece contributes on top of its own declared value. Java
    /// folds each into its finalizer *before* the stat bonus and level mod, so
    /// they are carried separately rather than merged into `p_def`/`weapon_p_atk`.
    enchant_p_atk: f64,
    enchant_m_atk: f64,
    enchant_p_def: f64,
    enchant_m_def: f64,
    /// `ShotsBonusFinalizer` — `1 + enchantLevel·0.003` off the equipped weapon.
    shots_bonus: f64,
}

/// Hand-written rather than derived: `shots_bonus`'s identity is **1**, and a
/// derived `0.0` would silently delete every soulshot's damage bonus.
impl Default for EquippedBonuses {
    fn default() -> Self {
        Self {
            weapon_p_atk: None,
            weapon_m_atk: None,
            weapon_p_atk_spd: None,
            weapon_crit: None,
            weapon_m_crit: None,
            weapon_atk_range: None,
            weapon_random_dmg: None,
            p_def: 0.0,
            m_def: 0.0,
            accuracy: 0.0,
            magic_accuracy: 0.0,
            evasion: 0.0,
            magic_evasion: 0.0,
            p_def_slot_sub: 0.0,
            m_def_slot_sub: 0.0,
            enchant_p_atk: 0.0,
            enchant_m_atk: 0.0,
            enchant_p_def: 0.0,
            enchant_m_def: 0.0,
            shots_bonus: 1.0,
        }
    }
}

impl EquippedBonuses {
    fn from_inventory(inventory: &Inventory, data: &GameData, t: &PlayerTemplate) -> Self {
        use crate::model::inventory::PaperdollSlot;
        let mut eq = EquippedBonuses::default();

        // P/MDefenseFinalizer's slot loops: for every occupied armor slot,
        // subtract the class template's naked defense for that slot. The pDef
        // legs slot also counts when a full-armor chest covers the legs (its
        // `isPaperdollSlotEmpty(LEGS) || (CHEST is FULL_ARMOR)` guard).
        let occupied = |slot: PaperdollSlot| inventory.paperdoll_item(slot).is_some();
        let chest_is_full_armor = inventory
            .paperdoll_item(PaperdollSlot::Chest)
            .and_then(|it| data.item_data.get(it.item_id))
            .map(|tpl| tpl.body_part == crate::data::item_data::SLOT_FULL_ARMOR)
            .unwrap_or(false);
        for slot in [
            PaperdollSlot::Chest,
            PaperdollSlot::Head,
            PaperdollSlot::Feet,
            PaperdollSlot::Gloves,
            PaperdollSlot::Under,
            PaperdollSlot::Cloak,
            PaperdollSlot::Hair,
        ] {
            if occupied(slot) {
                eq.p_def_slot_sub += t.base_def_by_slot(slot as usize) as f64;
            }
        }
        if occupied(PaperdollSlot::Legs) || chest_is_full_armor {
            eq.p_def_slot_sub += t.base_def_by_slot(PaperdollSlot::Legs as usize) as f64;
        }
        for slot in [
            PaperdollSlot::LFinger,
            PaperdollSlot::RFinger,
            PaperdollSlot::LEar,
            PaperdollSlot::REar,
            PaperdollSlot::Neck,
        ] {
            if occupied(slot) {
                eq.m_def_slot_sub += t.base_def_by_slot(slot as usize) as f64;
            }
        }

        // `ShotsBonusFinalizer`: `1 + enchantLevel·0.003`, read off the active
        // weapon instance. Java's `getActiveWeaponInstance()` is the right hand.
        if let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand) {
            eq.shots_bonus = crate::model::enchant_bonus::shots_bonus(weapon.enchant_level);
        }

        // `calcEnchantedItemBonus`, run once over the paperdoll instead of once
        // per finalizer: Java calls it from `PAttackFinalizer`,
        // `MAttackFinalizer`, `PDefenseFinalizer` and `MDefenseFinalizer`, each
        // asking about its own stat, and the per-item gate differs by stat only
        // through `enchant_bonus_applies`.
        for item in inventory
            .equipped_items()
            .into_iter()
            .filter(|i| i.enchant_level > 0)
        {
            let Some(tpl) = data.item_data.get(item.item_id) else {
                continue;
            };
            let declares = |stat: Stat| {
                data.item_data
                    .item_stats(item.item_id)
                    .is_some_and(|st| st.bonuses.iter().any(|&(s, v)| s == stat && v > 0.0))
            };
            let body_part = tpl.body_part;
            use crate::model::enchant_bonus::{
                enchant_bonus_applies, enchant_def_bonus, enchant_m_atk_bonus, enchant_p_atk_bonus,
            };
            // `stat == PHYSICAL_ATTACK && equippedItem.isWeapon()` — the extra
            // weapon test Java applies only on this arm.
            if tpl.kind == crate::data::item_data::kinds::ItemKind::Weapon
                && enchant_bonus_applies(body_part, declares(Stat::PhysicalAttack), false)
            {
                eq.enchant_p_atk += enchant_p_atk_bonus(
                    tpl.crystal_type,
                    body_part,
                    data.item_data.weapon_type(item.item_id),
                    item.enchant_level,
                );
            }
            if enchant_bonus_applies(body_part, declares(Stat::MagicalAttack), false) {
                eq.enchant_m_atk += enchant_m_atk_bonus(tpl.crystal_type, item.enchant_level);
            }
            if enchant_bonus_applies(body_part, declares(Stat::PhysicalDefence), true) {
                eq.enchant_p_def += enchant_def_bonus(tpl.crystal_type, item.enchant_level);
            }
            if enchant_bonus_applies(body_part, declares(Stat::MagicalDefence), true) {
                eq.enchant_m_def += enchant_def_bonus(tpl.crystal_type, item.enchant_level);
            }
        }

        // Weapon-replace stats come from the right-hand slot only (Java
        // `calcWeaponBaseValue`); a two-handed weapon also lives in RHand.
        if let Some(weapon) = inventory.paperdoll_item(PaperdollSlot::RHand)
            && let Some(stats) = data.item_data.item_stats(weapon.item_id)
        {
            for &(stat, val) in &stats.bonuses {
                match stat {
                    Stat::PhysicalAttack => eq.weapon_p_atk = Some(val),
                    Stat::MagicalAttack => eq.weapon_m_atk = Some(val),
                    Stat::PhysicalAttackSpeed => eq.weapon_p_atk_spd = Some(val),
                    Stat::CriticalRate => eq.weapon_crit = Some(val),
                    Stat::MagicCriticalRate => eq.weapon_m_crit = Some(val),
                    _ => {}
                }
            }
            eq.weapon_atk_range = stats.atk_range;
            eq.weapon_random_dmg = stats.random_damage;
        }

        // Sum-add stats are summed across every equipped piece (Java's
        // finalizer paperdoll loop / `calcWeaponPlusBaseValue`). `accCombat`
        // lives on weapons too, so this deliberately includes the weapon.
        for item in inventory.equipped_items() {
            let Some(stats) = data.item_data.item_stats(item.item_id) else {
                continue;
            };
            for &(stat, val) in &stats.bonuses {
                match stat {
                    Stat::PhysicalDefence => eq.p_def += val,
                    Stat::MagicalDefence => eq.m_def += val,
                    Stat::AccuracyCombat => eq.accuracy += val,
                    Stat::AccuracyMagic => eq.magic_accuracy += val,
                    Stat::EvasionRate => eq.evasion += val,
                    Stat::MagicEvasionRate => eq.magic_evasion += val,
                    // maxHp/maxMp item bonuses are folded in by
                    // `calc_max_hp`/`calc_max_mp` themselves
                    // (`equipped_stat_sum`) — adding them here would count
                    // them twice.
                    _ => {}
                }
            }
        }
        eq
    }
}

impl Player {
    /// Java `CreatureStat.recalculateStats` narrowed to the combat stats G6
    /// computes. Re-derives from the class template's base values (not from
    /// `self`, so it's idempotent) × `BaseStat` bonus × level mod, folds in the
    /// equipped gear's `<stats>` contributions, then `stats_add`/`stats_mul`
    /// (buffs). Call after any level/buff/gear change. Gear applies in two
    /// ways, matching the Java finalizers (see [`EquippedBonuses`]): the
    /// weapon's pAtk/mAtk/atk-speed/crit *replace* the naked class base before
    /// the STR/level multipliers; armor/jewel pDef/mDef/accuracy/evasion are
    /// *summed* on top. maxHp/maxMp gear bonuses are **not** missing: they are
    /// computed on a separate path, `calc_max_hp`/`calc_max_mp`, which folds the
    /// same `equipped_stat_sum` for `Stat::MaxHp`/`MaxMp`.
    pub fn recalculate_stats(
        &self,
        data: &GameData,
        base: &BaseStats,
        mods: &StatModifiers,
        inventory: &Inventory,
        speeds: &mut Speeds,
        combat: &mut CombatStats,
    ) {
        let t = data
            .player_templates
            .get_or_base(self.class_id, self.base_class_id)
            .cloned()
            .unwrap_or_default();
        let eq = EquippedBonuses::from_inventory(inventory, data, &t);
        let level_mod = (self.level as f64 + 89.0) / 100.0;
        let sb = &data.stat_bonus;
        let str_bonus = sb.bonus(BaseStat::Str, base.str_);
        let dex_bonus = sb.bonus(BaseStat::Dex, base.dex);
        let int_bonus = sb.bonus(BaseStat::Int, base.int_);
        let wit_bonus = sb.bonus(BaseStat::Wit, base.wit);
        // Java's stat display getters (`getPAtk`/`getPDef`/…) return `(int)
        // getValue()` — a truncation toward zero, *not* a round. The engine
        // stores the finalized double and the packet layer truncates (`as i32`
        // in `user_info`), so nothing here rounds; the `as i32`/`.trunc()`
        // casts below match Java's display exactly.

        // Java `IStatFunction.calcWeaponBaseValue`: a transform's `<base>` block
        // stands in for the equipped weapon — but only for the forms the weapon
        // branch *excludes*. That `else if` fires when the player is
        // untransformed **or** the form is `COMBAT`/`MODE_CHANGE`, and it
        // overwrites whatever the transform contributed; so a COMBAT form keeps
        // swinging its real weapon, and every other form (NON_COMBAT,
        // RIDING_MODE, PURE_STAT, FLYING, CURSED) fights with the template's
        // numbers instead. `None` here means "weapon rules apply as usual".
        //
        // Live on both transforms a player can actually enter on this dist —
        // 105 (NON_COMBAT) and 20008 (RIDING_MODE); see the reachability census
        // in `data::transform_data`.
        let tf_base = (self.transform_id != 0)
            .then(|| data.transforms.get(self.transform_id))
            .flatten()
            .filter(|tf| !tf.kind.weapon_overrides_base())
            .and_then(|tf| tf.template(self.is_female).base.as_ref());
        // Each field falls back to the *class* template value, never to the
        // weapon's: Java's `getStats(stat, baseTemplateValue)` hands back the
        // template default for any key the transform doesn't set.
        let tf_or = |tf: Option<f64>, weapon: Option<f64>, class_base: f64| {
            if tf_base.is_some() { tf } else { weapon }.unwrap_or(class_base)
        };

        // PAttackFinalizer / MAttackFinalizer: the equipped weapon's pAtk/mAtk
        // replaces the naked base (`calcWeaponBaseValue`) before STR/level.
        let p_atk_base = tf_or(
            tf_base.and_then(|b| b.p_atk),
            eq.weapon_p_atk,
            t.base_p_atk as f64,
        );
        let m_atk_base = tf_or(
            tf_base.and_then(|b| b.m_atk),
            eq.weapon_m_atk,
            t.base_m_atk as f64,
        );
        let caps = &data.combat_caps;
        // Every max cap below goes through Java's `validateValue`, which skips
        // the ceiling for creatures with the MAX_STATS_VALUE cond override —
        // granted to GMs on login (Player.restore). Floors still apply.
        let cap = |max: f64| if self.is_gm(data) { f64::MAX } else { max };
        // Java adds `calcEnchantedItemBonus` to the weapon base *before* the
        // stat bonus and level mod, so an enchant on a level-80 character is
        // worth far more than the flat table suggests.
        combat.p_atk = finalize(
            mods,
            Stat::PhysicalAttack,
            (p_atk_base + eq.enchant_p_atk) * str_bonus * level_mod,
        )
        .clamp(0.0, cap(caps.max_p_atk));
        combat.m_atk = finalize(
            mods,
            Stat::MagicalAttack,
            (m_atk_base + eq.enchant_m_atk) * (int_bonus * level_mod).powf(2.2072),
        )
        .clamp(0.0, cap(caps.max_m_atk));

        // P/MDefenseFinalizer: (naked base + summed gear def − the naked defense
        // of every occupied slot) × levelMod (mDef also × MEN bonus), then the
        // `defaultValue` mul(≥0.5)/add and the `base × 0.2` floor.
        let p_def_pre =
            (t.base_p_def as f64 + eq.enchant_p_def + eq.p_def - eq.p_def_slot_sub) * level_mod;
        combat.p_def = finalize_def(
            mods,
            Stat::PhysicalDefence,
            p_def_pre,
            t.base_p_def as f64 * 0.2,
        );
        let men_bonus = if base.men > 0 {
            sb.bonus(BaseStat::Men, base.men)
        } else {
            1.0
        };
        let m_def_pre = (t.base_m_def as f64 + eq.enchant_m_def + eq.m_def - eq.m_def_slot_sub)
            * men_bonus
            * level_mod;
        combat.m_def = finalize_def(
            mods,
            Stat::MagicalDefence,
            m_def_pre,
            t.base_m_def as f64 * 0.2,
        );

        // P/MAttackSpeedFinalizer: weapon replaces base; `mul` floors at 0.7.
        // `<base attackSpeed=…>` feeds `Stat.PHYSICAL_ATTACK_SPEED` only —
        // no transform block sets a magic attack speed, so `m_atk_spd` below
        // keeps the class base under every form.
        let p_atk_spd_base = tf_or(
            tf_base.and_then(|b| b.attack_speed),
            eq.weapon_p_atk_spd,
            t.base_p_atk_spd as f64,
        );
        combat.p_atk_spd =
            finalize_speed(mods, Stat::PhysicalAttackSpeed, p_atk_spd_base * dex_bonus)
                .clamp(1.0, cap(caps.max_p_atk_speed)) as i32;
        combat.m_atk_spd = finalize_speed(
            mods,
            Stat::MagicAttackSpeed,
            t.base_m_atk_spd as f64 * wit_bonus,
        )
        .clamp(1.0, cap(caps.max_m_atk_speed)) as i32;

        // P/MCritRateFinalizer (in per-mille, ×10): weapon replaces base crit.
        // Only the *physical* rate goes through `calcWeaponBaseValue`; Java's
        // `MCritRateFinalizer` uses `calcWeaponPlusBaseValue`, which a
        // transform's `<base>` never contributes a MAGIC_CRITICAL_RATE key to.
        let crit_base = tf_or(
            tf_base.and_then(|b| b.crit_rate),
            eq.weapon_crit,
            t.base_crit_rate as f64,
        );
        let m_crit_base = eq.weapon_m_crit.unwrap_or(t.base_m_crit_rate as f64);
        combat.crit_hit = finalize(mods, Stat::CriticalRate, crit_base * dex_bonus * 10.0)
            .clamp(0.0, cap(caps.max_p_crit_rate));
        combat.m_crit_hit = finalize(
            mods,
            Stat::MagicCriticalRate,
            m_crit_base * wit_bonus * 10.0,
        )
        .clamp(0.0, cap(caps.max_m_crit_rate));

        // P/MAccuracyFinalizer, P/MEvasionRateFinalizer. Gear accuracy/evasion
        // sums add on top (`calcWeaponPlusBaseValue`). `as i32` truncates toward
        // zero, matching Java's `(int)` display getter. The high-level +N steps
        // above level 69 apply only to the *physical* P{Accuracy,EvasionRate}
        // finalizers for players (the M-variants for players have no steps).
        let level = self.level as f64;
        // High-level bonus steps from P{Accuracy,EvasionRate}Finalizer: at lv 80
        // this sums to +12 (11 for >69, +1 for >77).
        let hi_level_step = |lvl: i32| -> f64 {
            let mut b = 0.0;
            if lvl > 69 {
                b += (lvl - 69) as f64;
            }
            if lvl > 77 {
                b += 1.0;
            }
            if lvl > 80 {
                b += 2.0;
            }
            if lvl > 87 {
                b += 2.0;
            }
            if lvl > 92 {
                b += 1.0;
            }
            if lvl > 97 {
                b += 1.0;
            }
            b
        };
        let acc_ev_step = hi_level_step(self.level);
        combat.accuracy = finalize(
            mods,
            Stat::AccuracyCombat,
            (base.dex as f64).sqrt() * 5.0 + level + acc_ev_step + eq.accuracy,
        ) as i32;
        combat.magic_accuracy = finalize(
            mods,
            Stat::AccuracyMagic,
            (base.wit as f64).sqrt() * 3.0 + level * 2.0 + eq.magic_accuracy,
        ) as i32;
        // `PEvasionRateFinalizer` ends on `validateValue(…, Double.NEGATIVE_INFINITY,
        // MAX_EVASION)` — a **ceiling only**. Evasion is allowed to go negative,
        // and 309 skills on this dist carry a `PhysicalEvasion` effect reaching
        // −60, which is more than a low-level character's whole base; flooring
        // it at 0 would hand them evasion they should not have.
        combat.evasion = finalize(
            mods,
            Stat::EvasionRate,
            (base.dex as f64).sqrt() * 5.0 + level + acc_ev_step + eq.evasion,
        )
        .min(cap(caps.max_evasion)) as i32;
        // `MEvasionRateFinalizer` runs the **same** `validateValue` ceiling as its
        // physical twin — `MAX_EVASION` (250 here), which a level-80 caster's
        // `sqrt(WIT)·3 + level·2` base can pass once buffs pile on.
        combat.magic_evasion = finalize(
            mods,
            Stat::MagicEvasionRate,
            (base.wit as f64).sqrt() * 3.0 + level * 2.0 + eq.magic_evasion,
        )
        .min(cap(caps.max_evasion)) as i32;

        // Weapon range / damage spread replace the class template constants
        // while a weapon is equipped (`PRangeFinalizer` / `RandomDamageFinalizer`).
        // `PRangeFinalizer` is a plain `defaultValue(base*mul+add)` finalizer —
        // Archery 431/Long Shot 113/Rapid Fire 413/Snipe 972 (`PhysicalAttackRange`,
        // all `<weaponType>BOW</weaponType>`-conditioned) previously had no stat
        // to land on here at all.
        combat.atk_range = finalize(
            mods,
            Stat::PhysicalAttackRange,
            tf_or(
                tf_base.and_then(|b| b.attack_range),
                eq.weapon_atk_range.map(|r| r as f64),
                t.base_atk_range as f64,
            ),
        ) as i32;
        // `RandomDamageFinalizer` is `calcWeaponBaseValue` too, so a
        // transform's `randomDamage` replaces the weapon's spread the same way.
        // The bare `10` is the class-template stand-in Java reads from the
        // player template's own RANDOM_DAMAGE default.
        combat.random_dmg = tf_or(
            tf_base.and_then(|b| b.random_damage),
            eq.weapon_random_dmg.map(|d| d as f64),
            10.0,
        ) as i32;
        // `ShotsBonusFinalizer`. Nothing on this dist declares a `shotBonus`
        // stat modifier, so `Stat.defaultValue`'s mul/add pair is the identity
        // and the weapon enchant is the whole of it.
        combat.shots_bonus_add = eq.shots_bonus - 1.0;

        // SpeedFinalizer: every player speed stat gets `Config.RUN_SPD_BOOST`
        // added in `getBaseSpeed` (35 on this dist — see `CombatCaps`).
        // Buffs (Speed effect) apply through the add/mul maps like the combat
        // stats above; stored as f64 (Speeds is shared with NPCs, whose
        // templates don't take the player boost). The `as i16` in `user_info`
        // truncates for display, matching Java's `(int)` getter.
        // `SpeedFinalizer.getBaseSpeed`: a mounted player's base speeds are the
        // mount's `speed_on_ride` row (looked up at the *mount's* level),
        // halved when the mount is 10+ levels above the rider — the class
        // template only stands in when the species has no row (Java gets null
        // back and keeps `calcWeaponPlusBaseValue`).
        //
        // Java halves again on `player.isHungry()`, which is **inert** for a
        // rider — the predicate requires `hasPet()` and `mount()` unsummons the
        // pet a line after starting the feed, so it can never be true. See
        // `game_loop::admin::mounts::is_hungry`; omitted here deliberately
        // rather than "not ported".
        let ride = if self.is_mounted() {
            data.pet_data
                .get(self.mount_npc_id)
                .and_then(|pet| pet.level_row(self.mount_level))
        } else {
            None
        };
        let level_gap_penalty = if self.mount_level - self.level >= 10 {
            0.5
        } else {
            1.0
        };
        let base_speed = |ride_spd: Option<f64>, class_base: f64| {
            ride_spd.map_or(class_base, |s| s * level_gap_penalty) + caps.run_spd_boost
        };
        speeds.run_spd = finalize(
            mods,
            Stat::RunSpeed,
            base_speed(ride.map(|r| r.ride_run_spd), t.base_run_spd as f64),
        );
        speeds.walk_spd = finalize(
            mods,
            Stat::WalkSpeed,
            base_speed(ride.map(|r| r.ride_walk_spd), t.base_walk_spd as f64),
        );
        speeds.swim_run_spd = finalize(
            mods,
            Stat::SwimRunSpeed,
            base_speed(
                ride.map(|r| r.ride_fast_swim_spd),
                t.base_swim_run_spd as f64,
            ),
        );
        speeds.swim_walk_spd = finalize(
            mods,
            Stat::SwimWalkSpeed,
            base_speed(
                ride.map(|r| r.ride_slow_swim_spd),
                t.base_swim_walk_spd as f64,
            ),
        );

        // A transform replaces the class base run/walk with the template's
        // `<moving>` values (Java's transform move-speed override), still folding
        // the buff modifiers on top. Absolute template speeds — the class
        // `RUN_SPD_BOOST` is not re-added (the transform values are self-tuned).
        if self.transform_id != 0
            && let Some(tf) = data.transforms.get(self.transform_id)
        {
            let tmpl = tf.template(self.is_female);
            if let Some(run) = tmpl.run_spd {
                speeds.run_spd = finalize(mods, Stat::RunSpeed, run);
            }
            if let Some(walk) = tmpl.walk_spd {
                speeds.walk_spd = finalize(mods, Stat::WalkSpeed, walk);
            }
        }

        // `SpeedFinalizer`: a playable inside a `SwampZone` has every speed
        // scaled, after the boost and before the clamp.
        if speeds.swamp_multiplier != 1.0 {
            let m = speeds.swamp_multiplier;
            speeds.run_spd *= m;
            speeds.walk_spd *= m;
            speeds.swim_run_spd *= m;
            speeds.swim_walk_spd *= m;
        }

        // SpeedFinalizer's `validateValue`: players clamp to [1, MaxRunSpeed]
        // (300 on this dist).
        let speed_cap = cap(caps.max_run_speed);
        for spd in [
            &mut speeds.run_spd,
            &mut speeds.walk_spd,
            &mut speeds.swim_run_spd,
            &mut speeds.swim_walk_spd,
        ] {
            *spd = spd.clamp(1.0, speed_cap);
        }
    }
}
