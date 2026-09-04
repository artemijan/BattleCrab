//! `admin/transforms.rs` and `admin/mounts.rs` — transforming and riding,
//! and what each does to the action bar and the equipped weapon.

use super::*;

/// `//ride_strider` mounts the GM (durable `mount_type`/`mount_npc_id` + a Ride
/// broadcast); `//unride` clears it.
#[test]
fn admin_ride_and_unride() {
    let (mut world, ..) = admin_world();
    let mut gm_rx = ingame_player_access(&mut world, 1, 8920, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("ride_strider"));
    let p = world.objects.get_component::<Player>(&8920).unwrap();
    assert_eq!(p.mount_type, 1, "strider = MountType 1");
    assert_eq!(p.mount_npc_id, 12526, "strider npc id");
    assert!(
        drain(&mut gm_rx)
            .iter()
            .any(|pk| pk[0] == server_packets::opcodes::RIDE),
        "Ride broadcast sent"
    );

    // Re-riding while mounted is refused (Java "already have a summon").
    on_packet(&mut world, 1, build_admin("ride_wolf"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .mount_type,
        1,
        "still on the strider"
    );

    on_packet(&mut world, 1, build_admin("unride"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8920)
            .unwrap()
            .mount_type,
        0,
        "dismounted"
    );
}

/// Java `AdminRide`'s `isMounted() || hasSummon()` gate runs before *every*
/// `//ride_*` branch — including the transform-based `//ride_horse` — and
/// `AdminTransform` refuses a mounted target with SM 2063: a strider rider
/// can't stack a horse or a polymorph on top of the mount.
#[test]
fn admin_mounted_blocks_horse_and_transform() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut gm_rx = ingame_player_access(&mut world, 1, 8925, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("ride_strider"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8925)
            .unwrap()
            .mount_type,
        1,
        "on the strider"
    );

    on_packet(&mut world, 1, build_admin("ride_horse"));
    {
        let p = world.objects.get_component::<Player>(&8925).unwrap();
        assert_eq!(p.transform_id, 0, "horse refused while mounted");
        assert_eq!(p.mount_type, 1, "still on the strider");
    }

    on_packet(&mut world, 1, build_admin("transform 106"));
    let p = world.objects.get_component::<Player>(&8925).unwrap();
    assert_eq!(p.transform_id, 0, "//transform refused while mounted");
    assert_eq!(p.mount_type, 1, "mount untouched");
}

/// `//transform` refuses **in water** (Java `player.isInWater()` → SM 2060).
///
/// The gate had no reader until `position::is_in_water` landed with the
/// water/swim work; the marker outlived its own blocker.
#[test]
fn admin_transform_refused_in_water() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    let mut rx = ingame_player_access(&mut world, 1, 8925, 100);
    drain(&mut rx);

    world.cfg.general.allow_water = true;
    // A water zone over the GM, then the revalidation Java runs on movement —
    // `checkWaterState` is what starts the drowning task, and that task (not
    // the zone) is what `Player.isInWater()` reports.
    insert_zone(
        &mut world,
        crate::data::zone_data::ZoneKind::Water,
        -1000,
        1000,
        -1000,
        1000,
    );
    // Go through `revalidate_zone`, not `check_water_state` directly: since
    // the hot-paths work the latter reads the cached `ZoneFlags` mask that
    // revalidation writes, rather than walking the zone grid itself.
    zones::revalidate_zone(&mut world, 8925, true);
    assert!(
        crate::game_loop::space::water::is_drowning_task_active(&world, 8925),
        "fixture must actually be drowning for this to mean anything"
    );
    drain(&mut rx);

    on_packet(&mut world, 1, build_admin("transform 106"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8925)
            .unwrap()
            .transform_id,
        0,
        "//transform refused in water"
    );
    assert!(
        ids_after_opcode(&drain(&mut rx), server_packets::opcodes::SYSTEM_MESSAGE)
            .contains(&server_packets::sm_ids::YOU_CANNOT_POLYMORPH_INTO_THE_DESIRED_FORM_IN_WATER),
        "and says why"
    );
}

