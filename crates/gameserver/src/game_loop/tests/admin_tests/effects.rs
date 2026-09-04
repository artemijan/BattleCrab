//! `admin/effects.rs` and `admin/flags.rs` — abnormal visuals, social
//! gestures, the hide/invis/targetable/team flags, and the speed commands.

use super::*;

/// The GM startup block (`EnterWorld` GM branch) sets invul + invisible from
/// config, each gated by the `admin_invul`/`admin_invisible` access right.
#[test]
fn gm_startup_applies_invul_and_invisible() {
    let (mut world, ..) = admin_world();
    world.data.gm.startup_invulnerable = true;
    world.data.gm.startup_invisible = true;

    let mut rx = ingame_player_access(&mut world, 1, 6411, 100);
    drain(&mut rx);
    admin::apply_gm_startup(&mut world, 1, 6411);

    let f = world
        .objects
        .get_component::<AdminFlags>(&6411)
        .copied()
        .unwrap_or_default();
    assert!(f.invul, "GMStartupInvulnerable applied");
    assert!(f.hidden, "GMStartupInvisible applied");
    assert!(!f.silence && !f.diet, "unset startup flags stay off");
}

/// `GMStartupBuilderHide` hides the GM and **breaks** the startup process, so
/// the invul/invisible/silence/diet flags below the break are not applied
/// (Java's `break gmStartupProcess`). The three "…default for builder" notices
/// are sent.
#[test]
fn gm_startup_builder_hide_short_circuits() {
    let (mut world, ..) = admin_world();
    world.data.gm.startup_builder_hide = true;
    world.data.gm.startup_invulnerable = true; // would apply if not short-circuited

    let mut rx = ingame_player_access(&mut world, 1, 6421, 100);
    drain(&mut rx);
    admin::apply_gm_startup(&mut world, 1, 6421);

    let f = world
        .objects
        .get_component::<AdminFlags>(&6421)
        .copied()
        .unwrap_or_default();
    assert!(f.hidden, "builder hide set");
    assert!(!f.invul, "builder hide broke before the invul flag");
    assert_eq!(
        count_system_messages(&drain(&mut rx)),
        3,
        "three builder notices"
    );
}

/// `//silence` toggles message-refusal mode: on → MESSAGE_REFUSAL_MODE (177),
/// flag set, and an `EtcStatusUpdate` with the refusal bit so the client draws
/// the chat-block icon; a second toggle → MESSAGE_ACCEPTANCE_MODE (178), flag
/// cleared, and the bit cleared.
#[test]
fn admin_silence_toggles_refusal_mode() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6471, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("silence")].concat(),
    );
    let pkts = drain(&mut rx);
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&6471)
            .unwrap()
            .silence,
        "silence on"
    );
    assert!(has_system_message(&pkts, 177), "MESSAGE_REFUSAL_MODE");
    assert_eq!(
        etc_status_mask(&pkts).map(|m| m & 1),
        Some(1),
        "EtcStatusUpdate refusal bit set"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("silence")].concat(),
    );
    let pkts = drain(&mut rx);
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&6471)
            .unwrap()
            .silence,
        "silence off"
    );
    assert!(has_system_message(&pkts, 178), "MESSAGE_ACCEPTANCE_MODE");
    assert_eq!(
        etc_status_mask(&pkts).map(|m| m & 1),
        Some(0),
        "EtcStatusUpdate refusal bit cleared"
    );
}

/// `//hide` sends the GM's own client an `ExUserInfoAbnormalVisualEffect` with
/// the STEALTH effect present (so the invisible state renders), and clears it
/// on unhide.
#[test]
fn admin_hide_sends_stealth_visual() {
    let (mut world, ..) = admin_world();
    let mut rx = ingame_player_access(&mut world, 1, 6491, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("hide")].concat(),
    );
    assert_eq!(
        ave_effect_count(&drain(&mut rx)),
        Some(1),
        "STEALTH present when hidden"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("hide")].concat(),
    );
    assert_eq!(
        ave_effect_count(&drain(&mut rx)),
        Some(0),
        "no effects when visible again"
    );
}

