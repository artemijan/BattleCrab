//! Clans — the G11 creation/display slice: `ClanTable.createClan` behind
//! the village-master `create_clan` bypass, the pledge-window packets, the
//! enter/leave-world roster notifications, the Clan Advent leader-online aura
//! (`ClanMaster`'s login/logout listeners), and clan (pledge) skills —
//! `//give_clan_skills`, re-applied to each member on login and stripped on
//! dispersal (Java `Clan.addNewSkill`/`addSkillEffects`/`removeSkillEffects`).
//! Invites/wars/levels/crests and everything else clan stay deferred (see the
//! G11 plan).

use commons::network::PacketReader;
use tracing::warn;

use crate::db::DbCommand;
use crate::model::clan::{Clan, ClanMember, ALL_CLAN_PRIVILEGES};
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
        expires_at_tick: u64::MAX,
        passive: true,
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
/// Advanced Headquarters (326) is noble-only; nobility isn't tracked yet, so it
/// is skipped. TODO(G24): the cast behaviour (flag/HQ spawn, castle engrave)
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
    let current: std::collections::HashMap<i32, i32> =
        world.clans.get(&clan_id).map(|c| c.skills.clone()).unwrap_or_default();
    let to_add = world.data.pledge_skill_trees.max_pledge_skills(clan_level, &current, include_squad);
    for &(skill_id, level) in &to_add {
        add_clan_skill(world, clan_id, skill_id, level);
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
    to_add.len()
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
        }
    };
    let clan = Clan { id: clan_id, name: name.clone(), leader_id: leader_oid, level: 0, reputation_score: 0, castle_id: 0, members: vec![leader], skills: Default::default(), warehouse: Default::default() };
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
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.clan_leader = is_leader;
        p.pledge_class = pledge_class;
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
