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
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
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
    let Some(clan) = world.clans.get(&clan_id) else { return Vec::new() };
    clan.members.iter().map(|m| m.char_id).filter(|&oid| client_for_player(world, oid).is_some()).collect()
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
    let Some(skill) = world.data.skill_data.get(CLAN_ADVENT_SKILL_ID, CLAN_ADVENT_SKILL_LEVEL).cloned() else {
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
    let is_leader = world.clans.get(&clan_id).is_some_and(|c| c.leader_id == object_id);
    if is_leader {
        for oid in online_members(world, clan_id) {
            apply_clan_advent(world, oid);
        }
    } else {
        let leader_online =
            world.clans.get(&clan_id).map(|c| c.leader_id).is_some_and(|lid| client_for_player(world, lid).is_some());
        if leader_online {
            apply_clan_advent(world, object_id);
        }
    }
}

// --- Clan skills (pledge skill tree): `//give_clan_skills` + login re-apply ---

/// Java `addSkillEffects`'s per-skill gate: a member receives clan skill
/// `(id, level)` when it has no `<socialClass>` requirement, or their pledge
/// class clears it (`pledgeClass + 1 >= socialClass.ordinal()`).
fn member_qualifies_for_clan_skill(world: &World, clan_id: i32, member_oid: i32, skill_id: i32, level: i32) -> bool {
    let Some(req) = world.data.pledge_skill_trees.social_class_of(skill_id, level) else {
        return true;
    };
    let pledge_class = world.clans.get(&clan_id).map(|c| c.pledge_class_of(member_oid)).unwrap_or(0);
    pledge_class as i32 + 1 >= req as i32
}

/// Fold a clan (passive) skill's stat effects onto a member as a hidden
/// permanent buff — the `passive: true` route [`crate::model::
/// conditioned_passive_buffs`] uses, so it contributes to stats without an
/// abnormal-status icon. Records it in [`ClanSkills`] (transient, never written
/// to `character_skills`). Replaces any lower-level instance; a no-op if already
/// applied at `level`.
fn apply_clan_skill_to_member(world: &mut World, member_oid: i32, skill_id: i32, level: i32) {
    let existing = world.objects.get_component::<ClanSkills>(&member_oid).and_then(|c| c.0.get(&skill_id).copied());
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
    let effects = world.data.skill_data.get(skill_id, level).map(|s| s.stat_modifier_effects()).unwrap_or_default();
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
    if let Some((target, base, mut mods, inventory, mut buffs, mut speeds, mut combat)) = world
        .objects
        .get_many_mut::<(&mut Player, &BaseStats, &mut StatModifiers, &Inventory, &mut Buffs, &mut Speeds, &mut CombatStats)>(
            &oid,
        )
    {
        target.apply_buff(&world.data, base, &mut mods, &inventory, &mut buffs, &mut speeds, &mut combat, buff);
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
    world.clans.get(&clan_id).map(|c| c.skills.iter().map(|(&k, &v)| (k, v)).collect()).unwrap_or_default()
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
    let Some(clan) = world.clans.get(&clan_id) else { return };
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
    let ids: Vec<i32> =
        world.objects.get_component::<ClanSkills>(&member_oid).map(|c| c.0.keys().copied().collect()).unwrap_or_default();
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
    let name = world.data.skill_data.get(skill_id, level).map(|s| s.name.clone()).unwrap_or_default();
    let _ = world.db.send(DbCommand::SaveClanSkill { clan_id, skill_id, skill_level: level, skill_name: name });
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
                    &[SmParam::SkillName { id: skill_id, level }],
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
        .map(|c| c.skills.keys().copied().filter(|&id| world.data.pledge_skill_trees.is_residence_skill(id)).collect())
        .unwrap_or_default();
    for id in residence {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.skills.remove(&id);
        }
        let _ = world.db.send(DbCommand::DeleteClanSkill { clan_id, skill_id: id });
        for oid in online_members(world, clan_id) {
            if world.objects.get_component::<ClanSkills>(&oid).is_some_and(|c| c.0.contains_key(&id)) {
                crate::game_loop::skills::effects::handle_buff_expire(world, oid, id);
                if let Some(c) = world.objects.get_component_mut::<ClanSkills>(&oid) {
                    c.0.remove(&id);
                }
            }
        }
    }

    let current: std::collections::HashMap<i32, i32> =
        world.clans.get(&clan_id).map(|c| c.skills.clone()).unwrap_or_default();
    let to_add = world.data.pledge_skill_trees.max_pledge_skills(clan_level, &current, include_squad);
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
    world.clans.get(&clan_id).map(|c| c.skills.len()).unwrap_or(0)
}