/// `//gmspeed N` sets the move multiplier to **N** (0 resets) and rebroadcasts
/// UserInfo.
///
/// Java names the argument `runSpeedBoost` but feeds it to `addFixedValue`, and
/// a fixed value is an *override* — `CreatureStat.getValue` returns it and never
/// calls the finalizer. So the speed becomes `base * N`, not `base * (1 + N)`,
/// and `//gmspeed 1` is a no-op. This asserted `1 + N`, one whole multiple of
/// base speed too fast at every setting except 0.
#[test]
fn admin_gmspeed_sets_move_multiplier() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7103, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 3"));
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&7103)
            .unwrap()
            .move_multiplier,
        3.0,
        "the argument is the multiplier itself"
    );

    // The witness for the distinction: at 1 the fixed value equals the base, so
    // nothing moves. Under `1 + N` this would read 2.0.
    on_packet(&mut world, 1, build_admin("gmspeed 1"));
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&7103)
            .unwrap()
            .move_multiplier,
        1.0,
        "//gmspeed 1 is a no-op, not double speed"
    );
    assert!(
        drain(&mut gm_rx).iter().any(|p| p[0] == 0x32),
        "UserInfo (0x32) rebroadcast"
    );

    on_packet(&mut world, 1, build_admin("gmspeed 0"));
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&7103)
            .unwrap()
            .move_multiplier,
        1.0,
        "boost 0 resets"
    );
}

/// `//gmspeed` out of range answers the usage line and changes nothing.
#[test]
fn admin_gmspeed_rejects_out_of_range() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7107, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 99"));
    assert_eq!(count_system_messages(&drain(&mut gm_rx)), 1, "usage line");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&7107)
            .unwrap()
            .move_multiplier,
        1.0,
        "unchanged"
    );
}

/// `//hide` removes the GM from nearby players' view (DeleteObject) and toggling
/// it off re-introduces them (CharInfo).
#[test]
fn admin_hide_toggles_visibility() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7901, 100);
    let mut obs_rx = ingame_player_access(&mut world, 2, 7902, 0);
    drain(&mut gm_rx);
    drain(&mut obs_rx);

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&7901)
            .unwrap()
            .hidden
    );
    assert!(
        drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT
                && i32::from_le_bytes([p[1], p[2], p[3], p[4]]) == 7901),
        "observer got DeleteObject for the hidden GM"
    );

    on_packet(&mut world, 1, build_admin("hide"));
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&7901)
            .unwrap()
            .hidden
    );
    assert!(
        drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "observer got CharInfo when the GM reappeared"
    );
}

/// `//superhaste` applies and **persists**: Super Haste (7029) is a toggle with
/// no `abnormalTime`, so the buff must be permanent (Java `EffectList` never
/// schedules its stop) — it previously expired the same tick it landed.
#[test]
fn admin_superhaste_applies_and_persists() {
    use model::components::skills::Buffs;
    use model::components::stats::Speeds;
    let (mut world, ..) = admin_world();
    // Full datapack, not just `skill_data`: Super Haste also carries
    // `MpConsumePerLevel` (G19) — Java's `AdminSuperHaste` casts it through
    // the real `applyEffects` path (`superHasteSkill.applyEffects(player,
    // player, true, time)`), so it drains MP like any other toggle. The drain
    // is negligible (`power` 0.0001) but still needs a real MP pool: with
    // `for_test()`'s empty `player_templates` a level-1 dummy char computes 0
    // max MP, and the very first tick would exceed it and cancel the toggle.
    world.data = dist::game_data_owned();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8202, 100);
    drain(&mut gm_rx);

    let base_spd = world
        .objects
        .get_component::<Speeds>(&8202)
        .unwrap()
        .run_spd;
    on_packet(&mut world, 1, build_admin("superhaste 2"));

    // The buff is present, permanent, and raised run speed.
    let buff = world
        .objects
        .get_component::<Buffs>(&8202)
        .unwrap()
        .0
        .iter()
        .find(|b| b.skill_id == 7029)
        .cloned();
    let buff = buff.expect("super-haste buff applied");
    assert_eq!(buff.expires_at_tick, u64::MAX, "toggle buff is permanent");
    assert!(
        world
            .objects
            .get_component::<Speeds>(&8202)
            .unwrap()
            .run_spd
            > base_spd,
        "run speed increased"
    );

    // No BuffExpire was scheduled, so advancing the world keeps it.
    world.tick += 100;
    apply_due_tasks(&mut world);
    assert!(
        world
            .objects
            .get_component::<Buffs>(&8202)
            .unwrap()
            .0
            .iter()
            .any(|b| b.skill_id == 7029),
        "still active after ticks"
    );

    // //superhaste 0 clears it.
    on_packet(&mut world, 1, build_admin("superhaste 0"));
    assert!(
        !world
            .objects
            .get_component::<Buffs>(&8202)
            .unwrap()
            .0
            .iter()
            .any(|b| b.skill_id == 7029),
        "cleared by level 0"
    );
}

