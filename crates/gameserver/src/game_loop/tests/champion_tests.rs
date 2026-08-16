//! Champion monsters (`Custom/ChampionMonsters.ini`) — the spawn lottery and
//! every consumer that reads the resulting flag.
//!
//! The feature is off by default (`ChampionConfig::default().enable == false`),
//! so each test turns it on explicitly and pins `frequency` to 0 or 100 rather
//! than letting the dist's 1 % roll decide anything.

use super::*;

use crate::config::ChampionConfig;
use crate::data::npc_data::default_template;
use crate::model::npc::{Npc, roll_champion};

const MOB: i32 = NPC_OID;
const KILLER: i32 = 2001;
const KILLER_CID: u32 = 1;

/// `ChampionEnable = True` with a certain roll, and the dist's tuning values,
/// so a test asserts on the multiplier rather than on the lottery.
fn champion_on() -> ChampionConfig {
    ChampionConfig {
        enable: true,
        frequency: 100,
        min_level: 1,
        max_level: 99,
        hp: 10,
        atk: 4.0,
        spd_atk: 2.0,
        hp_regen: 2.0,
        rewards_exp_sp: 10.0,
        rewards_chance: 5.0,
        rewards_amount: 5.0,
        adenas_rewards_chance: 10.0,
        adenas_rewards_amount: 10.0,
        passive: true,
        aura: true,
        title: "Champion".to_string(),
        reward_items: vec![(6393, 1)],
        reward_lower_level_item_chance: 0,
        reward_higher_level_item_chance: 100,
        ..Default::default()
    }
}

fn monster(level: i32) -> crate::data::npc_data::NpcTemplate {
    let mut t = default_template(90_001);
    t.type_name = "Monster".into();
    t.level = level;
    t
}

// ---------------------------------------------------------------------------
// The lottery (`Attackable.onRespawn`)
// ---------------------------------------------------------------------------

#[test]
fn an_eligible_monster_wins_the_certain_roll() {
    assert!(roll_champion(&champion_on(), &monster(40)));
}

#[test]
fn the_master_gate_and_the_frequency_each_stop_the_roll() {
    let mut cfg = champion_on();
    cfg.enable = false;
    assert!(!roll_champion(&cfg, &monster(40)), "ChampionEnable = False");

    let mut cfg = champion_on();
    cfg.frequency = 0;
    assert!(
        !roll_champion(&cfg, &monster(40)),
        "ChampionFrequency = 0 disables the roll even with the feature on"
    );
}

/// Java's guard chain, one exclusion per assert. Each of these is a mob a
/// player must be able to fight at its stated difficulty: a quest target with
/// 10× HP would stall a quest outright, and an undying NPC cannot die at all.
#[test]
fn the_guard_chain_excludes_every_kind_java_excludes() {
    let cfg = champion_on();

    let mut folk = monster(40);
    folk.type_name = "Folk".into();
    assert!(!roll_champion(&cfg, &folk), "not in the Monster subtree");

    let mut quest_mob = monster(40);
    quest_mob.title = "Quest Monster".into();
    assert!(
        !roll_champion(&cfg, &quest_mob),
        "isQuestMonster() — the title contains \"Quest\""
    );

    let mut undying = monster(40);
    undying.undying = true;
    assert!(!roll_champion(&cfg, &undying), "<status undying=\"true\">");

    let mut raid = monster(40);
    raid.type_name = "RaidBoss".into();
    assert!(
        !roll_champion(&cfg, &raid),
        "raid bosses are never champions"
    );

    let mut grand = monster(40);
    grand.type_name = "GrandBoss".into();
    assert!(!roll_champion(&cfg, &grand), "…nor grand bosses");
}

