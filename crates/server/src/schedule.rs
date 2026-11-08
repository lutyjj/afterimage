//! When a schedule slot or a sleep window is current, which playlist a device's
//! schedule calls for, which of its screens comes next, whether the device
//! sleeps, and when it checks in next.
//!
//! A slot and a sleep window are current on their days, from their `from` time
//! until their `until` time, in the server's time zone. One whose `until` comes
//! before its `from` runs past midnight and belongs to the day it starts; one
//! whose times are equal is current all day. A device shows the playlist of the
//! slot that is current, or else its usual one; no two of its slots are current
//! at once. Each check-in shows the playlist's screen after the one shown last,
//! and starts over at the first. A device sleeps while any of its sleep windows
//! is current.

use std::cmp::Ordering;

use jiff::civil::Weekday;
use jiff::{ToSpan, Zoned};

const DAY_MINUTES: u16 = 24 * 60;

/// The longest a device sleeps at once, so one asleep around the clock still
/// checks in, and takes a change, once a day.
pub const LONGEST_SLEEP_MINUTES: i64 = 24 * 60;

/// How often a device held asleep outside its windows checks in, to learn of
/// its release: it cannot be woken from the server.
pub const HELD_SLEEP_MINUTES: i64 = 60;

/// When a schedule slot or a sleep window is current. Days are bits, Monday as
/// 1 through Sunday as 64; times are minutes after midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hours {
    pub days: Option<u8>,
    pub from_minute: Option<u16>,
    pub until_minute: Option<u16>,
}

impl Hours {
    /// Whether the hours include `at`.
    #[must_use]
    pub fn is_current(&self, at: &Zoned) -> bool {
        self.includes(at.weekday(), minute_of_day(at))
    }

    /// Whether the hours include the minute of the day `now` on `weekday`.
    fn includes(&self, weekday: Weekday, now: u16) -> bool {
        let on = |weekday: Weekday| self.days.is_none_or(|days| days & day_bit(weekday) != 0);
        let from = self.from_minute.unwrap_or(0);
        let until = self.until_minute.unwrap_or(DAY_MINUTES);
        match from.cmp(&until) {
            Ordering::Equal => on(weekday),
            Ordering::Less => on(weekday) && from <= now && now < until,
            Ordering::Greater => {
                (on(weekday) && now >= from) || (on(weekday.previous()) && now < until)
            }
        }
    }
}

/// Days and times another playlist takes the usual one's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub playlist_id: i64,
    pub hours: Hours,
}

/// Which playlist an awake device shows when: a slot's while it is current,
/// else its usual one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Schedule {
    /// The usual playlist; `None` shows that nothing is scheduled.
    pub playlist_id: Option<i64>,
    pub slots: Vec<Slot>,
}

/// Two slots of a schedule current at one minute of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlap {
    /// The slots' positions, the first before the second.
    pub slots: (usize, usize),
    pub weekday: Weekday,
    /// Minutes after midnight.
    pub minute: u16,
}

impl std::fmt::Display for Overlap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const NAMES: [&str; 7] = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ];
        let (first, second) = self.slots;
        let day = NAMES[usize::from(self.weekday.to_monday_zero_offset().unsigned_abs())];
        write!(
            formatter,
            "Slots {} and {} are both current on {day} at {:02}:{:02}; a display shows one playlist at a time.",
            first + 1,
            second + 1,
            self.minute / 60,
            self.minute % 60
        )
    }
}

impl Schedule {
    /// The position of the slot current at `at`.
    fn slot_at(&self, at: &Zoned) -> Option<usize> {
        self.slots.iter().position(|slot| slot.hours.is_current(at))
    }

    /// The playlist the schedule calls for at `at`.
    #[must_use]
    pub fn playlist_at(&self, at: &Zoned) -> Option<i64> {
        self.slot_at(at)
            .map(|slot| self.slots[slot].playlist_id)
            .or(self.playlist_id)
    }

    /// The first minute of the week, from Monday's midnight, at which two
    /// slots are current; `None` when no two ever are. The week is the days and
    /// times the hours name, so a minute a change of clocks skips counts too.
    #[must_use]
    pub fn overlap(&self) -> Option<Overlap> {
        let mut weekday = Weekday::Monday;
        for _ in 0..7 {
            for minute in 0..DAY_MINUTES {
                let mut current = self
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, slot)| slot.hours.includes(weekday, minute))
                    .map(|(position, _)| position);
                if let (Some(first), Some(second)) = (current.next(), current.next()) {
                    return Some(Overlap {
                        slots: (first, second),
                        weekday,
                        minute,
                    });
                }
            }
            weekday = weekday.next();
        }
        None
    }
}