/// `Speeds::client_move_multiplier` is Java's `getMovementSpeedMultiplier`
/// (moveSpeed ÷ raw template base) — the leg-animation rate. A stat speed buff
/// raises `run_spd` and must raise the multiplier proportionally; a bare
/// `move_multiplier` there left buffed characters gliding with base-cadence legs
/// (the reported Super Haste "slow legs" symptom). `//gmspeed` keeps working
/// because it scales through `move_multiplier`, which folds in via `move_speed`.
#[test]
fn client_move_multiplier_tracks_speed_buffs() {
    use model::components::stats::Speeds;
    // base template run 132, +35 RunSpeedBoost folded into run_spd → 167 at rest.
    let mut s = Speeds {
        run_spd: 167.0,
        walk_spd: 90.0,
        swim_run_spd: 0.0,
        swim_walk_spd: 0.0,
        move_multiplier: 1.0,
        base_run_spd: 132.0,
        base_walk_spd: 88.0,
        base_swim_run_spd: 50.0,
        base_swim_walk_spd: 50.0,
        running: true,
        swimming: false,
        swamp_multiplier: 1.0,
    };
    // At rest it matches Java exactly: 167 / 132.
    assert!((s.client_move_multiplier() - 167.0 / 132.0).abs() < 1e-9);
    // Super Haste ×4 on run_spd → the multiplier quadruples with it.
    s.run_spd = 668.0;
    assert!((s.client_move_multiplier() - 668.0 / 132.0).abs() < 1e-9);
    // //gmspeed (move_multiplier) still folds through move_speed().
    s.run_spd = 167.0;
    s.move_multiplier = 4.0;
    assert!((s.client_move_multiplier() - 167.0 * 4.0 / 132.0).abs() < 1e-9);
    // Unknown base (0) is a safe no-op multiplier.
    s.base_run_spd = 0.0;
    assert_eq!(s.client_move_multiplier(), 1.0);
}

/// `CombatStats::client_atk_speed_multiplier` is Java's `getAttackSpeedMultiplier`
/// (`Formulas.calcAtkSpdMultiplier` = `pAtkSpd / 333`) — the swing-animation rate,
/// the haste counterpart of the move multiplier. Super Haste ×4 on `p_atk_spd`
/// must scale the swing animation with it; the old hardcoded `1.0` left it at
/// base cadence.
#[test]
fn client_atk_speed_multiplier_tracks_haste() {
    use model::components::stats::CombatStats;
    let mut c = CombatStats {
        p_atk_spd: 300,
        ..Default::default()
    };
    // Base p_atk_spd 300 → 300 / 333 (matches Java calcAtkSpdMultiplier).
    assert!((c.client_atk_speed_multiplier() - 300.0 / 333.0).abs() < 1e-9);
    // Super Haste ×4 on p_atk_spd → the multiplier quadruples with it.
    c.p_atk_spd = 1200;
    assert!((c.client_atk_speed_multiplier() - 1200.0 / 333.0).abs() < 1e-9);
}

/// `//social <id>` broadcasts a `SocialAction` on the GM (self, no target).
#[test]
fn admin_social_broadcasts_gesture() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8801, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("social 3"));
    let pkts = drain(&mut gm_rx);
    let social = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION)
        .expect("a SocialAction was broadcast");
    assert_eq!(
        i32::from_le_bytes(social[1..5].try_into().unwrap()),
        8801,
        "on the GM"
    );
    assert_eq!(
        i32::from_le_bytes(social[5..9].try_into().unwrap()),
        3,
        "action id 3"
    );
}