/// `VillageMaster.onBypassFeedback`'s `create_clan` branch: parse the typed name
/// (rejecting embedded spaces, Java's `isValidName` reject) then run
/// [`create_clan`] for the acting player. `args` is everything after the verb.
pub(crate) fn handle_create_clan(world: &mut World, client_id: u32, player_oid: i32, args: &str) {
    let mut tokens = args.split(' ').filter(|t| !t.is_empty());
    let Some(name) = tokens.next().map(str::to_string) else { return }; // empty → silent, like Java
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
        send_sm(world, leader_client, sm_ids::YOU_DO_NOT_MEET_THE_CRITERIA_IN_ORDER_TO_CREATE_A_CLAN);
        return None;
    }
    if p.clan_id != 0 {
        send_sm(world, leader_client, sm_ids::YOU_HAVE_FAILED_TO_CREATE_A_CLAN);
        return None;
    }
    if now_millis() < p.clan_create_expiry_time {
        send_sm(world, leader_client, sm_ids::YOU_MUST_WAIT_10_DAYS_BEFORE_CREATING_A_NEW_CLAN);
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) || name.len() < 2 {
        send_sm(world, leader_client, sm_ids::CLAN_NAME_IS_INVALID);
        return None;
    }
    if name.len() > 16 {
        send_sm(world, leader_client, sm_ids::CLAN_NAME_S_LENGTH_IS_INCORRECT);
        return None;
    }
    if world.clans.values().any(|c| c.name.eq_ignore_ascii_case(&name)) {
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
        let p = world.objects.get_component::<Player>(&leader_oid).expect("checked above");
        ClanMember {
            char_id: leader_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
            power_grade: 1, // Java restore: the leader holds grade 1
            title: p.title.clone(),
        }
    };
    let clan = Clan { id: clan_id, name: name.clone(), leader_id: leader_oid, level: 0, reputation_score: 0, castle_id: 0, members: vec![leader], skills: Default::default(), warehouse: Default::default(), char_penalty_expiry_time: 0, dissolving_expiry_time: 0, rank_privs: Default::default(), new_leader_id: 0 };
    let _ = world.db.send(DbCommand::InsertClan { clan_id, name: name.clone(), leader_id: leader_oid });
    let _ = world.db.send(DbCommand::UpdateCharClan {
        char_id: leader_oid,
        clan_id,
        clan_privs: ALL_CLAN_PRIVILEGES,
    });
    {
        let p = world.objects.get_component_mut::<Player>(&leader_oid).expect("checked above");
        p.clan_id = clan_id;
        p.clan_privs = ALL_CLAN_PRIVILEGES;
        p.clan_leader = true;
        p.power_grade = 1;
        p.pledge_class = clan.pledge_class_of(leader_oid); // 0 at level 0
    }

    if let Some(cs) = world.clients.get(&leader_client) {
        cs.send(server_packets::pledge_show_info_update(&clan));
        cs.send(server_packets::pledge_show_member_list_all(&clan, &world.objects));
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
        let Some(clan) = world.clans.get_mut(&clan_id) else { return };
        clan.level = level;
        clan.members.iter().map(|m| m.char_id).collect()
    };
    let _ = world.db.send(DbCommand::UpdateClanLevel { clan_id, level });
    let info = server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
    broadcast_to_clan(world, clan_id, &info);
    broadcast_to_clan(world, clan_id, &crate::network::enter_world::system_message(sm_ids::YOUR_CLAN_S_LEVEL_HAS_INCREASED));
    for oid in member_ids {
        // The level change may cross a pledge-class boundary (the on-head crown);
        // recompute per member before the UserInfo/CharInfo re-broadcast.
        let pledge_class = world.clans.get(&clan_id).map_or(0, |c| c.pledge_class_of(oid));
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
    let _ = world.db.send(DbCommand::UpdateClanReputation { clan_id, reputation: new_score });
    let info = server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("checked above"));
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
        let Some(clan) = world.clans.get(&clan_id) else { return };
        (clan.leader_id, clan.members.iter().map(|m| m.char_id).collect::<Vec<_>>())
    };
    // Java broadcasts the dispersal message before tearing the roster down.
    broadcast_to_clan(world, clan_id, &crate::network::enter_world::system_message(sm_ids::CLAN_HAS_DISPERSED));

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
    let _ = world.db.send(DbCommand::DestroyClan { clan_id, leader_id, leader_expiry });
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
    if !matches!(world.clients.get(&client_id), Some(crate::session::ClientSession::InGame(_))) {
        return;
    }
    let Some(clan_id) = PacketReader::new(body).read_i32() else { return };
    let Some(clan) = world.clans.get(&clan_id) else { return };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_info(clan));
    }
}

/// `RequestPledgeRecruitInfo` (ex 0xD3): a clan's recruitment summary,
/// answered with `ExPledgeRecruitInfo`. Java resolves the clan through
/// `ClanTable` and stays silent for an unknown id.
pub(crate) fn handle_request_pledge_recruit_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(clan_id) = PacketReader::new(ex_body).read_i32() else { return };
    let Some(clan) = world.clans.get(&clan_id) else { return };
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_info(clan));
    }
}

