mod common;

use common::App;
use flow_core::model::{Slot, Status};
use flow_core::rules::calibration;

fn complete_in(app: &mut App, estimate: i64, actual: i64) -> String {
    let task = app.ready(&format!("Task {estimate}/{actual}"), estimate);
    flow_core::api::complete_task(&mut app.conn, &app.clock, &task.id, Some(actual)).unwrap();
    task.id
}

#[test]
fn t35_multiplier_threshold_and_median() {
    let mut app = App::new();
    app.set_multiplier(1.25);
    for _ in 0..4 {
        complete_in(&mut app, 60, 90);
    }
    assert_eq!(
        flow_core::api::stats_view(&app.conn, &app.clock)
            .unwrap()
            .multiplier,
        1.25
    );
    complete_in(&mut app, 60, 90);
    let stats = flow_core::api::stats_view(&app.conn, &app.clock).unwrap();
    assert_eq!(stats.multiplier, 1.5);
    assert_eq!(stats.sample_count, 5);

    let mut clamped = App::new();
    for _ in 0..3 {
        complete_in(&mut clamped, 60, 6);
    }
    for _ in 0..2 {
        complete_in(&mut clamped, 60, 600);
    }
    assert_eq!(
        flow_core::api::stats_view(&clamped.conn, &clamped.clock)
            .unwrap()
            .multiplier,
        0.8
    );
}

#[test]
fn t36_split_samples_are_included_and_empty_data_is_counted() {
    let mut app = App::new();
    app.set_ready_cap(20);
    let today = app.today();
    let parent = app.ready("Split parent", 60);
    app.assign(&today, &parent.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        flow_core::model::SessionKind::Focus,
        Some(parent.id.clone()),
    )
    .unwrap();
    app.advance_minutes(5);
    flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
    flow_core::api::split_task(
        &mut app.conn,
        &app.clock,
        &parent.id,
        vec![
            flow_core::model::SplitChild {
                title: "Child A".into(),
                first_step: "Do A".into(),
                estimate_mins: 30,
            },
            flow_core::model::SplitChild {
                title: "Child B".into(),
                first_step: "Do B".into(),
                estimate_mins: 30,
            },
        ],
    )
    .unwrap();
    let done = app.ready("No time", 30);
    app.complete(&done.id).unwrap();
    let micro = app.ready("Micro", 15);
    flow_core::api::complete_task(&mut app.conn, &app.clock, &micro.id, Some(15)).unwrap();

    let stats = flow_core::api::stats_view(&app.conn, &app.clock).unwrap();
    assert_eq!(stats.sample_count, 1);
    assert_eq!(stats.no_time_data_count, 1);
}

#[test]
fn t37_calibrated_estimate_rounding_and_floor() {
    assert_eq!(calibration::calibrated_estimate(60, 1.38), 85);
    assert_eq!(calibration::calibrated_estimate(15, 0.8), 10);
    assert_eq!(calibration::calibrated_estimate(5, 0.5), 5);
}

#[test]
fn stats_reports_focus_minutes_and_rollover_leaders() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Rolled", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        flow_core::model::SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(25);
    flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
    app.set_rollover(&task.id, 2);
    let stats = flow_core::api::stats_view(&app.conn, &app.clock).unwrap();
    assert_eq!(stats.focus_mins_last_14.len(), 14);
    let today_mins = stats
        .focus_mins_last_14
        .iter()
        .find(|entry| entry.day == today)
        .unwrap();
    assert_eq!(today_mins.mins, 25);
    assert_eq!(stats.most_rolled_over[0].id, task.id);
    assert_eq!(stats.most_rolled_over[0].rollover_count, 2);
}

#[test]
fn stats_completed_task_stays_planned() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("History", 30);
    app.assign(&today, &task.id, Slot::Three).unwrap();
    let done = app.complete(&task.id).unwrap();
    assert_eq!(done.status, Status::Done);
    assert_eq!(done.planned_for.as_deref(), Some(today.as_str()));
    assert_eq!(done.slot, Some(Slot::Three));
}
