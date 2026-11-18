use chrono::{Duration, TimeZone, Utc};
use flow_core::clock::parse_date;
use flow_core::db;
use flow_core::model::{
    Cadence, CadenceKind, Habit, HabitEntryStatus, HabitInput, QuotaPeriod, TimeOfDay,
};
use flow_core::rules::habit;
use flow_core::FixedClock;
use rusqlite::Connection;

struct Fixture {
    conn: Connection,
    clock: FixedClock,
}

impl Fixture {
    fn new() -> Self {
        let conn = db::open_memory().expect("database");
        let clock = FixedClock::utc(
            Utc.with_ymd_and_hms(2026, 5, 4, 9, 0, 0)
                .single()
                .expect("clock"),
        );
        Self { conn, clock }
    }

    fn create(&mut self, title: &str, schedule: Cadence) -> Habit {
        self.create_full(title, schedule, 1, TimeOfDay::Anytime, 0, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_full(
        &mut self,
        title: &str,
        schedule: Cadence,
        target_count: i64,
        time_of_day: TimeOfDay,
        sort_order: i64,
        starts_on: Option<&str>,
        ends_on: Option<&str>,
    ) -> Habit {
        let input = HabitInput {
            title: title.to_string(),
            why: None,
            cue: None,
            area_id: None,
            schedule,
            target_count,
            unit_label: None,
            time_of_day,
            sort_order,
            starts_on: starts_on.map(str::to_string),
            ends_on: ends_on.map(str::to_string),
        };
        let clock = &self.clock;
        habit::create(&mut self.conn, clock, input).expect("create habit")
    }

    fn today(&self, date: &str) -> Vec<flow_core::model::HabitToday> {
        habit::today(&self.conn, &self.clock, Some(date.to_string())).expect("habit today")
    }
}

fn date(value: &str) -> chrono::NaiveDate {
    parse_date(value).expect("date")
}

fn daily(anchor_on: &str) -> Cadence {
    Cadence {
        kind: CadenceKind::Daily,
        anchor_on: anchor_on.to_string(),
        ..Cadence::default()
    }
}

fn weekly(anchor_on: &str, weekdays_mask: i64, interval_n: i64) -> Cadence {
    Cadence {
        kind: CadenceKind::Weekly,
        anchor_on: anchor_on.to_string(),
        weekdays_mask,
        interval_n,
        ..Cadence::default()
    }
}

fn every_n_days(anchor_on: &str, interval_n: i64) -> Cadence {
    Cadence {
        kind: CadenceKind::EveryNDays,
        anchor_on: anchor_on.to_string(),
        interval_n,
        ..Cadence::default()
    }
}

fn quota(anchor_on: &str, count: i64, period: QuotaPeriod) -> Cadence {
    Cadence {
        kind: CadenceKind::Quota,
        anchor_on: anchor_on.to_string(),
        quota_count: Some(count),
        quota_period: Some(period),
        ..Cadence::default()
    }
}

#[test]
fn daily_habits_are_due_every_day_after_the_anchor() {
    let mut fixture = Fixture::new();
    let habit = fixture.create("Stretch", daily("2026-05-01"));

    assert!(habit::due_on(&fixture.conn, &habit, date("2026-05-01")).unwrap());
    assert!(habit::due_on(&fixture.conn, &habit, date("2026-05-04")).unwrap());
    assert!(!habit::due_on(&fixture.conn, &habit, date("2026-04-30")).unwrap());

    let items = fixture.today("2026-05-04");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].habit.id, habit.id);
    assert_eq!(items[0].cadence_label, "Daily");
    assert!(items[0].entry.is_none());
    assert_eq!(items[0].streak, 0);
    assert_eq!(items[0].quota_target, 0);
}

