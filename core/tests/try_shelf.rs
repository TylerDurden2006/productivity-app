use chrono::{Duration, TimeZone, Utc};
use flow_core::error::ErrorCode;
use flow_core::model::{
    EnergyLevel, SocialMode, TimeOfDay, TryCost, TryEffort, TryItem, TryItemInput, TryStatus,
};
use flow_core::rules::try_shelf;
use flow_core::{db, FixedClock};
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

    fn create(&mut self, title: &str) -> TryItem {
        try_shelf::create(&mut self.conn, &self.clock, input(title)).expect("create")
    }

    fn advance(&self, minutes: i64) {
        self.clock.advance(Duration::minutes(minutes));
    }

    fn add_task(&self, id: &str) {
        let title = format!("Fixture task {id}");
        self.conn
            .execute(
                "INSERT INTO tasks (id, title, status, value, pinned, rollover_count, created_at, \
                 updated_at) VALUES (?1, ?2, 'INBOX', 3, 0, 0, '2026-05-04T09:00:00Z', \
                 '2026-05-04T09:00:00Z')",
                rusqlite::params![id, title],
            )
            .expect("task");
    }
}

fn input(title: &str) -> TryItemInput {
    TryItemInput {
        title: title.to_string(),
        notes: None,
        area_id: None,
        category: None,
        link: None,
        effort: None,
        cost: None,
        social_mode: None,
        energy: None,
    }
}

fn error_code<T: std::fmt::Debug>(result: flow_core::Result<T>) -> ErrorCode {
    result.expect_err("expected an error").code
}

#[test]
fn create_applies_defaults_and_normalizes_text() {
    let mut fixture = Fixture::new();
    let item = try_shelf::create(
        &mut fixture.conn,
        &fixture.clock,
        TryItemInput {
            title: "  Learn to sail  ".into(),
            notes: Some("  coastal course  ".into()),
            area_id: None,
            category: Some("   ".into()),
            link: Some("  https://example.com/sail  ".into()),
            effort: None,
            cost: None,
            social_mode: None,
            energy: None,
        },
    )
    .expect("create");

    assert_eq!(item.title, "Learn to sail");
    assert_eq!(item.notes.as_deref(), Some("coastal course"));
    assert_eq!(item.category, None);
    assert_eq!(item.link.as_deref(), Some("https://example.com/sail"));
    assert_eq!(item.status, TryStatus::Idea);
    assert_eq!(item.effort, TryEffort::Medium);
    assert_eq!(item.cost, TryCost::Low);
    assert_eq!(item.social_mode, SocialMode::Either);
    assert_eq!(item.energy, EnergyLevel::Medium);
    assert_eq!(item.scheduled_for, None);
    assert_eq!(item.scheduled_day_part, None);
    assert_eq!(item.linked_task_id, None);
    assert_eq!(item.rating, None);
    assert_eq!(item.reflection, None);
    assert_eq!(item.tried_at, None);
    assert_eq!(item.created_at, "2026-05-04T09:00:00Z");
    assert_eq!(item.updated_at, item.created_at);
    assert_eq!(
        try_shelf::get(&fixture.conn, &item.id).expect("get").id,
        item.id
    );
}

#[test]
fn create_honors_explicit_classification() {
    let mut fixture = Fixture::new();
    let item = try_shelf::create(
        &mut fixture.conn,
        &fixture.clock,
        TryItemInput {
            title: "Pottery class".into(),
            notes: None,
            area_id: None,
            category: Some("Craft".into()),
            link: None,
            effort: Some(TryEffort::High),
            cost: Some(TryCost::Free),
            social_mode: Some(SocialMode::Social),
            energy: Some(EnergyLevel::Low),
        },
    )
    .expect("create");

    assert_eq!(item.effort, TryEffort::High);
    assert_eq!(item.cost, TryCost::Free);
    assert_eq!(item.social_mode, SocialMode::Social);
    assert_eq!(item.energy, EnergyLevel::Low);
    assert_eq!(item.status, TryStatus::Idea);
}

#[test]
fn list_returns_items_in_creation_order() {
    let mut fixture = Fixture::new();
    let first = fixture.create("First");
    fixture.advance(1);
    let second = fixture.create("Second");
    fixture.advance(1);
    let third = fixture.create("Third");

    let items = try_shelf::list(&fixture.conn).expect("list");
    let ids = items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![first.id.as_str(), second.id.as_str(), third.id.as_str()]
    );
}

