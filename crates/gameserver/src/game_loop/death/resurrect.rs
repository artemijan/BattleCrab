use super::*;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::helpers::player_name_or_empty;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::send_sm_bare_to_player;
use crate::game_loop::helpers::send_sm_to_player;
use crate::game_loop::helpers::send_to_player;
use crate::game_loop::helpers::vitals_pair;
use bevy_ecs::world::Mut;

/// `Formulas.calculateSkillResurrectRestorePercent` — the reviver's WIT scales
/// how much of the lost XP their resurrection gives back.
///
/// ```java
/// if (base == 0 || base == 100) return base;
/// restore = base * WIT.calcBonus(caster);
/// if ((restore - base) > 20.0) restore += 20.0;
/// return min(max(restore, base), 90.0);
/// ```
///
/// Note the quirk on the third line: a bonus that already exceeds +20 gets a
/// *further* flat +20, so high-WIT revivers jump rather than scale smoothly.
/// Ported as written.
pub(crate) fn resurrect_restore_percent(base: f64, wit_bonus: f64) -> f64 {
    if base == 0.0 || base == 100.0 {
        return base;
    }
    let mut restore = base * wit_bonus;
    if (restore - base) > 20.0 {
        restore += 20.0;
    }
    restore.max(base).min(90.0)
}

/// `Player.reviveRequest` — propose a resurrection to a dead player.
///
/// Nothing is restored here: the corpse gets a `ConfirmDlg` and decides.
/// A second proposal while one is outstanding is refused with Java's
/// "Resurrection has already been proposed" notice, which is what stops two
/// clerics from racing.
#[allow(clippy::too_many_arguments)]
/// Java `ConditionPlayerCanResurrect.testImpl`'s siege block — the part that
/// decides whether a resurrection may even be *proposed* on a battlefield.
///
/// **Every branch refuses.** Inside a siege in progress, a normal resurrection
/// skill never works; the control-tower count and the attacker's flag count
/// only pick which of three rejection messages the caster reads. That is why
/// `Siege.control_tower_count` looked cosmetic — it is, but the *refusal* it
/// sits behind was missing entirely, so a Bishop could freely raise defenders
/// mid-siege.
///
/// Returns the system-message id to send, or `None` when the resurrection may
/// go ahead.
fn siege_resurrect_refusal(world: &World, corpse_oid: i32, skill_id: i32) -> Option<i16> {
    use crate::network::server_packets::sm_ids;
    // Java: `skill.getId() != 2393` — the Blessed Scroll of Resurrection
    // (Battleground) is the one thing that works on a battlefield.
    if skill_id == BATTLEGROUND_RESURRECTION_SKILL_ID {
        return None;
    }
    let pos = world
        .objects
        .get_component::<crate::model::components::Position>(&corpse_oid)?;
    let castle_id = world.data.zone_data.siege_castle_at(pos.x, pos.y, pos.z)?;
    let siege = world.sieges.get(&castle_id)?;
    if !siege.in_progress {
        return None;
    }
    let clan_id = clan_of_or_zero(world, corpse_oid);
    // Java keeps this branch separate from the fallthrough below, but both send
    // the same generic line — it is redundant upstream too, and kept only
    // because the shape mirrors the reference. No test can tell them apart.
    if clan_id == 0 {
        return Some(sm_ids::IT_IS_NOT_POSSIBLE_TO_RESURRECT_IN_BATTLEGROUNDS);
    }
    if siege.is_defender(clan_id) && siege.control_tower_count == 0 {
        return Some(
            sm_ids::THE_GUARDIAN_TOWER_HAS_BEEN_DESTROYED_AND_RESURRECTION_IS_NOT_POSSIBLE,
        );
    }
    if siege.is_attacker(clan_id) && !siege.flags.iter().any(|(owner, _)| *owner == clan_id) {
        return Some(sm_ids::IF_A_BASE_CAMP_DOES_NOT_EXIST_RESURRECTION_IS_NOT_POSSIBLE);
    }
    Some(sm_ids::IT_IS_NOT_POSSIBLE_TO_RESURRECT_IN_BATTLEGROUNDS)
}

/// Java `2393` — "Blessed Scroll of Resurrection (Battleground)".
const BATTLEGROUND_RESURRECTION_SKILL_ID: i32 = 2393;

