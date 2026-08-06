//! Skill & buff commands — `AdminSkill`'s `//add_skill`/`//remove_skill` and
//! `AdminBuffs`' `//buff`/`//getbuffs`/`//stopbuff`/`//stopallbuffs`.

use crate::model::Player;
use crate::model::components::{Buffs, SkillBook};
use crate::world::World;

use super::{current_target, send_message};

/// `AdminSkill`'s `//add_skill <id> [level]` — grant a skill to the targeted
/// player (or self) and refresh their skill list. Passive stat effects apply on
/// the next recompute/relog (the full immediate-passive path is TODO).
pub(super) fn admin_add_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //add_skill <id> [level]");
        return;
    };
    let level = args
        .get(1)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(1)
        .max(1);
    if world.data.skill_data.get(skill_id, level).is_none() {
        send_message(
            world,
            client_id,
            &format!("Skill {skill_id} level {level} does not exist."),
        );
        return;
    }
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        book.0.insert(skill_id, level);
    }
    // Java\'s `addSkill` applies a passive\'s effects as it is learned. Without
    // this a `//add_skill`-ed passive sat inert until the next equip change or
    // relog, which makes testing one look like the skill is broken.
    super::super::passive_skills::refresh_conditioned_passives(world, target);
    refresh_skill_list(world, target);
    send_message(
        world,
        client_id,
        &format!("Added skill {skill_id} (level {level})."),
    );
    show_char_skills(world, client_id, target);
}

/// `AdminSkill`'s `//remove_skill <id>` — remove a skill from the targeted
/// player (or self).
pub(super) fn admin_remove_skill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //remove_skill <id>");
        return;
    };
    let target = current_target(world, object_id)
        .filter(|oid| world.objects.has_component::<Player>(oid))
        .unwrap_or(object_id);
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        book.0.remove(&skill_id);
    }
    super::super::passive_skills::refresh_conditioned_passives(world, target);
    refresh_skill_list(world, target);
    send_message(world, client_id, &format!("Removed skill {skill_id}."));
    show_char_skills(world, client_id, target);
}

/// `AdminSkill`'s `//setskill <id> <level>` — add a skill to the GM
/// themselves (Java always targets `activeChar`), then refresh their list.
pub(super) fn admin_setskill(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let (Some(id), Some(level)) = (
        args.first().and_then(|s| s.parse::<i32>().ok()),
        args.get(1).and_then(|s| s.parse::<i32>().ok()),
    ) else {
        send_message(world, client_id, "Usage: //setskill <id> <level>");
        return;
    };
    if world.data.skill_data.get(id, level).is_none() {
        send_message(
            world,
            client_id,
            &format!("No such skill found. Id: {id} Level: {level}"),
        );
        return;
    }
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&object_id) {
        book.0.insert(id, level);
    }
    refresh_skill_list(world, object_id);
    send_message(
        world,
        client_id,
        &format!("You added yourself skill {id} level {level}."),
    );
}

/// `AdminSkill`'s `//give_all_skills` / `//give_all_skills_fs` — grant the
/// targeted player every skill their class can learn at their level (Java
/// `giveAvailableSkills`). Reuses the class-tree grant path. The `_fs`
/// (forgotten-scroll) variant is identical here — forgotten-scroll skills
/// aren't modelled separately yet.
pub(super) fn admin_give_all_skills(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        super::send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    super::death::reward_skills(world, target);
    refresh_skill_list(world, target);
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    send_message(
        world,
        client_id,
        &format!("Gave available skills to {name}."),
    );
    show_char_skills(world, client_id, target);
}