/// The level window is inclusive on both ends, and it reads the *monster's*
/// level, not the killer's.
#[test]
fn the_level_window_is_inclusive_on_both_ends() {
    let mut cfg = champion_on();
    cfg.min_level = 30;
    cfg.max_level = 85;

    assert!(!roll_champion(&cfg, &monster(29)), "below the window");
    assert!(roll_champion(&cfg, &monster(30)), "min is inclusive");
    assert!(roll_champion(&cfg, &monster(85)), "max is inclusive");
    assert!(!roll_champion(&cfg, &monster(86)), "above the window");
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// `PAttackFinalizer`/`MAttackFinalizer`/`P|MAttackSpeedFinalizer`: the
/// champion multipliers ride the base value. Max HP is deliberately *not*
/// multiplied — Java models bulk as a damage divisor instead.
#[test]
fn champion_stat_multipliers_scale_attack_but_not_max_hp() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();

    let mut t = monster(40);
    t.base_p_atk = 100.0;
    t.base_m_atk = 80.0;
    t.base_p_atk_spd = 200;
    t.base_m_atk_spd = 300;
    t.base_hp_max = 1000.0;
    world.data.npc_data.insert_for_test(t.clone());

    let plain = crate::model::npc_finalized_stats(
        &world.data,
        &t,
        &Buffs::default(),
        crate::model::NpcStatMods::default(),
    );
    let champ = crate::model::npc_finalized_stats(
        &world.data,
        &t,
        &Buffs::default(),
        crate::model::NpcStatMods::of(&world.cfg, true, false),
    );

    assert!(
        (champ.0.p_atk - plain.0.p_atk * 4.0).abs() < 0.001,
        "ChampionAtk = 4 → P.Atk ×4 ({} vs {})",
        champ.0.p_atk,
        plain.0.p_atk
    );
    assert!(
        (champ.0.m_atk - plain.0.m_atk * 4.0).abs() < 0.001,
        "…and M.Atk ×4"
    );
    assert_eq!(
        champ.0.p_atk_spd,
        plain.0.p_atk_spd * 2,
        "ChampionSpdAtk = 2 → P.Atk speed ×2"
    );
    assert_eq!(champ.0.m_atk_spd, plain.0.m_atk_spd * 2, "…and M.Atk speed");
    assert_eq!(
        champ.2, plain.2,
        "max HP is untouched — the bulk comes from the damage divisor"
    );
}

/// The multipliers must survive a stat recompute. A buff landing on a champion
/// runs the whole pipeline again, and a neutral recompute there would quietly
/// strip its P.Atk back to the template value mid-fight.
#[test]
fn a_stat_recompute_keeps_the_champion_multipliers() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();

    let mut t = monster(40);
    t.base_p_atk = 100.0;
    world.data.npc_data.insert_for_test(t.clone());

    let mods = crate::model::NpcStatMods::of(&world.cfg, true, false);
    let before = crate::model::npc_finalized_stats(&world.data, &t, &Buffs::default(), mods);

    let mut combat = before.0;
    let mut speeds = before.1;
    let mut vitals = Vitals {
        max_hp: before.2 as i32,
        cur_hp: before.2,
        max_mp: before.3 as i32,
        cur_mp: before.3,
        dead: false,
    };
    crate::model::recompute_npc_stats_from_buffs(
        &world.data,
        &t,
        &Buffs::default(),
        mods,
        &mut combat,
        &mut speeds,
        &mut vitals,
    );
    assert!(
        (combat.p_atk - before.0.p_atk).abs() < 0.001,
        "recompute kept ×4 P.Atk ({} vs {})",
        combat.p_atk,
        before.0.p_atk
    );
}

// ---------------------------------------------------------------------------
// Damage
// ---------------------------------------------------------------------------

/// `Creature.reduceCurrentHp`: the hit is divided by `ChampionHp`.
#[test]
fn incoming_damage_is_divided_by_champion_hp() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();
    add_test_npc(&mut world, MOB, 90_010, "Monster", 40, 0, 0, 0);

    let full = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;

    // Plain mob first: 50 damage removes 50 HP.
    crate::game_loop::combat::npc_receive_damage(&mut world, MOB, KILLER, 50.0, false);
    let plain_after = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;
    assert!(
        (full - plain_after - 50.0).abs() < 0.001,
        "a normal mob takes the full hit"
    );

    // Same mob, now a champion: the next 50 removes only 5.
    world
        .objects
        .get_component_mut::<Npc>(&MOB)
        .unwrap()
        .champion = true;
    crate::game_loop::combat::npc_receive_damage(&mut world, MOB, KILLER, 50.0, false);
    let champ_after = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;
    assert!(
        (plain_after - champ_after - 5.0).abs() < 0.001,
        "ChampionHp = 10 → 50 damage costs 5 HP (took {})",
        plain_after - champ_after
    );
}

