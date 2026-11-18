use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::clock::{now_rfc3339, parse_rfc3339, Clock};
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    ClosedReason, Day, Event, Recurrence, Session, SessionKind, Slot, Status, Task,
};

const SCHEMA_SQL: &str = include_str!("../migrations/001_init.sql");
const MIGRATIONS: &[(i64, &str)] = &[
    (2, include_str!("../migrations/002_planning.sql")),
    (3, include_str!("../migrations/003_habits.sql")),
    (4, include_str!("../migrations/004_try.sql")),
    (5, include_str!("../migrations/005_flexible.sql")),
    (6, include_str!("../migrations/006_carry_notes.sql")),
    (7, include_str!("../migrations/007_goal_estimate.sql")),
    (8, include_str!("../migrations/008_midnight_rollover.sql")),
    (9, include_str!("../migrations/009_open_slots.sql")),
];
pub const CURRENT_SCHEMA_VERSION: i64 = 9;

pub const TASK_COLUMNS: &str =
    "id, title, first_step, done_when, estimate_mins, status, value, pinned, \
deadline_on, waiting_on, review_on, breadcrumb, cue, parent_id, closed_reason, rollover_count, \
planned_for, slot, slot_pos, recurrence_id, occurrence_on, created_at, updated_at, completed_at";

const SESSION_COLUMNS: &str = "id, kind, task_id, day, planned_mins, extended, accumulated_secs, \
running_since, last_seen_at, started_at, ended_at";

const RECURRENCE_COLUMNS: &str =
    "id, title, first_step, done_when, estimate_mins, value, cue, freq, \
interval_n, weekdays_mask, starts_on, active, created_at, updated_at";

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

pub fn migrate(conn: &Connection) -> Result<()> {
    let mut version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 0 {
        conn.execute_batch(SCHEMA_SQL)?;
        version = 1;
    }
    if version > CURRENT_SCHEMA_VERSION {
        return Err(AppError::hard(
            ErrorCode::Internal,
            format!("unsupported database schema version {version}"),
        ));
    }
    for (target, sql) in MIGRATIONS {
        if version < *target {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", target)?;
            version = *target;
        }
    }
    if version != CURRENT_SCHEMA_VERSION {
        return Err(AppError::hard(
            ErrorCode::Internal,
            format!("unsupported database schema version {version}"),
        ));
    }
    Ok(())
}

pub fn integrity_ok(conn: &Connection) -> Result<bool> {
    let result: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    Ok(result == "ok")
}

pub fn with_tx<T>(
    conn: &mut Connection,
    f: impl FnOnce(&Transaction<'_>) -> Result<T>,
) -> Result<T> {
    let tx = conn.transaction()?;
    let value = f(&tx)?;
    tx.commit()?;
    Ok(value)
}

pub fn now(clock: &dyn Clock) -> String {
    now_rfc3339(clock)
}

pub fn now_dt(clock: &dyn Clock) -> DateTime<Utc> {
    clock.now_utc()
}

pub fn today(conn: &Connection, clock: &dyn Clock) -> Result<NaiveDate> {
    let rollover = rollover_hour(conn)?;
    Ok(clock.logical_today(rollover as u32))
}

pub fn today_string(conn: &Connection, clock: &dyn Clock) -> Result<String> {
    Ok(crate::clock::today_string(today(conn, clock)?))
}

pub fn rollover_hour(conn: &Connection) -> Result<i64> {
    Ok(settings(conn)?.day_rollover_hour)
}

pub fn settings(conn: &Connection) -> Result<crate::model::Settings> {
    let row = conn
        .query_row(
            "SELECT id, day_rollover_hour, daily_focus_budget_mins, ready_cap, focus_block_mins, \
             batch_block_mins, default_multiplier, last_weekly_review_at, week_starts_on, \
             auto_carry_enabled, dashboard_default_range, notes_autosave, notify_block_end, \
             planning_reminder_enabled, planning_reminder_time, reduced_motion \
             FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(crate::model::Settings {
                    id: row.get(0)?,
                    day_rollover_hour: row.get(1)?,
                    daily_focus_budget_mins: row.get(2)?,
                    ready_cap: row.get(3)?,
                    focus_block_mins: row.get(4)?,
                    batch_block_mins: row.get(5)?,
                    default_multiplier: row.get(6)?,
                    last_weekly_review_at: row.get(7)?,
                    week_starts_on: row.get(8)?,
                    auto_carry_enabled: row.get::<_, i64>(9)? != 0,
                    dashboard_default_range: row.get(10)?,
                    notes_autosave: row.get::<_, i64>(11)? != 0,
                    notify_block_end: row.get::<_, i64>(12)? != 0,
                    planning_reminder_enabled: row.get::<_, i64>(13)? != 0,
                    planning_reminder_time: row.get(14)?,
                    reduced_motion: row.get::<_, i64>(15)? != 0,
                })
            },
        )
        .optional()?;
    row.ok_or_else(|| AppError::internal("settings row missing"))
}