/// `AdminSkill`'s `//give_clan_skills` / `//give_all_clan_skills` — grant the
/// targeted clan member's clan every pledge skill it qualifies for at its level
/// (Java `adminGiveClanSkills`; `include_squad` also grants squad/sub-pledge
/// skills). Each granted skill applies to the qualifying online members.
pub(super) fn admin_give_clan_skills(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    include_squad: bool,
) {
    use crate::network::server_packets::{SmParam, sm_ids};
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        super::send_sm(world, client_id, sm_ids::INVALID_TARGET);
        return;
    };
    let Some(clan_id) = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.clan_id)
        .filter(|&c| c != 0)
    else {
        super::send_sm(world, client_id, sm_ids::THE_TARGET_MUST_BE_A_CLAN_MEMBER);
        return;
    };
    let target_name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    // Java warns when the target isn't the leader but grants to the clan anyway.
    if world
        .clans
        .get(&clan_id)
        .map(|c| c.leader_id != target)
        .unwrap_or(true)
        && let Some(cs) = world.clients.get(&client_id)
    {
        cs.send(crate::network::server_packets::system_message_with(
            sm_ids::S1_IS_NOT_A_CLAN_LEADER,
            &[SmParam::Text(target_name.clone())],
        ));
    }
    let count = crate::game_loop::clans::give_clan_skills(world, clan_id, include_squad);
    let clan_name = world
        .clans
        .get(&clan_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    send_message(
        world,
        client_id,
        &format!("You gave {count} skills to {target_name}'s clan {clan_name}."),
    );
    if let Some(cid) = super::helpers::client_for_player(world, target) {
        send_message(world, cid, &format!("Your clan received {count} skills."));
    }
}

/// `AdminSkill`'s `//remove_all_skills` — strip every skill from the targeted
/// player.
pub(super) fn admin_remove_all_skills(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        super::send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        book.0.clear();
    }
    // Java `getAllSkills()` includes clan/residence/siege skills (added via
    // `addSkill`), so `removeSkill` strips them too. Those live in the transient
    // `ClanSkills` channel here (not `SkillBook`), so clear it and revert the
    // passive buffs it applied — otherwise //remove_all_skills leaves the clan
    // passives on the character. Player-only (like Java): the clan still owns
    // them, so they re-apply on relog.
    crate::game_loop::clans::remove_clan_skills_from_member(world, target);
    refresh_skill_list(world, target);
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    send_message(
        world,
        client_id,
        &format!("You have removed all skills from {name}."),
    );
    show_char_skills(world, client_id, target);
}

/// `AdminSkill`'s `//get_skills` — replace the GM's skill set with the targeted
/// player's (Java `adminGetSkills`: removes the GM's own skills, then adds the
/// target's; `//reset_skills` re-derives class skills). Ends on the char-skills
/// window (Java `showMainPage`).
pub(super) fn admin_get_skills(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        super::send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    if target == object_id {
        send_message(world, client_id, "You cannot use this on yourself.");
        return;
    }
    let src = world
        .objects
        .get_component::<SkillBook>(&target)
        .cloned()
        .unwrap_or_default();
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&object_id) {
        // Java removes the GM's own skills first, then adds the target's.
        book.0.clear();
        for (&id, &lvl) in &src.0 {
            book.0.insert(id, lvl);
        }
    }
    refresh_skill_list(world, object_id);
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    send_message(
        world,
        client_id,
        &format!("You now have all the skills of {name}."),
    );
    show_char_skills(world, client_id, target);
}

