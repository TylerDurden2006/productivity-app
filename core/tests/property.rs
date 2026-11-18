mod common;

use common::App;
use flow_core::model::{
    OutcomeKind, ResolveInput, SessionKind, Slot, SplitChild, Status, TriageInput, TriageTarget,
};
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<T: Clone>(&mut self, values: &[T]) -> T {
        values[(self.next() as usize) % values.len()].clone()
    }
}

fn check_invariants(app: &App) {
    let mut stmt = app
        .conn
        .prepare(
            "SELECT planned_for, slot, COUNT(*) FROM tasks WHERE planned_for IS NOT NULL \
             AND slot IS NOT NULL AND status = 'READY' GROUP BY planned_for, slot",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap();
    for row in rows {
        let (day, slot, count) = row.unwrap();
        let capacity = match slot.as_str() {
            "ONE" => 1,
            "THREE" => 3,
            "BATCH" => 5,
            _ => i64::MAX,
        };
        assert!(count <= capacity, "{day} has {count} {slot} tasks");
    }
    let bad: i64 = app
        .conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE planned_for IS NOT NULL \
             AND status NOT IN ('READY', 'DONE')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(bad, 0, "a non-working task stayed on a plan");
    let active: i64 = app
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(active <= 1, "{active} active sessions");
    let invalid: i64 = app
        .conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE (status = 'WAITING' OR status = 'SOMEDAY') \
             AND (planned_for IS NOT NULL OR slot IS NOT NULL OR slot_pos IS NOT NULL)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(invalid, 0, "waiting/someday task still has plan fields");
}

#[test]
fn t42_random_valid_sequences_keep_the_invariants() {
    let mut app = App::new();
    app.set_ready_cap(50);
    let mut rng = Rng(0x5eed_1234_9876_abcd);
    let mut titles = 0;
    for step in 0..250 {
        let tasks = flow_core::db::all_tasks(&app.conn).unwrap();
        let today = app.today();
        let tomorrow = app.tomorrow();
        match rng.next() % 14 {
            0 => {
                titles += 1;
                app.capture(&format!("Task {titles}"));
            }
            1 => {
                if let Some(task) = tasks.iter().find(|task| task.status == Status::Inbox) {
                    let estimate = rng.pick(&[15, 30, 30, 60, 90, 120]);
                    let _ = app.clarify(
                        &task.id,
                        &task.title,
                        Some("First step"),
                        Some(estimate),
                        Some((rng.next() % 5 + 1) as i64),
                        None,
                        None,
                        None,
                    );
                }
            }
            2 => {
                if let Some(task) = tasks
                    .iter()
                    .find(|task| task.status == Status::Ready && task.planned_for.is_none())
                {
                    let day = if rng.next().is_multiple_of(2) {
                        &today
                    } else {
                        &tomorrow
                    };
                    let slot = rng.pick(&[Slot::One, Slot::Three, Slot::Batch, Slot::Extra]);
                    let _ = app.assign(day, &task.id, slot);
                }
            }
            3 => {
                if let Some(task) = tasks.iter().find(|task| task.planned_for.is_some()) {
                    let _ =
                        flow_core::api::unassign_task(&mut app.conn, &app.clock, &task.id, None);
                }
            }
            4 => {
                if let Some(task) = tasks.iter().find(|task| task.status == Status::Ready) {
                    let manual = if rng.next().is_multiple_of(3) {
                        Some((rng.next() % 120) as i64)
                    } else {
                        None
                    };
                    let _ =
                        flow_core::api::complete_task(&mut app.conn, &app.clock, &task.id, manual);
                }
            }
            5 => {
                if let Some(task) = tasks.iter().find(|task| {
                    matches!(task.status, Status::Inbox | Status::Ready | Status::Someday)
                }) {
                    let to = rng.pick(&[
                        TriageTarget::Waiting,
                        TriageTarget::Someday,
                        TriageTarget::Dropped,
                    ]);
                    let _ = flow_core::api::triage_task(
                        &mut app.conn,
                        &app.clock,
                        &task.id,
                        TriageInput {
                            to,
                            waiting_on: Some("Someone".into()),
                            review_on: Some(today.clone()),
                        },
                    );
                }
            }
            6 => {
                if rng.next().is_multiple_of(2) {
                    if let Some(task) = tasks.iter().find(|task| {
                        task.status == Status::Ready
                            && task.planned_for.as_deref() == Some(today.as_str())
                            && task.slot != Some(Slot::Batch)
                    }) {
                        let _ = flow_core::api::start_session(
                            &mut app.conn,
                            &app.clock,
                            SessionKind::Focus,
                            Some(task.id.clone()),
                        );
                    }
                } else {
                    let _ = flow_core::api::start_session(
                        &mut app.conn,
                        &app.clock,
                        SessionKind::Batch,
                        None,
                    );
                }
            }
            7 => {
                let _ = match rng.next() % 4 {
                    0 => flow_core::api::pause_session(&mut app.conn, &app.clock),
                    1 => flow_core::api::resume_session(&mut app.conn, &app.clock),
                    2 => flow_core::api::stop_session(&mut app.conn, &app.clock, None),
                    _ => flow_core::api::extend_session(&mut app.conn, &app.clock),
                };
            }
            8 => {
                if let Some(task) = tasks.iter().find(|task| {
                    task.status == Status::Ready
                        && task.planned_for.as_deref() == Some(today.as_str())
                }) {
                    let outcome = rng.pick(&[
                        OutcomeKind::Carry,
                        OutcomeKind::Waiting,
                        OutcomeKind::Someday,
                        OutcomeKind::Drop,
                        OutcomeKind::Shrink,
                    ]);
                    let _ = flow_core::api::resolve_task(
                        &mut app.conn,
                        &app.clock,
                        &task.id,
                        ResolveInput {
                            outcome,
                            waiting_on: Some("Someone".into()),
                            review_on: Some(today.clone()),
                            breadcrumb: None,
                            children: None,
                            first_step: Some("Smaller".into()),
                            estimate_mins: Some(15),
                        },
                    );
                }
            }
            9 => {
                let _ = app.lock_today();
            }
            10 => {
                let _ = flow_core::api::carry_all(&mut app.conn, &app.clock);
                let _ = flow_core::api::complete_wrapup(&mut app.conn, &app.clock, &today, None);
            }
            11 => {
                if let Some(task) = tasks.iter().find(|task| task.status == Status::Ready) {
                    let _ = flow_core::api::split_task(
                        &mut app.conn,
                        &app.clock,
                        &task.id,
                        vec![
                            SplitChild {
                                title: "Part A".into(),
                                first_step: "Do A".into(),
                                estimate_mins: 30,
                            },
                            SplitChild {
                                title: "Part B".into(),
                                first_step: "Do B".into(),
                                estimate_mins: 30,
                            },
                        ],
                    );
                }
            }
            12 => {
                if let Some(task) = tasks
                    .iter()
                    .find(|task| matches!(task.status, Status::Waiting | Status::Someday))
                {
                    let _ = flow_core::api::reactivate_task(
                        &mut app.conn,
                        &app.clock,
                        &task.id,
                        None,
                        None,
                    );
                }
            }
            _ => {
                if let Some(task) = tasks.iter().find(|task| {
                    task.status == Status::Dropped
                        && task.closed_reason == Some(flow_core::model::ClosedReason::NotDoing)
                }) {
                    let _ = flow_core::api::restore_task(&mut app.conn, &app.clock, &task.id);
                }
            }
        }
        if step % 23 == 22 {
            app.advance_minutes(24 * 60);
            let _ = flow_core::api::reconcile(&mut app.conn, &app.clock);
        }
        check_invariants(&app);
    }
}
