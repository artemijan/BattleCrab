use super::CLAN_ADVENT_SKILL_ID;
use super::CLAN_ADVENT_SKILL_LEVEL;
use crate::db::DbCommand;
use crate::game_loop::abnormal::has_buff;
use crate::game_loop::admin::refresh_skill_list;
use crate::game_loop::clans::clan_of;
use crate::game_loop::helpers;
use crate::game_loop::skills::skill_by_id;
use crate::model::Player;
use crate::model::components::skills::ClanSkills;
use crate::model::skill::active_buff::ActiveBuff;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;

/// The clan's member object-ids that are currently online (leader included).
pub(crate) fn online_members(world: &World, clan_id: i32) -> Vec<i32> {
    let Some(clan) = world.clans.get(&clan_id) else {
        return Vec::new();
    };
    clan.members
        .iter()
        .map(|m| m.char_id)
        .filter(|&oid| helpers::client_for_player(world, oid).is_some())
        .collect()
}

/// Java `ClanMaster`'s `CommonSkill.CLAN_ADVENT.getSkill().applyEffects(p, p)`:
/// self-cast Clan Advent onto one online member. Skipped when already present —
/// the buff is permanent and `irreplacableBuff` (a single abnormal slot), so a
/// re-trigger must not stack a second copy (Java's `EffectList` replaces in
/// place; skipping the identical permanent buff is equivalent).
pub(crate) fn apply_clan_advent(world: &mut World, object_id: i32) {
    let already = has_buff(world, object_id, CLAN_ADVENT_SKILL_ID);
    if already {
        return;
    }
    let Some(skill) = skill_by_id(world, CLAN_ADVENT_SKILL_ID, CLAN_ADVENT_SKILL_LEVEL) else {
        return;
    };
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, object_id, &skill);
}

/// Java `getEffectList().stopSkillEffects(REMOVED, CommonSkill.CLAN_ADVENT)`:
/// strip Clan Advent from one player (no-op if absent). Reuses the buff-expiry
/// path, which drops the buff, reverts its stat contribution, and rebroadcasts
/// UserInfo + the abnormal-status row.
pub(crate) fn remove_clan_advent(world: &mut World, object_id: i32) {
    crate::game_loop::skills::effects::handle_buff_expire(world, object_id, CLAN_ADVENT_SKILL_ID);
}

/// Java `ClanMaster.onPlayerLogin`: the leader's login lights the Clan Advent
/// aura on every online member; a plain member's login lights it on themselves
/// only if the leader is already online.
///
/// All four of `ClanMaster`'s listeners are ported. The other three:
/// `ON_PLAYER_CLAN_JOIN` and `ON_PLAYER_CLAN_LEFT` in
/// [`membership`](membership), and `ON_PLAYER_PROFESSION_CHANGE`
/// through [`reapply_clan_advent_on_profession_change`].
pub(crate) fn apply_clan_advent_on_login(world: &mut World, clan_id: i32, object_id: i32) {
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
            .is_some_and(|lid| helpers::client_for_player(world, lid).is_some());
        if leader_online {
            apply_clan_advent(world, object_id);
        }
    }
}

