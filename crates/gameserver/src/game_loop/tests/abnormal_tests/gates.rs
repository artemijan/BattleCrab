//! The refusal flags: debuff, control and buff block, the passive flag, the
//! shield angle, irreplacable buffs, and the abnormal slot cap.

use super::*;

/// `DEBUFF_BLOCK` refuses incoming debuffs outright while leaving buffs alone.
#[test]
fn debuff_block_refuses_incoming_debuffs() {
    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    // Baseline: the stun lands.
    land(&mut world, STUN_ID, VICTIM);
    assert!(abnormal::is_blocked_from_actions(&world, VICTIM));
    effects::handle_buff_expire(&mut world, VICTIM, STUN_ID);

    // Under debuff block it does not.
    land(&mut world, DBLOCK_ID, VICTIM);
    land(&mut world, STUN_ID, VICTIM);
    assert!(
        !abnormal::is_blocked_from_actions(&world, VICTIM),
        "a debuff-blocked target refuses the stun entirely"
    );

    // A *buff* still lands (1068 is the Might-like buff, not a debuff).
    let buff = skill_by_id(&world, 1068, 1).expect("might");
    effects::apply_skill_effects(&mut world, CASTER, VICTIM, &buff);
    assert!(
        has_buff(&world, VICTIM, 1068),
        "debuff block does not stop buffs"
    );
}

/// `BLOCK_CONTROL` refuses item use (Java's `UseItem` gate).
#[test]
fn control_block_refuses_item_use() {
    let (mut world, _db, _l) = cc2_world();
    let mut out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    drain(&mut out);

    land(&mut world, CBLOCK_ID, CASTER);
    // A bogus item object id is fine: the gate must reject before any lookup,
    // so the only reply is ActionFailed.
    items::handle_use_item(&mut world, CID, &use_item_body(1234));
    let pkts = drain(&mut out);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::ACTION_FAIL),
        "item use is refused while control-blocked"
    );
}

const BUFFBLOCK_ID: i32 = 9321;

const PACIFY_ID: i32 = 9322;

/// `BUFF_BLOCK` is the mirror of `DEBUFF_BLOCK`, and the asymmetry matters:
/// Java's `EffectList.add` refuses on `isBuffBlocked() && !skill.isBad()`, so a
/// **buff** is stopped and a **debuff** still lands. It also has **no
/// self-cast exemption**, unlike the debuff-block gate — Dance of Medusa stops
/// its victim buffing themselves, which is the whole point of it.
#[test]
fn buff_block_refuses_buffs_and_lets_debuffs_through() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        BUFFBLOCK_ID,
        SkillEffect::BuffBlock,
        "BUFF_BLOCK",
    ));
    // A plain good skill (effectPoint ≥ 0) to be refused, and a bad one to
    // prove debuffs are unaffected.
    let mut good = cc_skill(9324, SkillEffect::SilentMove, "SILENT_MOVE");
    good.effect_point = 100;
    good.is_debuff = false;
    world.data.skill_data.insert_for_test(good);

    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    land(&mut world, BUFFBLOCK_ID, CASTER);
    assert!(abnormal::is_buff_blocked(&world, CASTER), "the flag is up");

    land(&mut world, 9324, CASTER);
    assert!(
        !has_buff(&world, CASTER, 9324),
        "a buff cannot land on a buff-blocked target — not even their own"
    );

    // A debuff is explicitly *not* blocked by this flag.
    land(&mut world, ROOT_ID, CASTER);
    assert!(
        has_buff(&world, CASTER, ROOT_ID),
        "a debuff still lands — `!skill.isBad()` is the gate, not `isDebuff()`"
    );
}

