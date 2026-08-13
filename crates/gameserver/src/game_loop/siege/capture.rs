//! The ownership change when a siege is won: `capture`, the post-siege
//! bookkeeping (blood alliance, hero diary, ticket reset, hour-window reopen),
//! and the battlefield teleports.

use super::*;
/// Java `Siege.saveCastleSiege()`'s registration half: when a siege ends, the
/// castle's owner gets **24 hours** to pick the next siege's hour, so
/// `regTimeEnd` is stamped `now + 1 day` and `regTimeOver` reopens.
///
/// Java also calls `setNextSiegeDate()` here to push the next siege two weeks
/// out. That is deliberately not ported: this dist drives the date from
/// `SiegeSchedule.xml` (weekday + hour per castle), which
/// [`effective_siege_millis`] already reads — a stored "two weeks from now"
/// would fight the schedule rather than agree with it.
///
/// This is the half that makes hour-picking reachable at all. `regTimeOver`
/// defaults to `true` (closed) on a fresh castle row, so before this the window
/// never opened on its own and `RequestSetCastleSiegeTime` stayed dormant.
pub(super) fn reopen_time_registration(world: &mut World, castle_id: i32) {
    const ONE_DAY_MS: i64 = 86_400_000;
    let end = commons::util::now_millis() + ONE_DAY_MS;
    let Some(c) = world.castle_mut(castle_id) else {
        return;
    };
    c.siege_time_registration_end = end;
    c.time_registration_over = false;
    let siege_date = c.siege_date;
    let _ = world.db.send(DbCommand::UpdateCastleSiegeTime {
        castle_id,
        siege_date,
        time_registration_over: false,
        siege_time_registration_end: Some(end),
    });
}

/// `SiegeManager.getBloodAllianceReward()` — `Siege.ini BloodAllianceReward = 0`
/// on this dist, so holding a castle awards nothing in Interlude Classic. Kept
/// as the single knob: raising it lights up the whole [`increase_blood_alliance`]
/// path without any other change.
pub(crate) const BLOOD_ALLIANCE_REWARD: i32 = 0;

/// Java `Clan.increaseBloodAllianceCount` — the owner held its castle through
/// the siege, so bump (and persist) its blood-alliance count by the reward.
pub(super) fn increase_blood_alliance(world: &mut World, clan_id: i32) {
    let Some(clan) = world.clans.get_mut(&clan_id) else {
        return;
    };
    clan.blood_alliance_count += BLOOD_ALLIANCE_REWARD;
    let count = clan.blood_alliance_count;
    let _ = world
        .db
        .send(DbCommand::UpdateClanBloodAlliance { clan_id, count });
}

/// Java `Castle.setTicketBuyCount(0)` — the castle changed hands, so the former
/// owner's placed-mercenary count is cleared. A no-op (and no DB write) when it
/// was already 0, which it always is until the mercenary system lands.
pub(super) fn reset_castle_ticket_count(world: &mut World, castle_id: i32) {
    let Some(castle) = world.castle_mut(castle_id) else {
        return;
    };
    if castle.ticket_buy_count == 0 {
        return;
    }
    castle.ticket_buy_count = 0;
    let _ = world.db.send(DbCommand::UpdateCastleTicketCount {
        castle_id,
        count: 0,
    });
}

/// `Hero.ACTION_CASTLE_TAKEN`.
const HERO_ACTION_CASTLE_TAKEN: i32 = 3;

/// Java `endSiege`'s `Hero.setCastleTaken` loop: every online **noble** member
/// of the capturing clan gets a `heroes_diary` "castle taken" entry (the
/// hero-eligibility record). A player's object id is their character id, so it
/// keys the diary row directly. The in-memory hero-diary display (only for a
/// currently-crowned hero) isn't modelled, so only the persistent row is written.
pub(super) fn record_castle_taken_for_nobles(world: &mut World, clan_id: i32, castle_id: i32) {
    let now = commons::util::now_millis();
    let nobles: Vec<i32> = crate::game_loop::clans::online_members(world, clan_id)
        .into_iter()
        .filter(|oid| {
            world
                .objects
                .get_component::<Player>(oid)
                .is_some_and(|p| p.is_noble)
        })
        .collect();
    for char_id in nobles {
        let _ = world.db.send(DbCommand::SaveHeroDiary {
            char_id,
            time: now,
            action: HERO_ACTION_CASTLE_TAKEN,
            param: castle_id,
        });
    }
}

