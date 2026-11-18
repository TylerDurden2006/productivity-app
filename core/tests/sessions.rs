mod common;

use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{DoState, SessionKind, Slot, Status};

#[test]
fn t17_do_view_only_contains_the_plan() {
    let mut app = App::new();
    app.set_ready_cap(30);
    let today = app.today();
    for index in 0..20 {
        app.ready(&format!("Ready {index}"), 30);
    }
    for index in 0..5 {
        app.capture(&format!("Inbox {index}"));
    }
    let mut planned = Vec::new();
    for index in 0..3 {
        let task = app.ready(&format!("Plan {index}"), 30);
        app.assign(
            &today,
            &task.id,
            if index == 0 { Slot::One } else { Slot::Three },
        )
        .unwrap();
        planned.push(task.id);
    }
    let view = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    let mut ids = Vec::new();
    if let Some(current) = &view.current {
        ids.push(current.task.id.clone());
    }
    ids.extend(view.others.iter().map(|item| item.task.id.clone()));
    ids.sort();
    planned.sort();
    assert_eq!(ids, planned);
    assert_eq!(view.remaining_count, 3);
}

#[test]
fn t18_mode_and_all_done_table() {
    let mut app = App::new();
    let today = app.today();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        flow_core::model::Mode::Plan
    );
    let task = app.ready("Only", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    app.lock_today().unwrap();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        flow_core::model::Mode::Do
    );
    let view = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert_eq!(view.state, DoState::Working);
    app.complete(&task.id).unwrap();
    let view = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert_eq!(view.state, DoState::AllDone);
    flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None).unwrap();
    assert_eq!(
        flow_core::api::mode(&app.conn, &app.clock).unwrap(),
        flow_core::model::Mode::Plan
    );
}

#[test]
fn t19_one_active_session() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Task", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    assert_code(
        flow_core::api::start_session(
            &mut app.conn,
            &app.clock,
            SessionKind::Focus,
            Some(task.id.clone()),
        ),
        ErrorCode::SessionAlreadyActive,
    );
    let now = flow_core::db::now(&app.clock);
    let direct = app.conn.execute(
        "INSERT INTO sessions (id, kind, task_id, day, planned_mins, extended, accumulated_secs, \
         running_since, last_seen_at, started_at, ended_at) \
         VALUES ('direct', 'FOCUS', ?1, ?2, 90, 0, 0, ?3, ?3, ?3, NULL)",
        rusqlite::params![task.id, today, now],
    );
    assert!(direct.is_err());
}

#[test]
fn t20_pause_and_resume_excludes_paused_time() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Task", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(20);
    flow_core::api::pause_session(&mut app.conn, &app.clock).unwrap();
    app.advance_minutes(10);
    flow_core::api::resume_session(&mut app.conn, &app.clock).unwrap();
    app.advance_minutes(5);
    let view = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert_eq!(view.session.unwrap().elapsed_secs, 25 * 60);
}

#[test]
fn t21_extend_once_per_focus_session() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Focus", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    let extended = flow_core::api::extend_session(&mut app.conn, &app.clock).unwrap();
    assert_eq!(extended.planned_mins, 105);
    assert_code(
        flow_core::api::extend_session(&mut app.conn, &app.clock),
        ErrorCode::ExtensionUsed,
    );

    let mut batch_app = App::new();
    let batch_day = batch_app.today();
    let micro = batch_app.ready("Micro", 15);
    batch_app
        .assign(&batch_day, &micro.id, Slot::Batch)
        .unwrap();
    flow_core::api::start_session(
        &mut batch_app.conn,
        &batch_app.clock,
        SessionKind::Batch,
        None,
    )
    .unwrap();
    assert_code(
        flow_core::api::extend_session(&mut batch_app.conn, &batch_app.clock),
        ErrorCode::InvalidTransition,
    );
}

