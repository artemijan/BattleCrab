//! SummonNpc symbols (G19, PLAN_G19_SYMBOLS.md): the `EffectPoint` totem a
//! ground cast drops, its fixed-rate `union_skill` pulses, the owner
//! exemption through `SummonerRef`, the 15 s lifetime, and the `OpExistNpc`
//! re-cast gate.

use super::*;
use crate::game_loop::abnormal::has_buff;
use crate::model::components::{Casting, SummonerRef};
use crate::model::skill::{
    AffectObject, AffectScope, OpExistNpcCondition, OperateType, Skill, SkillCondition,
    SkillEffect, TargetType,
};
use crate::model::stats::{Stat, StatModifierType};

const CASTER: i32 = 2001;
const BYSTANDER: i32 = 2002;
const CID: u32 = 1;
const BID: u32 = 2;

/// Fixture ids well away from anything real.
const TOTEM_NPC: i32 = 91300;
const SYMBOL_SKILL: i32 = 9300;
const AURA_SKILL: i32 = 9301;

/// The totem template: `EffectPoint`, pulsing `AURA_SKILL` every 200 ms,
/// living 1 s — the dist shape (2 s / 15 s) on a test-sized clock.
fn register_totem_template(world: &mut World) {
    let mut t = crate::data::npc_data::default_template(TOTEM_NPC);
    t.type_name = "EffectPoint".into();
    t.level = 70;
    t.base_hp_max = 2444.0;
    t.base_mp_max = 1345.0;
    t.ai_params.insert("skill_delay".into(), "0.2".into());
    t.ai_params.insert("despawn_time".into(), "1".into());
    t.ai_skill_params
        .insert("union_skill".into(), (AURA_SKILL, 1));
    world.data.npc_data.insert_for_test(t);
}

/// The aura the totem pulses: a SELF + POINT_BLANK + NOT_FRIEND speed debuff,
/// the shape of Day of Doom 5145.
fn aura_skill() -> Skill {
    Skill {
        self_continuous: false,
        id: AURA_SKILL,
        name: "Test Seal Aura".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        affect_scope: AffectScope::PointBlank,
        affect_object: AffectObject::NotFriend,
        affect_range: 200,
        effect_point: -100,
        is_continuous: true,
        is_debuff: true,
        abnormal_time: 120,
        abnormal_level: 1,
        abnormal_type: "MULTI_DEBUFF".into(),
        magic_type: 2,
        effects: vec![SkillEffect::StatModifier(
            crate::model::skill::StatModifierEffect {
                stat: Stat::RunSpeed,
                mode: StatModifierType::Per,
                amount: -50.0,
                armor_condition: 0,
                weapon_condition: 0,
                qualifier: None,
                two_handed: false,
            },
        )],
        ..Default::default()
    }
}

/// The symbol skill: a GROUND cast whose only effect drops the totem.
fn symbol_skill() -> Skill {
    Skill {
        self_continuous: false,
        id: SYMBOL_SKILL,
        name: "Test Symbol".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Ground,
        affect_scope: AffectScope::Single,
        cast_range: 900,
        effect_range: 1000,
        hit_time: 500,
        magic_type: 2,
        effects: vec![SkillEffect::SummonNpc {
            npc_id: TOTEM_NPC,
            npc_count: 1,
            despawn_delay: 0,
        }],
        ..Default::default()
    }
}

fn ground_body(x: i32, y: i32, z: i32, skill_id: i32) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_i32(x);
    w.write_i32(y);
    w.write_i32(z);
    w.write_i32(skill_id);
    w.write_i32(0);
    w.write_u8(0);
    w.into_bytes()
}

fn learn(world: &mut World, oid: i32, skill: &Skill) {
    world.data.skill_data.insert_for_test(skill.clone());
    world
        .objects
        .get_component_mut::<crate::model::components::SkillBook>(&oid)
        .unwrap()
        .0
        .insert(skill.id, 1);
}

/// The totem entity standing at (x, y), if any.
fn totem_at(world: &mut World) -> Option<i32> {
    world.npc_regions.values().flatten().copied().find(|oid| {
        world
            .objects
            .get_component::<crate::model::npc::Npc>(oid)
            .is_some_and(|n| n.npc_id == TOTEM_NPC)
    })
}

// ---------------------------------------------------------------------------
// Dist parse
// ---------------------------------------------------------------------------

