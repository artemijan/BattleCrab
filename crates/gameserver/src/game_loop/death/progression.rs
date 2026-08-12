use super::*;
use crate::game_loop::helpers::vitals_pair;
use crate::game_loop::helpers::{
    send_sm_bare_to_client, send_sm_bare_to_player, send_sm_to_client, send_sm_to_player,
    send_to_client,
};

/// `Attackable.calculateOverhitExp` — the bonus XP a killing `<overHit>` blow
/// earns, and the "over-hit!" notice that goes with it.
///
/// The bonus is the excess damage as a share of the victim's **max** HP,
/// **capped at 25 %**, applied to that attacker's exp share. Returns 0 for
/// anyone who didn't land the over-hit blow, and clears the record so a single
/// kill pays it once.
pub(crate) fn overhit_bonus(world: &mut World, npc_oid: i32, attacker_oid: i32, exp: f64) -> f64 {
    use crate::model::components::Overhit;
    let Some(oh) = world.objects.get_component::<Overhit>(&npc_oid).copied() else {
        return 0.0;
    };
    if oh.attacker != attacker_oid {
        return 0.0;
    }
    let max_hp = world
        .objects
        .get_component::<Vitals>(&npc_oid)
        .map(|v| v.max_hp as f64)
        .unwrap_or(0.0);
    if max_hp <= 0.0 {
        return 0.0;
    }
    world.objects.remove_component::<Overhit>(&npc_oid);
    let percentage = ((oh.damage * 100.0) / max_hp).min(25.0);
    send_sm_bare_to_player(world, attacker_oid, sm_ids::OVER_HIT);
    (percentage / 100.0) * exp
}

/// The vitality half of `Attackable.onKill`'s reward block: charge the killer
/// for the kill (`updateVitalityPoints(getVitalityPoints(level, exp, isRaid),
/// true, false)`).
///
/// `RaidbossUseVitality = False` on this dist, so raid kills are skipped
/// outright — Java expresses the same thing through
/// `Config.RAIDBOSS_USE_VITALITY` gating `_isRaid` into the boss branch.
pub(crate) fn consume_kill_vitality(
    world: &mut World,
    player_oid: i32,
    player_level: i32,
    t: &NpcTemplate,
    exp: f64,
) {
    if !world.cfg.character.enable_vitality {
        return;
    }
    let is_boss = t.is_raid();
    if is_boss && !world.cfg.character.raidboss_use_vitality {
        return;
    }
    let delta = crate::game_loop::vitality::kill_vitality_delta(
        world,
        t.level,
        t.exp,
        player_level,
        exp,
        is_boss,
    );
    crate::game_loop::vitality::update_vitality_points(world, player_oid, delta, true, false);
    // (Java's `givePcCafePoint` sits beside this call in `Attackable.onKill`,
    // but outside the vitality-enabled guard above — see the two call sites.)
}

