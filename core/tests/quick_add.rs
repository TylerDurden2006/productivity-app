mod common;

use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{Band, GoalInput, HorizonKind, QuickAddInput, Slot, Status};
use flow_core::{api, AppError};

fn quick(
    app: &mut App,
    title: &str,
    estimate: i64,
    slot: Slot,
) -> flow_core::Result<flow_core::model::Task> {
    let day = app.today();
    api::add_task(
        &mut app.conn,
        &app.clock,
        QuickAddInput {
            title: title.into(),
            estimate_mins: estimate,
            slot,
            day,
            value: Some(3),
        },
    )
}

#[test]
fn quick_add_bypasses_big_task_and_ready_cap() {
    let mut app = App::new();
    app.set_multiplier(1.25);
    app.set_ready_cap(0);

    let seed = app.capture("Seed");
    assert_code(
        app.clarify(
            &seed.id,
            "Seed",
            Some("Start"),
            Some(120),
            Some(3),
            None,
            None,
            None,
        ),
        ErrorCode::Boulder,
    );

    let task = quick(&mut app, "Big quick task", 120, Slot::Three).expect("quick add");
    assert_eq!(task.status, Status::Ready);
    assert_eq!(task.estimate_mins, Some(120));
    assert_eq!(task.slot, Some(Slot::Three));
    assert_eq!(task.first_step.as_deref(), Some("Big quick task"));
    assert_eq!(app.event_count("OVERRIDE"), 0);
}

#[test]
fn quick_add_bypasses_locked_plan_and_budget() {
    let mut app = App::new();
    let today = app.today();
    let one = app.ready("One", 120);
    app.assign(&today, &one.id, Slot::One).unwrap();
    let three = app.ready("Three", 90);
    app.assign(&today, &three.id, Slot::Three).unwrap();
    app.lock_today().unwrap();

    let task = quick(&mut app, "Over budget", 60, Slot::Three).expect("quick add");
    assert_eq!(task.status, Status::Ready);
    assert_eq!(task.planned_for.as_deref(), Some(today.as_str()));
    assert_eq!(task.slot, Some(Slot::Three));
}

#[test]
fn quick_add_falls_back_to_open_slots() {
    let mut app = App::new();
    let today = app.today();
    for index in 0..3 {
        let task = app.ready(&format!("Next {index}"), 30);
        app.assign(&today, &task.id, Slot::Three).unwrap();
    }

    let batch = quick(&mut app, "Falls to small", 30, Slot::Three).expect("quick add");
    assert_eq!(batch.slot, Some(Slot::Batch));

    for index in 0..4 {
        let task = app.ready(&format!("Small {index}"), 15);
        app.assign(&today, &task.id, Slot::Batch).unwrap();
    }

    let extra = quick(&mut app, "Falls to more", 30, Slot::Three).expect("quick add");
    assert_eq!(extra.slot, Some(Slot::Extra));
    assert!(extra.slot_pos.unwrap_or(0) >= 1);
}

#[test]
fn quick_add_rolls_back_on_invalid_estimate() {
    let mut app = App::new();
    let result = quick(&mut app, "Bad estimate", 45, Slot::Three);
    assert_code(result, ErrorCode::InvalidEstimate);
    assert_eq!(app.task_count(), 0);
    assert_eq!(app.event_count("CREATED"), 0);
}

#[test]
fn quick_add_rolls_back_when_day_is_invalid() {
    let mut app = App::new();
    let result = api::add_task(
        &mut app.conn,
        &app.clock,
        QuickAddInput {
            title: "Wrong day".into(),
            estimate_mins: 30,
            slot: Slot::Three,
            day: "2000-01-01".into(),
            value: Some(3),
        },
    );
    assert_code(result, ErrorCode::InvalidDay);
    assert_eq!(app.task_count(), 0);
    assert_eq!(app.event_count("CREATED"), 0);
}

#[test]
fn quick_assign_bypasses_gates_and_falls_back() {
    let mut app = App::new();
    let today = app.today();
    for index in 0..3 {
        let task = app.ready(&format!("Next {index}"), 30);
        app.assign(&today, &task.id, Slot::Three).unwrap();
    }
    let target = app.ready("Pull me", 30);
    app.lock_today().unwrap();

    let task = api::assign_task_quick(&mut app.conn, &app.clock, &today, &target.id, Slot::Three)
        .expect("quick assign");
    assert_eq!(task.status, Status::Ready);
    assert_eq!(task.planned_for.as_deref(), Some(today.as_str()));
    assert_eq!(task.slot, Some(Slot::Batch));
}

#[test]
fn quick_goal_falls_back_bands() {
    let mut app = App::new();
    let input = GoalInput {
        horizon_kind: HorizonKind::Week,
        period_start: None,
        area_id: None,
        parent_id: None,
        band: Band::Focus,
        title: "Main goal".into(),
        description: None,
        success_definition: None,
        target_on: None,
        estimate_mins: Some(30),
    };
    api::create_goal(&mut app.conn, &app.clock, input.clone()).expect("goal");

    let fallback = api::create_goal_quick(&mut app.conn, &app.clock, input).expect("quick goal");
    assert_eq!(fallback.band, Band::Secondary);
    assert_eq!(fallback.estimate_mins, Some(30));
}

#[test]
fn quick_add_surfaces_only_hard_errors() {
    let mut app = App::new();
    let error: AppError = quick(&mut app, "", 30, Slot::Three).unwrap_err();
    assert_eq!(error.code, ErrorCode::MissingRequiredField);
    assert!(!error.overridable);
    assert_eq!(app.task_count(), 0);
}
