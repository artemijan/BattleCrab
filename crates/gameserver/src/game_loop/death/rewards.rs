use super::add_exp_and_sp;
use super::award_raid_points;
use super::consume_kill_vitality;
use super::overhit_bonus;
use crate::data::item_data::ADENA_ID;
use crate::data::npc_data::DropHolder;
use crate::data::npc_data::NpcTemplate;
use crate::game_loop::ground_items::reserve_for;
use crate::game_loop::guard::clan_of_or_zero;
use crate::game_loop::guard::maybe_position;
use crate::game_loop::helpers::client_for_player;
use crate::game_loop::helpers::region_cell_of;
use crate::game_loop::helpers::{send_sm_to_client, send_to_client};
use crate::model::Player;
use crate::model::components::Position;
use crate::model::components::Vitals;
use crate::model::formulas;
use crate::network::server_packets;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
use crate::world::regions_adjacent;

/// XP/SP shares from the aggro list + drops to the top damage dealer.
/// Party members pool shares and split via `Party.distributeXpAndSp` (G10).
/// Narrowings: no overhit bonus (no overhit skills), no raid points, no
/// champion mods, no command channels.
pub(crate) fn calculate_rewards(world: &mut World, npc_oid: i32, killer_oid: i32) {
    let Some(npc) = world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
    else {
        return;
    };
    let Some(t) = npc.template(world).cloned() else {
        return;
    };
    let Some(npc_region) = region_cell_of(world, npc_oid) else {
        return;
    };
    let (nx, ny, nz) = {
        let Some(pos) = world.objects.get_component::<Position>(&npc_oid) else {
            return;
        };
        (pos.x, pos.y, pos.z)
    };
    // `Config.CHAMPION_ENABLE && _champion` — multiplies the drops below and
    // the exp/sp further down, and decides `useVitalityRate()`, which gates
    // whether this kill charges vitality and pays PA points at all.
    let is_champion = world.cfg.champion.enable
        && world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .is_some_and(|n| n.champion);
    let use_vitality_rate = world.cfg.champion.uses_vitality_rate(is_champion);

    // Damage shares (players only, > 1 damage, still within reward range —
    // `Util.checkIfInRange(ALT_PARTY_RANGE, …)`).
    let reward_range = world.cfg.character.alt_party_range as f64;
    let mut shares: Vec<(i32, f64)> = Vec::new();
    let mut total_damage = 0.0;
    let mut max_dealer: Option<(i32, f64)> = None;
    // Java `calculateRewards`: `info.getAttacker().getActingPlayer()` — every
    // damage dealer is resolved to the player behind it, so a **summon's**
    // damage counts for its owner. Without this a summoner's pet did all the
    // work and the owner earned nothing.
    //
    // Collected first because resolving borrows the store while the aggro list
    // is still borrowed.
    let entries: Vec<(i32, f64)> = {
        let Some(aggro) = world
            .objects
            .get_component::<crate::model::npc::AggroList>(&npc_oid)
        else {
            return;
        };
        aggro
            .0
            .iter()
            .map(|(&oid, info)| (oid, info.damage))
            .collect()
    };
    for (dealer_oid, damage) in entries {
        let player_oid = crate::game_loop::pvp::acting_player(world, dealer_oid);
        // Only players earn. A summon resolves to its owner; a mob to itself,
        // which is not a player and so is skipped as before.
        if !world.objects.has_component::<Player>(&player_oid) {
            continue;
        }
        // Range is measured from the **earner**, as Java does — a pet fighting
        // out of its owner's reward range earns them nothing.
        let Some(ppos) = world.objects.get_component::<Position>(&player_oid) else {
            continue;
        };
        if damage <= 1.0 {
            continue;
        }
        let dist = (((ppos.x - nx) as f64).powi(2) + ((ppos.y - ny) as f64).powi(2)).sqrt();
        if dist > reward_range {
            continue;
        }
        total_damage += damage;
        // A player who both hit the mob and had a pet hit it appears twice;
        // merge rather than double-counting them in the share split.
        if let Some(existing) = shares.iter_mut().find(|(id, _)| *id == player_oid) {
            existing.1 += damage;
        } else {
            shares.push((player_oid, damage));
        }
        let merged = shares
            .iter()
            .find(|(id, _)| *id == player_oid)
            .map(|(_, d)| *d)
            .unwrap_or(damage);
        if max_dealer.is_none_or(|(_, d)| merged > d) {
            max_dealer = Some((player_oid, merged));
        }
    }

    // Drops go to the top damage dealer (fall back to the killer); a looter
    // in a party routes every item through `Party.distributeItem`
    // (`Player.doAutoLoot`).
    // The killer fallback resolves too: a pet's killing blow loots for its
    // owner.
    let killer_oid = crate::game_loop::pvp::acting_player(world, killer_oid);

    // Raid points go to the same player the drops do — Java's
    // `maxDealer != null && isOnline() ? maxDealer : lastAttacker`.
    if let Some(earner) = max_dealer.map(|(id, _)| id).or(Some(killer_oid)) {
        award_raid_points(world, npc_oid, earner);
    }
    let looter = max_dealer.map(|(id, _)| id).or_else(|| {
        world
            .objects
            .has_component::<Player>(&killer_oid)
            .then_some(killer_oid)
    });
    if let Some(looter) = looter {
        // `doItemDrop`: a mob that died spoiled rolls its `<spoil>` list into
        // the sweep loot (`DropType.SPOIL`), stashed on the corpse until a
        // `Sweeper` cast claims it. The level-gap gate uses the same main
        // damage dealer as the death drops (Java passes the same `player`).
        let spoiled = world
            .objects
            .get_component::<crate::model::npc::Npc>(&npc_oid)
            .map(|n| n.spoiler_object_id != 0)
            .unwrap_or(false);
        if spoiled {
            let sweep = roll_spoil_drops(world, &t, looter);
            if let Some(npc) = world
                .objects
                .get_component_mut::<crate::model::npc::Npc>(&npc_oid)
            {
                // `_sweepItems.set(...)`: `null`/empty → `isSweepActive()` false.
                npc.sweep_items = if sweep.is_empty() { None } else { Some(sweep) };
            }
        }

        // `Chest.doItemDrop` — a chest that was **not** unlocked rolls a
        // *different* npc id's drop list (18265-18286 shift by +3536, the
        // 18287-18298 pairs map onto six fixed ids). So smashing a box open
        // and picking its lock are two different loot tables, and only the
        // Unlock path (`setSpecialDrop`) gets the box's own.
        let drop_template = chest_drop_template(world, npc_oid, &t);
        let drops = roll_drops(
            world,
            drop_template.as_ref().unwrap_or(&t),
            looter,
            is_champion,
        );
        let party_id = world
            .objects
            .get_component::<crate::model::components::PartyRef>(&looter)
            .map(|r| r.0);
        // A raid's drops follow `AutoLootRaids` (off on this dist — they hit
        // the ground even though `AutoLoot` is on), everything else `AutoLoot`
        // (Java `Attackable.doItemDrop`).
        let is_raid = crate::game_loop::raid_curse::gives_raid_curse(world, npc_oid);
        // Loot protection (`ItemData.createItem("loot")`): a raid drop is
        // owned by the privileged command channel's *leader* for
        // `RaidLootRightsInterval`; an ordinary ground drop by the killer for
        // 15 s. A raid without an active claim is owned by nobody.
        let (owner_id, protect_ticks) = if is_raid {
            match crate::game_loop::command_channel::loot_rights_cc(world, npc_oid)
                .and_then(|cc| world.command_channels.get(&cc))
            {
                Some(cc) => (
                    cc.leader,
                    world.cfg.character.raid_loot_rights_interval * 10,
                ),
                None => (0, 0),
            }
        } else {
            (looter, 150)
        };
        for (item_id, count) in drops {
            if !auto_loots(world, item_id, is_raid) {
                let ground_oid = crate::game_loop::ground_items::spawn_ground_item(
                    world,
                    item_id,
                    count,
                    0,
                    nx,
                    ny,
                    nz,
                    npc_oid,
                    crate::game_loop::ground_items::DropSource::Npc,
                );
                reserve_for(world, ground_oid, owner_id, protect_ticks);
                continue;
            }
            match party_id {
                Some(pid) => crate::game_loop::party::distribute_item(
                    world,
                    pid,
                    looter,
                    item_id,
                    count,
                    (nx, ny),
                ),
                None => give_item(world, looter, item_id, count),
            }
        }
    }

    if total_damage <= 0.0 {
        return;
    }
    // `Attackable.calculateRewards`: `if (!_mustRewardExpSp) return;` — an
    // unlocked chest hands out its loot but pays no exp or sp.
    if world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .is_some_and(|n| !n.must_reward_exp_sp)
    {
        return;
    }
    // `calculateExpAndSp` per attacker: template reward × rate × damage
    // share × level-gap multiplier. Attackers in a party pool their shares
    // once (the Java party branch); the rest reward solo.
    let (rate_xp, rate_sp) = (world.cfg.rates.rate_xp, world.cfg.rates.rate_sp);
    let champion_exp_sp = if is_champion {
        world.cfg.champion.rewards_exp_sp
    } else {
        1.0
    };
    let mut processed: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for &(player_oid, damage) in &shares {
        if processed.contains(&player_oid) {
            continue;
        }
        let party_id = world
            .objects
            .get_component::<crate::model::components::PartyRef>(&player_oid)
            .map(|r| r.0);
        let Some(party_id) = party_id else {
            // Solo branch (unchanged from G9).
            let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
                continue;
            };
            let Some(pregion) = region_cell_of(world, player_oid) else {
                continue;
            };
            if !regions_adjacent(npc_region, pregion) {
                continue; // Java `isInSurroundingRegion(attacker)`.
            }
            let player_level = p.level;
            let gap = formulas::exp_sp_level_gap_multiplier(player_level, t.level);
            let mut exp = (t.exp * rate_xp * damage / total_damage * gap).max(0.0);
            let mut sp = (t.sp * rate_sp * damage / total_damage * gap).max(0.0);
            // Java multiplies both by `CHAMPION_REWARDS_EXP_SP` here — before
            // the over-hit bonus and before the premium rates.
            exp *= champion_exp_sp;
            sp *= champion_exp_sp;
            // `Attackable.onKill`: the over-hit bonus rides on this attacker's
            // share, but only for whoever actually landed the killing blow.
            exp += overhit_bonus(world, npc_oid, player_oid, exp);
            // `Attackable.onKill`: premium rates apply *before* the vitality /
            // skill bonus multiplier `addExpAndSp` folds in.
            if crate::game_loop::admin::premium::has_premium_status(world, player_oid) {
                exp *= world.cfg.premium.rate_xp;
                sp *= world.cfg.premium.rate_sp;
            }
            add_exp_and_sp(world, player_oid, exp, sp, use_vitality_rate);
            // Java consumes vitality only when `exp > 0`, and keys the amount on
            // the *pre-bonus* exp — the same value it just handed to
            // `addExpAndSp`. A champion kill skips the whole block unless
            // `ChampionEnableVitality` (Java `useVitalityRate()`).
            if exp > 0.0 && use_vitality_rate {
                consume_kill_vitality(world, player_oid, player_level, &t, exp);
                // Java pairs the PA-point award with `updateVitalityPoints`
                // inside `if (useVitalityRate())` — but it is *not* behind
                // `Config.ENABLE_VITALITY`, which `consume_kill_vitality`
                // early-returns on, so it has to sit out here.
                crate::game_loop::pc_cafe::give_point(world, player_oid, exp);
            }
            continue;
        };

        // Party branch: pool every member's share; alive members within
        // `ALT_PARTY_RANGE` of the corpse are rewarded, the top rewarded
        // level keys the level-gap multiplier and the cutoff. In a command
        // channel the *whole channel* shares (Java `Attackable` line 621:
        // `isInCommandChannel() ? cc.getMembers() : party.getMembers()`).
        let cc_id = crate::game_loop::command_channel::cc_id_of_party(world, party_id);
        let members = crate::game_loop::command_channel::cc_or_party_members(world, party_id);
        let share_of: std::collections::HashMap<i32, f64> = shares.iter().copied().collect();
        let mut party_dmg = 0.0;
        let mut rewarded: Vec<(i32, i32)> = Vec::new();
        let mut party_lvl = 0;
        for &m in &members {
            let dead = world
                .objects
                .get_component::<Vitals>(&m)
                .map(|v| v.dead)
                .unwrap_or(true);
            if dead {
                continue; // their leftover share rewards nothing (Java parity)
            }
            if !crate::geo::distance::within_2d_xy(world, m, nx, ny, reward_range) {
                continue;
            }
            if let Some(&share) = share_of.get(&m) {
                party_dmg += share;
                processed.insert(m);
            }
            rewarded.push((
                m,
                world
                    .objects
                    .get_component::<Player>(&m)
                    .map(|p| p.level)
                    .unwrap_or(0),
            ));
            party_lvl = party_lvl.max(rewarded.last().unwrap().1);
        }
        // In a CC the level-gap key is the channel's level (its highest party
        // level), not the rewarded members' max (Java lines 642-646).
        if let Some(id) = cc_id
            && let Some(cc) = world.command_channels.get(&id)
        {
            party_lvl = cc.level;
        }
        processed.insert(player_oid);
        if party_dmg <= 0.0 || rewarded.is_empty() {
            continue;
        }
        // `calculateExpAndSp(partyLvl, partyDmg, totalDamage)` then
        // `exp *= partyMul` — Java applies the damage fraction twice when
        // outsiders contributed; kept for parity.
        let party_mul = if party_dmg < total_damage {
            party_dmg / total_damage
        } else {
            1.0
        };
        let gap = formulas::exp_sp_level_gap_multiplier(party_lvl, t.level);
        let exp = (t.exp * rate_xp * party_dmg / total_damage * gap).max(0.0)
            * party_mul
            * champion_exp_sp;
        let sp = (t.sp * rate_sp * party_dmg / total_damage * gap).max(0.0)
            * party_mul
            * champion_exp_sp;
        crate::game_loop::party::distribute_xp_and_sp(
            world,
            &rewarded,
            party_lvl,
            exp,
            sp,
            &t,
            use_vitality_rate,
        );
    }
}