/// `//ride_bike` transforms the GM (transform 20001): durable transform id +
/// display id, the run speed overridden to the template's, and the transform's
/// skills granted; `//unride` reverts all of it.
#[test]
fn admin_ride_bike_transforms_and_reverts() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    // Jet bike (20001) exists in the dist with run=170 + a Dismount skill.
    let bike = world
        .data
        .transforms
        .get(20001)
        .expect("jet bike transform loaded");
    let bike_run = bike.template(false).run_spd.expect("bike has a run speed");
    let bike_skill = bike
        .template(false)
        .skills
        .first()
        .map(|(id, _)| *id)
        .expect("bike grants a skill");

    let mut gm_rx = ingame_player_access(&mut world, 1, 8930, 100);
    drain(&mut gm_rx);
    let base_run = world
        .objects
        .get_component::<Speeds>(&8930)
        .unwrap()
        .run_spd;

    on_packet(&mut world, 1, build_admin("ride_bike"));
    {
        let p = world.objects.get_component::<Player>(&8930).unwrap();
        assert_eq!(p.transform_id, 20001, "transformed into the bike");
        assert_eq!(
            p.transform_display_id, 20001,
            "display id == id on this dist"
        );
    }
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8930)
            .unwrap()
            .run_spd,
        bike_run,
        "run speed overridden by the transform"
    );
    assert!(
        world
            .objects
            .get_component::<SkillBook>(&8930)
            .unwrap()
            .0
            .contains_key(&bike_skill),
        "transform skill granted"
    );

    // Re-transforming while transformed is refused (Java polymorph message).
    on_packet(&mut world, 1, build_admin("ride_horse"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8930)
            .unwrap()
            .transform_id,
        20001,
        "still the bike"
    );

    on_packet(&mut world, 1, build_admin("unride"));
    let p = world.objects.get_component::<Player>(&8930).unwrap();
    assert_eq!(p.transform_id, 0, "reverted");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8930)
            .unwrap()
            .run_spd,
        base_run,
        "run speed restored"
    );
    assert!(
        !world
            .objects
            .get_component::<SkillBook>(&8930)
            .unwrap()
            .0
            .contains_key(&bike_skill),
        "transform skill removed"
    );
}

/// The transform-granted Dismount skill (839, `DispelBySlot TRANSFORM,-1` in
/// the dist) reverts a GM `//ride_bike` transform even though no buff backs it
/// — Java's `DispelBySlot` dispels "transformations (buff and by GM)" via
/// `stopTransformation`, and that skill is the only in-client revert path for
/// a ride transform. Before the fix the dispel only swept the buff list, so
/// clicking "transform back" was a silent no-op and the player stayed a bike.
#[test]
fn dismount_skill_reverts_gm_ride_transform() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();

    let mut gm_rx = ingame_player_access(&mut world, 1, 8935, 100);
    drain(&mut gm_rx);
    let base_run = world
        .objects
        .get_component::<Speeds>(&8935)
        .unwrap()
        .run_spd;

    on_packet(&mut world, 1, build_admin("ride_bike"));
    assert_eq!(
        world
            .objects
            .get_component::<Player>(&8935)
            .unwrap()
            .transform_id,
        20001,
        "transformed into the bike"
    );

    // The dist parses 839 into a DispelBySlot with the TRANSFORM,-1 entry.
    let dismount = world
        .data
        .skill_data
        .get(839, 1)
        .expect("Dismount 839 parsed from dist")
        .clone();
    assert!(
        dismount.effects.iter().any(|e| matches!(
            e,
            model::skill::effects::SkillEffect::DispelBySlot { dispel }
                if dispel.iter().any(|(ty, lvl)| ty == "TRANSFORM" && *lvl < 0)
        )),
        "Dismount carries DispelBySlot TRANSFORM,-1, got {:?}",
        dismount.effects
    );

    crate::game_loop::skills::effects::apply_skill_effects(&mut world, 8935, 8935, &dismount);
    let p = world.objects.get_component::<Player>(&8935).unwrap();
    assert_eq!(p.transform_id, 0, "transform dispelled");
    assert_eq!(p.transform_display_id, 0, "display cleared");
    assert_eq!(
        world
            .objects
            .get_component::<Speeds>(&8935)
            .unwrap()
            .run_spd,
        base_run,
        "run speed restored"
    );
}

