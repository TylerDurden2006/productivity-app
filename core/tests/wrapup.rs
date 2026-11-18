mod common;

use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{OutcomeKind, SessionKind, Slot, Status};

fn started_task(app: &mut App, title: &str, minutes: i64) -> String {
    let today = app.today();
    let task = app.ready(title, 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(minutes);
    flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
    task.id
}

#[test]
fn t27_carry_with_high_progress_does_not_increment() {
    let mut app = App::new();
    let id = started_task(&mut app, "Progress", 40);
    app.resolve(&id, OutcomeKind::Carry, None, None, None, None, None)
        .unwrap();
    let task = app.task(&id);
    assert_eq!(task.rollover_count, 0);
    assert_eq!(task.status, Status::Ready);
    assert!(task.planned_for.is_none());
}

#[test]
fn t28_low_progress_increments_and_stuck_is_flagged() {
    let mut app = App::new();
    let id = started_task(&mut app, "Low progress", 5);
    app.resolve(&id, OutcomeKind::Carry, None, None, None, None, None)
        .unwrap();
    assert_eq!(app.task(&id).rollover_count, 1);

    for _ in 0..2 {
        app.advance_minutes(24 * 60);
        let today = app.today();
        app.assign(&today, &id, Slot::One).unwrap();
        flow_core::api::start_session(
            &mut app.conn,
            &app.clock,
            SessionKind::Focus,
            Some(id.clone()),
        )
        .unwrap();
        app.advance_minutes(5);
        flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
        app.resolve(&id, OutcomeKind::Carry, None, None, None, None, None)
            .unwrap();
    }
    assert_eq!(app.task(&id).rollover_count, 3);
    let view = flow_core::api::wrapup_view(&app.conn, &app.clock).unwrap();
    assert!(view.stuck.iter().any(|task| task.id == id));
}

#[test]
fn t29_shrink_requires_smaller_estimate_and_resets_counter() {
    let mut app = App::new();
    let task = app.ready("Stuck", 60);
    app.set_rollover(&task.id, 3);
    assert_code(
        app.resolve(
            &task.id,
            OutcomeKind::Shrink,
            None,
            None,
            None,
            Some("Tiny step"),
            Some(90),
        ),
        ErrorCode::InvalidEstimate,
    );
    assert_eq!(app.task(&task.id).rollover_count, 3);
    let shrunk = app
        .resolve(
            &task.id,
            OutcomeKind::Shrink,
            None,
            None,
            None,
            Some("Tiny step"),
            Some(30),
        )
        .unwrap();
    assert_eq!(shrunk.status, Status::Ready);
    assert_eq!(shrunk.first_step.as_deref(), Some("Tiny step"));
    assert_eq!(shrunk.estimate_mins, Some(30));
    assert_eq!(shrunk.rollover_count, 0);
}

#[test]
fn t30_waiting_requires_both_fields() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Waiting", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    app.set_rollover(&task.id, 2);
    assert_code(
        app.resolve(
            &task.id,
            OutcomeKind::Waiting,
            None,
            Some(&today),
            None,
            None,
            None,
        ),
        ErrorCode::MissingRequiredField,
    );
    assert_code(
        app.resolve(
            &task.id,
            OutcomeKind::Waiting,
            Some("Alex"),
            None,
            None,
            None,
            None,
        ),
        ErrorCode::MissingRequiredField,
    );
    let waiting = app
        .resolve(
            &task.id,
            OutcomeKind::Waiting,
            Some("Alex"),
            Some(&today),
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(waiting.status, Status::Waiting);
    assert_eq!(waiting.rollover_count, 2);
    assert!(waiting.planned_for.is_none());
}

#[test]
fn t31_complete_wrapup_requires_outcomes_and_is_idempotent() {
    let mut app = App::new();
    let today = app.today();
    let first = app.ready("First", 30);
    let second = app.ready("Second", 30);
    app.assign(&today, &first.id, Slot::One).unwrap();
    app.assign(&today, &second.id, Slot::Three).unwrap();
    assert_code(
        flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None),
        ErrorCode::MissingOutcome,
    );
    let carried = flow_core::api::carry_all(&mut app.conn, &app.clock).unwrap();
    assert_eq!(carried.len(), 2);
    let done = flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None).unwrap();
    assert!(done.wrapup_done_at.is_some());
    let events = app.event_count("WRAPUP");
    let again = flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None).unwrap();
    assert_eq!(again.wrapup_done_at, done.wrapup_done_at);
    assert_eq!(app.event_count("WRAPUP"), events);
}

#[test]
fn t32_reconcile_carries_exactly_once() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Carry", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    app.advance_minutes(2 * 24 * 60);
    let first = flow_core::api::reconcile(&mut app.conn, &app.clock).unwrap();
    assert_eq!(first.carried.len(), 1);
    let carried = app.task(&task.id);
    assert_eq!(carried.rollover_count, 1);
    assert_eq!(carried.planned_for.as_deref(), Some(app.today().as_str()));
    assert_eq!(carried.slot, Some(Slot::One));
    let second = flow_core::api::reconcile(&mut app.conn, &app.clock).unwrap();
    assert!(second.carried.is_empty());
    assert_eq!(app.task(&task.id).rollover_count, 1);
}

#[test]
fn t46_reopen_today_returns_to_do_mode() {
    let mut app = App::new();
    let today = app.today();
    let tomorrow = app.tomorrow();
    let task = app.ready("Task", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    app.lock_today().unwrap();
    app.complete(&task.id).unwrap();
    flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None).unwrap();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        flow_core::model::Mode::Plan
    );
    flow_core::api::reopen_wrapup(&mut app.conn, &app.clock, None).unwrap();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        flow_core::model::Mode::Do
    );
    assert_code(
        flow_core::api::reopen_wrapup(&mut app.conn, &app.clock, Some(tomorrow)),
        ErrorCode::InvalidDay,
    );
}