/// Java `Castle.setOwner` (from the throne-room artifact) + `Siege.midVictory`
/// core: an attacker captures the castle mid-siege. Ownership transfers to
/// `new_clan_id`; the old owner/defenders become attackers and the captor
/// becomes the OWNER defender.
///
/// Reached in production: the Holy Artifact (type `Artefact`, e.g. Gludio's
/// 35063) is a permanent castle spawn, so an attacker touching it during an
/// active siege calls [`try_capture_artifact`] → here.
///
/// Castle crests are modelled end to end even though the display is inert on
/// this dist: `Npc.onSpawn`'s tax-zone `setClanId` lives in
/// `spawn_npc_entity` (gated on `ShowCrestWithoutQuest ||
/// castle.show_npc_crest` — `NPC.ini` ships the former `False`, the DB column
/// defaults `'false'`, and `setShowNpcCrest(true)` appears nowhere in the
/// Java tree, so only an operator flipping one of them shows anything),
/// `NpcInfo` carries the CLAN component (`visibility::npc_clan_block`), and
/// every ownership change resets the castle flag to false exactly like
/// `Castle.setOwner`.
///
/// (`Castle.removeUpgrade()` needs nothing: castle upgrades — the door/trap
/// tiers bought from the chamberlain — are not modelled at all, so there is
/// nothing to strip.)
/// `Castle.setShowNpcCrest` — flip the flag and persist it when it changes.
pub(crate) fn set_show_npc_crest(world: &mut World, castle_id: i32, show: bool) {
    if let Some(c) = world.castle_mut(castle_id)
        && c.show_npc_crest != show
    {
        c.show_npc_crest = show;
        let _ = world
            .db
            .send(DbCommand::UpdateCastleShowNpcCrest { castle_id, show });
    }
}

pub(crate) fn capture(world: &mut World, castle_id: i32, new_clan_id: i32) {
    if !world.sieges.get(&castle_id).is_some_and(|s| s.in_progress) {
        return;
    }
    let old_owner = owner_clan_id_opt(world, castle_id);
    // Transfer ownership: the old owner loses `hasCastle`, the captor gains it.
    if let Some(old) = old_owner {
        if let Some(c) = world.clans.get_mut(&old) {
            c.castle_id = 0;
        }
        let _ = world.db.send(DbCommand::UpdateClanCastle {
            clan_id: old,
            castle_id: 0,
        });
    }
    if let Some(c) = world.clans.get_mut(&new_clan_id) {
        c.castle_id = castle_id;
    }
    let _ = world.db.send(DbCommand::UpdateClanCastle {
        clan_id: new_clan_id,
        castle_id,
    });
    // `Castle.setOwner` → `setShowNpcCrest(false)`: a change of hands always
    // resets the crest display.
    set_show_npc_crest(world, castle_id, false);

    // Java `Castle.setOwner`: strip the castle's residential skills from the
    // former owner's online members, and grant them to the captor's.
    if let Some(old) = old_owner {
        crate::game_loop::clans::strip_residential_skills_from_clan(world, old, castle_id);
        // …and take back this castle's circlet (`RemoveCastleCirclets`, True
        // on this dist). Java runs it once, guarded by `_formerOwner == null`,
        // so a second mid-siege flip does not strip the *new* former owner
        // twice — here `capture` is the only caller, so once per flip.
        crate::game_loop::castle::remove_circlets_from_clan(world, old, castle_id);
    }
    crate::game_loop::clans::grant_residential_skills_to_clan(world, new_clan_id, castle_id);

    // Reshuffle siege roles: every other side becomes an attacker, the captor
    // becomes the OWNER; then re-persist the changed rows.
    let changed: Vec<(i32, i32)> = match world.sieges.get_mut(&castle_id) {
        Some(siege) => {
            for sc in siege.clans.iter_mut() {
                if sc.clan_id != new_clan_id
                    && matches!(
                        sc.kind,
                        SiegeClanType::Owner
                            | SiegeClanType::Defender
                            | SiegeClanType::DefenderPending
                    )
                {
                    sc.kind = SiegeClanType::Attacker;
                }
            }
            match siege.clans.iter_mut().find(|c| c.clan_id == new_clan_id) {
                Some(sc) => sc.kind = SiegeClanType::Owner,
                None => siege.add_clan(new_clan_id, SiegeClanType::Owner),
            }
            siege
                .clans
                .iter()
                .map(|c| (c.clan_id, c.kind.as_db()))
                .collect()
        }
        None => Vec::new(),
    };
    for (clan_id, kind) in changed {
        let _ = world.db.send(DbCommand::SaveSiegeClan {
            castle_id,
            clan_id,
            kind,
        });
    }

    // `_castle.setFirstMidVictory(true)` — the castle has now been engraved
    // once, which is what finally lets two *attacker* clans fight each other
    // (`isAutoAttackable`'s siege block reads this).
    if let Some(c) = world.castle_mut(castle_id) {
        c.first_mid_victory = true;
    }
    // `teleportPlayer(Attacker, SIEGEFLAG)` — the *new* attackers (the clans
    // that were defending a moment ago) are thrown out of the castle. Java
    // aims them at their siege flag, which an ex-defender has never had, so
    // `getTeleToLocation` falls through to the town respawn either way.
    teleport_side_out(world, castle_id, SiegeClanType::Attacker);

    // `removeDefenderFlags()` — run *after* the reshuffle, so it strips the
    // **captor's** own base camp: you do not keep a siege HQ once the castle is
    // yours.
    remove_flags_of_defenders(world, castle_id);

    // `_castle.spawnDoor(true)` — respawn the (now the captor's) gates at 50% HP.
    spawn_castle_doors(world, castle_id, true);

    // `removeTowers()` then `spawnControlTower()`/`spawnFlameTower()` with
    // `_controlTowerCount = 0` in between — "each new siege midvictory CT are
    // completely respawned". The count reset matters: without it the respawn
    // would add to a stale count and the guardian-tower resurrection message
    // would never fire again.
    respawn_siege_towers(world, castle_id);

    // `updatePlayerSiegeStateFlags(false)` — every side just changed, so every
    // member's icon and attackability has to be re-pushed.
    update_player_siege_state_flags(world, castle_id, false);
}

