use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Transaction};

use crate::clock::{format_date, parse_rfc3339, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{Session, SessionKind, SessionView, Slot, Status, Task};
use crate::rules::{calibration, carry, recurrence};

fn at_string(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn view(session: &Session) -> SessionView {
    SessionView {
        id: session.id.clone(),
        kind: session.kind,
        planned_mins: session.planned_mins,
        elapsed_secs: session.elapsed_secs,
        running: session.running,
        extended: session.extended,
        running_since: session.running_since.clone(),
    }
}

fn insert_session(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    kind: SessionKind,
    task_id: Option<&str>,
    day: &str,
    planned_mins: i64,
    running: bool,
) -> Result<Session> {
    let now = db::now(clock);
    let id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO sessions (id, kind, task_id, day, planned_mins, extended, accumulated_secs, \
         running_since, last_seen_at, started_at, ended_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, ?6, ?6, NULL)",
        params![
            id,
            kind.as_str(),
            task_id,
            day,
            planned_mins,
            if running { Some(now) } else { None }
        ],
    )?;
    db::get_session(tx, &id, clock)?.ok_or_else(|| AppError::internal("session insert failed"))
}

fn ensure_no_active(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<()> {
    if db::active_session(tx, clock)?.is_some() {
        return Err(AppError::hard(
            ErrorCode::SessionAlreadyActive,
            "a session is already active",
        ));
    }
    Ok(())
}

pub fn start(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    kind: SessionKind,
    task_id: Option<&str>,
) -> Result<Session> {
    let today = db::today(tx, clock)?;
    let day = format_date(today);
    db::ensure_day(tx, &day)?;
    ensure_no_active(tx, clock)?;
    match kind {
        SessionKind::Focus => {
            let id = task_id.ok_or_else(|| AppError::missing("focus needs a task"))?;
            let task = db::require_task(tx, id)?;
            if task.status != Status::Ready || task.planned_for.as_deref() != Some(day.as_str()) {
                return Err(AppError::hard(
                    ErrorCode::NotPlannedForToday,
                    "focus works only on today's planned tasks",
                ));
            }
            let settings = db::settings(tx)?;
            let session = insert_session(
                tx,
                clock,
                SessionKind::Focus,
                Some(id),
                &day,
                settings.focus_block_mins,
                true,
            )?;
            db::insert_event(
                tx,
                &db::now(clock),
                Some(id),
                "STARTED",
                Some("FOCUS"),
                None,
            )?;
            Ok(session)
        }
        SessionKind::Batch => {
            if task_id.is_some() {
                return Err(AppError::invalid_transition(
                    "batch sessions do not take a task",
                ));
            }
            let batch_count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'READY' AND planned_for = ?1 \
                 AND slot = 'BATCH'",
                params![day],
                |row| row.get(0),
            )?;
            if batch_count == 0 {
                return Err(AppError::invalid_transition(
                    "plan at least one batch task first",
                ));
            }
            let batch_non_micro: i64 = tx.query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'READY' AND planned_for = ?1 \
                 AND slot = 'BATCH' AND estimate_mins <> 15",
                params![day],
                |row| row.get(0),
            )?;
            if batch_non_micro > 0 {
                return Err(AppError::invalid_transition(
                    "quick batch is only for 15-minute tasks; focus the other tasks individually",
                ));
            }
            let settings = db::settings(tx)?;
            let session = insert_session(
                tx,
                clock,
                SessionKind::Batch,
                None,
                &day,
                settings.batch_block_mins,
                true,
            )?;
            db::insert_event(tx, &db::now(clock), None, "STARTED", Some("BATCH"), None)?;
            Ok(session)
        }
        SessionKind::Manual => Err(AppError::invalid_transition(
            "manual sessions are recorded on completion",
        )),
    }
}