/// A player-invalid social id (< 2) is rejected with `NOTHING_HAPPENED` and no
/// gesture is sent.
#[test]
fn admin_social_rejects_out_of_range_action() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8802, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("social 1"));
    let pkts = drain(&mut gm_rx);
    assert!(
        !pkts
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SOCIAL_ACTION),
        "no gesture for an out-of-range action"
    );
    assert!(count_system_messages(&pkts) >= 1, "NOTHING_HAPPENED sent");
}

/// `//social <id> <radius>` affects other creatures within the radius.
#[test]
fn admin_social_radius_affects_nearby_player() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8803, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 8804, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);
    // Place both at the same spot so the other is in range and region-adjacent.
    let pos = *world.objects.get_component::<Position>(&8803).unwrap();
    if let Some(p) = world.objects.get_component_mut::<Position>(&8804) {
        *p = pos;
    }

    on_packet(&mut world, 1, build_admin("social 3 500"));
    assert!(
        drain(&mut other_rx).iter().any(|p| is_for(
            p,
            server_packets::opcodes::SOCIAL_ACTION,
            8804
        )),
        "the nearby player got the gesture"
    );
}

/// `//earthquake <intensity> <duration>` broadcasts an Earthquake to the GM.
#[test]
fn admin_earthquake_broadcasts() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8805, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("earthquake 20 10"));
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::EARTHQUAKE),
        "Earthquake broadcast"
    );
}

/// `//atmosphere sky day` sends `SunRise` to every online player.
#[test]
fn admin_atmosphere_broadcasts_to_all() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8806, 100);
    let mut other_rx = ingame_player_access(&mut world, 2, 8807, 0);
    drain(&mut gm_rx);
    drain(&mut other_rx);

    on_packet(&mut world, 1, build_admin("atmosphere sky day 0"));
    assert!(
        drain(&mut other_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::SUN_RISE),
        "SunRise reached an unrelated online player"
    );
}

/// `//play_sound <name>` plays the sound and confirms to the GM.
#[test]
fn admin_play_sound_plays() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8808, 100);
    drain(&mut gm_rx);

    on_packet(
        &mut world,
        1,
        build_admin("play_sound ItemSound.quest_middle"),
    );
    let pkts = drain(&mut gm_rx);
    assert!(
        pkts.iter()
            .any(|p| p[0] == server_packets::opcodes::PLAY_SOUND),
        "PlaySound sent"
    );
    assert!(count_system_messages(&pkts) >= 1, "confirmation line");
}

/// `//effect <skill>` broadcasts a cosmetic `MagicSkillUse` (self animation).
#[test]
fn admin_effect_broadcasts_msu() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8809, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("effect 1177 1 1"));
    let pkts = drain(&mut gm_rx);
    let msu = pkts
        .iter()
        .find(|p| p[0] == server_packets::opcodes::MAGIC_SKILL_USE)
        .expect("MagicSkillUse broadcast");
    // caster object id is at [5..9] (after the leading casting-bar int at [1..5]).
    assert_eq!(
        i32::from_le_bytes(msu[5..9].try_into().unwrap()),
        8809,
        "GM is the animation source"
    );
}

/// `//ave_abnormal <NAME>` toggles a GM-pinned abnormal visual on the target
/// (self when untargeted), and rejects an unknown effect name. The pinned set
/// is folded alongside buff-derived visuals by `abnormal::visual_effects`.
#[test]
fn admin_ave_abnormal_toggles_a_pinned_visual() {
    use crate::game_loop::abnormal::visual_effects;

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6481, 100);
    drain(&mut rx);

    assert!(
        visual_effects(&world, 6481).is_empty(),
        "nothing pinned to begin with"
    );

    // BIG_HEAD is client id 14.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("ave_abnormal BIG_HEAD"),
        ]
        .concat(),
    );
    assert!(visual_effects(&world, 6481).contains(&14), "pinned on");
    drain(&mut rx);

    // Toggling the same name removes it.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("ave_abnormal BIG_HEAD"),
        ]
        .concat(),
    );
    assert!(!visual_effects(&world, 6481).contains(&14), "pinned off");
    drain(&mut rx);

    // An unknown name changes nothing.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("ave_abnormal NOT_REAL"),
        ]
        .concat(),
    );
    assert!(
        visual_effects(&world, 6481).is_empty(),
        "an unknown effect name is rejected"
    );
}

