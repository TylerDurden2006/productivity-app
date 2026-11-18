use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::clock::{add_days, format_date, parse_date, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    Area, AreaInput, Band, DaySummary, Goal, GoalBands, GoalInput, GoalStatus, GoalView,
    HorizonKind, HorizonView,
};
use crate::rules::{calibration, validate};

fn ymd(year: i32, month: u32, day: u32) -> Result<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| AppError::internal("invalid period date"))
}

fn end_of_month(year: i32, month: u32) -> Result<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    ymd(next_year, next_month, 1)?
        .pred_opt()
        .ok_or_else(|| AppError::internal("invalid period date"))
}

fn parse_period_start(value: &str) -> Result<NaiveDate> {
    let trimmed = value.trim();
    parse_date(trimmed)
        .ok_or_else(|| AppError::hard(ErrorCode::InvalidSetting, "date must use YYYY-MM-DD"))
}

pub fn period_bounds(
    kind: HorizonKind,
    period_start: Option<&str>,
    today: NaiveDate,
    week_starts_on: i64,
) -> Result<(Option<String>, Option<String>, String)> {
    if kind == HorizonKind::LongTerm {
        return Ok((None, None, "Long term".to_string()));
    }
    let anchor = match period_start {
        Some(value) if !value.trim().is_empty() => parse_period_start(value)?,
        _ => today,
    };
    let (start, end, label) = match kind {
        HorizonKind::Week => {
            let weekday = anchor.weekday().num_days_from_monday() as i64;
            let offset = (weekday - week_starts_on.rem_euclid(7)).rem_euclid(7);
            let start = add_days(anchor, -offset);
            let end = add_days(start, 6);
            let label = format!("Week of {} {}", start.day(), start.format("%b %Y"));
            (start, end, label)
        }
        HorizonKind::Month => {
            let start = ymd(anchor.year(), anchor.month(), 1)?;
            let end = end_of_month(anchor.year(), anchor.month())?;
            let label = start.format("%B %Y").to_string();
            (start, end, label)
        }
        HorizonKind::Quarter => {
            let quarter = (anchor.month() - 1) / 3;
            let start_month = quarter * 3 + 1;
            let start = ymd(anchor.year(), start_month, 1)?;
            let end = end_of_month(anchor.year(), start_month + 2)?;
            let label = format!("Q{} {}", quarter + 1, anchor.year());
            (start, end, label)
        }
        HorizonKind::Year => {
            let start = ymd(anchor.year(), 1, 1)?;
            let end = ymd(anchor.year(), 12, 31)?;
            (start, end, anchor.year().to_string())
        }
        HorizonKind::LongTerm => unreachable!(),
    };
    Ok((Some(format_date(start)), Some(format_date(end)), label))
}

fn period_key(
    kind: HorizonKind,
    period_start: Option<&str>,
    today: NaiveDate,
    week_starts_on: i64,
) -> Result<Option<String>> {
    Ok(period_bounds(kind, period_start, today, week_starts_on)?.0)
}

fn note_period(
    kind: HorizonKind,
    period_start: &str,
    today: NaiveDate,
    week_starts_on: i64,
) -> Result<String> {
    if kind == HorizonKind::LongTerm {
        return Ok(String::new());
    }
    period_key(kind, Some(period_start), today, week_starts_on)?
        .ok_or_else(|| AppError::internal("missing period start"))
}