/// `NpcTemplate.calculateDrops(DROP)` — grouped + ungrouped lists with the
/// level-gap gates, per-item rate multipliers, and the max-occurrence cap.
/// The temporary "replace highest-chance random drop" reshuffling Java does
/// when the cap is hit mid-list is simplified to a hard stop at the cap.
#[cfg(test)]
pub(crate) fn roll_spoil_drops_for_test(
    world: &mut World,
    t: &NpcTemplate,
    killer_oid: i32,
) -> Vec<(i32, i64)> {
    roll_spoil_drops(world, t, killer_oid)
}

#[cfg(test)]
pub(crate) fn roll_drops_for_test(
    world: &mut World,
    t: &NpcTemplate,
    killer_oid: i32,
) -> Vec<(i32, i64)> {
    roll_drops(world, t, killer_oid, false)
}

#[cfg(test)]
pub(crate) fn roll_champion_drops_for_test(
    world: &mut World,
    t: &NpcTemplate,
    killer_oid: i32,
) -> Vec<(i32, i64)> {
    roll_drops(world, t, killer_oid, true)
}

/// `Chest.doItemDrop`'s template swap. Returns `Some(other_template)` when
/// this corpse is a chest that was killed *without* being unlocked — every
/// other NPC (and every unlocked chest) drops from its own list.
/// Test hook for [`chest_drop_template`], which is private to this module.
#[cfg(test)]
pub(crate) fn chest_drop_template_for_test(
    world: &World,
    npc_oid: i32,
    t: &NpcTemplate,
) -> Option<NpcTemplate> {
    chest_drop_template(world, npc_oid, t)
}

