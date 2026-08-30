//! G34 S9 — the effects and conditions the *live-reachability* pass surfaced.
//!
//! The coverage census ranks its residue by "reachable", i.e. referenced
//! anywhere in the datapack XML. Most of what that leaves is later-chronicle
//! content the dist carries but this server can never hand a player. Ranking
//! instead by what is genuinely obtainable — a spawned NPC, a drop off one, a
//! buylist/multisell/recipe/quest reward — left a short list of real Interlude
//! content, and these are its tests.

use super::*;
use crate::game_loop;

use crate::model::components::{Buffs, Position, SkillBook, Vitals};
use crate::model::skill::{
    AffectObject, AffectScope, CompanionKind, EscapeDest, OperateType, ResidenceType, Skill,
    SkillCondition, SkillEffect, TargetType,
};

const CASTER: i32 = 4101;
const CID: u32 = 1;

/// A minimal instant skill carrying exactly one effect.
fn instant(id: i32, effect: SkillEffect) -> Skill {
    Skill {
        id,
        level: 1,
        name: format!("S{id}"),
        operate_type: OperateType::Active,
        target_type: TargetType::Self_,
        affect_scope: AffectScope::Single,
        affect_object: AffectObject::All,
        effects: vec![effect],
        ..Default::default()
    }
}

fn land(world: &mut World, skill: &Skill, caster: i32, target: i32) {
    effects::apply_skill_effects(world, caster, target, skill);
}

// ---------------------------------------------------------------------------
// Escape — the residence destinations
// ---------------------------------------------------------------------------

/// A hall owned by `owner_id`, restarting at `owner_restart`.
fn hall(id: i32, owner_id: i32, owner_restart: (i32, i32, i32)) -> model::clan_hall::ClanHall {
    model::clan_hall::ClanHall {
        id,
        name: format!("Hall {id}"),
        grade: model::clan_hall::ClanHallGrade::None,
        hall_type: model::clan_hall::ClanHallType::Auctionable,
        min_bid: 0,
        lease: 0,
        deposit: 0,
        npcs: Vec::new(),
        doors: Vec::new(),
        owner_restart,
        banish: (0, 0, 0),
        owner_id,
        paid_until: 0,
    }
}

/// The bare minimum clan these tests read: an id and a castle.
fn clan_owning(id: i32, castle_id: i32) -> Clan {
    Clan {
        id,
        name: format!("C{id}"),
        leader_id: 0,
        level: 5,
        reputation_score: 0,
        castle_id,
        blood_alliance_count: 0,
        members: Vec::new(),
        skills: Default::default(),
        warehouse: Default::default(),
        char_penalty_expiry_time: 0,
        dissolving_expiry_time: 0,
        rank_privs: Default::default(),
        new_leader_id: 0,
        sub_pledges: Default::default(),
        ally_id: 0,
        ally_name: String::new(),
        ally_penalty_expiry_time: 0,
        ally_penalty_type: 0,
        crest_id: 0,
        crest_large_id: 0,
        ally_crest_id: 0,
    }
}

fn pos_of(world: &World, oid: i32) -> (i32, i32) {
    let (x, y, _) = crate::game_loop::helpers::pos_of(world, oid).expect("object has a position");
    (x, y)
}

/// Scroll of Escape: Clan Hall (2040) sends the owner to the hall's
/// `<ownerRestartPoint>` — the whole point of the scroll, and inert until S9
/// because only `escapeType=TOWN` had an arm.
#[test]
fn clan_hall_escape_lands_at_the_owned_halls_restart_point() {
    let (mut world, ..) = cast_test_world();
    with_town(&mut world);
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .clan_id = 77;
    world
        .clan_halls
        .insert(21, hall(21, 77, (-100, -200, -300)));

    let skill = instant(
        2040,
        SkillEffect::Escape {
            dest: EscapeDest::ClanHall,
        },
    );
    land(&mut world, &skill, CASTER, CASTER);
    assert_eq!(
        pos_of(&world, CASTER),
        (-100, -200),
        "the hall's owner restart point, not the town"
    );
}

/// `getTeleToLocation` only returns early when it *resolves* a residence, so a
/// clanless player burning a Scroll of Escape: Clan Hall is not left standing
/// there — they take the town escape. Getting this wrong would make the scroll
/// look broken rather than merely unhelpful.
#[test]
fn clan_hall_escape_falls_through_to_town_without_a_hall() {
    let (mut world, ..) = cast_test_world();
    with_town(&mut world);
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);

    let skill = instant(
        2040,
        SkillEffect::Escape {
            dest: EscapeDest::ClanHall,
        },
    );
    land(&mut world, &skill, CASTER, CASTER);
    assert_eq!(pos_of(&world, CASTER), (5000, 6000), "town fallback");
}

