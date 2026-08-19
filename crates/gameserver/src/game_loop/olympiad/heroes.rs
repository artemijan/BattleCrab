//! Heroes: the boot-loaded crown, login re-apply, `claim_hero`, the
//! season-end hero computation, the hero diary and the `ExHeroList` roll.

use super::HERO_ACTION_GAINED_HERO;
use super::HERO_SOCIAL_ACTION;
use crate::db::DbCommand;
use crate::db::HeroRow;
use crate::game_loop::helpers::send_to_client;
use crate::model::Player;
use crate::network::server_packets as sp;
use crate::network::server_packets::SmParam;
use crate::network::server_packets::sm_ids;
use crate::world::World;
/// Apply the boot-loaded `heroes` rows (Java `Hero.init`) into the live crown.
pub(crate) fn apply_heroes_loaded(
    world: &mut World,
    heroes: Vec<HeroRow>,
    diary: Vec<(i32, i64, i8, i32)>,
) {
    world.olympiad.heroes = heroes.iter().map(|h| (h.char_id, h.class_id)).collect();
    world.olympiad.hero_counts = heroes.iter().map(|h| (h.char_id, h.count)).collect();
    world.olympiad.claimed_heroes = heroes
        .iter()
        .filter(|h| h.claimed)
        .map(|h| h.char_id)
        .collect();
    world.olympiad.hero_info = heroes
        .iter()
        .map(|h| {
            (
                h.char_id,
                crate::model::olympiad::HeroInfo {
                    name: h.name.clone(),
                    clan_id: h.clan_id,
                    message: h.message.clone(),
                },
            )
        })
        .collect();
    // Group the diary entries by hero (already oldest-first from the query).
    let mut hero_diary: std::collections::HashMap<i32, Vec<crate::model::olympiad::DiaryEntry>> =
        std::collections::HashMap::new();
    for (char_id, time, action, param) in diary {
        hero_diary
            .entry(char_id)
            .or_default()
            .push(crate::model::olympiad::DiaryEntry {
                time,
                action,
                param,
            });
    }
    world.olympiad.hero_diary = hero_diary;
    tracing::info!("GameLoop: loaded {} Olympiad heroes.", heroes.len());
}

/// On enter-world, apply hero status to a crowned character (Java
/// `Player.setHero(Hero.isHero(objectId))` — crowned **and** claimed, so a
/// hero who has not visited the monument yet logs in without the status).
pub(crate) fn on_enter_world(world: &mut World, object_id: i32) {
    if world.olympiad.is_hero(object_id) {
        crate::game_loop::admin::hero::set_hero(world, object_id, true);
    }
}

/// Java `Hero.claimHero`: the crowned character collects the status — at the
/// Monument of Heroes, or through a GM's `//givehero`. Marks the crown claimed
/// (in memory and in `heroes.claimed`), pays the clan its reputation, grants
/// hero status/skills, plays the hero animation, and logs the deed in the diary.
///
/// The caller is responsible for the eligibility gate
/// ([`OlympiadState::is_unclaimed_hero`]); Java's `claimHero` itself would
/// happily crown a non-hero, and both of its call sites check first.
pub(crate) fn claim_hero(world: &mut World, object_id: i32) {
    world.olympiad.claimed_heroes.insert(object_id);
    let _ = world.db.send(DbCommand::ClaimHero { char_id: object_id });

    // "Clan member $c1 was named a hero. $s2 points have been added to your Clan
    // Reputation." — clan level 3+ only, and the reputation is the clan's, not
    // the hero's.
    let (clan_id, name) = world
        .objects
        .get_component::<Player>(&object_id)
        .map(|p| (p.clan_id, p.name.clone()))
        .unwrap_or((0, String::new()));
    let points = world.cfg.feature.hero_points;
    if clan_id != 0 && world.clans.get(&clan_id).is_some_and(|c| c.level >= 3) {
        crate::game_loop::clans::add_clan_reputation(world, clan_id, points);
        let sm = sp::system_message_with(
            sm_ids::CLAN_MEMBER_C1_WAS_NAMED_A_HERO_S2_POINTS_HAVE_BEEN_ADDED_TO_YOUR_CLAN_REPUTATION,
            &[SmParam::Text(name), SmParam::Int(points)],
        );
        for member in crate::game_loop::clans::online_members(world, clan_id) {
            crate::game_loop::helpers::send_to_player(world, member, sm.clone());
        }
    }

    crate::game_loop::admin::hero::set_hero(world, object_id, true);
    // `broadcastPacket(new SocialAction(objectId, 20016))` — the hero animation.
    crate::game_loop::helpers::broadcast_including_self(
        world,
        object_id,
        &sp::social_action(object_id, HERO_SOCIAL_ACTION),
    );
    crate::game_loop::player_info::broadcast_user_info(world, object_id);
    // `setHeroGained` — the diary's "gained hero" entry.
    let _ = world.db.send(DbCommand::SaveHeroDiary {
        char_id: object_id,
        time: commons::util::now_millis(),
        action: HERO_ACTION_GAINED_HERO,
        param: 0,
    });
}