/// The divisor is gated on the master flag, so flipping `ChampionEnable` off
/// makes an already-spawned champion take full damage again — Java re-checks
/// `Config.CHAMPION_ENABLE` at the consumer, it does not trust the stored flag.
#[test]
fn the_damage_divisor_respects_the_master_gate() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();
    world.cfg.champion.enable = false;
    add_test_npc(&mut world, MOB, 90_011, "Monster", 40, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Npc>(&MOB)
        .unwrap()
        .champion = true;

    let before = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;
    crate::game_loop::combat::npc_receive_damage(&mut world, MOB, KILLER, 50.0, false);
    let after = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;
    assert!(
        (before - after - 50.0).abs() < 0.001,
        "the flag alone must not divide damage"
    );
}

/// `ChampionHp = 0` disables the division (Java's `!= 0` guard) rather than
/// dividing by zero.
#[test]
fn a_zero_champion_hp_disables_the_division() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();
    world.cfg.champion.hp = 0;
    add_test_npc(&mut world, MOB, 90_012, "Monster", 40, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Npc>(&MOB)
        .unwrap()
        .champion = true;

    let before = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;
    crate::game_loop::combat::npc_receive_damage(&mut world, MOB, KILLER, 30.0, false);
    let after = world.objects.get_component::<Vitals>(&MOB).unwrap().cur_hp;
    assert!(
        (before - after - 30.0).abs() < 0.001 && after.is_finite(),
        "no division, no NaN"
    );
}

// ---------------------------------------------------------------------------
// Title & aura
// ---------------------------------------------------------------------------

/// Java's two title arms differ: the decorated branch *prefixes*, the plain
/// branch *replaces*.
#[test]
fn the_champion_title_prefixes_when_decorated_and_replaces_when_not() {
    use crate::network::server_packets::npc_title;

    let mut t = monster(20);
    t.title = "Fighter".into();
    t.is_aggressive = true;

    let mut cfg = crate::config::NpcConfig::default();

    // Plain branch (both decorations off): the template title is dropped.
    cfg.show_npc_level = false;
    cfg.show_npc_aggression = false;
    assert_eq!(npc_title(&t, &cfg, None), "Fighter");
    assert_eq!(
        npc_title(&t, &cfg, Some("Champion")),
        "Champion",
        "the plain branch returns CHAMP_TITLE alone, dropping \"Fighter\""
    );

    // Decorated branch: prefix onto the whole decorated string.
    cfg.show_npc_level = true;
    assert_eq!(npc_title(&t, &cfg, None), "Lv 20 Fighter");
    assert_eq!(
        npc_title(&t, &cfg, Some("Champion")),
        "Champion Lv 20 Fighter"
    );
}