fn chest_drop_template(world: &World, npc_oid: i32, t: &NpcTemplate) -> Option<NpcTemplate> {
    if t.type_name != "Chest" {
        return None;
    }
    if world
        .objects
        .get_component::<crate::model::npc::Npc>(&npc_oid)
        .is_some_and(|n| n.special_drop)
    {
        return None;
    }
    let id = match t.id {
        18265..=18286 => t.id + 3536,
        18287 | 18288 => 21671,
        18289 | 18290 => 21694,
        18291 | 18292 => 21717,
        18293 | 18294 => 21740,
        18295 | 18296 => 21763,
        18297 | 18298 => 21786,
        _ => return None,
    };
    world.data.npc_data.get(id).cloned()
}

/// Everything `calculateGroupDrops`/`calculateUngroupedDrops` read off the
/// world once, so the two loops below can take `&mut World` for their rolls
/// without re-borrowing config out of it per item.
struct DropCtx {
    killer_level: i32,
    victim_level: i32,
    adena_gap_chance: f64,
    item_gap_chance: f64,
    /// `victim.isRaid() ? DROP_MAX_OCCURRENCES_RAIDBOSS : DROP_MAX_OCCURRENCES_NORMAL`
    /// — **per list**: Java gives the grouped and ungrouped passes a counter
    /// each, so a template carrying both can pay out two random drops.
    occurrences: i32,
    is_raid: bool,
    champion: bool,
    premium: bool,
    chance_mult: f64,
    amount_mult: f64,
    herb_chance_mult: f64,
    herb_amount_mult: f64,
    raid_chance_mult: f64,
    raid_amount_mult: f64,
    by_id_chance: std::collections::HashMap<i32, f64>,
    by_id_amount: std::collections::HashMap<i32, f64>,
    champion_cfg: crate::config::ChampionConfig,
}