#[test]
fn weekly_habits_respect_weekdays_and_intervals() {
    let mut fixture = Fixture::new();
    let monday = fixture.create("Mondays", weekly("2026-05-04", 1, 1));

    assert!(habit::due_on(&fixture.conn, &monday, date("2026-05-04")).unwrap());
    assert!(!habit::due_on(&fixture.conn, &monday, date("2026-05-05")).unwrap());
    assert!(habit::due_on(&fixture.conn, &monday, date("2026-05-11")).unwrap());
    assert!(fixture.today("2026-05-05").is_empty());
    assert_eq!(fixture.today("2026-05-11").len(), 1);

    let biweekly = fixture.create("Every other Monday", weekly("2026-05-04", 1, 2));
    assert!(habit::due_on(&fixture.conn, &biweekly, date("2026-05-04")).unwrap());
    assert!(!habit::due_on(&fixture.conn, &biweekly, date("2026-05-11")).unwrap());
    assert!(habit::due_on(&fixture.conn, &biweekly, date("2026-05-18")).unwrap());

    let streak_habit = fixture.create("Monday streak", weekly("2026-04-06", 1, 1));
    for day in ["2026-04-20", "2026-04-27", "2026-05-04"] {
        habit::complete(
            &mut fixture.conn,
            &fixture.clock,
            &streak_habit.id,
            day,
            None,
            None,
        )
        .unwrap();
    }
    let stats = habit::stats(&fixture.conn, &fixture.clock, &streak_habit.id, None).unwrap();
    assert_eq!(stats.current_streak, 3);
}

#[test]
fn every_37_days_uses_the_anchor() {
    let mut fixture = Fixture::new();
    let habit = fixture.create("Deep clean", every_n_days("2026-01-01", 37));

    assert!(habit::due_on(&fixture.conn, &habit, date("2026-01-01")).unwrap());
    assert!(habit::due_on(&fixture.conn, &habit, date("2026-02-07")).unwrap());
    assert!(!habit::due_on(&fixture.conn, &habit, date("2026-02-08")).unwrap());
    assert_eq!(fixture.today("2026-02-07").len(), 1);
    assert!(fixture.today("2026-02-08").is_empty());

    habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-02-07",
        None,
        None,
    )
    .unwrap();
    let stats = habit::stats(
        &fixture.conn,
        &fixture.clock,
        &habit.id,
        Some("2026-02-07".into()),
    )
    .unwrap();
    assert_eq!(stats.current_streak, 1);
    assert_eq!(stats.scheduled_count, 1);
}

#[test]
fn quota_habits_hide_once_the_period_quota_is_met() {
    let mut fixture = Fixture::new();
    let habit = fixture.create("Practice", quota("2026-05-01", 2, QuotaPeriod::Week));

    let items = fixture.today("2026-05-04");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].quota_done, 0);
    assert_eq!(items[0].quota_target, 2);

    habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-04",
        None,
        None,
    )
    .unwrap();
    let items = fixture.today("2026-05-04");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].quota_done, 1);
    assert!(items[0].entry.is_some());

    habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-05",
        None,
        None,
    )
    .unwrap();
    let items = fixture.today("2026-05-05");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].quota_done, 2);

    assert!(fixture.today("2026-05-06").is_empty());

    let next_week = fixture.today("2026-05-11");
    assert_eq!(next_week.len(), 1);
    assert_eq!(next_week[0].quota_done, 0);
    assert_eq!(next_week[0].quota_target, 2);

    let streak_habit = fixture.create("Quota streak", quota("2026-04-01", 2, QuotaPeriod::Week));
    for day in [
        "2026-04-20",
        "2026-04-21",
        "2026-04-27",
        "2026-04-28",
        "2026-05-04",
        "2026-05-05",
    ] {
        habit::complete(
            &mut fixture.conn,
            &fixture.clock,
            &streak_habit.id,
            day,
            None,
            None,
        )
        .unwrap();
    }
    let stats = habit::stats(
        &fixture.conn,
        &fixture.clock,
        &streak_habit.id,
        Some("2026-05-05".into()),
    )
    .unwrap();
    assert_eq!(stats.current_streak, 3);
    assert_eq!(stats.best_streak, 3);
    assert_eq!(stats.done_count, 3);
    assert_eq!(stats.scheduled_count, 5);
    assert!((stats.completion_rate - 0.6).abs() < f64::EPSILON);
}