// ---------------------------------------------------------------------------
// AdminEffects' G19 tail: //setteam, //para, //settargetable, //event_trigger,
// //playmovie, //bighead.
// ---------------------------------------------------------------------------

/// `//setteam blue` colors the aura (self when untargeted); `//setteam none`
/// clears it; a bad color is refused with usage text.
#[test]
fn admin_setteam_sets_the_aura_color() {
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6491, 100);
    drain(&mut rx);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam blue"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        1
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam red"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        2
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam purple"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        2,
        "bad color leaves it"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("setteam none"),
        ]
        .concat(),
    );
    assert_eq!(
        world.objects.get_component::<Player>(&6491).unwrap().team,
        0
    );
}

/// `//para` freezes the target — the block-actions and movement gates both
/// hold, and the PARALYZE visual (11) is pinned — and `//unpara` releases.
#[test]
fn admin_para_blocks_actions_until_unpara() {
    use crate::game_loop::abnormal::{
        is_blocked_from_actions, is_movement_disabled, visual_effects,
    };

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6492, 100);
    drain(&mut rx);

    assert!(!is_blocked_from_actions(&world, 6492));
    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("para")].concat(),
    );
    assert!(
        is_blocked_from_actions(&world, 6492),
        "GM paralysis blocks actions"
    );
    assert!(is_movement_disabled(&world, 6492), "and movement");
    assert!(
        visual_effects(&world, 6492).contains(&11),
        "PARALYZE visual pinned"
    );

    on_packet(
        &mut world,
        1,
        [vec![cop::SEND_BYPASS_BUILD_CMD], build_cmd_body("unpara")].concat(),
    );
    assert!(!is_blocked_from_actions(&world, 6492), "released");
    assert!(
        !visual_effects(&world, 6492).contains(&11),
        "visual unpinned"
    );
}

/// `//settargetable` makes the GM unselectable: another player's click no
/// longer sets their target; toggling back restores it.
#[test]
fn admin_settargetable_blocks_selection() {
    use model::components::combat::TargetRef;

    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 6493, 100);
    let mut rx2 = ingame_player_access(&mut world, 2, 6494, 0);
    drain(&mut rx);
    drain(&mut rx2);

    let click_gm = {
        let mut w = PacketWriter::new();
        w.write_i32(6493);
        w.write_i32(0);
        w.write_i32(0);
        w.write_i32(0);
        w.write_u8(0);
        w.into_bytes()
    };
    handle_action(&mut world, 2, &click_gm);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&6494)
            .copied()
            .unwrap_or_default()
            .0,
        Some(6493),
        "targetable by default"
    );
    set_target(&mut world, 2, 6494, None);

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("settargetable"),
        ]
        .concat(),
    );
    handle_action(&mut world, 2, &click_gm);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&6494)
            .copied()
            .unwrap_or_default()
            .0,
        None,
        "untargetable GM can't be selected"
    );

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("settargetable"),
        ]
        .concat(),
    );
    handle_action(&mut world, 2, &click_gm);
    assert_eq!(
        world
            .objects
            .get_component::<TargetRef>(&6494)
            .copied()
            .unwrap_or_default()
            .0,
        Some(6493),
        "toggled back"
    );
}