impl DropCtx {
    fn gap_chance(&self, item_id: i32) -> f64 {
        if item_id == ADENA_ID {
            self.adena_gap_chance
        } else {
            self.item_gap_chance
        }
    }
}

/// `calculateGroupDrop`/`calculateUngroupedDrop`'s chance chain, in Java's
/// order: the per-item override first (with its champion-adena rider), then
/// herb, then raid, then the flat death rate times the champion rider.
///
/// **The champion multiplier lives in two different arms.**
/// `CHAMPION_ADENAS_REWARDS_CHANCE` only fires inside the per-item
/// `RATE_DROP_CHANCE_BY_ID` branch (so a server with no per-id adena rate never
/// sees it), while `CHAMPION_REWARDS_CHANCE` rides the flat death-drop rate in
/// the `else`. Collapsing them to one multiplier would silently change the
/// payout on this dist, where adena *does* carry a per-id rate.
fn death_rate_chance(world: &World, ctx: &DropCtx, item_id: i32) -> f64 {
    let mut rate = match ctx.by_id_chance.get(&item_id) {
        Some(&by_id) => {
            if ctx.champion && item_id == ADENA_ID {
                by_id * ctx.champion_cfg.adenas_rewards_chance
            } else {
                by_id
            }
        }
        None if is_herb(world, item_id) => ctx.herb_chance_mult,
        None if ctx.is_raid => ctx.raid_chance_mult,
        None => {
            ctx.chance_mult
                * if ctx.champion {
                    ctx.champion_cfg.rewards_chance
                } else {
                    1.0
                }
        }
    };
    if ctx.premium {
        rate *= premium_drop_mult(world, item_id, ctx.is_raid, PremiumDropRate::Chance);
    }
    // Java also folds `Stat.BONUS_DROP_RATE` in here for a player killer. No
    // skill, item or option on this dist declares `bonusDropRate` (nor
    // `bonusDropAmount`/`bonusDropAdena`/`bonusSpoilRate`), so the term is a
    // fixed 1.0 and is left out rather than plumbed to nothing.
    rate
}

/// The amount half of the same chain.
fn death_rate_amount(world: &World, ctx: &DropCtx, item_id: i32) -> f64 {
    let mut rate = match ctx.by_id_amount.get(&item_id) {
        Some(&by_id) => {
            if ctx.champion && item_id == ADENA_ID {
                by_id * ctx.champion_cfg.adenas_rewards_amount
            } else {
                by_id
            }
        }
        None if is_herb(world, item_id) => ctx.herb_amount_mult,
        None if ctx.is_raid => ctx.raid_amount_mult,
        None => {
            ctx.amount_mult
                * if ctx.champion {
                    ctx.champion_cfg.rewards_amount
                } else {
                    1.0
                }
        }
    };
    if ctx.premium {
        rate *= premium_drop_mult(world, item_id, ctx.is_raid, PremiumDropRate::Amount);
    }
    rate
}

fn is_herb(world: &World, item_id: i32) -> bool {
    world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|i| i.ex_immediate_effect)
}

/// The occurrence budget's bookkeeping, which is the part of Java's drop
/// algorithm that reads like a bug and is load-bearing anyway.
///
/// `DropMaxOccurrences*` is 1 on this dist, so at most one *random* line pays
/// out per list. Java does not simply stop at the cap: when the budget is spent
/// and another random line comes up, it **takes the earliest random drop back
/// out** of the results (`randomDrops.remove(0)`, and the same holder out of
/// `calculatedDrops`), parks it in `cachedItem` and gives the counter one more
/// unit — so the *last* eligible line wins, not the first. If nothing replaces
/// the parked drop, it is added back at the end.
///
/// The port used to `continue` at the cap instead, which kept the first line
/// and dropped the rest.
#[derive(Default)]
struct OccurrenceBudget {
    counter: i32,
    calculated: Vec<(i32, i64)>,
    random: Vec<(i32, i64)>,
    cached: Option<(i32, i64)>,
}

impl OccurrenceBudget {
    fn new(counter: i32) -> Self {
        Self {
            counter,
            ..Default::default()
        }
    }

    /// Java's `if ((dropOccurrenceCounter == 0) && (chance < 100) && …)` block.
    /// `evict` is Java's `rateChance == 1` guard on the grouped path; the
    /// ungrouped path evicts unconditionally.
    fn make_room(&mut self, chance: f64, evict: bool) {
        if self.counter != 0
            || chance >= 100.0
            || self.random.is_empty()
            || self.calculated.is_empty()
        {
            return;
        }
        if evict {
            let cached = self.random.remove(0);
            if let Some(pos) = self.calculated.iter().position(|&item| item == cached) {
                self.calculated.remove(pos);
            }
            self.cached = Some(cached);
        }
        self.counter = 1;
    }

    /// The post-drop bookkeeping: a line whose **computed** chance is under
    /// 100 % spends a unit of the budget and becomes evictable. Java tests the
    /// computed chance times the per-item override *again* on the by-id arm —
    /// double-counting a multiplier it already applied — and this reproduces
    /// that rather than the intent.
    fn record(
        &mut self,
        ctx: &DropCtx,
        item_id: i32,
        chance: f64,
        item: (i32, i64),
        evictable: bool,
    ) {
        let under_100 = match ctx.by_id_chance.get(&item_id) {
            Some(&by_id) => chance * by_id < 100.0,
            None => chance < 100.0,
        };
        if under_100 {
            self.counter -= 1;
            if evictable {
                self.random.push(item);
            }
        }
        self.calculated.push(item);
    }

    /// `if ((dropOccurrenceCounter > 0) && (cachedItem != null)) calculatedDrops.add(cachedItem);`
    fn finish(mut self) -> Vec<(i32, i64)> {
        if self.counter > 0
            && let Some(cached) = self.cached
        {
            self.calculated.push(cached);
        }
        self.calculated
    }
}

