//! Birthday gifts — port of `taskmanager/tasks/TaskBirthday`.
//!
//! A character's `create_date` is its birthday, and on each anniversary the
//! server mails it a present. The task runs as part of the 06:30 daily beat
//! (Java registers it as `TYPE_GLOBAL_TASK` at exactly that time, which is the
//! slot [`crate::game_loop::upkeep::daily_tasks`] already owns).
//!
//! Three things about it are worth knowing before reading the code:
//!
//! * **It catches up.** Java walks day by day from the task's last activation
//!   to today, so a server that was down over a birthday still sends the gift
//!   when it comes back. That makes this the first reader of the
//!   `DAILY_TASK_RESET` stamp the daily reset has been writing all along.
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

/// `TaskBirthday.onTimeElapsed` — ask the DB for today's birthdays, plus any
/// day missed since the last reset.
///
/// The reply arrives as [`crate::db::DbEvent::BirthdaysLoaded`].
pub(crate) fn check_birthdays(world: &mut World) {
    let now = commons::util::now_millis();
    let last = super::global_vars::get_i64(world, super::global_vars::DAILY_TASK_RESET, 0);
    let days = days_to_check(last, now);
    if days.is_empty() {
        return;
    }
    let _ = world.db.send(DbCommand::LoadBirthdays { days });
}

/// The `MM-DD` keys to look for, one per day from the last activation through
/// today (Java's `for (; !TODAY.before(lastExecDate); lastExecDate.add(DATE,
/// 1))`), each tagged with the year whose anniversary it is.
///
/// A `last` of 0 — a server that has never stamped a reset — checks today
/// alone, which is what Java's `lastActivation > 0` guard amounts to.
fn days_to_check(last: i64, now: i64) -> Vec<BirthdayDay> {
    const MILLIS_PER_DAY: i64 = 86_400_000;
    let today = now.div_euclid(MILLIS_PER_DAY);
    let first = if last > 0 {
        last.div_euclid(MILLIS_PER_DAY).min(today)
    } else {
        today
    };
    let mut out = Vec::new();
    for day in first..=today {
        let (year, month, dom) = commons::util::civil_from_days(day);
        out.push(BirthdayDay {
            month_day: format!("{month:02}-{dom:02}"),
            year: year as i32,
        });
        // "If character birthday is 29-Feb and year isn't leap, send gift on
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

/// `TaskBirthday.checkBirthday`'s loop body: one gift mail per matching
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

    /// With no stored stamp — a server that has never run a daily reset —
    /// only today is checked.
    #[test]
    fn a_first_run_checks_today_alone() {
        let days = days_to_check(0, at(2026, 8, 19));
        assert_eq!(keys(&days), ["08-19"]);
        assert_eq!(days[0].year, 2026);
    }

    /// Java walks from the last activation **through** today, so days the
    /// server was down are not skipped — the gift is late, not lost.
    #[test]
    fn a_missed_run_is_caught_up_day_by_day() {
        let days = days_to_check(at(2026, 8, 16), at(2026, 8, 19));
        assert_eq!(keys(&days), ["08-16", "08-17", "08-18", "08-19"]);
    }

    /// A stamp from later than "now" (a clock moved backwards) must not run
    /// the loop backwards; Java's `!TODAY.before(lastExecDate)` stops it too.
    #[test]
    fn a_stamp_in_the_future_still_checks_today() {
        let days = days_to_check(at(2026, 9, 1), at(2026, 8, 19));
        assert_eq!(keys(&days), ["08-19"]);
    }

    /// "If character birthday is 29-Feb and year isn't leap, send gift on
    /// 28-feb" — the 29th is checked alongside the 28th, and only then.
    #[test]
    fn a_leap_day_birthday_is_paid_on_the_28th_in_a_common_year() {
        assert_eq!(keys(&days_to_check(0, at(2027, 2, 28))), ["02-28", "02-29"]);
        // 2028 is a leap year: the 29th arrives on its own, so the 28th is
        // just the 28th.
        assert_eq!(keys(&days_to_check(0, at(2028, 2, 28))), ["02-28"]);
        assert_eq!(keys(&days_to_check(0, at(2028, 2, 29))), ["02-29"]);
        // 2100 is divisible by 100 but not 400 — not a leap year.
        assert_eq!(keys(&days_to_check(0, at(2100, 2, 28))), ["02-28", "02-29"]);
    }
}