/// **The gm_menu "Invis" button works end-to-end.** `admin_invis_menu` was
/// undispatched ("not implemented yet"): it must toggle invisibility — the
/// observer's selection drops (TargetUnselected) before the DeleteObject —
/// re-serve `gm_menu.htm`, suppress CharInfo rebroadcasts while hidden (the
/// old `broadcast_user_info` leaked the GM back onto nearby clients), and
/// re-describe the GM on the second press.
#[test]
fn admin_invis_menu_hides_and_reserves_panel() {
    use model::components::combat::TargetRef;
    use model::components::player::AdminFlags;
    let (mut world, ..) = admin_world();
    world.data.root = crate::data::DIST_GAME.to_string();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7101, 100);
    let mut obs_rx = ingame_player_access(&mut world, 2, 7102, 0);
    world.objects.add_components(&7102, TargetRef(Some(7101)));
    drain(&mut gm_rx);
    drain(&mut obs_rx);

    on_packet(&mut world, 1, build_admin("invis_menu"));
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&7101)
            .is_some_and(|f| f.hidden),
        "GM hidden after the Invis button"
    );
    let obs = drain(&mut obs_rx);
    assert!(
        obs.iter()
            .any(|p| p[0] == server_packets::opcodes::TARGET_UNSELECTED),
        "observer's selection dropped"
    );
    assert!(
        obs.iter()
            .any(|p| p[0] == server_packets::opcodes::DELETE_OBJECT),
        "GM removed from the observer's client"
    );
    assert!(
        drain(&mut gm_rx)
            .iter()
            .filter_map(|p| decode_npc_html(p))
            .any(|h| h.contains("admin_invis_menu")),
        "gm_menu.htm re-served to keep the panel up"
    );

    // While hidden, a UserInfo broadcast must not leak CharInfo to others.
    crate::game_loop::character::player_info::broadcast_user_info(&mut world, 7101);
    assert!(
        !drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "no CharInfo leak to the observer while hidden"
    );

    on_packet(&mut world, 1, build_admin("invis_menu"));
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&7101)
            .unwrap()
            .hidden,
        "second press unhides"
    );
    assert!(
        drain(&mut obs_rx)
            .iter()
            .any(|p| p[0] == server_packets::opcodes::CHAR_INFO),
        "CharInfo re-sent to the observer on unhide"
    );
}

/// **`//vis` sets visible, never toggles.** The old alias collapsed the whole
/// family onto the `//hide` toggle, so `//vis` while visible *hid* you.
/// `//invis` is likewise an idempotent set.
#[test]
fn vis_and_invis_are_sets_not_toggles() {
    use model::components::player::AdminFlags;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7111, 100);
    drain(&mut gm_rx);
    let hidden = |world: &World| {
        world
            .objects
            .get_component::<AdminFlags>(&7111)
            .is_some_and(|f| f.hidden)
    };

    on_packet(&mut world, 1, build_admin("vis"));
    assert!(!hidden(&world), "//vis while visible stays visible");

    on_packet(&mut world, 1, build_admin("invis"));
    assert!(hidden(&world), "//invis hides");
    on_packet(&mut world, 1, build_admin("invis"));
    assert!(hidden(&world), "//invis is idempotent");

    on_packet(&mut world, 1, build_admin("visible"));
    assert!(!hidden(&world), "//visible unhides");
}

/// **`//setinvis` acts on the *target*, not the GM.** The old alias hid the
/// GM themself.
#[test]
fn setinvis_toggles_the_targeted_player() {
    use model::components::combat::TargetRef;
    use model::components::player::AdminFlags;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7121, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7122, 0);
    world.objects.add_components(&7121, TargetRef(Some(7122)));
    drain(&mut gm_rx);
    drain(&mut victim_rx);

    on_packet(&mut world, 1, build_admin("setinvis"));
    assert!(
        world
            .objects
            .get_component::<AdminFlags>(&7122)
            .is_some_and(|f| f.hidden),
        "the targeted player is hidden"
    );
    assert!(
        !world
            .objects
            .get_component::<AdminFlags>(&7121)
            .is_some_and(|f| f.hidden),
        "the GM themself stays visible"
    );
}

