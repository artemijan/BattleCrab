//! Custom actions: heal, teleport, delevel, buff and premium purchase.

use super::account_of;
use super::charge;
use super::charge_item;
use super::read_html;
use super::send_cb_html;
use super::serve_page;
use crate::game_loop::helpers::is_dead;
use crate::game_loop::helpers::send_message;
use crate::game_loop::helpers::send_to_client;
use crate::game_loop::helpers::skill_by_id;
use crate::model::Player;
use crate::model::inventory::Inventory;
use crate::network::server_packets as sp;
use crate::world::World;
use tracing::warn;
/// `HomeBoard`'s `_bbsheal;<page>` branch: full HP/MP/CP restore, then re-render
/// the page. Reuses the `//heal` primitive.
pub(super) fn do_heal(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_heal {
        return;
    }
    let price = world.cfg.community_board.heal_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    crate::game_loop::admin::vitals::heal_creature(world, object_id);
    // Java tops up `getPet()` and every `getServitors()` entry alongside the
    // owner — both summonable since G29, so the leg is live now.
    for summon in [
        crate::game_loop::servitor::pet_of(world, object_id),
        crate::game_loop::servitor::servitor_of(world, object_id),
    ]
    .into_iter()
    .flatten()
    {
        crate::game_loop::admin::vitals::heal_creature(world, summon);
    }
    send_message(world, client_id, "You used heal!");
    serve_page(
        world,
        client_id,
        object_id,
        command.strip_prefix("_bbsheal;"),
        "",
    );
}

/// `HomeBoard`'s `_bbsteleport;<x> <y> <z>` branch: charge, hide the board and
/// teleport to the whitelisted destination. Reuses the gatekeeper teleport
/// primitive.
pub(super) fn do_teleport(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_teleports {
        return;
    }
    let Some(key) = command.strip_prefix("_bbsteleport;") else {
        return;
    };
    let Some(&(x, y, z)) = world
        .cfg
        .community_board
        .available_teleports
        .get(key.trim())
    else {
        warn!("CommunityBoard: teleport [{key}] not in the gatekeeper whitelist.");
        return;
    };
    let price = world.cfg.community_board.teleport_price;
    if !charge(world, client_id, object_id, price) {
        return;
    }
    // Java hides the board (`new ShowBoard()`) and `disableAllSkills()` for
    // 3 s around the teleport; `SkillsDisabled` + the timed re-enable mirror
    // the `enableAllSkills` ThreadPool.schedule.
    send_to_client(world, client_id, sp::show_board_hide());
    world
        .objects
        .add_components(&object_id, crate::model::components::SkillsDisabled);
    world.scheduler.schedule(
        world.tick + 30,
        crate::scheduler::ScheduledTask::SkillsReenable { object_id },
    );
    let dead = is_dead(world, object_id);
    if !dead {
        crate::game_loop::death::teleport_player(world, object_id, x, y, z);
    }
}

/// `HomeBoard`'s `_bbsdelevel` branch — config-off on this dist
/// (`EnableDelevel = False`), ported per the config-disabled rule: pay the
/// currency, drop exactly one level, come back at full HP/MP/CP. Java's
/// refusal order is funds first, then the level-1 floor, and only then the
/// charge; `set_level` carries the `checkPlayerSkills()` re-check.
pub(super) fn do_delevel(world: &mut World, client_id: u32, object_id: i32) {
    if !world.cfg.community_board.enable_delevel {
        return;
    }
    let price = world.cfg.community_board.delevel_price;
    let currency = world.cfg.community_board.currency_id;
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map_or(0, |inv| inv.count_of(currency));
    if have < price {
        send_message(world, client_id, "Not enough currency!");
        return;
    }
    let level = world
        .objects
        .get_component::<Player>(&object_id)
        .map_or(0, |p| p.level);
    if level <= 1 {
        send_message(world, client_id, "You are at minimum level!");
        return;
    }
    if !charge(world, client_id, object_id, price) {
        return;
    }
    let new_level = level - 1;
    let exp = world.data.experience.exp_for_level(new_level);
    if let Some(p) = world.objects.get_component_mut::<Player>(&object_id) {
        p.exp = exp;
    }
    crate::game_loop::death::set_level(world, object_id, new_level);
    crate::game_loop::admin::vitals::heal_creature(world, object_id);
    if let Some(html) = read_html(
        world,
        client_id,
        "data/html/CommunityBoard/Custom/delevel/complete.html",
    ) {
        send_cb_html(world, client_id, &html);
    }
}