/// `NpcInfo` must carry the red team byte, and only while the aura is on —
/// together with the title it is what makes a champion recognisable before it
/// hits.
///
/// The test world leaves `ShowNpcLevel`/`ShowNpcAggression` off, which is
/// exactly the configuration that isolates Java's `|| npc.isChampion()` arm of
/// the TITLE gate: with both decorations off a plain mob carries no title
/// component at all, so every byte of difference below is the champion's own.
#[test]
fn npc_info_carries_the_red_team_only_with_the_aura_on() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();
    add_test_npc(&mut world, MOB, 90_020, "Monster", 40, 0, 0, 0);

    let build = |world: &World| {
        let v = crate::model::npc::NpcView::of(&world.objects, MOB).expect("a live mob");
        let t = v.npc.template(world).expect("its template");
        crate::network::server_packets::npc_info(
            &v,
            t,
            &world.cfg.npc,
            &world.cfg.champion,
            &[],
            None,
        )
    };

    let plain = build(&world);
    world
        .objects
        .get_component_mut::<Npc>(&MOB)
        .unwrap()
        .champion = true;
    let champ = build(&world);
    assert_ne!(
        plain, champ,
        "the champion's NpcInfo must differ — TEAM and TITLE are both added"
    );
    // TEAM is one byte (2 = `Team.RED`); TITLE is the UTF-16 `"Champion"` plus
    // its terminator — 8 chars × 2 + 2 = 18.
    assert_eq!(
        champ.len(),
        plain.len() + 1 + 18,
        "TEAM (1 byte) + the \"Champion\" TITLE string (18 bytes)"
    );

    // Turning the aura off drops only the team byte. The title must survive:
    // an operator who runs champions without the red glow still needs the name
    // to tell them apart, and Java's TITLE gate does not consult `ChampionAura`.
    world.cfg.champion.aura = false;
    let no_aura = build(&world);
    assert_eq!(
        no_aura.len(),
        plain.len() + 18,
        "ChampionAura = False drops the team byte but keeps the title"
    );
    assert!(
        contains_utf16(&no_aura, "Champion"),
        "the champion title must still reach the client without the aura"
    );
    assert!(
        !contains_utf16(&plain, "Champion"),
        "a plain mob must carry no champion title"
    );
}

/// The champion arm must not *replace* the normal gate: with `ShowNpcLevel` on
/// a champion still gets the decorated title, prefix and all.
#[test]
fn npc_info_title_survives_with_the_show_npc_decorations_on() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();
    world.cfg.npc.show_npc_level = true;
    add_test_npc(&mut world, MOB, 90_020, "Monster", 40, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Npc>(&MOB)
        .unwrap()
        .champion = true;

    let v = crate::model::npc::NpcView::of(&world.objects, MOB).expect("a live mob");
    let t = v.npc.template(&world).expect("its template");
    let pkt = crate::network::server_packets::npc_info(
        &v,
        t,
        &world.cfg.npc,
        &world.cfg.champion,
        &[],
        None,
    );
    assert!(
        contains_utf16(&pkt, "Champion Lv 40"),
        "the decorated branch prefixes CHAMP_TITLE onto \"Lv 40\""
    );
}

/// Needle search over the packet's little-endian UTF-16 string encoding.
fn contains_utf16(haystack: &[u8], needle: &str) -> bool {
    let encoded: Vec<u8> = needle.encode_utf16().flat_map(u16::to_le_bytes).collect();
    haystack.windows(encoded.len()).any(|w| w == encoded)
}

// ---------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------

/// `AttackableAI.autoAttackCondition`: `ChampionPassive` stops a champion
/// seeding hate from its aggro scan.
#[test]
fn champion_passive_stops_the_aggro_scan() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on();
    let _rx = ingame_caster(&mut world, KILLER_CID, KILLER, 0, 0);

    let mut t = default_template(90_030);
    t.type_name = "Monster".into();
    t.level = 40;
    t.is_aggressive = true;
    t.aggro_range = 500;
    t.base_hp_max = 100.0;
    world.data.npc_data.insert_for_test(t);
    add_test_npc(&mut world, MOB, 90_030, "Monster", 40, 50, 0, 0);
    // The scan only runs once the spawn-calm counter has run out.
    world
        .objects
        .get_component_mut::<crate::model::npc::NpcAi>(&MOB)
        .unwrap()
        .global_aggro = 0;

    // Aggressive and not a champion: hate is seeded.
    crate::game_loop::ai::npc_ai_tick(&mut world);
    let hated = world
        .objects
        .get_component::<crate::model::npc::AggroList>(&MOB)
        .is_some_and(|a| !a.0.is_empty());
    assert!(hated, "an ordinary aggressive mob seeds hate on the player");

    // Same mob as a passive champion: the scan must find nothing new.
    //
    // Resetting the *intention* back to Active is what makes this half real —
    // the first scan left the mob in `Attack`, and a second tick would then run
    // the attack loop instead of the aggro scan, so the assert below would hold
    // no matter what the champion gate did. (It did exactly that until a
    // sabotage run caught it.)
    world
        .objects
        .get_component_mut::<crate::model::npc::AggroList>(&MOB)
        .unwrap()
        .0
        .clear();
    world
        .objects
        .get_component_mut::<Npc>(&MOB)
        .unwrap()
        .champion = true;
    {
        let ai = world
            .objects
            .get_component_mut::<crate::model::npc::NpcAi>(&MOB)
            .unwrap();
        ai.global_aggro = 0;
        ai.intention = crate::model::npc::NpcIntention::Active;
    }
    crate::game_loop::ai::npc_ai_tick(&mut world);
    assert!(
        world
            .objects
            .get_component::<crate::model::npc::AggroList>(&MOB)
            .is_some_and(|a| a.0.is_empty()),
        "ChampionPassive = True → a champion stands still until attacked"
    );
}

