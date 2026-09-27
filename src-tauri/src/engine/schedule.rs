//! When scheduled workflows are due, in the Mac's own time zone, and the record
//! of how far each has been checked. See docs/engine.md, "Schedule".
//!
//! A workflow runs at most once per check, for the latest time it was due, and
//! only if that was within 12 hours: a Mac asleep all weekend runs a daily
//! workflow once on Monday morning, not three times.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, TimeDelta, TimeZone, Weekday};
use serde::{Deserialize, Serialize};

use crate::storage::atomic::write_atomic;
use crate::workflow::format::{Every, Schedule};

/// How late a missed time may still be run.
pub const CATCH_UP: TimeDelta = TimeDelta::hours(12);

/// The latest time the schedule was due, at or before `now`.
pub fn latest_due<Tz: TimeZone>(schedule: &Schedule, now: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    let time = time_of(schedule)?;
    let today = now.date_naive();
    (0..=8)
        .filter_map(|back| today.checked_sub_days(Days::new(back)))
        .filter(|day| on_day(schedule, *day))
        .filter_map(|day| at(&now.timezone(), day, time))
        .find(|due| due <= now)
}

/// The first time the schedule is due after `after`.
pub fn next_due<Tz: TimeZone>(schedule: &Schedule, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    let time = time_of(schedule)?;
    let today = after.date_naive();
    (0..=8)
        .filter_map(|ahead| today.checked_add_days(Days::new(ahead)))
        .filter(|day| on_day(schedule, *day))
        .filter_map(|day| at(&after.timezone(), day, time))
        .find(|due| due > after)
}

fn time_of(schedule: &Schedule) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(&schedule.time, "%H:%M").ok()
}

fn on_day(schedule: &Schedule, day: NaiveDate) -> bool {
    match schedule.every {
        Every::Day => true,
        Every::Weekday => !matches!(day.weekday(), Weekday::Sat | Weekday::Sun),
        Every::Week => schedule
            .weekday
            .is_some_and(|w| u32::from(w) == day.weekday().num_days_from_sunday()),
    }
}

/// `time` on `day` in the zone. A time the clocks skip (spring forward) is
/// the moment they land on; a time that happens twice (fall back) is the first.
fn at<Tz: TimeZone>(zone: &Tz, day: NaiveDate, time: NaiveTime) -> Option<DateTime<Tz>> {
    let mut local = day.and_time(time);
    for _ in 0..=180 {
        if let Some(found) = zone.from_local_datetime(&local).earliest() {
            return Some(found);
        }
        local += TimeDelta::minutes(1);
    }
    None
}

/// Each scheduled workflow that is on, with the time up to which it has been
/// checked. Kept in `engine/schedules.json`, so a restart neither misses nor
/// repeats a time.
pub struct Schedules {
    path: PathBuf,
    on: BTreeMap<String, On>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct On {
    schedule: Schedule,
    /// RFC 3339. Times up to here have been run or passed over.
    checked: String,
}

impl Schedules {
    /// The record at `path`; empty if it's missing or damaged.
    pub fn load(path: PathBuf) -> Schedules {
        let on = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Schedules { path, on }
    }

    /// Starts following a workflow's schedule. Newly on, or on a new
    /// schedule, it starts from `now`: times already past don't run.
    pub fn turn_on<Tz: TimeZone>(
        &mut self,
        id: &str,
        schedule: &Schedule,
        now: &DateTime<Tz>,
    ) -> io::Result<()> {
        if self.on.get(id).is_some_and(|on| &on.schedule == schedule) {
            return Ok(());
        }
        self.on.insert(
            id.to_owned(),
            On {
                schedule: schedule.clone(),
                checked: now.to_rfc3339(),
            },
        );
        self.save()
    }

    pub fn turn_off(&mut self, id: &str) -> io::Result<()> {
        if self.on.remove(id).is_some() {
            self.save()?;
        }
        Ok(())
    }

    pub fn is_on(&self, id: &str) -> bool {
        self.on.contains_key(id)
    }

    /// The workflows to run now: each whose latest time is after its last
    /// check and no more than 12 hours ago. Every workflow with a time since
    /// its last check counts as checked, run or not.
    pub fn due<Tz: TimeZone>(&mut self, now: &DateTime<Tz>) -> io::Result<Vec<String>> {
        let mut run = Vec::new();
        let mut changed = false;
        for (id, on) in &mut self.on {
            let Some(due) = latest_due(&on.schedule, now) else {
                continue;
            };
            let checked = DateTime::parse_from_rfc3339(&on.checked).ok();
            if checked.is_some_and(|c| due <= c) {
                continue;
            }
            if now.clone() - due <= CATCH_UP {
                run.push(id.clone());
            }
            on.checked = now.to_rfc3339();
            changed = true;
        }
        if changed {
            self.save()?;
        }
        Ok(run)
    }

