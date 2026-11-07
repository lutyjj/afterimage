//! Which playlist entry a device shows at a check-in, whether it sleeps, and
//! when it checks in next.
//!
//! A playlist entry and a sleep window are current on their days, from their
//! `from` time until their `until` time, in the server's time zone. One whose
//! `until` comes before its `from` runs past midnight and belongs to the day it
//! starts; one whose times are equal is current all day. Each check-in shows the
//! current entry after the one shown last, in playlist order, and starts over at
//! the first. A device sleeps while any of its sleep windows is current.

use std::cmp::Ordering;

use jiff::civil::Weekday;
use jiff::{ToSpan, Zoned};

use crate::devices::{Entry, Hours};

const DAY_MINUTES: u16 = 24 * 60;

/// The longest a device sleeps at once, so one asleep around the clock still
/// checks in, and takes a change, once a day.
pub const LONGEST_SLEEP_MINUTES: i64 = 24 * 60;

/// How often a device held asleep outside its windows checks in, to learn of
/// its release: it cannot be woken from the server.
pub const HELD_SLEEP_MINUTES: i64 = 60;

fn day_bit(weekday: Weekday) -> u8 {
    1 << weekday.to_monday_zero_offset()
}

fn minute_of_day(at: &Zoned) -> u16 {
    u16::try_from(i32::from(at.hour()) * 60 + i32::from(at.minute())).unwrap_or(0)
}

/// Whether the hours include `at`.
#[must_use]
pub fn is_current(hours: &Hours, at: &Zoned) -> bool {
    let on = |weekday: Weekday| hours.days.is_none_or(|days| days & day_bit(weekday) != 0);
    let from = hours.from_minute.unwrap_or(0);
    let until = hours.until_minute.unwrap_or(DAY_MINUTES);
    let now = minute_of_day(at);
    match from.cmp(&until) {
        Ordering::Equal => on(at.weekday()),
        Ordering::Less => on(at.weekday()) && from <= now && now < until,
        Ordering::Greater => {
            (on(at.weekday()) && now >= from) || (on(at.weekday().previous()) && now < until)
        }
    }
}

/// Whether the device sleeps at `at`.
#[must_use]
pub fn asleep(windows: &[Hours], at: &Zoned) -> bool {
    windows.iter().any(|window| is_current(window, at))
}

fn current(playlist: &[Entry], at: &Zoned) -> Vec<usize> {
    (0..playlist.len())
        .filter(|&position| is_current(&playlist[position].hours, at))
        .collect()
}

/// The position to show at `at`: the first current one after `last`, or else the
/// first current one; `None` when no entry is current.
#[must_use]
pub fn next(playlist: &[Entry], last: Option<usize>, at: &Zoned) -> Option<usize> {
    let current = current(playlist, at);
    current
        .iter()
        .copied()
        .find(|&position| last.is_none_or(|last| position > last))
        .or_else(|| current.first().copied())
}

/// Seconds until the device should check in again. In a sleep window, until no
/// window is current, a day at most. Held asleep outside them, an hour. Awake,
/// its refresh interval, or less when the set of current entries of `playlist`
/// changes or it falls asleep sooner; a device pinned to one screen passes no
/// playlist.
#[must_use]
pub fn wait(
    playlist: &[Entry],
    sleep: &[Hours],
    held: bool,
    refresh_minutes: i64,
    at: &Zoned,
) -> i64 {
    if asleep(sleep, at) {
        until_change(at, LONGEST_SLEEP_MINUTES, |later| asleep(sleep, later))
    } else if held {
        HELD_SLEEP_MINUTES * 60
    } else {
        until_change(at, refresh_minutes.max(1), |later| {
            (current(playlist, later), asleep(sleep, later))
        })
    }
}