pub fn pause(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<Session> {
    let session = db::active_session(tx, clock)?
        .ok_or_else(|| AppError::invalid_transition("no active session"))?;
    if !session.running {
        return Err(AppError::invalid_transition("session is already paused"));
    }
    freeze_session(tx, &session, clock.now_utc())?;
    db::get_session(tx, &session.id, clock)?
        .ok_or_else(|| AppError::internal("session disappeared"))
}

pub fn resume(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<Session> {
    let session = db::active_session(tx, clock)?
        .ok_or_else(|| AppError::invalid_transition("no session to resume"))?;
    if session.running {
        return Err(AppError::invalid_transition("session is already running"));
    }
    let now = db::now(clock);
    tx.execute(
        "UPDATE sessions SET running_since = ?2, last_seen_at = ?2 WHERE id = ?1",
        params![session.id, now],
    )?;
    db::get_session(tx, &session.id, clock)?
        .ok_or_else(|| AppError::internal("session disappeared"))
}

pub fn heartbeat(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<()> {
    if let Some(session) = db::active_session(tx, clock)? {
        if session.running {
            tx.execute(
                "UPDATE sessions SET last_seen_at = ?2 WHERE id = ?1",
                params![session.id, db::now(clock)],
            )?;
        }
    }
    Ok(())
}

pub fn extend(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<Session> {
    let session = db::active_session(tx, clock)?
        .ok_or_else(|| AppError::invalid_transition("no active session"))?;
    if session.kind != SessionKind::Focus {
        return Err(AppError::invalid_transition(
            "only focus sessions can be extended",
        ));
    }
    if session.extended {
        return Err(AppError::hard(
            ErrorCode::ExtensionUsed,
            "this session has already been extended",
        ));
    }
    tx.execute(
        "UPDATE sessions SET planned_mins = planned_mins + 15, extended = 1 WHERE id = ?1",
        params![session.id],
    )?;
    db::get_session(tx, &session.id, clock)?
        .ok_or_else(|| AppError::internal("session disappeared"))
}

pub fn close_session(tx: &Transaction<'_>, session: &Session, at: DateTime<Utc>) -> Result<()> {
    let mut accumulated = session.accumulated_secs;
    if let Some(since) = session.running_since.as_deref().and_then(parse_rfc3339) {
        accumulated += (at - since).num_seconds().max(0);
    }
    let stamp = at_string(at);
    tx.execute(
        "UPDATE sessions SET accumulated_secs = ?2, running_since = NULL, last_seen_at = ?3, \
         ended_at = ?3 WHERE id = ?1",
        params![session.id, accumulated, stamp],
    )?;
    Ok(())
}

pub fn freeze_session(tx: &Transaction<'_>, session: &Session, at: DateTime<Utc>) -> Result<()> {
    let mut accumulated = session.accumulated_secs;
    if let Some(since) = session.running_since.as_deref().and_then(parse_rfc3339) {
        accumulated += (at - since).num_seconds().max(0);
    }
    tx.execute(
        "UPDATE sessions SET accumulated_secs = ?2, running_since = NULL, last_seen_at = ?3 \
         WHERE id = ?1",
        params![session.id, accumulated, at_string(at)],
    )?;
    Ok(())
}

pub fn stop(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    breadcrumb: Option<String>,
) -> Result<Session> {
    let session = db::active_session(tx, clock)?
        .ok_or_else(|| AppError::invalid_transition("no active session"))?;
    close_session(tx, &session, clock.now_utc())?;
    if let Some(task_id) = &session.task_id {
        if let Some(text) = crate::rules::validate::optional_text(breadcrumb) {
            let mut task = db::require_task(tx, task_id)?;
            task.breadcrumb = Some(text);
            task.updated_at = db::now(clock);
            db::write_task_row(tx, &task)?;
        }
    }
    db::insert_event(
        tx,
        &db::now(clock),
        session.task_id.as_deref(),
        "STOPPED",
        Some("STOP_FOR_NOW"),
        None,
    )?;
    db::get_session(tx, &session.id, clock)?
        .ok_or_else(|| AppError::internal("session disappeared"))
}

fn remaining_batch(tx: &Transaction<'_>, day: &str) -> Result<i64> {
    Ok(tx.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'READY' AND planned_for = ?1 AND slot = 'BATCH'",
        params![day],
        |row| row.get(0),
    )?)
}

pub fn complete(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    task_id: &str,
    manual_mins: Option<i64>,
) -> Result<Task> {
    let mut task = db::require_task(tx, task_id)?;
    if task.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "only ready tasks can be completed",
        ));
    }
    let today = db::today(tx, clock)?;
    let day = format_date(today);
    db::ensure_day(tx, &day)?;
    let is_recurring = task.recurrence_id.is_some();
    if !is_recurring
        && task.planned_for.is_some()
        && task.planned_for.as_deref() != Some(day.as_str())
    {
        return Err(AppError::invalid_transition(
            "only today's planned tasks can be completed",
        ));
    }
    let active = db::active_session(tx, clock)?;
    if let Some(session) = &active {
        let same_task = session.task_id.as_deref() == Some(task_id);
        match session.kind {
            SessionKind::Focus | SessionKind::Manual => {
                if !same_task {
                    return Err(AppError::invalid_transition(
                        "a session is active for a different task",
                    ));
                }
            }
            SessionKind::Batch => {
                if task.slot != Some(Slot::Batch) {
                    return Err(AppError::invalid_transition("a batch session is active"));
                }
            }
        }
    }

    if let Some(minutes) = manual_mins {
        let already_timed: i64 = tx.query_row(
            "SELECT COUNT(*) FROM sessions WHERE task_id = ?1 AND day = ?2",
            params![task_id, day],
            |row| row.get(0),
        )?;
        if active.is_none() && !is_recurring && minutes > 0 && already_timed == 0 {
            let now = db::now(clock);
            let session_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO sessions (id, kind, task_id, day, planned_mins, extended, \
                 accumulated_secs, running_since, last_seen_at, started_at, ended_at) \
                 VALUES (?1, 'MANUAL', ?2, ?3, ?4, 0, ?5, NULL, ?6, ?6, ?6)",
                params![session_id, task_id, day, minutes, minutes * 60, now],
            )?;
        }
    }

    let before = serde_json::to_string(&task)?;
    let now = db::now(clock);
    task.status = Status::Done;
    task.completed_at = Some(now.clone());
    task.updated_at = now.clone();
    db::write_task_row(tx, &task)?;
    db::insert_event(tx, &now, Some(task_id), "DONE", None, Some(&before))?;

    if let Some(session) = &active {
        let close_current = session.task_id.as_deref() == Some(task_id);
        let close_batch = session.kind == SessionKind::Batch
            && task.slot == Some(Slot::Batch)
            && remaining_batch(tx, &day)? == 0;
        if close_current || close_batch {
            close_session(tx, session, clock.now_utc())?;
        }
    }

    if let Some(recurrence_id) = &task.recurrence_id {
        let series = db::require_recurrence(tx, recurrence_id)?;
        recurrence::ensure_next_after(tx, clock, &series, today)?;
    }
    Ok(task)
}

