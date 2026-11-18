mod common;

use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{ReactivateInput, SplitChild, Status, TriageInput, TriageTarget};

fn triage(app: &mut App, id: &str, to: TriageTarget) {
    let review_on = app.today();
    flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        id,
        TriageInput {
            to,
            waiting_on: Some("Alex".into()),
            review_on: Some(review_on),
        },
    )
    .unwrap();
}

#[test]
fn t47_lifecycle_valid_and_invalid_changes() {
    let mut app = App::new();
    let inbox = app.capture("Inbox");
    let ready = app.ready("Ready", 30);
    let waiting = app.ready("Waiting", 30);
    triage(&mut app, &waiting.id, TriageTarget::Waiting);
    let someday = app.ready("Someday", 30);
    triage(&mut app, &someday.id, TriageTarget::Someday);
    let dropped = app.ready("Dropped", 30);
    triage(&mut app, &dropped.id, TriageTarget::Dropped);
    let done = app.ready("Done", 30);
    app.complete(&done.id).unwrap();

    assert_code(app.complete(&inbox.id), ErrorCode::InvalidTransition);
    assert_code(app.complete(&waiting.id), ErrorCode::InvalidTransition);
    assert_code(app.complete(&someday.id), ErrorCode::InvalidTransition);
    assert_code(app.complete(&dropped.id), ErrorCode::InvalidTransition);
    assert_code(
        flow_core::api::restore_task(&mut app.conn, &app.clock, &done.id),
        ErrorCode::InvalidTransition,
    );
    assert_code(
        flow_core::api::restore_task(&mut app.conn, &app.clock, &ready.id),
        ErrorCode::InvalidTransition,
    );
    assert_code(
        triage_checked(&mut app, &done.id, TriageTarget::Someday),
        ErrorCode::InvalidTransition,
    );
    assert_code(
        flow_core::api::reactivate_task(&mut app.conn, &app.clock, &ready.id, None, None),
        ErrorCode::InvalidTransition,
    );
    assert_code(
        flow_core::api::split_task(&mut app.conn, &app.clock, &waiting.id, vec![]),
        ErrorCode::InvalidTransition,
    );
    assert_code(
        app.clarify(
            &ready.id,
            "Ready",
            Some("Start now"),
            Some(30),
            None,
            None,
            None,
            None,
        ),
        ErrorCode::InvalidTransition,
    );

    let revived =
        flow_core::api::reactivate_task(&mut app.conn, &app.clock, &waiting.id, None, None)
            .unwrap();
    assert_eq!(revived.status, Status::Ready);
    let waiting_again = triage_checked(&mut app, &someday.id, TriageTarget::Waiting).unwrap();
    assert_eq!(waiting_again.status, Status::Waiting);

    let restored = flow_core::api::restore_task(&mut app.conn, &app.clock, &dropped.id).unwrap();
    assert_eq!(restored.status, Status::Inbox);
    let clarified = app
        .clarify(
            &restored.id,
            "Restored",
            Some("Do it"),
            Some(30),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(clarified.status, Status::Ready);
}

fn triage_checked(
    app: &mut App,
    id: &str,
    to: TriageTarget,
) -> flow_core::Result<flow_core::model::Task> {
    let review_on = app.today();
    flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        id,
        TriageInput {
            to,
            waiting_on: Some("Alex".into()),
            review_on: Some(review_on),
        },
    )
}

#[test]
fn t47_planned_to_waiting_clears_the_plan_even_when_locked() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Planned", 30);
    app.assign(&today, &task.id, flow_core::model::Slot::Three)
        .unwrap();
    app.lock_today().unwrap();
    let waiting = triage_checked(&mut app, &task.id, TriageTarget::Waiting).unwrap();
    assert_eq!(waiting.status, Status::Waiting);
    assert!(waiting.planned_for.is_none());
    assert!(waiting.slot.is_none());
    assert!(waiting.slot_pos.is_none());
    let stop = app.plan(None);
    assert!(stop.plan.three.is_empty());
}

#[test]
fn t48_reactivate_and_restore_edge_cases() {
    let mut app = App::new();
    let today = app.today();
    let someday = app.capture("Bare someday");
    flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        &someday.id,
        TriageInput {
            to: TriageTarget::Someday,
            waiting_on: None,
            review_on: Some(today.clone()),
        },
    )
    .unwrap();
    assert_code(
        flow_core::api::reactivate_task(&mut app.conn, &app.clock, &someday.id, None, None),
        ErrorCode::MissingRequiredField,
    );
    let ready = flow_core::api::reactivate_task(
        &mut app.conn,
        &app.clock,
        &someday.id,
        Some(ReactivateInput {
            first_step: Some("Start small".into()),
            done_when: None,
            estimate_mins: Some(30),
            value: None,
            deadline_on: None,
        }),
        None,
    )
    .unwrap();
    assert_eq!(ready.status, Status::Ready);

    let with_fields = app.ready("Has fields", 30);
    triage(&mut app, &with_fields.id, TriageTarget::Someday);
    let revived =
        flow_core::api::reactivate_task(&mut app.conn, &app.clock, &with_fields.id, None, None)
            .unwrap();
    assert_eq!(revived.status, Status::Ready);

    let dropped = app.ready("Dropped", 30);
    triage(&mut app, &dropped.id, TriageTarget::Dropped);
    let restored = flow_core::api::restore_task(&mut app.conn, &app.clock, &dropped.id).unwrap();
    assert_eq!(restored.status, Status::Inbox);
    assert!(restored.closed_reason.is_none());

    let parent = app.ready("Split parent", 60);
    flow_core::api::split_task(
        &mut app.conn,
        &app.clock,
        &parent.id,
        vec![
            SplitChild {
                title: "A".into(),
                first_step: "Do A".into(),
                estimate_mins: 30,
            },
            SplitChild {
                title: "B".into(),
                first_step: "Do B".into(),
                estimate_mins: 30,
            },
        ],
    )
    .unwrap();
    assert_code(
        flow_core::api::restore_task(&mut app.conn, &app.clock, &parent.id),
        ErrorCode::InvalidTransition,
    );
}

#[test]
fn t51_due_waiting_and_someday_can_be_pushed_or_dropped() {
    let mut app = App::new();
    let task = app.capture("Due task");
    let waiting = triage_checked(&mut app, &task.id, TriageTarget::Waiting).unwrap();
    assert_eq!(waiting.status, Status::Waiting);

    let pushed = triage_checked(&mut app, &task.id, TriageTarget::Someday).unwrap();
    assert_eq!(pushed.status, Status::Someday);
    assert!(pushed.review_on.is_some());

    let dropped = triage_checked(&mut app, &task.id, TriageTarget::Dropped).unwrap();
    assert_eq!(dropped.status, Status::Dropped);
}