#[allow(clippy::too_many_arguments)]
pub(crate) fn revive_request(
    world: &mut World,
    reviver_oid: i32,
    target_oid: i32,
    power: i32,
    hp_percent: i32,
    mp_percent: i32,
    cp_percent: i32,
    // The casting skill: its id picks out the battleground scroll, and its
    // `affectRange` is Java's blanket bypass (see below).
    skill_id: i32,
    affect_range: i32,
) {
    use crate::network::server_packets::sm_ids;
    let send_to_reviver = |world: &World, id: i16| {
        send_sm_bare_to_player(world, reviver_oid, id);
    };
    // **Java's first clause skips the whole condition for an AoE resurrection**
    // — `if (skill.getAffectRange() > 0) return true;`, carrying the comment
    // "Need skill rework for fix that properly". So Mass Resurrection (1254)
    // ignores the siege block, the already-proposed check and everything else.
    // An upstream shortcut, but a load-bearing one: it is the only normal
    // resurrection that works on a battlefield.
    if affect_range <= 0
        && let Some(refusal) = siege_resurrect_refusal(world, target_oid, skill_id)
    {
        send_to_reviver(world, refusal);
        return;
    }
    // `isResurrectionBlocked()` — Java also ORs `isInvul()`; the flag is the
    // ported half (`BlockResurrection` has no learnable source on this dist).
    if crate::game_loop::abnormal::flags_of(world, target_oid)
        & crate::model::skill::effect_flag::BLOCK_RESURRECTION
        != 0
    {
        return;
    }
    // Java `Resurrection` calls `effected.getActingPlayer().reviveRequest(…,
    // effected.isPet(), …)`: casting on a dead **pet** puts the dialog in front
    // of its **owner**, who is the one who answers. So resolve the corpse to
    // the player who will be asked, and remember which of the two is dying.
    let is_pet = world
        .objects
        .has_component::<crate::model::components::PetOf>(&target_oid);
    let corpse_oid = target_oid;
    let target_oid = if is_pet {
        match world
            .objects
            .get_component::<crate::model::components::ServitorOf>(&corpse_oid)
        {
            Some(s) => s.owner_object_id,
            None => return,
        }
    } else {
        target_oid
    };

    let Some(target) = world
        .objects
        .get_component::<crate::model::Player>(&target_oid)
    else {
        return;
    };
    if world
        .objects
        .get_component::<Vitals>(&corpse_oid)
        .is_none_or(|v| !v.dead)
    {
        return;
    }
    if target.revive_request.is_some() {
        send_sm_bare_to_player(
            world,
            reviver_oid,
            sm_ids::RESURRECTION_HAS_ALREADY_BEEN_PROPOSED,
        );
        return;
    }
    // `calculateSkillResurrectRestorePercent(power, reviver)`.
    let wit_bonus = world
        .objects
        .get_component::<crate::model::components::BaseStats>(&reviver_oid)
        .map(|b| {
            world
                .data
                .stat_bonus
                .bonus(crate::model::stats::BaseStat::Wit, b.wit)
        })
        .unwrap_or(1.0);
    let restore_percent = resurrect_restore_percent(power as f64, wit_bonus);

    let lost = if is_pet {
        // A pet's restorable exp is the gap the death penalty opened.
        world
            .objects
            .get_component::<crate::model::components::PetOf>(&corpse_oid)
            .map(|p| (p.exp_before_death - p.exp).max(0))
            .unwrap_or(0)
    } else {
        world
            .objects
            .get_component::<crate::model::Player>(&target_oid)
            .map(|p| p.lost_exp_on_death)
            .unwrap_or(0)
    };
    let restore_exp = ((lost as f64 * restore_percent) / 100.0).round() as i64;

    if let Some(p) = world
        .objects
        .get_component_mut::<crate::model::Player>(&target_oid)
    {
        p.revive_request = Some(crate::model::ReviveRequest {
            reviver: reviver_oid,
            restore_percent,
            hp_percent,
            mp_percent,
            cp_percent,
            is_pet,
        });
    }
    // Java's `ConfirmDlg(C1_IS_ATTEMPTING_TO_DO_A_RESURRECTION_THAT_RESTORES_S2_S3_XP_ACCEPT)`.
    // This port has only the generic text dialog, so the message is rendered
    // rather than composed from the client's string table.
    let reviver_name = player_name_or_empty(world, reviver_oid);
    send_to_player(
        world,
        target_oid,
        server_packets::confirm_dlg_text(&format!(
            "{reviver_name} is attempting to resurrect you, restoring {restore_exp} XP ({restore_percent:.0}%). Accept?"
        )),
    );
}

