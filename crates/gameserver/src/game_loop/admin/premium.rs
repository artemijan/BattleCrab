//! Premium-account commands — `AdminPremium` (`//premium_menu` + the
//! `//premium_add1|add2|add3|info|remove <account>` subcommands). Ports the
//! `PremiumManager` account-premium store: an in-memory `World.premium` cache
//! (`account_name` lowercase → enddate millis) mirroring `_premiumData`, with
//! immediate write-through to `account_premium`. All operate on a typed account
//! name (online or offline) and re-render `premium_menu.htm` afterwards, like
//! the Java handler.

use crate::game_loop::helpers::send_to_client;
use crate::world::World;

use super::send_message;

/// `Config.PREMIUM_SYSTEM_ENABLED` — now read from
/// `config/Custom/PremiumSystem.ini` via [`crate::config::PremiumConfig`]
/// (G16 replaced the previously inlined `true` with the real loader).
pub(crate) fn premium_system_enabled(world: &World) -> bool {
    world.cfg.premium.enabled
}

/// Java `Player.hasPremiumStatus()` — `PREMIUM_SYSTEM_ENABLED && _premiumStatus`.
///
/// Java caches the flag on the `Player` at login (`PremiumManager` loads the
/// account's row then); this port keeps the whole `account_premium` table in
/// `World.premium` and resolves through the character's account name, so a
/// `//premium_add`/`//premium_remove` while the player is online takes effect
/// immediately rather than at next login.
pub(crate) fn has_premium_status(world: &World, object_id: i32) -> bool {
    if !premium_system_enabled(world) {
        return false;
    }
    let Some(p) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return false;
    };
    get_premium_expiration(world, &p.account) > commons::util::now_millis()
}

/// One day of premium in milliseconds — the unit `_bbspremium` grants (Java
/// `addPremiumTime(account, premiumDays, DAYS)`).
pub(crate) const DAY_MILLIS: i64 = 86_400_000;

/// One month of premium = 30 days (Java `addPremiumTime(account, months * 30,
/// DAYS)`), in milliseconds.
const MONTH_MILLIS: i64 = 30 * DAY_MILLIS;

/// The account a `//premium_*` subcommand acts on: the typed argument, or —
/// when none is given — the account of the GM's current target.
///
/// **This is a step past Java.** `AdminPremium` there takes an account name and
/// nothing else; with no argument it answers "Please enter a valid account
/// name" and stops. Selecting the character and clicking the menu button is the
/// obvious gesture, though, and the menu's own buttons pass no argument, so the
/// target is used as the fallback (GitHub #5). The error message is kept for
/// the case where there is no argument *and* no player targeted.
fn account_arg(world: &World, client_id: u32, object_id: i32, args: &[&str]) -> Option<String> {
    if let Some(&typed) = args.first() {
        return Some(typed.to_string());
    }
    let target = crate::game_loop::target::current(world, object_id)?;
    world
        .objects
        .get_component::<crate::model::Player>(&target)
        .map(|p| p.account.clone())
        .or_else(|| {
            send_message(world, client_id, "Please enter a valid account name.");
            None
        })
}

/// `AdminPremium.useAdminCommand` — route a `//premium_*` command and always
/// re-render the menu afterwards (Java sends the `premium_menu.htm`
/// `NpcHtmlMessage` at the end regardless of subcommand).
pub(super) fn admin_premium(
    world: &mut World,
    client_id: u32,
    object_id: i32,
    command: &str,
    args: &[&str],
) {
    match command {
        "admin_premium_add1" => add_premium(world, client_id, object_id, 1, args),
        "admin_premium_add2" => add_premium(world, client_id, object_id, 2, args),
        "admin_premium_add3" => add_premium(world, client_id, object_id, 3, args),
        "admin_premium_info" => view_premium(world, client_id, object_id, args),
        "admin_premium_remove" => remove_premium(world, client_id, object_id, args),
        // "admin_premium_menu" and anything else: just (re)show the menu.
        _ => {}
    }
    super::menu::show_admin_html(world, client_id, "premium_menu.htm");
}