fn day_bit(weekday: Weekday) -> u8 {
    1 << weekday.to_monday_zero_offset()
}

fn minute_of_day(at: &Zoned) -> u16 {
    u16::try_from(i32::from(at.hour()) * 60 + i32::from(at.minute())).unwrap_or(0)
}

/// Whether the device sleeps at `at`.
#[must_use]
pub fn asleep(windows: &[Hours], at: &Zoned) -> bool {
    windows.iter().any(|window| window.is_current(at))
}

/// The position to show in a playlist of `screens` screens: the one after
/// `last`, or else the first; `None` for an empty playlist.
#[must_use]
pub fn next(screens: usize, last: Option<usize>) -> Option<usize> {
    (screens > 0).then(|| {
        last.map(|last| last + 1)
            .filter(|&position| position < screens)
            .unwrap_or(0)
    })
}

/// Seconds until the device should check in again. In a sleep window, until no
/// window is current, a day at most. Held asleep outside them, an hour. Awake,
/// its refresh interval, or less when another of `schedule`'s playlists takes
/// over or it falls asleep sooner; a device pinned to a playlist or a screen
/// passes no schedule.
#[must_use]
pub fn wait(
    schedule: Option<&Schedule>,
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
            (
                schedule.and_then(|schedule| schedule.slot_at(later)),
                asleep(sleep, later),
            )
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

    fn slot(playlist_id: i64, days: Option<u8>, from: Option<u16>, until: Option<u16>) -> Slot {
        Slot {
            playlist_id,
            hours: hours(days, from, until),
        }
    }

    fn schedule(slots: Vec<Slot>) -> Schedule {
        Schedule {
            playlist_id: Some(9),
            slots,
        }
    }

    const MONDAY: u8 = 1;
    const TUESDAY: u8 = 1 << 1;
    const WEDNESDAY: u8 = 1 << 2;
    const SUNDAY: u8 = 1 << 6;

    #[test]
    fn hours_without_days_or_times_are_always_current() {
        assert!(hours(None, None, None).is_current(&wednesday("00:00")));
        assert!(hours(None, None, None).is_current(&wednesday("23:59")));
    }

    #[test]
    fn times_are_a_half_open_range_of_the_day() {
        let morning = hours(Some(WEDNESDAY), Some(7 * 60), Some(9 * 60 + 30));
        assert!(!morning.is_current(&wednesday("06:59")));
        assert!(morning.is_current(&wednesday("07:00")));
        assert!(morning.is_current(&wednesday("09:29")));
        assert!(!morning.is_current(&wednesday("09:30")));
        assert!(!hours(Some(TUESDAY), Some(7 * 60), None).is_current(&wednesday("08:00")));
    }

    #[test]
    fn a_range_past_midnight_belongs_to_the_day_it_starts() {
        let tuesday_night = hours(Some(TUESDAY), Some(22 * 60), Some(2 * 60));
        assert!(tuesday_night.is_current(&wednesday("01:59")));
        assert!(!tuesday_night.is_current(&wednesday("02:00")));
        assert!(
            !tuesday_night.is_current(&wednesday("23:00")),
            "Wednesday night is not Tuesday's"
        );
    }

    #[test]
    fn equal_times_are_the_whole_day() {
        let all_day = hours(Some(WEDNESDAY), Some(8 * 60), Some(8 * 60));
        assert!(all_day.is_current(&wednesday("03:00")));
        assert!(!hours(Some(TUESDAY), Some(8 * 60), Some(8 * 60)).is_current(&wednesday("03:00")));
    }

    #[test]
    fn a_current_slot_takes_the_usual_playlist_s_place() {
        let schedule = schedule(vec![
            slot(1, Some(TUESDAY), None, None),
            slot(2, None, Some(7 * 60), Some(9 * 60)),
        ]);
        assert_eq!(schedule.playlist_at(&wednesday("08:00")), Some(2));
        assert_eq!(
            schedule.playlist_at(&wednesday("09:00")),
            Some(9),
            "the usual one"
        );
        let nothing = Schedule {
            playlist_id: None,
            ..schedule
        };
        assert_eq!(nothing.playlist_at(&wednesday("12:00")), None);
    }

    #[test]
    fn goes_through_a_playlist_and_starts_over() {
        assert_eq!(next(3, None), Some(0));
        assert_eq!(next(3, Some(0)), Some(1));
        assert_eq!(next(3, Some(2)), Some(0));
        assert_eq!(
            next(2, Some(5)),
            Some(0),
            "a position past the end starts over"
        );
        assert_eq!(next(0, None), None);
    }

    #[test]
    fn slots_that_meet_do_not_overlap() {
        let schedule = schedule(vec![
            slot(1, Some(MONDAY), Some(7 * 60), Some(9 * 60)),
            slot(2, Some(MONDAY), Some(9 * 60), Some(7 * 60)),
            slot(3, Some(WEDNESDAY), None, None),
        ]);
        assert_eq!(schedule.overlap(), None);
    }

    #[test]
    fn finds_the_first_minute_two_slots_share() {
        let schedule = schedule(vec![
            slot(1, Some(TUESDAY), None, None),
            slot(2, None, Some(7 * 60), Some(9 * 60)),
        ]);
        assert_eq!(
            schedule.overlap(),
            Some(Overlap {
                slots: (0, 1),
                weekday: Weekday::Tuesday,
                minute: 7 * 60,
            })
        );
    }

    #[test]
    fn sunday_night_runs_into_monday() {
        let schedule = schedule(vec![
            slot(1, Some(MONDAY), Some(60), Some(3 * 60)),
            slot(2, Some(SUNDAY), Some(22 * 60), Some(2 * 60)),
        ]);
        assert_eq!(
            schedule.overlap(),
            Some(Overlap {
                slots: (0, 1),
                weekday: Weekday::Monday,
                minute: 60,
            })
        );
    }

    #[test]
    fn wakes_when_another_playlist_takes_over_before_the_refresh() {
        let schedule = schedule(vec![slot(1, None, Some(12 * 60 + 10), Some(13 * 60))]);
        let at: Zoned = "2026-11-04T12:00:30[Europe/Berlin]".parse().unwrap();
        assert_eq!(wait(Some(&schedule), &[], false, 15, &at), 9 * 60 + 30);
        assert_eq!(
            wait(Some(&schedule), &[], false, 5, &at),
            5 * 60,
            "the refresh comes first"
        );
        assert_eq!(
            wait(None, &[], false, 15, &at),
            15 * 60,
            "a pinned device follows no schedule"
        );
    }

    #[test]
    fn counts_real_seconds_across_a_daylight_saving_change() {
        // Berlin leaves summer time at 03:00 on 2026-10-25, repeating 02:00 to 03:00.
        let schedule = schedule(vec![slot(1, None, Some(3 * 60), None)]);
        // 2026-10-25T00:00:00Z is 02:00 summer time in Berlin.
        let at = jiff::Timestamp::from_second(1_792_886_400)
            .unwrap()
            .to_zoned(TimeZone::get("Europe/Berlin").unwrap());
        assert_eq!(at.hour(), 2);
        assert_eq!(
            wait(Some(&schedule), &[], false, 24 * 60, &at),
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
            wait(Some(&schedule(Vec::new())), &nights, false, 15, &at),
            8 * 60 * 60,
            "through both windows, past the refresh"
        );
    }

    #[test]
    fn sleeps_a_day_at_most() {
        let always = [hours(None, None, None)];
        let at = wednesday("12:00");
        assert_eq!(
            wait(None, &always, false, 15, &at),
            LONGEST_SLEEP_MINUTES * 60
        );
    }

    #[test]
    fn wakes_in_time_to_fall_asleep() {
        let nights = [hours(None, Some(23 * 60), Some(7 * 60))];
        let at: Zoned = "2026-11-04T22:50:00[Europe/Berlin]".parse().unwrap();
        assert_eq!(
            wait(Some(&schedule(Vec::new())), &nights, false, 60, &at),
            10 * 60,
            "at 23:00, before its hour is up"
        );
    }

    #[test]
    fn a_device_held_asleep_checks_in_hourly_outside_its_windows() {
        let nights = [hours(None, Some(23 * 60), Some(7 * 60))];
        let schedule = schedule(Vec::new());
        assert_eq!(
            wait(Some(&schedule), &nights, true, 15, &wednesday("12:00")),
            HELD_SLEEP_MINUTES * 60
        );
        let night: Zoned = "2026-11-04T23:00:00[Europe/Berlin]".parse().unwrap();
        assert_eq!(
            wait(Some(&schedule), &nights, true, 15, &night),
            8 * 60 * 60,
            "in a window it sleeps the window through, held or not"
        );
    }
}