/// `RequestPledgeRecruitBoardSearch` (ex 0xD4): the recruit-board tab's
/// filter search, answered with one `ExPledgeRecruitBoardSearch` page. Java
/// picks an unsorted/sorted/by-name list from `ClanEntryManager` depending on
/// the filters; the registry is unported (TODO(G18): `ClanEntryManager` +
/// the `RequestPledgeRecruit*` board/apply family), so every branch pages an
/// empty list — only the echoed page number survives. The full field order is
/// still read so a malformed packet is dropped like Java's failed `readImpl`.
pub(crate) fn handle_request_pledge_recruit_board_search(
    world: &World,
    client_id: u32,
    ex_body: &[u8],
) {
    let mut r = PacketReader::new(ex_body);
    let Some(_clan_level) = r.read_i32() else { return };
    let Some(_karma) = r.read_i32() else { return };
    let Some(_search_type) = r.read_i32() else { return };
    let Some(_query) = r.read_string() else { return };
    let Some(_sort) = r.read_i32() else { return };
    let Some(_descending) = r.read_i32() else { return };
    let Some(page) = r.read_i32() else { return };
    // Trailing applicationType int: Java reads it but never uses it.
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_board_search_empty(page));
    }
}

/// `RequestPledgeRecruitApplyInfo` (ex 0xDE): the clan window polls the
/// player's clan-entry status on open. Java answers ORDERED for the leader
/// of a clan registered in `ClanEntryManager` and WAITING for a clanless
/// player with a pending application; the recruitment registry is unported
/// (TODO(G18): `ClanEntryManager` + the `RequestPledgeRecruit*` board/apply
/// family), so with no way to register both branches fall through to
/// DEFAULT — exactly Java's answer on an empty registry.
pub(crate) fn handle_request_pledge_recruit_apply_info(world: &World, client_id: u32) {
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_pledge_recruit_apply_info(0));
    }
}