/// `Player.reviveAnswer` — the corpse's `ConfirmDlg` reply.
///
/// Returns `true` when a pending proposal was consumed, so the shared
/// `DlgAnswer` dispatch knows this reply was ours and not the admin flow's.
pub(crate) fn handle_revive_answer(world: &mut World, player_oid: i32, accepted: bool) -> bool {
    let Some(request) = world
        .objects
        .get_component_mut::<crate::model::Player>(&player_oid)
        .and_then(|p| p.revive_request.take())
    else {
        return false;
    };
    // Java re-checks the corpse is still dead — it may have used "to village"
    // while the dialog sat on screen.
    // The corpse to revive: the pet when this was a pet proposal, else the
    // answering player themselves.
    let corpse_oid = if request.is_pet {
        match crate::game_loop::servitor::pet_of(world, player_oid) {
            Some(oid) => oid,
            None => return true, // the pet went away while the dialog sat open
        }
    } else {
        player_oid
    };
    if world
        .objects
        .get_component::<Vitals>(&corpse_oid)
        .is_none_or(|v| !v.dead)
    {
        return true;
    }
    if !accepted {
        return true;
    }
    if request.is_pet {
        revive_pet(
            world,
            player_oid,
            corpse_oid,
            request.restore_percent,
            request.hp_percent,
            request.mp_percent,
        );
        return true;
    }
    let crate::model::ReviveRequest {
        restore_percent,
        hp_percent,
        mp_percent,
        cp_percent,
        ..
    } = request;
    do_revive_with(
        world,
        player_oid,
        hp_percent,
        mp_percent,
        cp_percent,
        restore_percent,
    );
    true
}

/// The three components every player revive path writes together.
fn revive_target(
    world: &mut World,
    player_oid: i32,
) -> Option<(
    Mut<'_, crate::model::Player>,
    Mut<'_, Vitals>,
    Mut<'_, PlayerVitals>,
)> {
    world
        .objects
        .get_many_mut::<(&mut crate::model::Player, &mut Vitals, &mut PlayerVitals)>(&player_oid)
}

/// Refill each pool to `percent` of its maximum, clamped there. A percentage of
/// zero or less leaves that pool as it is — Java's `if (reviveHp > 0)` guards,
/// which is how a skill that names only some of the three keeps whatever the
/// config restore already gave the others.
fn restore_pools(vitals: &mut Vitals, pvitals: &mut PlayerVitals, hp: f64, mp: f64, cp: f64) {
    if hp > 0.0 {
        vitals.cur_hp = (vitals.max_hp as f64 * hp / 100.0).min(vitals.max_hp as f64);
    }
    if mp > 0.0 {
        vitals.cur_mp = (vitals.max_mp as f64 * mp / 100.0).min(vitals.max_mp as f64);
    }
    if cp > 0.0 {
        pvitals.cur_cp = (pvitals.max_cp as f64 * cp / 100.0).min(pvitals.max_cp as f64);
    }
}

/// `Player.doRevive(double revivePower)` — revive with the skill's own
/// percentages rather than the config respawn ones, and give back
/// `revivePower`% of the XP the death cost.
pub(crate) fn do_revive_with(
    world: &mut World,
    player_oid: i32,
    hp_percent: i32,
    mp_percent: i32,
    cp_percent: i32,
    restore_percent: f64,
) {
    do_revive(world, player_oid);
    {
        let Some((mut p, mut vitals, mut pvitals)) = revive_target(world, player_oid) else {
            return;
        };
        // The skill's percentages override `do_revive`'s config defaults.
        restore_pools(
            &mut vitals,
            &mut pvitals,
            hp_percent as f64,
            mp_percent as f64,
            cp_percent as f64,
        );
        let restored = ((p.lost_exp_on_death as f64 * restore_percent) / 100.0).round() as i64;
        p.exp += restored;
        p.lost_exp_on_death = 0;
    }
    crate::game_loop::player_info::broadcast_user_info(world, player_oid);
}

/// `Player.doRevive`: restore the configured percentages (`RespawnRestoreHP`
/// = 65% on the stock config) and broadcast `Revive`.
pub(crate) fn do_revive(world: &mut World, player_oid: i32) {
    {
        let c = &world.cfg.character;
        let (hp, mp, cp) = (
            c.respawn_restore_hp,
            c.respawn_restore_mp,
            c.respawn_restore_cp,
        );
        let Some((mut p, mut vitals, mut pvitals)) = revive_target(world, player_oid) else {
            return;
        };
        vitals.dead = false;
        p.pending_revive = false;
        restore_pools(&mut vitals, &mut pvitals, hp, mp, cp);
    }
    broadcast_including_self(world, player_oid, &server_packets::revive(player_oid));
    crate::game_loop::party::notify_party_vitals(world, player_oid);
    let Some((vitals, pvitals)) = vitals_pair(world, player_oid) else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (
                    server_packets::status_update_type::CUR_HP,
                    vitals.cur_hp as i32,
                ),
                (
                    server_packets::status_update_type::CUR_MP,
                    vitals.cur_mp as i32,
                ),
                (
                    server_packets::status_update_type::CUR_CP,
                    pvitals.cur_cp as i32,
                ),
            ],
        ),
    );
}

