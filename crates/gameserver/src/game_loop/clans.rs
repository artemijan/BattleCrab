//! Clans — the G11 creation/display slice: `ClanTable.createClan` behind
//! the village-master `create_clan` bypass, the pledge-window packets, the
//! enter/leave-world roster notifications, the Clan Advent leader-online aura
//! (`ClanMaster`'s login/logout listeners), and clan (pledge) skills —
//! `//give_clan_skills`, re-applied to each member on login and stripped on
//! dispersal (Java `Clan.addNewSkill`/`addSkillEffects`/`removeSkillEffects`).
//! G18 slice 1 adds the membership lifecycle: invite (`RequestJoinPledge` /
//! `RequestAnswerJoinPledge` through the `PendingRequest` transaction slot),
//! leave (`RequestWithdrawalPledge`), oust (`RequestOustPledgeMember`), and
//! the village-master `dissolve_clan`/`recover_clan` verbs with the delayed
//! `ScheduledTask::ClanDissolve` removal — see PLAN_G18_CLANS.md.
//! Wars/levels/crests/ranks/sub-pledges stay deferred (later G18 slices).

use commons::network::PacketReader;
use tracing::warn;

use crate::db::DbCommand;
use crate::model::clan::{Clan, ClanMember, ALL_CLAN_PRIVILEGES, CL_DISMISS, CL_JOIN_CLAN};
use crate::model::components::ClanSkills;
use crate::model::skill::ActiveBuff;
use crate::model::Player;
use crate::network::server_packets::{self, sm_ids, SmParam};
use crate::world::World;

use super::helpers::client_for_player;

fn send_sm(world: &World, client_id: u32, id: i16) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(crate::network::enter_world::system_message(id));
    }
}

/// Wall-clock millis (Java `System.currentTimeMillis()`).
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `DaysBeforeCreateAClan = 10` on this dist → the recreate cooldown in millis.
const CLAN_CREATE_COOLDOWN_MS: i64 = 10 * 86_400_000;

/// `CommonSkill.CLAN_ADVENT` (skill 19009 lv.1): the clan-leader-online aura —
/// PAtk/PDef/MDef +5%, MAtk +6%, HP/MP regen +5 on every clan member while the
/// leader is logged in. `abnormalTime=-1` (permanent) + `irreplacableBuff`, so
/// it lasts until explicitly removed on the leader's logout / clan dispersal.
const CLAN_ADVENT_SKILL_ID: i32 = 19009;
const CLAN_ADVENT_SKILL_LEVEL: i32 = 1;

/// The clan's member object-ids that are currently online (leader included).
fn online_members(world: &World, clan_id: i32) -> Vec<i32> {
    let Some(clan) = world.clans.get(&clan_id) else {
        return Vec::new();
    };
    clan.members
        .iter()
        .map(|m| m.char_id)
        .filter(|&oid| client_for_player(world, oid).is_some())
        .collect()
}

/// Java `ClanMaster`'s `CommonSkill.CLAN_ADVENT.getSkill().applyEffects(p, p)`:
/// self-cast Clan Advent onto one online member. Skipped when already present —
/// the buff is permanent and `irreplacableBuff` (a single abnormal slot), so a
/// re-trigger must not stack a second copy (Java's `EffectList` replaces in
/// place; skipping the identical permanent buff is equivalent).
fn apply_clan_advent(world: &mut World, object_id: i32) {
    let already = world
        .objects
        .get_component::<crate::model::components::Buffs>(&object_id)
        .is_some_and(|b| b.0.iter().any(|x| x.skill_id == CLAN_ADVENT_SKILL_ID));
    if already {
        return;
    }
    let Some(skill) = world
        .data
        .skill_data
        .get(CLAN_ADVENT_SKILL_ID, CLAN_ADVENT_SKILL_LEVEL)
        .cloned()
    else {
        return;
    };
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
}

/// Java `getEffectList().stopSkillEffects(REMOVED, CommonSkill.CLAN_ADVENT)`:
/// strip Clan Advent from one player (no-op if absent). Reuses the buff-expiry
/// path, which drops the buff, reverts its stat contribution, and rebroadcasts
/// UserInfo + the abnormal-status row.
fn remove_clan_advent(world: &mut World, object_id: i32) {
    crate::game_loop::skills::effects::handle_buff_expire(world, object_id, CLAN_ADVENT_SKILL_ID);
}

/// Java `ClanMaster.onPlayerLogin`: the leader's login lights the Clan Advent
/// aura on every online member; a plain member's login lights it on themselves
/// only if the leader is already online. (Java's `ON_PLAYER_CLAN_JOIN` /
/// `ON_PLAYER_PROFESSION_CHANGE` refreshers stay TODO — clan invites and the
/// subclass system aren't ported, so the login/logout pair is the whole surface
/// that can fire today.)
fn apply_clan_advent_on_login(world: &mut World, clan_id: i32, object_id: i32) {
    let is_leader = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.leader_id == object_id);
    if is_leader {
        for oid in online_members(world, clan_id) {
            apply_clan_advent(world, oid);
        }
    } else {
        let leader_online = world
            .clans
            .get(&clan_id)
            .map(|c| c.leader_id)
            .is_some_and(|lid| client_for_player(world, lid).is_some());
        if leader_online {
            apply_clan_advent(world, object_id);
        }
    }
}

// --- Clan skills (pledge skill tree): `//give_clan_skills` + login re-apply ---

/// Java `addSkillEffects`'s per-skill gate: a member receives clan skill
/// `(id, level)` when it has no `<socialClass>` requirement, or their pledge
/// class clears it (`pledgeClass + 1 >= socialClass.ordinal()`).
fn member_qualifies_for_clan_skill(
    world: &World,
    clan_id: i32,
    member_oid: i32,
    skill_id: i32,
    level: i32,
) -> bool {
    let Some(req) = world
        .data
        .pledge_skill_trees
        .social_class_of(skill_id, level)
    else {
        return true;
    };
    let pledge_class = world
        .clans
        .get(&clan_id)
        .map(|c| c.pledge_class_of(member_oid))
        .unwrap_or(0);
    pledge_class as i32 + 1 >= req as i32
}

/// Fold a clan (passive) skill's stat effects onto a member as a hidden
/// permanent buff — the `passive: true` route [`crate::model::
/// conditioned_passive_buffs`] uses, so it contributes to stats without an
/// abnormal-status icon. Records it in [`ClanSkills`] (transient, never written
/// to `character_skills`). Replaces any lower-level instance; a no-op if already
/// applied at `level`.
fn apply_clan_skill_to_member(world: &mut World, member_oid: i32, skill_id: i32, level: i32) {
    let existing = world
        .objects
        .get_component::<ClanSkills>(&member_oid)
        .and_then(|c| c.0.get(&skill_id).copied());
    if existing == Some(level) {
        return;
    }
    if existing.is_some() {
        // Drop the old level's passive buff before re-adding (EffectList single slot).
        crate::game_loop::skills::effects::handle_buff_expire(world, member_oid, skill_id);
    }
    if let Some(c) = world.objects.get_component_mut::<ClanSkills>(&member_oid) {
        c.0.insert(skill_id, level);
    }
    let effects = world
        .data
        .skill_data
        .get(skill_id, level)
        .map(|s| s.stat_modifier_effects())
        .unwrap_or_default();
    if effects.is_empty() {
        return; // known for the skill list, but nothing to fold into stats
    }
    let buff = ActiveBuff {
        skill_id,
        skill_level: level,
        abnormal_type_client_id: -1,
        abnormal_type: "NONE".to_string(),
        abnormal_level: 0,
        slot: crate::model::skill::BuffSlot::Uncapped,
        expires_at_tick: u64::MAX,
        passive: true,
        // Synthetic buff (passive/clan/expertise pump): no abnormal state.
        effect_flags: 0,
        blocked_abnormals: Vec::new(),
        abnormal_visuals: Vec::new(),
        effects,
    };
    apply_permanent_passive_buff(world, member_oid, buff);
}

/// The buff-application half of `apply_skill_effects` for a permanent passive:
/// fold the effects into the member's stat maps, then rebroadcast UserInfo (no
/// AbnormalStatusUpdate — passive buffs carry no icon).
fn apply_permanent_passive_buff(world: &mut World, oid: i32, buff: ActiveBuff) {
    use crate::model::components::{BaseStats, Buffs, CombatStats, Speeds, StatModifiers};
    use crate::model::inventory::Inventory;
    if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) =
        world.objects.get_many_mut::<(
            &mut Player,
            &BaseStats,
            &mut StatModifiers,
            &Inventory,
            &mut Buffs,
            &mut Speeds,
            &mut CombatStats,
        )>(&oid)
    {
        target.apply_buff(
            &world.data,
            base,
            &mut mods,
            &inventory,
            &mut buffs,
            &mut speeds,
            &mut combat,
            buff,
        );
    }
    // Clan skills like Clan Health / Clan Mind carry MaxHp/MaxMp modifiers that
    // `recalculate_stats` doesn't consume — fold them into the vitals too.
    crate::game_loop::skills::effects::recompute_max_vitals(world, oid);
    super::party::broadcast_user_info(world, oid);
}

/// Resend a member's merged `SkillList` (own skills + clan skills).
fn refresh_member_skill_list(world: &World, member_oid: i32) {
    if let Some(cid) = client_for_player(world, member_oid) {
        if let Some(pkt) = super::helpers::skill_list_packet(world, member_oid) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt);
            }
        }
    }
}

/// The clan's full skill set as an `(id, level)` list (for `PledgeSkillList`).
fn clan_skill_pairs(world: &World, clan_id: i32) -> Vec<(i32, i32)> {
    world
        .clans
        .get(&clan_id)
        .map(|c| c.skills.iter().map(|(&k, &v)| (k, v)).collect())
        .unwrap_or_default()
}

/// Java `Clan.addSkillEffects(player)`: (re-)apply every qualifying clan skill to
/// one member, then resend their `SkillList` + the clan window's
/// `PledgeSkillList`. Called on member login (`on_enter_world`).
pub(crate) fn apply_clan_skills_to_member(world: &mut World, clan_id: i32, member_oid: i32) {
    let skills = clan_skill_pairs(world, clan_id);
    if skills.is_empty() {
        return;
    }
    let mut applied = false;
    for (id, level) in skills {
        // Residence skills (a castle/clan-hall benefit) are never applied through
        // the pledge-grant channel — guards against legacy rows a pre-fix grant
        // persisted, so they don't re-apply on login. TODO(G24): residence
        // ownership applies these through its own path.
        if world.data.pledge_skill_trees.is_residence_skill(id) {
            continue;
        }
        if member_qualifies_for_clan_skill(world, clan_id, member_oid, id, level) {
            apply_clan_skill_to_member(world, member_oid, id, level);
            applied = true;
        }
    }
    if applied {
        refresh_member_skill_list(world, member_oid);
    }
    // The clan window's skill tab (Java sends `PledgeSkillList` on enter-world).
    if let Some(cid) = client_for_player(world, member_oid) {
        let pkt = server_packets::pledge_skill_list(&clan_skill_pairs(world, clan_id));
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(pkt);
        }
    }
}

/// Java `SiegeManager.getSiegeClanMinLevel()` (siege.config default): a clan
/// leader gets the siege/leader skills once the clan reaches this level.
const SIEGE_CLAN_MIN_LEVEL: i32 = 5;

/// Port of `SiegeManager.addSiegeSkills` — the clan-leader-only skills the
/// client files under the "Clan" skill tab: Imprint of Light/Darkness and Build
/// Headquarters, plus the two Outpost skills once the clan owns a castle. Java
/// adds them with `addSkill(sk, false)` (transient, not persisted); we mirror
/// that through the [`ClanSkills`] channel — they're active skills with no stat
/// effects, so this only registers them for the merged `SkillList`.
///
/// Advanced Headquarters (326) is noble-only and comes from the *noble* skill
/// tree (G17), not from this clan-siege set, so it is still skipped here.
/// TODO(G24): the cast behaviour (flag/HQ spawn, castle engrave)
/// lands with the siege-combat milestone; this only makes the skills appear.
fn apply_siege_skills_to_leader(world: &mut World, clan_id: i32, member_oid: i32) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.leader_id != member_oid || clan.level < SIEGE_CLAN_MIN_LEVEL {
        return;
    }
    let has_castle = clan.castle_id > 0;
    let mut ids = vec![
        19034, // Imprint of Light
        19035, // Imprint of Darkness
        247,   // Build Headquarters
    ];
    if has_castle {
        ids.push(844); // Outpost Construction
        ids.push(845); // Outpost Demolition
    }
    for id in ids {
        apply_clan_skill_to_member(world, member_oid, id, 1);
    }
    refresh_member_skill_list(world, member_oid);
}

/// Java `Clan.removeSkillEffects(player)` — strip every clan skill from a member
/// (clan left / dispersed): revert each passive buff and clear [`ClanSkills`].
pub(crate) fn remove_clan_skills_from_member(world: &mut World, member_oid: i32) {
    let ids: Vec<i32> = world
        .objects
        .get_component::<ClanSkills>(&member_oid)
        .map(|c| c.0.keys().copied().collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return;
    }
    for id in ids {
        crate::game_loop::skills::effects::handle_buff_expire(world, member_oid, id);
    }
    if let Some(c) = world.objects.get_component_mut::<ClanSkills>(&member_oid) {
        c.0.clear();
    }
    refresh_member_skill_list(world, member_oid);
}

/// Java `Clan.addNewSkill` for one skill: store it on the clan, persist it, and
/// push it to every qualifying online member (buff + skill list +
/// `PledgeSkillListAdd` + "clan skill added" message).
fn add_clan_skill(world: &mut World, clan_id: i32, skill_id: i32, level: i32) {
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.skills.insert(skill_id, level);
    }
    let name = world
        .data
        .skill_data
        .get(skill_id, level)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let _ = world.db.send(DbCommand::SaveClanSkill {
        clan_id,
        skill_id,
        skill_level: level,
        skill_name: name,
    });
    for oid in online_members(world, clan_id) {
        if !member_qualifies_for_clan_skill(world, clan_id, oid, skill_id, level) {
            continue;
        }
        apply_clan_skill_to_member(world, oid, skill_id, level);
        if let Some(cid) = client_for_player(world, oid) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(server_packets::pledge_skill_list_add(skill_id, level));
                cs.send(server_packets::system_message_with(
                    sm_ids::THE_CLAN_SKILL_S1_HAS_BEEN_ADDED,
                    &[SmParam::SkillName {
                        id: skill_id,
                        level,
                    }],
                ));
            }
        }
        refresh_member_skill_list(world, oid);
    }
}

/// Java `AdminSkill.adminGiveClanSkills`: grant the clan every pledge skill it
/// qualifies for at its level (plus squad skills when `include_squad`), applying
/// each to online members. Returns how many skills were granted; the caller does
/// the target/clan/leader checks and the summary messages.
pub(crate) fn give_clan_skills(world: &mut World, clan_id: i32, include_squad: bool) -> usize {
    let clan_level = world.clans.get(&clan_id).map(|c| c.level).unwrap_or(0);

    // Self-heal: a pre-fix `//give_clan_skills` (which read the wrong
    // `residenceSkill` attribute) may have stored residence skills on the clan.
    // Those belong to castle/clan-hall ownership, never this grant — strip them
    // from the clan, revert them on online members, and delete the DB rows so a
    // relog doesn't re-apply them. TODO(G24): residence ownership grants these
    // through its own channel, not the pledge tree.
    let residence: Vec<i32> = world
        .clans
        .get(&clan_id)
        .map(|c| {
            c.skills
                .keys()
                .copied()
                .filter(|&id| world.data.pledge_skill_trees.is_residence_skill(id))
                .collect()
        })
        .unwrap_or_default();
    for id in residence {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.skills.remove(&id);
        }
        let _ = world.db.send(DbCommand::DeleteClanSkill {
            clan_id,
            skill_id: id,
        });
        for oid in online_members(world, clan_id) {
            if world
                .objects
                .get_component::<ClanSkills>(&oid)
                .is_some_and(|c| c.0.contains_key(&id))
            {
                crate::game_loop::skills::effects::handle_buff_expire(world, oid, id);
                if let Some(c) = world.objects.get_component_mut::<ClanSkills>(&oid) {
                    c.0.remove(&id);
                }
            }
        }
    }

    let current: std::collections::HashMap<i32, i32> = world
        .clans
        .get(&clan_id)
        .map(|c| c.skills.clone())
        .unwrap_or_default();
    let to_add =
        world
            .data
            .pledge_skill_trees
            .max_pledge_skills(clan_level, &current, include_squad);
    for &(skill_id, level) in &to_add {
        add_clan_skill(world, clan_id, skill_id, level);
    }
    // Re-apply every owned clan skill to each online member (idempotent — the
    // `apply_clan_skill_to_member` level check no-ops when already applied). This
    // makes the grant take effect immediately with no relog even when the clan
    // already owned the skills (`to_add` empty), which is how a saturated clan
    // ends up otherwise showing nothing changed.
    for oid in online_members(world, clan_id) {
        apply_clan_skills_to_member(world, clan_id, oid);
    }
    // Java broadcasts the full `PledgeSkillList` to online members afterward.
    let pkt = server_packets::pledge_skill_list(&clan_skill_pairs(world, clan_id));
    for oid in online_members(world, clan_id) {
        if let Some(cid) = client_for_player(world, oid) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt.clone());
            }
        }
    }
    // Report the clan's total (non-residence) skill count now in force, not just
    // the newly-added ones — a re-run on an already-stocked clan then reports the
    // real number it re-applied instead of a confusing "0 skills".
    world
        .clans
        .get(&clan_id)
        .map(|c| c.skills.len())
        .unwrap_or(0)
}

/// `VillageMaster.onBypassFeedback`'s `create_clan` branch: parse the typed name
/// (rejecting embedded spaces, Java's `isValidName` reject) then run
/// [`create_clan`] for the acting player. `args` is everything after the verb.
pub(crate) fn handle_create_clan(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut tokens = args.split(' ').filter(|t| !t.is_empty());
    let Some(name) = tokens.next().map(str::to_string) else {
        return;
    }; // empty → silent, like Java
    if tokens.next().is_some() {
        // A second token means the typed name had a space — Java folds this
        // into the isValidName reject. (`ClanNameTemplate = .*` on this
        // dist, so the regex itself is not ported.)
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    create_clan(world, player_oid, &name);
}

/// Core `ClanTable.createClan(leader, name)`: the guards in Java's order, then
/// the insert + the pledge windows sent to the leader's own client. Returns the
/// new clan id, or `None` on any guarded failure (the matching sysmsg has
/// already gone to the leader). Admin `//pledge create` bypasses the recreate
/// cooldown the same way Java does — by zeroing the target's expiry field
/// *before* calling this (and restoring it on failure), not via a flag here.
pub(crate) fn create_clan(world: &mut World, leader_oid: i32, name: &str) -> Option<i32> {
    let leader_client = client_for_player(world, leader_oid)?;
    let name = name.to_string();

    // --- ClanTable.createClan guards, in order ---
    let p = world.objects.get_component::<Player>(&leader_oid)?;
    if p.level < 10 {
        send_sm(
            world,
            leader_client,
            sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN,
        );
        return None;
    }
    if p.clan_id != 0 {
        send_sm(
            world,
            leader_client,
            sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN,
        );
        return None;
    }
    if now_millis() < p.clan_create_expiry_time {
        send_sm(
            world,
            leader_client,
            sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN,
        );
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) || name.len() < 2 {
        send_sm(world, leader_client, sm_ids::CLAN_NAME_IS_INVALID);
        return None;
    }
    if name.len() > 16 {
        send_sm(
            world,
            leader_client,
            sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT,
        );
        return None;
    }
    if world
        .clans
        .values()
        .any(|c| c.name.eq_ignore_ascii_case(&name))
    {
        if let Some(cs) = world.clients.get(&leader_client) {
            cs.send(server_packets::system_message_with(
                sm_ids::S1_ALREADY_EXISTS,
                &[SmParam::Text(name.clone())],
            ));
        }
        return None;
    }

    // --- create ---
    let Some(clan_id) = world.alloc_object_id() else {
        warn!("create_clan: object-id pool exhausted.");
        return None;
    };
    let leader = {
        let p = world
            .objects
            .get_component::<Player>(&leader_oid)
            .expect("checked above");
        ClanMember {
            char_id: leader_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
            power_grade: 1, // Java restore: the leader holds grade 1
            title: p.title.clone(),
            pledge_type: 0,
        }
    };
    let clan = Clan {
        id: clan_id,
        name: name.clone(),
        leader_id: leader_oid,
        level: 0,
        reputation_score: 0,
        castle_id: 0,
        members: vec![leader],
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
    };
    let _ = world.db.send(DbCommand::InsertClan {
        clan_id,
        name: name.clone(),
        leader_id: leader_oid,
    });
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: leader_oid,
        clan_id,
        clan_privs: ALL_CLAN_PRIVILEGES,
    });
    {
        let p = world
            .objects
            .get_component_mut::<Player>(&leader_oid)
            .expect("checked above");
        p.clan_id = clan_id;
        p.clan_privs = ALL_CLAN_PRIVILEGES;
        p.clan_leader = true;
        p.power_grade = 1;
        p.pledge_class = clan.pledge_class_of(leader_oid); // 0 at level 0
    }

    if let Some(cs) = world.clients.get(&leader_client) {
        cs.send(server_packets::pledge_show_info_update(&clan));
        cs.send(server_packets::pledge_show_member_list_all(
            &clan,
            &world.objects,
        ));
        if let Some(m) = clan.member(leader_oid) {
            cs.send(server_packets::pledge_show_member_list_update(m, true));
        }
    }
    send_sm(world, leader_client, sm_ids::YOUR_CLAN_HAS_BEEN_CREATED);
    world.clans.insert(clan_id, clan);
    // `broadcastUserInfo(RELATION, CLAN)` — the full re-send stands in
    // (same G10 substitution for RelationChanged).
    super::party::broadcast_user_info(world, leader_oid);
    Some(clan_id)
}

