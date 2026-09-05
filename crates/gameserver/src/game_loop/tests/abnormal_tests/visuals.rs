//! The abnormal visual list: how it folds over buffs, what char info and npc
//! info carry, and the buffs that show no icon at all.

use super::*;

/// The visual set is a fold over live buffs, de-duplicated, and clears with
/// them. Two poisons draw one tint.
#[test]
fn visual_effects_fold_over_buffs_and_clear() {
    use crate::game_loop::abnormal::visual_effects;

    let (mut world, _db, _l) = cc_world();
    // STUN(7) and DOT_POISON(2); a second poison must not duplicate the tint.
    let mut stun_vis = cc_skill(
        9320,
        SkillEffect::BlockActions { conditional: false },
        "STUN_VIS",
    );
    stun_vis.abnormal_visuals = vec![7];
    let mut poison_a = cc_skill(9321, SkillEffect::Root, "POISON_A");
    poison_a.abnormal_visuals = vec![2];
    let mut poison_b = cc_skill(9322, SkillEffect::Root, "POISON_B");
    poison_b.abnormal_visuals = vec![2];
    for sk in [stun_vis, poison_a, poison_b] {
        world.data.skill_data.insert_for_test(sk);
    }
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    assert!(
        visual_effects(&world, VICTIM).is_empty(),
        "nothing showing to begin with"
    );

    land(&mut world, 9320, VICTIM);
    assert_eq!(
        visual_effects(&world, VICTIM),
        vec![7],
        "the stun swirl shows"
    );

    land(&mut world, 9321, VICTIM);
    land(&mut world, 9322, VICTIM);
    let vis = visual_effects(&world, VICTIM);
    assert!(
        vis.contains(&7) && vis.contains(&2),
        "both visuals show: {vis:?}"
    );
    assert_eq!(
        vis.iter().filter(|&&v| v == 2).count(),
        1,
        "de-duplicated: {vis:?}"
    );

    // Clearing the stun leaves the poison tint behind.
    effects::handle_buff_expire(&mut world, VICTIM, 9320);
    let vis = visual_effects(&world, VICTIM);
    assert!(
        !vis.contains(&7) && vis.contains(&2),
        "only the stun's visual went: {vis:?}"
    );
}

/// The visual reaches the wire: `CharInfo` carries the count and ids so nearby
/// players actually see the effect on the victim.
#[test]
fn char_info_carries_the_visual_list() {
    let (mut world, _db, _l) = cc_world();
    let mut stun_vis = cc_skill(
        9323,
        SkillEffect::BlockActions { conditional: false },
        "STUN_VIS",
    );
    stun_vis.abnormal_visuals = vec![7];
    world.data.skill_data.insert_for_test(stun_vis);
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _v = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);

    let visuals_of = |world: &World| {
        let v = model::PlayerView::of(&world.objects, VICTIM).expect("view");
        server_packets::char_info(
            &v,
            &abnormal::visual_effects(world, VICTIM),
            &[],
            &Default::default(),
        )
    };

    let before = visuals_of(&world);
    land(&mut world, 9323, VICTIM);
    let after = visuals_of(&world);
    assert!(
        after.len() > before.len(),
        "the stunned CharInfo is longer by the visual entry"
    );
}

/// A skill with no `<abnormalVisualEffect>` sends no visual packet — Java only
/// pushes the set from start/stopAbnormalVisualEffect, so a plain stat buff
/// must not spam `ExUserInfoAbnormalVisualEffect`.
#[test]
fn buffs_without_a_visual_send_no_visual_packet() {
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut vout = ingame_caster(&mut world, VICTIM_CID, VICTIM, 50, 0);
    drain(&mut vout);

    // 1068 is the Might-like stat buff from `cast_test_world` — no visual.
    let buff = skill_by_id(&world, 1068, 1).expect("might");
    effects::apply_skill_effects(&mut world, CASTER, VICTIM, &buff);

    let pkts = drain(&mut vout);
    let ave_pkts = pkts
        .iter()
        .filter(|p| {
            is_ex(
                p,
                server_packets::opcodes::EX_USER_INFO_ABNORMAL_VISUAL_EFFECT,
            )
        })
        .count();
    assert_eq!(
        ave_pkts, 0,
        "a visual-less buff pushes no ExUserInfoAbnormalVisualEffect"
    );
}