pub fn get_day(conn: &Connection, day: &str) -> Result<Option<Day>> {
    let row = conn
        .query_row(
            "SELECT day, plan_locked_at, multiplier_at_lock, budget_at_lock, wrapup_done_at, win_note \
             FROM days WHERE day = ?1",
            params![day],
            |row| {
                Ok(Day {
                    day: row.get(0)?,
                    plan_locked_at: row.get(1)?,
                    multiplier_at_lock: row.get(2)?,
                    budget_at_lock: row.get(3)?,
                    wrapup_done_at: row.get(4)?,
                    win_note: row.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn day_or_default(conn: &Connection, day: &str) -> Result<Day> {
    Ok(get_day(conn, day)?.unwrap_or(Day {
        day: day.to_string(),
        plan_locked_at: None,
        multiplier_at_lock: None,
        budget_at_lock: None,
        wrapup_done_at: None,
        win_note: None,
    }))
}

pub fn ensure_day(tx: &Transaction<'_>, day: &str) -> Result<()> {
    tx.execute("INSERT OR IGNORE INTO days (day) VALUES (?1)", params![day])?;
    Ok(())
}

pub fn task_from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
    let status_text: String = row.get(5)?;
    let pinned_int: i64 = row.get(7)?;
    let closed_text: Option<String> = row.get(14)?;
    let slot_text: Option<String> = row.get(17)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        first_step: row.get(2)?,
        done_when: row.get(3)?,
        estimate_mins: row.get(4)?,
        status: Status::from_db(&status_text),
        value: row.get(6)?,
        pinned: pinned_int != 0,
        deadline_on: row.get(8)?,
        waiting_on: row.get(9)?,
        review_on: row.get(10)?,
        breadcrumb: row.get(11)?,
        cue: row.get(12)?,
        parent_id: row.get(13)?,
        closed_reason: closed_text.map(|text| ClosedReason::from_db(&text)),
        rollover_count: row.get(15)?,
        planned_for: row.get(16)?,
        slot: slot_text.map(|text| Slot::from_db(&text)),
        slot_pos: row.get(18)?,
        recurrence_id: row.get(19)?,
        occurrence_on: row.get(20)?,
        created_at: row.get(21)?,
        updated_at: row.get(22)?,
        completed_at: row.get(23)?,
    })
}

pub fn session_from_row(row: &Row<'_>, now: DateTime<Utc>) -> rusqlite::Result<Session> {
    let kind_text: String = row.get(1)?;
    let extended_int: i64 = row.get(5)?;
    let accumulated: i64 = row.get(6)?;
    let running_since: Option<String> = row.get(7)?;
    let ended_at: Option<String> = row.get(10)?;
    let running = running_since.is_some() && ended_at.is_none();
    let live_secs = running_since
        .as_deref()
        .and_then(parse_rfc3339)
        .map(|since| (now - since).num_seconds().max(0))
        .unwrap_or(0);
    Ok(Session {
        id: row.get(0)?,
        kind: SessionKind::from_db(&kind_text),
        task_id: row.get(2)?,
        day: row.get(3)?,
        planned_mins: row.get(4)?,
        extended: extended_int != 0,
        accumulated_secs: accumulated,
        running_since,
        last_seen_at: row.get(8)?,
        started_at: row.get(9)?,
        ended_at,
        elapsed_secs: accumulated + live_secs,
        running,
    })
}

pub fn recurrence_from_row(row: &Row<'_>) -> rusqlite::Result<Recurrence> {
    let freq_text: String = row.get(7)?;
    let active_int: i64 = row.get(11)?;
    Ok(Recurrence {
        id: row.get(0)?,
        title: row.get(1)?,
        first_step: row.get(2)?,
        done_when: row.get(3)?,
        estimate_mins: row.get(4)?,
        value: row.get(5)?,
        cue: row.get(6)?,
        freq: crate::model::Freq::from_db(&freq_text),
        interval_n: row.get(8)?,
        weekdays_mask: row.get(9)?,
        starts_on: row.get(10)?,
        active: active_int != 0,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub fn write_task_row(tx: &Transaction<'_>, task: &Task) -> Result<()> {
    tx.execute(
        "UPDATE tasks SET title = ?2, first_step = ?3, done_when = ?4, estimate_mins = ?5, \
         status = ?6, value = ?7, pinned = ?8, deadline_on = ?9, waiting_on = ?10, review_on = ?11, \
         breadcrumb = ?12, cue = ?13, parent_id = ?14, closed_reason = ?15, rollover_count = ?16, \
         planned_for = ?17, slot = ?18, slot_pos = ?19, recurrence_id = ?20, occurrence_on = ?21, \
         created_at = ?22, updated_at = ?23, completed_at = ?24 WHERE id = ?1",
        params![
            task.id,
            task.title,
            task.first_step,
            task.done_when,
            task.estimate_mins,
            task.status.as_str(),
            task.value,
            if task.pinned { 1 } else { 0 },
            task.deadline_on,
            task.waiting_on,
            task.review_on,
            task.breadcrumb,
            task.cue,
            task.parent_id,
            task.closed_reason.map(|reason| reason.as_str()),
            task.rollover_count,
            task.planned_for,
            task.slot.map(|slot| slot.as_str()),
            task.slot_pos,
            task.recurrence_id,
            task.occurrence_on,
            task.created_at,
            task.updated_at,
            task.completed_at
        ],
    )?;
    Ok(())
}

pub fn insert_task_row(tx: &Transaction<'_>, task: &Task) -> Result<()> {
    tx.execute(
        "INSERT INTO tasks (id, title, first_step, done_when, estimate_mins, status, value, pinned, \
         deadline_on, waiting_on, review_on, breadcrumb, cue, parent_id, closed_reason, rollover_count, \
         planned_for, slot, slot_pos, recurrence_id, occurrence_on, created_at, updated_at, completed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, \
         ?20, ?21, ?22, ?23, ?24)",
        params![
            task.id,
            task.title,
            task.first_step,
            task.done_when,
            task.estimate_mins,
            task.status.as_str(),
            task.value,
            if task.pinned { 1 } else { 0 },
            task.deadline_on,
            task.waiting_on,
            task.review_on,
            task.breadcrumb,
            task.cue,
            task.parent_id,
            task.closed_reason.map(|reason| reason.as_str()),
            task.rollover_count,
            task.planned_for,
            task.slot.map(|slot| slot.as_str()),
            task.slot_pos,
            task.recurrence_id,
            task.occurrence_on,
            task.created_at,
            task.updated_at,
            task.completed_at
        ],
    )?;
    Ok(())
}

pub fn get_task(conn: &Connection, id: &str) -> Result<Option<Task>> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
    let task = conn
        .query_row(&sql, params![id], task_from_row)
        .optional()?;
    Ok(task)
}

pub fn require_task(conn: &Connection, id: &str) -> Result<Task> {
    get_task(conn, id)?.ok_or_else(|| AppError::not_found("task"))
}

pub fn list_inbox(conn: &Connection) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'INBOX' ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_by_status(conn: &Connection, status: Status) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = ?1 ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![status.as_str()], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_ready_candidates(conn: &Connection) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'READY' AND planned_for IS NULL \
         AND recurrence_id IS NULL AND estimate_mins >= 30 ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_quick_candidates(conn: &Connection) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'READY' AND planned_for IS NULL \
         AND recurrence_id IS NULL AND estimate_mins = 15 ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_planned(conn: &Connection, day: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'READY' AND planned_for = ?1 \
         ORDER BY CASE slot WHEN 'ONE' THEN 1 WHEN 'THREE' THEN 2 WHEN 'BATCH' THEN 3 ELSE 4 END, \
         slot_pos ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![day], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_plan_all(conn: &Connection, day: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE planned_for = ?1 AND slot IS NOT NULL \
         ORDER BY CASE slot WHEN 'ONE' THEN 1 WHEN 'THREE' THEN 2 WHEN 'BATCH' THEN 3 ELSE 4 END, \
         slot_pos ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![day], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_done_for_day(conn: &Connection, day: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE planned_for = ?1 AND status = 'DONE' \
         ORDER BY completed_at DESC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![day], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_due_recurring(conn: &Connection, today: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE recurrence_id IS NOT NULL AND status = 'READY' \
         AND occurrence_on <= ?1 ORDER BY occurrence_on ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_recently_dropped(conn: &Connection, limit: i64) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'DROPPED' ORDER BY updated_at DESC, id ASC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_waiting_due(conn: &Connection, today: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'WAITING' AND review_on IS NOT NULL \
         AND review_on <= ?1 ORDER BY review_on ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_review_due(conn: &Connection, today: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status IN ('WAITING', 'SOMEDAY') \
         AND review_on IS NOT NULL AND review_on <= ?1 ORDER BY review_on ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_stuck(conn: &Connection) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE status = 'READY' AND planned_for IS NULL \
         AND recurrence_id IS NULL AND rollover_count >= 3 ORDER BY rollover_count DESC, created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn ready_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'READY' AND planned_for IS NULL \
         AND recurrence_id IS NULL AND estimate_mins >= 30",
        [],
        |row| row.get(0),
    )?)
}

pub fn find_occurrence(conn: &Connection, recurrence_id: &str) -> Result<Option<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE recurrence_id = ?1 AND status = 'READY' \
         AND occurrence_on IS NOT NULL"
    );
    let task = conn
        .query_row(&sql, params![recurrence_id], task_from_row)
        .optional()?;
    Ok(task)
}

pub fn latest_occurrence_on(conn: &Connection, recurrence_id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT occurrence_on FROM tasks WHERE recurrence_id = ?1 \
             AND occurrence_on IS NOT NULL ORDER BY occurrence_on DESC LIMIT 1",
            params![recurrence_id],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn get_recurrence(conn: &Connection, id: &str) -> Result<Option<Recurrence>> {
    let sql = format!("SELECT {RECURRENCE_COLUMNS} FROM recurrences WHERE id = ?1");
    let row = conn
        .query_row(&sql, params![id], recurrence_from_row)
        .optional()?;
    Ok(row)
}

pub fn require_recurrence(conn: &Connection, id: &str) -> Result<Recurrence> {
    get_recurrence(conn, id)?.ok_or_else(|| AppError::not_found("recurrence"))
}

pub fn list_recurrences(conn: &Connection) -> Result<Vec<Recurrence>> {
    let sql =
        format!("SELECT {RECURRENCE_COLUMNS} FROM recurrences ORDER BY created_at ASC, id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], recurrence_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn active_session(conn: &Connection, clock: &dyn Clock) -> Result<Option<Session>> {
    let sql = format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE ended_at IS NULL LIMIT 1");
    let now = now_dt(clock);
    let row = conn
        .query_row(&sql, [], |row| session_from_row(row, now))
        .optional()?;
    Ok(row)
}

pub fn get_session(conn: &Connection, id: &str, clock: &dyn Clock) -> Result<Option<Session>> {
    let sql = format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1");
    let now = now_dt(clock);
    let row = conn
        .query_row(&sql, params![id], |row| session_from_row(row, now))
        .optional()?;
    Ok(row)
}

pub fn list_sessions_for_day(
    conn: &Connection,
    day: &str,
    clock: &dyn Clock,
) -> Result<Vec<Session>> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS} FROM sessions WHERE day = ?1 ORDER BY started_at ASC, id ASC"
    );
    let now = now_dt(clock);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![day], |row| session_from_row(row, now))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_sessions_since(
    conn: &Connection,
    from_day: &str,
    clock: &dyn Clock,
) -> Result<Vec<Session>> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS} FROM sessions WHERE day >= ?1 ORDER BY day ASC, started_at ASC"
    );
    let now = now_dt(clock);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![from_day], |row| session_from_row(row, now))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_sessions_for_task(
    conn: &Connection,
    task_id: &str,
    clock: &dyn Clock,
) -> Result<Vec<Session>> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS} FROM sessions WHERE task_id = ?1 ORDER BY started_at ASC, id ASC"
    );
    let now = now_dt(clock);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![task_id], |row| session_from_row(row, now))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn insert_event(
    tx: &Transaction<'_>,
    at: &str,
    task_id: Option<&str>,
    kind: &str,
    reason: Option<&str>,
    before_json: Option<&str>,
) -> Result<()> {
    tx.execute(
        "INSERT INTO events (at, task_id, kind, reason, before_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![at, task_id, kind, reason, before_json],
    )?;
    Ok(())
}