/// `Clan.changeLevel` (admin `//pledge setlevel`): set the level, persist it,
/// and refresh every online member — Java broadcasts `YOUR_CLAN_S_LEVEL_HAS_
/// INCREASED` + `PledgeShowInfoUpdate`, and AdminPledge re-broadcasts UserInfo
/// (`RELATION, CLAN`). Siege-skill grant/removal on the level-5 boundary is
/// deferred (no siege system yet).
pub(crate) fn set_clan_level(world: &mut World, clan_id: i32, level: i32) {
    let member_ids: Vec<i32> = {
        let Some(clan) = world.clans.get_mut(&clan_id) else {
            return;
        };
        clan.level = level;
        clan.members.iter().map(|m| m.char_id).collect()
    };
    let _ = world.db.send(DbCommand::UpdateClanLevel { clan_id, level });
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    broadcast_to_clan(world, clan_id, &info);
    broadcast_to_clan(
        world,
        clan_id,
        &crate::network::enter_world::system_message(sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED),
    );
    for oid in member_ids {
        // The level change may cross a pledge-class boundary (the on-head crown);
        // recompute per member before the UserInfo/CharInfo re-broadcast.
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map_or(0, |c| c.pledge_class_of(oid));
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.pledge_class = pledge_class;
        }
        super::party::broadcast_user_info(world, oid);
    }
    // Java `Clan.changeLevel`: on reaching the siege min level the online leader
    // gains the siege/leader skills (`SiegeManager.addSiegeSkills(leader)`).
    let leader_id = world.clans.get(&clan_id).map(|c| c.leader_id).unwrap_or(0);
    if leader_id != 0 && client_for_player(world, leader_id).is_some() {
        apply_siege_skills_to_leader(world, clan_id, leader_id);
        // Java `changeLevel`: crossing level 5 tells the leader the clan can now
        // accumulate reputation.
        if level > 4 {
            if let Some(cid) = client_for_player(world, leader_id) {
                send_sm(world, cid, sm_ids::NOW_THAT_YOUR_CLAN_LEVEL_IS_ABOVE_LEVEL_5_IT_CAN_ACCUMULATE_CLAN_REPUTATION);
            }
        }
    }
}

/// `Clan.addReputationScore` (admin `//pledge rep`): add signed points, clamp,
/// persist, and refresh every online member's pledge window. Returns the new
/// score. Clan-skill (de)activation on the zero crossing is deferred (clan
/// skills unported).
pub(crate) fn add_clan_reputation(world: &mut World, clan_id: i32, points: i32) -> Option<i32> {
    let new_score = world.clans.get_mut(&clan_id)?.add_reputation_score(points);
    let _ = world.db.send(DbCommand::UpdateClanReputation {
        clan_id,
        reputation: new_score,
    });
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    broadcast_to_clan(world, clan_id, &info);
    Some(new_score)
}

/// `ClanTable.destroyClan` (admin `//pledge dismiss`), narrowed to what the port
/// models: broadcast `CLAN_HAS_DISPERSED`, reset every member's clan state (in
/// memory for the online ones + a blanket DB reset for all), close their clan
/// windows, and drop the clan. Siege/fort/clan-hall/ally/war teardown, apprentice
/// links, and clan-skill removal are deferred (those systems are unported).
pub(crate) fn destroy_clan(world: &mut World, clan_id: i32) {
    let (leader_id, member_ids) = {
        let Some(clan) = world.clans.get(&clan_id) else {
            return;
        };
        (
            clan.leader_id,
            clan.members.iter().map(|m| m.char_id).collect::<Vec<_>>(),
        )
    };
    // Java broadcasts the dispersal message before tearing the roster down.
    broadcast_to_clan(
        world,
        clan_id,
        &crate::network::enter_world::system_message(sm_ids::CLAN_HAS_DISPERSED),
    );

    // Java stamps the recreate cooldown on the (online) leader in removeClanMember.
    let leader_expiry = now_millis() + CLAN_CREATE_COOLDOWN_MS;
    let delete_all = server_packets::pledge_show_member_list_delete_all();
    for oid in &member_ids {
        let online = {
            if let Some(p) = world.objects.get_component_mut::<Player>(oid) {
                p.clan_id = 0;
                p.clan_privs = 0;
                p.clan_leader = false;
                p.pledge_class = 0;
                p.ally_id = 0;
                if *oid == leader_id {
                    p.clan_create_expiry_time = leader_expiry;
                }
                // TODO(G25): Java clears the title only for non-nobles; the noble
                // system is unported, so every ex-member loses their title here.
                p.title.clear();
                true
            } else {
                false
            }
        };
        if online {
            // Java `removeClanMember` stops Clan Advent + all clan skills on each
            // member as the clan disperses; the ex-members stay online, so both
            // the aura and the pledge skills must drop.
            remove_clan_advent(world, *oid);
            remove_clan_skills_from_member(world, *oid);
            if let Some(cid) = client_for_player(world, *oid) {
                if let Some(cs) = world.clients.get(&cid) {
                    cs.send(delete_all.clone());
                }
            }
        }
    }
    world.clans.remove(&clan_id);
    let _ = world.db.send(DbCommand::DestroyClan {
        clan_id,
        leader_id,
        leader_expiry,
    });
    // broadcastUserInfo for the now clan-less online members.
    for oid in member_ids {
        super::party::broadcast_user_info(world, oid);
    }
}

/// `RequestPledgeInfo.runImpl`: answer with the clan's name/ally names for a
/// clan id (Java resolves through `ClanTable.getClan`; unknown ids are
/// silently dropped, matching the "should not happen" early return).
pub(crate) fn handle_request_pledge_info(world: &World, client_id: u32, body: &[u8]) {
    // Java guards on a logged-in player before touching the clan table.
    if !matches!(
        world.clients.get(&client_id),
        Some(crate::session::ClientSession::InGame(_))
    ) {
        return;
    }
    let Some(clan_id) = PacketReader::new(body).read_i32() else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_info(clan));
    }
}

/// `RequestPledgeRecruitInfo` (ex 0xD3): a clan's recruitment summary,
/// answered with `ExPledgeRecruitInfo`. Java resolves the clan through
/// `ClanTable` and stays silent for an unknown id.
pub(crate) fn handle_request_pledge_recruit_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_info(clan));
    }
}

/// `RequestPledgeRecruitApplyInfo` (ex 0xDE): the clan window polls the
/// player's clan-entry status on open. Java's `ClanEntryStatus`: DEFAULT=0,
/// ORDERED=1 (the leader of a clan registered on the board),
/// CLAN_REGISTRATION=2, UNKNOWN=3, WAITING=4 (a clanless player with a
/// pending application to any clan).
pub(crate) fn handle_request_pledge_recruit_apply_info(world: &World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let status = if p.clan_id != 0 && p.clan_leader && world.recruit_clans.contains_key(&p.clan_id)
    {
        1 // ORDERED
    } else if p.clan_id == 0 && world.recruit_waiting.contains_key(&player) {
        4 // WAITING
    } else {
        0 // DEFAULT
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_apply_info(status));
    }
}

/// `EnterWorld.runImpl`'s clan section (narrowed): fix the leader flag from
/// the live table, refresh the member's level in the roster, send the
/// pledge window to the enterer and the online-status update to the rest.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        return;
    }
    let level = p.level;
    let (is_leader, pledge_class) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.leader_id == object_id, c.pledge_class_of(object_id)))
        .unwrap_or((false, 0));
    // Java `Player.restore`: the leader gets all privileges + grade 1; anyone
    // else gets their rank's mask (grade defaulting to 5) — the stored
    // `clan_privs` column never wins over the live rank table.
    let rank_privs = {
        let grade = p.power_grade;
        world.clans.get(&clan_id).map(|c| {
            let grade = if grade == 0 { 5 } else { grade };
            (grade, c.rank_privs_of(grade))
        })
    };
    let (ally_id, ally_crest_id, clan_crest_id) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.ally_id, c.ally_crest_id, c.crest_id))
        .unwrap_or((0, 0, 0));
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.clan_leader = is_leader;
        p.pledge_class = pledge_class;
        p.ally_id = ally_id;
        p.ally_crest_id = ally_crest_id;
        p.clan_crest_id = clan_crest_id;
        if is_leader {
            p.clan_privs = ALL_CLAN_PRIVILEGES;
            p.power_grade = 1;
        } else if let Some((grade, privs)) = rank_privs {
            p.power_grade = grade;
            p.clan_privs = privs;
        }
    }
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        warn!("Player {object_id} carries unknown clan id {clan_id}.");
        return;
    };
    if let Some(m) = clan.members.iter_mut().find(|m| m.char_id == object_id) {
        m.level = level;
    }
    let clan = world.clans.get(&clan_id).expect("checked above");
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_show_member_list_all(
            clan,
            &world.objects,
        ));
    }
    notify_members(world, clan_id, object_id, true);
    // Clan Advent (skill 19009) — Java `ClanMaster.onPlayerLogin`.
    apply_clan_advent_on_login(world, clan_id, object_id);
    // Clan skills — Java `EnterWorld` → `clan.addSkillEffects(player)`.
    apply_clan_skills_to_member(world, clan_id, object_id);
    // Siege/leader skills — Java `EnterWorld`: `if (clan.getLevel() >=
    // siegeClanMinLevel && isClanLeader()) addSiegeSkills(player)`.
    apply_siege_skills_to_leader(world, clan_id, object_id);
}

/// `Player.deleteMe`'s clan half: the offline ping to online members, plus the
/// Clan Advent teardown (Java `ClanMaster.onPlayerLogout`). When the leader logs
/// out the aura drops from every *other* online member; the leaver themselves is
/// despawned right after this returns, so stripping it from self would be moot.
pub(crate) fn on_leave_world(world: &mut World, object_id: i32, clan_id: i32) {
    if clan_id == 0 {
        return;
    }
    notify_members(world, clan_id, object_id, false);
    let is_leader = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.leader_id == object_id);
    if is_leader {
        for oid in online_members(world, clan_id) {
            if oid != object_id {
                remove_clan_advent(world, oid);
            }
        }
    }
}

/// `PledgeShowMemberListUpdate` about `subject` to every *other* online
/// clan member.
fn notify_members(world: &World, clan_id: i32, subject: i32, online: bool) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let Some(subject_member) = clan.member(subject) else {
        return;
    };
    let pkt = server_packets::pledge_show_member_list_update(subject_member, online);
    for m in &clan.members {
        if m.char_id == subject {
            continue;
        }
        if let Some(cid) = client_for_player(world, m.char_id) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt.clone());
            }
        }
    }
}

/// Clan chat (`ChatType::Clan` in `Say2`): `CreatureSay` to every online
/// member including the speaker (Java `Clan.broadcastToOnlineMembers`).
pub(crate) fn broadcast_to_clan(world: &World, clan_id: i32, pkt: &[u8]) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    for m in &clan.members {
        if let Some(cid) = client_for_player(world, m.char_id) {
            if let Some(cs) = world.clients.get(&cid) {
                cs.send(pkt.to_vec());
            }
        }
    }
}

// --- G18 slice 1: membership lifecycle -------------------------------------

/// `DaysBeforeJoinAClan = 1` on this dist → the rejoin penalty in millis
/// (stamped on a leaver/oustee and on the ousting clan).
const CLAN_JOIN_PENALTY_MS: i64 = 86_400_000;

/// `DaysToPassToDissolveAClan = 7` on this dist → the dissolution delay.
const CLAN_DISSOLVE_DELAY_MS: i64 = 7 * 86_400_000;

/// The game loop runs at 10 ticks/s — wall-clock millis to scheduler ticks.
const MS_PER_TICK: i64 = 100;

fn send_sm_with(world: &World, oid: i32, id: i16, params: &[SmParam]) {
    if let Some(cs) = client_for_player(world, oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::system_message_with(id, params));
    }
}

fn send_to_member(world: &World, oid: i32, pkt: Vec<u8>) {
    if let Some(cs) = client_for_player(world, oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(pkt);
    }
}

fn player_name(world: &World, oid: i32) -> String {
    world
        .objects
        .get_component::<Player>(&oid)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

/// `ClassId.level()` — occupation tier via the `*_CLASS_GROUP` categories
/// (same mapping the henna/support-magic gates use).
fn class_level(world: &World, class_id: i32) -> i32 {
    let c = &world.data.categories;
    if c.contains("FOURTH_CLASS_GROUP", class_id) {
        3
    } else if c.contains("THIRD_CLASS_GROUP", class_id) {
        2
    } else if c.contains("SECOND_CLASS_GROUP", class_id) {
        1
    } else {
        0
    }
}

/// Java `Clan.checkClanJoinCondition(player, target, pledgeType)` — the invite
/// guard chain, with each reject's system message sent to the inviter. Run at
/// invite time and re-run when the answer arrives (conditions can change while
/// the dialog is up — Java's "double check").
fn check_clan_join_condition(
    world: &World,
    requestor_oid: i32,
    target_oid: i32,
    pledge_type: i32,
) -> bool {
    let Some(req) = world.objects.get_component::<Player>(&requestor_oid) else {
        return false;
    };
    let clan_id = req.clan_id;
    let requestor_privs = req.clan_privs;
    let Some(clan) = world.clans.get(&clan_id) else {
        return false;
    };
    if !clan.has_privilege(requestor_oid, requestor_privs, CL_JOIN_CLAN) {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return false;
    }
    let Some(target) = world.objects.get_component::<Player>(&target_oid) else {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return false;
    };
    if requestor_oid == target_oid {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN,
            &[],
        );
        return false;
    }
    if clan.char_penalty_expiry_time > now_millis() {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY,
            &[],
        );
        return false;
    }
    if target.clan_id != 0 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_IS_ALREADY_A_MEMBER_OF_ANOTHER_CLAN,
            &[SmParam::Text(target.name.clone())],
        );
        return false;
    }
    if target.clan_join_expiry_time > now_millis() {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING,
            &[SmParam::Text(target.name.clone())],
        );
        return false;
    }
    if (target.level > 40 || class_level(world, target.class_id) >= 2) && pledge_type == -1 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_DOES_NOT_MEET_THE_REQUIREMENTS_TO_JOIN_A_CLAN_ACADEMY,
            &[SmParam::Text(target.name.clone())],
        );
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::IN_ORDER_TO_JOIN_THE_CLAN_ACADEMY_YOU_MUST_BE_UNAFFILIATED,
            &[],
        );
        return false;
    }
    if clan.sub_pledge_members_count(pledge_type) >= clan.max_members_of(pledge_type) {
        if pledge_type == 0 {
            send_sm_with(
                world,
                requestor_oid,
                sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS,
                &[SmParam::Text(clan.name.clone())],
            );
        } else {
            send_sm_with(world, requestor_oid, sm_ids::THE_CLAN_IS_FULL, &[]);
        }
        return false;
    }
    true
}

/// `RequestJoinPledge` (0x26): a clan member invites the target player. Guards,
/// then parks the invite in the `PendingRequest` slot and puts `AskJoinPledge`
/// on the target's screen.
pub(crate) fn handle_request_join_pledge(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(target_oid) = r.read_i32() else {
        return;
    };
    let Some(pledge_type) = r.read_i32() else {
        return;
    };

    let clan_id = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    if clan_id == 0 {
        return; // Java: getClan() == null → silent
    }
    // Java resolves the target through `World.getPlayer(objectId)` (online only).
    if client_for_player(world, target_oid).is_none() {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return;
    }
    if !check_clan_join_condition(world, player, target_oid, pledge_type) {
        return;
    }
    if pledge_type != 0
        && !world
            .clans
            .get(&clan_id)
            .is_some_and(|c| c.sub_pledges.contains_key(&pledge_type))
    {
        // The client only ever offers real sub-units in the invite dialog; a
        // request naming a pledge type the clan hasn't founded is dropped
        // (Java trusts the client here too — this is the port's own guard
        // against corrupting the roster on a malformed/hacked packet).
        warn!("Clan invite with pledge type {pledge_type} refused — no such sub-unit.");
        return;
    }
    // Java `player.getRequest().setRequest(target, this)` — busy targets answer
    // "on another task" (the shared transaction-slot behavior).
    if world
        .objects
        .has_component::<crate::model::components::PendingRequest>(&player)
        || world
            .objects
            .has_component::<crate::model::components::PendingRequest>(&target_oid)
    {
        send_sm_with(
            world,
            player,
            sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
            &[SmParam::Text(player_name(world, target_oid))],
        );
        return;
    }
    super::party::install_request(
        world,
        player,
        target_oid,
        crate::model::components::RequestKind::ClanInvite {
            clan_id,
            pledge_type,
        },
        super::party::REQUEST_TIMEOUT_TICKS,
    );
    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    send_to_member(
        world,
        target_oid,
        server_packets::ask_join_pledge(
            player,
            &player_name(world, player),
            pledge_type,
            &clan_name,
        ),
    );
}

/// `RequestAnswerJoinPledge` (0x27): the invited player answered the
/// `AskJoinPledge` dialog. Decline notifies both sides; accept re-checks the
/// join condition and runs `Clan.addClanMember` (roster + packets + skills).
pub(crate) fn handle_request_answer_join_pledge(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let answer = PacketReader::new(body).read_i32().unwrap_or(0);

    let Some(req) = world
        .objects
        .get_component::<crate::model::components::PendingRequest>(&player)
        .copied()
    else {
        return;
    };
    let crate::model::components::RequestKind::ClanInvite {
        clan_id,
        pledge_type,
    } = req.kind
    else {
        return;
    };
    if !req.answerer {
        return;
    }
    super::party::clear_linked_request(world, player);
    let requestor = req.other;

    if answer == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_DIDN_T_RESPOND_TO_S1_S_INVITATION_JOINING_HAS_BEEN_CANCELLED,
            &[SmParam::Text(player_name(world, requestor))],
        );
        send_sm_with(
            world,
            requestor,
            sm_ids::S1_DID_NOT_RESPOND_INVITATION_TO_THE_CLAN_HAS_BEEN_CANCELLED,
            &[SmParam::Text(player_name(world, player))],
        );
        return;
    }
    // "conditions can be changed, i.e. another player could join" — re-check,
    // and the requestor must still be in the clan the invite was for.
    if world
        .objects
        .get_component::<Player>(&requestor)
        .map(|p| p.clan_id)
        != Some(clan_id)
    {
        return;
    }
    if !check_clan_join_condition(world, requestor, player, pledge_type) {
        return;
    }
    if world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0)
        != 0
    {
        return;
    }
    add_clan_member(world, clan_id, player, pledge_type);
}