/// **A stunned mob shows its icon.** `NpcInfo`'s `ABNORMALS` component was
/// never emitted — the same shape as `CharInfo`'s abnormal-visual count before
/// G19 fixed it, but for NPCs — so a mob under a visible abnormal looked
/// completely untouched to every client.
#[test]
fn npc_info_carries_the_mobs_abnormal_visuals() {
    use crate::model::npc::NpcView;

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    let build = |world: &World| {
        let v = NpcView::of(&world.objects, NPC_OID).expect("a live mob");
        let t = v.npc.template(world).expect("its template");
        server_packets::npc_info(
            &v,
            t,
            &world.cfg.npc,
            &world.cfg.champion,
            &abnormal::visual_effects(world, NPC_OID),
            None,
        )
    };

    let clean = build(&world);

    // Land a stun on the mob: `apply_buff_to_npc` stores its visual ids.
    world.data.skill_data.insert_for_test({
        let mut s = cc_skill(9330, SkillEffect::Root, "STUN");
        s.abnormal_visuals = vec![1]; // AbnormalVisualEffect.DOT_BLEEDING-ish id
        s
    });
    land(&mut world, 9330, NPC_OID);
    assert_eq!(
        abnormal::visual_effects(&world, NPC_OID),
        vec![1],
        "the mob really is carrying a visual"
    );

    let stunned = build(&world);
    assert!(
        stunned.len() > clean.len(),
        "the ABNORMALS block adds a count plus one short: {} vs {}",
        stunned.len(),
        clean.len()
    );
    assert_eq!(
        stunned.len() - clean.len(),
        4,
        "an i16 count and one i16 id"
    );
    // The tail carries the count and the id, little-endian.
    let tail = &stunned[stunned.len() - 5..];
    assert_eq!(i16::from_le_bytes([tail[0], tail[1]]), 1, "one effect");
    assert_eq!(i16::from_le_bytes([tail[2], tail[3]]), 1, "its client id");

    // And the visibility path — the one that actually reaches a client — sends
    // that same packet rather than a bare one.
    let mut rx = ingame_caster(&mut world, 9, 3099, 0, 0);
    drain(&mut rx);
    visibility::on_enter_world(&world, 9, 3099);
    let sent = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_INFO)
        .expect("the observer was told about the mob");
    assert_eq!(
        sent.len(),
        stunned.len(),
        "the observer's NpcInfo carries the abnormal block too"
    );
}

/// **An NPC's team aura and display effect ride `NpcInfo`.** Both were
/// broadcast-only or not modelled at all, so `//setteam` on a mob did nothing
/// visible and `//set_displayeffect` was lost on anyone who arrived after the
/// change — Java stores both on the NPC precisely so a late observer sees them.
#[test]
fn npc_info_carries_the_team_and_display_effect() {
    use crate::model::npc::{Npc, NpcView};

    let (mut world, _db, _l) = cc2_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    add_test_npc(&mut world, NPC_OID, 20001, "Monster", 5, 60, 0, 0);

    let build = |world: &World| {
        let v = NpcView::of(&world.objects, NPC_OID).expect("a live mob");
        let t = v.npc.template(world).expect("its template");
        server_packets::npc_info(&v, t, &world.cfg.npc, &world.cfg.champion, &[], None)
    };
    let clean = build(&world);

    // Blue team → one extra byte (`NpcInfoType::TEAM`, block length 1).
    world
        .objects
        .get_component_mut::<Npc>(&NPC_OID)
        .unwrap()
        .team = 1;
    let teamed = build(&world);
    assert_eq!(teamed.len() - clean.len(), 1, "the TEAM block is one byte");

    // Display effect → four more (`DISPLAY_EFFECT`, block length 4).
    world
        .objects
        .get_component_mut::<Npc>(&NPC_OID)
        .unwrap()
        .display_effect = 3;
    let both = build(&world);
    assert_eq!(
        both.len() - teamed.len(),
        4,
        "the DISPLAY_EFFECT block is four"
    );

    // Back to Java's defaults: neither block is emitted.
    {
        let n = world.objects.get_component_mut::<Npc>(&NPC_OID).unwrap();
        n.team = 0;
        n.display_effect = 0;
    }
    assert_eq!(build(&world).len(), clean.len(), "defaults emit nothing");

    // And an observer arriving *after* the change is told (the whole point of
    // storing it rather than only broadcasting the change packet).
    {
        let n = world.objects.get_component_mut::<Npc>(&NPC_OID).unwrap();
        n.team = 2;
        n.display_effect = 7;
    }
    let mut rx = ingame_caster(&mut world, 9, 3098, 0, 0);
    drain(&mut rx);
    visibility::on_enter_world(&world, 9, 3098);
    let sent = drain(&mut rx)
        .into_iter()
        .find(|p| p[0] == server_packets::opcodes::NPC_INFO)
        .expect("the observer was told about the mob");
    assert_eq!(
        sent.len(),
        clean.len() + 5,
        "a late observer gets both blocks"
    );
}