/// **`//ave_abnormal` with no argument opens the effect list.** Java's
/// `AdminEffects` treats a missing (or numeric) first token as "show the menu"
/// and pages `AbnormalVisualEffect.values()` 100 at a time into
/// `data/html/admin/ave_abnormal.htm`; only a non-numeric token toggles an
/// effect. The port only printed a usage line, so the Game panel's "Abnormal
/// Visual Effects" button opened nothing.
#[test]
fn ave_abnormal_without_args_serves_the_paged_effect_list() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7501, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("ave_abnormal"));
    let page0 = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("the menu html is served");
    assert!(
        page0.contains("Abnormal Visual Effects"),
        "ave_abnormal.htm was loaded"
    );
    assert!(
        page0.contains("bypass admin_ave_abnormal STUN") && page0.contains("STUN(7)"),
        "each effect is a button that re-enters the command by name"
    );
    assert!(
        !page0.contains("bypass admin_ave_abnormal YOGI\""),
        "YOGI sits at enum index 100, so it opens page 2 (100 per page)"
    );

    // The pager's links carry a bare page number (Java's DefaultFormatter),
    // which the command parses as a page rather than an effect name.
    assert!(
        page0.contains("bypass -h admin_ave_abnormal 1"),
        "next-page link is `<bypass> <page>`"
    );
    on_packet(&mut world, 1, build_admin("ave_abnormal 1"));
    let page1 = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("page 2 html");
    assert!(
        page1.contains("bypass admin_ave_abnormal YOGI\""),
        "page 2 holds the next 100 effects"
    );
    assert!(
        !page1.contains("bypass admin_ave_abnormal STUN"),
        "and not the first page's"
    );

    // A name still toggles, so the buttons work.
    world.objects.add_components(&7501, TargetRef(Some(7501)));
    on_packet(&mut world, 1, build_admin("ave_abnormal AURA_BUFF"));
    assert!(
        abnormal::visual_effects(&world, 7501).contains(&57),
        "clicking a button applies the effect"
    );
}

/// **The Effects panel's buttons come back with a page.** Java ends
/// `AdminEffects.useAdminCommand` with `if (command.contains("menu"))
/// showMainPage(...)`, which re-serves `effects_menu.htm` — or `social.htm`
/// for the social commands, the panel's own sub-page. The port ran the action
/// and sent nothing, so every button press dropped the panel and "Social"
/// opened nothing at all. `//transform_menu` belongs to `AdminTransform` and
/// has its own page (`transform.htm`), not the main GM menu the port served.
#[test]
fn effects_panel_menu_commands_reserve_their_pages() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7510, 100);
    world.objects.add_components(&7510, TargetRef(Some(7510)));
    drain(&mut rx);

    let click =
        |world: &mut World, rx: &mut UnboundedReceiver<bytes::Bytes>, cmd: &str| -> String {
            on_packet(world, 1, build_admin(cmd));
            drain(rx)
                .iter()
                .filter_map(|p| decode_npc_html(p))
                .next_back()
                .unwrap_or_default()
        };

    assert!(
        click(&mut world, &mut rx, "social_menu 2").contains("Social Menu"),
        "social lands on social.htm"
    );
    assert!(
        click(&mut world, &mut rx, "effect_menu").contains("Effects Menu"),
        "effect_menu serves the panel"
    );
    for cmd in [
        "para_menu",
        "unpara_menu",
        "para_all_menu",
        "unpara_all_menu",
    ] {
        assert!(
            click(&mut world, &mut rx, cmd).contains("Effects Menu"),
            "{cmd} leaves the panel up"
        );
    }
    assert!(
        click(&mut world, &mut rx, "earthquake_menu 20 10").contains("Effects Menu"),
        "earthquake_menu leaves the panel up"
    );
    assert!(
        click(&mut world, &mut rx, "transform_menu").contains("Transform"),
        "transform_menu opens the transform sub-page, not gm_menu"
    );
    // A non-menu command must NOT drag a page in.
    on_packet(&mut world, 1, build_admin("para"));
    assert!(
        drain(&mut rx)
            .iter()
            .filter_map(|p| decode_npc_html(p))
            .next()
            .is_none(),
        "the bare command sends no html, as in Java"
    );
}

