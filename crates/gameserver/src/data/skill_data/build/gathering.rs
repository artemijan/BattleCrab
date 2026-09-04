//! Spoil, sweep, sow and harvest.

use super::effect::Cx;
use crate::model::skill;
use crate::model::stats::Stat;

pub(super) fn build(cx: &Cx<'_>) -> Option<Vec<skill::effects::SkillEffect>> {
    let &Cx {
        xml_name,
        params: _,
        mode,
        groups,
        armor_condition,
        weapon_condition,
        values,
        level: _,
        modifier_mode,
        hp_percent,
    } = cx;
    let _ = (
        mode,
        groups,
        armor_condition,
        weapon_condition,
        values,
        modifier_mode,
        hp_percent,
    );
    let _param = |key: &str| cx.param(key);
    let _stat_mod = |stat: Stat, amount: f64| cx.stat_mod(stat, amount);

    Some(match xml_name.as_str() {
        // Spoil (254/…): mark the mob spoiled. No params — the
        // landing roll and target checks live in the effect handler.
        "Spoil" => vec![skill::effects::SkillEffect::Spoil],
        // Sweeper (42/474): claim the dead mob's spoil loot.
        "Sweeper" => vec![skill::effects::SkillEffect::Sweeper],
        // ConsumeBody (paired with Sweeper on 42): decay the corpse.
        "ConsumeBody" => vec![skill::effects::SkillEffect::ConsumeBody],
        // Sow (2097): the manor sow, cast via a Seed item.
        "Sow" => vec![skill::effects::SkillEffect::Sow],
        // Harvesting (2098): claim a sown corpse's crop.
        "Harvesting" => vec![skill::effects::SkillEffect::Harvesting],
        _ => return None,
    })
}