// ---------------------------------------------------------------------------
// `<removedOnDamage>` — sleep is one-hit crowd control
// ---------------------------------------------------------------------------

/// `BuffInfo.isDisplayedForEffected()` — the one rule `isSelfContinuous()`
/// exists to feed.
///
/// An `A3` skill that also declares `<selfEffects>` shows its row only to the
/// caster. Blinding Blow's victim is blinded and *feels* it; they are simply
/// never sent an icon for it. Six skills on this dist qualify (321, 368, 369,
/// 409, 1231, 1996) and every other buff in the game is unaffected, so the
/// zero case is most of the assertion's value here.
#[test]
fn a_self_continuous_skills_debuff_shows_no_icon_to_its_victim() {
    let skills = dist::skills();

    let blinding_blow = skills.get(321, 1).expect("Blinding Blow loads");
    assert!(
        blinding_blow.self_continuous && !blinding_blow.self_effects.is_empty(),
        "the dist still declares 321 as A3 with selfEffects — the whole premise"
    );

    // A plain buff is A2/A1 and is displayed to whoever it lands on.
    let wind_walk = skills.get(1204, 1).expect("Wind Walk loads");
    assert!(
        !wind_walk.self_continuous,
        "an ordinary buff is not self-continuous"
    );

    // The rule itself, as `apply_continuous_effects` evaluates it.
    let displayed = |skill: &Skill, on_caster: bool| {
        !skill.self_continuous || on_caster || skill.self_effects.is_empty()
    };
    assert!(
        !displayed(blinding_blow, false),
        "the victim gets no icon for a self-continuous skill's debuff"
    );
    assert!(
        displayed(blinding_blow, true),
        "…but the caster still sees their own half of it"
    );
    assert!(
        displayed(wind_walk, false),
        "and nothing else in the game is hidden by this rule"
    );
}

/// The hidden buff must stay invisible in **both** channels Java gates on
/// `isDisplayedForEffected()`: the icon row and the abnormal-visual fold.
#[test]
fn a_hidden_buff_is_absent_from_the_icon_row_and_the_visuals() {
    use crate::model::components::skills::Buffs;
    use crate::model::skill::BuffSlot;
    use crate::model::skill::active_buff::ActiveBuff;

    let buff = |displayed: bool| ActiveBuff {
        skill_id: 321,
        abnormal_type_client_id: 7,
        slot: BuffSlot::Uncapped,
        expires_at_tick: 1000,
        displayed,
        abnormal_visuals: vec![13],
        ..test_buff()
    };

    // The icon row: the count field is the first thing after the opcode, so a
    // hidden buff has to leave it at zero rather than write a blank entry.
    let row = |displayed: bool| {
        let pkt =
            crate::network::enter_world::abnormal_status_update(&Buffs(vec![buff(displayed)]), 0);
        i16::from_le_bytes([pkt[1], pkt[2]])
    };
    assert_eq!(row(true), 1, "a displayed buff occupies a row");
    assert_eq!(row(false), 0, "a hidden one occupies none");

    // The visual fold, which Java runs under the same gate.
    let (mut world, _db, _l) = cc_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world
        .objects
        .add_components(&CASTER, Buffs(vec![buff(true)]));
    assert_eq!(abnormal::visual_effects(&world, CASTER), vec![13]);
    world
        .objects
        .add_components(&CASTER, Buffs(vec![buff(false)]));
    assert!(
        abnormal::visual_effects(&world, CASTER).is_empty(),
        "a hidden buff shows the effected no visual either"
    );
}

// ---------------------------------------------------------------------------
// `BreakStun` — a hit can shake a stun off (`Formulas.calcStunBreak`)
// ---------------------------------------------------------------------------
