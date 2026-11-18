use chrono::{NaiveDate, TimeZone, Utc};
use flow_core::error::ErrorCode;
use flow_core::model::{AreaInput, Band, Goal, GoalInput, GoalStatus, HorizonKind, HorizonView};
use flow_core::rules::horizon;
use flow_core::{db, FixedClock};
use rusqlite::{params, Connection};

struct Fixture {
    conn: Connection,
    clock: FixedClock,
}

impl Fixture {
    fn new() -> Self {
        let conn = db::open_memory().expect("database");
        conn.execute(
            "UPDATE settings SET week_starts_on = 0, default_multiplier = 1.0",
            [],
        )
        .expect("settings");
        let clock = FixedClock::utc(
            Utc.with_ymd_and_hms(2026, 1, 21, 9, 0, 0)
                .single()
                .expect("clock"),
        );
        Self { conn, clock }
    }

    fn input(&self, kind: HorizonKind, band: Band, title: &str) -> GoalInput {
        GoalInput {
            horizon_kind: kind,
            period_start: None,
            area_id: None,
            parent_id: None,
            band,
            title: title.to_string(),
            description: None,
            success_definition: None,
            target_on: None,
            estimate_mins: None,
        }
    }

    fn goal(&mut self, kind: HorizonKind, band: Band, title: &str) -> Goal {
        let input = self.input(kind, band, title);
        horizon::create_goal(&mut self.conn, &self.clock, input).expect("goal")
    }

    fn view(&self, kind: HorizonKind, period_start: Option<&str>) -> HorizonView {
        horizon::view(
            &self.conn,
            &self.clock,
            kind,
            period_start.map(str::to_string),
        )
        .expect("view")
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_task(
    conn: &Connection,
    id: &str,
    title: &str,
    status: &str,
    planned_for: Option<&str>,
    slot: Option<&str>,
    slot_pos: Option<i64>,
    created_at: &str,
) {
    let completed_at = if status == "DONE" {
        Some(created_at)
    } else {
        None
    };
    conn.execute(
        "INSERT INTO tasks (id, title, first_step, estimate_mins, status, value, planned_for, \
         slot, slot_pos, created_at, updated_at, completed_at) \
         VALUES (?1, ?2, 'Do it', 30, ?3, 3, ?4, ?5, ?6, ?7, ?7, ?8)",
        params![
            id,
            title,
            status,
            planned_for,
            slot,
            slot_pos,
            created_at,
            completed_at
        ],
    )
    .expect("task");
}

#[test]
fn period_math_normalizes_and_labels() {
    let today = NaiveDate::from_ymd_opt(2026, 1, 21).expect("date");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Week, None, today, 0).expect("week");
    assert_eq!(start.as_deref(), Some("2026-01-19"));
    assert_eq!(end.as_deref(), Some("2026-01-25"));
    assert_eq!(label, "Week of 19 Jan 2026");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Week, None, today, 1).expect("week");
    assert_eq!(start.as_deref(), Some("2026-01-20"));
    assert_eq!(end.as_deref(), Some("2026-01-26"));
    assert_eq!(label, "Week of 20 Jan 2026");

    let (start, _, _) =
        horizon::period_bounds(HorizonKind::Week, Some("2026-01-22"), today, 0).expect("week");
    assert_eq!(start.as_deref(), Some("2026-01-19"));

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Month, None, today, 0).expect("month");
    assert_eq!(start.as_deref(), Some("2026-01-01"));
    assert_eq!(end.as_deref(), Some("2026-01-31"));
    assert_eq!(label, "January 2026");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Month, Some("2026-02-17"), today, 0).expect("month");
    assert_eq!(start.as_deref(), Some("2026-02-01"));
    assert_eq!(end.as_deref(), Some("2026-02-28"));
    assert_eq!(label, "February 2026");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Quarter, None, today, 0).expect("quarter");
    assert_eq!(start.as_deref(), Some("2026-01-01"));
    assert_eq!(end.as_deref(), Some("2026-03-31"));
    assert_eq!(label, "Q1 2026");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Quarter, Some("2026-11-05"), today, 0)
            .expect("quarter");
    assert_eq!(start.as_deref(), Some("2026-10-01"));
    assert_eq!(end.as_deref(), Some("2026-12-31"));
    assert_eq!(label, "Q4 2026");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::Year, None, today, 0).expect("year");
    assert_eq!(start.as_deref(), Some("2026-01-01"));
    assert_eq!(end.as_deref(), Some("2026-12-31"));
    assert_eq!(label, "2026");

    let (start, end, label) =
        horizon::period_bounds(HorizonKind::LongTerm, Some("2026-01-01"), today, 0)
            .expect("long term");
    assert_eq!(start, None);
    assert_eq!(end, None);
    assert_eq!(label, "Long term");

    let error = horizon::period_bounds(HorizonKind::Week, Some("not-a-date"), today, 0)
        .expect_err("invalid date");
    assert_eq!(error.code, ErrorCode::InvalidSetting);
}