/// Java `RequestAnswerJoinPledge`'s accept half + `Clan.addClanMember`: put the
/// new member in the roster, wire their clan fields, and send the join burst.
/// New members start at power grade 5 with no rank privileges (the rank-privs
/// table is a later slice — Java's fresh-clan `getRankPrivs(5)` is CP_NOTHING).
fn add_clan_member(world: &mut World, clan_id: i32, player_oid: i32, pledge_type: i32) {
    send_to_member(world, player_oid, server_packets::join_pledge(clan_id));

    // Java: academy members start at power grade 9, everyone else at 5
    // ("not confirmed" per Java's own comment, kept faithfully).
    let grade = if pledge_type == crate::model::clan::SUBUNIT_ACADEMY {
        9
    } else {
        5
    };
    let member = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return;
        };
        ClanMember {
            char_id: player_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
            power_grade: grade,
            title: p.title.clone(),
            pledge_type,
        }
    };
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.members.push(member.clone());
    let pledge_class = clan.pledge_class_of(player_oid);
    // Java `player.setClanPrivileges(clan.getRankPrivs(player.getPowerGrade()))`.
    let privs = clan.rank_privs_of(grade);
    let (ally_id, ally_crest_id, clan_crest_id) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.ally_id, c.ally_crest_id, c.crest_id))
        .unwrap_or((0, 0, 0));
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.clan_id = clan_id;
        p.clan_privs = privs;
        p.clan_leader = false;
        p.power_grade = grade;
        p.pledge_type = pledge_type;
        p.pledge_class = pledge_class;
        p.ally_id = ally_id;
        p.ally_crest_id = ally_crest_id;
        p.clan_crest_id = clan_crest_id;
        p.clan_join_expiry_time = 0; // Java `setClanJoinExpiryTime(0)`
    }
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: player_oid,
        clan_id,
        clan_privs: privs,
    });
    let _ = world.db.send(DbCommand::UpdateCharPowerGrade {
        char_id: player_oid,
        power_grade: grade,
    });
    let _ = world.db.send(DbCommand::UpdateCharPledgeType {
        char_id: player_oid,
        pledge_type,
    });
    let _ = world.db.send(DbCommand::UpdateCharClanJoinExpiry {
        char_id: player_oid,
        expiry: 0,
    });
    // TODO(G18.6b): Java also sets `lvlJoinedAcademy` for the eventual academy
    // graduation reward (apprentice/sponsor links) — unported, no consumer yet.

    send_sm_with(world, player_oid, sm_ids::ENTERED_THE_CLAN, &[]);
    let joined = server_packets::system_message_with(
        sm_ids::S1_HAS_JOINED_THE_CLAN,
        &[SmParam::Text(player_name(world, player_oid))],
    );
    broadcast_to_clan(world, clan_id, &joined);

    // TODO(G24): Java gives castle/fort residential skills here when the clan
    // owns a residence, then `player.sendSkillList()`.
    // Clan skills + the merged skill list (Java `addClanMember` →
    // `addSkillEffects(player)` + `PledgeSkillList`).
    apply_clan_skills_to_member(world, clan_id, player_oid);
    // Clan Advent — Java fires ON_PLAYER_CLAN_JOIN, the ClanMaster script
    // lights the aura on the joiner when the leader is online.
    let leader_online = world
        .clans
        .get(&clan_id)
        .map(|c| c.leader_id)
        .is_some_and(|lid| client_for_player(world, lid).is_some());
    if leader_online {
        apply_clan_advent(world, player_oid);
    }

    let add = server_packets::pledge_show_member_list_add(&member);
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("inserted above"));
    let count = server_packets::ex_pledge_count(
        world
            .clans
            .get(&clan_id)
            .map(|c| c.members.len())
            .unwrap_or(0) as i32,
    );
    for oid in online_members(world, clan_id) {
        if oid != player_oid {
            send_to_member(world, oid, add.clone());
        }
        send_to_member(world, oid, info.clone());
        send_to_member(world, oid, count.clone());
    }
    // "this activates the clan tab on the new member".
    let all = server_packets::pledge_show_member_list_all(
        world.clans.get(&clan_id).expect("inserted above"),
        &world.objects,
    );
    send_to_member(world, player_oid, all);
    super::party::broadcast_user_info(world, player_oid);
}

/// Java `Clan.removeClanMember(objectId, clanJoinExpiryTime)`, narrowed to the
/// main pledge: drop the roster row, tear the member's clan state down (online)
/// or push the column reset (offline), and stamp the rejoin penalty. The
/// caller sends the leave/oust messages and the roster-delete broadcasts.
/// Apprentice/sponsor and sub-pledge-leader cleanup: TODO(G18.6); castle
/// circlet removal and residential-skill teardown: TODO(G24).
fn remove_clan_member(world: &mut World, clan_id: i32, member_oid: i32, clan_join_expiry: i64) {
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    let Some(idx) = clan.members.iter().position(|m| m.char_id == member_oid) else {
        warn!("Member {member_oid} not found in clan {clan_id} while trying to remove.");
        return;
    };
    clan.members.remove(idx);
    let was_leader = clan.leader_id == member_oid;
    let leader_expiry = if was_leader {
        now_millis() + CLAN_CREATE_COOLDOWN_MS
    } else {
        0
    };

    // Java `removeClanMember`: a departing sub-unit captain leaves the slot
    // vacant ("position becomes vacant and leader should appoint new via NPC").
    let vacated_sub_pledge = clan.leader_sub_pledge_of(member_oid);
    if vacated_sub_pledge != 0 {
        if let Some(sp) = clan.sub_pledges.get_mut(&vacated_sub_pledge) {
            sp.leader_id = 0;
        }
        let (name, leader_id) = clan
            .sub_pledges
            .get(&vacated_sub_pledge)
            .map(|sp| (sp.name.clone(), sp.leader_id))
            .unwrap_or_default();
        let _ = world.db.send(DbCommand::UpdateSubPledge {
            clan_id,
            pledge_type: vacated_sub_pledge,
            name,
            leader_id,
        });
    }

    let online = world.objects.get_component::<Player>(&member_oid).is_some();
    if online {
        // Java: title cleared unless noble, clan skills + Clan Advent stripped,
        // clan fields zeroed, join penalty stamped, window closed.
        remove_clan_advent(world, member_oid);
        remove_clan_skills_from_member(world, member_oid);
        if let Some(p) = world.objects.get_component_mut::<Player>(&member_oid) {
            if !p.is_noble {
                p.title.clear();
            }
            p.clan_id = 0;
            p.clan_privs = 0;
            p.clan_leader = false;
            p.pledge_class = 0;
            p.ally_id = 0;
            p.clan_join_expiry_time = clan_join_expiry;
            if was_leader {
                p.clan_create_expiry_time = leader_expiry;
            }
        }
        send_to_member(
            world,
            member_oid,
            server_packets::pledge_show_member_list_delete_all(),
        );
        super::party::broadcast_user_info(world, member_oid);
    }
    let _ = world.db.send(DbCommand::RemoveClanMember {
        char_id: member_oid,
        clan_join_expiry,
        clan_create_expiry: leader_expiry,
    });
}

/// `RequestWithdrawalPledge` (0x28): a member (never the leader) leaves their
/// clan, taking the 1-day rejoin penalty.
pub(crate) fn handle_request_withdrawal_pledge(world: &mut World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    if p.clan_leader {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_LEADER_CANNOT_WITHDRAW_FROM_THEIR_OWN_CLAN,
            &[],
        );
        return;
    }
    if super::combat::has_attack_stance(world, player) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_CANNOT_LEAVE_A_CLAN_WHILE_ENGAGED_IN_COMBAT,
            &[],
        );
        return;
    }

    let name = player_name(world, player);
    remove_clan_member(world, clan_id, player, now_millis() + CLAN_JOIN_PENALTY_MS);

    let withdrew = server_packets::system_message_with(
        sm_ids::S1_HAS_WITHDRAWN_FROM_THE_CLAN,
        &[SmParam::Text(name.clone())],
    );
    broadcast_to_clan(world, clan_id, &withdrew);
    broadcast_to_clan(
        world,
        clan_id,
        &server_packets::pledge_show_member_list_delete(&name),
    );
    let count = server_packets::ex_pledge_count(
        world
            .clans
            .get(&clan_id)
            .map(|c| c.members.len())
            .unwrap_or(0) as i32,
    );
    broadcast_to_clan(world, clan_id, &count);
    send_sm_with(world, player, sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_CLAN, &[]);
    send_sm_with(
        world,
        player,
        sm_ids::AFTER_LEAVING_A_CLAN_YOU_MUST_WAIT_A_DAY_BEFORE_JOINING_ANOTHER,
        &[],
    );
}

/// `RequestOustPledgeMember` (0x29): a member with CL_DISMISS expels another
/// member by name. Both sides take a 1-day penalty: the oustee cannot join a
/// clan, the clan cannot invite (`setCharPenaltyExpiryTime`).
pub(crate) fn handle_request_oust_pledge_member(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(target_name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if !clan.has_privilege(player, privs, CL_DISMISS) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if player_name(world, player).eq_ignore_ascii_case(&target_name) {
        send_sm_with(world, player, sm_ids::YOU_CANNOT_DISMISS_YOURSELF, &[]);
        return;
    }
    let Some(member) = clan
        .members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(&target_name))
        .cloned()
    else {
        warn!("Oust target ({target_name}) is not a member of clan {clan_id}.");
        return;
    };
    let member_online = client_for_player(world, member.char_id).is_some();
    if member_online && super::combat::has_attack_stance(world, member.char_id) {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_MEMBER_MAY_NOT_BE_DISMISSED_DURING_COMBAT,
            &[],
        );
        return;
    }

    let penalty_until = now_millis() + CLAN_JOIN_PENALTY_MS;
    remove_clan_member(world, clan_id, member.char_id, penalty_until);
    let dissolving = world
        .clans
        .get(&clan_id)
        .map(|c| c.dissolving_expiry_time)
        .unwrap_or(0);
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.char_penalty_expiry_time = penalty_until;
    }
    let _ = world.db.send(DbCommand::UpdateClanPenalties {
        clan_id,
        char_penalty_expiry_time: penalty_until,
        dissolving_expiry_time: dissolving,
    });

    let expelled = server_packets::system_message_with(
        sm_ids::CLAN_MEMBER_S1_HAS_BEEN_EXPELLED,
        &[SmParam::Text(member.name.clone())],
    );
    broadcast_to_clan(world, clan_id, &expelled);
    send_sm_with(
        world,
        player,
        sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN_MEMBER,
        &[],
    );
    send_sm_with(
        world,
        player,
        sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY,
        &[],
    );
    broadcast_to_clan(
        world,
        clan_id,
        &server_packets::pledge_show_member_list_delete(&member.name),
    );
    let count = server_packets::ex_pledge_count(
        world
            .clans
            .get(&clan_id)
            .map(|c| c.members.len())
            .unwrap_or(0) as i32,
    );
    broadcast_to_clan(world, clan_id, &count);
    if member_online {
        send_sm_with(
            world,
            member.char_id,
            sm_ids::YOU_HAVE_RECENTLY_BEEN_DISMISSED_FROM_A_CLAN,
            &[],
        );
    }
}

/// `VillageMaster.dissolveClan` (the `dissolve_clan` bypass): guard chain,
/// then stamp `dissolving_expiry_time`, hit the leader with a full death-XP
/// penalty, and schedule the delayed removal.
pub(crate) fn handle_dissolve_clan(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.ally_id != 0 {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISPERSE_THE_CLANS_IN_YOUR_ALLIANCE,
        );
        return;
    }
    if clan_is_at_war(world, clan_id) {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_ENGAGED_IN_A_WAR,
        );
        return;
    }
    if clan.castle_id != 0 {
        // Java folds castle/clan-hall/fort ownership into SM 266.
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_OWNING_A_CLAN_HALL_OR_CASTLE,
        );
        return;
    }
    if world.sieges.values().any(|s| s.is_registered(clan_id)) {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE,
        );
        return;
    }
    if let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&player_oid)
    {
        if world
            .data
            .zone_data
            .siege_castle_at(pos.x, pos.y, pos.z)
            .is_some()
        {
            send_sm(
                world,
                client_id,
                sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE,
            );
            return;
        }
    }
    if clan.dissolving_expiry_time > now_millis() {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_HAVE_ALREADY_REQUESTED_THE_DISSOLUTION_OF_YOUR_CLAN,
        );
        return;
    }

    let due = now_millis() + CLAN_DISSOLVE_DELAY_MS;
    let char_penalty = clan.char_penalty_expiry_time;
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.dissolving_expiry_time = due;
    }
    let _ = world.db.send(DbCommand::UpdateClanPenalties {
        clan_id,
        char_penalty_expiry_time: char_penalty,
        dissolving_expiry_time: due,
    });
    // "The clan leader should take the XP penalty of a full death."
    super::death::apply_death_exp_penalty(world, player_oid);
    schedule_clan_dissolve(world, clan_id, due);
}

/// `VillageMaster.recoverClan` (the `recover_clan` bypass): the leader cancels
/// a pending dissolution — the stamp is zeroed, the scheduled removal no-ops.
pub(crate) fn handle_recover_clan(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let char_penalty = {
        let Some(c) = world.clans.get_mut(&clan_id) else {
            return;
        };
        c.dissolving_expiry_time = 0;
        c.char_penalty_expiry_time
    };
    let _ = world.db.send(DbCommand::UpdateClanPenalties {
        clan_id,
        char_penalty_expiry_time: char_penalty,
        dissolving_expiry_time: 0,
    });
}

/// Arm the `ClanDissolve` task for `due` (wall clock) — used by the dissolve
/// bypass and re-armed at boot for persisted stamps (`ClanTable`'s constructor
/// schedules past-due dissolutions to fire immediately).
pub(crate) fn schedule_clan_dissolve(world: &mut World, clan_id: i32, due: i64) {
    let delay_ticks = ((due - now_millis()).max(0) / MS_PER_TICK) as u64;
    world.scheduler.schedule(
        world.tick + delay_ticks,
        crate::scheduler::ScheduledTask::ClanDissolve { clan_id },
    );
}

/// `ClanTable.scheduleRemoveClan`'s body at fire time: destroy only if the
/// dissolution is still requested and has come due (a `recover_clan` in the
/// meantime zeroes the stamp and turns this into a no-op).
pub(crate) fn handle_clan_dissolve_task(world: &mut World, clan_id: i32) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.dissolving_expiry_time == 0 || clan.dissolving_expiry_time > now_millis() {
        return;
    }
    destroy_clan(world, clan_id);
}

// --- G18 slice 2: clan level-up + rep-gated pledge skill learning ----------

/// `AcquireSkillType.PLEDGE` on the wire (skill lists + acquire packets).
const ACQUIRE_TYPE_PLEDGE: i16 = 2;

/// Blood Mark — the proof item the level 3/4/5 upgrades consume.
const BLOOD_MARK: i32 = 1419;
const ADENA: i32 = 57;

/// Java `Clan.levelUpClan`'s cost ladder (Classic values, `_level` → next):
/// `(sp, item_id, item_count)` — levels 0/1 charge adena, 2..4 Blood Marks.
const LEVEL_UP_COSTS: [(i64, i32, i64); 5] = [
    (1_000, ADENA, 150_000),
    (15_000, ADENA, 300_000),
    (100_000, BLOOD_MARK, 100),
    (1_000_000, BLOOD_MARK, 5_000),
    (5_000_000, BLOOD_MARK, 10_000),
];

/// `VillageMaster.onBypassFeedback`'s `increase_clan_level` branch →
/// `Clan.levelUpClan`: leader + not-dissolving gates, the SP/adena/proof-item
/// price for the current level, then `changeLevel(level + 1)` and the level-up
/// FX (`MagicSkillUse` 5103) broadcast from the leader.
pub(crate) fn handle_increase_clan_level(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let sp = p.sp;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if now_millis() < clan.dissolving_expiry_time {
        send_sm(
            world,
            client_id,
            sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_LEVEL_CANNOT_INCREASE,
        );
        return;
    }
    let level = clan.level;
    let Some(&(sp_cost, item_id, item_count)) = LEVEL_UP_COSTS.get(level as usize) else {
        // Level 5+ has no village-master upgrade on this dist (Java returns
        // false with no message past the ladder).
        return;
    };
    let has_items = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&player_oid)
        .is_some_and(|inv| inv.count_of(item_id) >= item_count);
    if sp < sp_cost || !has_items {
        send_sm(
            world,
            client_id,
            sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET,
        );
        return;
    }
    if !super::quests::take_items(world, client_id, player_oid, item_id, item_count) {
        send_sm(
            world,
            client_id,
            sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET,
        );
        return;
    }
    // The consumption messages: adena sends its own line (Java `reduceAdena
    // (sendMessage=true)`), proof items send the destroy line + `levelUpClan`'s
    // explicit `S1_DISAPPEARED` (Java double-messages here — kept faithful).
    if item_id == ADENA {
        send_sm_with(
            world,
            player_oid,
            sm_ids::S1_ADENA_DISAPPEARED,
            &[SmParam::Long(item_count)],
        );
    } else {
        send_sm_with(
            world,
            player_oid,
            sm_ids::S2_S1_S_DISAPPEARED,
            &[SmParam::ItemName(item_id), SmParam::Long(item_count)],
        );
        send_sm_with(
            world,
            player_oid,
            sm_ids::S1_DISAPPEARED,
            &[SmParam::ItemName(item_id)],
        );
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.sp -= sp_cost;
    }
    send_sm_with(
        world,
        player_oid,
        sm_ids::YOUR_SP_HAS_DECREASED_BY_S1,
        &[SmParam::Int(sp_cost as i32)],
    );

    // Java refreshes the leader's SP (UserInfo CURRENT_HPMPCP_EXP_SP) + item
    // list; the full re-send stands in (the port's usual substitution).
    super::party::broadcast_user_info(world, player_oid);

    set_clan_level(world, clan_id, level + 1);

    // The level-up flourish: `MagicSkillUse(player, 5103, 1, 0, 0)` +
    // `MagicSkillLaunched`, broadcast from the leader.
    if let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&player_oid)
        .copied()
    {
        let use_pkt = server_packets::magic_skill_use_raw(
            (player_oid, pos.x, pos.y, pos.z),
            (player_oid, pos.x, pos.y, pos.z),
            5103,
            1,
            0,
        );
        super::helpers::broadcast_including_self(world, player_oid, &use_pkt);
        let launched = server_packets::magic_skill_launched(player_oid, 5103, 1, &[player_oid]);
        super::helpers::broadcast_including_self(world, player_oid, &launched);
    }
}

/// `VillageMaster.showPledgeSkillList`: the leader-only learnable pledge-skill
/// window. Non-leaders get `NotClanLeader.htm`; an empty list explains when to
/// come back (SM 607 below clan level 8, `NoMoreSkills.htm` at 8+); otherwise
/// `ExAcquirableSkillListByClass(PLEDGE)`.
pub(crate) fn show_pledge_skill_list(world: &World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_villagemaster_html(world, client_id, "NotClanLeader.htm");
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let available = world
        .data
        .pledge_skill_trees
        .available_pledge_skills(clan.level, &clan.skills);
    if available.is_empty() {
        if clan.level < 8 {
            let next = if clan.level < 5 { 5 } else { clan.level + 1 };
            send_sm_with(
                world,
                player_oid,
                sm_ids::YOU_DO_NOT_HAVE_ANY_FURTHER_SKILLS_TO_LEARN_COME_BACK_AT_LEVEL_S1,
                &[SmParam::Int(next)],
            );
        } else {
            send_villagemaster_html(world, client_id, "NoMoreSkills.htm");
        }
        return;
    }
    let rows: Vec<(i32, i32, i32, i64)> = available
        .iter()
        .map(|l| (l.skill_id, l.skill_level, l.get_level, l.level_up_sp))
        .collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_acquirable_skill_list_by_class(
            ACQUIRE_TYPE_PLEDGE,
            &rows,
        ));
    }
}