// ---------------------------------------------------------------------------
// Config-level reward behaviour
// ---------------------------------------------------------------------------

/// `Attackable.useVitalityRate()` — the gate that decides whether a champion
/// kill charges vitality and pays PA points at all.
#[test]
fn a_champion_kill_skips_vitality_unless_the_flag_is_set() {
    let cfg = champion_on();
    assert!(
        cfg.uses_vitality_rate(false),
        "an ordinary kill always consumes vitality"
    );
    assert!(
        !cfg.uses_vitality_rate(true),
        "ChampionEnableVitality = False → a champion kill does not"
    );

    let mut with_vitality = champion_on();
    with_vitality.enable_vitality = true;
    assert!(with_vitality.uses_vitality_rate(true));
}

// ---------------------------------------------------------------------------
// Drops
// ---------------------------------------------------------------------------

const ADENA: i32 = 57;
const LOOTER: i32 = 8001;
const LOOTER_CID: u32 = 3;

/// The two halves of `calculateDrops`' champion arithmetic, end to end on the
/// fixture mob: the adena amount is multiplied, and `ChampionRewardItems` is
/// appended on top of whatever was rolled.
#[test]
fn a_champion_pays_multiplied_adena_plus_the_reward_item() {
    let (mut world, _db, _l) = combat_test_world();
    let _out = ingame_caster(&mut world, LOOTER_CID, LOOTER, 0, 0);
    world.cfg.champion = champion_on();

    let t = world.data.npc_data.get(40001).unwrap().clone();

    // Plain kill: gap gate + chance roll both forced to pass.
    world.forced_rolls.extend([0, 0]);
    let plain = crate::game_loop::death::roll_drops_for_test(&mut world, &t, LOOTER);
    let plain_adena = plain
        .iter()
        .find(|(id, _)| *id == ADENA)
        .map(|(_, c)| *c)
        .expect("the fixture mob drops adena");

    // Champion kill: same two rolls, plus the reward-item suppression roll.
    // The mob (level 1 fixture) is below the level-40 looter, and
    // `ChampionRewardLowerLvlItemChance = 0`, so nothing suppresses the item.
    world.forced_rolls.extend([0, 0, 50]);
    let champ = crate::game_loop::death::roll_champion_drops_for_test(&mut world, &t, LOOTER);
    let champ_adena = champ
        .iter()
        .find(|(id, _)| *id == ADENA)
        .map(|(_, c)| *c)
        .expect("a champion still drops its adena");

    assert!(
        champ_adena > plain_adena,
        "the champion adena multiplier applied ({champ_adena} vs {plain_adena})"
    );
    assert!(
        champ.contains(&(6393, 1)),
        "ChampionRewardItems appended: {champ:?}"
    );
    assert!(
        !plain.contains(&(6393, 1)),
        "…and only for a champion: {plain:?}"
    );
}

// ---------------------------------------------------------------------------
// Quest drops (`AbstractScript.giveItemRandomly`)
// ---------------------------------------------------------------------------

