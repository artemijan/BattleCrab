//! Premium-account commands — `AdminPremium` (`//premium_menu` + the
//! `//premium_add1|add2|add3|info|remove <account>` subcommands). Ports the
//! `PremiumManager` account-premium store: an in-memory `World.premium` cache
//! (`account_name` lowercase → enddate millis) mirroring `_premiumData`, with
//! immediate write-through to `account_premium`. All operate on a typed account
//! name (online or offline) and re-render `premium_menu.htm` afterwards, like
//! the Java handler.

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

/// `AdminPremium.useAdminCommand` — route a `//premium_*` command and always
/// re-render the menu afterwards (Java sends the `premium_menu.htm`
/// `NpcHtmlMessage` at the end regardless of subcommand).
pub(super) fn admin_premium(world: &mut World, client_id: u32, command: &str, args: &[&str]) {
    match command {
        "admin_premium_add1" => add_premium(world, client_id, 1, args),
        "admin_premium_add2" => add_premium(world, client_id, 2, args),
        "admin_premium_add3" => add_premium(world, client_id, 3, args),
        "admin_premium_info" => view_premium(world, client_id, args),
        "admin_premium_remove" => remove_premium(world, client_id, args),
        // "admin_premium_menu" and anything else: just (re)show the menu.
        _ => {}
    }
    super::menu::show_admin_html(world, client_id, "premium_menu.htm");
}

/// `AdminPremium.addPremiumStatus` — grant `months` × 30 days of premium.
fn add_premium(world: &mut World, client_id: u32, months: i64, args: &[&str]) {
    let Some(&account) = args.first() else {
        send_message(world, client_id, "Please enter a valid account name.");
        return;
    };
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
    // TODO(G16): Java also runs PcCafePointsManager.run(player) here when
    // Config.PC_CAFE_RETAIL_LIKE and the account is online; that manager is
    // unported.
}

/// `AdminPremium.viewPremiumInfo`.
fn view_premium(world: &World, client_id: u32, args: &[&str]) {
    let Some(&account) = args.first() else {
        send_message(world, client_id, "Please enter a valid account name.");
        return;
    };
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
fn remove_premium(world: &mut World, client_id: u32, args: &[&str]) {
    let Some(&account) = args.first() else {
        send_message(world, client_id, "Please enter a valid account name.");
        return;
    };
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
    let secs = millis.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (hour, minute) = (tod / 3600, (tod % 3600) / 60);

    let (year, month, day) = commons::util::civil_from_days(days);
    format!("{day:02}.{month:02}.{year:04} {hour:02}:{minute:02}")
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