/// Serve a `data/html/villagemaster/<file>` window (Java `NpcHtmlMessage.
/// setFile` with no NPC binding — object id 0).
fn send_villagemaster_html(world: &World, client_id: u32, file: &str) {
    let html = crate::data::htm_cache::read_htm(format!(
        "{}data/html/villagemaster/{file}",
        world.data.root
    ))
    .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(0, &html));
    }
}

/// `RequestAcquireSkillInfo`'s PLEDGE branch: the leader clicked a skill in the
/// pledge list — answer with the reputation cost (`AcquireSkillInfo`).
pub(crate) fn handle_request_pledge_skill_info(
    world: &World,
    client_id: u32,
    skill_id: i32,
    skill_level: i32,
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id == 0 || !p.clan_leader {
        return;
    }
    let Some(learn) = world
        .data
        .pledge_skill_trees
        .pledge_skill(skill_id, skill_level)
    else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::acquire_skill_info(
            learn.skill_id,
            learn.skill_level,
            learn.level_up_sp,
            ACQUIRE_TYPE_PLEDGE as i32,
        ));
    }
}

/// `RequestAcquireSkill`'s PLEDGE case: the leader confirms a pledge-skill
/// learn — validate it is the clan's next level of a tree entry the clan
/// level qualifies for, spend clan reputation, and grant through
/// `add_clan_skill` (which broadcasts `PledgeSkillListAdd` + applies the
/// passive to qualifying members). No required items on this dist's pledge
/// tree, so `LifeCrystalNeeded`'s item loop has nothing to consume.
pub(crate) fn handle_learn_pledge_skill(
    world: &mut World,
    client_id: u32,
    skill_id: i32,
    skill_level: i32,
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let Some(learn) = world
        .data
        .pledge_skill_trees
        .pledge_skill(skill_id, skill_level)
        .cloned()
    else {
        return;
    };
    // Java's hack checks: the previous level must be the clan's current one
    // (`prevSkillLevel != _level - 1` reject) and the clan level must qualify
    // (the client list only ever offers qualifying entries).
    if clan.skills.get(&skill_id).copied().unwrap_or(0) != skill_level - 1
        || clan.level < learn.get_level
    {
        return;
    }
    let rep_cost = learn.level_up_sp as i32;
    if clan.reputation_score < rep_cost {
        send_sm_with(
            world,
            player,
            sm_ids::SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION,
            &[],
        );
        show_pledge_skill_list(world, client_id, player);
        return;
    }
    // `takeReputationScore` (negative add: clamp + persist + pledge-window
    // refresh to every online member).
    add_clan_reputation(world, clan_id, -rep_cost);
    send_sm_with(
        world,
        player,
        sm_ids::S1_POINTS_HAVE_BEEN_DEDUCTED_FROM_THE_CLAN_S_REPUTATION,
        &[SmParam::Int(rep_cost)],
    );
    add_clan_skill(world, clan_id, skill_id, skill_level);
    // Java broadcasts the full `PledgeSkillList` to online members, acks the
    // dialog, and re-opens the (now shorter) learnable list.
    let pkt = server_packets::pledge_skill_list(&clan_skill_pairs(world, clan_id));
    for oid in online_members(world, clan_id) {
        send_to_member(world, oid, pkt.clone());
    }
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::acquire_skill_done());
    }
    show_pledge_skill_list(world, client_id, player);
}

// --- G18 slice 3: ranks & power grades + delegated leader transfer ---------

use crate::model::clan::{CL_MANAGE_RANKS, RANK9_PRIVS_MASK};

/// Java `Clan.broadcastClanStatus` — reset every online member's clan window
/// (DeleteAll + a fresh MemberListAll).
fn broadcast_clan_status(world: &World, clan_id: i32) {
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let delete_all = server_packets::pledge_show_member_list_delete_all();
    let all = server_packets::pledge_show_member_list_all(clan, &world.objects);
    for oid in online_members(world, clan_id) {
        send_to_member(world, oid, delete_all.clone());
        send_to_member(world, oid, all.clone());
    }
}

/// `RequestPledgePower` (0xCC): the rank-privilege editor. Every request is
/// answered with `ManagePledgePower`; `action == 2` from the leader stores the
/// edited mask (`Clan.setRankPrivs`) — rank 9 (academy) clamped to the
/// bestowable subset — and refreshes online members holding that rank.
pub(crate) fn handle_request_pledge_power(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(rank) = r.read_i32() else { return };
    let Some(action) = r.read_i32() else { return };
    let privs = if action == 2 {
        r.read_i32().unwrap_or(0)
    } else {
        0
    };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    if clan_id == 0 {
        return;
    }
    if action == 2 && is_leader {
        let privs = if rank == 9 {
            privs & RANK9_PRIVS_MASK
        } else {
            privs
        };
        set_rank_privs(world, clan_id, rank, privs);
    }
    let current = world
        .clans
        .get(&clan_id)
        .map(|c| c.rank_privs_of(rank))
        .unwrap_or(0);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::manage_pledge_power(rank, action, current));
    }
}

/// Java `Clan.setRankPrivs`: store + persist the rank's mask, push it onto
/// every online member holding that grade (bitmask + UserInfo), then reset the
/// clan windows.
fn set_rank_privs(world: &mut World, clan_id: i32, rank: i32, privs: i32) {
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.rank_privs.insert(rank, privs);
    let leader_id = clan.leader_id;
    let member_ids: Vec<i32> = clan.members.iter().map(|m| m.char_id).collect();
    let _ = world.db.send(DbCommand::SaveClanRankPrivs {
        clan_id,
        rank,
        privs,
    });
    for oid in member_ids {
        if oid == leader_id {
            continue;
        }
        let holds_rank = world
            .objects
            .get_component::<Player>(&oid)
            .is_some_and(|p| p.power_grade == rank);
        if holds_rank {
            if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
                p.clan_privs = privs;
            }
            super::party::broadcast_user_info(world, oid);
        }
    }
    broadcast_clan_status(world, clan_id);
}

/// `RequestPledgePowerGradeList` (ex 0x13): the rank list — Java sends all 9
/// initialized ranks regardless of stored rows.
pub(crate) fn handle_request_pledge_power_grade_list(world: &World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id == 0 {
        return;
    }
    let ranks: Vec<i32> = (1..=9).collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_power_grade_list(&ranks));
    }
}

/// Resolve a named member of the acting player's clan; `None` when the player
/// is clanless or the name is not in the roster.
fn clan_member_by_name(
    world: &World,
    player: i32,
    name: &str,
) -> Option<(i32, crate::model::clan::ClanMember)> {
    let clan_id = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .filter(|&c| c != 0)?;
    let clan = world.clans.get(&clan_id)?;
    clan.members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(name))
        .map(|m| (clan_id, m.clone()))
}

/// `RequestPledgeMemberPowerInfo` (ex 0x14): one member's rank + that rank's
/// current privilege mask.
pub(crate) fn handle_request_pledge_member_power_info(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(_unk) = r.read_i32() else { return };
    let Some(name) = r.read_string() else { return };
    let Some((clan_id, member)) = clan_member_by_name(world, player, &name) else {
        return;
    };
    // The live grade for an online member (roster snapshots refresh lazily).
    let grade = world
        .objects
        .get_component::<Player>(&member.char_id)
        .map(|p| p.power_grade)
        .unwrap_or(member.power_grade);
    let privs = world
        .clans
        .get(&clan_id)
        .map(|c| c.rank_privs_of(grade))
        .unwrap_or(0);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_receive_power_info(
            grade,
            &member.name,
            privs,
        ));
    }
}

/// `RequestPledgeMemberInfo` (ex 0x16): the member-detail pane.
pub(crate) fn handle_request_pledge_member_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(_unk) = r.read_i32() else { return };
    let Some(name) = r.read_string() else { return };
    let Some((clan_id, mut member)) = clan_member_by_name(world, player, &name) else {
        return;
    };
    // Live title/grade for online members.
    if let Some(p) = world.objects.get_component::<Player>(&member.char_id) {
        member.title = p.title.clone();
        member.power_grade = p.power_grade;
    }
    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_receive_member_info(
            &member, &clan_name,
        ));
    }
}

/// `RequestPledgeSetMemberPowerGrade` (ex 0x15): a CL_MANAGE_RANKS holder
/// re-ranks a member (never the leader). The new rank's privileges apply to
/// the online member immediately through the rank table refresh at
/// `broadcastClanStatus`-time in Java only on relog — we mirror Java: the
/// grade changes now, the mask follows at login/rank-edit.
pub(crate) fn handle_request_pledge_set_member_power_grade(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(name) = r.read_string() else { return };
    let Some(grade) = r.read_i32() else { return };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    let has_priv = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_MANAGE_RANKS));
    if !has_priv {
        return;
    }
    let Some((_, member)) = clan_member_by_name(world, player, &name) else {
        return;
    };
    let leader_id = world.clans.get(&clan_id).map(|c| c.leader_id).unwrap_or(0);
    if member.char_id == leader_id {
        return;
    }
    // TODO(G18.6): Java rejects academy members (SM 1754) — no academy yet.

    if let Some(c) = world.clans.get_mut(&clan_id) {
        if let Some(m) = c.members.iter_mut().find(|m| m.char_id == member.char_id) {
            m.power_grade = grade;
        }
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&member.char_id) {
        p.power_grade = grade;
    }
    let _ = world.db.send(DbCommand::UpdateCharPowerGrade {
        char_id: member.char_id,
        power_grade: grade,
    });

    let online = client_for_player(world, member.char_id).is_some();
    let update = {
        let c = world.clans.get(&clan_id).expect("checked above");
        c.member(member.char_id)
            .map(|m| server_packets::pledge_show_member_list_update(m, online))
    };
    if let Some(pkt) = update {
        broadcast_to_clan(world, clan_id, &pkt);
    }
    let sm = server_packets::system_message_with(
        sm_ids::CLAN_MEMBER_C1_S_PRIVILEGE_LEVEL_HAS_BEEN_CHANGED_TO_S2,
        &[SmParam::Text(member.name.clone()), SmParam::Int(grade)],
    );
    broadcast_to_clan(world, clan_id, &sm);
    broadcast_clan_status(world, clan_id);
}

/// `RequestPledgeReorganizeMember` (ex 0x2C): the leader (or a
/// CL_MANAGE_RANKS holder) swaps two main-pledge-or-below members' sub-unit
/// membership — `member_name` takes `new_pledge_type`, `selected_member`
/// takes whatever `member_name` had.
pub(crate) fn handle_request_pledge_reorganize_member(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(is_selected) = r.read_i32() else {
        return;
    };
    let Some(member_name) = r.read_string() else {
        return;
    };
    let Some(new_pledge_type) = r.read_i32() else {
        return;
    };
    let Some(selected_member) = r.read_string() else {
        return;
    };
    if is_selected == 0 {
        return;
    }
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if !clan.has_privilege(player, privs, CL_MANAGE_RANKS) {
        return;
    }
    // A malformed/hacked target type: the client only ever offers 0 or a real
    // sub-unit id, so anything else is dropped defensively.
    if new_pledge_type != 0 && !clan.sub_pledges.contains_key(&new_pledge_type) {
        return;
    }
    let leader_id = clan.leader_id;
    let Some(m1) = clan
        .members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(&member_name))
        .cloned()
    else {
        return;
    };
    let Some(m2) = clan
        .members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(&selected_member))
        .cloned()
    else {
        return;
    };
    if m1.char_id == leader_id || m2.char_id == leader_id {
        return;
    }
    let old_pledge_type = m1.pledge_type;
    if old_pledge_type == new_pledge_type {
        return;
    }

    if let Some(c) = world.clans.get_mut(&clan_id) {
        if let Some(m) = c.members.iter_mut().find(|m| m.char_id == m1.char_id) {
            m.pledge_type = new_pledge_type;
        }
        if let Some(m) = c.members.iter_mut().find(|m| m.char_id == m2.char_id) {
            m.pledge_type = old_pledge_type;
        }
    }
    for (oid, pledge_type) in [(m1.char_id, new_pledge_type), (m2.char_id, old_pledge_type)] {
        let _ = world.db.send(DbCommand::UpdateCharPledgeType {
            char_id: oid,
            pledge_type,
        });
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map(|c| c.pledge_class_of(oid))
            .unwrap_or(0);
        if let Some(mp) = world.objects.get_component_mut::<Player>(&oid) {
            mp.pledge_type = pledge_type;
            mp.pledge_class = pledge_class;
        }
        super::party::broadcast_user_info(world, oid);
    }
    broadcast_clan_status(world, clan_id);
}

/// `VillageMaster`'s `change_clan_leader <name>` bypass — the delegated
/// transfer flow (`AltClanLeaderInstantActivation = False` on this dist):
/// stamp `new_leader_id` + the confirmation html. The actual `setNewLeader`
/// application runs at the daily reset — TODO(G33): `DailyTaskManager.
/// onClanLeaderChange` (no daily scheduler yet, so the stamp waits).
pub(crate) fn handle_change_clan_leader(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_oid: i32,
    args: &str,
) {
    let name = args.trim();
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let player_name = p.name.clone();
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    if player_name.eq_ignore_ascii_case(name) {
        return;
    }
    let Some((_, member)) = clan_member_by_name(world, player_oid, name) else {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::system_message_with(
                sm_ids::S1_DOES_NOT_EXIST,
                &[SmParam::Text(name.to_string())],
            ));
        }
        return;
    };
    if client_for_player(world, member.char_id).is_none() {
        send_sm(
            world,
            client_id,
            sm_ids::THAT_PLAYER_IS_NOT_CURRENTLY_ONLINE,
        );
        return;
    }
    // TODO(G18.6): Java rejects academy members (SM 1754) — no academy yet.
    let already_pending = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.new_leader_id != 0);
    let file = if already_pending {
        "9000-07-in-progress.htm"
    } else {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.new_leader_id = member.char_id;
        }
        let _ = world.db.send(DbCommand::UpdateClanNewLeader {
            clan_id,
            new_leader_id: member.char_id,
        });
        "9000-07-success.htm"
    };
    send_clan_master_html(world, client_id, npc_oid, file);
}

/// `VillageMaster`'s `cancel_clan_leader_change` bypass.
pub(crate) fn handle_cancel_clan_leader_change(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    npc_oid: i32,
) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let pending = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.new_leader_id != 0);
    if pending {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.new_leader_id = 0;
        }
        let _ = world.db.send(DbCommand::UpdateClanNewLeader {
            clan_id,
            new_leader_id: 0,
        });
        send_clan_master_html(world, client_id, npc_oid, "9000-07-canceled.htm");
    } else if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(
            npc_oid,
            "<html><body>You don't have clan leader delegation applications submitted yet!</body></html>",
        ));
    }
}

/// Serve a `data/scripts/village_master/ClanMaster/<file>` page through the
/// clicked NPC (the leader-transfer confirmations live with the script htmls).
fn send_clan_master_html(world: &World, client_id: u32, npc_oid: i32, file: &str) {
    let html = crate::data::htm_cache::read_htm(format!(
        "{}data/scripts/village_master/ClanMaster/{file}",
        world.data.root
    ))
    .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string())
    .replace("%objectId%", &npc_oid.to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(npc_oid, &html));
    }
}

// --- G18 slice 4: clan wars ------------------------------------------------

use crate::model::clan::{ClanWar, ClanWarState, CL_PLEDGE_WAR, WAR_TIMEOUT_MS};

/// `AltClanMembersForWar = 15` on this dist.
const CLAN_MEMBERS_FOR_WAR: usize = 15;

/// `ReputationScorePerKill = 1` (Feature.ini) — the mutual-war kill transfer.
const REPUTATION_SCORE_PER_KILL: i32 = 1;

/// The war between two clans, either direction (Java `Clan.getWarWith`).
pub(crate) fn war_between<'a>(world: &'a World, a: i32, b: i32) -> Option<&'a ClanWar> {
    world.clan_wars.iter().find(|w| {
        (w.attacker_id == a && w.attacked_id == b) || (w.attacker_id == b && w.attacked_id == a)
    })
}

fn war_between_mut<'a>(world: &'a mut World, a: i32, b: i32) -> Option<&'a mut ClanWar> {
    world.clan_wars.iter_mut().find(|w| {
        (w.attacker_id == a && w.attacked_id == b) || (w.attacker_id == b && w.attacked_id == a)
    })
}

/// Whether a **mutual** war runs between the two clans — the state that makes
/// kills lawful (`checkIfPvP`) and both sides freely attackable.
pub(crate) fn mutual_war_between(world: &World, a: i32, b: i32) -> bool {
    a != 0 && b != 0 && war_between(world, a, b).is_some_and(|w| w.state == ClanWarState::Mutual)
}

/// Java `Player.atWarWith` (any war, whatever state) — quarters the death-XP
/// penalty when the killer is a war enemy.
pub(crate) fn at_war_between(world: &World, a: i32, b: i32) -> bool {
    a != 0 && b != 0 && war_between(world, a, b).is_some()
}

/// Java `Clan.isAtWar` — the dissolve gate.
pub(crate) fn clan_is_at_war(world: &World, clan_id: i32) -> bool {
    world.clan_wars.iter().any(|w| w.involves(clan_id))
}

fn store_war(world: &World, war: &ClanWar) {
    let _ = world.db.send(DbCommand::SaveClanWar {
        attacker: war.attacker_id,
        attacked: war.attacked_id,
        attacker_kills: war.attacker_kills,
        attacked_kills: war.attacked_kills,
        winner: war.winner_id,
        start_time: war.start_time,
        end_time: war.end_time,
        state: war.state as i32,
    });
}

/// Java `RelationChanged`'s war bits, from `Player.getRelation(target)`:
/// the subject's clan at war with the viewer's — one sword for a pending
/// declaration *by the subject's side*, both swords for MUTUAL.
pub(crate) fn war_relation_bits(world: &World, subject_oid: i32, viewer_oid: i32) -> i32 {
    const RELATION_DECLARED_WAR: i32 = 0x4000; // single sword
    const RELATION_MUTUAL_WAR: i32 = 0x8000; // double swords
    let subject_clan = world
        .objects
        .get_component::<Player>(&subject_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let viewer_clan = world
        .objects
        .get_component::<Player>(&viewer_oid)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    if subject_clan == 0 || viewer_clan == 0 || subject_clan == viewer_clan {
        return 0;
    }
    let Some(war) = war_between(world, subject_clan, viewer_clan) else {
        return 0;
    };
    match war.state {
        ClanWarState::Declaration | ClanWarState::BloodDeclaration => {
            if war.attacker_id != viewer_clan {
                RELATION_DECLARED_WAR
            } else {
                0
            }
        }
        ClanWarState::Mutual => RELATION_DECLARED_WAR | RELATION_MUTUAL_WAR,
        _ => 0,
    }
}

/// `broadcastUserInfo(UserInfoType.CLAN)` to every online member of both war
/// sides + the per-viewer relation refresh (the war swords ride
/// `RelationChanged`).
fn broadcast_war_status(world: &mut World, clan_a: i32, clan_b: i32) {
    for clan_id in [clan_a, clan_b] {
        for oid in online_members(world, clan_id) {
            super::party::broadcast_user_info(world, oid);
            super::pvp::broadcast_siege_relation(world, oid);
        }
    }
}

/// The war tab rows for one clan's `PledgeReceiveWarList`.
fn war_list_rows(world: &World, clan_id: i32) -> Vec<(String, i32, i32, i32, i32)> {
    world
        .clan_wars
        .iter()
        .filter(|w| w.involves(clan_id))
        .filter_map(|w| {
            let other = world.clans.get(&w.opposing(clan_id))?;
            Some((
                other.name.clone(),
                w.state_for(clan_id) as i32,
                w.remaining_time(),
                w.kill_difference(clan_id),
                w.kill_to_start(),
            ))
        })
        .collect()
}

fn send_war_list(world: &World, client_id: u32, clan_id: i32, tab: i32) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_receive_war_list(
            tab,
            &war_list_rows(world, clan_id),
        ));
    }
}

/// `RequestPledgeWarList` (ex 0x17).
pub(crate) fn handle_request_pledge_war_list(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let _unk = r.read_i32();
    let tab = r.read_i32().unwrap_or(0);
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id == 0 {
        return;
    }
    send_war_list(world, client_id, p.clan_id, tab);
}