/// Quest items never pass through `NpcTemplate.calculateDrops`, so Java repeats
/// the champion arm inside `giveItemRandomly`. Without it a champion is a pure
/// penalty on a collection quest — ten times the HP for the same payout.
///
/// Driven end to end through a real quest (`Q00358`, which asks for one snake
/// scale per kill) rather than the helper in isolation, so the `self.npc`
/// lookup that resolves the champion flag is exercised on the live kill path.
#[test]
fn a_champion_multiplies_the_quest_item_payout() {
    const SNAKE_SCALE: i32 = 5868;
    const SNAKE: i32 = 20672;
    const PLAYER: i32 = 3001;
    let quest = "Q00358_IllegitimateChildOfTheGoddess";

    // One kill of a plain mob, then of a champion, from identical worlds.
    let scales_from_one_kill = |champion: bool| -> i64 {
        let (mut world, _db, _l) = quest_test_world();
        add_quest_items(&mut world, &[(SNAKE_SCALE, "Snake Scale", true)]);
        world.cfg.champion = champion_on();
        // Pin the quest rate so the assertion reads the champion factor alone.
        world.cfg.rates.rate_quest_drop = 1.0;

        add_test_npc(&mut world, NPC_OID, 30862, "Folk", 60, 100, 0, 0);
        let _rx = ingame_player(&mut world, 1, PLAYER, 0, 0, 0);
        world
            .objects
            .get_component_mut::<Player>(&PLAYER)
            .unwrap()
            .level = 65;
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {quest}")),
        );
        handle_request_bypass_to_server(
            &mut world,
            1,
            &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} 30862-04.htm")),
        );
        assert_eq!(quest_cond(&world, PLAYER, quest), Some(1), "quest started");

        let mob = NPC_OID + 1;
        add_test_npc(&mut world, mob, SNAKE, "Monster", 65, 30, 0, 0);
        world
            .objects
            .get_component_mut::<Npc>(&mob)
            .unwrap()
            .champion = champion;
        world.forced_rolls.push_back(0); // give_item_randomly roll_f64 → hit
        crate::game_loop::death::npc_do_die(&mut world, mob, PLAYER);
        item_count(&world, PLAYER, SNAKE_SCALE)
    };

    let plain = scales_from_one_kill(false);
    let champ = scales_from_one_kill(true);
    assert_eq!(plain, 1, "a plain kill pays the script's flat 1 scale");
    assert_eq!(
        champ, 5,
        "a champion kill pays ChampionRewardsAmount (5.0) times as much"
    );
}

/// The master gate is re-read at the consumer, as everywhere else: a mob still
/// flagged from before `ChampionEnable` was turned off pays the plain amount.
#[test]
fn the_quest_item_multiplier_respects_the_master_gate() {
    const SNAKE_SCALE: i32 = 5868;
    const SNAKE: i32 = 20672;
    const PLAYER: i32 = 3001;
    let quest = "Q00358_IllegitimateChildOfTheGoddess";

    let (mut world, _db, _l) = quest_test_world();
    add_quest_items(&mut world, &[(SNAKE_SCALE, "Snake Scale", true)]);
    world.cfg.champion = ChampionConfig {
        enable: false,
        ..champion_on()
    };
    world.cfg.rates.rate_quest_drop = 1.0;

    add_test_npc(&mut world, NPC_OID, 30862, "Folk", 60, 100, 0, 0);
    let _rx = ingame_player(&mut world, 1, PLAYER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&PLAYER)
        .unwrap()
        .level = 65;
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {quest}")),
    );
    handle_request_bypass_to_server(
        &mut world,
        1,
        &bypass_body(&format!("npc_{NPC_OID}_Quest {quest} 30862-04.htm")),
    );

    let mob = NPC_OID + 1;
    add_test_npc(&mut world, mob, SNAKE, "Monster", 65, 30, 0, 0);
    world
        .objects
        .get_component_mut::<Npc>(&mob)
        .unwrap()
        .champion = true;
    world.forced_rolls.push_back(0);
    crate::game_loop::death::npc_do_die(&mut world, mob, PLAYER);
    assert_eq!(
        item_count(&world, PLAYER, SNAKE_SCALE),
        1,
        "ChampionEnable = False → no multiplier, even on a flagged mob"
    );
}

// ---------------------------------------------------------------------------
// The live spawn path
// ---------------------------------------------------------------------------