#[test]
fn band_capacity_is_one_three_five_and_more_is_unlimited() {
    let mut fixture = Fixture::new();
    fixture.goal(HorizonKind::Week, Band::Focus, "Focus");
    let input = fixture.input(HorizonKind::Week, Band::Focus, "Second focus");
    let error =
        horizon::create_goal(&mut fixture.conn, &fixture.clock, input).expect_err("focus is full");
    assert_eq!(error.code, ErrorCode::SlotFull);
    assert_eq!(error.message, "That band is full.");

    for index in 0..3 {
        let goal = fixture.goal(
            HorizonKind::Week,
            Band::Secondary,
            &format!("Secondary {index}"),
        );
        assert_eq!(goal.position, index + 1);
    }
    let input = fixture.input(HorizonKind::Week, Band::Secondary, "Fourth secondary");
    let error = horizon::create_goal(&mut fixture.conn, &fixture.clock, input)
        .expect_err("secondary is full");
    assert_eq!(error.code, ErrorCode::SlotFull);

    for index in 0..5 {
        let goal = fixture.goal(
            HorizonKind::Week,
            Band::Commitment,
            &format!("Commitment {index}"),
        );
        assert_eq!(goal.position, index + 1);
    }
    let input = fixture.input(HorizonKind::Week, Band::Commitment, "Sixth commitment");
    let error = horizon::create_goal(&mut fixture.conn, &fixture.clock, input)
        .expect_err("commitment is full");
    assert_eq!(error.code, ErrorCode::SlotFull);

    for index in 0..8 {
        let goal = fixture.goal(HorizonKind::Week, Band::More, &format!("More {index}"));
        assert_eq!(goal.position, index + 1);
    }
    let view = fixture.view(HorizonKind::Week, None);
    assert_eq!(view.bands.more.len(), 8);
}

#[test]
fn moving_goals_reindexes_bands_and_respects_capacity() {
    let mut fixture = Fixture::new();
    let first = fixture.goal(HorizonKind::Week, Band::Secondary, "First");
    let second = fixture.goal(HorizonKind::Week, Band::Secondary, "Second");
    let third = fixture.goal(HorizonKind::Week, Band::Secondary, "Third");
    assert_eq!((first.position, second.position, third.position), (1, 2, 3));
    let focus = fixture.goal(HorizonKind::Week, Band::Focus, "Focus");

    let input = fixture.input(HorizonKind::Week, Band::More, "Second");
    let moved = horizon::update_goal(&mut fixture.conn, &fixture.clock, &second.id, input)
        .expect("move to more");
    assert_eq!(moved.band, Band::More);
    assert_eq!(moved.position, 1);

    let view = fixture.view(HorizonKind::Week, None);
    let positions: Vec<(String, i64)> = view
        .bands
        .secondary
        .iter()
        .map(|item| (item.goal.title.clone(), item.goal.position))
        .collect();
    assert_eq!(
        positions,
        vec![("First".to_string(), 1), ("Third".to_string(), 2)]
    );

    let fourth = fixture.goal(HorizonKind::Week, Band::Secondary, "Fourth");
    assert_eq!(fourth.position, 3);

    let input = fixture.input(HorizonKind::Week, Band::Focus, "Fourth");
    let error = horizon::update_goal(&mut fixture.conn, &fixture.clock, &fourth.id, input)
        .expect_err("focus is full");
    assert_eq!(error.code, ErrorCode::SlotFull);

    let input = fixture.input(HorizonKind::Week, Band::More, "Focus");
    horizon::update_goal(&mut fixture.conn, &fixture.clock, &focus.id, input)
        .expect("move focus out");

    let input = fixture.input(HorizonKind::Week, Band::Focus, "Fourth");
    let promoted = horizon::update_goal(&mut fixture.conn, &fixture.clock, &fourth.id, input)
        .expect("promote to focus");
    assert_eq!(promoted.band, Band::Focus);
    assert_eq!(promoted.position, 1);
    assert_eq!(
        fixture
            .view(HorizonKind::Week, None)
            .bands
            .focus
            .expect("focus")
            .goal
            .id,
        fourth.id
    );
}