/// `RequestStartPledgeWar` (0x03): declare war by clan name — the full Java
/// guard chain, the redeclare-makes-mutual branch, then a fresh
/// BLOOD_DECLARATION war with the 7-day answer window.
pub(crate) fn handle_request_start_pledge_war(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.level < 3 || clan.members.len() < CLAN_MEMBERS_FOR_WAR {
        send_sm_with(
            world,
            player,
            sm_ids::CLAN_WAR_NEEDS_LEVEL_3_AND_15_MEMBERS,
            &[],
        );
        return;
    }
    if !clan.has_privilege(player, privs, CL_PLEDGE_WAR) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if world
        .clan_wars
        .iter()
        .filter(|w| w.involves(clan_id))
        .count()
        >= 30
    {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_WAR_ON_MORE_THAN_30_CLANS,
            &[],
        );
        return;
    }
    let Some(target) = world
        .clans
        .values()
        .find(|c| c.name.eq_ignore_ascii_case(&name))
    else {
        send_sm_with(world, player, sm_ids::CLAN_WAR_TARGET_DOES_NOT_EXIST, &[]);
        return;
    };
    let target_id = target.id;
    if target_id == clan_id {
        send_sm_with(
            world,
            player,
            sm_ids::FOOL_YOU_CANNOT_DECLARE_WAR_AGAINST_YOUR_OWN_CLAN,
            &[],
        );
        return;
    }
    let same_ally = {
        let a = world.clans.get(&clan_id).map(|c| c.ally_id).unwrap_or(0);
        a != 0 && a == target.ally_id
    };
    if same_ally {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_WAR_ON_ALLIED_CLAN,
            &[],
        );
        return;
    }
    if target.level < 3 || target.members.len() < CLAN_MEMBERS_FOR_WAR {
        send_sm_with(
            world,
            player,
            sm_ids::CLAN_WAR_NEEDS_LEVEL_3_AND_15_MEMBERS,
            &[],
        );
        return;
    }
    if target.dissolving_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_WAR_ON_DISSOLVING_CLAN,
            &[],
        );
        return;
    }
    let target_name = target.name.clone();

    if let Some(war) = war_between(world, clan_id, target_id) {
        match war.state_for(clan_id) {
            ClanWarState::Win => {
                send_sm_with(
                    world,
                    player,
                    sm_ids::CANNOT_DECLARE_WAR_21_DAYS_AFTER_DEFEAT_WITH_S1,
                    &[SmParam::Text(target_name)],
                );
                return;
            }
            ClanWarState::Mutual => {
                send_sm_with(
                    world,
                    player,
                    sm_ids::S1_TEXT,
                    &[SmParam::Text(format!(
                        "You have already been at war with {target_name}."
                    ))],
                );
                return;
            }
            ClanWarState::BloodDeclaration | ClanWarState::Declaration => {
                // Java `mutualClanWarAccepted`: the declaration answered in kind
                // goes MUTUAL (the pending timeout no-ops on the state change).
                if let Some(w) = war_between_mut(world, clan_id, target_id) {
                    w.state = ClanWarState::Mutual;
                }
                let war = war_between(world, clan_id, target_id)
                    .expect("just updated")
                    .clone();
                store_war(world, &war);
                let started_a = server_packets::system_message_with(
                    sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                    &[SmParam::Text(target_name.clone())],
                );
                let clan_name = world
                    .clans
                    .get(&clan_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                let started_b = server_packets::system_message_with(
                    sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                    &[SmParam::Text(clan_name)],
                );
                broadcast_to_clan(world, clan_id, &started_a);
                broadcast_to_clan(world, target_id, &started_b);
                broadcast_war_status(world, clan_id, target_id);
                send_war_list(world, client_id, clan_id, 0);
                return;
            }
            _ => {}
        }
    }

    // A fresh declaration.
    let war = ClanWar {
        attacker_id: clan_id,
        attacked_id: target_id,
        state: ClanWarState::BloodDeclaration,
        winner_id: 0,
        start_time: now_millis(),
        end_time: 0,
        attacker_kills: 0,
        attacked_kills: 0,
    };
    store_war(world, &war);
    let timeout_ticks = (WAR_TIMEOUT_MS / MS_PER_TICK) as u64;
    world.scheduler.schedule(
        world.tick + timeout_ticks,
        crate::scheduler::ScheduledTask::ClanWarTimeout {
            attacker: clan_id,
            attacked: target_id,
        },
    );
    world.clan_wars.push(war);
    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let declared = server_packets::system_message_with(
        sm_ids::YOU_HAVE_DECLARED_A_CLAN_WAR_WITH_S1,
        &[SmParam::Text(target_name.clone())],
    );
    broadcast_to_clan(world, clan_id, &declared);
    let warned = server_packets::system_message_with(
        sm_ids::S1_HAS_DECLARED_A_CLAN_WAR_KILL_5_TO_START,
        &[SmParam::Text(clan_name)],
    );
    broadcast_to_clan(world, target_id, &warned);
    broadcast_war_status(world, clan_id, target_id);
    send_war_list(world, client_id, clan_id, 0);
}

/// `RequestStopPledgeWar` (0x05): a mutual cease-fire — costs 500 reputation,
/// blocked while any clan member is in combat.
pub(crate) fn handle_request_stop_pledge_war(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    let Some(target) = world
        .clans
        .values()
        .find(|c| c.name.eq_ignore_ascii_case(&name))
    else {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text("No such clan.".to_string())],
        );
        return;
    };
    let target_id = target.id;
    if war_between(world, clan_id, target_id).is_none() {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text(
                "You aren't at war with this clan.".to_string(),
            )],
        );
        return;
    }
    let has_priv = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_PLEDGE_WAR));
    if !has_priv {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if world
        .clans
        .get(&clan_id)
        .map(|c| c.reputation_score)
        .unwrap_or(0)
        <= 500
    {
        send_sm_with(world, player, sm_ids::THE_CLAN_REPUTATION_IS_TOO_LOW, &[]);
        return;
    }
    let member_in_combat = online_members(world, clan_id)
        .iter()
        .any(|&oid| super::combat::has_attack_stance(world, oid));
    if member_in_combat {
        send_sm_with(
            world,
            player,
            sm_ids::CEASE_FIRE_CANNOT_BE_CALLED_WHILE_MEMBERS_IN_BATTLE,
            &[],
        );
        return;
    }

    add_clan_reputation(world, clan_id, -500);
    let lost = crate::network::enter_world::system_message(
        sm_ids::YOUR_CLAN_LOST_500_REPUTATION_FOR_WITHDRAWING_FROM_THE_WAR,
    );
    broadcast_to_clan(world, clan_id, &lost);
    delete_clan_wars(world, clan_id, target_id);
    broadcast_war_status(world, clan_id, target_id);
}

/// `RequestSurrenderPledgeWar` (0x07) → `ClanWar.cancel`: declare defeat in a
/// mutual war — 500 reputation, the other side wins, the war ends and is torn
/// down moments later.
pub(crate) fn handle_request_surrender_pledge_war(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    let player_name = p.name.clone();
    if clan_id == 0 {
        return;
    }
    let member_in_combat = online_members(world, clan_id)
        .iter()
        .any(|&oid| super::combat::has_attack_stance(world, oid));
    if member_in_combat {
        send_sm_with(
            world,
            player,
            sm_ids::CEASE_FIRE_CANNOT_BE_CALLED_WHILE_MEMBERS_IN_BATTLE,
            &[],
        );
        return;
    }
    let Some(target) = world
        .clans
        .values()
        .find(|c| c.name.eq_ignore_ascii_case(&name))
    else {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text("No such clan.".to_string())],
        );
        return;
    };
    let target_id = target.id;
    let target_name = target.name.clone();
    let has_priv = world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_PLEDGE_WAR));
    if !has_priv {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    let Some(war) = war_between(world, clan_id, target_id) else {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_HAVE_NOT_DECLARED_A_CLAN_WAR_AGAINST_THE_CLAN_S1,
            &[SmParam::Text(target_name.clone())],
        );
        return;
    };
    if war.state == ClanWarState::BloodDeclaration {
        send_sm_with(
            world,
            player,
            sm_ids::CANNOT_DECLARE_DEFEAT_BEFORE_7_DAYS_WITH_CLAN_S1,
            &[SmParam::Text(target_name.clone())],
        );
        return;
    }

    // `ClanWar.cancel(player, cancelor)`.
    add_clan_reputation(world, clan_id, -500);
    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::surrender_pledge_war(
            &clan_name,
            &player_name,
        ));
    }
    let lost = server_packets::system_message_with(
        sm_ids::THE_WAR_ENDED_BY_YOUR_DEFEAT_DECLARATION_WITH_THE_S1_CLAN,
        &[SmParam::Text(target_name)],
    );
    broadcast_to_clan(world, clan_id, &lost);
    let won = server_packets::system_message_with(
        sm_ids::THE_WAR_ENDED_BY_THE_S1_CLAN_S_DEFEAT_DECLARATION,
        &[SmParam::Text(clan_name)],
    );
    broadcast_to_clan(world, target_id, &won);
    if let Some(w) = war_between_mut(world, clan_id, target_id) {
        w.winner_id = target_id;
        w.end_time = now_millis();
    }
    let war = war_between(world, clan_id, target_id)
        .expect("just updated")
        .clone();
    store_war(world, &war);
    // Java tears the ended war down 5 s later (the 21-day retention constant is
    // dead code in the live path).
    world.scheduler.schedule(
        world.tick + 50,
        crate::scheduler::ScheduledTask::ClanWarDelete {
            clan1: clan_id,
            clan2: target_id,
        },
    );
    broadcast_war_status(world, clan_id, target_id);
}

/// `ClanTable.deleteClanWars` — drop the war from memory + DB and reset both
/// clans' windows.
pub(crate) fn delete_clan_wars(world: &mut World, clan1: i32, clan2: i32) {
    world.clan_wars.retain(|w| {
        !((w.attacker_id == clan1 && w.attacked_id == clan2)
            || (w.attacker_id == clan2 && w.attacked_id == clan1))
    });
    let _ = world.db.send(DbCommand::DeleteClanWar { clan1, clan2 });
    broadcast_clan_status(world, clan1);
    broadcast_clan_status(world, clan2);
}

/// `ClanWar.clanWarTimeout` — 7 days of BLOOD_DECLARATION without an answer:
/// the war goes TIE and is torn down. A war gone MUTUAL in the meantime makes
/// this a no-op (Java cancels the task; the state check is our seq).
pub(crate) fn handle_clan_war_timeout(world: &mut World, attacker: i32, attacked: i32) {
    let Some(war) = war_between(world, attacker, attacked) else {
        return;
    };
    if war.state != ClanWarState::BloodDeclaration && war.state != ClanWarState::Declaration {
        return;
    }
    let attacker_name = world
        .clans
        .get(&attacker)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let attacked_name = world
        .clans
        .get(&attacked)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let cancelled = server_packets::system_message_with(
        sm_ids::A_CLAN_WAR_DECLARED_BY_CLAN_S1_WAS_CANCELLED,
        &[SmParam::Text(attacker_name)],
    );
    broadcast_to_clan(world, attacked, &cancelled);
    let no_fight_back = server_packets::system_message_with(
        sm_ids::BECAUSE_CLAN_S1_DID_NOT_FIGHT_BACK_THE_WAR_WAS_CANCELLED,
        &[SmParam::Text(attacked_name)],
    );
    broadcast_to_clan(world, attacker, &no_fight_back);
    if let Some(w) = war_between_mut(world, attacker, attacked) {
        w.state = ClanWarState::Tie;
        w.end_time = now_millis();
    }
    let war = war_between(world, attacker, attacked)
        .expect("just updated")
        .clone();
    store_war(world, &war);
    world.scheduler.schedule(
        world.tick + 50,
        crate::scheduler::ScheduledTask::ClanWarDelete {
            clan1: attacker,
            clan2: attacked,
        },
    );
    broadcast_war_status(world, attacker, attacked);
}

/// Boot re-arm (Java `ClanWar`'s restore constructor): pending declarations
/// get their remaining answer window; already-ended wars are torn down
/// shortly after boot (the live Java path's behavior).
pub(crate) fn rearm_clan_wars_at_boot(world: &mut World) {
    let now = now_millis();
    let wars: Vec<(i32, i32, i64, ClanWarState)> = world
        .clan_wars
        .iter()
        .map(|w| (w.attacker_id, w.attacked_id, w.start_time, w.state))
        .collect();
    for (attacker, attacked, start, state) in wars {
        let ended = world
            .clan_wars
            .iter()
            .find(|w| w.attacker_id == attacker && w.attacked_id == attacked)
            .is_some_and(|w| w.end_time > 0);
        if ended {
            world.scheduler.schedule(
                world.tick + 100,
                crate::scheduler::ScheduledTask::ClanWarDelete {
                    clan1: attacker,
                    clan2: attacked,
                },
            );
        } else if matches!(
            state,
            ClanWarState::BloodDeclaration | ClanWarState::Declaration
        ) {
            let remaining_ticks = (((start + WAR_TIMEOUT_MS) - now).max(0) / MS_PER_TICK) as u64;
            world.scheduler.schedule(
                world.tick + remaining_ticks,
                crate::scheduler::ScheduledTask::ClanWarTimeout { attacker, attacked },
            );
        }
    }
}

/// Java `ClanWar.onKill` — a war-relevant player kill. The caller (the death
/// pipeline) has already checked: killer and victim are players outside
/// PVP/siege zones, both clanned. (Java also exempts academy members —
/// TODO(G18.6) — and runs an AntiFeed check, unported.)
pub(crate) fn clan_war_on_kill(world: &mut World, killer_oid: i32, victim_oid: i32) {
    let (killer_clan, killer_name) = match world.objects.get_component::<Player>(&killer_oid) {
        Some(p) => (p.clan_id, p.name.clone()),
        None => return,
    };
    let (victim_clan, victim_name, victim_level, victim_rep) =
        match world.objects.get_component::<Player>(&victim_oid) {
            Some(p) => (p.clan_id, p.name.clone(), p.level, p.reputation),
            None => return,
        };
    if killer_clan == 0 || victim_clan == 0 {
        return;
    }
    let Some(war) = war_between(world, killer_clan, victim_clan) else {
        return;
    };
    let (state, attacker_id) = (war.state, war.attacker_id);

    if victim_level > 4 && state == ClanWarState::Mutual {
        // Mutual war: 1 reputation moves from the victim's clan to the
        // killer's — but only while the victim clan has any to lose.
        if world
            .clans
            .get(&victim_clan)
            .map(|c| c.reputation_score)
            .unwrap_or(0)
            > 0
        {
            add_clan_reputation(world, victim_clan, -REPUTATION_SCORE_PER_KILL);
            add_clan_reputation(world, killer_clan, REPUTATION_SCORE_PER_KILL);
        }
        let killer_clan_name = world
            .clans
            .get(&killer_clan)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let victim_clan_name = world
            .clans
            .get(&victim_clan)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let down = server_packets::system_message_with(
            sm_ids::BECAUSE_C1_KILLED_BY_S2_CLAN_REPUTATION_DECREASED_BY_1,
            &[
                SmParam::PlayerName(victim_name),
                SmParam::Text(killer_clan_name),
            ],
        );
        broadcast_to_clan_except(world, victim_clan, victim_oid, &down);
        let up = server_packets::system_message_with(
            sm_ids::BECAUSE_S1_MEMBER_KILLED_BY_C2_CLAN_REPUTATION_INCREASED_BY_1,
            &[
                SmParam::Text(victim_clan_name),
                SmParam::PlayerName(killer_name),
            ],
        );
        broadcast_to_clan_except(world, killer_clan, killer_oid, &up);
        if let Some(w) = war_between_mut(world, killer_clan, victim_clan) {
            if killer_clan == attacker_id {
                w.attacker_kills += 1;
            } else {
                w.attacked_kills += 1;
            }
        }
    } else if state == ClanWarState::BloodDeclaration
        && victim_clan == attacker_id
        && victim_rep >= 0
    {
        // The attacked side kills a declarer: 5 such kills force the war MUTUAL.
        let kill_count = {
            let w = war_between_mut(world, killer_clan, victim_clan).expect("checked above");
            w.attacked_kills += 1;
            w.attacked_kills
        };
        if kill_count >= 5 {
            if let Some(w) = war_between_mut(world, killer_clan, victim_clan) {
                w.state = ClanWarState::Mutual;
            }
            let killer_clan_name = world
                .clans
                .get(&killer_clan)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let victim_clan_name = world
                .clans
                .get(&victim_clan)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let started_k = server_packets::system_message_with(
                sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                &[SmParam::Text(victim_clan_name)],
            );
            broadcast_to_clan(world, killer_clan, &started_k);
            let started_v = server_packets::system_message_with(
                sm_ids::CLAN_WAR_STARTED_WITH_CLAN_S1,
                &[SmParam::Text(killer_clan_name)],
            );
            broadcast_to_clan(world, victim_clan, &started_v);
            broadcast_war_status(world, killer_clan, victim_clan);
        } else {
            let victim_clan_name = world
                .clans
                .get(&victim_clan)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            let progress = server_packets::system_message_with(
                sm_ids::S1_MEMBER_KILLED_S2_MORE_KILLS_TO_START_WAR,
                &[
                    SmParam::Text(victim_clan_name),
                    SmParam::Int(5 - kill_count),
                ],
            );
            broadcast_to_clan(world, killer_clan, &progress);
        }
    } else {
        return;
    }
    let war = war_between(world, killer_clan, victim_clan)
        .expect("checked above")
        .clone();
    store_war(world, &war);
}

/// `Clan.broadcastToOtherOnlineMembers` — every online member except `except`.
fn broadcast_to_clan_except(world: &World, clan_id: i32, except: i32, pkt: &[u8]) {
    for oid in online_members(world, clan_id) {
        if oid != except {
            send_to_member(world, oid, pkt.to_vec());
        }
    }
}

// --- G18 slice 5: alliances ------------------------------------------------

use crate::model::clan::{
    ALLY_PENALTY_TYPE_CLAN_DISMISSED, ALLY_PENALTY_TYPE_CLAN_LEAVED,
    ALLY_PENALTY_TYPE_DISMISS_CLAN, ALLY_PENALTY_TYPE_DISSOLVE_ALLY,
};

/// `AltMaxNumOfClansInAlly = 3` on this dist.
const MAX_CLANS_IN_ALLY: usize = 3;

/// The ally penalties all run `DaysBefore… = 1` day on this dist.
const ALLY_PENALTY_MS: i64 = 86_400_000;

/// Persist a clan's ally membership + penalty stamps (the ally half of
/// `Clan.updateClanInDB`).
fn store_clan_ally(world: &World, clan_id: i32) {
    let Some(c) = world.clans.get(&clan_id) else {
        return;
    };
    let _ = world.db.send(DbCommand::UpdateClanAlly {
        clan_id,
        ally_id: c.ally_id,
        ally_name: c.ally_name.clone(),
        penalty_expiry: c.ally_penalty_expiry_time,
        penalty_type: c.ally_penalty_type,
    });
}

/// Sync every online member's denormalized `Player.ally_id` with the clan and
/// re-broadcast their UserInfo/CharInfo (the ally id rides both).
fn refresh_ally_on_members(world: &mut World, clan_id: i32) {
    let (ally_id, ally_crest_id) = world
        .clans
        .get(&clan_id)
        .map(|c| (c.ally_id, c.ally_crest_id))
        .unwrap_or((0, 0));
    for oid in online_members(world, clan_id) {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.ally_id = ally_id;
            p.ally_crest_id = ally_crest_id;
        }
        super::party::broadcast_user_info(world, oid);
    }
}

/// The clans of one alliance (Java `ClanTable.getClanAllies`).
fn ally_clan_ids(world: &World, ally_id: i32) -> Vec<i32> {
    if ally_id == 0 {
        return Vec::new();
    }
    world
        .clans
        .values()
        .filter(|c| c.ally_id == ally_id)
        .map(|c| c.id)
        .collect()
}

/// `Clan.broadcastToOnlineAllyMembers`.
fn broadcast_to_ally(world: &World, ally_id: i32, pkt: &[u8]) {
    for clan_id in ally_clan_ids(world, ally_id) {
        broadcast_to_clan(world, clan_id, pkt);
    }
}

