//! Schedule: a scheduled workflow runs at its time on the Mac's clock, once,
//! and catches up once for a time it missed within 12 hours. See
//! docs/engine.md, "Schedule". The time math, daylight saving included, is
//! tested in `engine::schedule`.

mod common;

use chrono::{DateTime, FixedOffset, Local, TimeZone};
use serde_json::json;
use vela_lib::engine::runs::{RunStatus, TriggerKind};
use vela_lib::workflow::Workflow;

use common::engine::{engine, EngineHarness};

/// 14 September 2026 at this time on the Mac's own clock.
fn at(hour: u32, minute: u32) -> DateTime<FixedOffset> {
    Local
        .with_ymd_and_hms(2026, 9, 14, hour, minute, 0)
        .earliest()
        .unwrap()
        .fixed_offset()
}

fn morning(h: &EngineHarness) -> Workflow {
    let wf = h.workflow(
        "Morning",
        json!([
            { "id": "t", "type": "schedule", "title": "Every day at 9:00",
              "schedule": { "every": "day", "time": "09:00" }, "next": "n",
              "position": { "x": 0, "y": 0 } },
            { "id": "n", "type": "notify", "title": "Say", "message": "Good morning, it's {date}",
              "next": null, "position": { "x": 0, "y": 0 } },
        ]),
    );
    h.switch(&wf, true)
}

#[tokio::test]
async fn it_runs_at_its_time_once() {
    let mut h = engine();
    h.clock.set(at(8, 0));
    morning(&h);

    h.clock.set(at(8, 59));
    h.engine.check_schedules();
    assert!(h.settle().await.is_empty());

    h.clock.set(at(9, 0));
    h.engine.check_schedules();
    h.engine.check_schedules();
    let runs = h.settle().await;

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].trigger.kind, TriggerKind::Schedule);
    assert_eq!(runs[0].status, RunStatus::Done);
    assert_eq!(h.notes.bodies(), ["Good morning, it's 2026-09-14"]);
}

#[tokio::test]
async fn turned_on_after_its_time_it_waits_for_tomorrow() {
    let mut h = engine();
    h.clock.set(at(10, 0));
    morning(&h);
    h.engine.check_schedules();
    assert!(h.settle().await.is_empty());
}

#[tokio::test]
async fn a_time_missed_while_quit_runs_once_at_start_if_within_12_hours() {
    let h = engine();
    h.clock.set(at(8, 0));
    morning(&h);
    h.clock.set(at(15, 0));
    let mut h = h.restart();
    let runs = h.settle().await;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].trigger.kind, TriggerKind::Schedule);

    // Quit again until 22:00 the next day, 13 hours after its time: too late.
    h.clock.set(at(22, 0) + chrono::Duration::days(1));
    let mut h = h.restart();
    assert_eq!(h.settle().await.len(), 1);
}

#[tokio::test]
async fn turning_it_off_stops_it() {
    let mut h = engine();
    h.clock.set(at(8, 0));
    let wf = morning(&h);
    h.switch(&wf, false);
    h.clock.set(at(9, 0));
    h.engine.check_schedules();
    assert!(h.settle().await.is_empty());
}

#[tokio::test]
async fn a_time_that_passes_while_paused_is_skipped_not_run_later() {
    let mut h = engine();
    h.clock.set(at(8, 0));
    morning(&h);
    h.engine.pause_all(true).unwrap();

    h.clock.set(at(9, 0));
    h.engine.check_schedules();
    h.clock.set(at(9, 30));
    h.engine.pause_all(false).unwrap();
    h.engine.check_schedules();

    assert!(h.settle().await.is_empty());
}
