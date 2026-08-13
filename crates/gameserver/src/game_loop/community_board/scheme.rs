//! The scheme buffer: create/delete/apply named buff schemes and their
//! rendering.

use super::*;
/// `HomeBoard`'s `_bbs_buff_scheme_*` branch: create a scheme from the player's
/// active buffs, delete one, or execute (re-cast) one, then re-render the
/// return page with any validation error banner. The bypass carries
/// space-separated args: `<cmd> <name> <returnPath> [self|pet]`.
pub(super) fn do_scheme(world: &mut World, client_id: u32, object_id: i32, command: &str) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    // Return page: `parts[2]` when present, else `parts[1]` (Java `parts.length
    // < 3`); only if it names an html.
    let return_path = if parts.len() < 3 {
        parts.get(1)
    } else {
        parts.get(2)
    }
    .copied()
    .filter(|p| p.ends_with(".html"));

    // Java loads the return html first, runs the command (which may set an error
    // message), then re-renders — so we always serve the return page.
    let error = run_scheme_command(world, client_id, object_id, &parts)
        .err()
        .unwrap_or_default();
    serve_page(world, client_id, object_id, return_path, &error);
}

/// Port of `HomeBoard.parseSchemeNameOrError` + the create/delete/execute
/// dispatch. `Err(msg)` becomes the `%errorMessage%` banner.
pub(super) fn run_scheme_command(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    parts: &[&str],
) -> Result<(), String> {
    if parts.len() < 3 {
        return Err("Please enter scheme name.".to_string());
    }
    let command_name = parts[0];
    let scheme_name = parts[1];
    if scheme_name.chars().count() > 14 {
        return Err("Scheme's name must contain up to 14 chars.".to_string());
    }
    if !is_alphanumeric(scheme_name) {
        return Err("Please use plain alphanumeric characters.".to_string());
    }
    if command_name == "_bbs_buff_scheme_create"
        && let Some(schemes) = world.buffer_schemes.get(&object_id)
    {
        if schemes.len() >= MAX_SCHEMES {
            return Err("Maximum schemes amount is already reached.".to_string());
        }
        if schemes
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(scheme_name))
        {
            return Err("The scheme name already exists.".to_string());
        }
    }

    match command_name {
        "_bbs_buff_scheme_create" => scheme_create(world, object_id, scheme_name),
        "_bbs_buff_scheme_delete" => {
            scheme_delete(world, object_id, scheme_name);
            Ok(())
        }
        "_bbs_buff_scheme_execute" => {
            let is_pet = parts.get(3) == Some(&"pet");
            apply_scheme(world, client_id, object_id, scheme_name, is_pet)
        }
        _ => Ok(()),
    }
}

/// Java create branch: snapshot the player's currently-active whitelisted buffs
/// into a new scheme, write it through to `buffer_schemes`.
pub(super) fn scheme_create(
    world: &mut World,
    object_id: i32,
    scheme_name: &str,
) -> Result<(), String> {
    let buffs: Vec<i32> = world
        .objects
        .get_component::<crate::model::components::Buffs>(&object_id)
        .map(|b| {
            b.0.iter()
                .map(|a| a.skill_id)
                .filter(|id| world.cfg.community_board.available_buffs.contains(id))
                .collect()
        })
        .unwrap_or_default();
    if buffs.is_empty() {
        return Err("You don't have any buffs applied.".to_string());
    }
    let skills = buffs
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    world
        .buffer_schemes
        .entry(object_id)
        .or_default()
        .push((scheme_name.to_string(), buffs));
    let _ = world.db.send(crate::db::DbCommand::StoreBufferScheme {
        object_id,
        scheme_name: scheme_name.to_string(),
        skills,
    });
    Ok(())
}

/// Java `removeScheme` + the shutdown save collapse into an immediate delete.
pub(super) fn scheme_delete(world: &mut World, object_id: i32, scheme_name: &str) {
    if let Some(schemes) = world.buffer_schemes.get_mut(&object_id) {
        schemes.retain(|(n, _)| !n.eq_ignore_ascii_case(scheme_name));
    }
    let _ = world.db.send(crate::db::DbCommand::DeleteBufferScheme {
        object_id,
        scheme_name: scheme_name.to_string(),
    });
}