/// `calculateGroupDrops`. Each `<group>` is a roulette, not a set of
/// independent lines: at x1 rates the group's own chances **accumulate**
/// (`totalChance += dropItem.getChance()`) so each successive line is tested
/// against the running total, and the group `break`s after its first payout.
/// The port used to roll every line of every group independently at
/// `line.chance × group.chance / 100`, which both pays out too often and
/// weights the lines wrongly. All 17 templates with groups on this dist are
/// the Spiked Stakato Nest mobs (22105–22121), 46 groups between them.
fn calculate_group_drops(world: &mut World, t: &NpcTemplate, ctx: &DropCtx) -> Vec<(i32, i64)> {
    let mut budget = OccurrenceBudget::new(ctx.occurrences);
    if ctx.occurrences > 0 {
        for group in &t.drop_groups {
            let mut total_chance = 0.0;
            for drop in &group.items {
                let rate_chance = death_rate_chance(world, ctx, drop.item_id);
                // "only use total chance on x1, custom rates break this logic
                // because total chance is more than 100%"
                if rate_chance == 1.0 {
                    total_chance += drop.chance;
                } else {
                    total_chance = drop.chance;
                }
                let group_item_chance = total_chance * (group.chance / 100.0) * rate_chance;
                budget.make_room(group_item_chance, rate_chance == 1.0);
                if world.roll_f64() * 100.0 > ctx.gap_chance(drop.item_id) {
                    continue;
                }
                if world.roll_f64() * 100.0 >= group_item_chance {
                    continue;
                }
                let rate_amount = death_rate_amount(world, ctx, drop.item_id);
                let item = (drop.item_id, rolled_count(world, drop, rate_amount));
                budget.record(
                    ctx,
                    drop.item_id,
                    group_item_chance,
                    item,
                    rate_chance == 1.0,
                );
                if rate_chance == 1.0 {
                    break;
                }
            }
        }
    }
    let mut out = budget.finish();
    champion_reward_items(world, ctx, &mut out);
    out
}

/// `calculateUngroupedDrops`, for both `<drop>` (death) and `<spoil>`: one
/// independent roll per line, with the same occurrence bookkeeping.
///
/// The `SPOIL` arm of `calculateUngroupedDrop` seeds its rates from the spoil
/// multipliers and reads **no** per-item `RATE_DROP_*_BY_ID` override — but
/// note the occurrence test in the enclosing loop consults that map either way,
/// because Java's lookup there is not drop-type aware.
fn calculate_ungrouped_drops(
    world: &mut World,
    list: &[DropHolder],
    spoil: bool,
    ctx: &DropCtx,
) -> Vec<(i32, i64)> {
    let mut budget = OccurrenceBudget::new(ctx.occurrences);
    let (spoil_chance_mult, spoil_amount_mult) = if spoil {
        let r = &world.cfg.rates;
        let (mut chance, mut amount) = (
            r.spoil_drop_chance_multiplier,
            r.spoil_drop_amount_multiplier,
        );
        if ctx.premium {
            chance *= world.cfg.premium.rate_spoil_chance;
            amount *= world.cfg.premium.rate_spoil_amount;
        }
        (chance, amount)
    } else {
        (1.0, 1.0)
    };
    if ctx.occurrences > 0 {
        for drop in list {
            budget.make_room(drop.chance, true);
            if world.roll_f64() * 100.0 > ctx.gap_chance(drop.item_id) {
                continue;
            }
            let rate_chance = if spoil {
                spoil_chance_mult
            } else {
                death_rate_chance(world, ctx, drop.item_id)
            };
            if world.roll_f64() * 100.0 >= drop.chance * rate_chance {
                continue;
            }
            let rate_amount = if spoil {
                spoil_amount_mult
            } else {
                death_rate_amount(world, ctx, drop.item_id)
            };
            let item = (drop.item_id, rolled_count(world, drop, rate_amount));
            budget.record(ctx, drop.item_id, drop.chance, item, true);
        }
    }
    let mut out = budget.finish();
    if !spoil {
        champion_reward_items(world, ctx, &mut out);
    }
    out
}

/// `calculateDrops`' champion tail, which lives at the end of **each** of the
/// two functions above — so a template carrying both a group list and an
/// ungrouped one runs it twice, and the level-suppression roll with it.
///
/// The guard is Java's verbatim `if (!calculatedDrops.containsAll(ITEMS))
/// calculatedDrops.addAll(ITEMS)` — an **all-or-nothing** test on the whole
/// configured list, not a per-item one, and `ItemHolder` compares id *and*
/// count. So a champion that happened to roll `(6393, 1)` from its own drop
/// list adds nothing, while one that rolled only *some* of a multi-item reward
/// list gets the entire list appended, duplicating the one it already had.
/// Faithful to the quirk rather than to the intent.
///
/// The suppression roll only happens when the levels actually differ: Java's
/// two `if`s are `victim < killer` and `victim > killer`, so an equal-level
/// champion consumes no roll at all.
fn champion_reward_items(world: &mut World, ctx: &DropCtx, out: &mut Vec<(i32, i64)>) {
    if !ctx.champion {
        return;
    }
    if ctx.victim_level != ctx.killer_level {
        let roll = world.roll(100);
        if ctx
            .champion_cfg
            .suppresses_reward_items(ctx.victim_level, ctx.killer_level, roll)
        {
            return;
        }
    }
    let contains_all = ctx
        .champion_cfg
        .reward_items
        .iter()
        .all(|reward| out.contains(reward));
    if !contains_all {
        out.extend_from_slice(&ctx.champion_cfg.reward_items);
    }
}

/// `NpcTemplate.calculateDrops(DropType.DROP)` — the grouped pass, then the
/// ungrouped one, each with its own occurrence budget, exactly as Java
/// concatenates `groupDrops` and `ungroupedDrops`.
fn roll_drops(
    world: &mut World,
    t: &NpcTemplate,
    killer_oid: i32,
    champion: bool,
) -> Vec<(i32, i64)> {
    let Some(ctx) = drop_ctx(world, t, killer_oid, champion) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if !t.drop_groups.is_empty() {
        out.extend(calculate_group_drops(world, t, &ctx));
    }
    if !t.drop_list_death.is_empty() {
        let list = t.drop_list_death.clone();
        out.extend(calculate_ungrouped_drops(world, &list, false, &ctx));
    }
    out
}

