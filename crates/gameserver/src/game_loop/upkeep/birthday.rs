//! Birthday gifts — the mail half of Salvation's `events/CharacterBirthday`.
//!
//! A character's `create_date` is its birthday, and on each anniversary the
//! server mails it a present. Salvation runs this from the script's
//! `ON_DAILY_RESET` listener, so it rides the 06:30 beat in
//! [`crate::game_loop::upkeep::daily_tasks`]. Interlude ran it as a separate
//! `TaskBirthday` global task, caught up from the last activation stamp; that
//! task and its `global_tasks` table are gone.
//!
//! Four things about it are worth knowing before reading the code:
//!
//! * **It looks back 24 hours, not to the last reset.** Java starts from
//!   `now - 24 h` and walks through today, so it checks yesterday and today.
//!   A day the server was down over is not caught up.
//! * **So every anniversary is checked twice**, once by each of two
//!   consecutive resets, and a character gets two gifts. That is Java's
//!   behaviour and is kept.
//! * **The recipient does not have to be online** — the gift is a mail row, and
//!   the query is over the `characters` table rather than over the world.
//! * **The age is measured against the day being checked**, not against the
//!   creation date's own year, and a character created *this* year gets
//!   nothing (`age <= 0` → skip).
//!
//! Java's one calendar quirk is ported with it: a 29-February character is
//! given its gift on the 28th in non-leap years.

use crate::db::{BirthdayDay, BirthdayMatch, DbCommand};
use crate::model::mail::{MailType, Message};
use crate::world::World;

/// `CharacterBirthday.onDailyReset` — ask the DB for the birthdays of
/// yesterday and today.
///
/// The reply arrives as [`crate::db::DbEvent::BirthdaysLoaded`].
pub(crate) fn check_birthdays(world: &mut World) {
    let days = days_to_check(commons::util::now_millis());
    let _ = world.db.send(DbCommand::LoadBirthdays { days });
}

/// The `MM-DD` keys to look for, each tagged with the year whose anniversary
/// it is. Java's loop (`lastExecDate = now - 24 h; for (; !today.before(
/// lastExecDate); lastExecDate.add(DATE, 1))`) visits the day 24 hours ago and
/// then today.
fn days_to_check(now: i64) -> Vec<BirthdayDay> {
    const MILLIS_PER_DAY: i64 = 86_400_000;
    let today = now.div_euclid(MILLIS_PER_DAY);
    let mut out = Vec::new();
    for day in [today - 1, today] {
        let (year, month, dom) = commons::util::civil_from_days(day);
        out.push(BirthdayDay {
            month_day: format!("{month:02}-{dom:02}"),
            year: year as i32,
        });
        // "If character birthday is 29-Feb and year is not leap, send gift on
        // 28-feb" — Java checks the 29th as well while standing on the 28th.
        if month == 2 && dom == 28 && !is_leap_year(year) {
            out.push(BirthdayDay {
                month_day: "02-29".to_string(),
                year: year as i32,
            });
        }
    }
    out
}

/// The Gregorian rule, as `GregorianCalendar.isLeapYear` applies it here.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// `CharacterBirthday.checkBirthday`'s loop body: one gift mail per matching
/// character, skipping the ones whose "anniversary" is their creation day.
pub(crate) fn apply_loaded(world: &mut World, rows: Vec<BirthdayMatch>) {
    let mut sent = 0;
    for row in rows {
        let Some(created_year) = row
            .create_date
            .split('-')
            .next()
            .and_then(|y| y.parse::<i32>().ok())
        else {
            continue;
        };
        let age = row.year - created_year;
        if age <= 0 {
            continue;
        }
        if send_gift(world, row.char_id, &row.name, age) {
            sent += 1;
        }
    }
    if sent > 0 {
        tracing::info!("BirthdayManager: {sent} gift(s) sent.");
    }
}

/// The message itself: subject and text from `General.ini`, `$c1`/`$s1`
/// substituted, and the gift as its single attachment.
fn send_gift(world: &mut World, char_id: i32, name: &str, age: i32) -> bool {
    let (subject, gift_id) = (
        world.cfg.general.alt_birthday_mail_subject.clone(),
        world.cfg.general.alt_birthday_gift,
    );
    let text = world
        .cfg
        .general
        .alt_birthday_mail_text
        .replace("$c1", name)
        .replace("$s1", &age.to_string());
    let Some(message_id) = world.alloc_object_id() else {
        return false;
    };
    let mut msg = Message::new_system_mail(
        message_id,
        char_id,
        subject,
        text,
        MailType::Birthday,
        commons::util::now_millis(),
    );
    // Java always attaches: `createAttachments()` then `addItem`.
    msg.has_attachments = true;
    world.mail.insert(msg);
    let Some(object_id) = world.alloc_object_id() else {
        return false;
    };
    let catalog = &world.data.item_data;
    world
        .mail
        .attachments
        .entry(message_id)
        .or_default()
        .insert_instance(catalog, object_id, gift_id, 1, 0, -1);
    crate::game_loop::mail::persist_message(world, message_id);
    crate::game_loop::mail::persist_attachments(world, message_id);
    // An online recipient gets the chime and the badge; an offline one finds
    // the mail waiting at login, which `mail::on_enter_world` sends then.
    crate::game_loop::helpers::send_to_player(
        world,
        char_id,
        crate::network::server_packets::ex_notice_post_arrived(true),
    );
    crate::game_loop::mail::send_unread_count(world, char_id);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    /// Epoch millis for a UTC date, so the calendar cases read as dates.
    fn at(year: i64, month: i64, day: i64) -> i64 {
        // days_from_civil, the inverse of `civil_from_days`.
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (month + 9) % 12;
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        (era * 146_097 + doe - 719_468) * DAY
    }

    fn keys(days: &[BirthdayDay]) -> Vec<String> {
        days.iter().map(|d| d.month_day.clone()).collect()
    }

    /// The day 24 hours back, then today.
    #[test]
    fn yesterday_and_today_are_checked() {
        let days = days_to_check(at(2026, 8, 19) + 6 * 3_600_000);
        assert_eq!(keys(&days), ["08-18", "08-19"]);
        assert!(days.iter().all(|d| d.year == 2026));
    }

    /// Across new year, each day keeps its own year, so the age is right.
    #[test]
    fn each_day_carries_its_own_year() {
        let days = days_to_check(at(2027, 1, 1));
        assert_eq!(keys(&days), ["12-31", "01-01"]);
        assert_eq!((days[0].year, days[1].year), (2026, 2027));
    }

    /// "If character birthday is 29-Feb and year is not leap, send gift on
    /// 28-feb" — the 29th is checked alongside the 28th, and only then.
    #[test]
    fn a_leap_day_birthday_is_paid_on_the_28th_in_a_common_year() {
        assert_eq!(
            keys(&days_to_check(at(2027, 2, 28))),
            ["02-27", "02-28", "02-29"]
        );
        // 2028 is a leap year: the 29th arrives on its own, so the 28th is
        // just the 28th.
        assert_eq!(keys(&days_to_check(at(2028, 2, 28))), ["02-27", "02-28"]);
        assert_eq!(keys(&days_to_check(at(2028, 2, 29))), ["02-28", "02-29"]);
        // 2100 is divisible by 100 but not 400 — not a leap year.
        assert_eq!(
            keys(&days_to_check(at(2100, 2, 28))),
            ["02-27", "02-28", "02-29"]
        );
    }
}
