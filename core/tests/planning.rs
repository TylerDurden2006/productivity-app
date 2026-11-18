mod common;

use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{Mode, OutcomeKind, Slot, Status, TriageInput, TriageTarget};

#[test]
fn t08_slot_capacity_and_batch_rules() {
    let mut app = App::new();
    let today = app.today();
    let tomorrow = app.tomorrow();

    let one = app.ready("One", 30);
    app.assign(&today, &one.id, Slot::One).unwrap();
    let second = app.ready("Second one", 30);
    assert_code(
        app.assign(&today, &second.id, Slot::One),
        ErrorCode::SlotFull,
    );

    for index in 0..3 {
        let task = app.ready(&format!("Three {index}"), 30);
        app.assign(&today, &task.id, Slot::Three).unwrap();
    }
    let fourth = app.ready("Fourth three", 30);
    assert_code(
        app.assign(&today, &fourth.id, Slot::Three),
        ErrorCode::SlotFull,
    );

    let wrong = app.ready("Any duration batch", 30);
    app.assign(&tomorrow, &wrong.id, Slot::Batch).unwrap();
    for index in 0..4 {
        let task = app.ready(&format!("Batch {index}"), 15);
        app.assign(&tomorrow, &task.id, Slot::Batch).unwrap();
    }
    let sixth = app.ready("Sixth batch", 15);
    assert_code(
        app.assign(&tomorrow, &sixth.id, Slot::Batch),
        ErrorCode::SlotFull,
    );
}

#[test]
fn completed_tasks_do_not_hold_a_section_open() {
    let mut app = App::new();
    let today = app.today();
    for index in 0..3 {
        let task = app.ready(&format!("Finished {index}"), 30);
        app.assign(&today, &task.id, Slot::Three).unwrap();
        app.complete(&task.id).unwrap();
    }

    let overflow = app.ready("Overflow", 30);
    app.assign(&today, &overflow.id, Slot::Extra).unwrap();
    flow_core::api::reorder_task(
        &mut app.conn,
        &app.clock,
        &today,
        &overflow.id,
        Slot::Three,
        0,
    )
    .expect("completed tasks should not fill the section");
    assert_eq!(app.task(&overflow.id).slot, Some(Slot::Three));

    let first = app.ready("First open", 30);
    app.assign(&today, &first.id, Slot::Three)
        .expect("a slot filled with completed tasks still accepts open work");
    let second = app.ready("Second open", 30);
    app.assign(&today, &second.id, Slot::Three).unwrap();
    let fourth = app.ready("Fourth open", 30);
    assert_code(
        app.assign(&today, &fourth.id, Slot::Three),
        ErrorCode::SlotFull,
    );
}

#[test]
fn tasks_move_between_next_and_more() {
    let mut app = App::new();
    let today = app.today();
    let next = app.ready("Next", 30);
    let overflow = app.ready("Overflow", 30);
    app.assign(&today, &next.id, Slot::Three).unwrap();
    app.assign(&today, &overflow.id, Slot::Extra).unwrap();

    flow_core::api::reorder_task(&mut app.conn, &app.clock, &today, &next.id, Slot::Extra, 0)
        .expect("next to more");
    assert_eq!(app.task(&next.id).slot, Some(Slot::Extra));

    flow_core::api::reorder_task(&mut app.conn, &app.clock, &today, &next.id, Slot::Three, 0)
        .expect("more to next");
    assert_eq!(app.task(&next.id).slot, Some(Slot::Three));
}

#[test]
fn t09_only_unplanned_ready_tasks_can_be_assigned() {
    let mut app = App::new();
    let today = app.today();
    let ready = app.ready("Ready", 30);
    app.assign(&today, &ready.id, Slot::One).unwrap();
    assert_code(
        app.assign(&today, &ready.id, Slot::Three),
        ErrorCode::InvalidTransition,
    );

    let inbox = app.capture("Inbox");
    assert_code(
        app.assign(&today, &inbox.id, Slot::One),
        ErrorCode::InvalidTransition,
    );

    let waiting = app.ready("Waiting", 30);
    flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        &waiting.id,
        TriageInput {
            to: TriageTarget::Waiting,
            waiting_on: Some("Alex".into()),
            review_on: Some(today.clone()),
        },
    )
    .unwrap();
    assert_code(
        app.assign(&today, &waiting.id, Slot::One),
        ErrorCode::InvalidTransition,
    );

    let done = app.ready("Done", 30);
    app.complete(&done.id).unwrap();
    assert_code(
        app.assign(&today, &done.id, Slot::One),
        ErrorCode::InvalidTransition,
    );
}