/// `AdminPremium.addPremiumStatus` — grant `months` × 30 days of premium.
fn add_premium(world: &mut World, client_id: u32, object_id: i32, months: i64, args: &[&str]) {
    let Some(account) = account_arg(world, client_id, object_id, args) else {
        return;
    };
    let account = account.as_str();
    if !premium_system_enabled(world) {
        send_message(world, client_id, "Premium system is disabled.");
        return;
    }
    let enddate = add_premium_time(world, account, months * MONTH_MILLIS);
    send_message(
        world,
        client_id,
        &format!(
            "Account {account} will now have premium status until {}.",
            format_datetime(enddate)
        ),
    );
    // Java re-arms the PA-point timer for that account's online character, if
    // any. Note the `break`: the first match wins, so a dual-boxed account only
    // gets one of its characters re-armed. `pc_cafe::run` re-checks
    // `PC_CAFE_RETAIL_LIKE` itself.
    let online = world.clients.values().find_map(|cs| match cs {
        crate::session::ClientSession::InGame(s) => {
            let oid = s.player_object_id();
            world
                .objects
                .get_component::<crate::model::Player>(&oid)
                .filter(|p| p.account == account)
                .map(|_| oid)
        }
        _ => None,
    });
    if let Some(oid) = online {
        super::super::pc_cafe::run(world, oid);
    }
}

/// `AdminPremium.viewPremiumInfo`.
fn view_premium(world: &World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(account) = account_arg(world, client_id, object_id, args) else {
        return;
    };
    let account = account.as_str();
    if !premium_system_enabled(world) {
        send_message(world, client_id, "Premium system is disabled.");
        return;
    }
    let expiration = get_premium_expiration(world, account);
    if expiration > 0 {
        send_message(
            world,
            client_id,
            &format!(
                "Account {account} has premium status until {}.",
                format_datetime(expiration)
            ),
        );
    } else {
        send_message(
            world,
            client_id,
            &format!("Account {account} has no premium status."),
        );
    }
}

/// `AdminPremium.removePremium`.
fn remove_premium(world: &mut World, client_id: u32, object_id: i32, args: &[&str]) {
    let Some(account) = account_arg(world, client_id, object_id, args) else {
        return;
    };
    let account = account.as_str();
    if !premium_system_enabled(world) {
        send_message(world, client_id, "Premium system is disabled.");
        return;
    }
    if get_premium_expiration(world, account) > 0 {
        let key = account.to_lowercase();
        world.premium.remove(&key);
        let _ = world
            .db
            .send(crate::db::DbCommand::DeletePremium { account_name: key });
        send_message(
            world,
            client_id,
            &format!("Account {account} has no longer premium status."),
        );
    } else {
        send_message(
            world,
            client_id,
            &format!("Account {account} has no premium status."),
        );
    }
}

/// Java `PremiumManager.getPremiumExpiration` — the cached enddate, or 0.
fn get_premium_expiration(world: &World, account: &str) -> i64 {
    world
        .premium
        .get(&account.to_lowercase())
        .copied()
        .unwrap_or(0)
}

/// Java `PremiumManager.addPremiumTime` — extend from `max(now, current)` by
/// `duration` ms, cache it, and write through to `account_premium`. Returns the
/// new enddate.
pub(crate) fn add_premium_time(world: &mut World, account: &str, duration_millis: i64) -> i64 {
    let key = account.to_lowercase();
    let now = commons::util::now_millis();
    let new_end = now.max(get_premium_expiration(world, account)) + duration_millis;
    world.premium.insert(key.clone(), new_end);
    let _ = world.db.send(crate::db::DbCommand::StorePremium {
        account_name: key,
        enddate: new_end,
    });
    new_end
}

/// Format a UTC epoch-millis timestamp as `dd.MM.yyyy HH:mm` (Java uses
/// `SimpleDateFormat` in the server's local zone; UTC here, as no time-zone crate
/// is pulled in — a documented cosmetic deviation).
pub(crate) fn format_datetime(millis: i64) -> String {
    let (year, month, day, hour, minute, _) = commons::util::civil_from_millis(millis);
    format!("{day:02}.{month:02}.{year:04} {hour:02}:{minute:02}")
}