/// `PlayerStat.addExpAndSp(addToExp, addToSp, useBonuses)`.
///
/// `use_bonuses` is Java's third argument: the kill path passes
/// `Attackable.useVitalityRate()` — true for an ordinary mob, and false for a
/// champion unless `ChampionEnableVitality` — while quest rewards and
/// `//add_exp_sp` go through the two-argument overload, which passes **false**. When set, the vitality/skill exp bonus
/// multiplies the reward and the acquisition message reports the surplus in its
/// "bonus" slots — which is where the client's floating "+N XP bonus" comes
/// from.
///
/// Java's fishing-rod branch (`FANCY_FISHING_ROD_SKILL` → ×1.5) is not ported —
/// fishing is G32. Amounts stay `f64` until the final `Math.round`, as in Java,
/// so the bonus never compounds a rounding error.
pub(crate) fn add_exp_and_sp(
    world: &mut World,
    player_oid: i32,
    exp: f64,
    sp: f64,
    use_bonuses: bool,
) {
    let (bonus_exp, bonus_sp) = if use_bonuses {
        // Java reads the exp and sp multipliers separately; with BONUS_EXP /
        // BONUS_SP unmodelled they are the same value today.
        (
            crate::game_loop::vitality::exp_bonus_multiplier(world, player_oid),
            crate::game_loop::vitality::exp_bonus_multiplier(world, player_oid),
        )
    } else {
        (1.0, 1.0)
    };
    let (base_exp, base_sp) = (exp, sp);
    let (mut add_exp, mut add_sp) = (exp * bonus_exp, sp * bonus_sp);

    // Java `PlayerStat.addExpAndSp`: a nearby pet takes its cut **out of the
    // owner's award**, not on top of it — hunting with a pet costs the player
    // exp. The split happens after the bonuses, so the pet shares them.
    let (owner_ratio, pet_exp, pet_sp) =
        crate::game_loop::servitor::split_exp_with_pet(world, player_oid, add_exp, add_sp);
    if pet_exp > 0.0 || pet_sp > 0.0 {
        crate::game_loop::servitor::add_pet_exp(world, player_oid, pet_exp, pet_sp);
    }
    add_exp *= owner_ratio;
    add_sp *= owner_ratio;

    let (exp, sp) = (add_exp.round() as i64, add_sp.round() as i64);

    let max_level = world.data.experience.max_level as i32;
    let cap = world.data.experience.exp_for_level(max_level) - 1;
    let (old_level, new_exp) = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        p.exp = (p.exp + exp.max(0)).min(cap);
        p.sp = p.sp.saturating_add(sp.max(0));
        (p.level, p.exp)
    };
    if exp > 0 || sp > 0 {
        send_sm_to_player(
            world,
            player_oid,
            sm_ids::YOU_HAVE_ACQUIRED_S1_XP_BONUS_S2_AND_S3_SP_BONUS_S4,
            &[
                SmParam::Long(exp),
                SmParam::Long((add_exp - base_exp).round() as i64),
                SmParam::Long(sp),
                SmParam::Long((add_sp - base_sp).round() as i64),
            ],
        );
    }

    let new_level = level_for_exp(world, new_exp, max_level);
    apply_level_change(world, player_oid, old_level, new_level);
}

/// Java `Player.removeExpAndSp` — subtract exp/sp (each floored at 0) and
/// delevel if the exp total now falls under the current level's threshold. The
/// mirror of [`add_exp_and_sp`]; used by the `//remove_exp_sp` admin command.
pub(crate) fn remove_exp_and_sp(world: &mut World, player_oid: i32, exp: i64, sp: i64) {
    let max_level = world.data.experience.max_level as i32;
    let (old_level, new_exp) = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        p.exp = (p.exp - exp.max(0)).max(0);
        p.sp = (p.sp - sp.max(0)).max(0);
        (p.level, p.exp)
    };
    let new_level = level_for_exp(world, new_exp, max_level);
    apply_level_change(world, player_oid, old_level, new_level);
}

/// Land an exp change: relevel if the threshold moved, otherwise just refresh
/// the exp bar (Java `player.updateUserInfo()`).
///
/// `set_level` already broadcasts, so the `UserInfo` send is only needed on the
/// no-level-change path.
fn apply_level_change(world: &mut World, player_oid: i32, old_level: i32, new_level: i32) {
    if new_level != old_level {
        set_level(world, player_oid, new_level);
        return;
    }
    let Some(client_id) = client_for_player(world, player_oid) else {
        return;
    };
    if let Some(v) = crate::model::PlayerView::of_world(world, player_oid) {
        send_to_client(
            world,
            client_id,
            crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                crate::game_loop::party::calculate_relation(world, v.p),
            ),
        );
    }
}

/// The `PlayableStat.addExp` level scan: highest level whose threshold the
/// exp total clears.
pub(crate) fn level_for_exp(world: &World, exp: i64, max_level: i32) -> i32 {
    let mut level = 1;
    for l in 1..=max_level {
        if exp >= world.data.experience.exp_for_level(l) {
            level = l;
        } else {
            break;
        }
    }
    level
}