/// `VillageMaster.onBypassFeedback`'s `create_ally` branch → `Clan.createAlly`:
/// the guard chain, then the clan becomes its own alliance's leader.
pub(crate) fn handle_create_ally(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let name = args.trim();
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(
            world,
            client_id,
            sm_ids::ONLY_CLAN_LEADERS_MAY_CREATE_ALLIANCES,
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.ally_id != 0 {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_ALREADY_BELONG_TO_ANOTHER_ALLIANCE,
        );
        return;
    }
    if clan.level < 5 {
        send_sm(
            world,
            client_id,
            sm_ids::TO_CREATE_AN_ALLIANCE_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER,
        );
        return;
    }
    if clan.ally_penalty_expiry_time > now_millis()
        && clan.ally_penalty_type == ALLY_PENALTY_TYPE_DISSOLVE_ALLY
    {
        send_sm(
            world,
            client_id,
            sm_ids::CANNOT_CREATE_A_NEW_ALLIANCE_WITHIN_1_DAY_OF_DISSOLUTION,
        );
        return;
    }
    if clan.dissolving_expiry_time > now_millis() {
        send_sm(
            world,
            client_id,
            sm_ids::SCHEDULED_FOR_CLAN_DISSOLUTION_NO_ALLIANCE_CAN_BE_CREATED,
        );
        return;
    }
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        send_sm(world, client_id, sm_ids::INCORRECT_ALLIANCE_NAME);
        return;
    }
    if name.len() > 16 || name.len() < 2 {
        send_sm(
            world,
            client_id,
            sm_ids::INCORRECT_LENGTH_FOR_AN_ALLIANCE_NAME,
        );
        return;
    }
    if world
        .clans
        .values()
        .any(|c| c.ally_name.eq_ignore_ascii_case(name))
    {
        send_sm(world, client_id, sm_ids::THAT_ALLIANCE_NAME_ALREADY_EXISTS);
        return;
    }

    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_id = clan_id;
        c.ally_name = name.to_string();
        c.ally_penalty_expiry_time = 0;
        c.ally_penalty_type = 0;
    }
    store_clan_ally(world, clan_id);
    refresh_ally_on_members(world, clan_id);
    // Java: "TODO: Need correct message id" — a plain text line.
    send_sm_with(
        world,
        player_oid,
        sm_ids::S1_TEXT,
        &[SmParam::Text(format!("Alliance {name} has been created."))],
    );
}

/// `Clan.dissolveAlly` (the `dissolve_ally` bypass and `RequestDismissAlly`).
pub(crate) fn handle_dissolve_ally(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let ally_id = clan.ally_id;
    if ally_id == 0 {
        send_sm(
            world,
            client_id,
            sm_ids::YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS,
        );
        return;
    }
    if !is_leader || clan_id != ally_id {
        send_sm(
            world,
            client_id,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
        );
        return;
    }
    if let Some(pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&player_oid)
    {
        if world
            .data
            .zone_data
            .siege_castle_at(pos.x, pos.y, pos.z)
            .is_some()
        {
            send_sm(
                world,
                client_id,
                sm_ids::CANNOT_DISSOLVE_ALLIANCE_WHILE_AFFILIATED_CLAN_IN_SIEGE,
            );
            return;
        }
    }

    let dissolved =
        crate::network::enter_world::system_message(sm_ids::THE_ALLIANCE_HAS_BEEN_DISSOLVED);
    broadcast_to_ally(world, ally_id, &dissolved);

    for cid in ally_clan_ids(world, ally_id) {
        if cid == clan_id {
            continue;
        }
        if let Some(c) = world.clans.get_mut(&cid) {
            c.ally_id = 0;
            c.ally_name.clear();
            c.ally_penalty_expiry_time = 0;
            c.ally_penalty_type = 0;
        }
        store_clan_ally(world, cid);
        refresh_ally_on_members(world, cid);
    }
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_id = 0;
        c.ally_name.clear();
        c.ally_crest_id = 0; // `changeAllyCrest(0, false)`
        c.ally_penalty_expiry_time = now_millis() + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_DISSOLVE_ALLY;
    }
    store_clan_ally(world, clan_id);
    refresh_ally_on_members(world, clan_id);
}

/// Java `Clan.checkAllyJoinCondition` — the invite guard chain (each reject's
/// message goes to the inviting alliance leader).
fn check_ally_join_condition(world: &World, requestor_oid: i32, target_oid: i32) -> bool {
    let Some(rp) = world.objects.get_component::<Player>(&requestor_oid) else {
        return false;
    };
    let leader_clan_id = rp.clan_id;
    let Some(leader_clan) = world.clans.get(&leader_clan_id) else {
        return false;
    };
    if leader_clan.ally_id == 0 || !rp.clan_leader || leader_clan_id != leader_clan.ally_id {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return false;
    }
    let now = now_millis();
    if leader_clan.ally_penalty_expiry_time > now
        && leader_clan.ally_penalty_type == ALLY_PENALTY_TYPE_DISMISS_CLAN
    {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::MAY_NOT_ACCEPT_ANY_CLAN_WITHIN_A_DAY_AFTER_EXPELLING,
            &[],
        );
        return false;
    }
    let Some(tp) = world.objects.get_component::<Player>(&target_oid) else {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return false;
    };
    if requestor_oid == target_oid {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN,
            &[],
        );
        return false;
    }
    if tp.clan_id == 0 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER,
            &[],
        );
        return false;
    }
    if !tp.clan_leader {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_IS_NOT_A_CLAN_LEADER,
            &[SmParam::Text(tp.name.clone())],
        );
        return false;
    }
    let Some(target_clan) = world.clans.get(&tp.clan_id) else {
        return false;
    };
    if target_clan.ally_id != 0 {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::S1_CLAN_IS_ALREADY_A_MEMBER_OF_S2_ALLIANCE,
            &[
                SmParam::Text(target_clan.name.clone()),
                SmParam::Text(target_clan.ally_name.clone()),
            ],
        );
        return false;
    }
    if target_clan.ally_penalty_expiry_time > now {
        if target_clan.ally_penalty_type == ALLY_PENALTY_TYPE_CLAN_LEAVED {
            send_sm_with(
                world,
                requestor_oid,
                sm_ids::S1_CLAN_CANNOT_JOIN_ALLIANCE_ONE_DAY_NOT_PASSED,
                &[
                    SmParam::Text(target_clan.name.clone()),
                    SmParam::Text(target_clan.ally_name.clone()),
                ],
            );
            return false;
        }
        if target_clan.ally_penalty_type == ALLY_PENALTY_TYPE_CLAN_DISMISSED {
            send_sm_with(
                world,
                requestor_oid,
                sm_ids::WITHDRAWN_OR_EXPELLED_CLAN_CANNOT_ENTER_ALLIANCE_FOR_A_DAY,
                &[],
            );
            return false;
        }
    }
    // Both standing in a siege zone.
    let both_in_siege = [requestor_oid, target_oid].iter().all(|&oid| {
        world
            .objects
            .get_component::<crate::model::components::Position>(&oid)
            .is_some_and(|pos| {
                world
                    .data
                    .zone_data
                    .siege_castle_at(pos.x, pos.y, pos.z)
                    .is_some()
            })
    });
    if both_in_siege {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::THE_OPPOSING_CLAN_IS_PARTICIPATING_IN_A_SIEGE_BATTLE,
            &[],
        );
        return false;
    }
    if at_war_between(world, leader_clan_id, tp.clan_id) {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_MAY_NOT_ALLY_WITH_A_CLAN_YOU_ARE_AT_WAR_WITH,
            &[],
        );
        return false;
    }
    if ally_clan_ids(world, leader_clan.ally_id).len() >= MAX_CLANS_IN_ALLY {
        send_sm_with(
            world,
            requestor_oid,
            sm_ids::YOU_HAVE_EXCEEDED_THE_LIMIT,
            &[],
        );
        return false;
    }
    true
}

/// `RequestJoinAlly` (0x8C): the alliance leader invites another clan's leader.
pub(crate) fn handle_request_join_ally(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(target_oid) = PacketReader::new(body).read_i32() else {
        return;
    };
    if client_for_player(world, target_oid).is_none() {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET,
            &[],
        );
        return;
    }
    let clan_id = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    if !check_ally_join_condition(world, player, target_oid) {
        return;
    }
    if world
        .objects
        .has_component::<crate::model::components::PendingRequest>(&player)
        || world
            .objects
            .has_component::<crate::model::components::PendingRequest>(&target_oid)
    {
        send_sm_with(
            world,
            player,
            sm_ids::C1_IS_ON_ANOTHER_TASK_PLEASE_TRY_AGAIN_LATER,
            &[SmParam::Text(player_name(world, target_oid))],
        );
        return;
    }
    let ally_id = world.clans.get(&clan_id).map(|c| c.ally_id).unwrap_or(0);
    let ally_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.ally_name.clone())
        .unwrap_or_default();
    super::party::install_request(
        world,
        player,
        target_oid,
        crate::model::components::RequestKind::AllyInvite { ally_id },
        super::party::REQUEST_TIMEOUT_TICKS,
    );
    if let Some(cs) = client_for_player(world, target_oid).and_then(|cid| world.clients.get(&cid)) {
        cs.send(server_packets::system_message_with(
            sm_ids::S1_LEADER_S2_HAS_REQUESTED_AN_ALLIANCE,
            &[
                SmParam::Text(ally_name),
                SmParam::Text(player_name(world, player)),
            ],
        ));
        cs.send(server_packets::ask_join_ally(
            player,
            &player_name(world, player),
        ));
    }
}

/// `RequestAnswerJoinAlly` (0x8D): the invited clan leader answered.
pub(crate) fn handle_request_answer_join_ally(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let answer = PacketReader::new(body).read_i32().unwrap_or(0);
    let Some(req) = world
        .objects
        .get_component::<crate::model::components::PendingRequest>(&player)
        .copied()
    else {
        return;
    };
    let crate::model::components::RequestKind::AllyInvite { ally_id } = req.kind else {
        return;
    };
    if !req.answerer {
        return;
    }
    super::party::clear_linked_request(world, player);
    let requestor = req.other;

    if answer == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::NO_RESPONSE_YOUR_ENTRANCE_TO_THE_ALLIANCE_HAS_BEEN_CANCELLED,
            &[],
        );
        send_sm_with(
            world,
            requestor,
            sm_ids::NO_RESPONSE_INVITATION_TO_JOIN_AN_ALLIANCE_HAS_BEEN_CANCELLED,
            &[],
        );
        return;
    }
    // Re-check (the requestor must still lead the same alliance).
    if world
        .objects
        .get_component::<Player>(&requestor)
        .map(|p| p.clan_id)
        != Some(ally_id)
    {
        return;
    }
    if !check_ally_join_condition(world, requestor, player) {
        return;
    }
    let ally_name = world
        .clans
        .get(&ally_id)
        .map(|c| c.ally_name.clone())
        .unwrap_or_default();
    let target_clan_id = world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0);
    let leader_crest = world
        .clans
        .get(&ally_id)
        .map(|c| c.ally_crest_id)
        .unwrap_or(0);
    if let Some(c) = world.clans.get_mut(&target_clan_id) {
        c.ally_id = ally_id;
        c.ally_name = ally_name;
        c.ally_penalty_expiry_time = 0;
        c.ally_penalty_type = 0;
        c.ally_crest_id = leader_crest; // `changeAllyCrest(leaderCrest, true)`
    }
    store_clan_ally(world, target_clan_id);
    refresh_ally_on_members(world, target_clan_id);
    // Java sends the (wrong) friend-added message to the requestor — kept.
    send_sm_with(
        world,
        requestor,
        sm_ids::SUCCESSFULLY_ADDED_TO_YOUR_FRIEND_LIST,
        &[],
    );
    send_sm_with(world, player, sm_ids::YOU_HAVE_ACCEPTED_THE_ALLIANCE, &[]);
}

/// `AllyLeave` (0x8E): a member clan's leader withdraws their clan.
pub(crate) fn handle_ally_leave(world: &mut World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    if !p.clan_leader {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_MAY_APPLY_FOR_WITHDRAWAL_FROM_THE_ALLIANCE,
            &[],
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.ally_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS,
            &[],
        );
        return;
    }
    if clan.id == clan.ally_id {
        send_sm_with(world, player, sm_ids::ALLIANCE_LEADERS_CANNOT_WITHDRAW, &[]);
        return;
    }
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_id = 0;
        c.ally_name.clear();
        c.ally_crest_id = 0; // `changeAllyCrest(0, true)`
        c.ally_penalty_expiry_time = now_millis() + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_CLAN_LEAVED;
    }
    store_clan_ally(world, clan_id);
    refresh_ally_on_members(world, clan_id);
    send_sm_with(
        world,
        player,
        sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_ALLIANCE,
        &[],
    );
}

/// `AllyDismiss` (0x8F): the alliance leader expels a member clan by name.
pub(crate) fn handle_ally_dismiss(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(name) = PacketReader::new(body).read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION,
            &[],
        );
        return;
    }
    let Some(leader_clan) = world.clans.get(&clan_id) else {
        return;
    };
    let ally_id = leader_clan.ally_id;
    if ally_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_CURRENTLY_ALLIED_WITH_ANY_CLANS,
            &[],
        );
        return;
    }
    if !is_leader || clan_id != ally_id {
        send_sm_with(
            world,
            player,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return;
    }
    let Some(target) = world
        .clans
        .values()
        .find(|c| c.name.eq_ignore_ascii_case(&name))
    else {
        send_sm_with(world, player, sm_ids::THAT_CLAN_DOES_NOT_EXIST, &[]);
        return;
    };
    let target_id = target.id;
    if target_id == clan_id {
        send_sm_with(world, player, sm_ids::ALLIANCE_LEADERS_CANNOT_WITHDRAW, &[]);
        return;
    }
    if target.ally_id != ally_id {
        send_sm_with(world, player, sm_ids::DIFFERENT_ALLIANCE, &[]);
        return;
    }

    let now = now_millis();
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.ally_penalty_expiry_time = now + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_DISMISS_CLAN;
    }
    store_clan_ally(world, clan_id);
    if let Some(c) = world.clans.get_mut(&target_id) {
        c.ally_id = 0;
        c.ally_name.clear();
        c.ally_crest_id = 0; // `changeAllyCrest(0, true)`
        c.ally_penalty_expiry_time = now + ALLY_PENALTY_MS;
        c.ally_penalty_type = ALLY_PENALTY_TYPE_CLAN_DISMISSED;
    }
    store_clan_ally(world, target_id);
    refresh_ally_on_members(world, target_id);
    send_sm_with(
        world,
        player,
        sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN,
        &[],
    );
}

/// `RequestDismissAlly` (0x90): the alliance leader dissolves the whole ally.
pub(crate) fn handle_request_dismiss_ally(world: &mut World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let is_leader = world
        .objects
        .get_component::<Player>(&player)
        .is_some_and(|p| p.clan_leader);
    if !is_leader {
        send_sm_with(
            world,
            player,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return;
    }
    handle_dissolve_ally(world, client_id, player);
}

/// `RequestAllyInfo` (0x2E): the ally window (`AllianceInfo`) + the SM cascade.
pub(crate) fn handle_request_ally_info(world: &World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let ally_id = world.clans.get(&p.clan_id).map(|c| c.ally_id).unwrap_or(0);
    if ally_id == 0 {
        send_sm_with(world, player, sm_ids::YOU_ARE_NOT_IN_AN_ALLIANCE, &[]);
        return;
    }
    let clans = ally_clan_ids(world, ally_id);
    let rows: Vec<(String, i32, String, i32, i32)> = clans
        .iter()
        .filter_map(|&cid| {
            let c = world.clans.get(&cid)?;
            let online = online_members(world, cid).len() as i32;
            Some((
                c.name.clone(),
                c.level,
                c.leader_name().to_string(),
                c.members.len() as i32,
                online,
            ))
        })
        .collect();
    let total: i32 = rows.iter().map(|r| r.3).sum();
    let online: i32 = rows.iter().map(|r| r.4).sum();
    let (ally_name, leader_clan_name, leader_player_name) = world
        .clans
        .get(&ally_id)
        .map(|c| {
            (
                c.ally_name.clone(),
                c.name.clone(),
                c.leader_name().to_string(),
            )
        })
        .unwrap_or_default();
    let Some(cs) = world.clients.get(&client_id) else {
        return;
    };
    cs.send(server_packets::alliance_info(
        &ally_name,
        total,
        online,
        &leader_clan_name,
        &leader_player_name,
        &rows,
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::ALLIANCE_INFORMATION,
        &[],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::ALLIANCE_NAME_S1,
        &[SmParam::Text(ally_name)],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::ALLIANCE_LEADER_S2_OF_S1,
        &[
            SmParam::Text(leader_clan_name),
            SmParam::Text(leader_player_name),
        ],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::CONNECTION_S1_TOTAL_S2,
        &[SmParam::Int(online), SmParam::Int(total)],
    ));
    cs.send(server_packets::system_message_with(
        sm_ids::AFFILIATED_CLANS_TOTAL_S1_CLAN_S,
        &[SmParam::Int(rows.len() as i32)],
    ));
    for (name, level, leader, c_total, c_online) in &rows {
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_INFORMATION,
            &[],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_NAME_S1,
            &[SmParam::Text(name.clone())],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_LEADER_S1,
            &[SmParam::Text(leader.clone())],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CLAN_LEVEL_S1,
            &[SmParam::Int(*level)],
        ));
        cs.send(server_packets::system_message_with(
            sm_ids::CONNECTION_S1_TOTAL_S2,
            &[SmParam::Int(*c_online), SmParam::Int(*c_total)],
        ));
        cs.send(server_packets::system_message_with(sm_ids::EMPTY_4, &[]));
    }
}

// --- G18 slice 6: sub-pledges & academy ------------------------------------

use crate::model::clan::{SubPledge, SUBUNIT_ACADEMY, SUBUNIT_KNIGHT1, SUBUNIT_ROYAL1};

/// `CreateRoyalGuardCost = 5000` (Feature.ini) — the reputation price of a
/// royal-guard unit.
const ROYAL_GUARD_COST: i32 = 5000;
/// `CreateKnightUnitCost = 10000` — the reputation price of a knight unit.
const KNIGHT_UNIT_COST: i32 = 10_000;

/// `VillageMaster.isValidName`/name-length checks shared by clan/sub-pledge
/// names: alphanumeric, 2..=16 chars (this dist's `ClanNameTemplate = .*`, so
/// the retail regex itself is not ported — same simplification `create_clan`
/// makes).
fn valid_pledge_name(name: &str) -> bool {
    name.chars().all(|c| c.is_ascii_alphanumeric()) && (2..=16).contains(&name.len())
}