pub fn latest_event(conn: &Connection) -> Result<Option<Event>> {
    let row = conn
        .query_row(
            "SELECT id, at, task_id, kind, reason, before_json FROM events ORDER BY id DESC LIMIT 1",
            [],
            event_from_row,
        )
        .optional()?;
    Ok(row)
}

pub fn latest_event_for_task(conn: &Connection, task_id: &str) -> Result<Option<Event>> {
    let row = conn
        .query_row(
            "SELECT id, at, task_id, kind, reason, before_json FROM events WHERE task_id = ?1 \
             ORDER BY id DESC LIMIT 1",
            params![task_id],
            event_from_row,
        )
        .optional()?;
    Ok(row)
}

pub fn find_undoable_event(
    conn: &Connection,
    since: &str,
    kinds: &[&str],
) -> Result<Option<Event>> {
    let placeholders = std::iter::repeat_n("?", kinds.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT e.id, e.at, e.task_id, e.kind, e.reason, e.before_json FROM events e \
         WHERE e.at >= ?1 AND e.task_id IS NOT NULL AND e.kind IN ({placeholders}) \
         AND NOT EXISTS (SELECT 1 FROM events e2 WHERE e2.task_id = e.task_id AND e2.id > e.id) \
         ORDER BY e.id DESC LIMIT 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut values: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(kinds.len() + 1);
    values.push(&since);
    for kind in kinds {
        values.push(kind);
    }
    let row = stmt
        .query_row(values.as_slice(), event_from_row)
        .optional()?;
    Ok(row)
}

pub fn count_events(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?)
}

pub fn all_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY created_at ASC, id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn all_days(conn: &Connection) -> Result<Vec<Day>> {
    let mut stmt = conn.prepare(
        "SELECT day, plan_locked_at, multiplier_at_lock, budget_at_lock, wrapup_done_at, win_note \
         FROM days ORDER BY day ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Day {
            day: row.get(0)?,
            plan_locked_at: row.get(1)?,
            multiplier_at_lock: row.get(2)?,
            budget_at_lock: row.get(3)?,
            wrapup_done_at: row.get(4)?,
            win_note: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn all_events(conn: &Connection) -> Result<Vec<Event>> {
    let mut stmt = conn
        .prepare("SELECT id, at, task_id, kind, reason, before_json FROM events ORDER BY id ASC")?;
    let rows = stmt.query_map([], event_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        at: row.get(1)?,
        task_id: row.get(2)?,
        kind: row.get(3)?,
        reason: row.get(4)?,
        before_json: row.get(5)?,
    })
}

pub fn max_slot_pos(conn: &Connection, day: &str, slot: Slot) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(slot_pos), 0) FROM tasks WHERE planned_for = ?1 AND slot = ?2",
        params![day, slot.as_str()],
        |row| row.get(0),
    )?)
}