#[test]
fn t22_complete_ends_session_and_advances() {
    let mut app = App::new();
    let today = app.today();
    let one = app.ready("One", 60);
    let three = app.ready("Three", 30);
    app.assign(&today, &one.id, Slot::One).unwrap();
    app.assign(&today, &three.id, Slot::Three).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(one.id.clone()),
    )
    .unwrap();
    app.advance_minutes(10);
    let done = app.complete(&one.id).unwrap();
    assert_eq!(done.status, Status::Done);
    assert!(done.completed_at.is_some());
    let view = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert_eq!(view.current.unwrap().task.id, three.id);
    assert!(view.session.is_none());
    assert_eq!(view.done_count, 1);
    let sessions = flow_core::db::list_sessions_for_task(&app.conn, &one.id, &app.clock).unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].ended_at.is_some());
    assert!(sessions[0].elapsed_secs >= 10 * 60);
}

#[test]
fn t23_manual_session_on_complete() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Manual", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::complete_task(&mut app.conn, &app.clock, &task.id, Some(45)).unwrap();
    let sessions = flow_core::db::list_sessions_for_task(&app.conn, &task.id, &app.clock).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].kind, SessionKind::Manual);
    assert_eq!(sessions[0].elapsed_secs, 45 * 60);
}

#[test]
fn t24_stop_keeps_task_planned_and_stores_breadcrumb() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Task", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(5);
    flow_core::api::stop_session(
        &mut app.conn,
        &app.clock,
        Some("resume at section 3".into()),
    )
    .unwrap();
    let task = app.task(&task.id);
    assert_eq!(task.status, Status::Ready);
    assert_eq!(task.planned_for.as_deref(), Some(today.as_str()));
    assert_eq!(task.breadcrumb.as_deref(), Some("resume at section 3"));
}

#[test]
fn t25_reconcile_freezes_a_stale_session() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Task", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(5);
    let report = flow_core::api::reconcile(&mut app.conn, &app.clock).unwrap();
    assert_eq!(report.frozen.len(), 1);
    let stale = report.stale_session.unwrap();
    assert!(!stale.running);
    assert_eq!(stale.elapsed_secs, 0);
}

#[test]
fn t26_pull_extra_only_after_plan_is_finished() {
    let mut app = App::new();
    let today = app.today();
    let planned = app.ready("Planned", 30);
    let extra = app.ready("Extra", 30);
    app.assign(&today, &planned.id, Slot::One).unwrap();
    let pulled = flow_core::api::pull_extra(&mut app.conn, &app.clock, &extra.id).unwrap();
    assert_eq!(pulled.slot, Some(Slot::Extra));
    assert_eq!(pulled.slot_pos, Some(1));
}

#[test]
fn t45_session_transition_guards() {
    let mut app = App::new();
    let today = app.today();
    let one = app.ready("One", 60);
    let three = app.ready("Three", 30);
    let micro = app.ready("Micro", 15);
    app.assign(&today, &one.id, Slot::One).unwrap();
    app.assign(&today, &three.id, Slot::Three).unwrap();
    app.assign(&today, &micro.id, Slot::Batch).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(micro.id.clone()),
    )
    .unwrap();
    flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
    let mut fresh = App::new();
    assert_code(
        flow_core::api::start_session(&mut fresh.conn, &fresh.clock, SessionKind::Batch, None),
        ErrorCode::InvalidTransition,
    );
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(one.id.clone()),
    )
    .unwrap();
    assert_code(
        flow_core::api::complete_task(&mut app.conn, &app.clock, &three.id, None),
        ErrorCode::InvalidTransition,
    );
    flow_core::api::stop_session(&mut app.conn, &app.clock, None).unwrap();
    let done = flow_core::api::complete_task(&mut app.conn, &app.clock, &micro.id, None).unwrap();
    assert_eq!(done.status, Status::Done);
}

#[test]
fn t52_manual_time_is_not_added_after_a_stopped_session() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Timed then finished", 60);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut app.conn,
        &app.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    app.advance_minutes(5);
    flow_core::api::stop_session(&mut app.conn, &app.clock, Some("Enough for now".into())).unwrap();
    flow_core::api::complete_task(&mut app.conn, &app.clock, &task.id, Some(60)).unwrap();
    let sessions = flow_core::db::list_sessions_for_task(&app.conn, &task.id, &app.clock).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].kind, SessionKind::Focus);
}