/// `HomeBoard`'s `_bbsbuff;<id,lvl>;…;<page>` branch: apply each whitelisted
/// buff to the player, then re-render the page. Reuses the effect engine.
pub(super) fn do_buff(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    if !world.cfg.community_board.enable_buffs {
        return;
    }
    let body = command.strip_prefix("_bbsbuff;").unwrap_or("");
    let parts: Vec<&str> = body.split(';').collect();
    // Last token is the return page; the rest are `id,level` pairs.
    let (page, buffs) = parts
        .split_last()
        .map(|(p, b)| (Some(*p), b))
        .unwrap_or((None, &[]));

    let price = world.cfg.community_board.buff_price * buffs.len() as i64;
    if !charge(world, client_id, object_id, price) {
        return;
    }

    for spec in buffs {
        let mut it = spec.split(',');
        let (Some(id), Some(lvl)) = (
            it.next().and_then(|s| s.trim().parse::<i32>().ok()),
            it.next().and_then(|s| s.trim().parse::<i32>().ok()),
        ) else {
            continue;
        };
        if !world.cfg.community_board.available_buffs.contains(&id) {
            continue; // anti-exploit whitelist (Java `COMMUNITY_AVAILABLE_BUFFS`)
        }
        let Some(skill) = skill_by_id(world, id, lvl) else {
            warn!("CommunityBoard: buff skill {id}/{lvl} missing from skill data.");
            continue;
        };
        // Java builds one target list — `[player, pet?, servitor…]` — and casts
        // each buff at every member of it, gated on `isSharedWithSummon() ||
        // target.isPlayer()`: a non-shared buff reaches only the player.
        //
        // The servitor is in this list *and* picks the same buff up again
        // through `Skill.applyEffects`' own sharing branch. That double-apply is
        // Java's too, and it refreshes rather than stacks, so the literal target
        // list is kept rather than "optimised" into something that diverges.
        for target in buff_targets(world, object_id) {
            if !skill.shared_with_summon && target != object_id {
                continue;
            }
            crate::game_loop::skills::effects::apply_skill_effects(
                world, object_id, target, &skill,
            );
            // `CommunityCastAnimations`: Java sends this to the **caster only** —
            // its own source carries a commented-out `broadcastPacket` with the
            // note "not recommend broadcast", so onlookers see nothing.
            if world.cfg.community_board.cast_animations {
                cast_animation(world, client_id, object_id, target, &skill);
            }
        }
    }
    serve_page(world, client_id, object_id, page, "");
}

/// Java's `targets` list in `_bbsbuff`: the player, their pet if any, then
/// their servitors. Order matters only for the animation packets.
pub(super) fn buff_targets(world: &World, object_id: i32) -> Vec<i32> {
    let mut targets = vec![object_id];
    targets.extend(crate::game_loop::servitor::pet_of(world, object_id));
    targets.extend(crate::game_loop::servitor::servitor_of(world, object_id));
    targets
}

/// The `CommunityCastAnimations` `MagicSkillUse`, sent to the buying player
/// only. The caster is the player in every case — including the pet/servitor
/// targets, which Java also credits to the owner rather than to the summon.
pub(super) fn cast_animation(
    world: &World,
    client_id: u32,
    caster_oid: i32,
    target_oid: i32,
    skill: &crate::model::skill::Skill,
) {
    let Some(caster) = world.objects.get_component::<Player>(&caster_oid) else {
        return;
    };
    let Some(caster_pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&caster_oid)
    else {
        return;
    };
    let Some(target_pos) = world
        .objects
        .get_component::<crate::model::components::Position>(&target_oid)
    else {
        return;
    };
    let pkt = sp::magic_skill_use(
        caster,
        caster_pos,
        (target_oid, target_pos.x, target_pos.y, target_pos.z),
        skill.id,
        skill.level,
        skill.hit_time,
        skill.reuse_delay_group,
        skill.reuse_delay,
    );
    send_to_client(world, client_id, pkt);
}

/// `HomeBoard`'s `_bbspremium;<days>` branch: buy `<days>` (1–30) days of
/// account premium at `premium_price_per_day` each, then serve the thank-you
/// page. Reuses the `PremiumManager` store already ported for `//premium_*`.
pub(super) fn do_premium(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    use crate::game_loop::admin::premium;
    // `HomeBoard.CUSTOM_COMMANDS` only registers `_bbspremium` when both the
    // global premium system and the community premium option are on.
    if !premium::premium_system_enabled(world)
        || !world.cfg.community_board.community_premium_system
    {
        return;
    }
    // `_bbspremium;<days>` → Java splits the tail on `,` and takes the first field.
    let days: i64 = command
        .strip_prefix("_bbspremium;")
        .and_then(|t| t.split(',').next())
        .and_then(|d| d.trim().parse().ok())
        .unwrap_or(0);
    let price = world
        .cfg
        .community_board
        .premium_price_per_day
        .saturating_mul(days);
    // Java folds the range check into the "Not enough currency!" guard.
    if !(1..=30).contains(&days) {
        send_message(world, client_id, "Not enough currency!");
        return;
    }
    let coin = world.cfg.community_board.premium_coin_id;
    if !charge_item(world, client_id, object_id, coin, price) {
        return;
    }

    let Some(account) = account_of(world, client_id) else {
        return;
    };
    let enddate = premium::add_premium_time(world, &account, days * premium::DAY_MILLIS);
    send_message(
        world,
        client_id,
        &format!(
            "Your account will now have premium status until {}.",
            premium::format_datetime(enddate)
        ),
    );
    // `HomeBoard`: a fresh premium account re-arms the PA-point timer (the
    // `PcCafeOnlyPremium` gate may only now be satisfied).
    crate::game_loop::pc_cafe::run(world, object_id);
    serve_page(
        world,
        client_id,
        object_id,
        Some("premium/thankyou.html"),
        "",
    );
}