#[test]
fn create_and_update_reject_blank_titles() {
    let mut fixture = Fixture::new();
    assert_eq!(
        error_code(try_shelf::create(
            &mut fixture.conn,
            &fixture.clock,
            input("   ")
        )),
        ErrorCode::MissingRequiredField
    );
    assert!(try_shelf::list(&fixture.conn).expect("list").is_empty());

    let item = fixture.create("Sourdough");
    assert_eq!(
        error_code(try_shelf::update(
            &mut fixture.conn,
            &fixture.clock,
            &item.id,
            input("")
        )),
        ErrorCode::MissingRequiredField
    );
    assert_eq!(
        try_shelf::get(&fixture.conn, &item.id).expect("get").title,
        "Sourdough"
    );
}

#[test]
fn update_preserves_omitted_fields_and_leaves_status_alone() {
    let mut fixture = Fixture::new();
    let item = try_shelf::create(
        &mut fixture.conn,
        &fixture.clock,
        TryItemInput {
            title: "Pottery".into(),
            notes: Some("studio downtown".into()),
            area_id: None,
            category: Some("Craft".into()),
            link: Some("https://example.com/pottery".into()),
            effort: Some(TryEffort::High),
            cost: Some(TryCost::Free),
            social_mode: Some(SocialMode::Social),
            energy: Some(EnergyLevel::Low),
        },
    )
    .expect("create");
    let scheduled = try_shelf::schedule(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        "2026-05-10",
        TimeOfDay::Evening,
    )
    .expect("schedule");
    fixture.advance(5);

    let updated = try_shelf::update(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        input("Pottery wheel"),
    )
    .expect("update");
    assert_eq!(updated.title, "Pottery wheel");
    assert_eq!(updated.notes.as_deref(), Some("studio downtown"));
    assert_eq!(updated.category.as_deref(), Some("Craft"));
    assert_eq!(updated.link.as_deref(), Some("https://example.com/pottery"));
    assert_eq!(updated.effort, TryEffort::High);
    assert_eq!(updated.cost, TryCost::Free);
    assert_eq!(updated.social_mode, SocialMode::Social);
    assert_eq!(updated.energy, EnergyLevel::Low);
    assert_eq!(updated.status, TryStatus::Scheduled);
    assert_eq!(
        updated.scheduled_for.as_deref(),
        scheduled.scheduled_for.as_deref()
    );
    assert_eq!(updated.scheduled_day_part, Some(TimeOfDay::Evening));
    assert_eq!(updated.created_at, item.created_at);
    assert_eq!(updated.updated_at, "2026-05-04T09:05:00Z");

    let changed = try_shelf::update(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        TryItemInput {
            title: "Pottery wheel".into(),
            notes: Some("   ".into()),
            area_id: None,
            category: None,
            link: None,
            effort: Some(TryEffort::Low),
            cost: None,
            social_mode: Some(SocialMode::Solo),
            energy: None,
        },
    )
    .expect("update");
    assert_eq!(changed.notes, None);
    assert_eq!(changed.category.as_deref(), Some("Craft"));
    assert_eq!(changed.link.as_deref(), Some("https://example.com/pottery"));
    assert_eq!(changed.effort, TryEffort::Low);
    assert_eq!(changed.cost, TryCost::Free);
    assert_eq!(changed.social_mode, SocialMode::Solo);
    assert_eq!(changed.energy, EnergyLevel::Low);
}

#[test]
fn schedule_and_unschedule_transition_status_and_dates() {
    let mut fixture = Fixture::new();
    let item = fixture.create("Bouldering");

    let scheduled = try_shelf::schedule(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        "2026-05-10",
        TimeOfDay::Morning,
    )
    .expect("schedule");
    assert_eq!(scheduled.status, TryStatus::Scheduled);
    assert_eq!(scheduled.scheduled_for.as_deref(), Some("2026-05-10"));
    assert_eq!(scheduled.scheduled_day_part, Some(TimeOfDay::Morning));

    fixture.advance(1);
    let unscheduled =
        try_shelf::unschedule(&mut fixture.conn, &fixture.clock, &item.id).expect("unschedule");
    assert_eq!(unscheduled.status, TryStatus::Idea);
    assert_eq!(unscheduled.scheduled_for, None);
    assert_eq!(unscheduled.scheduled_day_part, None);
    assert_eq!(unscheduled.updated_at, "2026-05-04T09:01:00Z");
}

