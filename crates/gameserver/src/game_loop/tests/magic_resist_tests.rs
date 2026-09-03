//! `ResistDDMagic` → `Stat::MAGIC_SUCCESS_RES` (G19).
//!
//! Anti Magic 146 and M. Def. 147 are mage-defence passives that make incoming
//! spells more likely to be resisted. The port fixed Java's `resModifier` at
//! 1.0 on the grounds that the only *items* touching `magicSuccRes` declare it
//! additively — which overlooked that **skills** grant it multiplicatively,
//! which is exactly what `getMul` reads.

use super::*;

use crate::model::formulas::magic::{MagicSuccess, calc_magic_success_rate};
use crate::model::stats::Stat;

const DIST: &str = crate::data::DIST_GAME;

/// A PvP-branch input with a known failure term, so the resist multiplier's
/// effect is arithmetic rather than incidental.
fn pvp_input(res_modifier: f64) -> MagicSuccess<'static> {
    MagicSuccess {
        pve: false,
        target_level: 40,
        effective_level: 40,
        caster_player_level: Some(40),
        target_is_raid: false,
        min_npc_level_for_magic_penalty: 78,
        skill_chance_penalty: &[],
        // The step table bands on `magic_accuracy - magic_evasion`:
        //   > -20 → 2, > -25 → 30, > -30 → 60, > -35 → 90, else 100.
        // A deficit of **26** lands in the 60 band (-26 > -30). Picking -31
        // instead lands in the 90 band, which is what tripped my first draft.
        magic_accuracy: 0,
        magic_evasion: 26,
        res_modifier,
    }
}

/// Identity leaves the old behaviour exactly as it was — the guarantee that
/// keeps the existing magic-success tests meaningful.
#[test]
fn an_identity_res_modifier_changes_nothing() {
    assert_eq!(calc_magic_success_rate(&pvp_input(1.0)), 100 - 60);
}

/// The modifier scales the **failure** term, so raising it *lowers* the
/// attacker's success rate. Getting the direction backwards would turn a
/// defensive passive into an offensive one.
#[test]
fn a_higher_res_modifier_lowers_the_success_rate() {
    let base = calc_magic_success_rate(&pvp_input(1.0));
    let resisted = calc_magic_success_rate(&pvp_input(1.5));
    assert!(
        resisted < base,
        "more resistance means less success: {base} -> {resisted}"
    );
    // 60 * 1.5 = 90, so the rate is 100 - 90 = 10.
    assert_eq!(resisted, 10);
}

/// And a modifier below 1 makes the caster *more* likely to land — the mirror
/// case, worth pinning so the multiplication isn't quietly clamped.
///
/// Note the rate is deliberately unclamped at both ends (Java's own comment:
/// it "may fall below 0, meaning always fails"), so a large enough resist can
/// drive it negative rather than to zero.
#[test]
fn a_lower_res_modifier_raises_the_success_rate() {
    assert_eq!(calc_magic_success_rate(&pvp_input(0.5)), 100 - 30);
}

/// Both carriers parse to the `PER` stat. Their level-1 amounts are 0 — like
/// Rage and Resurrection, these skills only start doing something at a higher
/// level, so a level-1 assertion would prove nothing.
#[test]
fn real_dist_carriers_parse() {
    assert_eq!(
        stat_value_of(146, 1, Stat::MagicSuccessRes),
        Some(0.0),
        "Anti Magic does nothing at level 1"
    );
    assert_eq!(
        stat_value_of(146, 3, Stat::MagicSuccessRes),
        Some(5.0),
        "and 5% from level 3"
    );
    assert!(
        stat_value_of(147, 1, Stat::MagicSuccessRes).is_some(),
        "M. Def. carries the stat too"
    );
}

/// Learned as a passive, Anti Magic folds a `>1.0` multiplier — which is what
/// `calc_magic_success` reads off the *defender*.
#[test]
fn anti_magic_folds_a_raising_multiplier() {
    let (link_tx, _link_rx) = tokio::sync::mpsc::unbounded_channel();
    let (db_tx, _db_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut data = GameData::for_test();
    data.player_templates = dist::player_templates_owned();
    data.stat_bonus = crate::data::stat_bonus::StatBonus::load_from(DIST);
    data.item_data = dist::items_owned();
    data.skill_data = dist::skills_owned();
    let world = World::new(link_tx, 7, 3, 0, data, db_tx);

    let bare = Player::from_char(&world.data, &dummy_char(9801, "Bare"));
    assert_eq!(
        bare.stat_modifiers.mul.get(&Stat::MagicSuccessRes),
        None,
        "no skill, no modifier"
    );

    let mut chr = dummy_char(9802, "Warded");
    chr.skills = vec![(146, 3, 0)]; // level 3 — the first with a non-zero amount
    let bundle = Player::from_char(&world.data, &chr);
    let mul = bundle
        .stat_modifiers
        .mul
        .get(&Stat::MagicSuccessRes)
        .copied()
        .unwrap_or(1.0);
    assert!(
        (mul - 1.05).abs() < 1e-9,
        "+5% PER folds to x1.05, got {mul}"
    );
    assert!(mul > 1.0, "and it raises the failure term, i.e. defends");
}