/// `EnterWorld.runImpl`'s clan section (narrowed): fix the leader flag from
/// the live table, refresh the member's level in the roster, send the
/// pledge window to the enterer and the online-status update to the rest.
pub(crate) fn on_enter_world(world: &mut World, client_id: u32, object_id: i32) {
    let Some(p) = world.objects.get_component::<Player>(&object_id) else { return };
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
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.clan_leader = is_leader;
        p.pledge_class = pledge_class;
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
        cs.send(server_packets::pledge_show_member_list_all(clan, &world.objects));
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
    let is_leader = world.clans.get(&clan_id).is_some_and(|c| c.leader_id == object_id);
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
    let Some(clan) = world.clans.get(&clan_id) else { return };
    let Some(subject_member) = clan.member(subject) else { return };
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
    let Some(clan) = world.clans.get(&clan_id) else { return };
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
    world.objects.get_component::<Player>(&oid).map(|p| p.name.clone()).unwrap_or_default()
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
fn check_clan_join_condition(world: &World, requestor_oid: i32, target_oid: i32, pledge_type: i32) -> bool {
    let Some(req) = world.objects.get_component::<Player>(&requestor_oid) else { return false };
    let clan_id = req.clan_id;
    let requestor_privs = req.clan_privs;
    let Some(clan) = world.clans.get(&clan_id) else { return false };
    if !clan.has_privilege(requestor_oid, requestor_privs, CL_JOIN_CLAN) {
        send_sm_with(world, requestor_oid, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT, &[]);
        return false;
    }
    let Some(target) = world.objects.get_component::<Player>(&target_oid) else {
        send_sm_with(world, requestor_oid, sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET, &[]);
        return false;
    };
    if requestor_oid == target_oid {
        send_sm_with(world, requestor_oid, sm_ids::YOU_CANNOT_ASK_YOURSELF_TO_APPLY_TO_A_CLAN, &[]);
        return false;
    }
    if clan.char_penalty_expiry_time > now_millis() {
        send_sm_with(world, requestor_oid, sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY, &[]);
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
        send_sm_with(world, requestor_oid, sm_ids::IN_ORDER_TO_JOIN_THE_CLAN_ACADEMY_YOU_MUST_BE_UNAFFILIATED, &[]);
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
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(target_oid) = r.read_i32() else { return };
    let Some(pledge_type) = r.read_i32() else { return };

    let clan_id = world.objects.get_component::<Player>(&player).map(|p| p.clan_id).unwrap_or(0);
    if clan_id == 0 {
        return; // Java: getClan() == null → silent
    }
    // Java resolves the target through `World.getPlayer(objectId)` (online only).
    if client_for_player(world, target_oid).is_none() {
        send_sm_with(world, player, sm_ids::YOU_HAVE_INVITED_THE_WRONG_TARGET, &[]);
        return;
    }
    if !check_clan_join_condition(world, player, target_oid, pledge_type) {
        return;
    }
    if pledge_type != 0 {
        // TODO(G18.6): academy/royal/knight-unit invites need sub-pledges; the
        // Java accept path files the member under the sub-unit and (for the
        // academy) sets power grade 9 + lvlJoinedAcademy.
        warn!("Clan invite with pledge type {pledge_type} refused — sub-pledges unported.");
        return;
    }
    // Java `player.getRequest().setRequest(target, this)` — busy targets answer
    // "on another task" (the shared transaction-slot behavior).
    if world.objects.has_component::<crate::model::components::PendingRequest>(&player)
        || world.objects.has_component::<crate::model::components::PendingRequest>(&target_oid)
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
        crate::model::components::RequestKind::ClanInvite { clan_id, pledge_type },
        super::party::REQUEST_TIMEOUT_TICKS,
    );
    let clan_name = world.clans.get(&clan_id).map(|c| c.name.clone()).unwrap_or_default();
    send_to_member(
        world,
        target_oid,
        server_packets::ask_join_pledge(player, &player_name(world, player), pledge_type, &clan_name),
    );
}

/// `RequestAnswerJoinPledge` (0x27): the invited player answered the
/// `AskJoinPledge` dialog. Decline notifies both sides; accept re-checks the
/// join condition and runs `Clan.addClanMember` (roster + packets + skills).
pub(crate) fn handle_request_answer_join_pledge(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let answer = PacketReader::new(body).read_i32().unwrap_or(0);

    let Some(req) = world.objects.get_component::<crate::model::components::PendingRequest>(&player).copied() else {
        return;
    };
    let crate::model::components::RequestKind::ClanInvite { clan_id, pledge_type } = req.kind else { return };
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
    if world.objects.get_component::<Player>(&requestor).map(|p| p.clan_id) != Some(clan_id) {
        return;
    }
    if !check_clan_join_condition(world, requestor, player, pledge_type) {
        return;
    }
    if world.objects.get_component::<Player>(&player).map(|p| p.clan_id).unwrap_or(0) != 0 {
        return;
    }
    add_clan_member(world, clan_id, player);
}

/// Java `RequestAnswerJoinPledge`'s accept half + `Clan.addClanMember`: put the
/// new member in the roster, wire their clan fields, and send the join burst.
/// New members start at power grade 5 with no rank privileges (the rank-privs
/// table is a later slice — Java's fresh-clan `getRankPrivs(5)` is CP_NOTHING).
fn add_clan_member(world: &mut World, clan_id: i32, player_oid: i32) {
    send_to_member(world, player_oid, server_packets::join_pledge(clan_id));

    let member = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
        ClanMember {
            char_id: player_oid,
            name: p.name.clone(),
            level: p.level,
            class_id: p.class_id,
            sex: p.is_female as i32,
            race: p.race,
            power_grade: 5, // Java: "new member starts at 5"
            title: p.title.clone(),
        }
    };
    let Some(clan) = world.clans.get_mut(&clan_id) else { return };
    clan.members.push(member.clone());
    let pledge_class = clan.pledge_class_of(player_oid);
    // Java `player.setClanPrivileges(clan.getRankPrivs(player.getPowerGrade()))`.
    let privs = clan.rank_privs_of(5);
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.clan_id = clan_id;
        p.clan_privs = privs;
        p.clan_leader = false;
        p.power_grade = 5;
        p.pledge_class = pledge_class;
        p.clan_join_expiry_time = 0; // Java `setClanJoinExpiryTime(0)`
    }
    let _ = world.db.send(DbCommand::UpdateCharClan { char_id: player_oid, clan_id, clan_privs: privs });
    let _ = world.db.send(DbCommand::UpdateCharPowerGrade { char_id: player_oid, power_grade: 5 });
    let _ = world.db.send(DbCommand::UpdateCharClanJoinExpiry { char_id: player_oid, expiry: 0 });

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
    let info = server_packets::pledge_show_info_update(world.clans.get(&clan_id).expect("inserted above"));
    let count =
        server_packets::ex_pledge_count(world.clans.get(&clan_id).map(|c| c.members.len()).unwrap_or(0) as i32);
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
    let Some(clan) = world.clans.get_mut(&clan_id) else { return };
    let Some(idx) = clan.members.iter().position(|m| m.char_id == member_oid) else {
        warn!("Member {member_oid} not found in clan {clan_id} while trying to remove.");
        return;
    };
    clan.members.remove(idx);
    let was_leader = clan.leader_id == member_oid;
    let leader_expiry = if was_leader { now_millis() + CLAN_CREATE_COOLDOWN_MS } else { 0 };

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
            p.clan_join_expiry_time = clan_join_expiry;
            if was_leader {
                p.clan_create_expiry_time = leader_expiry;
            }
        }
        send_to_member(world, member_oid, server_packets::pledge_show_member_list_delete_all());
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
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 {
        send_sm_with(world, player, sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION, &[]);
        return;
    }
    if p.clan_leader {
        send_sm_with(world, player, sm_ids::A_CLAN_LEADER_CANNOT_WITHDRAW_FROM_THEIR_OWN_CLAN, &[]);
        return;
    }
    if super::combat::has_attack_stance(world, player) {
        send_sm_with(world, player, sm_ids::YOU_CANNOT_LEAVE_A_CLAN_WHILE_ENGAGED_IN_COMBAT, &[]);
        return;
    }

    let name = player_name(world, player);
    remove_clan_member(world, clan_id, player, now_millis() + CLAN_JOIN_PENALTY_MS);

    let withdrew =
        server_packets::system_message_with(sm_ids::S1_HAS_WITHDRAWN_FROM_THE_CLAN, &[SmParam::Text(name.clone())]);
    broadcast_to_clan(world, clan_id, &withdrew);
    broadcast_to_clan(world, clan_id, &server_packets::pledge_show_member_list_delete(&name));
    let count =
        server_packets::ex_pledge_count(world.clans.get(&clan_id).map(|c| c.members.len()).unwrap_or(0) as i32);
    broadcast_to_clan(world, clan_id, &count);
    send_sm_with(world, player, sm_ids::YOU_HAVE_WITHDRAWN_FROM_THE_CLAN, &[]);
    send_sm_with(world, player, sm_ids::AFTER_LEAVING_A_CLAN_YOU_MUST_WAIT_A_DAY_BEFORE_JOINING_ANOTHER, &[]);
}