#[test]
fn schedule_rejects_malformed_dates() {
    let mut fixture = Fixture::new();
    let item = fixture.create("Birdwatching");

    for bad in ["05/10/2026", "2026-5-4", "2026-02-30", "   "] {
        assert_eq!(
            error_code(try_shelf::schedule(
                &mut fixture.conn,
                &fixture.clock,
                &item.id,
                bad,
                TimeOfDay::Anytime
            )),
            ErrorCode::InvalidSetting,
            "accepted {bad:?}"
        );
    }

    let stored = try_shelf::get(&fixture.conn, &item.id).expect("get");
    assert_eq!(stored.status, TryStatus::Idea);
    assert_eq!(stored.scheduled_for, None);
    assert_eq!(stored.scheduled_day_part, None);
}

#[test]
fn mark_tried_records_status_rating_reflection_and_timestamp() {
    let mut fixture = Fixture::new();
    let item = fixture.create("Cold plunge");
    fixture.advance(3);

    let tried = try_shelf::mark_tried(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        Some(5),
        Some("  bracing  ".into()),
    )
    .expect("mark tried");
    assert_eq!(tried.status, TryStatus::Tried);
    assert_eq!(tried.rating, Some(5));
    assert_eq!(tried.reflection.as_deref(), Some("bracing"));
    assert_eq!(tried.tried_at.as_deref(), Some("2026-05-04T09:03:00Z"));
    assert_eq!(tried.updated_at, "2026-05-04T09:03:00Z");
    assert_eq!(
        try_shelf::get(&fixture.conn, &item.id).expect("get").rating,
        Some(5)
    );
}

#[test]
fn mark_tried_allows_an_omitted_rating() {
    let mut fixture = Fixture::new();
    let item = fixture.create("Open mic night");

    let tried = try_shelf::mark_tried(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        None,
        Some("glad I went".into()),
    )
    .expect("mark tried");
    assert_eq!(tried.status, TryStatus::Tried);
    assert_eq!(tried.rating, None);
    assert_eq!(tried.reflection.as_deref(), Some("glad I went"));
}

#[test]
fn mark_tried_rejects_out_of_range_ratings() {
    let mut fixture = Fixture::new();
    let item = fixture.create("Sauna");

    for bad in [0, 6, -1] {
        assert_eq!(
            error_code(try_shelf::mark_tried(
                &mut fixture.conn,
                &fixture.clock,
                &item.id,
                Some(bad),
                None
            )),
            ErrorCode::InvalidSetting,
            "accepted {bad}"
        );
    }

    let stored = try_shelf::get(&fixture.conn, &item.id).expect("get");
    assert_eq!(stored.status, TryStatus::Idea);
    assert_eq!(stored.rating, None);
    assert_eq!(stored.tried_at, None);
}

#[test]
fn skip_marks_item_skipped() {
    let mut fixture = Fixture::new();
    let item = fixture.create("Karaoke");

    let skipped = try_shelf::skip_item(&mut fixture.conn, &fixture.clock, &item.id).expect("skip");
    assert_eq!(skipped.status, TryStatus::Skipped);
    assert_eq!(skipped.scheduled_for, None);
    assert_eq!(
        try_shelf::get(&fixture.conn, &item.id).expect("get").status,
        TryStatus::Skipped
    );
}

#[test]
fn linked_task_must_exist_and_can_be_cleared() {
    let mut fixture = Fixture::new();
    fixture.add_task("task-1");
    let item = fixture.create("Film photography");

    let linked = try_shelf::set_linked_task(
        &mut fixture.conn,
        &fixture.clock,
        &item.id,
        Some("task-1".into()),
    )
    .expect("link");
    assert_eq!(linked.linked_task_id.as_deref(), Some("task-1"));

    assert_eq!(
        error_code(try_shelf::set_linked_task(
            &mut fixture.conn,
            &fixture.clock,
            &item.id,
            Some("missing-task".into())
        )),
        ErrorCode::NotFound
    );
    assert_eq!(
        try_shelf::get(&fixture.conn, &item.id)
            .expect("get")
            .linked_task_id
            .as_deref(),
        Some("task-1")
    );

    let cleared = try_shelf::set_linked_task(&mut fixture.conn, &fixture.clock, &item.id, None)
        .expect("clear");
    assert_eq!(cleared.linked_task_id, None);
}

#[test]
fn missing_items_return_not_found() {
    let mut fixture = Fixture::new();
    assert_eq!(
        error_code(try_shelf::get(&fixture.conn, "missing")),
        ErrorCode::NotFound
    );
    assert_eq!(
        error_code(try_shelf::update(
            &mut fixture.conn,
            &fixture.clock,
            "missing",
            input("Thing")
        )),
        ErrorCode::NotFound
    );
    assert_eq!(
        error_code(try_shelf::unschedule(
            &mut fixture.conn,
            &fixture.clock,
            "missing"
        )),
        ErrorCode::NotFound
    );
}