/// Scroll of Escape: Castle (1830) — the owning clan's castle respawn, and the
/// **chaotic** list instead once the player's reputation goes negative.
#[test]
fn castle_escape_honours_ownership_and_reputation() {
    for (reputation, expect) in [(0, (700, 800)), (-5, (900, 1000))] {
        let (mut world, ..) = cast_test_world();
        with_town(&mut world);
        let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
        {
            let p = world.objects.get_component_mut::<Player>(&CASTER).unwrap();
            p.clan_id = 77;
            p.reputation = reputation;
        }
        world.clans.insert(77, clan_owning(77, 3));
        world.data.castle_restart_points.insert(
            3,
            crate::data::castle_zone_data::CastleRespawnPoints {
                spawn: vec![(700, 800, 10)],
                chaotic: vec![(900, 1000, 10)],
                ..Default::default()
            },
        );

        let skill = instant(
            2041,
            SkillEffect::Escape {
                dest: EscapeDest::Castle,
            },
        );
        land(&mut world, &skill, CASTER, CASTER);
        assert_eq!(
            pos_of(&world, CASTER),
            expect,
            "reputation {reputation} picks the right list"
        );
    }
}

/// Owning no castle is a town escape, not a refusal.
#[test]
fn castle_escape_falls_through_to_town_without_a_castle() {
    let (mut world, ..) = cast_test_world();
    with_town(&mut world);
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);

    let skill = instant(
        2041,
        SkillEffect::Escape {
            dest: EscapeDest::Castle,
        },
    );
    land(&mut world, &skill, CASTER, CASTER);
    assert_eq!(pos_of(&world, CASTER), (5000, 6000), "town fallback");
}

// ---------------------------------------------------------------------------
// DispelAll — skill 4177 Cancellation
// ---------------------------------------------------------------------------

/// `stopAllEffects()` strips the buff bar wholesale, but spares an
/// `irreplacableBuff`. Until S9 the raid bosses that cast Cancellation stripped
/// nothing at all.
#[test]
fn dispel_all_strips_every_buff_but_spares_the_irreplacable_ones() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);

    // Two buffs: an ordinary one and one Java would keep (the port folds
    // `<irreplacableBuff>` into `stay_after_death`).
    for (id, stays) in [(9401, false), (9402, true)] {
        let mut s = instant(id, SkillEffect::StatModifier(Default::default()));
        s.abnormal_type = format!("T{id}");
        s.abnormal_time = 600;
        s.is_continuous = true;
        s.stay_after_death = stays;
        s.target_type = TargetType::Target;
        world.data.skill_data.insert_for_test(s.clone());
        land(&mut world, &s, CASTER, CASTER);
    }
    let before = world
        .objects
        .get_component::<Buffs>(&CASTER)
        .map_or(0, |b| b.0.iter().filter(|x| !x.passive).count());
    assert_eq!(before, 2, "both buffs landed");

    let cancel = instant(4177, SkillEffect::DispelAll);
    land(&mut world, &cancel, CASTER, CASTER);

    let left: Vec<i32> = live_buffs(&world, CASTER);
    assert_eq!(left, vec![9402], "only the irreplacable buff survives");
}

// ---------------------------------------------------------------------------
// GiveSp — SP Scrolls, Primeval Isle crystals
// ---------------------------------------------------------------------------

#[test]
fn give_sp_credits_the_caster() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    let before = world.objects.get_component::<Player>(&CASTER).unwrap().sp;

    let skill = instant(2167, SkillEffect::GiveSp { sp: 5_000 });
    land(&mut world, &skill, CASTER, CASTER);

    let after = world.objects.get_component::<Player>(&CASTER).unwrap().sp;
    assert_eq!(after - before, 5_000, "the scroll's flat SP grant");
}

/// Java's guard is `effected.isAlikeDead()` — a corpse gets nothing, which
/// also means the effector is not paid for casting at one.
#[test]
fn give_sp_pays_nothing_when_the_target_is_dead() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    world
        .objects
        .get_component_mut::<Vitals>(&CASTER)
        .unwrap()
        .dead = true;
    let before = world.objects.get_component::<Player>(&CASTER).unwrap().sp;

    let skill = instant(2167, SkillEffect::GiveSp { sp: 5_000 });
    land(&mut world, &skill, CASTER, CASTER);

    assert_eq!(
        world.objects.get_component::<Player>(&CASTER).unwrap().sp,
        before
    );
}