    /// The soonest time any schedule is due after `now`.
    pub fn next<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        self.on
            .values()
            .filter_map(|on| next_due(&on.schedule, now))
            .min()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.on).map_err(io::Error::other)?;
        write_atomic(&self.path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Europe::Berlin;

    fn every(every: Every, time: &str, weekday: Option<u8>) -> Schedule {
        Schedule {
            every,
            time: time.into(),
            weekday,
        }
    }

    fn berlin(text: &str) -> DateTime<chrono_tz::Tz> {
        let naive = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap();
        Berlin.from_local_datetime(&naive).earliest().unwrap()
    }

    fn shown(t: Option<DateTime<chrono_tz::Tz>>) -> String {
        t.unwrap().format("%a %Y-%m-%d %H:%M %Z").to_string()
    }

    #[test]
    fn a_daily_time_is_due_today_once_it_has_passed_and_yesterday_before() {
        let daily = every(Every::Day, "09:00", None);
        assert_eq!(
            shown(latest_due(&daily, &berlin("2026-09-14 09:00"))),
            "Mon 2026-09-14 09:00 CEST"
        );
        assert_eq!(
            shown(latest_due(&daily, &berlin("2026-09-14 08:59"))),
            "Sun 2026-09-13 09:00 CEST"
        );
        assert_eq!(
            shown(next_due(&daily, &berlin("2026-09-14 09:00"))),
            "Tue 2026-09-15 09:00 CEST"
        );
    }

    #[test]
    fn weekdays_skip_the_weekend_and_weekly_picks_its_day() {
        let weekdays = every(Every::Weekday, "10:00", None);
        // Friday 2026-09-18, after ten: next is Monday.
        assert_eq!(
            shown(next_due(&weekdays, &berlin("2026-09-18 11:00"))),
            "Mon 2026-09-21 10:00 CEST"
        );
        assert_eq!(
            shown(latest_due(&weekdays, &berlin("2026-09-20 12:00"))),
            "Fri 2026-09-18 10:00 CEST"
        );
        let sundays = every(Every::Week, "18:30", Some(0));
        assert_eq!(
            shown(next_due(&sundays, &berlin("2026-09-14 12:00"))),
            "Sun 2026-09-20 18:30 CEST"
        );
    }

    #[test]
    fn daylight_saving_keeps_the_time_on_the_clock() {
        let daily = every(Every::Day, "09:00", None);
        // Clocks go back on 25 October: still 9:00 on the wall, now CET.
        assert_eq!(
            shown(next_due(&daily, &berlin("2026-10-24 12:00"))),
            "Sun 2026-10-25 09:00 CET"
        );
        // 2:30 doesn't exist on 29 March: it runs when the clocks land on 3:00.
        let early = every(Every::Day, "02:30", None);
        assert_eq!(
            shown(next_due(&early, &berlin("2026-03-28 12:00"))),
            "Sun 2026-03-29 03:00 CEST"
        );
        // 2:30 happens twice on 25 October: it runs the first time only.
        let first = next_due(&early, &berlin("2026-10-24 12:00")).unwrap();
        assert_eq!(shown(Some(first)), "Sun 2026-10-25 02:30 CEST");
        assert_eq!(shown(next_due(&early, &first)), "Mon 2026-10-26 02:30 CET");
    }

    #[test]
    fn a_missed_time_runs_once_within_twelve_hours_and_never_twice() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("schedules.json");
        let mut s = Schedules::load(path.clone());
        let daily = every(Every::Day, "09:00", None);
        s.turn_on("w", &daily, &berlin("2026-09-14 08:00")).unwrap();

        assert!(s.due(&berlin("2026-09-14 08:59")).unwrap().is_empty());
        // Asleep from 8:59 until 15:00: runs once, late.
        assert_eq!(s.due(&berlin("2026-09-14 15:00")).unwrap(), ["w"]);
        assert!(s.due(&berlin("2026-09-14 15:01")).unwrap().is_empty());
        // Away for three days, back at 10:00: once, for today's time.
        assert_eq!(s.due(&berlin("2026-09-17 10:00")).unwrap(), ["w"]);
        // Back at 22:00 the next day, 13 hours late: not run, and not later either.
        assert!(s.due(&berlin("2026-09-18 22:00")).unwrap().is_empty());
        assert!(s.due(&berlin("2026-09-18 22:01")).unwrap().is_empty());

        // The record survives a restart.
        let mut again = Schedules::load(path);
        assert!(again.due(&berlin("2026-09-18 23:00")).unwrap().is_empty());
        assert_eq!(again.due(&berlin("2026-09-19 09:00")).unwrap(), ["w"]);
    }

    #[test]
    fn turning_on_skips_times_already_past_and_a_new_time_starts_over() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = Schedules::load(tmp.path().join("schedules.json"));
        let daily = every(Every::Day, "09:00", None);
        s.turn_on("w", &daily, &berlin("2026-09-14 10:00")).unwrap();
        assert!(s.due(&berlin("2026-09-14 10:00")).unwrap().is_empty());

        // Saving the same schedule again changes nothing.
        s.turn_on("w", &daily, &berlin("2026-09-15 12:00")).unwrap();
        assert_eq!(s.due(&berlin("2026-09-15 12:00")).unwrap(), ["w"]);

        s.turn_on(
            "w",
            &every(Every::Day, "08:00", None),
            &berlin("2026-09-16 12:00"),
        )
        .unwrap();
        assert!(s.due(&berlin("2026-09-16 12:00")).unwrap().is_empty());
        s.turn_off("w").unwrap();
        assert!(s.due(&berlin("2026-09-17 12:00")).unwrap().is_empty());
        assert!(!s.is_on("w"));
    }
}