/// `RequestOustPledgeMember` (0x29): a member with CL_DISMISS expels another
/// member by name. Both sides take a 1-day penalty: the oustee cannot join a
/// clan, the clan cannot invite (`setCharPenaltyExpiryTime`).
pub(crate) fn handle_request_oust_pledge_member(world: &mut World, client_id: u32, body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let Some(target_name) = PacketReader::new(body).read_string() else { return };
    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        send_sm_with(world, player, sm_ids::YOU_ARE_NOT_A_CLAN_MEMBER_AND_CANNOT_PERFORM_THIS_ACTION, &[]);
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else { return };
    if !clan.has_privilege(player, privs, CL_DISMISS) {
        send_sm_with(world, player, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT, &[]);
        return;
    }
    if player_name(world, player).eq_ignore_ascii_case(&target_name) {
        send_sm_with(world, player, sm_ids::YOU_CANNOT_DISMISS_YOURSELF, &[]);
        return;
    }
    let Some(member) = clan.members.iter().find(|m| m.name.eq_ignore_ascii_case(&target_name)).cloned() else {
        warn!("Oust target ({target_name}) is not a member of clan {clan_id}.");
        return;
    };
    let member_online = client_for_player(world, member.char_id).is_some();
    if member_online && super::combat::has_attack_stance(world, member.char_id) {
        send_sm_with(world, player, sm_ids::A_CLAN_MEMBER_MAY_NOT_BE_DISMISSED_DURING_COMBAT, &[]);
        return;
    }

    let penalty_until = now_millis() + CLAN_JOIN_PENALTY_MS;
    remove_clan_member(world, clan_id, member.char_id, penalty_until);
    let dissolving = world.clans.get(&clan_id).map(|c| c.dissolving_expiry_time).unwrap_or(0);
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
    send_sm_with(world, player, sm_ids::YOU_HAVE_SUCCEEDED_IN_EXPELLING_THE_CLAN_MEMBER, &[]);
    send_sm_with(world, player, sm_ids::AFTER_A_CLAN_MEMBER_IS_DISMISSED_THE_CLAN_MUST_WAIT_A_DAY, &[]);
    broadcast_to_clan(world, clan_id, &server_packets::pledge_show_member_list_delete(&member.name));
    let count =
        server_packets::ex_pledge_count(world.clans.get(&clan_id).map(|c| c.members.len()).unwrap_or(0) as i32);
    broadcast_to_clan(world, clan_id, &count);
    if member_online {
        send_sm_with(world, member.char_id, sm_ids::YOU_HAVE_RECENTLY_BEEN_DISMISSED_FROM_A_CLAN, &[]);
    }
}