/// `PlayerStat.addLevel` (up or down): recompute vitals/stats, grant new
/// autoGet skills, broadcast the level-up flourish.
pub(crate) fn set_level(world: &mut World, player_oid: i32, new_level: i32) {
    let leveled_up = {
        let Some(p) = world
            .objects
            .get_component_mut::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        let up = new_level > p.level;
        p.level = new_level;
        up
    };
    // Vitals follow the level tables (`getMaxHp` etc. read level).
    {
        let data = &world.data;
        let Some((p, mut vitals, mut pvitals, base, mods, inventory, mut speeds, mut combat)) =
            world.objects.get_many_mut::<(
                &mut crate::model::Player,
                &mut Vitals,
                &mut PlayerVitals,
                &BaseStats,
                &StatModifiers,
                &crate::model::inventory::Inventory,
                &mut Speeds,
                &mut crate::model::components::CombatStats,
            )>(&player_oid)
        else {
            return;
        };
        let t = data
            .player_templates
            .get_or_base(p.class_id, p.base_class_id)
            .cloned()
            .unwrap_or_default();
        vitals.max_hp = crate::model::calc_max_hp(data, &t, p.level, Some(inventory), mods) as i32;
        vitals.max_mp = crate::model::calc_max_mp(data, &t, p.level, Some(inventory), mods) as i32;
        pvitals.max_cp = crate::model::calc_max_cp(data, &t, p.level, mods) as i32;
        if leveled_up {
            // Classic level-up: all vitals refill (Mobius Java only refills
            // CP here, but retail Classic restores HP/MP too).
            vitals.cur_hp = vitals.max_hp as f64;
            vitals.cur_mp = vitals.max_mp as f64;
            pvitals.cur_cp = pvitals.max_cp as f64;
        } else {
            vitals.cur_hp = vitals.cur_hp.min(vitals.max_hp as f64);
            vitals.cur_mp = vitals.cur_mp.min(vitals.max_mp as f64);
            pvitals.cur_cp = pvitals.cur_cp.min(pvitals.max_cp as f64);
        }
        p.recalculate_stats(data, base, mods, inventory, &mut speeds, &mut combat);
    }

    // `rewardSkills`: grant the skills now reachable (autoGet only, or — with
    // `AutoLearnSkills` — every reachable class skill).
    reward_skills(world, player_oid);

    // `Player.checkPlayerSkills` (`PlayableStat.addLevel` on a delevel, and
    // inside `rewardSkills`): downgrade/remove any skill that now outranks the
    // level. No-op on a level-up (nothing sits above the higher level).
    check_player_skills(world, player_oid);

    if leveled_up {
        broadcast_including_self(
            world,
            player_oid,
            &server_packets::social_action(player_oid, server_packets::SOCIAL_ACTION_LEVEL_UP),
        );
    }
    // Status + full info refresh (`broadcastStatusUpdate` + `updateUserInfo`
    // + `SkillList`).
    let Some((vitals, pvitals)) = vitals_pair(world, player_oid) else {
        return;
    };
    broadcast_including_self(
        world,
        player_oid,
        &server_packets::status_update(
            player_oid,
            &[
                (server_packets::status_update_type::MAX_HP, vitals.max_hp),
                (
                    server_packets::status_update_type::CUR_HP,
                    vitals.cur_hp as i32,
                ),
                (server_packets::status_update_type::MAX_MP, vitals.max_mp),
                (
                    server_packets::status_update_type::CUR_MP,
                    vitals.cur_mp as i32,
                ),
                (server_packets::status_update_type::MAX_CP, pvitals.max_cp),
                (
                    server_packets::status_update_type::CUR_CP,
                    pvitals.cur_cp as i32,
                ),
            ],
        ),
    );
    // Java `PlayerStat.addLevel` → `PartySmallWindowUpdate(this, true)`.
    crate::game_loop::party::notify_party_all(world, player_oid);
    if let Some(client_id) = client_for_player(world, player_oid)
        && let Some(v) = crate::model::PlayerView::of_world(world, player_oid)
    {
        if leveled_up {
            send_sm_bare_to_client(world, client_id, sm_ids::YOUR_LEVEL_HAS_INCREASED);
        }
        send_to_client(
            world,
            client_id,
            crate::network::user_info::user_info(
                &v,
                &world.data,
                &world.cfg.character,
                crate::game_loop::party::calculate_relation(world, v.p),
            ),
        );
        let Some(pkt) = crate::game_loop::helpers::skill_list_packet(world, player_oid) else {
            return;
        };
        send_to_client(world, client_id, pkt);
    }
}