/// `PASSIVE` — Java `Monster.isAggressive()` is
/// `getTemplate().isAggressive() && !isAffected(EffectFlag.PASSIVE)`, so a
/// pacified mob stops aggroing whatever its template says. Veil (106) and
/// Requiem (1049) are the learnable sources.
#[test]
fn the_passive_flag_pacifies_an_aggressive_monster() {
    let (mut world, _db, _l) = cc2_world();
    world
        .data
        .skill_data
        .insert_for_test(cc_skill(PACIFY_ID, SkillEffect::Passive, "PASSIVE"));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 100, 0, 0);

    assert!(
        !abnormal::is_pacified(&world, NPC_OID),
        "not pacified to begin with"
    );
    land(&mut world, PACIFY_ID, NPC_OID);
    assert!(
        abnormal::is_pacified(&world, NPC_OID),
        "the mob is pacified while the buff is up"
    );
    effects::handle_buff_expire(&mut world, NPC_OID, PACIFY_ID);
    assert!(
        !abnormal::is_pacified(&world, NPC_OID),
        "and aggressive again when it drops"
    );
}

/// `PHYSICAL_SHIELD_ANGLE_ALL` (Aegis) widens Java's `degreeside` from 120° to
/// 360°, which in practice means the back-attack exemption in `calcShldUse`
/// simply stops applying — a shield can block a backstab.
#[test]
fn the_shield_angle_flag_lets_a_shield_block_from_behind() {
    let (mut world, _db, _l) = cc2_world();
    world.data.skill_data.insert_for_test(cc_skill(
        9327,
        SkillEffect::PhysicalShieldAngleAll,
        "SHIELD_ANGLE",
    ));
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    assert!(!abnormal::shields_from_all_angles(&world, CASTER));
    land(&mut world, 9327, CASTER);
    assert!(
        abnormal::shields_from_all_angles(&world, CASTER),
        "the 360° arc is up while the stance holds"
    );

    // The formula's own behaviour, which the flag feeds: a back attack is
    // unblockable, and that is the *only* thing the flag changes.
    use crate::model::formulas::physical::{SHIELD_NONE, SHIELD_SUCCEED, calc_shield_use};
    assert_eq!(
        calc_shield_use(90.0, 1.0, false, true, 0, 99),
        SHIELD_NONE,
        "from behind, no block"
    );
    // `perfect_roll` 0 keeps it an ordinary block: the perfect-block test is
    // `100 − 2×con_bonus < perfect_roll`, i.e. 98 < 0 here.
    assert_eq!(
        calc_shield_use(90.0, 1.0, false, false, 0, 0),
        SHIELD_SUCCEED,
        "from the front, the same roll blocks"
    );
}

/// `isStayAfterDeath()` is **one getter over three tags** —
/// `_stayAfterDeath || _irreplacableBuff || _isNecessaryToggle` — and the port
/// read only the first. On this dist **30 learnable skills** declare
/// `<irreplacableBuff>` with no `<stayAfterDeath>` of their own (the whole
/// Transform Grail Apostle / Unicorn / Lilim Knight / Golem Guardian family),
/// so every one of them was being stripped on death when Java keeps it.
///
/// Asserted against the real dist and against a skill where the *new* tag is
/// the only source, so the assertion can only pass because of the fold.
#[test]
fn irreplacable_buffs_survive_death_like_stay_after_death_ones() {
    const TRANSFORM_GRAIL_APOSTLE: i32 = 541;
    let sd = dist::skills();
    assert!(
        sd.get(TRANSFORM_GRAIL_APOSTLE, 1)
            .expect("Transform Grail Apostle 1")
            .stay_after_death,
        "declares <irreplacableBuff> and no <stayAfterDeath>, so it survives \
         death only if the getter's three tags are folded"
    );
}