/// **The pager is `PageBuilder`'s default, numbered one.** `AdminEffects` never
/// calls `pageHandler()`, so `//ave_abnormal` gets `DefaultPageHandler` +
/// `ButtonsStyle`: a numbered strip whose current page is plain text and whose
/// others are buttons. The port had rendered `NextPrevPageHandler`'s
/// `First | Prev | Page: x/y | Next | Last` strip, which this page never uses.
#[test]
fn ave_menu_pager_is_the_numbered_default_handler() {
    const ROOT: &str = crate::data::DIST_GAME;
    let (mut world, ..) = admin_world();
    world.data.root = ROOT.to_string();
    let mut rx = ingame_player_access(&mut world, 1, 7511, 100);
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("ave_abnormal"));
    let page0 = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("menu html");
    // 206 effects at 100 per page = 3 pages, all three linked from page one
    // (`DefaultPageHandler`'s window is the current page ± 2).
    assert!(
        page0.contains("<td>1</td>"),
        "the current page is plain text, not a button: {page0}"
    );
    for page in ["1", "2"] {
        assert!(
            page0.contains(&format!(
                "<button action=\"bypass -h admin_ave_abnormal {page}\" value=\"{}\" ",
                page.parse::<i32>().unwrap() + 1
            )),
            "page {page} is a numbered button"
        );
    }
    assert!(
        !page0.contains("admin_ave_abnormal 3"),
        "no link past the last page (index 2)"
    );
    assert!(
        !page0.contains("Page: 1/") && !page0.contains("value=\"Last\""),
        "not the next/prev strip"
    );
    // The fullest page must stay under the ~17k the client chokes on.
    assert!(
        page0.len() < 17_000,
        "page html is {} chars — over the client's limit",
        page0.len()
    );

    // The final page is reachable and populated.
    on_packet(&mut world, 1, build_admin("ave_abnormal 2"));
    let last = drain(&mut rx)
        .iter()
        .filter_map(|p| decode_npc_html(p))
        .next()
        .expect("last page html");
    assert!(
        last.contains("bypass admin_ave_abnormal BR_Y_3_ACCESSORY_NECKRACE"),
        "the last page holds the tail of the enum"
    );
    assert!(
        last.contains("<td>3</td>") && last.contains("admin_ave_abnormal 0"),
        "and pages back to the first"
    );
}

/// Java's `//gmspeed` target is any **Creature**, not just a player — an NPC
/// can be sped up too, and it gets `broadcastInfo()` rather than `UserInfo`.
#[test]
fn admin_gmspeed_scales_a_targeted_npc() {
    use model::components::combat::TargetRef;
    use model::components::stats::Speeds;
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7110, 100);
    scan_npc(&mut world, NPC_OID, 7110, 50, 0, 0);
    world
        .objects
        .add_components(&7110, TargetRef(Some(NPC_OID)));
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("gmspeed 5"));
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&NPC_OID)
            .unwrap()
            .move_multiplier,
        5.0,
        "the NPC target is scaled"
    );
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&7110)
            .unwrap()
            .move_multiplier,
        1.0,
        "and the GM is left alone"
    );
}

/// **`//para` has to be visible** (GitHub #10). Java's
/// `startAbnormalVisualEffect` ends in `updateAbnormalVisualEffects()`, which
/// sends the owner their own `ExUserInfoAbnormalVisualEffect` alongside the
/// `CharInfo` broadcast. The port set the flag and broadcast `UserInfo` only —
/// so the target was paralysed with nothing to show for it.
#[test]
fn para_sends_the_abnormal_visual_to_the_target() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 7601, 100);
    let mut victim_rx = ingame_player_access(&mut world, 2, 7602, 0);
    drain(&mut gm_rx);
    drain(&mut victim_rx);
    world.objects.add_components(&7601, TargetRef(Some(7602)));

    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("para_menu"),
        ]
        .concat(),
    );

    let effects = ave_effect_count(&drain(&mut victim_rx));
    assert_eq!(
        effects,
        Some(1),
        "the paralysed player is told about the one visual effect on them"
    );

    // …and lifting it tells them again, with the set back to empty.
    on_packet(
        &mut world,
        1,
        [
            vec![cop::SEND_BYPASS_BUILD_CMD],
            build_cmd_body("unpara_menu"),
        ]
        .concat(),
    );
    let effects = ave_effect_count(&drain(&mut victim_rx));
    assert_eq!(effects, Some(0), "and about it going away");
}