/// Java `Player.rewardSkills` skill selection: with `AutoLearnSkills` on,
/// every class skill reachable at `level`; otherwise autoGet skills only.
/// Returns the `(id, level)` pairs that are new or an upgrade over `known`.
pub(crate) fn reward_skill_grants(
    data: &crate::data::GameData,
    cfg: &crate::config::CharacterConfig,
    class_id: i32,
    level: i32,
    known: &std::collections::HashMap<i32, i32>,
    is_gm: bool,
) -> Vec<(i32, i32)> {
    if cfg.auto_learn_skills {
        return data.skill_trees.all_available_skills(
            class_id,
            level,
            known,
            cfg.auto_learn_skills_without_items,
            cfg.auto_learn_divine_inspiration || is_gm,
        );
    }
    let mut granted = Vec::new();
    let mut seen: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    for learn in data.skill_trees.auto_get_skills(class_id, level) {
        let cur = seen
            .get(&learn.skill_id)
            .copied()
            .unwrap_or_else(|| known.get(&learn.skill_id).copied().unwrap_or(0));
        if learn.skill_level > cur {
            seen.insert(learn.skill_id, learn.skill_level);
            granted.push((learn.skill_id, learn.skill_level));
        }
    }
    granted
}

/// `Player.rewardSkills` for a live in-world player: grant the reachable
/// skills, persist them, and roll any upgrades into panel shortcuts. With
/// `AutoLearnSkills` it mirrors Java's `ShortCutInit` + "learned N skills"
/// notice.
pub(crate) fn reward_skills(world: &mut World, player_oid: i32) {
    let (class_id, level, known, is_gm) = {
        let Some(p) = world
            .objects
            .get_component::<crate::model::Player>(&player_oid)
        else {
            return;
        };
        let skills = world
            .objects
            .get_component::<SkillBook>(&player_oid)
            .cloned()
            .unwrap_or_default();
        (p.class_id, p.level, skills.0, p.is_gm(&world.data))
    };
    let granted = reward_skill_grants(
        &world.data,
        &world.cfg.character,
        class_id,
        level,
        &known,
        is_gm,
    );
    if granted.is_empty() {
        return;
    }
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
        for &(id, lvl) in &granted {
            book.0.insert(id, lvl);
        }
    }
    for &(id, lvl) in &granted {
        // Memory-first: the grant already landed in the `SkillBook`; it persists
        // on the next flush. `updateShortCuts` — panel slots holding the skill
        // pick up the level (also in-memory).
        crate::game_loop::shortcuts::update_skill_shortcuts(world, player_oid, id, lvl);
    }
    if world.cfg.character.auto_learn_skills
        && let Some(client_id) = client_for_player(world, player_oid)
    {
        let count = granted
            .iter()
            .map(|&(id, _)| id)
            .collect::<std::collections::HashSet<_>>()
            .len();
        if let Some(shortcuts) = world
            .objects
            .get_component::<crate::model::components::Shortcuts>(&player_oid)
        {
            send_to_client(world, client_id, server_packets::shortcut_init(shortcuts));
        }
        send_sm_to_client(
            world,
            client_id,
            sm_ids::S1_TEXT,
            &[SmParam::Text(format!(
                "You have learned {count} new skills."
            ))],
        );
    }
}