/// Java `AdminSkill.showMainPage` — the `charskills.htm` management window for
/// the GM's current target (name / level / class). Several skill commands end
/// by refreshing it.
pub(super) fn show_char_skills(world: &mut World, client_id: u32, target: i32) {
    let Some(p) = world.objects.get_component::<Player>(&target).cloned() else {
        super::send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    let r: Vec<(&str, String)> = vec![
        ("name", p.name.clone()),
        ("level", p.level.to_string()),
        ("class", p.class_id.to_string()),
    ];
    super::menu::show_admin_html_replace(world, client_id, "charskills.htm", &r);
}

/// `AdminSkill`'s `//reset_skills` — restore the targeted player's class skills
/// (Java pairs this with `//get_skills`; here it re-grants the class-tree
/// skills the character should have at its level, a documented simplification).
pub(super) fn admin_reset_skills(world: &mut World, client_id: u32, object_id: i32) {
    let Some(target) =
        current_target(world, object_id).filter(|oid| world.objects.has_component::<Player>(oid))
    else {
        super::send_sm(
            world,
            client_id,
            crate::network::server_packets::sm_ids::INVALID_TARGET,
        );
        return;
    };
    if let Some(book) = world.objects.get_component_mut::<SkillBook>(&target) {
        book.0.clear();
    }
    super::death::reward_skills(world, target);
    refresh_skill_list(world, target);
    send_message(world, client_id, "Skills reset to class defaults.");
    show_char_skills(world, client_id, target);
}

/// `AdminSkill`'s `//cast` / `//castnow <id> [level]` — apply a skill's effects
/// to the GM's current target (or self). Java's `//cast` runs the full cast
/// (with animation delay) and `//castnow` applies instantly to every affected
/// target; both land on the same effect pipeline here (documented
/// simplification — no cast bar for the admin path).
pub(super) fn admin_cast(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //cast <skillId> <skillLevel>");
        return;
    };
    let level = args
        .get(1)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or_else(|| world.data.skill_data.max_level(id));
    let Some(skill) = world.data.skill_data.get(id, level).cloned() else {
        send_message(
            world,
            client_id,
            &format!("Skill with id: {id}, level: {level} not found."),
        );
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    send_message(
        world,
        client_id,
        &format!("Admin casting {} ({id},{level})", skill.name),
    );
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, target, &skill);
}

/// `AdminSkill`'s HTML-menu commands (`//show_skills`, `//skill_list`,
/// `//skill_index <n>`, `//remove_skills [page]`) — open the corresponding
/// admin HTML page.
pub(super) fn admin_skill_menu(world: &mut World, client_id: u32, command: &str, args: &[&str]) {
    let path = match command {
        "admin_skill_list" | "admin_show_skills" => "skills.htm".to_string(),
        "admin_skill_index" => format!("skills/{}.htm", args.first().copied().unwrap_or("1")),
        // `//remove_skills` is a generated char-skill list in Java; we fall back
        // to the static skills page (the per-char generated page is TODO).
        _ => "skills.htm".to_string(),
    };
    super::menu::show_admin_html(world, client_id, &path);
}

/// Resend a player's `SkillList` after a skill-book change.
pub(crate) fn refresh_skill_list(world: &World, target: i32) {
    let Some(cid) = super::helpers::client_for_player(world, target) else {
        return;
    };
    let Some(packet) = super::helpers::skill_list_packet(world, target) else {
        return;
    };
    if let Some(cs) = world.clients.get(&cid) {
        cs.send(packet);
    }
}

/// `AdminBuffs`'s `//buff <skillId> [level]` — apply a skill's effects to the
/// target (any creature) or self, exactly as a cast would (reuses the cast
/// effect pipeline, so buffs/heals broadcast correctly).
pub(super) fn admin_buff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //buff <skillId> [level]");
        return;
    };
    let level = args
        .get(1)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(1)
        .max(1);
    let Some(skill) = world.data.skill_data.get(skill_id, level).cloned() else {
        send_message(
            world,
            client_id,
            &format!("Skill {skill_id} level {level} does not exist."),
        );
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    crate::game_loop::skills::effects::apply_skill_effects(world, object_id, target, &skill);
}

/// `AdminBuffs`'s `//getbuffs` — the target's active buffs as the `getbuffs.htm`
/// window (Java `showBuffs`), each row carrying an `X` cancel button.
pub(super) fn admin_getbuffs(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    show_buffs(world, client_id, object_id, args, false);
}

/// `AdminBuffs`'s `//getbuffs_ps` — the passive page of the same window.
pub(super) fn admin_getbuffs_ps(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    show_buffs(world, client_id, object_id, args, true);
}

/// Entries per page (Java `PageBuilder.newBuilder(effects, 3, pageLink)`).
///
/// Three is not a typo. Java pages *buffs*, and renders one row per
/// `AbstractEffect` **inside** each one, so a page of three buffs can be a
/// dozen rows. This port keeps its existing one-row-per-buff table — it has no
/// per-effect display model — so its pages are shorter than Java's. The page
/// *size* matches; the rows per page do not, and that is a pre-existing
/// difference in the row shape, not something pagination introduced.
const BUFF_PAGE_SIZE: usize = 3;