/// Day of Doom 1422 as the dist writes it: a `SummonNpc` of totem 13028 gated
/// by `OpExistNpc` (ids 13018–13024, 200 around the caster, isAround=false),
/// and totem 13028's template carries the pulse parameters. The dist quirk —
/// 13028 itself is NOT in the gate's id list, only the Interlude-era symbol
/// ids are — is data, not a bug; ported as written.
#[test]
fn day_of_doom_parses_with_its_totem_and_gate() {
    let sd = dist::skills();
    let dod = sd.get(1422, 1).expect("Day of Doom lvl 1");
    assert!(
        dod.effects.iter().any(|e| matches!(
            e,
            SkillEffect::SummonNpc {
                npc_id: 13028,
                npc_count: 1,
                ..
            }
        )),
        "SummonNpc totem 13028: {:?}",
        dod.effects
    );
    // G34 S1: the gate is now a `SkillCondition` in the skill's GENERAL
    // condition list rather than a dedicated `Skill` field — one
    // representation, evaluated by `skills::conditions` with every other
    // condition.
    assert_eq!(
        dod.conditions,
        vec![SkillCondition::ExistNpc(OpExistNpcCondition {
            npc_ids: vec![13018, 13019, 13020, 13021, 13022, 13023, 13024],
            range: 200,
            is_around: false,
        })],
        "OpExistNpc parsed into the condition list"
    );

    let npcs = dist::npcs();
    let totem = npcs.get(13028).expect("totem 13028");
    assert_eq!(totem.type_name, "EffectPoint");
    assert_eq!(totem.ai_param_f64("skill_delay", 0.0), 2.0);
    assert_eq!(totem.ai_param_f64("despawn_time", 0.0), 15.0);
    assert_eq!(totem.ai_skill_params.get("union_skill"), Some(&(5145, 1)));
    // The dist declares 5145 in BOTH places — as the union_skill parameter
    // and again in `<skillList>` — so the parameter parse must not have eaten
    // the skill-list row (or vice versa).
    assert!(
        totem.skill_list.contains(&(5145, 1)),
        "the skillList row survives alongside the parameter"
    );
}

// ---------------------------------------------------------------------------
// The seal's life
// ---------------------------------------------------------------------------

/// The full arc: ground-cast the symbol → the totem stands at the point →
/// its pulses debuff the bystander in range but never the owner → it
/// despawns on time and the pulses stop.
#[test]
fn a_seal_pulses_its_aura_and_expires() {
    let (mut world, _db, _l) = cast_test_world();
    // The owner stands INSIDE the future aura radius (100 from the point) —
    // without the acting-player friendship hop they would be cursed too, so
    // the exemption assertion below actually bites.
    let _out = ingame_caster(&mut world, CID, CASTER, 400, 0);
    let _out2 = ingame_caster(&mut world, BID, BYSTANDER, 520, 0);
    register_totem_template(&mut world);
    world.data.skill_data.insert_for_test(aura_skill());
    learn(&mut world, CASTER, &symbol_skill());

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, SYMBOL_SKILL),
    );
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "the symbol cast starts"
    );

    // Past hit (500 ms) + cancel + the first pulses: 3 s covers spawn and a
    // few 200 ms pulses, while the totem (1 s lifetime from spawn) also dies
    // inside this window — so assert the effects, then the corpse.
    advance_ticks(&mut world, 12);
    let totem = totem_at(&mut world).expect("the totem spawned");
    assert_eq!(
        world
            .objects
            .get_component::<SummonerRef>(&totem)
            .map(|s| s.0),
        Some(CASTER),
        "the totem knows its owner"
    );
    let pos = world
        .objects
        .get_component::<Position>(&totem)
        .copied()
        .unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (500, 0),
        "it stands at the aimed point, not on the caster"
    );

    advance_ticks(&mut world, 5);
    assert!(
        has_buff(&world, BYSTANDER, AURA_SKILL),
        "the bystander 20 units from the seal is cursed"
    );
    assert!(
        !has_buff(&world, CASTER, AURA_SKILL),
        "the owner is exempt (acting-player friendship)"
    );

    // The 1 s lifetime runs out; the seal and its pulses go with it.
    advance_ticks(&mut world, 20);
    assert!(totem_at(&mut world).is_none(), "the seal despawned");
}

