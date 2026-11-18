mod common;

use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{IfFull, Status, TriageInput, TriageTarget};

#[test]
fn t01_capture_creates_inbox() {
    let mut app = App::new();
    assert_code(
        flow_core::api::capture_task(&mut app.conn, &app.clock, "   "),
        ErrorCode::MissingRequiredField,
    );
    let task = app.capture("  Buy milk  ");
    assert_eq!(task.title, "Buy milk");
    assert_eq!(task.status, Status::Inbox);
    assert_eq!(task.value, 3);
    assert!(task.first_step.is_none());
    assert!(task.estimate_mins.is_none());
    assert!(task.planned_for.is_none());
}

#[test]
fn t02_clarify_requires_all_fields() {
    let mut app = App::new();
    let task = app.capture("Task");
    assert_code(
        app.clarify(&task.id, "Task", None, Some(30), None, None, None, None),
        ErrorCode::MissingRequiredField,
    );
    assert_code(
        app.clarify(
            &task.id,
            "Task",
            Some("ab"),
            Some(30),
            None,
            None,
            None,
            None,
        ),
        ErrorCode::MissingRequiredField,
    );
    assert_code(
        app.clarify(&task.id, "Task", Some("abc"), None, None, None, None, None),
        ErrorCode::MissingRequiredField,
    );
    let ready = app
        .clarify(
            &task.id,
            "Task",
            Some("abc"),
            Some(30),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(ready.status, Status::Ready);
    assert_eq!(ready.value, 3);
    assert_eq!(ready.estimate_mins, Some(30));
}

#[test]
fn t03_invalid_estimate_bucket() {
    let mut app = App::new();
    let task = app.capture("Task");
    assert_code(
        app.clarify(
            &task.id,
            "Task",
            Some("abc"),
            Some(45),
            None,
            None,
            None,
            None,
        ),
        ErrorCode::InvalidEstimate,
    );
}

#[test]
fn t04_boulder_gate_can_be_overridden_once() {
    let mut app = App::new();
    app.set_multiplier(1.25);
    let task = app.capture("Long task");
    let error = app
        .clarify(
            &task.id,
            "Long task",
            Some("Start"),
            Some(120),
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Boulder);
    assert!(error.overridable);
    let ready = app
        .clarify(
            &task.id,
            "Long task",
            Some("Start"),
            Some(120),
            None,
            None,
            None,
            Some("client deadline"),
        )
        .unwrap();
    assert_eq!(ready.status, Status::Ready);
    assert_eq!(app.event_count("OVERRIDE"), 1);

    let smaller = app.capture("Ninety");
    let ready = app
        .clarify(
            &smaller.id,
            "Ninety",
            Some("Start"),
            Some(90),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(ready.status, Status::Ready);
    assert_eq!(app.event_count("OVERRIDE"), 1);
}

#[test]
fn t05_split_validation_and_inheritance() {
    let mut app = App::new();
    app.set_ready_cap(10);
    let parent = app.ready_full("Parent", 60, 5, Some("2026-05-20"));
    for index in 0..9 {
        app.ready(&format!("Filler {index}"), 30);
    }
    let one = flow_core::api::split_task(
        &mut app.conn,
        &app.clock,
        &parent.id,
        vec![flow_core::model::SplitChild {
            title: "Only".into(),
            first_step: "Start".into(),
            estimate_mins: 30,
        }],
    );
    assert_code(one, ErrorCode::MissingRequiredField);
    let missing = flow_core::api::split_task(
        &mut app.conn,
        &app.clock,
        &parent.id,
        vec![
            flow_core::model::SplitChild {
                title: "One".into(),
                first_step: "".into(),
                estimate_mins: 30,
            },
            flow_core::model::SplitChild {
                title: "Two".into(),
                first_step: "Start".into(),
                estimate_mins: 30,
            },
        ],
    );
    assert_code(missing, ErrorCode::MissingRequiredField);

    let children = flow_core::api::split_task(
        &mut app.conn,
        &app.clock,
        &parent.id,
        vec![
            flow_core::model::SplitChild {
                title: "One".into(),
                first_step: "Start one".into(),
                estimate_mins: 30,
            },
            flow_core::model::SplitChild {
                title: "Two".into(),
                first_step: "Start two".into(),
                estimate_mins: 30,
            },
        ],
    )
    .unwrap();
    assert_eq!(children.len(), 2);
    let closed = app.task(&parent.id);
    assert_eq!(closed.status, Status::Dropped);
    assert_eq!(
        closed.closed_reason,
        Some(flow_core::model::ClosedReason::Split)
    );
    for child in children {
        assert_eq!(child.status, Status::Ready);
        assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(child.value, 5);
        assert_eq!(child.deadline_on.as_deref(), Some("2026-05-20"));
    }
}

#[test]
fn t06_ready_cap_and_someday_overflow() {
    let mut app = App::new();
    app.set_ready_cap(10);
    for index in 0..10 {
        app.ready(&format!("Deep {index}"), 30);
    }
    let eleventh = app.capture("Eleventh");
    assert_code(
        app.clarify(
            &eleventh.id,
            "Eleventh",
            Some("Start"),
            Some(30),
            None,
            None,
            None,
            None,
        ),
        ErrorCode::ReadyFull,
    );
    let overflow = app
        .clarify(
            &eleventh.id,
            "Eleventh",
            Some("Start"),
            Some(30),
            None,
            None,
            Some(IfFull::Someday),
            None,
        )
        .unwrap();
    assert_eq!(overflow.status, Status::Someday);
    assert_eq!(overflow.review_on.as_deref(), Some("2026-05-18"));
    let micro = app.capture("Micro");
    let micro = app
        .clarify(
            &micro.id,
            "Micro",
            Some("Start"),
            Some(15),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(micro.status, Status::Ready);
}

#[test]
fn t07_deep_order_uses_pin_urgency_value_and_estimate() {
    let mut app = App::new();
    app.set_ready_cap(20);
    let a = app.ready_full("A value five", 120, 5, None);
    let b = app.ready_full("B value three", 30, 3, None);
    let pinned = app.ready_full("Pinned", 60, 1, None);
    flow_core::api::set_pinned(&mut app.conn, &app.clock, &pinned.id, true).unwrap();
    let ten_days = app.ready_full("Ten days", 60, 1, Some("2026-05-14"));
    let three_days = app.ready_full("Three days", 60, 1, Some("2026-05-07"));

    let view = app.plan(None);
    let ids: Vec<&str> = view
        .deep_candidates
        .iter()
        .map(|task| task.id.as_str())
        .collect();
    assert_eq!(ids[0], pinned.id);
    assert!(position(&ids, &a.id) < position(&ids, &b.id));
    assert!(position(&ids, &ten_days.id) < position(&ids, &a.id));
    assert!(position(&ids, &three_days.id) < position(&ids, &a.id));

    app.advance_minutes(2 * 24 * 60);
    let view = app.plan(None);
    let ids: Vec<&str> = view
        .deep_candidates
        .iter()
        .map(|task| task.id.as_str())
        .collect();
    assert!(position(&ids, &three_days.id) < position(&ids, &ten_days.id));
}

fn position(ids: &[&str], id: &str) -> usize {
    ids.iter()
        .position(|value| *value == id)
        .expect("id in view")
}

#[test]
fn t34_waiting_review_due_is_read_only() {
    let mut app = App::new();
    let task = app.capture("Waiting task");
    let today = app.today();
    let waiting = flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        &task.id,
        TriageInput {
            to: TriageTarget::Waiting,
            waiting_on: Some("Alex".into()),
            review_on: Some(today),
        },
    )
    .unwrap();
    assert_eq!(waiting.status, Status::Waiting);
    let view = app.plan(None);
    assert!(view
        .banners
        .due_for_review
        .iter()
        .any(|task| task.id == waiting.id));
    let unchanged = app.task(&waiting.id);
    assert_eq!(unchanged.status, Status::Waiting);
    assert_eq!(unchanged.review_on, waiting.review_on);
}