// ---------------------------------------------------------------------------
// SetSkill — the Ancient Book: Divine Inspiration family
// ---------------------------------------------------------------------------

#[test]
fn set_skill_grants_the_skill_and_never_downgrades_it() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);

    let book3 = instant(
        9216,
        SkillEffect::SetSkill {
            skill_id: 1405,
            skill_level: 3,
        },
    );
    land(&mut world, &book3, CASTER, CASTER);
    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&CASTER)
            .and_then(|b| b.0.get(&1405).copied()),
        Some(3),
        "Divine Inspiration granted at the book's level"
    );

    // A lower-level book must not undo it (Java's `addSkill` replaces by id).
    let book1 = instant(
        9214,
        SkillEffect::SetSkill {
            skill_id: 1405,
            skill_level: 1,
        },
    );
    land(&mut world, &book1, CASTER, CASTER);
    assert_eq!(
        world
            .objects
            .get_component::<SkillBook>(&CASTER)
            .and_then(|b| b.0.get(&1405).copied()),
        Some(3),
        "the higher level is kept"
    );
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

fn refuses(world: &World, caster: i32, skill: &Skill, target: i32) -> bool {
    conditions::check_cast(world, caster, skill, target).is_err()
}

/// `OpHome` is the blessed scrolls' gate: unlike the plain scroll they refuse
/// outright rather than falling through to town.
#[test]
fn op_home_gates_on_actually_owning_the_residence() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    let mut skill = instant(
        2177,
        SkillEffect::Escape {
            dest: EscapeDest::ClanHall,
        },
    );
    skill.conditions = vec![SkillCondition::Home {
        residence: ResidenceType::ClanHall,
    }];

    assert!(refuses(&world, CASTER, &skill, CASTER), "no clan, no hall");

    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .clan_id = 77;
    assert!(
        refuses(&world, CASTER, &skill, CASTER),
        "a clan without a hall is still refused"
    );

    world
        .clan_halls
        .insert(21, hall(21, 77, (-100, -200, -300)));
    assert!(
        !refuses(&world, CASTER, &skill, CASTER),
        "hall owner passes"
    );
}

/// `OpAlignment` on the caster — the race-village Scrolls of Escape are
/// `LAWFUL`-only, so a PK cannot scroll home.
#[test]
fn op_alignment_reads_the_casters_reputation() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    let mut lawful = instant(
        2213,
        SkillEffect::Escape {
            dest: EscapeDest::Town,
        },
    );
    lawful.conditions = vec![SkillCondition::Alignment {
        affect: model::skill::AffectType::Caster,
        chaotic: false,
    }];

    assert!(!refuses(&world, CASTER, &lawful, CASTER), "clean player");
    world
        .objects
        .get_component_mut::<Player>(&CASTER)
        .unwrap()
        .reputation = -1;
    assert!(refuses(&world, CASTER, &lawful, CASTER), "a PK is refused");
}

/// `OpSkill` asks the caster's own book for an **exact** level, and its
/// negative form is "not at that level" rather than "absent" — which is what
/// keeps an Ancient Book usable at every level below the one it grants.
#[test]
fn op_skill_matches_an_exact_level_on_the_casters_own_book() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    let mut book = instant(
        9217,
        SkillEffect::SetSkill {
            skill_id: 1405,
            skill_level: 4,
        },
    );
    book.conditions = vec![SkillCondition::SkillKnown {
        skill_id: 1405,
        skill_level: 4,
        has_learned: false,
    }];

    // Not known at all → "not at level 4" → the book may be used.
    assert!(!refuses(&world, CASTER, &book, CASTER));
    // Known at 3 → still not level 4 → still usable.
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(1405, 3);
    assert!(!refuses(&world, CASTER, &book, CASTER));
    // Known at exactly 4 → the book is spent, and refuses.
    world
        .objects
        .get_component_mut::<SkillBook>(&CASTER)
        .unwrap()
        .0
        .insert(1405, 4);
    assert!(refuses(&world, CASTER, &book, CASTER));
}

/// `OpCompanion` with `PET` wants an actual collar pet, not merely any target.
#[test]
fn op_companion_pet_requires_a_pet_target() {
    let (mut world, ..) = cast_test_world();
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    let mut scroll = instant(2179, SkillEffect::DispelAll);
    scroll.target_type = TargetType::Target;
    scroll.conditions = vec![SkillCondition::Companion {
        kind: CompanionKind::Pet,
    }];
    assert!(
        refuses(&world, CASTER, &scroll, CASTER),
        "a player is not a pet"
    );
}