/// `VillageMaster.dissolveClan` (the `dissolve_clan` bypass): guard chain,
/// then stamp `dissolving_expiry_time`, hit the leader with a full death-XP
/// penalty, and schedule the delayed removal.
pub(crate) fn handle_dissolve_clan(world: &mut World, client_id: u32, player_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else { return };
    // TODO(G18.5): Java rejects while in an alliance (SM 554) — no alliances yet.
    // TODO(G18.4): Java rejects while at war (SM 264) — no clan wars yet.
    if clan.castle_id != 0 {
        // Java folds castle/clan-hall/fort ownership into SM 266.
        send_sm(world, client_id, sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_WHILE_OWNING_A_CLAN_HALL_OR_CASTLE);
        return;
    }
    if world.sieges.values().any(|s| s.is_registered(clan_id)) {
        send_sm(world, client_id, sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE);
        return;
    }
    if let Some(pos) = world.objects.get_component::<crate::model::components::Position>(&player_oid) {
        if world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z).is_some() {
            send_sm(world, client_id, sm_ids::YOU_CANNOT_DISSOLVE_A_CLAN_DURING_A_SIEGE);
            return;
        }
    }
    if clan.dissolving_expiry_time > now_millis() {
        send_sm(world, client_id, sm_ids::YOU_HAVE_ALREADY_REQUESTED_THE_DISSOLUTION_OF_YOUR_CLAN);
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
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let char_penalty = {
        let Some(c) = world.clans.get_mut(&clan_id) else { return };
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
    world
        .scheduler
        .schedule(world.tick + delay_ticks, crate::scheduler::ScheduledTask::ClanDissolve { clan_id });
}

/// `ClanTable.scheduleRemoveClan`'s body at fire time: destroy only if the
/// dissolution is still requested and has come due (a `recover_clan` in the
/// meantime zeroes the stamp and turns this into a no-op).
pub(crate) fn handle_clan_dissolve_task(world: &mut World, clan_id: i32) {
    let Some(clan) = world.clans.get(&clan_id) else { return };
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
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
    let clan_id = p.clan_id;
    let sp = p.sp;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else { return };
    if now_millis() < clan.dissolving_expiry_time {
        send_sm(world, client_id, sm_ids::AS_YOU_ARE_SCHEDULED_FOR_CLAN_DISSOLUTION_LEVEL_CANNOT_INCREASE);
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
        send_sm(world, client_id, sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET);
        return;
    }
    if !super::quests::take_items(world, client_id, player_oid, item_id, item_count) {
        send_sm(world, client_id, sm_ids::THE_CONDITIONS_TO_INCREASE_THE_CLAN_S_LEVEL_HAVE_NOT_BEEN_MET);
        return;
    }
    // The consumption messages: adena sends its own line (Java `reduceAdena
    // (sendMessage=true)`), proof items send the destroy line + `levelUpClan`'s
    // explicit `S1_DISAPPEARED` (Java double-messages here — kept faithful).
    if item_id == ADENA {
        send_sm_with(world, player_oid, sm_ids::S1_ADENA_DISAPPEARED, &[SmParam::Long(item_count)]);
    } else {
        send_sm_with(
            world,
            player_oid,
            sm_ids::S2_S1_S_DISAPPEARED,
            &[SmParam::ItemName(item_id), SmParam::Long(item_count)],
        );
        send_sm_with(world, player_oid, sm_ids::S1_DISAPPEARED, &[SmParam::ItemName(item_id)]);
    }
    if let Some(p) = world.objects.get_component_mut::<Player>(&player_oid) {
        p.sp -= sp_cost;
    }
    send_sm_with(world, player_oid, sm_ids::YOUR_SP_HAS_DECREASED_BY_S1, &[SmParam::Int(sp_cost as i32)]);

    // Java refreshes the leader's SP (UserInfo CURRENT_HPMPCP_EXP_SP) + item
    // list; the full re-send stands in (the port's usual substitution).
    super::party::broadcast_user_info(world, player_oid);

    set_clan_level(world, clan_id, level + 1);

    // The level-up flourish: `MagicSkillUse(player, 5103, 1, 0, 0)` +
    // `MagicSkillLaunched`, broadcast from the leader.
    if let Some(pos) = world.objects.get_component::<crate::model::components::Position>(&player_oid).copied() {
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
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_villagemaster_html(world, client_id, "NotClanLeader.htm");
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else { return };
    let available = world.data.pledge_skill_trees.available_pledge_skills(clan.level, &clan.skills);
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
    let rows: Vec<(i32, i32, i32, i64)> =
        available.iter().map(|l| (l.skill_id, l.skill_level, l.get_level, l.level_up_sp)).collect();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::ex_acquirable_skill_list_by_class(ACQUIRE_TYPE_PLEDGE, &rows));
    }
}

/// Serve a `data/html/villagemaster/<file>` window (Java `NpcHtmlMessage.
/// setFile` with no NPC binding — object id 0).
fn send_villagemaster_html(world: &World, client_id: u32, file: &str) {
    let html = crate::data::htm_cache::read_htm(format!("{}data/html/villagemaster/{file}", world.data.root))
        .unwrap_or_else(|| "<html><body>My Text is missing:<br></body></html>".to_string());
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::npc_html_message(0, &html));
    }
}