/// `handlers/voicedcommandhandlers/Premium` — the `.premium` account panel.
///
/// Two layouts, chosen by whether the account has time left: a "Normal" page
/// that advertises what premium *would* give, and a "Premium" page showing the
/// rates in force plus the expiry. Java builds both by hand in a
/// `StringBuilder` and sends them as `NpcHtmlMessage(5)`.
///
/// The rates shown are Java's arithmetic verbatim: the premium multiplier is
/// applied *on top of* the base rate (`RATE_XP * PREMIUM_RATE_XP`), so the page
/// reports the effective rate rather than the multiplier.
pub(crate) fn show_premium_panel(world: &World, client_id: u32, object_id: i32) {
    let Some(player) = world
        .objects
        .get_component::<crate::model::Player>(&object_id)
    else {
        return;
    };
    let end_date = get_premium_expiration(world, &player.account);
    let r = &world.cfg.rates;
    let p = &world.cfg.premium;
    // Java prints raw `double`s, so a 1.0 rate reads "x1.0" rather than "x1".
    let row = |label: &str, value: f64| {
        format!("<tr><td>{label}: <font color=\"LEVEL\"> x{value}<br1></font></td></tr>")
    };
    let premium_rows = format!(
        "{}{}{}{}{}{}",
        row("Rate XP", r.rate_xp * p.rate_xp),
        row("Rate SP", r.rate_sp * p.rate_sp),
        row(
            "Drop Chance",
            r.death_drop_chance_multiplier * p.rate_drop_chance
        ),
        row(
            "Drop Amount",
            r.death_drop_amount_multiplier * p.rate_drop_amount
        ),
        row(
            "Spoil Chance",
            r.spoil_drop_chance_multiplier * p.rate_spoil_chance
        ),
        row(
            "Spoil Amount",
            r.spoil_drop_amount_multiplier * p.rate_spoil_amount
        ),
    );
    const RULES: &str = concat!(
        "<tr><td> <font color=\"70FFCA\">1. Premium benefits CAN NOT BE TRANSFERED.<br1></font></td></tr>",
        "<tr><td> <font color=\"70FFCA\">2. Premium does not effect party members.<br1></font></td></tr>",
        "<tr><td> <font color=\"70FFCA\">3. Premium benefits effect ALL characters in same account.</font></td></tr>",
    );
    let html = if end_date == 0 {
        format!(
            "<html><body><title>Account Details</title><center><table>\
             <tr><td><center>Account Status: <font color=\"LEVEL\">Normal<br></font></td></tr>\
             {}{}{}{}{}{}\
             <tr><td><center>Premium Info &amp; Rules<br></td></tr>\
             {premium_rows}{RULES}</table></center></body></html>",
            row("Rate XP", r.rate_xp),
            row("Rate SP", r.rate_sp),
            row("Drop Chance", r.death_drop_chance_multiplier),
            row("Drop Amount", r.death_drop_amount_multiplier),
            row("Spoil Chance", r.spoil_drop_chance_multiplier),
            row("Spoil Amount", r.spoil_drop_amount_multiplier),
        )
    } else {
        format!(
            "<html><body><title>Premium Account Details</title><center><table>\
             <tr><td><center>Account Status: <font color=\"LEVEL\">Premium<br></font></td></tr>\
             {premium_rows}\
             <tr><td>Expires: <font color=\"00A5FF\">{}</font></td></tr>\
             <tr><td>Current Date: <font color=\"70FFCA\">{}<br><br></font></td></tr>\
             <tr><td><center>Premium Info &amp; Rules<br></center></td></tr>\
             {RULES}\
             <tr><td><center>Thank you for supporting our server.</td></tr>\
             </table></center></body></html>",
            format_datetime(end_date),
            format_datetime(commons::util::now_millis()),
        )
    };
    send_to_client(
        world,
        client_id,
        crate::network::server_packets::npc_html_message(5, &html),
    );
}

#[cfg(test)]
mod tests {
    use super::format_datetime;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(format_datetime(0), "01.01.1970 00:00");
        // 2021-01-01 00:00:00 UTC = 1609459200 s.
        assert_eq!(format_datetime(1_609_459_200_000), "01.01.2021 00:00");
        // 2021-03-01 13:45:00 UTC = 1614606300 s (leap-year boundary check).
        assert_eq!(format_datetime(1_614_606_300_000), "01.03.2021 13:45");
    }
}