/// The lottery is only worth anything if the *real* spawn path runs it. Every
/// other test here either calls `roll_champion` directly or sets the flag by
/// hand, so none of them would notice `spawn_npc_entity` dropping the roll —
/// and the symptom of that would be exactly "champions are configured on but
/// no champion ever appears in the world".
#[test]
fn the_spawn_path_rolls_the_lottery_and_applies_the_multipliers() {
    let (mut world, ..) = cast_test_world();
    world.cfg.champion = champion_on(); // frequency = 100 → a certain roll

    let mut t = monster(40);
    t.id = 90_500;
    t.base_p_atk = 100.0;
    world.data.npc_data.insert_for_test(t.clone());

    let oid =
        crate::model::npc::spawn_npc_at(&mut world, 90_500, 0, 0, 0, 0).expect("the mob spawned");
    assert!(
        world
            .objects
            .get_component::<Npc>(&oid)
            .expect("a live mob")
            .champion,
        "spawn_npc_entity must run Attackable.onRespawn's lottery"
    );
    let p_atk = world
        .objects
        .get_component::<CombatStats>(&oid)
        .expect("its stats")
        .p_atk;
    let plain = crate::model::npc_finalized_stats(
        &world.data,
        &t,
        &Buffs::default(),
        crate::model::NpcStatMods::default(),
    )
    .0
    .p_atk;
    assert!(
        (p_atk - plain * 4.0).abs() < 0.001,
        "the spawn finalized its stats *with* the champion multipliers \
         ({p_atk} vs a plain {plain})"
    );

    // …and a zero frequency leaves the same spawn plain, so the assertion above
    // is reading the roll rather than an unconditional flag.
    world.cfg.champion.frequency = 0;
    let plain_oid = crate::model::npc::spawn_npc_at(&mut world, 90_500, 0, 0, 0, 0)
        .expect("the second mob spawned");
    assert!(
        !world
            .objects
            .get_component::<Npc>(&plain_oid)
            .expect("a live mob")
            .champion,
        "ChampionFrequency = 0 → never a champion"
    );
}

/// NPC weapon-conditioned passives (`<using kind>` masteries like 4415): the
/// stat pump applies when the template's `<equipment>` right hand matches the
/// condition, and contributes nothing bare-handed — Java evaluates the
/// condition against the template weapon, armor conditions stay false.
#[test]
fn npc_weapon_mastery_needs_the_template_weapon() {
    use crate::model::skill::{SkillEffect, StatModifierEffect};
    use crate::model::stats::{Stat, StatModifierType};

    let (mut world, ..) = cast_test_world();
    const SWORD: i32 = 9990;
    world
        .data
        .item_data
        .set_weapon_type_for_test(SWORD, crate::data::item_data::WeaponType::Sword);
    let mut mastery = passive_clan_test_skill(9403);
    mastery.effects = vec![SkillEffect::StatModifier(StatModifierEffect {
        stat: Stat::PhysicalAttack,
        mode: StatModifierType::Diff,
        amount: 50.0,
        armor_condition: 0,
        weapon_condition: crate::data::item_data::WeaponType::Sword.mask_bit(),
        qualifier: None,
        two_handed: false,
    })];
    world.data.skill_data.insert_for_test(mastery);

    let mut armed = monster(40);
    armed.base_p_atk = 100.0;
    armed.skill_list = vec![(9403, 1)];
    armed.rhand = SWORD;
    let mut bare = armed.clone();
    bare.rhand = 0;

    let with_sword = crate::model::npc_finalized_stats(
        &world.data,
        &armed,
        &Buffs::default(),
        crate::model::NpcStatMods::default(),
    )
    .0
    .p_atk;
    let barehanded = crate::model::npc_finalized_stats(
        &world.data,
        &bare,
        &Buffs::default(),
        crate::model::NpcStatMods::default(),
    )
    .0
    .p_atk;
    assert!(
        with_sword > barehanded,
        "the sword mastery applies only with the template sword ({with_sword} vs {barehanded})"
    );
    assert!(
        (with_sword - barehanded - 50.0).abs() < 0.001,
        "and by exactly its +50"
    );
}