#[test]
fn checking_off_upserts_and_clamps_the_count() {
    let mut fixture = Fixture::new();
    let habit = fixture.create_full(
        "Water",
        daily("2026-05-01"),
        3,
        TimeOfDay::Anytime,
        0,
        None,
        None,
    );

    let entry = habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-04",
        Some(2),
        Some("two glasses".into()),
    )
    .unwrap();
    assert_eq!(entry.count, 2);
    assert_eq!(entry.status, HabitEntryStatus::Done);
    assert_eq!(entry.note.as_deref(), Some("two glasses"));

    let entry = habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-04",
        None,
        None,
    )
    .unwrap();
    assert_eq!(entry.count, 3);
    assert_eq!(entry.status, HabitEntryStatus::Done);
    assert!(entry.note.is_none());

    let entry = habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-04",
        Some(99),
        None,
    )
    .unwrap();
    assert_eq!(entry.count, 3);

    let history = habit::history(&fixture.conn, &habit.id, "2026-05-04", "2026-05-04").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].count, 3);

    let simple = fixture.create("Read", daily("2026-05-01"));
    let entry = habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &simple.id,
        "2026-05-04",
        None,
        None,
    )
    .unwrap();
    assert_eq!(entry.count, 1);
}

#[test]
fn skipping_records_a_skipped_entry_and_reopen_removes_it() {
    let mut fixture = Fixture::new();
    let habit = fixture.create("Meditate", daily("2026-05-01"));

    let entry = habit::skip(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-04",
        Some("sick day".into()),
    )
    .unwrap();
    assert_eq!(entry.status, HabitEntryStatus::Skipped);
    assert_eq!(entry.count, 0);
    assert_eq!(entry.note.as_deref(), Some("sick day"));

    let items = fixture.today("2026-05-04");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].entry.as_ref().map(|entry| entry.status),
        Some(HabitEntryStatus::Skipped)
    );

    let entry = habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-04",
        None,
        None,
    )
    .unwrap();
    assert_eq!(entry.status, HabitEntryStatus::Done);
    assert_eq!(entry.count, 1);
    assert!(entry.note.is_none());

    habit::reopen(&mut fixture.conn, &habit.id, "2026-05-04").unwrap();
    assert!(
        habit::history(&fixture.conn, &habit.id, "2026-05-01", "2026-05-04")
            .unwrap()
            .is_empty()
    );
    let items = fixture.today("2026-05-04");
    assert_eq!(items.len(), 1);
    assert!(items[0].entry.is_none());

    habit::reopen(&mut fixture.conn, &habit.id, "2026-05-04").unwrap();
}