/// `VillageMaster.createSubPledge`: the shared academy/royal-guard/knight
/// creation flow. `requested_type` is the *family* id (`SUBUNIT_ACADEMY`,
/// `SUBUNIT_ROYAL1`, or `SUBUNIT_KNIGHT1`) — `Clan.getAvailablePledgeTypes`
/// resolves it to the next open slot in that family.
fn create_sub_pledge(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    requested_type: i32,
    min_clan_lvl: i32,
    name: &str,
    leader_name: Option<&str>,
) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.level < min_clan_lvl {
        let sm = if requested_type == SUBUNIT_ACADEMY {
            sm_ids::TO_ESTABLISH_A_CLAN_ACADEMY_YOUR_CLAN_MUST_BE_LEVEL_5_OR_HIGHER
        } else {
            sm_ids::THE_CONDITIONS_NECESSARY_TO_CREATE_A_MILITARY_UNIT_HAVE_NOT_BEEN_MET
        };
        send_sm(world, client_id, sm);
        return;
    }
    if !valid_pledge_name(name) {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    if name.len() > 16 {
        send_sm(world, client_id, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return;
    }
    // Java scans every clan's sub-pledges for a name clash; the port's
    // `ClanTable` equivalent is `World.clans`.
    let name_taken = world.clans.values().any(|c| {
        c.sub_pledges
            .values()
            .any(|sp| sp.name.eq_ignore_ascii_case(name))
    });
    if name_taken {
        if requested_type == SUBUNIT_ACADEMY {
            send_sm_with(
                world,
                player_oid,
                sm_ids::S1_ALREADY_EXISTS,
                &[SmParam::Text(name.to_string())],
            );
        } else {
            send_sm(
                world,
                client_id,
                sm_ids::ANOTHER_MILITARY_UNIT_ALREADY_USES_THAT_NAME,
            );
        }
        return;
    }

    // The leader-designate (royal/knight only): must be a main-pledge member
    // who doesn't already captain a sub-unit.
    let leader_id = if requested_type != SUBUNIT_ACADEMY {
        let Some(leader_name) = leader_name else {
            return;
        };
        let eligible = clan
            .members
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(leader_name))
            .filter(|m| m.pledge_type == 0)
            .filter(|m| clan.leader_sub_pledge_of(m.char_id) == 0);
        let Some(member) = eligible else {
            let sm = if requested_type >= SUBUNIT_KNIGHT1 {
                sm_ids::THE_CAPTAIN_OF_THE_ORDER_OF_KNIGHTS_CANNOT_BE_APPOINTED
            } else {
                sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED
            };
            send_sm(world, client_id, sm);
            return;
        };
        member.char_id
    } else {
        0
    };
    // `Clan.createSubPledge`'s own reject: the clan leader can't also
    // captain a sub-unit ("Leader is not correct" — a plain message, no SM).
    if leader_id != 0 && leader_id == clan.leader_id {
        send_sm_with(
            world,
            player_oid,
            sm_ids::S1_TEXT,
            &[SmParam::Text("Leader is not correct".to_string())],
        );
        return;
    }

    // `Clan.createSubPledge`'s own guard chain: the resolved slot in the
    // requested family, then (royal/knight only) the reputation price.
    let pledge_type = clan.available_pledge_type(requested_type);
    if pledge_type == 0 {
        if requested_type == SUBUNIT_ACADEMY {
            send_sm(
                world,
                client_id,
                sm_ids::YOUR_CLAN_HAS_ALREADY_ESTABLISHED_A_CLAN_ACADEMY,
            );
        } else {
            send_sm_with(
                world,
                player_oid,
                sm_ids::S1_TEXT,
                &[SmParam::Text(
                    "You can't create any more sub-units of this type".to_string(),
                )],
            );
        }
        return;
    }
    let cost = if requested_type == SUBUNIT_ACADEMY {
        0
    } else if pledge_type < SUBUNIT_KNIGHT1 {
        ROYAL_GUARD_COST
    } else {
        KNIGHT_UNIT_COST
    };
    if cost > 0 && clan.reputation_score < cost {
        send_sm(world, client_id, sm_ids::THE_CLAN_REPUTATION_IS_TOO_LOW);
        return;
    }

    let sub_pledge = SubPledge {
        id: pledge_type,
        name: name.to_string(),
        leader_id,
    };
    let clan = world.clans.get_mut(&clan_id).expect("checked above");
    clan.sub_pledges.insert(pledge_type, sub_pledge);
    if cost > 0 {
        clan.reputation_score -= cost;
    }
    let reputation = clan.reputation_score;
    let _ = world.db.send(DbCommand::InsertSubPledge {
        clan_id,
        pledge_type,
        name: name.to_string(),
        leader_id,
    });
    if cost > 0 {
        let _ = world.db.send(DbCommand::UpdateClanReputation {
            clan_id,
            reputation,
        });
    }
    let info =
        server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    let leader_display_name = if leader_id != 0 {
        player_name(world, leader_id)
    } else {
        String::new()
    };
    let created =
        server_packets::pledge_receive_sub_pledge_created(pledge_type, name, &leader_display_name);
    for oid in online_members(world, clan_id) {
        send_to_member(world, oid, info.clone());
        send_to_member(world, oid, created.clone());
    }

    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let sm = if requested_type == SUBUNIT_ACADEMY {
        server_packets::system_message_with(
            sm_ids::CONGRATULATIONS_THE_S1_S_CLAN_ACADEMY_HAS_BEEN_CREATED,
            &[SmParam::Text(clan_name)],
        )
    } else if pledge_type >= SUBUNIT_KNIGHT1 {
        server_packets::system_message_with(
            sm_ids::THE_KNIGHTS_OF_S1_HAVE_BEEN_CREATED,
            &[SmParam::Text(clan_name)],
        )
    } else {
        server_packets::system_message_with(
            sm_ids::THE_ROYAL_GUARD_OF_S1_HAVE_BEEN_CREATED,
            &[SmParam::Text(clan_name)],
        )
    };
    send_to_member(world, player_oid, sm);

    if leader_id != 0 {
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map(|c| c.pledge_class_of(leader_id))
            .unwrap_or(0);
        if let Some(lp) = world.objects.get_component_mut::<Player>(&leader_id) {
            lp.pledge_class = pledge_class;
        }
        super::party::broadcast_user_info(world, leader_id);
    }
}

/// `VillageMaster`'s `create_academy <name>` bypass.
pub(crate) fn handle_create_academy(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    args: &str,
) {
    let mut it = args.split_whitespace();
    let Some(name) = it.next() else { return };
    create_sub_pledge(world, client_id, player_oid, SUBUNIT_ACADEMY, 5, name, None);
}

/// `VillageMaster`'s `create_royal <name> <leaderName>` bypass.
pub(crate) fn handle_create_royal(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut it = args.split_whitespace();
    let Some(name) = it.next() else { return };
    let leader = it.next();
    create_sub_pledge(
        world,
        client_id,
        player_oid,
        SUBUNIT_ROYAL1,
        6,
        name,
        leader,
    );
}

/// `VillageMaster`'s `create_knight <name> <leaderName>` bypass.
pub(crate) fn handle_create_knight(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut it = args.split_whitespace();
    let Some(name) = it.next() else { return };
    let leader = it.next();
    create_sub_pledge(
        world,
        client_id,
        player_oid,
        SUBUNIT_KNIGHT1,
        7,
        name,
        leader,
    );
}

/// `VillageMaster.renameSubPledge` (`rename_pledge <pledgeTypeId> <newName>`).
pub(crate) fn handle_rename_pledge(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut it = args.split_whitespace();
    let Some(Ok(pledge_type)) = it.next().map(str::parse::<i32>) else {
        return;
    };
    let Some(new_name) = it.next() else { return };
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    if !world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.sub_pledges.contains_key(&pledge_type))
    {
        return; // "Pledge don't exists." (Java's own plain-text message)
    }
    if !valid_pledge_name(new_name) {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    if new_name.len() > 16 {
        send_sm(world, client_id, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return;
    }
    let leader_id = {
        let Some(c) = world.clans.get_mut(&clan_id) else {
            return;
        };
        let Some(sp) = c.sub_pledges.get_mut(&pledge_type) else {
            return;
        };
        sp.name = new_name.to_string();
        sp.leader_id
    };
    let _ = world.db.send(DbCommand::UpdateSubPledge {
        clan_id,
        pledge_type,
        name: new_name.to_string(),
        leader_id,
    });
    broadcast_clan_status(world, clan_id);
}

/// `VillageMaster.assignSubPledgeLeader` (`assign_subpl_leader <unitName>
/// <memberName>`).
pub(crate) fn handle_assign_subpledge_leader(
    world: &mut World,
    client_id: u32,
    player_oid: i32,
    args: &str,
) {
    let mut it = args.split_whitespace();
    let Some(unit_name) = it.next() else { return };
    let Some(member_name) = it.next() else { return };
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
        return;
    };
    let clan_id = p.clan_id;
    let player_display_name = p.name.clone();
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    if member_name.len() > 16 {
        send_sm(
            world,
            client_id,
            sm_ids::YOUR_TITLE_CANNOT_EXCEED_16_CHARACTERS,
        );
        return;
    }
    if player_display_name.eq_ignore_ascii_case(member_name) {
        send_sm(
            world,
            client_id,
            sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED,
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let Some(sub_pledge) = clan
        .sub_pledges
        .values()
        .find(|sp| sp.name.eq_ignore_ascii_case(unit_name))
    else {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    };
    if sub_pledge.id == SUBUNIT_ACADEMY {
        send_sm(world, client_id, sm_ids::CLAN_NAME_IS_INVALID);
        return;
    }
    let sub_pledge_id = sub_pledge.id;
    let eligible = clan
        .members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(member_name))
        .filter(|m| m.pledge_type == 0)
        .filter(|m| clan.leader_sub_pledge_of(m.char_id) == 0);
    let Some(member) = eligible.cloned() else {
        let sm = if sub_pledge_id >= SUBUNIT_KNIGHT1 {
            sm_ids::THE_CAPTAIN_OF_THE_ORDER_OF_KNIGHTS_CANNOT_BE_APPOINTED
        } else {
            sm_ids::THE_ROYAL_GUARD_CAPTAIN_CANNOT_BE_APPOINTED
        };
        send_sm(world, client_id, sm);
        return;
    };

    if let Some(c) = world.clans.get_mut(&clan_id) {
        if let Some(sp) = c.sub_pledges.get_mut(&sub_pledge_id) {
            sp.leader_id = member.char_id;
        }
    }
    let _ = world.db.send(DbCommand::UpdateSubPledge {
        clan_id,
        pledge_type: sub_pledge_id,
        name: unit_name.to_string(),
        leader_id: member.char_id,
    });

    let pledge_class = world
        .clans
        .get(&clan_id)
        .map(|c| c.pledge_class_of(member.char_id))
        .unwrap_or(0);
    if let Some(lp) = world.objects.get_component_mut::<Player>(&member.char_id) {
        lp.pledge_class = pledge_class;
    }
    super::party::broadcast_user_info(world, member.char_id);
    broadcast_clan_status(world, clan_id);
    let sm = server_packets::system_message_with(
        sm_ids::C1_HAS_BEEN_SELECTED_AS_THE_CAPTAIN_OF_S2,
        &[
            SmParam::Text(member.name.clone()),
            SmParam::Text(unit_name.to_string()),
        ],
    );
    broadcast_to_clan(world, clan_id, &sm);
}

// --- G18 slice 7: crests ----------------------------------------------------

use crate::model::clan::{
    Crest, CL_REGISTER_CREST, CREST_TYPE_ALLY, CREST_TYPE_PLEDGE, CREST_TYPE_PLEDGE_LARGE,
};

/// `CrestTable.createCrest`: allocate the next id, store the bitmap, persist.
fn create_crest(world: &mut World, data: &[u8], kind: i32) -> i32 {
    let id = world.next_crest_id;
    world.next_crest_id += 1;
    world.crests.insert(
        id,
        Crest {
            id,
            data: data.to_vec(),
            kind,
        },
    );
    let _ = world.db.send(DbCommand::InsertCrest {
        id,
        data: data.to_vec(),
        kind,
    });
    id
}

/// `CrestTable.removeCrest`: drop the bitmap, but never delete (or let a
/// caller reuse) the most recently allocated id — Java's guard against a
/// stale client cache showing the wrong image for a brand-new crest.
fn remove_crest(world: &mut World, crest_id: i32) {
    world.crests.remove(&crest_id);
    if crest_id == world.next_crest_id - 1 {
        return;
    }
    let _ = world.db.send(DbCommand::DeleteCrest { id: crest_id });
}

/// Sync every online member's denormalized `Player.clan_crest_id` with the
/// clan and re-broadcast their UserInfo/CharInfo — the small-crest half of
/// `Clan.changeClanCrest`'s `for (member : getOnlineMembers()) broadcastUserInfo()`.
fn refresh_clan_crest_on_members(world: &mut World, clan_id: i32) {
    let crest_id = world.clans.get(&clan_id).map(|c| c.crest_id).unwrap_or(0);
    for oid in online_members(world, clan_id) {
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_crest_id = crest_id;
        }
        super::party::broadcast_user_info(world, oid);
    }
}

/// `RequestSetPledgeCrest` (0x09): the small (≤256-byte) clan crest.
pub(crate) fn handle_request_set_pledge_crest(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(length) = r.read_i32() else { return };
    if length > 256 {
        return; // Java's own readImpl bails before the length even reaches runImpl
    }
    let data = if length > 0 {
        r.read_bytes(length as usize).map(|d| d.to_vec())
    } else {
        Some(Vec::new())
    };
    let Some(data) = data else { return };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.dissolving_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_CANNOT_REGISTER_OR_DELETE_CREST,
            &[],
        );
        return;
    }
    if !clan.has_privilege(player, privs, CL_REGISTER_CREST) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if data.is_empty() {
        if clan.crest_id != 0 {
            let old = clan.crest_id;
            remove_crest(world, old);
            if let Some(c) = world.clans.get_mut(&clan_id) {
                c.crest_id = 0;
            }
            let _ = world.db.send(DbCommand::UpdateClanCrest {
                clan_id,
                crest_id: 0,
            });
            refresh_clan_crest_on_members(world, clan_id);
            send_sm_with(world, player, sm_ids::THE_CLAN_MARK_HAS_BEEN_DELETED, &[]);
        }
        return;
    }
    if clan.level < 3 {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE,
            &[],
        );
        return;
    }
    let crest_id = create_crest(world, &data, CREST_TYPE_PLEDGE);
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.crest_id = crest_id;
    }
    let _ = world
        .db
        .send(DbCommand::UpdateClanCrest { clan_id, crest_id });
    refresh_clan_crest_on_members(world, clan_id);
    send_sm_with(
        world,
        player,
        sm_ids::THE_CREST_WAS_SUCCESSFULLY_REGISTERED,
        &[],
    );
}

/// `RequestPledgeCrest` (0x67): answer with the small crest's bitmap.
pub(crate) fn handle_request_pledge_crest(world: &World, client_id: u32, body: &[u8]) {
    let mut r = PacketReader::new(body);
    let Some(crest_id) = r.read_i32() else { return };
    let data = world.crests.get(&crest_id).map(|c| c.data.as_slice());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_crest(crest_id, data));
    }
}

/// `RequestExSetPledgeCrestLarge` (ex 0x11): the large (≤2176-byte) crest,
/// shown on clan-hall/castle items.
pub(crate) fn handle_request_ex_set_pledge_crest_large(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(length) = r.read_i32() else { return };
    if length > 2176 {
        return;
    }
    let data = if length > 0 {
        r.read_bytes(length as usize).map(|d| d.to_vec())
    } else {
        Some(Vec::new())
    };
    let Some(data) = data else { return };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    if length < 0 || length > 2176 {
        send_sm_with(
            world,
            player,
            sm_ids::THE_SIZE_OF_THE_UPLOADED_SYMBOL_DOES_NOT_MEET_STANDARDS,
            &[],
        );
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.dissolving_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_CANNOT_REGISTER_OR_DELETE_CREST,
            &[],
        );
        return;
    }
    if !clan.has_privilege(player, privs, CL_REGISTER_CREST) {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT,
            &[],
        );
        return;
    }
    if data.is_empty() {
        if clan.crest_large_id != 0 {
            let old = clan.crest_large_id;
            remove_crest(world, old);
            if let Some(c) = world.clans.get_mut(&clan_id) {
                c.crest_large_id = 0;
            }
            let _ = world.db.send(DbCommand::UpdateClanCrestLarge {
                clan_id,
                crest_large_id: 0,
            });
            // Java broadcasts UserInfo to every online member here too, even
            // though the large crest id isn't part of that packet (only
            // fetched on demand via `RequestExPledgeCrestLarge`) — kept
            // faithful; it's a no-op refresh for everyone but the actor.
            for oid in online_members(world, clan_id) {
                super::party::broadcast_user_info(world, oid);
            }
            send_sm_with(world, player, sm_ids::THE_CLAN_MARK_HAS_BEEN_DELETED, &[]);
        }
        return;
    }
    if clan.level < 3 {
        send_sm_with(
            world,
            player,
            sm_ids::A_CLAN_CREST_CAN_ONLY_BE_REGISTERED_WHEN_THE_CLAN_S_SKILL_LEVEL_IS_3_OR_ABOVE,
            &[],
        );
        return;
    }
    let crest_id = create_crest(world, &data, CREST_TYPE_PLEDGE_LARGE);
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.crest_large_id = crest_id;
    }
    let _ = world.db.send(DbCommand::UpdateClanCrestLarge {
        clan_id,
        crest_large_id: crest_id,
    });
    for oid in online_members(world, clan_id) {
        super::party::broadcast_user_info(world, oid);
    }
    send_sm_with(
        world,
        player,
        sm_ids::THE_CLAN_MARK_WAS_SUCCESSFULLY_REGISTERED_ON_ITEMS,
        &[],
    );
}

/// `RequestExPledgeCrestLarge` (ex 0x10): answer with the large crest's
/// bitmap, chunked into ≤14336-byte `ExPledgeEmblem` packets (always a single
/// chunk on this dist's 2176-byte cap, but the loop stays general).
pub(crate) fn handle_request_ex_pledge_crest_large(world: &World, client_id: u32, ex_body: &[u8]) {
    let mut r = PacketReader::new(ex_body);
    let Some(crest_id) = r.read_i32() else { return };
    let Some(clan_id) = r.read_i32() else { return };
    let Some(data) = world.crests.get(&crest_id).map(|c| c.data.clone()) else {
        return;
    };
    let Some(cs) = world.clients.get(&client_id) else {
        return;
    };
    const CHUNK: usize = 14_336;
    for i in 0..5 {
        let start = CHUNK * i;
        if start >= data.len() {
            continue;
        }
        let end = (start + CHUNK).min(data.len());
        cs.send(server_packets::ex_pledge_emblem(
            clan_id,
            crest_id,
            i as i32,
            &data[start..end],
        ));
    }
}

/// `RequestSetAllyCrest` (0x91): the alliance crest (≤192 bytes) — only the
/// alliance leader (the leader-clan's own clan leader) may set it.
pub(crate) fn handle_request_set_ally_crest(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(length) = r.read_i32() else { return };
    if length > 192 {
        return;
    }
    let data = if length > 0 {
        r.read_bytes(length as usize).map(|d| d.to_vec())
    } else {
        Some(Vec::new())
    };
    let Some(data) = data else { return };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let ally_id = p.ally_id;
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    if length < 0 {
        send_sm_with(
            world,
            player,
            sm_ids::S1_TEXT,
            &[SmParam::Text("File transfer error.".to_string())],
        );
        return;
    }
    if length > 192 {
        send_sm_with(
            world,
            player,
            sm_ids::PLEASE_ADJUST_THE_IMAGE_SIZE_TO_8X12,
            &[],
        );
        return;
    }
    if ally_id == 0 || clan_id != ally_id || !is_leader {
        send_sm_with(
            world,
            player,
            sm_ids::THIS_FEATURE_IS_ONLY_AVAILABLE_TO_ALLIANCE_LEADERS,
            &[],
        );
        return;
    }
    if data.is_empty() {
        let old = world
            .clans
            .get(&clan_id)
            .map(|c| c.ally_crest_id)
            .unwrap_or(0);
        if old != 0 {
            remove_crest(world, old);
            set_alliance_crest(world, ally_id, 0);
        }
        return;
    }
    let crest_id = create_crest(world, &data, CREST_TYPE_ALLY);
    set_alliance_crest(world, ally_id, crest_id);
    send_sm_with(
        world,
        player,
        sm_ids::THE_CREST_WAS_SUCCESSFULLY_REGISTERED,
        &[],
    );
}

/// `Clan.changeAllyCrest(id, onlyThisClan=false)`: push the crest id to every
/// clan in the alliance and refresh their online members.
fn set_alliance_crest(world: &mut World, ally_id: i32, crest_id: i32) {
    let clan_ids = ally_clan_ids(world, ally_id);
    for cid in &clan_ids {
        if let Some(c) = world.clans.get_mut(cid) {
            c.ally_crest_id = crest_id;
        }
    }
    let _ = world.db.send(DbCommand::UpdateAllyCrestForAlliance {
        ally_id,
        ally_crest_id: crest_id,
    });
    for cid in clan_ids {
        for oid in online_members(world, cid) {
            if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
                p.ally_crest_id = crest_id;
            }
            super::party::broadcast_user_info(world, oid);
        }
    }
}

/// `RequestAllyCrest` (0x92): answer with the alliance crest's bitmap.
pub(crate) fn handle_request_ally_crest(world: &World, client_id: u32, body: &[u8]) {
    let mut r = PacketReader::new(body);
    let Some(crest_id) = r.read_i32() else { return };
    let data = world.crests.get(&crest_id).map(|c| c.data.as_slice());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ally_crest(crest_id, data));
    }
}