#[test]
fn t10_budget_gate_and_unassign() {
    let mut app = App::new();
    let today = app.today();
    let one = app.ready("One", 120);
    app.assign(&today, &one.id, Slot::One).unwrap();
    let three = app.ready("Three", 90);
    app.assign(&today, &three.id, Slot::Three).unwrap();
    let over = app.ready("Over", 60);
    assert_code(
        app.assign(&today, &over.id, Slot::Three),
        ErrorCode::BudgetExceeded,
    );
    app.assign_reason(&today, &over.id, Slot::Three, Some("because today"))
        .unwrap();
    assert_eq!(app.event_count("OVERRIDE"), 1);
    app.unplan(&over.id);
    let view = app.plan(Some(&today));
    assert_eq!(view.budget.planned_mins, 210);
}

trait Unplan {
    fn unplan(&mut self, id: &str);
}

impl Unplan for App {
    fn unplan(&mut self, id: &str) {
        flow_core::api::unassign_task(&mut self.conn, &self.clock, id, None).expect("unassign");
    }
}

#[test]
fn t11_batch_counts_one_block() {
    let mut app = App::new();
    let today = app.today();
    for index in 0..5 {
        let task = app.ready(&format!("Micro {index}"), 15);
        app.assign(&today, &task.id, Slot::Batch).unwrap();
    }
    let budget = flow_core::api::budget(&app.conn, &today).unwrap();
    assert_eq!(budget.planned_mins, 75);
}

#[test]
fn t12_stuck_gate() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Stuck", 60);
    app.set_rollover(&task.id, 3);
    assert_code(
        app.assign(&today, &task.id, Slot::One),
        ErrorCode::StuckTask,
    );
    app.assign_reason(&today, &task.id, Slot::One, Some("must ship"))
        .unwrap();
    assert_eq!(app.event_count("OVERRIDE"), 1);
}

#[test]
fn t13_lock_requires_a_task_and_keeps_snapshot() {
    let mut app = App::new();
    assert!(app.lock_today().is_err());
    let today = app.today();
    let task = app.ready("Only three", 30);
    app.assign(&today, &task.id, Slot::Three).unwrap();
    let result = app.lock_today().unwrap();
    assert!(result.no_one);
    let row = flow_core::db::day_or_default(&app.conn, &today).unwrap();
    assert!(row.plan_locked_at.is_some());
    assert_eq!(row.multiplier_at_lock, Some(1.0));
    assert_eq!(row.budget_at_lock, Some(240));
}

#[test]
fn t14_post_lock_reason_and_swap() {
    let mut app = App::new();
    let today = app.today();
    let planned = app.ready("Planned", 60);
    app.assign(&today, &planned.id, Slot::One).unwrap();
    app.lock_today().unwrap();
    let extra = app.ready("Extra", 30);
    assert_code(
        app.assign(&today, &extra.id, Slot::Three),
        ErrorCode::PlanLocked,
    );
    assert_code(
        flow_core::api::unassign_task(&mut app.conn, &app.clock, &planned.id, None),
        ErrorCode::PlanLocked,
    );
    flow_core::api::swap_plan(
        &mut app.conn,
        &app.clock,
        &today,
        &planned.id,
        &extra.id,
        Some("change of mind".into()),
    )
    .unwrap();
    let out = app.task(&planned.id);
    let incoming = app.task(&extra.id);
    assert_eq!(out.status, Status::Ready);
    assert!(out.planned_for.is_none());
    assert_eq!(incoming.planned_for.as_deref(), Some(today.as_str()));
    assert_eq!(incoming.slot, Some(Slot::One));
    assert!(app.event_count("SWAPPED") >= 2);
}

#[test]
fn t15_days_are_independent() {
    let mut app = App::new();
    let today = app.today();
    let tomorrow = app.tomorrow();
    let first = app.ready("Today", 30);
    let second = app.ready("Tomorrow", 30);
    app.assign(&today, &first.id, Slot::One).unwrap();
    app.assign(&tomorrow, &second.id, Slot::One).unwrap();
    assert_eq!(
        app.task(&first.id).planned_for.as_deref(),
        Some(today.as_str())
    );
    assert_eq!(
        app.task(&second.id).planned_for.as_deref(),
        Some(tomorrow.as_str())
    );
}