/// `Pet.doRevive(revivePower)` — restore a share of the exp the death penalty
/// took, then bring the pet back.
///
/// Java's pet revive restores HP/MP by the skill's percentages like a player's,
/// but there is no CP on a pet.
fn revive_pet(
    world: &mut World,
    owner_oid: i32,
    pet_oid: i32,
    restore_percent: f64,
    hp_percent: i32,
    mp_percent: i32,
) {
    // `restoreExp` runs *before* `doRevive`, and consumes the record.
    crate::game_loop::servitor::pet_restore_exp(world, pet_oid, restore_percent);

    if let Some(v) = world.objects.get_component_mut::<Vitals>(&pet_oid) {
        v.dead = false;
        v.cur_hp = (v.max_hp as f64 * hp_percent as f64 / 100.0).max(1.0);
        v.cur_mp = v.max_mp as f64 * mp_percent as f64 / 100.0;
    }
    // The food clock stopped when the pet died; start it again.
    crate::game_loop::servitor::start_feed(world, pet_oid);
    crate::game_loop::servitor::send_pet_info(
        world,
        owner_oid,
        pet_oid,
        crate::game_loop::servitor::PetInfoKind::Default,
    );
    crate::game_loop::servitor::broadcast_summon_info(world, pet_oid, false);
    // The revived state is what should persist if the owner logs out now.
    crate::game_loop::servitor::sync_pet_row(world, owner_oid);
}

/// `calculateDistance3D(this) < ALT_PARTY_RANGE` — measured from the corpse.
fn in_range_of(world: &World, from: i32, to: i32, range: f64) -> bool {
    crate::geo::distance::distance_3d(world, from, to).is_some_and(|d| d < range)
}

/// `Attackable.calculateRewards`' raid-point block.
///
/// Raid points are a separate currency from exp: they go to the **top damage
/// dealer** (or the last attacker if that player has gone), and when that
/// player is in a party they are **split among party members in range**, each
/// getting at least 1.
///
/// Two conditions that are easy to lose:
/// - `!_isRaidMinion` — a boss's adds award nothing, only the boss itself.
/// - the party split uses `ALT_PARTY_RANGE` from the **corpse**, so a member
///   who hung back out of range earns nothing.
pub(crate) fn award_raid_points(world: &mut World, npc_oid: i32, earner_oid: i32) {
    use crate::network::server_packets::{SmParam, sm_ids};

    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    let Some(t) = world.data.npc_data.get(npc.npc_id) else {
        return;
    };
    // Only a real raid boss, never its minions.
    if !matches!(t.type_name.as_str(), "RaidBoss" | "GrandBoss") || t.raid_points <= 0.0 {
        return;
    }
    if world
        .objects
        .has_component::<crate::game_loop::minions::MinionOf>(&npc_oid)
    {
        return;
    }
    let total = (t.raid_points * world.cfg.rates.rate_raidboss_points) as i32;

    // `broadcastPacket(CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL)` — everyone
    // present hears it, not just the earner.
    if let Some(region) = region_cell_of(world, npc_oid) {
        broadcast_near_region_in(
            world,
            region,
            instance_of(world, npc_oid),
            &server_packets::system_message_with(
                sm_ids::CONGRATULATIONS_YOUR_RAID_WAS_SUCCESSFUL,
                &[],
            ),
        );
    }

    // Party members within range of the corpse, else the earner alone. In a
    // command channel the split spans the whole channel (Java line 452).
    let range = world.cfg.character.alt_party_range as f64;
    let earner_party = world
        .objects
        .get_component::<crate::model::components::PartyRef>(&earner_oid)
        .map(|r| r.0);
    let group: Option<Vec<i32>> =
        earner_party.map(|pid| crate::game_loop::command_channel::cc_or_party_members(world, pid));
    let members: Vec<i32> = match group {
        Some(g) => g
            .into_iter()
            .filter(|m| in_range_of(world, npc_oid, *m, range))
            .collect(),
        None => vec![earner_oid],
    };
    if members.is_empty() {
        return;
    }
    // `Math.max(points / size, 1)` — a split never rounds anyone down to zero.
    let each = (total / members.len() as i32).max(1);
    for m in members {
        if let Some(p) = world.objects.get_component_mut::<crate::model::Player>(&m) {
            p.raidboss_points += each;
        }
        send_sm_to_player(
            world,
            m,
            sm_ids::YOU_HAVE_EARNED_S1_RAID_POINTS,
            &[SmParam::Int(each)],
        );
    }
}
