use chrono::NaiveDate;
use rusqlite::Transaction;

use crate::clock::{format_date, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{Day, OutcomeKind, ResolveInput, UnfinishedView, WrapupView};
use crate::rules::{clarify, plan, ranking, session, validate};

pub fn view(conn: &rusqlite::Connection, clock: &dyn Clock) -> Result<WrapupView> {
    let today = db::today(conn, clock)?;
    let day = format_date(today);
    let planned = db::list_planned(conn, &day)?;
    let sessions = db::list_sessions_for_day(conn, &day, clock)?;
    let mut unfinished = Vec::new();
    for task in planned
        .into_iter()
        .filter(|task| task.recurrence_id.is_none())
    {
        let secs: i64 = sessions
            .iter()
            .filter(|session| {
                session.task_id.as_deref() == Some(task.id.as_str())
                    && session.kind != crate::model::SessionKind::Batch
            })
            .map(|session| session.elapsed_secs)
            .sum();
        let logged_today_mins = (secs + 59) / 60;
        unfinished.push(UnfinishedView {
            needs_breadcrumb: logged_today_mins > 0 && task.breadcrumb.is_none(),
            logged_today_mins,
            task,
        });
    }
    let mut stuck = db::list_stuck(conn)?;
    ranking::sort_deep(&mut stuck, today);
    let mut inbox = db::list_inbox(conn)?;
    for task in &mut inbox {
        task.updated_at = task.updated_at.clone();
    }
    let row = db::day_or_default(conn, &day)?;
    let tomorrow = format_date(crate::clock::add_days(today, 1));
    let tomorrow_view = plan::view(conn, clock, Some(tomorrow))?;
    Ok(WrapupView {
        unfinished,
        stuck,
        inbox,
        win_note: row.win_note,
        tomorrow: tomorrow_view,
    })
}

fn allowed(task: &crate::model::Task, today: &str) -> bool {
    if task.planned_for.as_deref() == Some(today) {
        return true;
    }
    task.planned_for.is_none() && task.rollover_count >= 3
}

pub fn resolve(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    task_id: &str,
    input: ResolveInput,
) -> Result<crate::model::Task> {
    let mut task = db::require_task(tx, task_id)?;
    if task.status != crate::model::Status::Ready {
        return Err(AppError::invalid_transition(
            "only ready tasks can be resolved",
        ));
    }
    if task.recurrence_id.is_some() {
        return Err(AppError::invalid_transition(
            "recurring occurrences are not resolved during wrap-up",
        ));
    }
    let today = db::today(tx, clock)?;
    let today_text = format_date(today);
    if !allowed(&task, &today_text) {
        return Err(AppError::hard(
            ErrorCode::NotPlannedForToday,
            "task is not unresolved on today's plan",
        ));
    }
    if let Some(breadcrumb) = validate::optional_text(input.breadcrumb) {
        task.breadcrumb = Some(breadcrumb);
    }
    let now = db::now(clock);
    match input.outcome {
        OutcomeKind::Carry => {
            if task.planned_for.is_none() {
                return Err(AppError::invalid_transition(
                    "a stuck task cannot be carried",
                ));
            }
            session::carry_task(tx, clock, &mut task, &now)?;
        }
        OutcomeKind::Waiting => {
            let waiting_on = validate::optional_text(input.waiting_on)
                .ok_or_else(|| AppError::missing("waiting tasks need who or what they wait on"))?;
            let review_on = validate::date_optional(input.review_on)?
                .ok_or_else(|| AppError::missing("waiting tasks need a review date"))?;
            let before = serde_json::to_string(&task)?;
            task.status = crate::model::Status::Waiting;
            task.waiting_on = Some(waiting_on);
            task.review_on = Some(review_on);
            task.planned_for = None;
            task.slot = None;
            task.slot_pos = None;
            task.updated_at = now.clone();
            db::write_task_row(tx, &task)?;
            db::insert_event(
                tx,
                &now,
                Some(task_id),
                "WAITING",
                task.waiting_on.as_deref(),
                Some(&before),
            )?;
        }
        OutcomeKind::Someday => {
            let review_on = validate::date_optional(input.review_on)?
                .ok_or_else(|| AppError::missing("someday tasks need a review date"))?;
            let before = serde_json::to_string(&task)?;
            task.status = crate::model::Status::Someday;
            task.review_on = Some(review_on);
            task.planned_for = None;
            task.slot = None;
            task.slot_pos = None;
            task.updated_at = now.clone();
            db::write_task_row(tx, &task)?;
            db::insert_event(tx, &now, Some(task_id), "SOMEDAY", None, Some(&before))?;
        }
        OutcomeKind::Drop => {
            let before = serde_json::to_string(&task)?;
            task.status = crate::model::Status::Dropped;
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
                task.breadcrumb.as_deref(),
                Some(&before),
            )?;
        }
        OutcomeKind::Split => {
            let children = input.children.unwrap_or_default();
            if !task.breadcrumb.as_deref().unwrap_or("").is_empty() {
                db::write_task_row(tx, &task)?;
            }
            let _created = clarify::split(tx, clock, task_id, children)?;
        }
        OutcomeKind::Shrink => {
            let first_step = input
                .first_step
                .as_deref()
                .ok_or_else(|| AppError::missing("shrink needs a new first step"))?;
            let estimate = input
                .estimate_mins
                .ok_or_else(|| AppError::missing("shrink needs a new estimate"))?;
            let estimate = validate::estimate(estimate)?;
            let old = task
                .estimate_mins
                .ok_or_else(|| AppError::missing("task has no estimate to shrink"))?;
            if estimate > old {
                return Err(AppError::hard(
                    ErrorCode::InvalidEstimate,
                    "a shrunk estimate cannot be larger than the original",
                ));
            }
            let before = serde_json::to_string(&task)?;
            task.first_step = Some(validate::required_first_step(first_step)?);
            task.estimate_mins = Some(estimate);
            task.planned_for = None;
            task.slot = None;
            task.slot_pos = None;
            task.rollover_count = 0;
            task.updated_at = now.clone();
            db::write_task_row(tx, &task)?;
            db::insert_event(
                tx,
                &now,
                Some(task_id),
                "REACTIVATED",
                Some("SHRINK"),
                Some(&before),
            )?;
        }
    }
    db::require_task(tx, task_id)
}