/// Transform-granted skills are session-only (Java `_transformSkills`, which
/// `storeSkills` never writes): a flush while transformed must not persist
/// them, and rows a pre-filter flush already leaked into `character_skills`
/// are dropped on restore. Before the fix an autosave during `//ride_bike`
/// wrote Dismount 839 + Dissonance 5437 as learned rows, and 5437's passive
/// (Accuracy -50, P./M. Atk -95%) then followed the character across every
/// relog.
#[test]
fn transform_skills_never_persist() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    let bike_skills: Vec<i32> = world
        .data
        .transforms
        .get(20001)
        .expect("jet bike transform loaded")
        .template(false)
        .skills
        .iter()
        .map(|&(id, _)| id)
        .collect();
    assert!(!bike_skills.is_empty(), "bike grants skills");

    let mut gm_rx = ingame_player_access(&mut world, 1, 8945, 100);
    drain(&mut gm_rx);
    on_packet(&mut world, 1, build_admin("ride_bike"));
    let book = world.objects.get_component::<SkillBook>(&8945).unwrap();
    for id in &bike_skills {
        assert!(
            book.0.contains_key(id),
            "skill {id} granted while transformed"
        );
    }

    // Flush mid-transform: the snapshot must not carry the transform skills.
    let save = build_save_data(&world, 8945).expect("save data");
    for id in &bike_skills {
        assert!(
            !save.skills.iter().any(|&(sid, _, _)| sid == *id),
            "transform skill {id} must not reach character_skills"
        );
    }

    // Restore with rows a pre-filter flush leaked: they're dropped, learned
    // skills survive.
    let mut chr = dummy_char(8946, "Poisoned");
    chr.skills = vec![(839, 1, 0), (5437, 2, 0), (1177, 1, 0)];
    Player::from_char(&world.data, &chr).spawn_into(&mut world);
    let book = world.objects.get_component::<SkillBook>(&8946).unwrap();
    assert!(
        !book.0.contains_key(&839),
        "stale Dismount dropped on restore"
    );
    assert!(
        !book.0.contains_key(&5437),
        "stale Dissonance dropped on restore"
    );
    assert!(book.0.contains_key(&1177), "learned skill survives restore");
}

/// Every `ExBasicActionList` (0xFE 0x60 00) in `pkts`, decoded back to its id
/// list, so a test can compare against the template it expects.
fn basic_action_lists(pkts: &[Vec<u8>]) -> Vec<Vec<i32>> {
    let mut out = Vec::new();
    for p in pkts {
        if p.len() < 7 || p[0] != 0xFE || p[1] != 0x60 || p[2] != 0x00 {
            continue;
        }
        let rd = |o: usize| i32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
        let count = rd(3) as usize;
        if p.len() < 7 + count * 4 {
            continue;
        }
        out.push((0..count).map(|i| rd(7 + i * 4)).collect());
    }
    out
}

/// Java `Transform.onTransform` sends `ExBasicActionList(template.actions)`,
/// and `onUntransform` sends `ExBasicActionList.STATIC_PACKET` — the client's
/// action bar becomes the form's own and is restored on the way out. All 174
/// templates on this dist carry an `<actions>` block, so the swap is the half
/// of the transform data a GM can reach on every single one of them.
#[test]
fn admin_transform_swaps_and_restores_the_action_bar() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    // The fixture world ships an *empty* ActionData, which would make the
    // restore leg below compare an empty bar against an empty bar and pass
    // while proving nothing. Load the real one.
    world.data.action_data = crate::data::ActionData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));

    // Transform 105 is one of the two forms a *player* can actually enter on
    // this dist (the Rabbits event casts it), which is why it is the one worth
    // pinning here rather than an admin-only id.
    let expected = world
        .data
        .transforms
        .get(105)
        .expect("transform 105 loaded")
        .template(false)
        .actions
        .clone();
    assert!(
        !expected.is_empty(),
        "the dist template carries an <actions> block"
    );
    let default_bar = world.data.action_data.action_ids().to_vec();
    assert_ne!(
        expected, default_bar,
        "the form's bar must differ from the default, or this test proves nothing"
    );

    let mut gm_rx = ingame_player_access(&mut world, 1, 8931, 100);
    drain(&mut gm_rx);

    on_packet(&mut world, 1, build_admin("transform 105"));
    let bars = basic_action_lists(&drain(&mut gm_rx));
    assert_eq!(
        bars.last(),
        Some(&expected),
        "transforming swaps the action bar for the template's <actions>"
    );

    on_packet(&mut world, 1, build_admin("untransform"));
    let bars = basic_action_lists(&drain(&mut gm_rx));
    assert_eq!(
        bars.last(),
        Some(&default_bar),
        "untransforming restores ExBasicActionList.STATIC_PACKET"
    );
}