/// `Olympiad.sortHerosToBe`: for each hero-title class (the `FOURTH_CLASS_GROUP`
/// category), the eligible noble with the most points becomes its hero. Eligible
/// = competitor on that class **or its parent 3rd class**, ≥ 10 matches, ≥ 1 win.
pub(crate) fn compute_heroes(world: &World) -> Vec<(i32, i32)> {
    let mut heroes = Vec::new();
    for hero_class in world.data.categories.ids("FOURTH_CLASS_GROUP") {
        let parent = world.data.skill_trees.parent_class(hero_class);
        let best = world
            .olympiad
            .nobles
            .iter()
            .filter(|(_, n)| {
                (n.class_id == hero_class || Some(n.class_id) == parent)
                    && n.comp_done >= world.cfg.olympiad.min_matches_for_points
                    && n.comp_won > 0
            })
            .max_by(|(_, a), (_, b)| {
                a.points
                    .cmp(&b.points)
                    .then(a.comp_done.cmp(&b.comp_done))
                    .then(a.comp_won.cmp(&b.comp_won))
            });
        if let Some((&char_id, _)) = best {
            heroes.push((char_id, hero_class));
        }
    }
    heroes
}

/// Java `Hero.showHeroDiary` (`_diary?class=<classId>&page=<n>`): render the
/// paginated notable-deeds log of the hero holding `classId`, in the clicked
/// NPC's window.
pub(crate) fn show_hero_diary(world: &mut World, client_id: u32, npc_oid: i32, args: &str) {
    const PER_PAGE: usize = 10;
    let class_id = query_param(args, "class").unwrap_or(0);
    let page = query_param(args, "page").unwrap_or(1).max(1) as usize;

    // Resolve the hero of that class.
    let Some(&(char_id, _)) = world
        .olympiad
        .heroes
        .iter()
        .find(|(_, cls)| *cls == class_id)
    else {
        return;
    };
    let Some(info) = world.olympiad.hero_info.get(&char_id).cloned() else {
        return;
    };
    let Some(template) = crate::data::htm_cache::read_htm_for_client(
        world,
        client_id,
        format!("{}data/html/olympiad/herodiary.htm", world.data.root),
    ) else {
        return;
    };

    // Entries newest-first; slice the requested page.
    let empty = Vec::new();
    let entries = world.olympiad.hero_diary.get(&char_id).unwrap_or(&empty);
    let total = entries.len();
    let mut list = String::new();
    let mut color = true;
    let start = (page - 1) * PER_PAGE;
    let mut last = start;
    for (i, entry) in entries.iter().rev().enumerate().skip(start).take(PER_PAGE) {
        last = i;
        let date = diary_date(entry.time);
        let action = diary_action_text(world, entry.action, entry.param);
        let bg = if color {
            "<table width=270 bgcolor=\"131210\">"
        } else {
            "<table width=270>"
        };
        list.push_str(&format!(
            "<tr><td>{bg}<tr><td width=270><font color=\"LEVEL\">{date}:xx</font></td></tr>\
             <tr><td width=270>{action}</td></tr><tr><td>&nbsp;</td></tr></table></td></tr>"
        ));
        color = !color;
    }

    // Pagination buttons (Java's prev = older page, next = newer page).
    let prev = if total > 0 && last < total - 1 {
        format!(
            "<button value=\"Prev\" action=\"bypass _diary?class={class_id}&page={}\" \
             width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
            page + 1
        )
    } else {
        String::new()
    };
    let next = if page > 1 {
        format!(
            "<button value=\"Next\" action=\"bypass _diary?class={class_id}&page={}\" \
             width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
            page - 1
        )
    } else {
        String::new()
    };

    let html = template
        .replace("%heroname%", &info.name)
        .replace("%message%", &info.message)
        .replace("%list%", &list)
        .replace("%buttprev%", &prev)
        .replace("%buttnext%", &next);
    send_to_client(world, client_id, sp::npc_html_message(npc_oid, &html));
}