/// Render `getbuffs.htm` for the target (Java `showBuffs`): a table of the
/// active (or passive) effects with per-row `admin_stopbuff` buttons, paged.
fn show_buffs(world: &mut World, client_id: u32, object_id: i32, args: &[&str], passive: bool) {
    // Java: `//getbuffs <playername>` wins over the target; otherwise **any
    // Creature** target is valid — which is what the `Buffs` button on the
    // shift-click NPC window relies on. Resolving this as `target_player`
    // (player target, else self) silently showed the *GM's own* buffs whenever
    // an NPC was selected.
    let target = match args.first() {
        Some(name) => match super::find_online_player(world, name) {
            Some(oid) => oid,
            None => {
                send_message(
                    world,
                    client_id,
                    &format!("The player {name} is not online."),
                );
                return;
            }
        },
        None => current_target(world, object_id)
            .filter(|oid| {
                world.objects.has_component::<Player>(oid)
                    || world.objects.has_component::<crate::model::npc::Npc>(oid)
            })
            // Java answers THAT_IS_AN_INCORRECT_TARGET with nothing selected;
            // this port's admin commands consistently fall back to self, as
            // does the `//buff` that fills this window.
            .unwrap_or(object_id),
    };
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .or_else(|| {
            let npc = world
                .objects
                .get_component::<crate::model::npc::Npc>(&target)?;
            Some(npc.template(world)?.name.clone())
        })
        .unwrap_or_default();
    // The pager link always carries the target's name (Java's `pageLink` uses
    // `target.getName()`), so a page button works even when the GM opened the
    // window off a selection rather than by typing a name.
    let name_for_link = name.clone();
    let now = world.tick;
    // `//getbuffs <name> <page>` — the pager appends the page to a link that
    // always carries the target's name, as Java's `pageLink` does.
    let page: i32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let total = world
        .objects
        .get_component::<Buffs>(&target)
        .map(|b| b.0.iter().filter(|x| x.passive == passive).count())
        .unwrap_or(0);
    let pages = total.div_ceil(BUFF_PAGE_SIZE) as i32;
    // Java's clamp tests `>`, not `>=` (see `flags::show_ave_menu`): a
    // hand-typed page equal to the count runs off the end and lists nothing.
    // No pager button can reach it — they only link 0..pages-1.
    let current = if page > pages { pages - 1 } else { page.max(0) };
    let start = (BUFF_PAGE_SIZE as i32 * current).max(0) as usize;
    let mut rows = String::new();
    if let Some(buffs) = world.objects.get_component::<Buffs>(&target) {
        for b in buffs
            .0
            .iter()
            .filter(|x| x.passive == passive)
            .skip(start)
            .take(BUFF_PAGE_SIZE)
        {
            let skill = world.data.skill_data.get(b.skill_id, b.skill_level);
            let sname = skill
                .map(|s| s.name.clone())
                .unwrap_or_else(|| format!("Skill {}", b.skill_id));
            let time = if passive {
                "P".to_string()
            } else {
                format!("{}s", b.expires_at_tick.saturating_sub(now) / 10)
            };
            rows.push_str(&format!(
                "<tr><td>{sname} Lv {}</td><td>{time}</td>\
                 <td><button value=\"X\" action=\"bypass -h admin_stopbuff {target} {}\" \
                 width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>",
                b.skill_level, b.skill_id
            ));
        }
    }
    let r: Vec<(&str, String)> = vec![
        ("targetName", name),
        ("targetObjId", target.to_string()),
        ("buffs", rows),
        // Java fills `%effectSize%` from the *whole* list, not the page.
        ("effectSize", total.to_string()),
        (
            "buffsText",
            if passive {
                "Hide Passives".into()
            } else {
                "Show Passives".into()
            },
        ),
        (
            "passives",
            if passive { String::new() } else { "_ps".into() },
        ),
        (
            "pages",
            if pages > 1 {
                format!(
                    "<table width=280 cellspacing=0><tr>{}</tr></table>",
                    super::spawn::default_pager(
                        &format!(
                            "bypass -h admin_getbuffs{} {name_for_link}",
                            if passive { "_ps" } else { "" }
                        ),
                        current,
                        pages,
                    )
                )
            } else {
                // Java: `if (result.getPages() > 0) … else replace("%pages%", "")`.
                String::new()
            },
        ),
    ];
    super::menu::show_admin_html_replace(world, client_id, "getbuffs.htm", &r);
}

