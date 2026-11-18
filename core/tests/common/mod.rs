#![allow(dead_code)]

use chrono::{DateTime, Duration, TimeZone, Utc};
use flow_core::error::{AppError, ErrorCode};
use flow_core::model::{
    ClarifyInput, IfFull, PlanView, ResolveInput, SettingsPatch, Slot, Task, TaskPatch,
};
use flow_core::{api, db, FixedClock};
use rusqlite::Connection;

pub struct App {
    pub conn: Connection,
    pub clock: FixedClock,
}

impl App {
    pub fn new() -> Self {
        let mut conn = db::open_memory().expect("database");
        let clock = FixedClock::utc(
            Utc.with_ymd_and_hms(2026, 5, 4, 9, 0, 0)
                .single()
                .expect("clock"),
        );
        api::update_settings(
            &mut conn,
            &clock,
            SettingsPatch {
                day_rollover_hour: None,
                daily_focus_budget_mins: None,
                ready_cap: None,
                focus_block_mins: None,
                batch_block_mins: None,
                default_multiplier: Some(1.0),
                last_weekly_review_at: None,
                week_starts_on: None,
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
        Self { conn, clock }
    }

    pub fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Self {
        let app = Self::new();
        app.set_time(year, month, day, hour, minute);
        app
    }

    pub fn set_time(&self, year: i32, month: u32, day: u32, hour: u32, minute: u32) {
        self.clock.set(
            Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
                .single()
                .expect("clock"),
        );
    }

    pub fn advance_minutes(&self, minutes: i64) {
        self.clock.advance(Duration::minutes(minutes));
    }

    pub fn set_multiplier(&mut self, value: f64) {
        api::update_settings(
            &mut self.conn,
            &self.clock,
            SettingsPatch {
                day_rollover_hour: None,
                daily_focus_budget_mins: None,
                ready_cap: None,
                focus_block_mins: None,
                batch_block_mins: None,
                default_multiplier: Some(value),
                last_weekly_review_at: None,
                week_starts_on: None,
                auto_carry_enabled: None,
                dashboard_default_range: None,
                notes_autosave: None,
                notify_block_end: None,
                planning_reminder_enabled: None,
                planning_reminder_time: None,
                reduced_motion: None,
            },
        )
        .expect("multiplier");
    }

    pub fn set_ready_cap(&mut self, value: i64) {
        api::update_settings(
            &mut self.conn,
            &self.clock,
            SettingsPatch {
                day_rollover_hour: None,
                daily_focus_budget_mins: None,
                ready_cap: Some(value),
                focus_block_mins: None,
                batch_block_mins: None,
                default_multiplier: None,
                last_weekly_review_at: None,
                week_starts_on: None,
                auto_carry_enabled: None,
                dashboard_default_range: None,
                notes_autosave: None,
                notify_block_end: None,
                planning_reminder_enabled: None,
                planning_reminder_time: None,
                reduced_motion: None,
            },
        )
        .expect("ready cap");
    }

    pub fn capture(&mut self, title: &str) -> Task {
        api::capture_task(&mut self.conn, &self.clock, title).expect("capture")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn clarify(
        &mut self,
        id: &str,
        title: &str,
        first_step: Option<&str>,
        estimate: Option<i64>,
        value: Option<i64>,
        deadline: Option<&str>,
        if_full: Option<IfFull>,
        reason: Option<&str>,
    ) -> flow_core::Result<Task> {
        api::clarify_task(
            &mut self.conn,
            &self.clock,
            id,
            ClarifyInput {
                title: title.to_string(),
                first_step: first_step.map(str::to_string),
                done_when: None,
                estimate_mins: estimate,
                value,
                deadline_on: deadline.map(str::to_string),
            },
            if_full,
            reason.map(str::to_string),
        )
    }

    pub fn ready(&mut self, title: &str, estimate: i64) -> Task {
        self.ready_full(title, estimate, 3, None)
    }

    pub fn ready_full(
        &mut self,
        title: &str,
        estimate: i64,
        value: i64,
        deadline: Option<&str>,
    ) -> Task {
        let task = self.capture(title);
        self.clarify(
            &task.id,
            title,
            Some("Start now"),
            Some(estimate),
            Some(value),
            deadline,
            None,
            None,
        )
        .expect("clarify")
    }

    pub fn task(&self, id: &str) -> Task {
        db::require_task(&self.conn, id).expect("task")
    }

    pub fn today(&self) -> String {
        db::today_string(&self.conn, &self.clock).expect("today")
    }

    pub fn tomorrow(&self) -> String {
        let today = db::today(&self.conn, &self.clock).expect("today");
        flow_core::clock::format_date(flow_core::clock::add_days(today, 1))
    }

    pub fn assign(&mut self, day: &str, id: &str, slot: Slot) -> flow_core::Result<Task> {
        api::assign_task(&mut self.conn, &self.clock, day, id, slot, None)
    }

    pub fn assign_reason(
        &mut self,
        day: &str,
        id: &str,
        slot: Slot,
        reason: Option<&str>,
    ) -> flow_core::Result<Task> {
        api::assign_task(
            &mut self.conn,
            &self.clock,
            day,
            id,
            slot,
            reason.map(str::to_string),
        )
    }

    pub fn lock_today(&mut self) -> flow_core::Result<flow_core::model::LockResult> {
        let today = self.today();
        api::lock_plan(&mut self.conn, &self.clock, &today, None)
    }

    pub fn complete(&mut self, id: &str) -> flow_core::Result<Task> {
        api::complete_task(&mut self.conn, &self.clock, id, None)
    }

    pub fn plan(&self, day: Option<&str>) -> PlanView {
        api::plan_view(&self.conn, &self.clock, day.map(str::to_string)).expect("plan")
    }

    pub fn update(&mut self, id: &str, patch: TaskPatch) -> flow_core::Result<Task> {
        api::update_task(&mut self.conn, &self.clock, id, patch)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        &mut self,
        id: &str,
        outcome: flow_core::model::OutcomeKind,
        waiting_on: Option<&str>,
        review_on: Option<&str>,
        breadcrumb: Option<&str>,
        first_step: Option<&str>,
        estimate: Option<i64>,
    ) -> flow_core::Result<Task> {
        api::resolve_task(
            &mut self.conn,
            &self.clock,
            id,
            ResolveInput {
                outcome,
                waiting_on: waiting_on.map(str::to_string),
                review_on: review_on.map(str::to_string),
                breadcrumb: breadcrumb.map(str::to_string),
                children: None,
                first_step: first_step.map(str::to_string),
                estimate_mins: estimate,
            },
        )
    }

    pub fn event_count(&self, kind: &str) -> i64 {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE kind = ?1",
                [kind],
                |row| row.get(0),
            )
            .expect("event count")
    }

    pub fn session_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .expect("session count")
    }

    pub fn set_rollover(&self, id: &str, count: i64) {
        self.conn
            .execute(
                "UPDATE tasks SET rollover_count = ?2 WHERE id = ?1",
                rusqlite::params![id, count],
            )
            .expect("rollover");
    }

    pub fn task_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .expect("task count")
    }
}

pub fn code(error: AppError) -> ErrorCode {
    error.code
}

pub fn assert_code<T>(result: flow_core::Result<T>, expected: ErrorCode) {
    let error = result.err().expect("expected an error");
    assert_eq!(error.code, expected, "{}", error.message);
}

pub fn num(value: DateTime<Utc>) -> i64 {
    value.timestamp()
}
