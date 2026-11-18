use chrono::{TimeZone, Utc};
use flow_core::model::{Cadence, CadenceKind, ClarifyInput, HabitInput, Slot, TimeOfDay};
use flow_core::{api, db, FixedClock};

fn app() -> (rusqlite::Connection, FixedClock) {
    let mut conn = db::open_memory().expect("database");
    let clock = FixedClock::utc(
        Utc.with_ymd_and_hms(2026, 10, 23, 9, 0, 0)
            .single()
            .expect("clock"),
    );
    api::update_settings(
        &mut conn,
        &clock,
        flow_core::model::SettingsPatch {
            day_rollover_hour: None,
            daily_focus_budget_mins: None,
            ready_cap: None,
            focus_block_mins: None,
            batch_block_mins: None,
            default_multiplier: Some(1.0),
            last_weekly_review_at: None,
            week_starts_on: Some(0),
            auto_carry_enabled: None,
            dashboard_default_range: None,
            notes_autosave: None,
            notify_block_end: None,
            planning_reminder_enabled: None,
            planning_reminder_time: None,
            reduced_motion: None,
        },
    )
    .expect("settings");
    (conn, clock)
}

fn habit_input(title: &str, kind: CadenceKind, day: &str) -> HabitInput {
    HabitInput {
        title: title.into(),
        why: None,
        cue: None,
        area_id: None,
        schedule: Cadence {
            kind,
            anchor_on: day.into(),
            ..Cadence::default()
        },
        target_count: 1,
        unit_label: None,
        time_of_day: TimeOfDay::Morning,
        sort_order: 0,
        starts_on: None,
        ends_on: None,
    }
}

fn ready(
    conn: &mut rusqlite::Connection,
    clock: &FixedClock,
    title: &str,
    estimate: i64,
) -> String {
    let task = api::capture_task(conn, clock, title).expect("capture");
    api::clarify_task(
        conn,
        clock,
        &task.id,
        ClarifyInput {
            title: title.into(),
            first_step: Some("Open it".into()),
            done_when: None,
            estimate_mins: Some(estimate),
            value: Some(3),
            deadline_on: None,
        },
        None,
        None,
    )
    .expect("clarify")
    .id
}

#[test]
fn daily_view_shows_all_due_habits_and_bands() {
    let (mut conn, clock) = app();
    let day = db::today_string(&conn, &clock).unwrap();
    api::create_habit(
        &mut conn,
        &clock,
        habit_input("Daily", CadenceKind::Daily, &day),
    )
    .unwrap();
    api::create_habit(
        &mut conn,
        &clock,
        habit_input("Every three", CadenceKind::EveryNDays, &day),
    )
    .unwrap();
    let one = ready(&mut conn, &clock, "The one", 60);
    api::assign_task(&mut conn, &clock, &day, &one, Slot::One, None).unwrap();
    let five = ready(&mut conn, &clock, "A 30 minute fifth", 30);
    api::assign_task(&mut conn, &clock, &day, &five, Slot::Batch, None).unwrap();
    let more = ready(&mut conn, &clock, "More", 30);
    api::assign_task(&mut conn, &clock, &day, &more, Slot::Extra, None).unwrap();

    let daily = api::daily_view(&conn, &clock, None).unwrap();
    assert_eq!(daily.habits.len(), 2);
    assert_eq!(daily.habit_total, 2);
    assert_eq!(daily.plan.one.as_ref().unwrap().title, "The one");
    assert_eq!(daily.plan.batch.len(), 1);
    assert_eq!(daily.plan.extra.len(), 1);
    assert_eq!(daily.budget.planned_mins, 90);
}

#[test]
fn daily_checkoff_moves_a_task_to_completed() {
    let (mut conn, clock) = app();
    let day = db::today_string(&conn, &clock).unwrap();
    let task = ready(&mut conn, &clock, "Check me", 30);
    api::assign_task(&mut conn, &clock, &day, &task, Slot::Three, None).unwrap();
    api::complete_task(&mut conn, &clock, &task, None).unwrap();
    let daily = api::daily_view(&conn, &clock, None).unwrap();
    assert_eq!(daily.plan.three.len(), 1);
    assert_eq!(daily.completed.len(), 1);
    assert_eq!(daily.completed[0].status, flow_core::model::Status::Done);
}

#[test]
fn daily_habit_checkoff_is_reflected() {
    let (mut conn, clock) = app();
    let day = db::today_string(&conn, &clock).unwrap();
    let habit = api::create_habit(
        &mut conn,
        &clock,
        habit_input("Drink water", CadenceKind::Daily, &day),
    )
    .unwrap();
    api::complete_habit(&mut conn, &clock, &habit.id, &day, None, None).unwrap();
    let daily = api::daily_view(&conn, &clock, None).unwrap();
    assert_eq!(daily.habit_done, 1);
    assert!(daily.habits[0].entry.is_some());
}

#[test]
fn extra_is_not_counted_in_the_daily_budget() {
    let (mut conn, clock) = app();
    let day = db::today_string(&conn, &clock).unwrap();
    let task = ready(&mut conn, &clock, "Overflow", 90);
    api::assign_task(&mut conn, &clock, &day, &task, Slot::Extra, None).unwrap();
    let daily = api::daily_view(&conn, &clock, None).unwrap();
    assert_eq!(daily.budget.planned_mins, 0);
}
