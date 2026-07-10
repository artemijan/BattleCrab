//! Port of `gameserver/model` — the game domain. G4 introduces the composed
//! `Player` (challenge #1: composition over inheritance) with just enough state
//! to enter the world and display correctly. Inventory, skills, effects, and the
//! full stat pipeline arrive in later milestones.

pub mod inventory;

use crate::character::CharData;
use crate::data::player_template::PlayerTemplate;
use crate::data::GameData;
use inventory::Inventory;

/// A player character in (or entering) the world. Owned by the `World` object
/// registry once in game; the `InGame` session links to it by `object_id`.
#[derive(Debug, Clone)]
pub struct Player {
    pub object_id: i32,
    pub name: String,
    pub account: String,
    pub title: String,

    pub level: i32,
    pub class_id: i32,
    pub base_class_id: i32,
    pub race: i32,
    pub is_female: bool,

    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,

    // Base primary stats (TODO(G7): + henna/items/buffs).
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int_: i32,
    pub wit: i32,
    pub men: i32,

    pub max_hp: i32,
    pub cur_hp: f64,
    pub max_mp: i32,
    pub cur_mp: f64,
    pub max_cp: i32,
    pub cur_cp: f64,

    pub exp: i64,
    pub sp: i64,
    pub reputation: i32,
    pub pk_kills: i32,
    pub pvp_kills: i32,
    pub vitality_points: i32,
    pub fame: i32,

    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,

    // Combat stats — base template values for now (TODO(G7): full stat calc).
    pub p_atk: i32,
    pub p_atk_spd: i32,
    pub p_def: i32,
    pub m_atk: i32,
    pub m_atk_spd: i32,
    pub m_def: i32,
    pub crit_hit: i32,
    pub m_crit_hit: i32,
    pub evasion: i32,
    pub accuracy: i32,
    pub magic_evasion: i32,
    pub magic_accuracy: i32,
    pub atk_range: i32,

    // Movement (pre-multiplier) + collision.
    pub run_spd: i32,
    pub walk_spd: i32,
    pub swim_run_spd: i32,
    pub swim_walk_spd: i32,
    pub move_multiplier: f64,
    pub collision_radius: f64,
    pub collision_height: f64,
    pub running: bool,

    pub inventory: Inventory,
}

impl Player {
    /// Build a `Player` from a stored character row + its class template.
    /// Max HP/MP/CP are recomputed (not read from the DB) so they display
    /// correctly; current HP/MP come from the row, clamped to the max.
    pub fn from_char(data: &GameData, c: &CharData) -> Self {
        // The active class's template (base classes only in G4).
        let t = data
            .player_templates
            .get(c.class_id)
            .or_else(|| data.player_templates.get(c.base_class_id))
            .cloned()
            .unwrap_or_default();

        let max_hp = calc_max_hp(data, &t, c.level);
        let max_mp = calc_max_mp(data, &t, c.level);
        let max_cp = calc_max_cp(data, &t, c.level);

        Player {
            object_id: c.object_id,
            name: c.name.clone(),
            account: c.account_name.clone(),
            title: String::new(),
            level: c.level,
            class_id: c.class_id,
            base_class_id: c.base_class_id,
            race: c.race,
            is_female: c.sex != 0,
            x: c.x,
            y: c.y,
            z: c.z,
            heading: 0,
            str_: t.base_str,
            dex: t.base_dex,
            con: t.base_con,
            int_: t.base_int,
            wit: t.base_wit,
            men: t.base_men,
            max_hp: max_hp as i32,
            cur_hp: c.cur_hp.min(max_hp),
            max_mp: max_mp as i32,
            cur_mp: c.cur_mp.min(max_mp),
            max_cp: max_cp as i32,
            cur_cp: 0.0,
            exp: c.exp,
            sp: c.sp,
            reputation: c.reputation,
            pk_kills: c.pk_kills,
            pvp_kills: c.pvp_kills,
            vitality_points: c.vitality_points,
            fame: 0,
            face: c.face,
            hair_style: c.hair_style,
            hair_color: c.hair_color,
            // TODO(G7): full combat-stat calc (STR/DEX bonuses, weapon, items).
            p_atk: t.base_p_atk,
            p_atk_spd: t.base_p_atk_spd,
            p_def: t.base_p_def,
            m_atk: t.base_m_atk,
            m_atk_spd: t.base_m_atk_spd,
            m_def: t.base_m_def,
            crit_hit: t.base_crit_rate,
            m_crit_hit: t.base_m_crit_rate,
            evasion: 0,
            accuracy: 0,
            magic_evasion: 0,
            magic_accuracy: 0,
            atk_range: t.base_atk_range,
            run_spd: t.base_run_spd,
            walk_spd: t.base_walk_spd,
            swim_run_spd: t.base_swim_run_spd,
            swim_walk_spd: t.base_swim_walk_spd,
            move_multiplier: 1.0,
            collision_radius: t.collision_radius,
            collision_height: t.collision_height,
            running: true,
            inventory: Inventory::from_rows(&c.items),
        }
    }

    /// Fraction of the way through the current level (for XP-bar display).
    pub fn exp_percent(&self, data: &GameData) -> f64 {
        let base = data.experience.exp_for_level(self.level);
        let next = data.experience.exp_for_level(self.level + 1);
        if next - base <= 0 {
            0.0
        } else {
            (self.exp - base) as f64 / (next - base) as f64
        }
    }
}

/// `MaxHpFinalizer`: `baseHpMax(level) * CON bonus`.
/// TODO(G7): the multiplicative/additive item & buff modifiers (`mul`/`add`).
pub fn calc_max_hp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_hp_max(level) * data.stat_bonus.con_bonus(t.base_con)
}

/// `MaxMpFinalizer`: `baseMpMax(level) * MEN bonus`.
pub fn calc_max_mp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_mp_max(level) * data.stat_bonus.men_bonus(t.base_men)
}

/// `MaxCpFinalizer`: `baseCpMax(level) * CON bonus`.
pub fn calc_max_cp(data: &GameData, t: &PlayerTemplate, level: i32) -> f64 {
    t.base_cp_max(level) * data.stat_bonus.con_bonus(t.base_con)
}