/// Java `ClanMaster.onProfessionChange` (`ON_PLAYER_PROFESSION_CHANGE`).
///
/// Gate is Java's: `isClanLeader() || clan.getLeader().isOnline()`. Note the
/// leader qualifies with **no** online check of their own — they are plainly
/// online to have changed profession at all. Clanless is a no-op.
pub(crate) fn reapply_clan_advent_on_profession_change(world: &mut World, object_id: i32) {
    let Some(clan_id) = clan_of(world, object_id) else {
        return;
    };
    let Some(leader_id) = world.clans.get(&clan_id).map(|c| c.leader_id) else {
        return;
    };
    if leader_id == object_id || helpers::client_for_player(world, leader_id).is_some() {
        apply_clan_advent(world, object_id);
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
    let buff = ActiveBuff::passive_pump(skill_id, level, effects);
    apply_permanent_passive_buff(world, member_oid, buff);
}

/// The buff-application half of `apply_skill_effects` for a permanent passive:
/// fold the effects into the member's stat maps, then rebroadcast UserInfo (no
/// AbnormalStatusUpdate — passive buffs carry no icon).
fn apply_permanent_passive_buff(world: &mut World, oid: i32, buff: ActiveBuff) {
    crate::game_loop::stats::context::with_stat_ctx(world, oid, |ctx| ctx.apply(buff));
    // Clan skills like Clan Health / Clan Mind carry MaxHp/MaxMp modifiers that
    // `recalculate_stats` doesn't consume — fold them into the vitals too.
    crate::game_loop::skills::effects::recompute_max_vitals(world, oid);
    crate::game_loop::character::player_info::broadcast_user_info(world, oid);
}

/// The clan's full skill set as an `(id, level)` list (for `PledgeSkillList`).
pub(crate) fn clan_skill_pairs(world: &World, clan_id: i32) -> Vec<(i32, i32)> {
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
    let mut applied = false;
    for (id, level) in clan_skill_pairs(world, clan_id) {
        // Residence skills (a castle/clan-hall benefit) are never applied through
        // the pledge-grant channel — guards against legacy rows a pre-fix grant
        // persisted, so they don't re-apply on login. Castle ownership grants
        // them through `give_residential_skills` below instead.
        if world.data.pledge_skill_trees.is_residence_skill(id) {
            continue;
        }
        if member_qualifies_for_clan_skill(world, clan_id, member_oid, id, level) {
            apply_clan_skill_to_member(world, member_oid, id, level);
            applied = true;
        }
    }
    // Java `Player.enterWorld`: a member of a castle-owning clan gets that
    // castle's residential skills on login.
    let castle_id = world.clans.get(&clan_id).map(|c| c.castle_id).unwrap_or(0);
    if castle_id > 0 {
        give_residential_skills(world, member_oid, castle_id, clan_id);
    }
    if applied {
        refresh_skill_list(world, member_oid);
    }
    // The clan window's skill tab (Java sends `PledgeSkillList` on enter-world).
    if let Some(cid) = helpers::client_for_player(world, member_oid) {
        let pkt = server_packets::pledge_skill_list(&clan_skill_pairs(world, clan_id));
        helpers::send_to_client(world, cid, pkt);
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
/// The cast behaviour behind them has since landed: Build Headquarters plants
/// a siege flag (`siege::build_headquarters`, reached from the
/// `CreateHeadquarter` effect) and the castle is taken by touching the throne
/// -room artifact (`siege::try_capture_artifact`).
pub(crate) fn apply_siege_skills_to_leader(world: &mut World, clan_id: i32, member_oid: i32) {
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
    refresh_skill_list(world, member_oid);
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
    refresh_skill_list(world, member_oid);
}

// --- Residential (castle/clan-hall) skills: `AbstractResidence.give/
// removeResidentialSkills`, granted through residence ownership rather than the
// `//give_clan_skills` pledge grant. They ride the same transient [`ClanSkills`]
// passive channel (never persisted), keyed by their own skill ids (590+). ---

/// Java `AbstractResidence.giveResidentialSkills(player)` — grant every
/// residential skill of `residence_id` this member's pledge class qualifies for
/// (the `pledgeClass + 1 >= socialClass` gate, shared with clan skills).
pub(crate) fn give_residential_skills(
    world: &mut World,
    member_oid: i32,
    residence_id: i32,
    clan_id: i32,
) {
    let skills: Vec<(i32, i32)> = world
        .data
        .pledge_skill_trees
        .available_residential_skills(residence_id)
        .iter()
        .map(|l| (l.skill_id, l.skill_level))
        .collect();
    let mut applied = false;
    for (id, level) in skills {
        if member_qualifies_for_clan_skill(world, clan_id, member_oid, id, level) {
            apply_clan_skill_to_member(world, member_oid, id, level);
            applied = true;
        }
    }
    if applied {
        refresh_skill_list(world, member_oid);
    }
}

/// Java `AbstractResidence.removeResidentialSkills(player)` — strip a residence's
/// skills from a member (unconditionally, unlike the gated grant), reverting each
/// passive buff the member actually holds.
pub(crate) fn remove_residential_skills(world: &mut World, member_oid: i32, residence_id: i32) {
    let ids: Vec<i32> = world
        .data
        .pledge_skill_trees
        .available_residential_skills(residence_id)
        .iter()
        .map(|l| l.skill_id)
        .collect();
    let mut removed = false;
    for id in ids {
        let has = world
            .objects
            .get_component::<ClanSkills>(&member_oid)
            .is_some_and(|c| c.0.contains_key(&id));
        if !has {
            continue;
        }
        crate::game_loop::skills::effects::handle_buff_expire(world, member_oid, id);
        if let Some(c) = world.objects.get_component_mut::<ClanSkills>(&member_oid) {
            c.0.remove(&id);
        }
        removed = true;
    }
    if removed {
        refresh_skill_list(world, member_oid);
    }
}

/// `Castle.setOwner`'s skill half, clan-wide: grant a residence's skills to
/// every **online** member. (Offline members pick them up at login, through
/// `apply_clan_skills_to_member`.)
pub(crate) fn grant_residential_skills_to_clan(world: &mut World, clan_id: i32, residence_id: i32) {
    for member in online_members(world, clan_id) {
        give_residential_skills(world, member, residence_id, clan_id);
    }
}

/// `Castle.removeOwner`'s skill half, clan-wide.
pub(crate) fn strip_residential_skills_from_clan(
    world: &mut World,
    clan_id: i32,
    residence_id: i32,
) {
    for member in online_members(world, clan_id) {
        remove_residential_skills(world, member, residence_id);
    }
}

/// Java `Clan.setNewLeader` as `//clan_changeleader` forces it: swap the
/// leader id, move the full-privilege mask, refresh both players' flags and
/// UserInfo, tell the clan, and persist. (The player-initiated deliberate
/// transfer flow stays unported; this is the GM override.)
pub(crate) fn force_new_leader(world: &mut World, clan_id: i32, new_leader: i32) -> bool {
    let Some(old_leader) = world.clans.get(&clan_id).map(|c| c.leader_id) else {
        return false;
    };
    if old_leader == new_leader {
        return false;
    }
    if let Some(c) = world.clans.get_mut(&clan_id) {
        c.leader_id = new_leader;
        c.new_leader_id = 0;
    }
    let _ = world.db.send(DbCommand::UpdateClanLeader {
        clan_id,
        leader_id: new_leader,
    });
    for (oid, is_leader) in [(old_leader, false), (new_leader, true)] {
        // Java `Clan.setNewLeader` re-derives **both** ranks through
        // `setPledgeClass(calculatePledgeClass(...))` — the handover moves the
        // crown, and with it the pledge class the clan-rank gear is gated on.
        let pledge_class = world
            .clans
            .get(&clan_id)
            .map_or(0, |c| c.pledge_class_of(oid));
        if let Some(p) = world.objects.get_component_mut::<Player>(&oid) {
            p.clan_leader = is_leader;
            p.clan_privs = if is_leader { i32::MAX } else { 0 };
            p.pledge_class = pledge_class;
        }
        if world.objects.has_component::<Player>(&oid) {
            // `setPledgeClass` → `checkItemRestriction()`, which Java also
            // spells out for the ex-leader on the line after the handover.
            crate::game_loop::items::check_item_restriction(world, oid);
            crate::game_loop::character::player_info::broadcast_user_info(world, oid);
        }
    }
    let name = helpers::player_name_or_empty(world, new_leader);
    for oid in online_members(world, clan_id) {
        helpers::send_sm_to_player(
            world,
            oid,
            sm_ids::CLAN_LEADER_PRIVILEGES_HAVE_BEEN_TRANSFERRED_TO_C1,
            &[SmParam::Text(name.clone())],
        );
    }
    true
}

/// Java `Clan.addNewSkill` for one skill: store it on the clan, persist it, and
/// push it to every qualifying online member (buff + skill list +
/// `PledgeSkillListAdd` + "clan skill added" message).
pub(crate) fn add_clan_skill(world: &mut World, clan_id: i32, skill_id: i32, level: i32) {
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
        helpers::send_to_player(
            world,
            oid,
            server_packets::pledge_skill_list_add(skill_id, level),
        );
        helpers::send_sm_to_player(
            world,
            oid,
            sm_ids::THE_CLAN_SKILL_S1_HAS_BEEN_ADDED,
            &[SmParam::SkillName {
                id: skill_id,
                level,
            }],
        );
        refresh_skill_list(world, oid);
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
    // relog doesn't re-apply them. Residence ownership grants these through its
    // own channel (`give_residential_skills`), never the pledge tree.
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
        helpers::send_to_player(world, oid, pkt.clone());
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