// ---------------------------------------------------------------------------
// Grow — the NPC swell on Might / Ultimate Buff / Berserker Spirit
// ---------------------------------------------------------------------------

/// `Grow.onStart` swells the cylinder, `onExit` puts it back. It is not merely
/// cosmetic: the collision radius feeds every reach test, so a grown mob really
/// does swing from further out.
#[test]
fn grow_swaps_the_npc_collision_cylinder_and_restores_it() {
    use crate::game_loop;
    use crate::model::components::Collision;

    let (mut world, ..) = cast_test_world();
    // A template with both cylinders, shaped like Timak Orc Prefect (20588),
    // registered before `add_test_npc` so it is not overwritten by the default.
    const NPC: i32 = 20588;
    // NPC object ids live above `FIRST_NPC_OBJECT_ID`; a player-range id
    // makes `is_npc_oid` take the player branch and the buff never lands.
    const MOB: i32 = game_loop::npc::FIRST_NPC_OBJECT_ID + 501;
    {
        let mut tpl = crate::data::npc_data::default_template(NPC);
        tpl.type_name = "Monster".into();
        tpl.level = 40;
        tpl.base_hp_max = 100.0;
        tpl.base_mp_max = 50.0;
        tpl.collision_radius = 12.0;
        tpl.collision_height = 24.0;
        tpl.collision_radius_grown = 14.5;
        tpl.collision_height_grown = 28.8;
        world.data.npc_data.insert_for_test(tpl);
    }
    add_test_npc(&mut world, MOB, NPC, "Monster", 40, 0, 0, 0);
    // `Npc::for_test` hard-codes its cylinder; start from the template's, the
    // way a real spawn does.
    {
        let c = world
            .objects
            .get_component_mut::<Collision>(&MOB)
            .expect("collision");
        c.radius = 12.0;
        c.height = 24.0;
    }

    let mut might = instant(4028, SkillEffect::Grow);
    might.abnormal_type = "PA_UP".into();
    might.abnormal_time = 300;
    might.is_continuous = true;
    might.target_type = TargetType::Target;
    world.data.skill_data.insert_for_test(might.clone());

    land(&mut world, &might, MOB, MOB);
    let c = *world.objects.get_component::<Collision>(&MOB).unwrap();
    assert_eq!((c.radius, c.height), (14.5, 28.8), "grown while buffed");

    effects::handle_buff_expire(&mut world, MOB, 4028);
    let c = *world.objects.get_component::<Collision>(&MOB).unwrap();
    assert_eq!((c.radius, c.height), (12.0, 24.0), "restored on exit");
}

// ---------------------------------------------------------------------------
// TeleportToTarget — skill 4671, the Splendor mobs' gap-closer
// ---------------------------------------------------------------------------

/// The caster lands 25 units **behind** the target — the target's own heading,
/// flipped. With the target facing +x (heading 0) that puts the caster 25 units
/// *down* the x axis from it, not on top of it and not in front.
#[test]
fn teleport_to_target_puts_the_caster_behind_its_target() {
    let (mut world, ..) = cast_test_world();
    const MOB: i32 = game_loop::npc::FIRST_NPC_OBJECT_ID + 601;
    add_test_npc(&mut world, MOB, 21524, "Monster", 60, 3000, 0, 0);
    let _rx = ingame_player(&mut world, CID, CASTER, 0, 0, 0);
    // Heading 0 = facing +x, so "behind" is -x.
    world
        .objects
        .get_component_mut::<Position>(&CASTER)
        .unwrap()
        .heading = 0;

    let skill = instant(4671, SkillEffect::TeleportToTarget);
    land(&mut world, &skill, MOB, CASTER);

    let (x, y) = pos_of(&world, MOB);
    assert_eq!((x, y), (-25, 0), "25 units behind a target facing +x");
    // And the target itself has not moved — this is a dash, not a swap.
    assert_eq!(pos_of(&world, CASTER), (0, 0));
}

// ---------------------------------------------------------------------------
// Row 18: the live tail after S9 — the appearance potions, the clan message,
// the vitality-loss rate, and the five conditions behind them.
// ---------------------------------------------------------------------------