fn drop_ctx(
    world: &mut World,
    t: &NpcTemplate,
    killer_oid: i32,
    champion: bool,
) -> Option<DropCtx> {
    let killer_level = world
        .objects
        .get_component::<Player>(&killer_oid)
        .map(|p| p.level)?;
    let level_diff = (t.level - killer_level) as f64;
    let r = &world.cfg.rates;
    let adena_gap_chance = formulas::map_range(
        level_diff,
        -(r.drop_adena_max_level_difference as f64),
        -(r.drop_adena_min_level_difference as f64),
        r.drop_adena_min_level_gap_chance,
        100.0,
    );
    let item_gap_chance = formulas::map_range(
        level_diff,
        -(r.drop_item_max_level_difference as f64),
        -(r.drop_item_min_level_difference as f64),
        r.drop_item_min_level_gap_chance,
        100.0,
    );
    let is_raid = t.is_raid();
    let ctx = DropCtx {
        killer_level,
        victim_level: t.level,
        adena_gap_chance,
        item_gap_chance,
        occurrences: if is_raid {
            r.drop_max_occurrences_raidboss
        } else {
            r.drop_max_occurrences_normal
        },
        is_raid,
        champion,
        premium: world.cfg.premium.enabled
            && crate::game_loop::admin::premium::has_premium_status(world, killer_oid),
        chance_mult: r.death_drop_chance_multiplier,
        amount_mult: r.death_drop_amount_multiplier,
        herb_chance_mult: r.herb_drop_chance_multiplier,
        herb_amount_mult: r.herb_drop_amount_multiplier,
        raid_chance_mult: r.raid_drop_chance_multiplier,
        raid_amount_mult: r.raid_drop_amount_multiplier,
        by_id_chance: r.drop_chance_by_id.clone(),
        by_id_amount: r.drop_amount_by_id.clone(),
        champion_cfg: world.cfg.champion.clone(),
    };
    Some(ctx)
}

/// The amount half of `calculateGroupDrop`/`calculateUngroupedDrop`: a uniform
/// pick across the line's `[min, max]` (a one-sided line skips the roll), scaled
/// by whichever amount rate the caller assembled. Rounded, then floored at 1, so
/// a rate small enough to round the count away still drops a single item.
fn rolled_count(world: &mut World, drop: &DropHolder, rate_amount: f64) -> i64 {
    let base = if drop.max > drop.min {
        drop.min + world.roll((drop.max - drop.min + 1) as i32) as i64
    } else {
        drop.min
    };
    ((base as f64) * rate_amount).round().max(1.0) as i64
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PremiumDropRate {
    Chance,
    Amount,
}

/// The premium half of `calculateGroupDrop`/`calculateUngroupedDrop`'s rate
/// chain, for a killer already known to hold premium status.
///
/// **The per-item map replaces the flat rate, it does not stack with it** —
/// Java's chain is `if (byId != null) … else if (herb) {} else if (raid) {}
/// else flat`. That matters on this dist: the flat amount rate is ×2 but the
/// listed jewels (6656-6662, 8191, 10170, 10314) are pinned to ×1, so premium
/// buys nothing on them.
///
/// **The herb and raid arms are empty in Java** — a premium killer gets no
/// bonus at all on a herb or a raid drop unless the item is in the map. The
/// two `Premium herb chance? :)` musings upstream are Java's own,
/// and returning 1.0 here is what the shipped code does.
pub(crate) fn premium_drop_mult(
    world: &World,
    item_id: i32,
    is_raid: bool,
    which: PremiumDropRate,
) -> f64 {
    let cfg = &world.cfg.premium;
    let by_id = match which {
        PremiumDropRate::Chance => &cfg.rate_drop_chance_by_id,
        PremiumDropRate::Amount => &cfg.rate_drop_amount_by_id,
    };
    if let Some(mult) = by_id.get(&item_id) {
        return *mult;
    }
    let is_herb = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|i| i.ex_immediate_effect);
    if is_herb || is_raid {
        return 1.0;
    }
    match which {
        PremiumDropRate::Chance => cfg.rate_drop_chance,
        PremiumDropRate::Amount => cfg.rate_drop_amount,
    }
}

/// Test hook for [`auto_loots`] — the drop path needs a full kill to reach it,
/// which is a lot of fixture for a rule about one predicate.
#[cfg(test)]
pub(crate) fn auto_loots_for_test(world: &World, item_id: i32, is_raid: bool) -> bool {
    auto_loots(world, item_id, is_raid)
}

/// `Attackable.doItemDrop`'s auto-loot test, in Java's own order:
///
/// ```text
/// AUTO_LOOT_ITEM_IDS.contains(id) || isFlying()
///   || (!hasExImmediateEffect() && ((!isRaid && AUTO_LOOT) || (isRaid && AUTO_LOOT_RAIDS)))
///   || (hasExImmediateEffect() && AUTO_LOOT_HERBS)
/// ```
///
/// The load-bearing clause is `!hasExImmediateEffect()` on the ordinary arm:
/// **herbs are excluded from plain `AutoLoot`** and can only be auto-looted by
/// `AutoLootHerbs`. The port used to apply `AutoLoot` to everything, so with
/// `AutoLoot = True` and `AutoLootHerbs = False` — this dist — herbs were
/// vacuumed straight into the inventory instead of falling to the ground for
/// the walk-over pickup that is the entire point of a herb.
///
/// `isFlying()` is a wyvern-rider test with no counterpart here: the port has
/// no flying *NPC* drop path, and this is the NPC's own flag, not the killer's.
fn auto_loots(world: &World, item_id: i32, is_raid: bool) -> bool {
    let cfg = &world.cfg.character;
    if cfg.auto_loot_item_ids.contains(&item_id) {
        return true;
    }
    let is_herb = world
        .data
        .item_data
        .get(item_id)
        .is_some_and(|t| t.ex_immediate_effect);
    if is_herb {
        return cfg.auto_loot_herbs;
    }
    if is_raid {
        cfg.auto_loot_raids
    } else {
        cfg.auto_loot
    }
}

