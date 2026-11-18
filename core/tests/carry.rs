mod common;

use common::App;
use flow_core::model::{Band, GoalInput, HorizonKind, Slot};

#[test]
fn carry_preserves_priority_and_displaces_the_next_day_one() {
    let mut app = App::new();
    app.set_time(2026, 5, 3, 9, 0);
    let yesterday = app.today();
    let old_one = app.ready("Wednesday main", 60);
    app.assign(&yesterday, &old_one.id, Slot::One).unwrap();

    app.set_time(2026, 5, 4, 9, 0);
    let today = app.today();
    let new_one = app.ready("Thursday main", 60);
    let new_three = app.ready("Thursday next", 30);
    app.assign(&today, &new_one.id, Slot::One).unwrap();
    app.assign(&today, &new_three.id, Slot::Three).unwrap();

    let report = flow_core::api::reconcile(&mut app.conn, &app.clock).unwrap();
    assert_eq!(report.carried, vec![old_one.id.clone()]);

    let carried = app.task(&old_one.id);
    assert_eq!(carried.planned_for.as_deref(), Some(today.as_str()));
    assert_eq!(carried.slot, Some(Slot::One));
    assert_eq!(carried.slot_pos, Some(1));

    let displaced = app.task(&new_one.id);
    assert_eq!(displaced.slot, Some(Slot::Three));
    assert_eq!(displaced.slot_pos, Some(1));

    let remaining = app.task(&new_three.id);
    assert_eq!(remaining.slot, Some(Slot::Three));
    assert_eq!(remaining.slot_pos, Some(2));
}

#[test]
fn carry_cascades_a_full_day_into_more() {
    let mut app = App::new();
    app.set_time(2026, 5, 3, 9, 0);
    let yesterday = app.today();
    let old_one = app.ready("Yesterday main", 60);
    app.assign(&yesterday, &old_one.id, Slot::One).unwrap();

    app.set_time(2026, 5, 4, 9, 0);
    let today = app.today();
    let new_one = app.ready("Today main", 60);
    app.assign(&today, &new_one.id, Slot::One).unwrap();
    for index in 0..3 {
        let task = app.ready(&format!("Next {index}"), 30);
        app.assign(&today, &task.id, Slot::Three).unwrap();
    }
    for index in 0..5 {
        let task = app.ready(&format!("Small {index}"), 15);
        app.assign(&today, &task.id, Slot::Batch).unwrap();
    }

    flow_core::api::reconcile(&mut app.conn, &app.clock).unwrap();
    assert_eq!(app.task(&old_one.id).slot, Some(Slot::One));
    let extra_count = app.plan(Some(&today)).plan.extra.len();
    assert_eq!(extra_count, 1);
    assert_eq!(app.plan(Some(&today)).plan.three.len(), 3);
    assert_eq!(app.plan(Some(&today)).plan.batch.len(), 5);
}

#[test]
fn reorder_moves_a_task_between_sections() {
    let mut app = App::new();
    let today = app.today();
    let one = app.ready("Main", 30);
    let three = app.ready("Next", 30);
    app.assign(&today, &one.id, Slot::One).unwrap();
    app.assign(&today, &three.id, Slot::Three).unwrap();

    flow_core::api::reorder_task(&mut app.conn, &app.clock, &today, &one.id, Slot::Three, 0)
        .unwrap();
    assert_eq!(app.task(&one.id).slot, Some(Slot::Three));
    assert_eq!(app.task(&one.id).slot_pos, Some(1));
    assert_eq!(app.task(&three.id).slot, Some(Slot::Three));
    assert_eq!(app.task(&three.id).slot_pos, Some(2));

    flow_core::api::reorder_task(&mut app.conn, &app.clock, &today, &three.id, Slot::One, 0)
        .unwrap();
    assert_eq!(app.task(&three.id).slot, Some(Slot::One));
}
#[test]
fn open_horizon_tasks_carry_to_the_current_period() {
    let mut app = App::new();
    let goal = flow_core::api::create_goal(
        &mut app.conn,
        &app.clock,
        GoalInput {
            horizon_kind: HorizonKind::Week,
            period_start: Some("2026-04-27".into()),
            area_id: None,
            parent_id: None,
            band: Band::Focus,
            title: "Last week's main thing".into(),
            description: None,
            success_definition: None,
            target_on: None,
            estimate_mins: None,
        },
    )
    .unwrap();
    flow_core::api::reconcile(&mut app.conn, &app.clock).unwrap();
    let view = flow_core::api::horizon_view(
        &app.conn,
        &app.clock,
        HorizonKind::Week,
        Some("2026-05-04".into()),
    )
    .unwrap();
    assert_eq!(view.bands.focus.unwrap().goal.id, goal.id);
}
#[test]
fn reorder_goal_moves_between_bands() {
    let mut app = App::new();
    let goal = flow_core::api::create_goal(
        &mut app.conn,
        &app.clock,
        GoalInput {
            horizon_kind: HorizonKind::Week,
            period_start: Some("2026-05-04".into()),
            area_id: None,
            parent_id: None,
            band: Band::Focus,
            title: "Main week task".into(),
            description: None,
            success_definition: None,
            target_on: None,
            estimate_mins: None,
        },
    )
    .unwrap();
    let next = flow_core::api::create_goal(
        &mut app.conn,
        &app.clock,
        GoalInput {
            horizon_kind: HorizonKind::Week,
            period_start: Some("2026-05-04".into()),
            area_id: None,
            parent_id: None,
            band: Band::Secondary,
            title: "Next week task".into(),
            description: None,
            success_definition: None,
            target_on: None,
            estimate_mins: None,
        },
    )
    .unwrap();
    flow_core::api::reorder_goal(
        &mut app.conn,
        HorizonKind::Week,
        "2026-05-04",
        &goal.id,
        Band::Secondary,
        0,
    )
    .unwrap();
    let view = flow_core::api::horizon_view(
        &app.conn,
        &app.clock,
        HorizonKind::Week,
        Some("2026-05-04".into()),
    )
    .unwrap();
    assert!(view.bands.focus.is_none());
    assert_eq!(view.bands.secondary[0].goal.id, goal.id);
    assert_eq!(view.bands.secondary[1].goal.id, next.id);
}