/// A player who walks into the seal's radius mid-lifetime is cursed by the
/// next pulse — the totem re-casts, it doesn't snapshot its victims.
#[test]
fn walking_into_a_live_seal_gets_cursed() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let _out2 = ingame_caster(&mut world, BID, BYSTANDER, 900, 0); // far away
    // Long-lived totem for this one, so the walk-in happens mid-life.
    world.data.npc_data.insert_for_test({
        let mut t = crate::data::npc_data::default_template(TOTEM_NPC);
        t.type_name = "EffectPoint".into();
        t.base_hp_max = 2444.0;
        t.base_mp_max = 1345.0;
        t.ai_params.insert("skill_delay".into(), "0.2".into());
        t.ai_params.insert("despawn_time".into(), "60".into());
        t.ai_skill_params
            .insert("union_skill".into(), (AURA_SKILL, 1));
        t
    });
    world.data.skill_data.insert_for_test(aura_skill());
    learn(&mut world, CASTER, &symbol_skill());

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, SYMBOL_SKILL),
    );
    advance_ticks(&mut world, 15);
    assert!(
        !has_buff(&world, BYSTANDER, AURA_SKILL),
        "at 400 units out, the first pulses miss"
    );

    world
        .objects
        .get_component_mut::<Position>(&BYSTANDER)
        .unwrap()
        .x = 520;
    advance_ticks(&mut world, 10);
    assert!(
        has_buff(&world, BYSTANDER, AURA_SKILL),
        "the next pulse catches the walk-in"
    );
}

// ---------------------------------------------------------------------------
// The OpExistNpc gate
// ---------------------------------------------------------------------------

/// With a listed totem within 200 of the **caster**, the re-cast is refused;
/// with the same totem 250 away it is allowed (Java sweeps around the caster,
/// not the aimed point).
#[test]
fn op_exist_npc_gates_recasting_next_to_a_seal() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 0, 0);
    register_totem_template(&mut world);
    world.data.skill_data.insert_for_test(aura_skill());
    let mut skill = symbol_skill();
    skill.conditions = vec![SkillCondition::ExistNpc(OpExistNpcCondition {
        npc_ids: vec![TOTEM_NPC],
        range: 200,
        is_around: false,
    })];
    learn(&mut world, CASTER, &skill);

    // A live listed totem 150 from the caster.
    add_test_npc(&mut world, NPC_OID, TOTEM_NPC, "EffectPoint", 70, 150, 0, 0);
    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, SYMBOL_SKILL),
    );
    assert!(
        !world.objects.has_component::<Casting>(&CASTER),
        "a listed seal within 200 of the caster refuses the cast"
    );

    // Move the existing totem out to 250: allowed.
    world
        .objects
        .get_component_mut::<Position>(&NPC_OID)
        .unwrap()
        .x = 250;
    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, SYMBOL_SKILL),
    );
    assert!(
        world.objects.has_component::<Casting>(&CASTER),
        "250 away is outside the 200 gate"
    );
}

/// **A seal wears its caster's name.** Java's `SummonNpc` handler calls
/// `effectPoint.setTitle(player.getName())`, which is the only way a bystander
/// can tell whose totem they are standing next to. The port's NPC titles are
/// template-derived, so this needs a per-instance override that wins over the
/// template *and* over the `ShowNpcLevel`/`ShowNpcAggression` decoration.
#[test]
fn a_seal_is_titled_with_its_casters_name() {
    let (mut world, _db, _l) = cast_test_world();
    let _out = ingame_caster(&mut world, CID, CASTER, 400, 0);
    register_totem_template(&mut world);
    world.data.skill_data.insert_for_test(aura_skill());
    learn(&mut world, CASTER, &symbol_skill());
    let caster_name = world
        .objects
        .get_component::<crate::model::Player>(&CASTER)
        .unwrap()
        .name
        .clone();

    crate::game_loop::skills::cast::handle_request_magic_skill_use_ground(
        &mut world,
        CID,
        &ground_body(500, 0, 0, SYMBOL_SKILL),
    );
    advance_ticks(&mut world, 8);
    let totem = totem_at(&mut world).expect("the totem spawned");

    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&totem)
            .and_then(|n| n.title_override.clone()),
        Some(caster_name.clone()),
        "the seal records whose it is"
    );
    // **Whether the client ever sees it is a separate gate**, and Java has the
    // same one: `NpcInfo` only emits the TITLE component for a template with
    // `usingServerSideTitle` (or a monster under ShowNpcLevel/Aggression). No
    // `EffectPoint` template on this dist sets that flag, so in Java the title
    // is stored and never transmitted either — the port matches, including the
    // pointlessness.
    let wire_title = |world: &World| {
        let view = crate::model::npc::NpcView::of(&world.objects, totem).unwrap();
        let template = world.data.npc_data.get(TOTEM_NPC).unwrap();
        let pkt = server_packets::npc_info(
            &view,
            template,
            &world.cfg.npc,
            &world.cfg.champion,
            &[],
            None,
        );
        // The name rides as UTF-16LE somewhere inside a mixed packet, so look
        // for its byte sequence rather than decoding the whole buffer (whose
        // string fields are not 2-byte aligned from offset 0).
        let needle: Vec<u8> = caster_name
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        pkt.windows(needle.len()).any(|w| w == needle)
    };
    assert!(
        !wire_title(&world),
        "a plain EffectPoint template sends no title block, as in Java"
    );

    // Opt the template in and the override is what goes out — not the
    // template's own title.
    {
        let mut t = world.data.npc_data.get(TOTEM_NPC).unwrap().clone();
        t.server_side_title = true;
        t.title = "Not This".into();
        world.data.npc_data.insert_for_test(t);
    }
    assert!(
        wire_title(&world),
        "with serverSideTitle on, NpcInfo carries the per-instance title"
    );
}