/// `AdminBuffs`'s `//areacancel <radius>` — clear every timed buff from all
/// players within `radius` (Java `Creature::stopAllEffects`).
pub(super) fn admin_areacancel(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(radius) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //areacancel <radius>");
        return;
    };
    for oid in super::creatures_in_range(world, object_id, radius, true, false) {
        let skill_ids: Vec<i32> = world
            .objects
            .get_component::<Buffs>(&oid)
            .map(|b| {
                b.0.iter()
                    .filter(|x| !x.passive)
                    .map(|x| x.skill_id)
                    .collect()
            })
            .unwrap_or_default();
        for skill_id in skill_ids {
            crate::game_loop::skills::effects::handle_buff_expire(world, oid, skill_id);
        }
    }
    send_message(
        world,
        client_id,
        &format!("All effects canceled within radius {radius}"),
    );
}

/// `AdminBuffs`'s `//removereuse [name]` — clear all skill reuse timers for the
/// targeted or named player and resend `SkillCoolTime`.
pub(super) fn admin_removereuse(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let target = match args.first() {
        Some(name) => match super::find_online_player(world, name) {
            Some(t) => t,
            None => {
                send_message(
                    world,
                    client_id,
                    &format!("The player {name} is not online."),
                );
                return;
            }
        },
        None => match current_target(world, object_id)
            .filter(|oid| world.objects.has_component::<Player>(oid))
        {
            Some(t) => t,
            None => {
                super::send_sm(
                    world,
                    client_id,
                    crate::network::server_packets::sm_ids::INVALID_TARGET,
                );
                return;
            }
        },
    };
    if let Some(reuses) = world
        .objects
        .get_component_mut::<crate::model::components::Reuses>(&target)
    {
        reuses.0.clear();
    }
    if let Some(cid) = super::helpers::client_for_player(world, target)
        && let Some(reuses) = world
            .objects
            .get_component::<crate::model::components::Reuses>(&target)
    {
        let packet = crate::network::server_packets::skill_cool_time(reuses, world.tick);
        if let Some(cs) = world.clients.get(&cid) {
            cs.send(packet);
        }
    }
    let name = world
        .objects
        .get_component::<Player>(&target)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    send_message(
        world,
        client_id,
        &format!("Skill reuse was removed from {name}."),
    );
}

/// `AdminBuffs`'s `//stopbuff <skillId>` — remove a single buff from the target
/// (reuses the buff-expiry path: stat revert + rebroadcast).
pub(super) fn admin_stopbuff(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(skill_id) = args.first().and_then(|s| s.parse::<i32>().ok()) else {
        send_message(world, client_id, "Usage: //stopbuff <skillId>");
        return;
    };
    let target = current_target(world, object_id).unwrap_or(object_id);
    crate::game_loop::skills::effects::handle_buff_expire(world, target, skill_id);
    send_message(world, client_id, &format!("Removed buff {skill_id}."));
}

/// `AdminBuffs`'s `//stopallbuffs` (confirmDlg) — remove every timed buff from
/// the target (passive grade-penalty pumps are kept). Each removal reverts its
/// stat contribution through the buff-expiry path.
pub(super) fn admin_stopallbuffs(world: &mut World, client_id: u32, object_id: i32) {
    let target = current_target(world, object_id).unwrap_or(object_id);
    let skill_ids: Vec<i32> = world
        .objects
        .get_component::<Buffs>(&target)
        .map(|b| {
            b.0.iter()
                .filter(|x| !x.passive)
                .map(|x| x.skill_id)
                .collect()
        })
        .unwrap_or_default();
    let count = skill_ids.len();
    for skill_id in skill_ids {
        crate::game_loop::skills::effects::handle_buff_expire(world, target, skill_id);
    }
    send_message(world, client_id, &format!("Removed {count} buff(s)."));
}