/// `NpcTemplate.calculateDrops(DropType.SPOIL)` — the `<spoil>` list through
/// the shared ungrouped pass, which is exactly what Java does: same loop, same
/// occurrence budget and level-gap gate, with the `SPOIL` arm of
/// `calculateUngroupedDrop` supplying the rates.
fn roll_spoil_drops(world: &mut World, t: &NpcTemplate, killer_oid: i32) -> Vec<(i32, i64)> {
    if t.drop_list_spoil.is_empty() {
        return Vec::new();
    }
    let Some(ctx) = drop_ctx(world, t, killer_oid, false) else {
        return Vec::new();
    };
    let list = t.drop_list_spoil.clone();
    calculate_ungrouped_drops(world, &list, true, &ctx)
}

/// `Player.addItem` (auto-loot path): stack or create, persist, notify.
/// Ground drops (`AutoLoot = False`) are not ported — with the dist config's
/// `AutoLoot = True` this is the live path.
pub(crate) fn give_item(world: &mut World, player_oid: i32, item_id: i32, count: i64) {
    if !world.cfg.character.auto_loot {
        return; // ground-drop path unported (see PROGRESS G9 notes).
    }
    let Some(changes) =
        crate::game_loop::helpers::add_inventory_item_changes(world, player_oid, item_id, count)
    else {
        tracing::warn!("give_item: object-id pool exhausted, dropping loot {item_id}×{count}");
        return;
    };

    // "You have obtained …" + InventoryUpdate + the weight/adena footers.
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    if item_id == ADENA_ID {
        send_sm_to_client(
            world,
            client_id,
            sm_ids::YOU_HAVE_OBTAINED_S1_ADENA,
            &[SmParam::Long(count)],
        );
    } else {
        send_to_client(
            world,
            client_id,
            server_packets::obtained_item_sm(item_id, count),
        );
    }
    // Java `Player.addItem` funnels through `PlayerInventory.addItem` →
    // `sendInventoryUpdate`, so the status-bar adena counter and weight bar
    // refresh with the loot. Sending the bare `InventoryUpdate` left the bar
    // stale until the next relog/item-list.
    crate::game_loop::helpers::send_inventory_update(world, player_oid, changes);
}

// ---------------------------------------------------------------------------
// XP/SP gain and level-ups (`PlayableStat.addExp` / `PlayerStat.addLevel`)
// ---------------------------------------------------------------------------

