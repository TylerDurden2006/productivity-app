mod common;

use chrono::TimeZone;
use common::{assert_code, App};
use flow_core::error::ErrorCode;
use flow_core::model::{
    Band, ClarifyInput, GoalInput, HorizonKind, OutcomeKind, SessionKind, Slot, SplitChild, Status,
    TriageInput, TriageTarget,
};
use flow_core::{api, db, FixedClock};
use rusqlite::Connection;

fn inject_fault(app: &App, kind: &str) {
    app.conn
        .execute_batch(&format!(
            "CREATE TRIGGER fault_{kind} AFTER INSERT ON events \
             WHEN NEW.kind = '{kind}' BEGIN SELECT RAISE(ABORT, 'fault'); END;"
        ))
        .unwrap();
}

fn clear_fault(app: &App, kind: &str) {
    app.conn
        .execute_batch(&format!("DROP TRIGGER IF EXISTS fault_{kind};"))
        .unwrap();
}

fn snapshot(app: &App) -> serde_json::Value {
    flow_core::api::export_json(&app.conn, &app.clock).unwrap()
}

#[test]
fn t38_faults_roll_back_lock_wrapup_split_and_swap() {
    let mut app = App::new();
    let today = app.today();
    let one = app.ready("One", 30);
    app.assign(&today, &one.id, Slot::One).unwrap();
    let before = snapshot(&app);
    inject_fault(&app, "LOCKED");
    assert!(flow_core::api::lock_plan(&mut app.conn, &app.clock, &today, None).is_err());
    assert_eq!(snapshot(&app), before);
    clear_fault(&app, "LOCKED");

    flow_core::api::lock_plan(&mut app.conn, &app.clock, &today, None).unwrap();
    flow_core::api::complete_task(&mut app.conn, &app.clock, &one.id, None).unwrap();
    let before = snapshot(&app);
    inject_fault(&app, "WRAPUP");
    assert!(flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None).is_err());
    assert_eq!(snapshot(&app), before);
    clear_fault(&app, "WRAPUP");

    let parent = app.ready("Parent", 60);
    let before = snapshot(&app);
    inject_fault(&app, "SPLIT");
    let children = vec![
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
    ];
    assert!(flow_core::api::split_task(&mut app.conn, &app.clock, &parent.id, children).is_err());
    assert_eq!(snapshot(&app), before);
    clear_fault(&app, "SPLIT");

    let mut swap_app = App::new();
    let day = swap_app.today();
    let out = swap_app.ready("Out", 30);
    let incoming = swap_app.ready("In", 30);
    swap_app.assign(&day, &out.id, Slot::One).unwrap();
    swap_app.lock_today().unwrap();
    let before = snapshot(&swap_app);
    inject_fault(&swap_app, "SWAPPED");
    assert!(flow_core::api::swap_plan(
        &mut swap_app.conn,
        &swap_app.clock,
        &day,
        &out.id,
        &incoming.id,
        Some("swap".to_string())
    )
    .is_err());
    assert_eq!(snapshot(&swap_app), before);
    clear_fault(&swap_app, "SWAPPED");
}

fn table_count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