pub fn drop_task(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    task_id: &str,
    reason: Option<String>,
) -> Result<Task> {
    let mut task = db::require_task(tx, task_id)?;
    if task.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "only ready tasks can be dropped",
        ));
    }
    let today = db::today(tx, clock)?;
    let before = serde_json::to_string(&task)?;
    let now = db::now(clock);
    if let Some(session) = db::active_session(tx, clock)? {
        if session.task_id.as_deref() == Some(task_id) {
            close_session(tx, &session, clock.now_utc())?;
        }
    }
    task.status = Status::Dropped;
    task.closed_reason = Some(crate::model::ClosedReason::NotDoing);
    task.planned_for = None;
    task.slot = None;
    task.slot_pos = None;
    task.updated_at = now.clone();
    db::write_task_row(tx, &task)?;
    db::insert_event(
        tx,
        &now,
        Some(task_id),
        "DROPPED",
        reason.as_deref(),
        Some(&before),
    )?;
    if let Some(recurrence_id) = &task.recurrence_id {
        let series = db::require_recurrence(tx, recurrence_id)?;
        recurrence::ensure_next_after(tx, clock, &series, today)?;
    }
    Ok(task)
}

pub fn carry_task(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    task: &mut Task,
    at: &str,
) -> Result<()> {
    let before = serde_json::to_string(task)?;
    let day = task.planned_for.clone();
    let mut increment = true;
    if let Some(estimate) = task.estimate_mins {
        if estimate > 15 {
            let logged =
                calibration::today_sessions_for_task(tx, &task.id, day.as_deref().unwrap_or(""))?;
            increment = (logged as f64) < 0.25 * (estimate as f64) * 60.0;
        }
    }
    task.planned_for = None;
    task.slot = None;
    task.slot_pos = None;
    if increment {
        task.rollover_count += 1;
    }
    task.updated_at = at.to_string();
    db::write_task_row(tx, task)?;
    db::insert_event(
        tx,
        at,
        Some(&task.id),
        "ROLLED_OVER",
        day.as_deref(),
        Some(&before),
    )?;
    let _ = clock;
    Ok(())
}

pub fn carry_overdue(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    today: chrono::NaiveDate,
) -> Result<Vec<String>> {
    let today_text = format_date(today);
    let mut stmt = tx.prepare(
        "SELECT id FROM tasks WHERE status = 'READY' AND planned_for IS NOT NULL \
         AND planned_for < ?1 ORDER BY planned_for ASC, id ASC",
    )?;
    let ids = stmt
        .query_map(params![today_text], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let now = db::now(clock);
    let mut carried = Vec::new();
    for id in ids {
        let Some(mut task) = db::get_task(tx, &id)? else {
            continue;
        };
        let day = task.planned_for.clone().unwrap_or_default();
        let day_row = db::day_or_default(tx, &day)?;
        if day_row.wrapup_done_at.is_some() {
            continue;
        }
        carry_task(tx, clock, &mut task, &now)?;
        carried.push(id);
    }
    Ok(carried)
}

pub fn reconcile(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<crate::model::ReconcileReport> {
    let today = db::today(tx, clock)?;
    let today_text = format_date(today);
    let mut frozen = Vec::new();
    let mut stale_session = None;

    if let Some(session) = db::active_session(tx, clock)? {
        let stale = session
            .last_seen_at
            .as_deref()
            .and_then(parse_rfc3339)
            .map(|last| clock.now_utc() - last > chrono::Duration::minutes(2))
            .unwrap_or(true);
        if session.running && (session.day != today_text || stale) {
            let bank_at = session
                .last_seen_at
                .as_deref()
                .and_then(parse_rfc3339)
                .unwrap_or_else(|| clock.now_utc());
            freeze_session(tx, &session, bank_at)?;
            frozen.push(session.id.clone());
            stale_session = db::get_session(tx, &session.id, clock)?;
        }
    }

    let settings = db::settings(tx)?;
    let carried = carry::daily(tx, clock)?;
    carry::horizons(tx, clock, &settings, today)?;
    recurrence::materialize_all(tx, clock, today)?;
    Ok(crate::model::ReconcileReport {
        carried,
        frozen,
        stale_session,
    })
}