/// Port of `HomeBoard.applyBuffs`: re-cast every skill in a scheme onto the
/// player, at the level from the buffer's available-buff table.
pub(crate) fn apply_scheme(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    scheme_name: &str,
    is_pet: bool,
) -> Result<(), String> {
    let scheme: Vec<i32> = world
        .buffer_schemes
        .get(&object_id)
        .and_then(|s| s.iter().find(|(n, _)| n.eq_ignore_ascii_case(scheme_name)))
        .map(|(_, skills)| skills.clone())
        .unwrap_or_default();

    // The "Pet" button buffs the player's summon (Java's `player.getPet()` /
    // `getServitors()`); with no summon it lands on the "no pet" branch. The
    // player still pays, so the cost checks below stay keyed to `object_id`.
    let target = if is_pet {
        match crate::game_loop::servitor::pet_of(world, object_id)
            .or_else(|| crate::game_loop::servitor::servitor_of(world, object_id))
        {
            Some(summon) => summon,
            None => return Err("You don't have a pet.".to_string()),
        }
    } else {
        object_id
    };

    let buff_price = world.cfg.community_board.buff_price;
    let cost = if buff_price > 0 {
        buff_price * scheme.len() as i64
    } else {
        0
    };
    // NOTE: Java's guard is `(cost == 0) || inventoryCount < cost` — an inverted
    // check that applies the scheme for free (dist `BuffPrice = 0`) and, were the
    // price ever positive, would refuse only when the player CAN pay. Ported
    // faithfully ("dist data is the spec"); with the dist price of 0, `cost` is
    // always 0 so the buffs always apply.
    let currency = world.cfg.community_board.currency_id;
    let have = world
        .objects
        .get_component::<Inventory>(&object_id)
        .map(|inv| inv.count_of(currency))
        .unwrap_or(0);
    if !(cost == 0 || have < cost) {
        return Err("You don't have enough items for this action.".to_string());
    }
    if cost > 0 {
        // Java `destroyItemByItemId("CB_Buff", CURRENCY, cost, …)` — a no-op in
        // dist; best-effort, never blocks the (already faithful) apply.
        charge(world, client_id, object_id, cost);
    }

    for skill_id in &scheme {
        if !world.cfg.community_board.available_buffs.contains(skill_id) {
            continue;
        }
        let Some(level) = world.data.scheme_buffer.level_of(*skill_id) else {
            continue;
        };
        let Some(skill) = skill_by_id(world, *skill_id, level) else {
            warn!("CommunityBoard: scheme buff {skill_id}/{level} missing from skill data.");
            continue;
        };
        crate::game_loop::skills::effects::apply_skill_effects(world, object_id, target, &skill);
    }
    Ok(())
}

/// Java `Util.isAlphaNumeric` — non-empty and every char a letter or digit.
pub(super) fn is_alphanumeric(s: &str) -> bool {
    !s.is_empty() && s.chars().all(char::is_alphanumeric)
}

/// The `%schemenames%` block: the player's scheme rows, or Java's empty-state
/// line when the player has no schemes registered (`getPlayerSchemes == null`).
pub(super) fn render_scheme_names(world: &World, object_id: i32) -> String {
    match world.buffer_schemes.get(&object_id) {
        Some(schemes) => build_scheme_html(schemes),
        None => {
            "No buffer schemes yet, please make sure you have buffs and then click Create Scheme."
                .to_string()
        }
    }
}

/// Java `HomeBoard.buildBufferSchemesHtml`: one execute/pet/delete button row
/// per scheme, names sorted case-insensitively (Java iterates a
/// `TreeMap(CASE_INSENSITIVE_ORDER)`).
pub(super) fn build_scheme_html(schemes: &[(String, Vec<i32>)]) -> String {
    const ROW: &str = concat!(
        "<td><button value=\"%schemename%\" action=\"bypass _bbs_buff_scheme_execute %schemename% buffer/schemes.html self\" height=\"26\" width=\"130\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
        "<td><button value=\"%schemename% (Pet)\" action=\"bypass _bbs_buff_scheme_execute %schemename% buffer/schemes.html pet\" height=\"26\" width=\"130\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
        "<td><button value=\"X\" action=\"bypass _bbs_buff_scheme_delete %schemename% buffer/schemes.html\" height=\"26\" width=\"26\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\" /></td>",
    );
    let mut names: Vec<&str> = schemes.iter().map(|(n, _)| n.as_str()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    let mut out = String::from("<table align=\"center\">");
    for name in names {
        out.push_str("<tr>");
        out.push_str(&ROW.replace("%schemename%", name));
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}