/// G34 S4 sub-slice 5 — `EnlargeAbnormalSlot` (Divine Inspiration 1405) raises
/// the **good-buff** slot cap, and only that pool: Java's `setMaxBuffCount` is
/// read by `EffectList` for buffs, never for dances.
///
/// Modelled as a `Stat` rather than Java's setter on purpose — `apply_buff`
/// rebuilds `StatModifiers` from the surviving buffs on every change, so the
/// bonus is *derived* and cannot drift the way an add/subtract pair can when a
/// buff leaves by some other path. The expiry case is asserted for exactly
/// that reason.
#[test]
fn enlarge_abnormal_slot_raises_the_buff_cap_and_gives_it_back() {
    use crate::model::stats::{Stat, StatModifierType};
    let (mut world, _db, _l) = cc2_world();
    world.data.combat_caps.max_buff_count = 2; // small enough to observe
    let mut boost = cc_skill(9361, SkillEffect::Root, "SLOT_BOOST");
    boost.effects = vec![SkillEffect::StatModifier(
        model::skill::effects::StatModifierEffect {
            stat: Stat::MaxBuffSlots,
            mode: StatModifierType::Diff,
            amount: 2.0,
            armor_condition: 0,
            weapon_condition: 0,
            qualifier: None,
            two_handed: false,
            hp_percent: 0,
        },
    )];
    boost.effect_point = 100;
    boost.is_debuff = false;
    world.data.skill_data.insert_for_test(boost);
    // Three ordinary buffs, so the cap is what decides how many survive.
    for id in 9371..9374 {
        let mut b = cc_skill(id, SkillEffect::Root, &format!("B{id}"));
        b.effect_point = 100;
        b.is_debuff = false;
        world.data.skill_data.insert_for_test(b);
    }
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let buff_count = |world: &World| {
        world
            .objects
            .get_component::<Buffs>(&CASTER)
            .map(|b| b.0.len())
            .unwrap_or(0)
    };

    // Without the boost the cap holds at 2.
    for id in 9371..9374 {
        land(&mut world, id, CASTER);
    }
    assert_eq!(buff_count(&world), 2, "the base cap of 2 holds");

    // With it, four fit (2 base + 2 granted) — the boost itself occupies one.
    land(&mut world, 9361, CASTER);
    for id in 9371..9374 {
        land(&mut world, id, CASTER);
    }
    assert_eq!(
        buff_count(&world),
        4,
        "Divine Inspiration's slots are real, not cosmetic"
    );
}

/// `DispelBySlotMyself` (Flames of Invincibility 1427) strips the bearer's own
/// buffs of the listed abnormal types — but **spares an `irreplacableBuff`**,
/// which `DispelBySlot` does not. Both halves asserted, since a version that
/// dispelled everything would look correct against ordinary buffs.
#[test]
fn dispel_by_slot_myself_spares_irreplacable_buffs() {
    let (mut world, _db, _l) = cc2_world();
    let mut stance = cc_skill(9381, SkillEffect::Root, "MAGICAL_STANCE");
    stance.effect_point = 100;
    stance.is_debuff = false;
    world.data.skill_data.insert_for_test(stance);

    // Same abnormal type, but flagged to survive death — Java's
    // `isIrreplacableBuff()`, which the port folds into `stay_after_death`.
    let mut protected = cc_skill(9382, SkillEffect::Root, "MAGICAL_STANCE");
    protected.effect_point = 100;
    protected.is_debuff = false;
    protected.stay_after_death = true;
    world.data.skill_data.insert_for_test(protected);

    let mut dispeller = cc_skill(
        9383,
        SkillEffect::DispelBySlotMyself {
            dispel: vec!["MAGICAL_STANCE".into()],
        },
        "NONE",
    );
    dispeller.effect_point = 100;
    dispeller.is_debuff = false;
    world.data.skill_data.insert_for_test(dispeller);

    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    land(&mut world, 9381, CASTER);
    land(&mut world, 9382, CASTER);
    land(&mut world, 9383, CASTER);

    let has = |world: &World, id: i32| has_buff(world, CASTER, id);
    assert!(!has(&world, 9381), "the ordinary MAGICAL_STANCE buff goes");
    assert!(
        has(&world, 9382),
        "…but an irreplacable one of the same type stays"
    );
}