/// `RequestAcquireSkillInfo`'s PLEDGE branch: the leader clicked a skill in the
/// pledge list — answer with the reputation cost (`AcquireSkillInfo`).
pub(crate) fn handle_request_pledge_skill_info(world: &World, client_id: u32, skill_id: i32, skill_level: i32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
    if p.clan_id == 0 || !p.clan_leader {
        return;
    }
    let Some(learn) = world.data.pledge_skill_trees.pledge_skill(skill_id, skill_level) else { return };
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
pub(crate) fn handle_learn_pledge_skill(world: &mut World, client_id: u32, skill_id: i32, skill_level: i32) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        return;
    }
    let Some(clan) = world.clans.get(&clan_id) else { return };
    let Some(learn) = world.data.pledge_skill_trees.pledge_skill(skill_id, skill_level).cloned() else { return };
    // Java's hack checks: the previous level must be the clan's current one
    // (`prevSkillLevel != _level - 1` reject) and the clan level must qualify
    // (the client list only ever offers qualifying entries).
    if clan.skills.get(&skill_id).copied().unwrap_or(0) != skill_level - 1 || clan.level < learn.get_level {
        return;
    }
    let rep_cost = learn.level_up_sp as i32;
    if clan.reputation_score < rep_cost {
        send_sm_with(world, player, sm_ids::SKILL_ACQUIRE_FAILED_INSUFFICIENT_CLAN_REPUTATION, &[]);
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
    let Some(clan) = world.clans.get(&clan_id) else { return };
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
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let mut r = PacketReader::new(body);
    let Some(rank) = r.read_i32() else { return };
    let Some(action) = r.read_i32() else { return };
    let privs = if action == 2 { r.read_i32().unwrap_or(0) } else { 0 };

    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
    let clan_id = p.clan_id;
    let is_leader = p.clan_leader;
    if clan_id == 0 {
        return;
    }
    if action == 2 && is_leader {
        let privs = if rank == 9 { privs & RANK9_PRIVS_MASK } else { privs };
        set_rank_privs(world, clan_id, rank, privs);
    }
    let current = world.clans.get(&clan_id).map(|c| c.rank_privs_of(rank)).unwrap_or(0);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::manage_pledge_power(rank, action, current));
    }
}

/// Java `Clan.setRankPrivs`: store + persist the rank's mask, push it onto
/// every online member holding that grade (bitmask + UserInfo), then reset the
/// clan windows.
fn set_rank_privs(world: &mut World, clan_id: i32, rank: i32, privs: i32) {
    let Some(clan) = world.clans.get_mut(&clan_id) else { return };
    clan.rank_privs.insert(rank, privs);
    let leader_id = clan.leader_id;
    let member_ids: Vec<i32> = clan.members.iter().map(|m| m.char_id).collect();
    let _ = world.db.send(DbCommand::SaveClanRankPrivs { clan_id, rank, privs });
    for oid in member_ids {
        if oid == leader_id {
            continue;
        }
        let holds_rank = world.objects.get_component::<Player>(&oid).is_some_and(|p| p.power_grade == rank);
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
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
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
fn clan_member_by_name(world: &World, player: i32, name: &str) -> Option<(i32, crate::model::clan::ClanMember)> {
    let clan_id = world.objects.get_component::<Player>(&player).map(|p| p.clan_id).filter(|&c| c != 0)?;
    let clan = world.clans.get(&clan_id)?;
    clan.members.iter().find(|m| m.name.eq_ignore_ascii_case(name)).map(|m| (clan_id, m.clone()))
}

/// `RequestPledgeMemberPowerInfo` (ex 0x14): one member's rank + that rank's
/// current privilege mask.
pub(crate) fn handle_request_pledge_member_power_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(_unk) = r.read_i32() else { return };
    let Some(name) = r.read_string() else { return };
    let Some((clan_id, member)) = clan_member_by_name(world, player, &name) else { return };
    // The live grade for an online member (roster snapshots refresh lazily).
    let grade = world
        .objects
        .get_component::<Player>(&member.char_id)
        .map(|p| p.power_grade)
        .unwrap_or(member.power_grade);
    let privs = world.clans.get(&clan_id).map(|c| c.rank_privs_of(grade)).unwrap_or(0);
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_receive_power_info(grade, &member.name, privs));
    }
}