/// `Player.onDieDropItem` — scatter part of the victim's inventory.
///
/// Two rate sets, per Java:
/// * a **playable** killer only triggers drops when the victim is a PK past
///   `MinimumPKRequiredToDrop` — this is the karma penalty, not a general
///   looting mechanic;
/// * a **monster** killer uses the (much gentler) player rates, which is why an
///   ordinary death to a mob can still cost an item.
///
/// Nothing drops inside a PVP zone when a player did the killing (arena deaths
/// are free), and GMs are exempt.
///
/// Items the datapack marks `is_dropable="false"` and time-limited items are
/// skipped, per Java's filter.
///
/// Java's per-item filter is ported in full: `isShadowItem() ||
/// isTimeLimitedItem() || !isDropable() || ADENA || TYPE2_QUEST`. The
/// shadow-item leg reads the **instance's** mana, not the template's
/// `duration`, because two copies of one item id can differ.
///
/// Not modelled: the active pet's control item — Java compares
/// `_pet.getControlObjectId()` (an *object* id) against `itemDrop.getId()` (an
/// *item* id), a comparison that cannot match, so porting it would add a
/// branch that never fires; revisit only if a capture ever shows a summoned
/// pet's collar surviving a karma drop through some other guard. Nor
/// the `KarmaListNonDroppableItems`/`..._PET_ITEMS` whitelists (neither is
/// populated on this dist). The clan-war exemption — a clean victim killed by
/// a war enemy drops nothing — is the first gate below.
pub(crate) fn on_die_drop_item(world: &mut World, victim_oid: i32, killer_oid: i32) {
    use crate::data::item_data;

    // `onDieDropItem`'s first gate: a clean (reputation >= 0) victim whose
    // clan is at war with the killer's drops nothing — war deaths are free.
    {
        let pk = crate::game_loop::pvp::acting_player(world, killer_oid);
        let vc = world
            .objects
            .get_component::<Player>(&victim_oid)
            .map(|p| (p.reputation, p.clan_id));
        let kc = clan_of_or_zero(world, pk);
        if let Some((rep, victim_clan)) = vc
            && rep >= 0
            && crate::game_loop::clans::at_war_between(world, kc, victim_clan)
        {
            return;
        }
    }
    let killer_is_player = world.objects.has_component::<Player>(&killer_oid);
    // Arena deaths cost nothing when another player did it.
    if killer_is_player
        && world
            .objects
            .get_component::<crate::model::components::ZoneFlags>(&victim_oid)
            .is_some_and(|f| f.contains(crate::data::zone_data::ZoneKind::Pvp))
    {
        return;
    }
    let Some(victim) = world.objects.get_component::<Player>(&victim_oid) else {
        return;
    };
    // `!isGM() || Config.KARMA_DROP_GM` — a GM's corpse drops only when the
    // config says it may. False on this dist, so a GM never drops.
    if victim.is_gm(&world.data) && !world.cfg.pvp.karma_drop_gm {
        return;
    }
    let (reputation, pk_kills) = (victim.reputation, victim.pk_kills);

    let r = &world.cfg.rates;
    let (rate, item_pct, equip_pct, weapon_pct, limit) = if killer_is_player {
        // Karma drops only once the victim is a repeat PK.
        if reputation >= 0 || pk_kills < r.karma_pk_limit {
            return;
        }
        (
            r.karma_rate_drop,
            r.karma_rate_drop_item,
            r.karma_rate_drop_equip,
            r.karma_rate_drop_equip_weapon,
            r.karma_drop_limit,
        )
    } else if crate::game_loop::combat::is_npc_oid(killer_oid) {
        (
            r.player_rate_drop,
            r.player_rate_drop_item,
            r.player_rate_drop_equip,
            r.player_rate_drop_equip_weapon,
            r.player_drop_limit,
        )
    } else {
        return;
    };

    if rate <= 0 || world.roll(100) >= rate {
        return;
    }

    // Snapshot first: dropping mutates the inventory underneath us. `mana_left`
    // rides along because `isShadowItem()` is a property of the **instance**
    // (Java `Item._mana >= 0`), not of the template — two copies of the same
    // item id can differ.
    let candidates: Vec<(i32, i32, i64, i32, i32)> = world
        .objects
        .get_component::<crate::model::inventory::Inventory>(&victim_oid)
        .map(|inv| {
            inv.items()
                .iter()
                .map(|i| {
                    (
                        i.object_id,
                        i.item_id,
                        i.count,
                        i.enchant_level,
                        i.mana_left,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let Some(pos) = maybe_position(world, victim_oid) else {
        return;
    };

    let mut dropped = 0;
    let mut dropped_equipped = false;
    for (obj_id, item_id, count, enchant, mana_left) in candidates {
        if limit > 0 && dropped >= limit {
            break;
        }
        let Some(t) = world.data.item_data.get(item_id) else {
            continue;
        };
        // Java's filter, in full: `isShadowItem() || isTimeLimitedItem() ||
        // !isDropable() || ADENA || TYPE2_QUEST || the active pet's control
        // item || either `PVP.ini` list`. The pet-control leg is **dead in
        // Java**: it compares `_pet.getControlObjectId()` — an id from the
        // world pool, which starts at 0x10000000 — against `itemDrop.getId()`,
        // a template id no higher than five digits, so the two can never be
        // equal. A summoned pet's collar is kept off the ground by
        // `ListOfPetItems` instead, which is what actually contains 2375. The shadow-item leg is the
        // *instance's* mana (`_mana >= 0`), which is why `mana_left` is carried
        // through the snapshot rather than read off the template — 295 shadow
        // items are reachable on this chronicle, and without this a Shadow
        // weapon scattered on a karma death.
        if item_id == ADENA_ID
            || t.is_quest_item
            || t.type2 == item_data::TYPE2_QUEST
            || !t.is_dropable()
            || t.is_time_limited()
            || crate::game_loop::item_mana::is_shadow_item(mana_left)
            // `PVP.ini`'s two lists (`KARMA_LIST_NONDROPPABLE_ITEMS` and its
            // pet twin): 30 ids a PK never scatters — adena, the hero
            // weapons and circlet, the newbie gear — plus 12 pet items.
            // Java binary-searches both; the lists are sorted at load, so
            // `binary_search` here is the same operation.
            || world
                .cfg
                .rates
                .karma_nondroppable_items
                .binary_search(&item_id)
                .is_ok()
            || world
                .cfg
                .rates
                .karma_nondroppable_pet_items
                .binary_search(&item_id)
                .is_ok()
        {
            continue;
        }
        let equipped = world
            .objects
            .get_component::<crate::model::inventory::Inventory>(&victim_oid)
            .is_some_and(|inv| inv.paperdoll_slot_of(obj_id).is_some());
        let chance = if equipped {
            if t.type2 == item_data::TYPE2_WEAPON {
                weapon_pct
            } else {
                equip_pct
            }
        } else {
            item_pct
        };
        if chance <= 0 || world.roll(100) >= chance {
            continue;
        }
        // Equipped items come off first (`unEquipItemInSlot`).
        if equipped
            && let Some(inv) = world
                .objects
                .get_component_mut::<crate::model::inventory::Inventory>(&victim_oid)
        {
            inv.unequip_item(obj_id);
            dropped_equipped = true;
        }
        // Java's unequip listener drops the augment bonuses. It has to run here,
        // while the instance is still in the bag: once the removal below takes
        // it out there is nothing left to read its option ids from, and the
        // wearer would keep an augmented weapon's stats and granted skills
        // after scattering it on the ground.
        if equipped {
            crate::game_loop::options::remove_item_options(world, victim_oid, obj_id);
        }
        if let Some(inv) = world
            .objects
            .get_component_mut::<crate::model::inventory::Inventory>(&victim_oid)
        {
            inv.remove_item(item_id, count);
        }
        crate::game_loop::ground_items::spawn_ground_item(
            world,
            item_id,
            count,
            enchant,
            pos.x,
            pos.y,
            pos.z,
            victim_oid,
            crate::game_loop::ground_items::DropSource::Player,
        );
        dropped += 1;
    }
    if dropped > 0
        && let Some(client_id) = client_for_player(world, victim_oid)
    {
        if let Some(v) = crate::model::PlayerView::of_world(world, victim_oid) {
            send_to_client(
                world,
                client_id,
                crate::network::enter_world::item_list(v.inventory, &world.data, false),
            );
        }
        // Anything that came off the paperdoll needs the client's own equip
        // snapshot resent, or the corpse keeps rendering gear it just scattered
        // on the ground (`ExUserInfoEquipSlot`, not `ItemList`, drives it).
        if dropped_equipped {
            crate::game_loop::items::refresh_equip_state(world, client_id, victim_oid);
            // Gear leaving the paperdoll can flip the grade penalty, the weight
            // penalty and any armor-conditioned passive — Java fires these off
            // the same unequip listener.
            crate::game_loop::items::refresh_after_paperdoll_change(world, victim_oid);
        }
    }
}