pub fn carry_all(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<Vec<crate::model::Task>> {
    let today = db::today(tx, clock)?;
    let day = format_date(today);
    let planned = db::list_planned(tx, &day)?;
    let mut carried = Vec::new();
    for task in planned
        .into_iter()
        .filter(|task| task.recurrence_id.is_none())
    {
        carried.push(resolve(
            tx,
            clock,
            &task.id,
            ResolveInput {
                outcome: OutcomeKind::Carry,
                waiting_on: None,
                review_on: None,
                breadcrumb: None,
                children: None,
                first_step: None,
                estimate_mins: None,
            },
        )?);
    }
    Ok(carried)
}

pub fn complete(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    day: &str,
    win_note: Option<String>,
) -> Result<Day> {
    let today = db::today(tx, clock)?;
    let today_text = format_date(today);
    if day != today_text {
        return Err(AppError::hard(
            ErrorCode::InvalidDay,
            "wrap-up applies to today",
        ));
    }
    let existing = db::day_or_default(tx, day)?;
    if existing.wrapup_done_at.is_some() {
        return Ok(existing);
    }
    let remaining: i64 = tx.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'READY' AND planned_for = ?1 \
         AND recurrence_id IS NULL",
        rusqlite::params![day],
        |row| row.get(0),
    )?;
    if remaining > 0 {
        return Err(AppError::hard(
            ErrorCode::MissingOutcome,
            format!("{remaining} planned tasks still need an outcome"),
        ));
    }
    let now = db::now(clock);
    db::ensure_day(tx, day)?;
    let note = validate::optional_text(win_note.clone());
    if win_note.is_some() {
        tx.execute(
            "UPDATE days SET win_note = ?2 WHERE day = ?1",
            rusqlite::params![day, note],
        )?;
    }
    tx.execute(
        "UPDATE days SET wrapup_done_at = ?2 WHERE day = ?1",
        rusqlite::params![day, now],
    )?;
    db::insert_event(tx, &now, None, "WRAPUP", Some(day), None)?;
    db::day_or_default(tx, day)
}

pub fn reopen(tx: &Transaction<'_>, clock: &dyn Clock, day: Option<&str>) -> Result<Day> {
    let today = db::today(tx, clock)?;
    let today_text = format_date(today);
    if let Some(value) = day {
        if value != today_text {
            return Err(AppError::hard(
                ErrorCode::InvalidDay,
                "only today's wrap-up can be reopened",
            ));
        }
    }
    db::ensure_day(tx, &today_text)?;
    tx.execute(
        "UPDATE days SET wrapup_done_at = NULL WHERE day = ?1",
        rusqlite::params![today_text],
    )?;
    db::day_or_default(tx, &today_text)
}

pub fn set_win_note(tx: &Transaction<'_>, clock: &dyn Clock, note: Option<String>) -> Result<Day> {
    let today = db::today(tx, clock)?;
    let day = format_date(today);
    let cleaned = validate::optional_text(note);
    db::ensure_day(tx, &day)?;
    tx.execute(
        "UPDATE days SET win_note = ?2 WHERE day = ?1",
        rusqlite::params![day, cleaned],
    )?;
    db::day_or_default(tx, &day)
}

pub fn tomorrow_day(conn: &rusqlite::Connection, clock: &dyn Clock) -> Result<String> {
    let today = db::today(conn, clock)?;
    Ok(format_date(crate::clock::add_days(today, 1)))
}

pub fn today_date(conn: &rusqlite::Connection, clock: &dyn Clock) -> Result<NaiveDate> {
    db::today(conn, clock)
}