#[test]
fn goals_complete_and_reopen_with_notes() {
    let mut fixture = Fixture::new();
    let goal = fixture.goal(HorizonKind::Week, Band::Focus, "Ship the release");
    horizon::set_note(
        &mut fixture.conn,
        &fixture.clock,
        HorizonKind::Week,
        "2026-01-19",
        Some("One clear outcome".into()),
        Some("Looks healthy".into()),
    )
    .expect("note");

    let view = fixture.view(HorizonKind::Week, None);
    assert_eq!(view.intent.as_deref(), Some("One clear outcome"));
    assert_eq!(view.review_note.as_deref(), Some("Looks healthy"));
    assert_eq!(
        view.bands.focus.as_ref().map(|item| item.goal.id.as_str()),
        Some(goal.id.as_str())
    );

    let completed =
        horizon::complete_goal(&mut fixture.conn, &fixture.clock, &goal.id).expect("complete");
    assert_eq!(completed.status, GoalStatus::Done);
    assert!(completed.completed_at.is_some());
    assert!(fixture.view(HorizonKind::Week, None).bands.focus.is_none());

    let reopened =
        horizon::reopen_goal(&mut fixture.conn, &fixture.clock, &goal.id).expect("reopen");
    assert_eq!(reopened.status, GoalStatus::Open);
    assert!(reopened.completed_at.is_none());
    assert_eq!(
        fixture
            .view(HorizonKind::Week, None)
            .bands
            .focus
            .expect("focus")
            .goal
            .id,
        goal.id
    );
}

#[test]
fn task_linking_counts_titles_and_week_days() {
    let mut fixture = Fixture::new();
    let goal = fixture.goal(HorizonKind::Week, Band::Focus, "Launch");

    insert_task(
        &fixture.conn,
        "task-open",
        "Write the announcement",
        "READY",
        Some("2026-01-20"),
        Some("THREE"),
        Some(1),
        "2026-01-01T09:00:00Z",
    );
    insert_task(
        &fixture.conn,
        "task-done",
        "Book the venue",
        "DONE",
        Some("2026-01-21"),
        Some("ONE"),
        Some(1),
        "2026-01-01T09:01:00Z",
    );
    fixture
        .conn
        .execute(
            "INSERT INTO days (day, plan_locked_at) VALUES ('2026-01-21', '2026-01-21T09:00:00Z')",
            [],
        )
        .expect("day");

    horizon::link_task(&mut fixture.conn, &fixture.clock, "task-open", &goal.id).expect("link");
    horizon::link_task(&mut fixture.conn, &fixture.clock, "task-done", &goal.id).expect("link");

    let view = fixture.view(HorizonKind::Week, None);
    let focus = view.bands.focus.expect("focus");
    assert_eq!(focus.linked_tasks, 2);
    assert_eq!(focus.done_tasks, 1);
    assert_eq!(focus.linked_titles.len(), 2);

    assert_eq!(view.days.len(), 7);
    let jan20 = view
        .days
        .iter()
        .find(|day| day.day == "2026-01-20")
        .expect("jan 20");
    assert_eq!(jan20.open_count, 1);
    assert_eq!(jan20.done_count, 0);
    assert_eq!(jan20.planned_mins, 30);
    assert!(!jan20.locked);
    let jan21 = view
        .days
        .iter()
        .find(|day| day.day == "2026-01-21")
        .expect("jan 21");
    assert_eq!(jan21.open_count, 0);
    assert_eq!(jan21.done_count, 1);
    assert_eq!(jan21.planned_mins, 0);
    assert!(jan21.locked);
    assert!(fixture.view(HorizonKind::Month, None).days.is_empty());

    horizon::unlink_task(&mut fixture.conn, "task-open", &goal.id).expect("unlink");
    horizon::link_task(&mut fixture.conn, &fixture.clock, "task-done", &goal.id).expect("relink");
    let focus = fixture
        .view(HorizonKind::Week, None)
        .bands
        .focus
        .expect("focus");
    assert_eq!(focus.linked_tasks, 1);
    assert_eq!(focus.done_tasks, 1);
}