// --- G18 slice 8: recruitment registry (ClanEntryManager) ------------------

use crate::model::clan_entry::{
    PledgeApplicantInfo, PledgeRecruitInfo, PledgeWaitingInfo, LOCK_TIME_TICKS,
};

fn is_player_recruit_locked(world: &World, player_id: i32) -> bool {
    world
        .recruit_player_lock
        .get(&player_id)
        .is_some_and(|&t| t > world.tick)
}

fn is_clan_recruit_locked(world: &World, clan_id: i32) -> bool {
    world
        .recruit_clan_lock
        .get(&clan_id)
        .is_some_and(|&t| t > world.tick)
}

/// Java `getPlayerLockTime`/`getClanLockTime` — minutes remaining, for the
/// "try again in N minutes" message.
fn player_lock_minutes(world: &World, player_id: i32) -> i64 {
    world
        .recruit_player_lock
        .get(&player_id)
        .map(|&t| (t.saturating_sub(world.tick) / 600) as i64)
        .unwrap_or(0)
}
fn clan_lock_minutes(world: &World, clan_id: i32) -> i64 {
    world
        .recruit_clan_lock
        .get(&clan_id)
        .map(|&t| (t.saturating_sub(world.tick) / 600) as i64)
        .unwrap_or(0)
}

/// `ClanEntryManager.addPlayerApplicationToClan`.
fn add_player_application(world: &mut World, clan_id: i32, info: PledgeApplicantInfo) -> bool {
    if is_player_recruit_locked(world, info.player_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::UpsertPledgeApplicant {
        player_id: info.player_id,
        clan_id,
        karma: info.karma,
        message: info.message.clone(),
    });
    world
        .recruit_applicants
        .entry(clan_id)
        .or_default()
        .insert(info.player_id, info);
    true
}

/// `ClanEntryManager.removePlayerApplication` (no lock — cancelling an
/// application is always allowed, matching Java).
fn remove_player_application(world: &mut World, clan_id: i32, player_id: i32) -> bool {
    let _ = world
        .db
        .send(DbCommand::DeletePledgeApplicant { player_id, clan_id });
    world
        .recruit_applicants
        .get_mut(&clan_id)
        .is_some_and(|m| m.remove(&player_id).is_some())
}

/// `ClanEntryManager.getClanIdForPlayerApplication`.
fn clan_id_for_player_application(world: &World, player_id: i32) -> i32 {
    world
        .recruit_applicants
        .iter()
        .find(|(_, m)| m.contains_key(&player_id))
        .map(|(&clan_id, _)| clan_id)
        .unwrap_or(0)
}

/// `ClanEntryManager.addToWaitingList`.
fn add_to_waiting_list(world: &mut World, info: PledgeWaitingInfo) -> bool {
    if is_player_recruit_locked(world, info.player_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::InsertPledgeWaiting {
        player_id: info.player_id,
        karma: info.karma,
    });
    world.recruit_waiting.insert(info.player_id, info);
    true
}

/// `ClanEntryManager.removeFromWaitingList` — also arms the re-registration
/// lock, unlike removing an application.
fn remove_from_waiting_list(world: &mut World, player_id: i32) -> bool {
    if !world.recruit_waiting.contains_key(&player_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::DeletePledgeWaiting { player_id });
    world.recruit_waiting.remove(&player_id);
    world
        .recruit_player_lock
        .insert(player_id, world.tick + LOCK_TIME_TICKS);
    true
}

/// `ClanEntryManager.addToClanList`.
fn add_to_clan_list(world: &mut World, clan_id: i32, info: PledgeRecruitInfo) -> bool {
    if world.recruit_clans.contains_key(&clan_id) || is_clan_recruit_locked(world, clan_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::InsertPledgeRecruit {
        clan_id,
        karma: info.karma,
        information: info.information.clone(),
        detailed_information: info.detailed_information.clone(),
        application_type: info.application_type,
        recruit_type: info.recruit_type,
    });
    world.recruit_clans.insert(clan_id, info);
    true
}

/// `ClanEntryManager.updateClanList`.
fn update_clan_list(world: &mut World, clan_id: i32, info: PledgeRecruitInfo) -> bool {
    if !world.recruit_clans.contains_key(&clan_id) || is_clan_recruit_locked(world, clan_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::UpdatePledgeRecruit {
        clan_id,
        karma: info.karma,
        information: info.information.clone(),
        detailed_information: info.detailed_information.clone(),
        application_type: info.application_type,
        recruit_type: info.recruit_type,
    });
    world.recruit_clans.insert(clan_id, info);
    true
}

/// `ClanEntryManager.removeFromClanList` — also arms the re-registration lock.
fn remove_from_clan_list(world: &mut World, clan_id: i32) -> bool {
    if !world.recruit_clans.contains_key(&clan_id) {
        return false;
    }
    let _ = world.db.send(DbCommand::DeletePledgeRecruit { clan_id });
    world.recruit_clans.remove(&clan_id);
    world
        .recruit_clan_lock
        .insert(clan_id, world.tick + LOCK_TIME_TICKS);
    true
}

/// `RequestPledgeRecruitBoardAccess` (ex 0xD5): the leader (or a
/// CL_MANAGE_RANKS holder) registers/updates/removes the clan's recruiting
/// listing. `apply_type`: 0 remove, 1 add, 2 update.
pub(crate) fn handle_request_pledge_recruit_board_access(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(apply_type) = r.read_i32() else {
        return;
    };
    let Some(karma) = r.read_i32() else { return };
    let Some(information) = r.read_string() else {
        return;
    };
    let Some(detailed_information) = r.read_string() else {
        return;
    };
    let Some(application_type) = r.read_i32() else {
        return;
    };
    let Some(recruit_type) = r.read_i32() else {
        return;
    };

    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN,
            &[],
        );
        return;
    }
    if !world
        .clans
        .get(&clan_id)
        .is_some_and(|c| c.has_privilege(player, privs, CL_MANAGE_RANKS))
    {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN,
            &[],
        );
        return;
    }
    let info = PledgeRecruitInfo {
        clan_id,
        karma,
        information,
        detailed_information,
        application_type,
        recruit_type,
    };
    match apply_type {
        0 => {
            remove_from_clan_list(world, clan_id);
        }
        1 => {
            if add_to_clan_list(world, clan_id, info) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTRY_APPLICATION_COMPLETE_AUTO_CANCELLED_AFTER_30_DAYS,
                    &[],
                );
            } else {
                send_sm_with(
                    world,
                    player,
                    sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
                    &[SmParam::Long(clan_lock_minutes(world, clan_id))],
                );
            }
        }
        2 => {
            if update_clan_list(world, clan_id, info) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTRY_APPLICATION_COMPLETE_AUTO_CANCELLED_AFTER_30_DAYS,
                    &[],
                );
            } else {
                send_sm_with(
                    world,
                    player,
                    sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
                    &[SmParam::Long(clan_lock_minutes(world, clan_id))],
                );
            }
        }
        _ => {}
    }
}

/// `RequestPledgeRecruitBoardDetail` (ex 0xD6): the full detail pane for one
/// recruiting clan.
pub(crate) fn handle_request_pledge_recruit_board_detail(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    let Some(info) = world.recruit_clans.get(&clan_id) else {
        return;
    };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_board_detail(
            info.clan_id,
            info.karma,
            &info.information,
            &info.detailed_information,
            info.application_type,
            info.recruit_type,
        ));
    }
}

/// `RequestPledgeRecruitBoardSearch` (ex 0xD4): the recruit-board search,
/// with Java's real unsorted/sorted/by-name branches and 12-per-page paging.
pub(crate) fn handle_request_pledge_recruit_board_search(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let mut r = PacketReader::new(ex_body);
    let Some(clan_level) = r.read_i32() else {
        return;
    };
    let Some(karma) = r.read_i32() else { return };
    let Some(search_type) = r.read_i32() else {
        return;
    };
    let Some(query) = r.read_string() else { return };
    let Some(sort) = r.read_i32() else { return };
    let Some(descending_raw) = r.read_i32() else {
        return;
    };
    let Some(page) = r.read_i32() else { return };
    let Some(_application_type) = r.read_i32() else {
        return;
    }; // read, unused (Java: "Helios")
    let descending = descending_raw == 2;

    let mut matches: Vec<&PledgeRecruitInfo> = if query.is_empty() {
        if karma < 0 && clan_level < 0 {
            world.recruit_clans.values().collect()
        } else {
            world
                .recruit_clans
                .values()
                .filter(|info| {
                    let level = world.clans.get(&info.clan_id).map(|c| c.level).unwrap_or(0);
                    let level_ok = clan_level < 0 || clan_level == level;
                    let karma_ok = karma < 0 || karma == info.karma;
                    level_ok && karma_ok
                })
                .collect()
        }
    } else {
        let q = query.to_lowercase();
        world
            .recruit_clans
            .values()
            .filter(|info| {
                let Some(c) = world.clans.get(&info.clan_id) else {
                    return false;
                };
                if search_type == 1 {
                    c.name.to_lowercase().contains(&q)
                } else {
                    c.leader_name().to_lowercase().contains(&q)
                }
            })
            .collect()
    };
    if query.is_empty() && !(karma < 0 && clan_level < 0) {
        let sort_by = sort.clamp(1, 4);
        matches.sort_by(|a, b| {
            let ord = match sort_by {
                1 => world
                    .clans
                    .get(&a.clan_id)
                    .map(|c| c.name.clone())
                    .cmp(&world.clans.get(&b.clan_id).map(|c| c.name.clone())),
                2 => world
                    .clans
                    .get(&a.clan_id)
                    .map(|c| c.leader_name().to_string())
                    .cmp(
                        &world
                            .clans
                            .get(&b.clan_id)
                            .map(|c| c.leader_name().to_string()),
                    ),
                3 => world
                    .clans
                    .get(&a.clan_id)
                    .map(|c| c.level)
                    .cmp(&world.clans.get(&b.clan_id).map(|c| c.level)),
                _ => a.karma.cmp(&b.karma),
            };
            if descending {
                ord.reverse()
            } else {
                ord
            }
        });
    }

    const PER_PAGE: usize = 12;
    let total = matches.len();
    let start = ((page.max(1) as usize) - 1) * PER_PAGE;
    let page_entries: Vec<_> = matches
        .into_iter()
        .skip(start)
        .take(PER_PAGE)
        .filter_map(|info| {
            let c = world.clans.get(&info.clan_id)?;
            Some((
                c.id,
                c.ally_id,
                c.crest_id,
                c.ally_crest_id,
                c.name.clone(),
                c.leader_name().to_string(),
                c.level,
                c.members.len() as i32,
                info.karma,
                info.information.clone(),
                info.application_type,
                info.recruit_type,
            ))
        })
        .collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_board_search(
            page,
            total,
            &page_entries,
        ));
    }
}

/// `RequestPledgeWaitingApply` (ex 0xD7): a clanless player applies to a
/// specific clan.
pub(crate) fn handle_request_pledge_waiting_apply(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(karma) = r.read_i32() else { return };
    let Some(clan_id) = r.read_i32() else { return };
    let Some(message) = r.read_string() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id != 0 || !world.clans.contains_key(&clan_id) {
        return;
    }
    let info = PledgeApplicantInfo {
        player_id: player,
        name: p.name.clone(),
        level: p.level,
        karma,
        clan_id,
        message,
    };
    if add_player_application(world, clan_id, info) {
        if let Some(cs) = world.clients.get(&client_id) {
            cs.send(server_packets::ex_pledge_recruit_apply_info(4)); // ClanEntryStatus::WAITING
        }
        let leader_id = world.clans.get(&clan_id).map(|c| c.leader_id).unwrap_or(0);
        send_to_member(
            world,
            leader_id,
            server_packets::ex_pledge_waiting_list_alarm(),
        );
    } else {
        send_sm_with(
            world,
            player,
            sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
            &[SmParam::Long(player_lock_minutes(world, player))],
        );
    }
}

/// `RequestPledgeWaitingApplied` (ex 0xD8): a clanless player checks their
/// own pending application.
pub(crate) fn handle_request_pledge_waiting_applied(world: &World, client_id: u32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    if world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0)
        != 0
    {
        return;
    }
    let clan_id = clan_id_for_player_application(world, player);
    if clan_id == 0 {
        return;
    }
    let Some(app) = world
        .recruit_applicants
        .get(&clan_id)
        .and_then(|m| m.get(&player))
    else {
        return;
    };
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    let recruit = world.recruit_clans.get(&clan_id);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_waiting_list_applied(
            clan.id,
            &clan.name,
            clan.leader_name(),
            clan.level,
            clan.members.len() as i32,
            recruit.map(|r| r.karma).unwrap_or(0),
            recruit.map(|r| r.information.as_str()).unwrap_or(""),
            &app.message,
        ));
    }
}

/// `RequestPledgeWaitingList` (ex 0xD9): the clan's applicant queue.
pub(crate) fn handle_request_pledge_waiting_list(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    if world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0)
        != clan_id
    {
        return;
    }
    send_waiting_list(world, client_id, clan_id);
}

fn send_waiting_list(world: &World, client_id: u32, clan_id: i32) {
    let rows: Vec<_> = world
        .recruit_applicants
        .get(&clan_id)
        .map(|m| {
            m.values()
                .map(|a| (a.player_id, a.name.clone(), 0, a.level))
                .collect()
        })
        .unwrap_or_default();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_waiting_list(&rows));
    }
}

/// `RequestPledgeWaitingUser` (ex 0xDA): one applicant's detail, or the whole
/// queue when that player has no application (Java's own fallback).
pub(crate) fn handle_request_pledge_waiting_user(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(clan_id) = r.read_i32() else { return };
    let Some(player_id) = r.read_i32() else {
        return;
    };
    if world
        .objects
        .get_component::<Player>(&player)
        .map(|p| p.clan_id)
        .unwrap_or(0)
        != clan_id
    {
        return;
    }
    match world
        .recruit_applicants
        .get(&clan_id)
        .and_then(|m| m.get(&player_id))
    {
        Some(app) => {
            if let Some(cs) = world.clients.get(&client_id) {
                cs.send(server_packets::ex_pledge_waiting_user(
                    app.player_id,
                    &app.message,
                ));
            }
        }
        None => send_waiting_list(world, client_id, clan_id),
    }
}

/// `RequestPledgeWaitingUserAccept` (ex 0xDB): accept (join the applicant
/// through the shared `add_clan_member` path) or reject an application.
pub(crate) fn handle_request_pledge_waiting_user_accept(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(accept) = r.read_i32() else { return };
    let Some(player_id) = r.read_i32() else {
        return;
    };
    let Some(_clan_id_echo) = r.read_i32() else {
        return;
    };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        return;
    }
    if accept != 1 {
        remove_player_application(world, clan_id, player_id);
        return;
    }
    if client_for_player(world, player_id).is_none() {
        return;
    }
    let target_ok = world
        .objects
        .get_component::<Player>(&player_id)
        .is_some_and(|t| t.clan_id == 0 && t.clan_join_expiry_time < now_millis());
    if !target_ok {
        let expiry = world
            .objects
            .get_component::<Player>(&player_id)
            .map(|t| t.clan_join_expiry_time)
            .unwrap_or(0);
        if expiry > now_millis() {
            send_sm_with(
                world,
                player,
                sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING,
                &[SmParam::Text(player_name(world, player_id))],
            );
        }
        return;
    }
    add_clan_member(world, clan_id, player_id, 0);
    remove_player_application(world, clan_id, player_id);
}

/// `RequestPledgeDraftListSearch` (ex 0xDC): the leader's search of clanless
/// waiting players.
pub(crate) fn handle_request_pledge_draft_list_search(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let mut r = PacketReader::new(ex_body);
    let Some(level_min) = r.read_i32() else {
        return;
    };
    let Some(level_max) = r.read_i32() else {
        return;
    };
    let Some(_class_id) = r.read_i32() else {
        return;
    }; // read, TODO: role filter unhandled in Java too
    let Some(query) = r.read_string() else { return };
    let Some(sort) = r.read_i32() else { return };
    let Some(descending_raw) = r.read_i32() else {
        return;
    };
    let descending = descending_raw == 2;

    let mut rows: Vec<&PledgeWaitingInfo> = if query.is_empty() {
        world
            .recruit_waiting
            .values()
            .filter(|p| p.level >= level_min && p.level <= level_max)
            .collect()
    } else {
        let q = query.to_lowercase();
        world
            .recruit_waiting
            .values()
            .filter(|p| p.name.to_lowercase().contains(&q))
            .collect()
    };
    if query.is_empty() {
        let sort_by = sort.clamp(1, 4);
        rows.sort_by(|a, b| {
            let ord = match sort_by {
                1 => a.name.cmp(&b.name),
                2 => a.karma.cmp(&b.karma),
                3 => a.level.cmp(&b.level),
                _ => a.class_id.cmp(&b.class_id),
            };
            if descending {
                ord.reverse()
            } else {
                ord
            }
        });
    }
    let out: Vec<_> = rows
        .iter()
        .map(|p| (p.player_id, p.name.clone(), p.karma, p.class_id, p.level))
        .collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_draft_list_search(&out));
    }
}

/// `RequestPledgeDraftListApply` (ex 0xDD): a clanless player adds/removes
/// themselves from the global waiting list. `apply_type`: 0 remove, 1 add.
pub(crate) fn handle_request_pledge_draft_list_apply(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(apply_type) = r.read_i32() else {
        return;
    };
    let Some(karma) = r.read_i32() else { return };
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id != 0 {
        send_sm_with(
            world,
            player,
            sm_ids::ONLY_THE_CLAN_LEADER_OR_RANK_MANAGER_MAY_REGISTER_THE_CLAN,
            &[],
        );
        return;
    }
    match apply_type {
        0 => {
            if remove_from_waiting_list(world, player) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTRY_APPLICATION_CANCELLED_YOU_MAY_APPLY_AFTER_5_MINUTES,
                    &[],
                );
            }
        }
        1 => {
            let info = PledgeWaitingInfo {
                player_id: player,
                level: p.level,
                karma,
                class_id: p.class_id,
                name: p.name.clone(),
            };
            if add_to_waiting_list(world, info) {
                send_sm_with(
                    world,
                    player,
                    sm_ids::ENTERED_INTO_WAITING_LIST_AUTO_DELETED_AFTER_30_DAYS,
                    &[],
                );
            } else {
                send_sm_with(
                    world,
                    player,
                    sm_ids::YOU_MAY_APPLY_FOR_ENTRY_AFTER_S1_MINUTES_DUE_TO_CANCELLING,
                    &[SmParam::Long(player_lock_minutes(world, player))],
                );
            }
        }
        _ => {}
    }
}

/// `RequestPledgeSignInForOpenJoiningMethod` (ex 0x111): instant self-join
/// into a clan whose recruitment listing is `application_type` open (no
/// leader approval needed).
pub(crate) fn handle_request_pledge_sign_in_for_open_joining_method(
    world: &mut World,
    client_id: u32,
    ex_body: &[u8],
) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else {
        return;
    };
    let player = session.player_object_id();
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else {
        return;
    };
    let Some(recruit) = world.recruit_clans.get(&clan_id) else {
        return;
    };
    let _ = recruit;
    let Some(p) = world.objects.get_component::<Player>(&player) else {
        return;
    };
    if p.clan_id != 0 {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else {
        return;
    };
    if clan.char_penalty_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY,
            &[],
        );
        return;
    }
    if p.clan_join_expiry_time > now_millis() {
        send_sm_with(
            world,
            player,
            sm_ids::C1_CANNOT_JOIN_THE_CLAN_ONE_DAY_HAS_NOT_PASSED_SINCE_LEAVING,
            &[SmParam::Text(p.name.clone())],
        );
        return;
    }
    if clan.sub_pledge_members_count(0) >= clan.max_members_of(0) {
        send_sm_with(
            world,
            player,
            sm_ids::S1_IS_FULL_AND_CANNOT_ACCEPT_ADDITIONAL_CLAN_MEMBERS,
            &[SmParam::Text(clan.name.clone())],
        );
        return;
    }
    add_clan_member(world, clan_id, player, 0);
    remove_player_application(world, clan_id, player);
}