/// `ChangeFace` / `ChangeHairStyle` / `ChangeHairColor` — the Facelifting, Hair
/// Style Change and Dye potions, all Interlude items whose skills did nothing.
#[test]
fn the_appearance_potions_change_the_head_and_say_so() {
    use crate::model::skill::AppearancePart;

    let (mut world, ..) = cast_test_world();
    let mut rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    drain(&mut rx);

    for (part, value, read) in [
        (AppearancePart::Face, 2, 0usize),
        (AppearancePart::HairStyle, 3, 1),
        (AppearancePart::HairColor, 1, 2),
    ] {
        let skill = instant(2122, SkillEffect::ChangeAppearance { part, value });
        land(&mut world, &skill, CASTER, CASTER);
        let p = world
            .objects
            .get_component::<Player>(&CASTER)
            .expect("player");
        let got = [p.face, p.hair_style, p.hair_color][read];
        assert_eq!(got, value, "{part:?} written");
    }
    // `broadcastUserInfo()` — without it the client keeps drawing the old head.
    assert!(
        drain(&mut rx)
            .iter()
            .any(|p| p[0] == crate::network::user_info::OPCODE_USER_INFO),
        "the change is broadcast"
    );
}

/// `SendSystemMessageToClan` — Clan Gate (3632) tells the whole clan.
#[test]
fn a_clan_message_effect_reaches_every_online_member() {
    let (mut world, ..) = cast_test_world();
    let mut a_rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    let mut b_rx = ingame_caster(&mut world, CID + 1, CASTER + 1, 20, 0);
    let mut clan = clan_owning(90, 0);
    // `broadcastToOnlineMembers` walks the **roster**, so the two have to be
    // on it — being in-world with a matching `clan_id` is not enough.
    clan.members = [CASTER, CASTER + 1]
        .iter()
        .map(|&oid| model::clan::ClanMember {
            char_id: oid,
            name: format!("P{oid}"),
            level: 1,
            class_id: 0,
            sex: 0,
            race: 0,
            power_grade: 5,
            title: String::new(),
            pledge_type: 0,
            apprentice: 0,
            sponsor: 0,
        })
        .collect();
    world.clans.insert(90, clan);
    for oid in [CASTER, CASTER + 1] {
        world
            .objects
            .get_component_mut::<Player>(&oid)
            .expect("player")
            .clan_id = 90;
    }
    drain(&mut a_rx);
    drain(&mut b_rx);

    let skill = instant(
        3632,
        SkillEffect::SendSystemMessageToClan { message_id: 1524 },
    );
    land(&mut world, &skill, CASTER, CASTER);

    assert!(has_system_message(&drain(&mut a_rx), 1524), "the caster");
    assert!(has_system_message(&drain(&mut b_rx), 1524), "and the clan");
}

/// `VitalityPointsRate` — the herb's -10 % scales vitality **loss** only, and
/// a rate that reaches 0 stops the loss entirely (Java returns early).
#[test]
fn the_vitality_consume_rate_scales_only_the_loss() {
    use crate::model::stats::Stat;

    let (mut world, ..) = cast_test_world();
    let _rx = ingame_caster(&mut world, CID, CASTER, 0, 0);
    world.cfg.character.enable_vitality = true;
    world.cfg.rates.rate_vitality_lost = 1.0;
    world.cfg.rates.rate_vitality_gain = 1.0;

    let vit = |w: &World| {
        w.objects
            .get_component::<Player>(&CASTER)
            .expect("player")
            .vitality_points
    };
    let set = |w: &mut World, v: i32| {
        w.objects
            .get_component_mut::<Player>(&CASTER)
            .expect("player")
            .vitality_points = v;
    };

    // No buff: the loss lands in full.
    set(&mut world, 10_000);
    crate::game_loop::character::vitality::update_vitality_points(
        &mut world, CASTER, -1000, true, true,
    );
    assert_eq!(vit(&world), 9_000);

    // -50 %: half of it.
    world
        .objects
        .get_component_mut::<crate::model::components::StatModifiers>(&CASTER)
        .expect("mods")
        .mul
        .insert(Stat::VitalityConsumeRate, 0.5);
    set(&mut world, 10_000);
    crate::game_loop::character::vitality::update_vitality_points(
        &mut world, CASTER, -1000, true, true,
    );
    assert_eq!(vit(&world), 9_500);

    // …and the gain side is untouched by it.
    set(&mut world, 10_000);
    crate::game_loop::character::vitality::update_vitality_points(
        &mut world, CASTER, 1000, true, true,
    );
    assert_eq!(vit(&world), 11_000, "the rate is loss-only");

    // A rate of 0 bails out before anything is spent.
    world
        .objects
        .get_component_mut::<crate::model::components::StatModifiers>(&CASTER)
        .expect("mods")
        .mul
        .insert(Stat::VitalityConsumeRate, 0.0);
    set(&mut world, 10_000);
    crate::game_loop::character::vitality::update_vitality_points(
        &mut world, CASTER, -1000, true, true,
    );
    assert_eq!(vit(&world), 10_000, "no consumption at all");
}