/// Java `Player.checkPlayerSkills` + `deacreaseSkillLevel`, as a reusable
/// filter: downgrade or remove the entries in `skills` that the character's
/// `level` no longer supports (config `StrictDelevelSkillRemoval` grace),
/// persisting each change to `character_skills`. Mutates `skills` in place and
/// returns the applied `(skill_id, Some(new_level) | None)` changes so the
/// caller can sync panel shortcuts and — for a live player — recompute passive
/// stats. Empty / no-op when `DecreaseSkillOnDelevel` is off.
///
/// The two call sites: character select (filtering the DB-loaded skill list
/// before the `Player` is built, so `from_char` folds the corrected passives)
/// and every level-down (`PlayerStat.addLevel`, via [`check_player_skills`]).
pub(crate) fn maybe_skill_remove_on_delevel(
    world: &World,
    char_id: i32,
    class_id: i32,
    level: i32,
    skills: &mut std::collections::HashMap<i32, i32>,
) -> Vec<(i32, Option<i32>)> {
    if !world.cfg.character.decrease_skill_level {
        return Vec::new();
    }
    let changes = world.data.skill_trees.delevel_skill_changes(
        class_id,
        level,
        skills,
        world.cfg.character.strict_delevel_skill_removal,
    );
    let _ = char_id; // memory-first: the changes below persist on the next flush.
    for &(skill_id, action) in &changes {
        match action {
            // `deacreaseSkillLevel` → `addSkill(getSkill(id, nextLevel))`.
            Some(new_level) => {
                skills.insert(skill_id, new_level);
            }
            // `deacreaseSkillLevel` → `removeSkill(skill, true)`.
            None => {
                skills.remove(&skill_id);
            }
        }
    }
    changes
}

/// `Player.checkPlayerSkills` for a live in-world player (a level-down):
/// [`maybe_skill_remove_on_delevel`] on the `SkillBook`, then roll the changes
/// into panel shortcuts and re-fold the passive stats (only passive skills move
/// `UserInfo` stats), broadcasting the fresh stats.
pub(crate) fn check_player_skills(world: &mut World, player_oid: i32) {
    let (class_id, level, mut known) = {
        let Some(p) = world.objects.get_component::<Player>(&player_oid) else {
            return;
        };
        let skills = world
            .objects
            .get_component::<SkillBook>(&player_oid)
            .cloned()
            .unwrap_or_default();
        (p.class_id, p.level, skills.0)
    };
    let changes = maybe_skill_remove_on_delevel(world, player_oid, class_id, level, &mut known);
    if changes.is_empty() {
        return;
    }
    // Write the filtered book back, then sync the panel shortcuts.
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&player_oid) {
        book.0 = known;
    }
    for &(skill_id, action) in &changes {
        match action {
            Some(new_level) => crate::game_loop::shortcuts::update_skill_shortcuts(
                world, player_oid, skill_id, new_level,
            ),
            None => {
                crate::game_loop::shortcuts::remove_skill_shortcuts(world, player_oid, skill_id)
            }
        }
    }
    recompute_passives_after_skill_change(world, player_oid, &changes);
}

/// Re-derive a live player's passive-skill stat contributions after a delevel
/// skill change: drop the removed skills' passive buffs, then re-fold the
/// armor-conditioned passives (a downgraded passive re-applies at its new
/// level). Only passive skills carry stat modifiers, so removing/downgrading an
/// active skill leaves the stats untouched here. Updates the stat components in
/// place but sends no packet — the caller (`set_level`) already broadcasts a
/// fresh `UserInfo` for the level change, so this avoids a redundant second one.
fn recompute_passives_after_skill_change(
    world: &mut World,
    player_oid: i32,
    changes: &[(i32, Option<i32>)],
) {
    let removed: Vec<i32> = changes
        .iter()
        .filter_map(|&(id, action)| action.is_none().then_some(id))
        .collect();
    if !removed.is_empty() {
        crate::game_loop::stat_ctx::with_stat_ctx(world, player_oid, |ctx| {
            for &skill_id in &removed {
                ctx.remove(skill_id);
            }
        });
    }
    // Re-fold conditioned passives from the corrected book (handles downgrades),
    // component-only — no send.
    crate::game_loop::passive_skills::recompute_conditioned_passives(world, player_oid);
}

// ---------------------------------------------------------------------------
// Player death + revive
// ---------------------------------------------------------------------------