#[test]
fn today_returns_every_due_habit_in_time_order() {
    let mut fixture = Fixture::new();
    fixture.create_full(
        "Morning five",
        daily("2026-05-01"),
        1,
        TimeOfDay::Morning,
        5,
        None,
        None,
    );
    fixture.create_full(
        "Morning one",
        daily("2026-05-01"),
        1,
        TimeOfDay::Morning,
        1,
        None,
        None,
    );
    let checked = fixture.create_full(
        "Afternoon checked",
        daily("2026-05-01"),
        1,
        TimeOfDay::Afternoon,
        5,
        None,
        None,
    );
    fixture.create_full(
        "Afternoon",
        daily("2026-05-01"),
        1,
        TimeOfDay::Afternoon,
        0,
        None,
        None,
    );
    fixture.create_full(
        "Evening",
        daily("2026-05-01"),
        1,
        TimeOfDay::Evening,
        0,
        None,
        None,
    );
    fixture.create_full(
        "Anytime",
        daily("2026-05-01"),
        1,
        TimeOfDay::Anytime,
        0,
        None,
        None,
    );

    fixture.create("Wrong weekday", weekly("2026-05-04", 2, 1));
    fixture.create_full(
        "Future",
        daily("2026-05-01"),
        1,
        TimeOfDay::Morning,
        0,
        Some("2026-05-05"),
        None,
    );
    fixture.create_full(
        "Finished",
        daily("2026-05-01"),
        1,
        TimeOfDay::Morning,
        0,
        None,
        Some("2026-05-03"),
    );
    let archived = fixture.create_full(
        "Archived",
        daily("2026-05-01"),
        1,
        TimeOfDay::Morning,
        0,
        None,
        None,
    );
    habit::archive(&mut fixture.conn, &fixture.clock, &archived.id).unwrap();

    habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &checked.id,
        "2026-05-04",
        None,
        None,
    )
    .unwrap();

    let items = fixture.today("2026-05-04");
    let titles = items
        .iter()
        .map(|item| item.habit.title.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        titles,
        vec![
            "Morning one",
            "Morning five",
            "Afternoon",
            "Afternoon checked",
            "Evening",
            "Anytime",
        ]
    );
    let checked_item = items
        .iter()
        .find(|item| item.habit.id == checked.id)
        .expect("checked habit");
    assert_eq!(
        checked_item.entry.as_ref().map(|entry| entry.status),
        Some(HabitEntryStatus::Done)
    );
    assert_eq!(checked_item.streak, 1);
}

#[test]
fn habits_reset_at_midnight() {
    let mut fixture = Fixture::new();
    let habit = fixture.create("Journal", daily("2026-05-01"));
    let day = db::today_string(&fixture.conn, &fixture.clock).unwrap();
    assert_eq!(day, "2026-05-04");

    habit::complete(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        &day,
        None,
        None,
    )
    .unwrap();
    let items = fixture.today(&day);
    assert!(items[0].entry.is_some());

    fixture.clock.advance(Duration::hours(15));
    let next_day = db::today_string(&fixture.conn, &fixture.clock).unwrap();
    assert_eq!(next_day, "2026-05-05");
    let items = habit::today(&fixture.conn, &fixture.clock, None).unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].entry.is_none());
}

#[test]
fn streaks_count_consecutive_scheduled_done_days() {
    let mut fixture = Fixture::new();
    let habit = fixture.create("Journal", daily("2026-05-01"));

    for day in ["2026-05-01", "2026-05-02", "2026-05-03", "2026-05-04"] {
        habit::complete(
            &mut fixture.conn,
            &fixture.clock,
            &habit.id,
            day,
            None,
            None,
        )
        .unwrap();
    }

    let stats = habit::stats(&fixture.conn, &fixture.clock, &habit.id, None).unwrap();
    assert_eq!(stats.current_streak, 4);
    assert_eq!(stats.best_streak, 4);
    assert_eq!(stats.done_count, 4);
    assert_eq!(stats.scheduled_count, 4);
    assert!((stats.completion_rate - 1.0).abs() < f64::EPSILON);

    let items = fixture.today("2026-05-04");
    assert_eq!(items[0].streak, 4);

    habit::reopen(&mut fixture.conn, &habit.id, "2026-05-02").unwrap();
    let stats = habit::stats(&fixture.conn, &fixture.clock, &habit.id, None).unwrap();
    assert_eq!(stats.current_streak, 2);
    assert_eq!(stats.best_streak, 2);
    assert_eq!(stats.done_count, 3);
    assert_eq!(stats.scheduled_count, 4);
    assert!((stats.completion_rate - 0.75).abs() < f64::EPSILON);

    habit::skip(
        &mut fixture.conn,
        &fixture.clock,
        &habit.id,
        "2026-05-03",
        None,
    )
    .unwrap();
    let stats = habit::stats(&fixture.conn, &fixture.clock, &habit.id, None).unwrap();
    assert_eq!(stats.current_streak, 1);
    assert_eq!(stats.best_streak, 1);
    assert_eq!(stats.done_count, 2);
    assert_eq!(stats.scheduled_count, 4);
    assert!((stats.completion_rate - 0.5).abs() < f64::EPSILON);
}