fn goal_from_row(row: &Row<'_>) -> rusqlite::Result<Goal> {
    let kind_text: String = row.get(1)?;
    let band_text: String = row.get(5)?;
    let status_text: String = row.get(10)?;
    Ok(Goal {
        id: row.get(0)?,
        horizon_kind: HorizonKind::from_db(&kind_text),
        period_start: row.get(2)?,
        area_id: row.get(3)?,
        parent_id: row.get(4)?,
        band: Band::from_db(&band_text),
        position: row.get(6)?,
        title: row.get(7)?,
        description: row.get(8)?,
        success_definition: row.get(9)?,
        status: GoalStatus::from_db(&status_text),
        target_on: row.get(11)?,
        estimate_mins: row.get(12)?,
        completed_at: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn area_from_row(row: &Row<'_>) -> rusqlite::Result<Area> {
    let active: i64 = row.get(4)?;
    Ok(Area {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        sort_order: row.get(3)?,
        active: active != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn fetch_goal(conn: &Connection, id: &str) -> Result<Goal> {
    conn.query_row(
        "SELECT id, horizon_kind, period_start, area_id, parent_id, band, position, title, \
         description, success_definition, status, target_on, estimate_mins, completed_at, \
         created_at, updated_at FROM goals WHERE id = ?1",
        params![id],
        goal_from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("goal"))
}

fn fetch_area(conn: &Connection, id: &str) -> Result<Area> {
    conn.query_row(
        "SELECT id, name, color, sort_order, active, created_at, updated_at FROM areas WHERE id = ?1",
        params![id],
        area_from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("area"))
}

fn ensure_area_exists(conn: &Connection, area_id: Option<&str>) -> Result<()> {
    if let Some(id) = area_id {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM areas WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        if count == 0 {
            return Err(AppError::not_found("area"));
        }
    }
    Ok(())
}

fn ensure_parent_exists(conn: &Connection, parent_id: Option<&str>) -> Result<()> {
    if let Some(id) = parent_id {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM goals WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        if count == 0 {
            return Err(AppError::not_found("goal"));
        }
    }
    Ok(())
}

fn band_used(
    conn: &Connection,
    kind: HorizonKind,
    period_start: Option<&str>,
    band: Band,
    exclude_id: Option<&str>,
) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM goals WHERE horizon_kind = ?1 AND period_start IS ?2 \
         AND band = ?3 AND status = 'OPEN' AND (?4 IS NULL OR id <> ?4)",
        params![kind.as_str(), period_start, band.as_str(), exclude_id],
        |row| row.get(0),
    )?)
}

fn ensure_space(
    conn: &Connection,
    kind: HorizonKind,
    period_start: Option<&str>,
    band: Band,
    exclude_id: Option<&str>,
) -> Result<()> {
    let used = band_used(conn, kind, period_start, band, exclude_id)?;
    if used >= band.capacity() {
        return Err(AppError::hard(ErrorCode::SlotFull, "That band is full."));
    }
    Ok(())
}

fn lowest_free_position(
    conn: &Connection,
    kind: HorizonKind,
    period_start: Option<&str>,
    band: Band,
    exclude_id: Option<&str>,
) -> Result<i64> {
    let mut stmt = conn.prepare(
        "SELECT position FROM goals WHERE horizon_kind = ?1 AND period_start IS ?2 \
         AND band = ?3 AND status = 'OPEN' AND (?4 IS NULL OR id <> ?4) ORDER BY position ASC",
    )?;
    let rows = stmt.query_map(
        params![kind.as_str(), period_start, band.as_str(), exclude_id],
        |row| row.get::<_, i64>(0),
    )?;
    let mut used = HashSet::new();
    for row in rows {
        used.insert(row?);
    }
    let mut position = 1;
    while used.contains(&position) {
        position += 1;
    }
    Ok(position)
}

fn reindex_band(
    tx: &Transaction<'_>,
    kind: HorizonKind,
    period_start: Option<&str>,
    band: Band,
) -> Result<()> {
    let mut stmt = tx.prepare(
        "SELECT id FROM goals WHERE horizon_kind = ?1 AND period_start IS ?2 AND band = ?3 \
         AND status = 'OPEN' ORDER BY position ASC, created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![kind.as_str(), period_start, band.as_str()], |row| {
        row.get::<_, String>(0)
    })?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    drop(stmt);
    for (index, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE goals SET position = ?2 WHERE id = ?1",
            params![id, index as i64 + 1],
        )?;
    }
    Ok(())
}

fn goal_view(conn: &Connection, goal: Goal) -> Result<GoalView> {
    let (linked_tasks, done_tasks): (i64, i64) = conn.query_row(
        "SELECT COUNT(t.id), COALESCE(SUM(CASE WHEN t.status = 'DONE' THEN 1 ELSE 0 END), 0) \
         FROM task_goals tg JOIN tasks t ON t.id = tg.task_id WHERE tg.goal_id = ?1",
        params![goal.id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut stmt = conn.prepare(
        "SELECT t.title FROM task_goals tg JOIN tasks t ON t.id = tg.task_id \
         WHERE tg.goal_id = ?1 ORDER BY t.created_at ASC, t.id ASC LIMIT 8",
    )?;
    let rows = stmt.query_map(params![goal.id], |row| row.get::<_, String>(0))?;
    let linked_titles = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut stmt = conn.prepare(
        "SELECT t.id FROM task_goals tg JOIN tasks t ON t.id = tg.task_id \
         WHERE tg.goal_id = ?1 ORDER BY t.created_at ASC, t.id ASC",
    )?;
    let rows = stmt.query_map(params![goal.id], |row| row.get::<_, String>(0))?;
    let linked_task_ids = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(GoalView {
        goal,
        linked_tasks,
        done_tasks,
        linked_titles,
        linked_task_ids,
    })
}

fn day_summary(conn: &Connection, day: NaiveDate) -> Result<DaySummary> {
    let day_text = format_date(day);
    let budget = calibration::day_budget(conn, &day_text)?;
    let (open_count, done_count): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN status = 'READY' THEN 1 ELSE 0 END), 0), \
         COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
         FROM tasks WHERE planned_for = ?1",
        params![day_text],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let locked = db::get_day(conn, &day_text)?
        .map(|row| row.plan_locked_at.is_some())
        .unwrap_or(false);
    Ok(DaySummary {
        day: day_text,
        planned_mins: budget.planned_mins,
        open_count,
        done_count,
        locked,
    })
}

pub fn view(
    conn: &Connection,
    clock: &dyn Clock,
    kind: HorizonKind,
    period_start: Option<String>,
) -> Result<HorizonView> {
    let today = db::today(conn, clock)?;
    let week_starts_on = db::settings(conn)?.week_starts_on;
    let (period_start, period_end, label) =
        period_bounds(kind, period_start.as_deref(), today, week_starts_on)?;

    let notes = if kind == HorizonKind::LongTerm {
        conn.query_row(
            "SELECT intent, review_note FROM horizon_notes WHERE horizon_kind = 'LONG_TERM' \
             ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?
    } else {
        conn.query_row(
            "SELECT intent, review_note FROM horizon_notes \
             WHERE horizon_kind = ?1 AND period_start = ?2",
            params![kind.as_str(), period_start.as_deref().unwrap_or_default()],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?
    };
    let (intent, review_note) = notes.unwrap_or((None, None));

    let mut stmt = conn.prepare(
        "SELECT id, horizon_kind, period_start, area_id, parent_id, band, position, title, \
         description, success_definition, status, target_on, estimate_mins, completed_at, \
         created_at, updated_at \
         FROM goals WHERE horizon_kind = ?1 AND period_start IS ?2 AND status <> 'DROPPED' \
         ORDER BY CASE band WHEN 'FOCUS' THEN 1 WHEN 'SECONDARY' THEN 2 \
         WHEN 'COMMITMENT' THEN 3 ELSE 4 END, position ASC, created_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(
        params![kind.as_str(), period_start.as_deref()],
        goal_from_row,
    )?;
    let mut goals = Vec::new();
    for row in rows {
        goals.push(goal_view(conn, row?)?);
    }

    let mut focus = None;
    let mut secondary = Vec::new();
    let mut commitment = Vec::new();
    let mut more = Vec::new();
    let mut completed = Vec::new();
    for item in goals {
        if item.goal.status == GoalStatus::Done {
            completed.push(item);
            continue;
        }
        match item.goal.band {
            Band::Focus => {
                if focus.is_none() {
                    focus = Some(item);
                }
            }
            Band::Secondary => secondary.push(item),
            Band::Commitment => commitment.push(item),
            Band::More => more.push(item),
        }
    }

    let mut days = Vec::new();
    if kind == HorizonKind::Week {
        if let Some(start) = period_start.as_deref().and_then(parse_date) {
            for offset in 0..7 {
                days.push(day_summary(conn, add_days(start, offset))?);
            }
        }
    }

    Ok(HorizonView {
        kind,
        period_start,
        period_end,
        label,
        intent,
        review_note,
        bands: GoalBands {
            focus,
            secondary,
            commitment,
            more,
        },
        completed,
        days,
    })
}

fn target_period(
    conn: &Connection,
    clock: &dyn Clock,
    kind: HorizonKind,
    period_start: Option<&str>,
) -> Result<Option<String>> {
    let today = db::today(conn, clock)?;
    let week_starts_on = db::settings(conn)?.week_starts_on;
    period_key(kind, period_start, today, week_starts_on)
}

pub fn create_goal(conn: &mut Connection, clock: &dyn Clock, input: GoalInput) -> Result<Goal> {
    db::with_tx(conn, |tx| {
        let title = validate::required_title(&input.title)?;
        let estimate = input.estimate_mins.map(validate::estimate).transpose()?;
        let period = target_period(tx, clock, input.horizon_kind, input.period_start.as_deref())?;
        ensure_area_exists(tx, input.area_id.as_deref())?;
        ensure_parent_exists(tx, input.parent_id.as_deref())?;
        ensure_space(tx, input.horizon_kind, period.as_deref(), input.band, None)?;
        let position =
            lowest_free_position(tx, input.horizon_kind, period.as_deref(), input.band, None)?;
        let now = db::now(clock);
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO goals (id, horizon_kind, period_start, area_id, parent_id, band, position, \
             title, description, success_definition, status, target_on, estimate_mins, completed_at, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'OPEN', ?11, \
             ?12, NULL, ?13, ?13)",
            params![
                id,
                input.horizon_kind.as_str(),
                period,
                input.area_id,
                input.parent_id,
                input.band.as_str(),
                position,
                title,
                validate::optional_text(input.description),
                validate::optional_text(input.success_definition),
                validate::date_optional(input.target_on)?,
                estimate,
                now
            ],
        )?;
        tx.execute(
            "UPDATE goals SET first_period_start = COALESCE(first_period_start, period_start) WHERE id = ?1",
            params![id],
        )?;
        reindex_band(tx, input.horizon_kind, period.as_deref(), input.band)?;
        fetch_goal(tx, &id)
    })
}

pub fn update_goal(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: GoalInput,
) -> Result<Goal> {
    db::with_tx(conn, |tx| {
        let existing = fetch_goal(tx, id)?;
        let title = validate::required_title(&input.title)?;
        let estimate = input.estimate_mins.map(validate::estimate).transpose()?;
        let period = target_period(tx, clock, input.horizon_kind, input.period_start.as_deref())?;
        ensure_area_exists(tx, input.area_id.as_deref())?;
        ensure_parent_exists(tx, input.parent_id.as_deref())?;
        if input.parent_id.as_deref() == Some(id) {
            return Err(AppError::invalid_transition(
                "a goal cannot be its own parent",
            ));
        }

        let scope_changed =
            existing.horizon_kind != input.horizon_kind || existing.period_start != period;
        let band_changed = existing.band != input.band;
        if existing.status == GoalStatus::Open && (scope_changed || band_changed) {
            ensure_space(
                tx,
                input.horizon_kind,
                period.as_deref(),
                input.band,
                Some(id),
            )?;
        }
        let position = if scope_changed || band_changed {
            lowest_free_position(
                tx,
                input.horizon_kind,
                period.as_deref(),
                input.band,
                Some(id),
            )?
        } else {
            existing.position
        };

        let now = db::now(clock);
        tx.execute(
            "UPDATE goals SET horizon_kind = ?2, period_start = ?3, area_id = ?4, parent_id = ?5, \
             band = ?6, position = ?7, title = ?8, description = ?9, success_definition = ?10, \
             target_on = ?11, estimate_mins = ?12, updated_at = ?13 WHERE id = ?1",
            params![
                id,
                input.horizon_kind.as_str(),
                period,
                input.area_id,
                input.parent_id,
                input.band.as_str(),
                position,
                title,
                validate::optional_text(input.description),
                validate::optional_text(input.success_definition),
                validate::date_optional(input.target_on)?,
                estimate,
                now
            ],
        )?;
        if scope_changed || band_changed {
            reindex_band(
                tx,
                existing.horizon_kind,
                existing.period_start.as_deref(),
                existing.band,
            )?;
            reindex_band(tx, input.horizon_kind, period.as_deref(), input.band)?;
        }
        fetch_goal(tx, id)
    })
}

pub fn complete_goal(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<Goal> {
    db::with_tx(conn, |tx| {
        let goal = fetch_goal(tx, id)?;
        if goal.status == GoalStatus::Done {
            return Ok(goal);
        }
        let now = db::now(clock);
        tx.execute(
            "UPDATE goals SET status = 'DONE', completed_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        reindex_band(
            tx,
            goal.horizon_kind,
            goal.period_start.as_deref(),
            goal.band,
        )?;
        fetch_goal(tx, id)
    })
}

pub fn reopen_goal(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<Goal> {
    db::with_tx(conn, |tx| {
        let goal = fetch_goal(tx, id)?;
        if goal.status == GoalStatus::Open {
            return Ok(goal);
        }
        ensure_space(
            tx,
            goal.horizon_kind,
            goal.period_start.as_deref(),
            goal.band,
            Some(id),
        )?;
        let position = lowest_free_position(
            tx,
            goal.horizon_kind,
            goal.period_start.as_deref(),
            goal.band,
            Some(id),
        )?;
        let now = db::now(clock);
        tx.execute(
            "UPDATE goals SET status = 'OPEN', completed_at = NULL, position = ?2, updated_at = ?3 \
             WHERE id = ?1",
            params![id, position, now],
        )?;
        reindex_band(
            tx,
            goal.horizon_kind,
            goal.period_start.as_deref(),
            goal.band,
        )?;
        fetch_goal(tx, id)
    })
}

pub fn link_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    task_id: &str,
    goal_id: &str,
) -> Result<()> {
    db::with_tx(conn, |tx| {
        if db::get_task(tx, task_id)?.is_none() {
            return Err(AppError::not_found("task"));
        }
        fetch_goal(tx, goal_id)?;
        tx.execute(
            "INSERT OR IGNORE INTO task_goals (task_id, goal_id, created_at) VALUES (?1, ?2, ?3)",
            params![task_id, goal_id, db::now(clock)],
        )?;
        Ok(())
    })
}

pub fn unlink_task(conn: &mut Connection, task_id: &str, goal_id: &str) -> Result<()> {
    db::with_tx(conn, |tx| {
        tx.execute(
            "DELETE FROM task_goals WHERE task_id = ?1 AND goal_id = ?2",
            params![task_id, goal_id],
        )?;
        Ok(())
    })
}

pub fn set_note(
    conn: &mut Connection,
    clock: &dyn Clock,
    kind: HorizonKind,
    period_start: &str,
    intent: Option<String>,
    review_note: Option<String>,
) -> Result<()> {
    db::with_tx(conn, |tx| {
        let today = db::today(tx, clock)?;
        let week_starts_on = db::settings(tx)?.week_starts_on;
        let period = note_period(kind, period_start, today, week_starts_on)?;
        let intent = validate::optional_text(intent);
        let review_note = validate::optional_text(review_note);
        let now = db::now(clock);
        let reviewed_at = review_note.as_ref().map(|_| now.clone());
        tx.execute(
            "INSERT INTO horizon_notes (horizon_kind, period_start, intent, review_note, \
             reviewed_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(horizon_kind, period_start) DO UPDATE SET \
             intent = excluded.intent, review_note = excluded.review_note, \
             reviewed_at = COALESCE(excluded.reviewed_at, horizon_notes.reviewed_at), \
             updated_at = excluded.updated_at",
            params![kind.as_str(), period, intent, review_note, reviewed_at, now],
        )?;
        Ok(())
    })
}

pub fn list_areas(conn: &Connection) -> Result<Vec<Area>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, sort_order, active, created_at, updated_at FROM areas \
         ORDER BY sort_order ASC, name ASC, id ASC",
    )?;
    let rows = stmt.query_map([], area_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn create_area(conn: &mut Connection, clock: &dyn Clock, input: AreaInput) -> Result<Area> {
    db::with_tx(conn, |tx| {
        let name = validate::required_title(&input.name)?;
        let color = validate::optional_text(input.color).unwrap_or_else(|| "forest".to_string());
        let sort_order = match input.sort_order {
            Some(value) => value,
            None => tx.query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM areas",
                [],
                |row| row.get(0),
            )?,
        };
        let now = db::now(clock);
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO areas (id, name, color, sort_order, active, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
            params![id, name, color, sort_order, now],
        )?;
        fetch_area(tx, &id)
    })
}