#[test]
fn t39_export_contains_all_tables_and_backup_matches() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Task", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    flow_core::api::complete_task(&mut app.conn, &app.clock, &task.id, Some(30)).unwrap();
    let exported = flow_core::api::export_json(&app.conn, &app.clock).unwrap();
    for key in [
        "version",
        "exported_at",
        "settings",
        "recurrences",
        "tasks",
        "sessions",
        "days",
        "events",
    ] {
        assert!(exported.get(key).is_some(), "missing {key}");
    }
    assert_eq!(exported["version"], 3);
    assert!(exported["tasks"].as_array().unwrap().len() == 1);
    assert!(exported["sessions"].as_array().unwrap().len() == 1);
    assert!(exported["days"].as_array().unwrap().len() == 1);
    assert!(!exported["events"].as_array().unwrap().is_empty());

    let path = std::env::temp_dir().join(format!("flow-test-{}.db", uuid::Uuid::new_v4()));
    flow_core::api::backup_to(&app.conn, &path).unwrap();
    flow_core::api::validate_database(&path).unwrap();
    let backup = Connection::open(&path).unwrap();
    for table in ["tasks", "sessions", "days", "events", "recurrences"] {
        assert_eq!(
            table_count(&app.conn, table),
            table_count(&backup, table),
            "{table}"
        );
    }
    assert_eq!(table_count(&backup, "settings"), 1);
    drop(backup);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn t40_migrations_are_idempotent() {
    let path = std::env::temp_dir().join(format!("flow-migrate-{}.db", uuid::Uuid::new_v4()));
    {
        let conn = flow_core::db::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, flow_core::db::CURRENT_SCHEMA_VERSION);
        flow_core::db::migrate(&conn).unwrap();
        conn.execute_batch("CREATE TABLE marker (id INTEGER PRIMARY KEY);")
            .unwrap();
    }
    {
        let conn = flow_core::db::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, flow_core::db::CURRENT_SCHEMA_VERSION);
        let marker: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'marker'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(marker, 1);
    }
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn task_and_goal_data_survive_reopen() {
    let path = std::env::temp_dir().join(format!("flow-persist-{}.db", uuid::Uuid::new_v4()));
    let clock = FixedClock::utc(
        chrono::Utc
            .with_ymd_and_hms(2026, 5, 4, 9, 0, 0)
            .single()
            .expect("clock"),
    );
    let task_id;
    let goal_id;
    {
        let mut conn = db::open(&path).unwrap();
        let task = api::capture_task(&mut conn, &clock, "Persisted task").unwrap();
        api::clarify_task(
            &mut conn,
            &clock,
            &task.id,
            ClarifyInput {
                title: "Persisted task".into(),
                first_step: Some("Open it".into()),
                done_when: None,
                estimate_mins: Some(30),
                value: Some(3),
                deadline_on: None,
            },
            None,
            None,
        )
        .unwrap();
        task_id = task.id;
        let goal = api::create_goal(
            &mut conn,
            &clock,
            GoalInput {
                horizon_kind: HorizonKind::Week,
                period_start: None,
                area_id: None,
                parent_id: None,
                band: Band::Focus,
                title: "Persisted goal".into(),
                description: None,
                success_definition: None,
                target_on: None,
                estimate_mins: Some(60),
            },
        )
        .unwrap();
        goal_id = goal.id;
    }
    {
        let conn = db::open(&path).unwrap();
        let task = db::get_task(&conn, &task_id).unwrap().expect("task");
        assert_eq!(task.status, Status::Ready);
        assert_eq!(task.estimate_mins, Some(30));
        let view = api::horizon_view(&conn, &clock, HorizonKind::Week, None).unwrap();
        let goal = view.bands.focus.expect("goal").goal;
        assert_eq!(goal.id, goal_id);
        assert_eq!(goal.estimate_mins, Some(60));
    }
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[test]
fn t41_undo_reverses_and_respects_guards() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Undo", 30);
    let assigned = app.assign(&today, &task.id, Slot::One).unwrap();
    assert_eq!(assigned.slot, Some(Slot::One));
    let undone = flow_core::api::undo_last(&mut app.conn, &app.clock).unwrap();
    assert!(undone.planned_for.is_none());
    assert_eq!(app.task(&task.id).slot, None);

    app.assign(&today, &task.id, Slot::One).unwrap();
    app.complete(&task.id).unwrap();
    let undone = flow_core::api::undo_last(&mut app.conn, &app.clock).unwrap();
    assert_eq!(undone.status, flow_core::model::Status::Ready);
    assert_eq!(undone.slot, Some(Slot::One));

    flow_core::api::unassign_task(&mut app.conn, &app.clock, &task.id, None).unwrap();
    let undone = flow_core::api::undo_last(&mut app.conn, &app.clock).unwrap();
    assert_eq!(undone.slot, Some(Slot::One));

    flow_core::api::drop_task(&mut app.conn, &app.clock, &task.id, None).unwrap();
    let undone = flow_core::api::undo_last(&mut app.conn, &app.clock).unwrap();
    assert_eq!(undone.status, flow_core::model::Status::Ready);

    let today = app.today();
    flow_core::api::triage_task(
        &mut app.conn,
        &app.clock,
        &task.id,
        TriageInput {
            to: TriageTarget::Someday,
            waiting_on: None,
            review_on: Some(today.clone()),
        },
    )
    .unwrap();
    let undone = flow_core::api::undo_last(&mut app.conn, &app.clock).unwrap();
    assert_eq!(undone.status, flow_core::model::Status::Ready);

    app.advance_minutes(11);
    assert_code(
        flow_core::api::undo_last(&mut app.conn, &app.clock),
        ErrorCode::NothingToUndo,
    );

    let mut newer = App::new();
    let day = newer.today();
    let task = newer.ready("Newer", 30);
    newer.assign(&day, &task.id, Slot::One).unwrap();
    flow_core::api::start_session(
        &mut newer.conn,
        &newer.clock,
        SessionKind::Focus,
        Some(task.id.clone()),
    )
    .unwrap();
    assert_code(
        flow_core::api::undo_last(&mut newer.conn, &newer.clock),
        ErrorCode::NothingToUndo,
    );

    let mut lock = App::new();
    let day = lock.today();
    let task = lock.ready("Locked", 30);
    lock.assign(&day, &task.id, Slot::One).unwrap();
    lock.lock_today().unwrap();
    assert_code(
        flow_core::api::undo_last(&mut lock.conn, &lock.clock),
        ErrorCode::NothingToUndo,
    );

    let mut wrapped = App::new();
    let day = wrapped.today();
    let task = wrapped.ready("Wrapped", 30);
    wrapped.assign(&day, &task.id, Slot::One).unwrap();
    wrapped.complete(&task.id).unwrap();
    flow_core::api::complete_wrapup(&mut wrapped.conn, &wrapped.clock, &day, None).unwrap();
    assert_code(
        flow_core::api::undo_last(&mut wrapped.conn, &wrapped.clock),
        ErrorCode::NothingToUndo,
    );

    let mut split = App::new();
    let parent = split.ready("Parent", 60);
    flow_core::api::split_task(
        &mut split.conn,
        &split.clock,
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
        flow_core::api::undo_last(&mut split.conn, &split.clock),
        ErrorCode::NothingToUndo,
    );
}

#[test]
fn carry_all_is_not_undoable() {
    let mut app = App::new();
    let today = app.today();
    let task = app.ready("Carry", 30);
    app.assign(&today, &task.id, Slot::One).unwrap();
    app.resolve(&task.id, OutcomeKind::Carry, None, None, None, None, None)
        .unwrap();
    assert_code(
        flow_core::api::undo_last(&mut app.conn, &app.clock),
        ErrorCode::NothingToUndo,
    );
}