/// `RequestPledgeMemberInfo` (ex 0x16): the member-detail pane.
pub(crate) fn handle_request_pledge_member_info(world: &World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(_unk) = r.read_i32() else { return };
    let Some(name) = r.read_string() else { return };
    let Some((clan_id, mut member)) = clan_member_by_name(world, player, &name) else { return };
    // Live title/grade for online members.
    if let Some(p) = world.objects.get_component::<Player>(&member.char_id) {
        member.title = p.title.clone();
        member.power_grade = p.power_grade;
    }
    let clan_name = world.clans.get(&clan_id).map(|c| c.name.clone()).unwrap_or_default();
    if let Some(cs) = world.clients.get(&client_id) {
        cs.send(server_packets::pledge_receive_member_info(&member, &clan_name));
    }
}

/// `RequestPledgeSetMemberPowerGrade` (ex 0x15): a CL_MANAGE_RANKS holder
/// re-ranks a member (never the leader). The new rank's privileges apply to
/// the online member immediately through the rank table refresh at
/// `broadcastClanStatus`-time in Java only on relog — we mirror Java: the
/// grade changes now, the mask follows at login/rank-edit.
pub(crate) fn handle_request_pledge_set_member_power_grade(world: &mut World, client_id: u32, ex_body: &[u8]) {
    let Some(crate::session::ClientSession::InGame(session)) = world.clients.get(&client_id) else { return };
    let player = session.player_object_id();
    let mut r = PacketReader::new(ex_body);
    let Some(name) = r.read_string() else { return };
    let Some(grade) = r.read_i32() else { return };

    let Some(p) = world.objects.get_component::<Player>(&player) else { return };
    let clan_id = p.clan_id;
    let privs = p.clan_privs;
    if clan_id == 0 {
        return;
    }
    let has_priv = world.clans.get(&clan_id).is_some_and(|c| c.has_privilege(player, privs, CL_MANAGE_RANKS));
    if !has_priv {
        return;
    }
    let Some((_, member)) = clan_member_by_name(world, player, &name) else { return };
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
    let _ = world.db.send(DbCommand::UpdateCharPowerGrade { char_id: member.char_id, power_grade: grade });

    let online = client_for_player(world, member.char_id).is_some();
    let update = {
        let c = world.clans.get(&clan_id).expect("checked above");
        c.member(member.char_id).map(|m| server_packets::pledge_show_member_list_update(m, online))
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

/// `RequestPledgeReorganizeMember` (ex 0x2C): swaps two members between
/// sub-units. With only the main pledge modelled the old and new pledge types
/// always match, which is Java's own early-out — parsed and dropped.
/// TODO(G18.6): real sub-unit moves.
pub(crate) fn handle_request_pledge_reorganize_member(_world: &mut World, _client_id: u32, ex_body: &[u8]) {
    let mut r = PacketReader::new(ex_body);
    let (Some(_selected), Some(_name), Some(_new_type), Some(_other)) =
        (r.read_i32(), r.read_string(), r.read_i32(), r.read_string())
    else {
        return;
    };
}

/// `VillageMaster`'s `change_clan_leader <name>` bypass — the delegated
/// transfer flow (`AltClanLeaderInstantActivation = False` on this dist):
/// stamp `new_leader_id` + the confirmation html. The actual `setNewLeader`
/// application runs at the daily reset — TODO(G33): `DailyTaskManager.
/// onClanLeaderChange` (no daily scheduler yet, so the stamp waits).
pub(crate) fn handle_change_clan_leader(world: &mut World, client_id: u32, player_oid: i32, npc_oid: i32, args: &str) {
    let name = args.trim();
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
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
        send_sm(world, client_id, sm_ids::THAT_PLAYER_IS_NOT_CURRENTLY_ONLINE);
        return;
    }
    // TODO(G18.6): Java rejects academy members (SM 1754) — no academy yet.
    let already_pending = world.clans.get(&clan_id).is_some_and(|c| c.new_leader_id != 0);
    let file = if already_pending {
        "9000-07-in-progress.htm"
    } else {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.new_leader_id = member.char_id;
        }
        let _ = world.db.send(DbCommand::UpdateClanNewLeader { clan_id, new_leader_id: member.char_id });
        "9000-07-success.htm"
    };
    send_clan_master_html(world, client_id, npc_oid, file);
}

/// `VillageMaster`'s `cancel_clan_leader_change` bypass.
pub(crate) fn handle_cancel_clan_leader_change(world: &mut World, client_id: u32, player_oid: i32, npc_oid: i32) {
    let Some(p) = world.objects.get_component::<Player>(&player_oid) else { return };
    let clan_id = p.clan_id;
    if clan_id == 0 || !p.clan_leader {
        send_sm(world, client_id, sm_ids::YOU_ARE_NOT_AUTHORIZED_TO_DO_THAT);
        return;
    }
    let pending = world.clans.get(&clan_id).is_some_and(|c| c.new_leader_id != 0);
    if pending {
        if let Some(c) = world.clans.get_mut(&clan_id) {
            c.new_leader_id = 0;
        }
        let _ = world.db.send(DbCommand::UpdateClanNewLeader { clan_id, new_leader_id: 0 });
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