/// Seconds from `at` to the first whole minute, at most `limit` minutes on,
/// where `state` differs from what it is at `at`; the limit when it never does.
fn until_change<T: PartialEq>(at: &Zoned, limit: i64, state: impl Fn(&Zoned) -> T) -> i64 {
    let now = state(at);
    let minute = at
        .round(
            jiff::ZonedRound::new()
                .smallest(jiff::Unit::Minute)
                .mode(jiff::RoundMode::Trunc),
        )
        .unwrap_or_else(|_| at.clone());
    for step in 1..=limit {
        let Ok(later) = minute.checked_add(step.minutes()) else {
            break;
        };
        if state(&later) != now {
            let seconds = later.timestamp().as_second() - at.timestamp().as_second();
            return seconds.max(1);
        }
    }
    limit * 60
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;

    use super::*;

    /// Wednesday, 2026-11-04, at `hh:mm` in Berlin.
    fn wednesday(time: &str) -> Zoned {
        format!("2026-11-04T{time}[Europe/Berlin]").parse().unwrap()
    }

    fn hours(days: Option<u8>, from: Option<u16>, until: Option<u16>) -> Hours {
        Hours {
            days,
            from_minute: from,
            until_minute: until,
        }
    }

    fn entry(days: Option<u8>, from: Option<u16>, until: Option<u16>) -> Entry {
        Entry {
            screen_id: 1,
            hours: hours(days, from, until),
        }
    }

    const WEDNESDAY: u8 = 1 << 2;
    const TUESDAY: u8 = 1 << 1;

    #[test]
    fn an_entry_without_days_or_times_is_always_current() {
        assert!(is_current(&hours(None, None, None), &wednesday("00:00")));
        assert!(is_current(&hours(None, None, None), &wednesday("23:59")));
    }

    #[test]
    fn times_are_a_half_open_range_of_the_day() {
        let morning = hours(Some(WEDNESDAY), Some(7 * 60), Some(9 * 60 + 30));
        assert!(!is_current(&morning, &wednesday("06:59")));
        assert!(is_current(&morning, &wednesday("07:00")));
        assert!(is_current(&morning, &wednesday("09:29")));
        assert!(!is_current(&morning, &wednesday("09:30")));
        assert!(!is_current(
            &hours(Some(TUESDAY), Some(7 * 60), None),
            &wednesday("08:00")
        ));
    }

    #[test]
    fn a_range_past_midnight_belongs_to_the_day_it_starts() {
        let tuesday_night = hours(Some(TUESDAY), Some(22 * 60), Some(2 * 60));
        assert!(is_current(&tuesday_night, &wednesday("01:59")));
        assert!(!is_current(&tuesday_night, &wednesday("02:00")));
        assert!(
            !is_current(&tuesday_night, &wednesday("23:00")),
            "Wednesday night is not Tuesday's"
        );
    }

    #[test]
    fn equal_times_are_the_whole_day() {
        let all_day = hours(Some(WEDNESDAY), Some(8 * 60), Some(8 * 60));
        assert!(is_current(&all_day, &wednesday("03:00")));
        assert!(!is_current(
            &hours(Some(TUESDAY), Some(8 * 60), Some(8 * 60)),
            &wednesday("03:00")
        ));
    }

    #[test]
    fn rotates_through_the_current_entries_and_starts_over() {
        let playlist = [
            entry(None, None, None),
            entry(Some(TUESDAY), None, None),
            entry(None, None, None),
        ];
        let at = wednesday("12:00");
        assert_eq!(next(&playlist, None, &at), Some(0));
        assert_eq!(
            next(&playlist, Some(0), &at),
            Some(2),
            "skips Tuesday's entry"
        );
        assert_eq!(next(&playlist, Some(2), &at), Some(0));
        assert_eq!(next(&[entry(Some(TUESDAY), None, None)], None, &at), None);
    }

    #[test]
    fn wakes_at_the_next_change_when_it_comes_before_the_refresh() {
        let playlist = [entry(None, Some(12 * 60 + 10), Some(13 * 60))];
        let at: Zoned = "2026-11-04T12:00:30[Europe/Berlin]".parse().unwrap();
        assert_eq!(wait(&playlist, &[], false, 15, &at), 9 * 60 + 30);
        assert_eq!(
            wait(&playlist, &[], false, 5, &at),
            5 * 60,
            "the refresh comes first"
        );
        assert_eq!(wait(&[], &[], false, 15, &at), 15 * 60);
    }

    #[test]
    fn counts_real_seconds_across_a_daylight_saving_change() {
        // Berlin leaves summer time at 03:00 on 2026-10-25, repeating 02:00 to 03:00.
        let playlist = [entry(None, Some(3 * 60), None)];
        // 2026-10-25T00:00:00Z is 02:00 summer time in Berlin.
        let at = jiff::Timestamp::from_second(1_792_886_400)
            .unwrap()
            .to_zoned(TimeZone::get("Europe/Berlin").unwrap());
        assert_eq!(at.hour(), 2);
        assert_eq!(
            wait(&playlist, &[], false, 24 * 60, &at),
            2 * 60 * 60,
            "two real hours to 03:00 winter time"
        );
    }

    #[test]
    fn sleeps_while_a_window_is_current() {
        let nights = [hours(None, Some(23 * 60), Some(7 * 60))];
        assert!(asleep(&nights, &wednesday("23:30")));
        assert!(asleep(&nights, &wednesday("06:59")));
        assert!(!asleep(&nights, &wednesday("07:00")));
        assert!(!asleep(&[], &wednesday("03:00")));
    }

    #[test]
    fn sleeps_until_no_window_is_current() {
        // Two windows that meet: 23:00 to 01:00, then 01:00 to 07:00.
        let nights = [
            hours(None, Some(23 * 60), Some(60)),
            hours(None, Some(60), Some(7 * 60)),
        ];
        let at: Zoned = "2026-11-04T23:00:00[Europe/Berlin]".parse().unwrap();
        assert_eq!(
            wait(&[entry(None, None, None)], &nights, false, 15, &at),
            8 * 60 * 60,
            "through both windows, past the refresh"
        );
    }

    #[test]
    fn sleeps_a_day_at_most() {
        let always = [hours(None, None, None)];
        let at = wednesday("12:00");
        assert_eq!(
            wait(&[], &always, false, 15, &at),
            LONGEST_SLEEP_MINUTES * 60
        );
    }

    #[test]
    fn wakes_in_time_to_fall_asleep() {
        let nights = [hours(None, Some(23 * 60), Some(7 * 60))];
        let at: Zoned = "2026-11-04T22:50:00[Europe/Berlin]".parse().unwrap();
        assert_eq!(
            wait(&[entry(None, None, None)], &nights, false, 60, &at),
            10 * 60,
            "at 23:00, before its hour is up"
        );
    }

    #[test]
    fn a_device_held_asleep_checks_in_hourly_outside_its_windows() {
        let nights = [hours(None, Some(23 * 60), Some(7 * 60))];
        let playlist = [entry(None, None, None)];
        assert_eq!(
            wait(&playlist, &nights, true, 15, &wednesday("12:00")),
            HELD_SLEEP_MINUTES * 60
        );
        let night: Zoned = "2026-11-04T23:00:00[Europe/Berlin]".parse().unwrap();
        assert_eq!(
            wait(&playlist, &nights, true, 15, &night),
            8 * 60 * 60,
            "in a window it sleeps the window through, held or not"
        );
    }
}