/// `removeDefenderFlags()` — drop every HQ flag belonging to a clan currently
/// registered as a defender (owner included), despawning the flag NPC with it.
fn remove_flags_of_defenders(world: &mut World, castle_id: i32) {
    let doomed: Vec<(i32, i32)> = match world.sieges.get(&castle_id) {
        Some(siege) => siege
            .flags
            .iter()
            .filter(|(clan_id, _)| siege.is_defender(*clan_id))
            .copied()
            .collect(),
        None => Vec::new(),
    };
    for (clan_id, flag_oid) in doomed {
        if let Some(region) = region_cell_of(world, flag_oid) {
            crate::game_loop::death::despawn_npc(world, flag_oid, region);
        }
        if let Some(siege) = world.sieges.get_mut(&castle_id) {
            siege
                .flags
                .retain(|(owner, oid)| !(*owner == clan_id && *oid == flag_oid));
        }
    }
}

/// `removeTowers()` + `spawnControlTower()` / `spawnFlameTower()`: tear down the
/// castle's control and flame towers and put a fresh set up for the new owner.
///
/// Only the *towers* are recycled — the stationed siege guards are left alone,
/// matching Java (`midVictory` never touches `spawnSiegeGuard`).
fn respawn_siege_towers(world: &mut World, castle_id: i32) {
    let towers: Vec<i32> = world
        .sieges
        .get(&castle_id)
        .map(|s| {
            s.spawned_npcs
                .iter()
                .copied()
                .filter(|&oid| is_siege_tower(world, oid))
                .collect()
        })
        .unwrap_or_default();
    for oid in towers {
        if let Some(region) = region_cell_of(world, oid) {
            crate::game_loop::death::despawn_npc(world, oid, region);
        }
        if let Some(siege) = world.sieges.get_mut(&castle_id) {
            siege.spawned_npcs.retain(|&o| o != oid);
        }
    }
    if let Some(siege) = world.sieges.get_mut(&castle_id) {
        siege.control_tower_count = 0;
    }
    let spawns = world
        .data
        .siege_towers
        .get(&castle_id)
        .cloned()
        .unwrap_or_default();
    spawn_siege_npcs(world, castle_id, &spawns);
}

/// Send each player home to their own race's town respawn — Java
/// `Siege.teleportPlayer(…, TeleportWhereType.TOWN)`, used both when a side is
/// unregistered mid-siege and when the zone is cleared at the end.
///
/// Anyone who has left the world in the meantime is skipped, and a race with
/// no respawn entry falls back to Human as the port does elsewhere.
pub(super) fn teleport_all_to_town(world: &mut World, targets: Vec<i32>) {
    for oid in targets {
        crate::game_loop::death::teleport_to_town(world, oid, 0);
    }
}

/// A control or flame tower, by template type.
fn is_siege_tower(world: &World, npc_oid: i32) -> bool {
    npc_template(world, npc_oid)
        .is_some_and(|t| matches!(t.type_name.as_str(), "ControlTower" | "FlameTower"))
}

/// `teleportPlayer(<side>, …)` — evict every online member of the clans
/// registered on one side of the siege.
fn teleport_side_out(world: &mut World, castle_id: i32, side: SiegeClanType) {
    let clans: Vec<i32> = match world.sieges.get(&castle_id) {
        Some(siege) => siege
            .clans
            .iter()
            .filter(|c| c.kind == side)
            .map(|c| c.clan_id)
            .collect(),
        None => Vec::new(),
    };
    if clans.is_empty() {
        return;
    }
    let targets: Vec<i32> = world
        .in_game_player_oids()
        .filter(|&oid| {
            world
                .objects
                .get_component::<Player>(&oid)
                .is_some_and(|p| clans.contains(&p.clan_id) && !p.is_gm(&world.data))
        })
        .collect();
    teleport_all_to_town(world, targets);
}

#[cfg(test)]
pub(crate) fn spawn_towers_for_test(world: &mut World, castle_id: i32) {
    let towers = world
        .data
        .siege_towers
        .get(&castle_id)
        .cloned()
        .unwrap_or_default();
    spawn_siege_npcs(world, castle_id, &towers);
}
