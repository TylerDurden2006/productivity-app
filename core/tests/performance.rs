mod common;

use std::time::{Duration, Instant};

use common::App;
use flow_core::model::Slot;

#[test]
fn t50_large_views_and_assignment_stay_responsive() {
    let mut app = App::new();
    let today = app.today();
    let now = flow_core::db::now(&app.clock);
    {
        let tx = app.conn.transaction().unwrap();
        {
            let mut insert_task = tx
                .prepare(
                    "INSERT INTO tasks (id, title, first_step, estimate_mins, status, value, \
                     pinned, rollover_count, created_at, updated_at) \
                     VALUES (?1, ?2, 'First step', 30, 'READY', 3, 0, 0, ?3, ?3)",
                )
                .unwrap();
            for index in 0..10_000 {
                insert_task
                    .execute(rusqlite::params![
                        format!("task-{index:05}"),
                        format!("Task {index}"),
                        now
                    ])
                    .unwrap();
            }
        }
        {
            let mut insert_event = tx
                .prepare("INSERT INTO events (at, task_id, kind) VALUES (?1, NULL, 'CREATED')")
                .unwrap();
            for _ in 0..100_000 {
                insert_event.execute([&now]).unwrap();
            }
        }
        tx.commit().unwrap();
    }

    let debug = cfg!(debug_assertions);
    let view_ceiling = if debug {
        Duration::from_millis(3000)
    } else {
        Duration::from_millis(50)
    };
    let assign_ceiling = if debug {
        Duration::from_millis(1500)
    } else {
        Duration::from_millis(50)
    };

    let started = Instant::now();
    let view = flow_core::api::plan_view(&app.conn, &app.clock, None).unwrap();
    assert_eq!(view.deep_candidates.len(), 10_000);
    assert!(
        started.elapsed() < view_ceiling,
        "plan_view took {:?}",
        started.elapsed()
    );

    let started = Instant::now();
    let do_view = flow_core::api::do_view(&app.conn, &app.clock).unwrap();
    assert!(do_view.current.is_none());
    assert!(
        started.elapsed() < view_ceiling,
        "do_view took {:?}",
        started.elapsed()
    );

    let started = Instant::now();
    let assigned = app.assign(&today, "task-05000", Slot::One).unwrap();
    assert_eq!(assigned.planned_for.as_deref(), Some(today.as_str()));
    assert!(
        started.elapsed() < assign_ceiling,
        "assign took {:?}",
        started.elapsed()
    );
}