#[test]
fn linked_titles_are_limited_to_eight() {
    let mut fixture = Fixture::new();
    let goal = fixture.goal(HorizonKind::Week, Band::Focus, "Many tasks");
    for index in 0..10 {
        let id = format!("task-{index}");
        let title = format!("Task {index}");
        insert_task(
            &fixture.conn,
            &id,
            &title,
            "INBOX",
            None,
            None,
            None,
            &format!("2026-01-01T09:{index:02}:00Z"),
        );
        horizon::link_task(&mut fixture.conn, &fixture.clock, &id, &goal.id).expect("link");
    }
    let focus = fixture
        .view(HorizonKind::Week, None)
        .bands
        .focus
        .expect("focus");
    assert_eq!(focus.linked_tasks, 10);
    assert_eq!(focus.linked_titles.len(), 8);
    assert_eq!(focus.linked_titles[0], "Task 0");
}

#[test]
fn areas_are_created_and_listed() {
    let mut fixture = Fixture::new();
    let area = horizon::create_area(
        &mut fixture.conn,
        &fixture.clock,
        AreaInput {
            name: "Health".into(),
            color: None,
            sort_order: None,
        },
    )
    .expect("area");
    assert_eq!(area.name, "Health");
    assert_eq!(area.color, "forest");
    assert!(area.active);
    assert_eq!(area.sort_order, 0);

    horizon::create_area(
        &mut fixture.conn,
        &fixture.clock,
        AreaInput {
            name: "Work".into(),
            color: Some("ocean".into()),
            sort_order: Some(5),
        },
    )
    .expect("area");

    let areas = horizon::list_areas(&fixture.conn).expect("areas");
    assert_eq!(areas.len(), 2);
    assert_eq!(areas[0].name, "Health");
    assert_eq!(areas[1].name, "Work");
    assert_eq!(areas[1].color, "ocean");
}

#[test]
fn goals_are_listed_under_each_timeframe() {
    let mut fixture = Fixture::new();
    for kind in [
        HorizonKind::Week,
        HorizonKind::Month,
        HorizonKind::Quarter,
        HorizonKind::Year,
        HorizonKind::LongTerm,
    ] {
        let title = format!("Plan for {kind:?}");
        let goal = fixture.goal(kind, Band::Focus, &title);
        let view = fixture.view(kind, None);
        let listed = view.bands.focus.expect("focus");
        assert_eq!(listed.goal.id, goal.id);
        assert_eq!(listed.goal.horizon_kind, kind);
        assert_eq!(listed.goal.period_start, goal.period_start);
    }
}

#[test]
fn goal_estimate_is_stored_and_updated() {
    let mut fixture = Fixture::new();
    let mut input = fixture.input(HorizonKind::Week, Band::Focus, "Write the draft");
    input.estimate_mins = Some(60);
    let goal = horizon::create_goal(&mut fixture.conn, &fixture.clock, input).expect("goal");
    assert_eq!(goal.estimate_mins, Some(60));

    let focus = fixture
        .view(HorizonKind::Week, None)
        .bands
        .focus
        .expect("focus");
    assert_eq!(focus.goal.estimate_mins, Some(60));

    let mut update = fixture.input(HorizonKind::Week, Band::Focus, "Write the draft");
    update.estimate_mins = Some(90);
    let updated =
        horizon::update_goal(&mut fixture.conn, &fixture.clock, &goal.id, update).expect("goal");
    assert_eq!(updated.estimate_mins, Some(90));
}