#[test]
fn t16_mode_rolls_into_the_next_day() {
    let mut app = App::new();
    let today = app.today();
    let tomorrow = app.tomorrow();
    let task = app.ready("Task", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    app.lock_today().unwrap();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        Mode::Do
    );
    app.complete(&task.id).unwrap();
    flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None).unwrap();
    let next = app.ready("Next", 30);
    app.assign(&tomorrow, &next.id, Slot::One).unwrap();
    flow_core::api::lock_plan(&mut app.conn, &app.clock, &tomorrow, None).unwrap();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        Mode::Plan
    );
    app.set_time(2026, 5, 5, 4, 1);
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        Mode::Do
    );
}

#[test]
fn t43_suggest_is_read_only_and_apply_matches() {
    let mut app = App::new();
    let today = app.today();
    app.set_ready_cap(30);
    let d1 = app.ready_full("D1", 60, 5, None);
    let d2 = app.ready_full("D2", 60, 4, None);
    let d3 = app.ready_full("D3", 60, 3, None);
    let d4 = app.ready_full("D4", 120, 2, None);
    let d5 = app.ready_full("D5", 120, 2, None);
    let d6 = app.ready_full("D6", 120, 1, None);
    let _ = (d4, d5, d6);
    let mut micros = Vec::new();
    for index in 0..4 {
        micros.push(app.ready(&format!("M{index}"), 15));
    }
    let before_tasks = app.task_count();
    let before_events = app.event_count("PLANNED");
    let proposal = flow_core::api::suggest_plan(&app.conn, &app.clock, &today).unwrap();
    assert_eq!(app.task_count(), before_tasks);
    assert_eq!(app.event_count("PLANNED"), before_events);
    assert!(proposal.planned_mins <= 240);
    let slots: Vec<Slot> = proposal.assignments.iter().map(|item| item.slot).collect();
    assert!(slots.contains(&Slot::One));
    assert_eq!(slots.iter().filter(|slot| **slot == Slot::Three).count(), 2);
    assert_eq!(slots.iter().filter(|slot| **slot == Slot::Batch).count(), 4);
    assert!(proposal.assignments.iter().any(|item| item.id == d1.id));
    assert!(proposal.assignments.iter().any(|item| item.id == d2.id));
    assert!(proposal.assignments.iter().any(|item| item.id == d3.id));
    for micro in &micros {
        assert!(proposal.assignments.iter().any(|item| item.id == micro.id));
    }
    let applied =
        flow_core::api::apply_plan(&mut app.conn, &app.clock, &today, &proposal.assignments)
            .unwrap();
    assert_eq!(applied.len(), proposal.assignments.len());
    let view = app.plan(Some(&today));
    assert_eq!(
        view.plan.one.as_ref().map(|task| task.id.as_str()),
        Some(d1.id.as_str())
    );
    assert_eq!(view.plan.three.len(), 2);
    assert_eq!(view.plan.batch.len(), 4);
}

#[test]
fn t44_only_today_and_tomorrow_can_be_planned() {
    let mut app = App::new();
    let today = flow_core::db::today(&app.conn, &app.clock).unwrap();
    let day_after = flow_core::clock::format_date(flow_core::clock::add_days(today, 2));
    let tomorrow = app.tomorrow();
    let task = app.ready("Tomorrow task", 30);
    assert_code(
        app.assign(&day_after, &task.id, Slot::One),
        ErrorCode::InvalidDay,
    );
    app.assign(&tomorrow, &task.id, Slot::One).unwrap();
}

#[test]
fn blocked_from_do_moves_current_to_waiting() {
    let mut app = App::new();
    let today = app.today();
    let first = app.ready("First", 30);
    let second = app.ready("Second", 30);
    app.assign(&today, &first.id, Slot::One).unwrap();
    app.assign(&today, &second.id, Slot::Three).unwrap();
    app.lock_today().unwrap();
    let before = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert_eq!(before.current.unwrap().task.id, first.id);
    flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        &first.id,
        TriageInput {
            to: TriageTarget::Waiting,
            waiting_on: Some("Alex".into()),
            review_on: Some(today),
        },
    )
    .unwrap();
    let after = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert_eq!(after.current.unwrap().task.id, second.id);
    assert_eq!(app.task(&first.id).rollover_count, 0);
    assert!(app.task(&first.id).planned_for.is_none());
}

#[test]
fn resolve_carry_keeps_rollover_when_progress_is_high() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Progress", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        flow_core::model::SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(40);
    flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
    app.resolve(&task.id, OutcomeKind::Carry, None, None, None, None, None)
        .unwrap();
    let task = app.task(&task.id);
    assert_eq!(task.rollover_count, 0);
    assert_eq!(task.status, Status::Ready);
    assert!(task.planned_for.is_none());
}