// ---------------------------------------------------------------------------
// The default plain-spawn branch (Holiday Trees, Squash seeds)
// ---------------------------------------------------------------------------

/// `SummonNpc`'s **default** branch — a non-`EffectPoint` template (the
/// Holiday Tree shape: `Folk` 13006 from item skill 2137) spawns as a plain
/// world NPC: linked to its summoner, registered in the player's
/// `SummonedNpcs`, wearing its own template name as its title, and despawning
/// after the effect's `despawnDelay`.
#[test]
fn plain_summon_spawns_folk_with_despawn() {
    use crate::model::components::{Position, SummonedNpcs};

    const TREE_NPC: i32 = 91301;
    const TREE_SKILL: i32 = 9302;

    let (mut world, _db, _l) = cast_test_world();
    let _rx = ingame_caster(&mut world, CID, CASTER, 0, 0);

    let mut t = crate::data::npc_data::default_template(TREE_NPC);
    t.type_name = "Folk".into();
    t.name = "Holiday Tree".into();
    t.level = 1;
    t.base_hp_max = 30.0;
    t.base_mp_max = 30.0;
    world.data.npc_data.insert_for_test(t);

    let tree = Skill {
        self_continuous: false,
        id: TREE_SKILL,
        name: "Summon Regular Tree".into(),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        effects: vec![SkillEffect::SummonNpc {
            npc_id: TREE_NPC,
            npc_count: 1,
            despawn_delay: 1_000,
        }],
        ..Default::default()
    };
    effects::apply_skill_effects(&mut world, CASTER, CASTER, &tree);

    let tree_oid = world
        .npc_regions
        .values()
        .flatten()
        .copied()
        .find(|oid| {
            world
                .objects
                .get_component::<crate::model::npc::Npc>(oid)
                .is_some_and(|n| n.npc_id == TREE_NPC)
        })
        .expect("the tree spawned");
    assert_eq!(
        world
            .objects
            .get_component::<SummonerRef>(&tree_oid)
            .map(|s| s.0),
        Some(CASTER),
        "linked to its summoner"
    );
    assert!(
        world
            .objects
            .get_component::<SummonedNpcs>(&CASTER)
            .is_some_and(|s| s.0.contains(&tree_oid)),
        "registered in the player's summoned-NPC list"
    );
    assert_eq!(
        world
            .objects
            .get_component::<crate::model::npc::Npc>(&tree_oid)
            .unwrap()
            .title_override
            .as_deref(),
        Some("Holiday Tree"),
        "a plain summon wears its template name as its title"
    );
    let caster_pos = *world.objects.get_component::<Position>(&CASTER).unwrap();
    let tree_pos = *world.objects.get_component::<Position>(&tree_oid).unwrap();
    assert_eq!(
        (tree_pos.x, tree_pos.y, tree_pos.z),
        (caster_pos.x, caster_pos.y, caster_pos.z),
        "spawned at the effected player's position"
    );

    // `scheduleDespawn(despawnDelay)` — 1 s on the test clock.
    advance_ticks(&mut world, 15);
    assert!(
        !world
            .objects
            .has_component::<crate::model::npc::Npc>(&tree_oid),
        "the despawnDelay removed the tree"
    );
}