/// Format one diary entry's action (Java `showHeroDiary`'s three `ACTION_*`
/// cases): 1 raid-killed (NPC name), 2 hero-gained, 3 castle-taken (castle name).
fn diary_action_text(world: &World, action: i8, param: i32) -> String {
    match action {
        1 => world
            .data
            .npc_data
            .get(param)
            .map(|t| format!("{} was defeated", t.name))
            .unwrap_or_default(),
        2 => "Gained Hero status".to_string(),
        3 => world
            .castles
            .iter()
            .find(|c| c.id == param)
            .map(|c| format!("{} Castle was successfuly taken", c.name))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Read an integer `key` from a `?a=1&b=2` query string.
fn query_param(args: &str, key: &str) -> Option<i32> {
    args.trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
}

/// Java `SimpleDateFormat("yyyy-MM-dd HH")` on the diary timestamp (UTC, like the
/// rest of the port). Hinnant's civil-from-days.
fn diary_date(millis: i64) -> String {
    let (y, m, d, hour, _, _) = commons::util::civil_from_millis(millis);
    format!("{y:04}-{m:02}-{d:02} {hour:02}")
}

/// Java `ExHeroList` (`Hero.getHeroes`): send the current heroes' roll — name,
/// class, and (resolved from the live clan registry) clan/ally names + crests +
/// the times-been-a-hero count.
pub(crate) fn send_hero_list(world: &World, client_id: u32) {
    let rows: Vec<sp::HeroListRow> = world
        .olympiad
        .heroes
        .iter()
        .map(|&(char_id, class_id)| {
            let info = world.olympiad.hero_info.get(&char_id);
            let name = info.map(|i| i.name.clone()).unwrap_or_default();
            let clan = info.and_then(|i| world.clans.get(&i.clan_id));
            let (clan_name, clan_crest) = clan
                .map(|c| (c.name.clone(), c.crest_id))
                .unwrap_or_default();
            let (ally_name, ally_crest) = clan
                .filter(|c| c.ally_id != 0)
                .map(|c| (c.ally_name.clone(), c.ally_crest_id))
                .unwrap_or_default();
            sp::HeroListRow {
                name,
                class_id,
                clan_name,
                clan_crest,
                ally_name,
                ally_crest,
                count: world
                    .olympiad
                    .hero_counts
                    .get(&char_id)
                    .copied()
                    .unwrap_or(0),
            }
        })
        .collect();
    send_to_client(world, client_id, sp::ex_hero_list(&rows));
}

#[cfg(test)]
mod diary_tests {
    use super::{diary_date, query_param};

    #[test]
    fn query_param_reads_class_and_page() {
        assert_eq!(query_param("?class=88&page=2", "class"), Some(88));
        assert_eq!(query_param("?class=88&page=2", "page"), Some(2));
        assert_eq!(query_param("?class=88", "page"), None);
    }

    #[test]
    fn diary_date_formats_utc_year_month_day_hour() {
        // 2024-01-01 00:00:00 UTC = epoch day 19723.
        let ms = 19723i64 * 86_400_000;
        assert_eq!(diary_date(ms), "2024-01-01 00");
        // + 13h30m → hour 13, same day.
        assert_eq!(
            diary_date(ms + 13 * 3_600_000 + 30 * 60_000),
            "2024-01-01 13"
        );
    }
}