/// Java `IStatFunction.calcWeaponBaseValue`: the transform's `<base>` values
/// replace the equipped weapon's for every form *except* `COMBAT` and
/// `MODE_CHANGE`, which keep the weapon. Both forms a player can enter on this
/// dist are on the transform-wins side of that line (105 = NON_COMBAT,
/// 20008 = RIDING_MODE).
#[test]
fn transform_base_replaces_the_weapon_only_for_non_combat_forms() {
    let (mut world, ..) = admin_world();
    world.data.transforms = crate::data::TransformData::load_from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/"
    ));
    world.data.skill_data = dist::skills_owned();
    // The fixture's single synthetic class template does not cover the test
    // player's class, so `recalculate_stats` would fall back to
    // `PlayerTemplate::default()` — every class base 0, which makes a ratio
    // assertion meaningless. Load the real templates.
    world.data.player_templates = dist::player_templates_owned();

    let non_combat = world.data.transforms.get(105).expect("105 loaded");
    assert!(
        !non_combat.kind.weapon_overrides_base(),
        "105 is NON_COMBAT — the transform's base wins"
    );
    let tf_p_atk = non_combat
        .template(false)
        .base
        .as_ref()
        .and_then(|b| b.p_atk)
        .expect("105 carries <base pAtk=…>");

    // A COMBAT form is the control: Java hands the weapon branch back to it.
    let combat = world.data.transforms.get(1).expect("1 loaded");
    assert!(
        combat.kind.weapon_overrides_base(),
        "transform 1 is COMBAT — the weapon wins"
    );

    let mut gm_rx = ingame_player_access(&mut world, 1, 8932, 100);
    drain(&mut gm_rx);
    let naked_p_atk = world
        .objects
        .get_component::<CombatStats>(&8932)
        .unwrap()
        .p_atk;
    // The finalizer is `base * STR bonus * levelMod` and only `base` moves, so
    // the expected total scales by exactly the ratio of the two bases. Deriving
    // it from the class template keeps the assertion honest whichever way the
    // numbers happen to fall.
    let class_base_p_atk = {
        let (class_id, base_class_id) = {
            let p = world.objects.get_component::<Player>(&8932).unwrap();
            (p.class_id, p.base_class_id)
        };
        // The same lookup `recalculate_stats` does, fallback included.
        world
            .data
            .player_templates
            .get(class_id)
            .or_else(|| world.data.player_templates.get(base_class_id))
            .expect("class template loaded")
            .base_p_atk as f64
    };
    assert!(
        class_base_p_atk > 0.0 && class_base_p_atk != tf_p_atk,
        "the two bases must differ, or this test proves nothing \
         (class {class_base_p_atk}, transform {tf_p_atk})"
    );

    on_packet(&mut world, 1, build_admin("transform 105"));
    let transformed = world
        .objects
        .get_component::<CombatStats>(&8932)
        .unwrap()
        .p_atk;
    let expected = naked_p_atk * tf_p_atk / class_base_p_atk;
    assert!(
        (transformed - expected).abs() < 1e-6,
        "the NON_COMBAT form's <base pAtk={tf_p_atk}> displaces the class base \
         {class_base_p_atk}: expected {expected}, got {transformed}"
    );

    on_packet(&mut world, 1, build_admin("untransform"));
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&8932)
            .unwrap()
            .p_atk,
        naked_p_atk,
        "reverting restores the untransformed base"
    );

    on_packet(&mut world, 1, build_admin("transform 1"));
    assert_eq!(
        world
            .objects
            .get_component::<CombatStats>(&8932)
            .unwrap()
            .p_atk,
        naked_p_atk,
        "a COMBAT form ignores <base> and keeps the weapon/class value"
    );
}
