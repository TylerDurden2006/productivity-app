use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::clock::{add_days, format_date, parse_date, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    Cadence, CadenceKind, Habit, HabitEntry, HabitEntryStatus, HabitInput, HabitStats, HabitToday,
    TimeOfDay,
};
use crate::rules::{cadence, validate};

const HABIT_COLUMNS: &str =
    "id, title, why, cue, area_id, schedule_json, anchor_on, target_count, \
unit_label, time_of_day, sort_order, active, starts_on, ends_on, created_at, updated_at";
const ENTRY_COLUMNS: &str =
    "id, habit_id, on_date, count, status, note, task_id, created_at, updated_at";
const MAX_ITERATIONS: usize = 400;
const BEST_WINDOW_DAYS: i64 = 365;
const RATE_WINDOW_DAYS: i64 = 30;

pub fn list(conn: &Connection) -> Result<Vec<Habit>> {
    let sql = format!("SELECT {HABIT_COLUMNS} FROM habits");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], habit_from_row)?;
    let mut habits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    sort_habits(&mut habits);
    Ok(habits)
}

pub fn get(conn: &Connection, id: &str) -> Result<Habit> {
    let sql = format!("SELECT {HABIT_COLUMNS} FROM habits WHERE id = ?1");
    conn.query_row(&sql, params![id], habit_from_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("habit"))
}

pub fn create(conn: &mut Connection, clock: &dyn Clock, input: HabitInput) -> Result<Habit> {
    let input = validate_input(input)?;
    let now = db::now(clock);
    db::with_tx(conn, |tx| insert_habit(tx, &now, input))
}

pub fn update(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: HabitInput,
) -> Result<Habit> {
    let input = validate_input(input)?;
    let now = db::now(clock);
    db::with_tx(conn, |tx| update_habit(tx, &now, id, input))
}

pub fn archive(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<()> {
    let now = db::now(clock);
    db::with_tx(conn, |tx| {
        require_habit(tx, id)?;
        tx.execute(
            "UPDATE habits SET active = 0, updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    })
}

pub fn today(
    conn: &Connection,
    clock: &dyn Clock,
    date: Option<String>,
) -> Result<Vec<HabitToday>> {
    let day = match date {
        Some(value) => parse_required_date(&value)?,
        None => db::today(conn, clock)?,
    };
    let day_text = format_date(day);
    let mut items = Vec::new();
    for habit in active_habits(conn)? {
        if !within_bounds(&habit, day) || !due_for_date(conn, &habit, day)? {
            continue;
        }
        let entry = entry_on(conn, &habit.id, &day_text)?;
        let (quota_done, quota_target) = if habit.schedule.kind == CadenceKind::Quota {
            let period_start = cadence::period_start(&habit.schedule, day);
            let from = period_start.max(start_bound(&habit));
            (
                count_quota_done(conn, &habit.id, from, day)?,
                quota_target(&habit),
            )
        } else {
            (0, 0)
        };
        let streak = streak(conn, &habit, day)?;
        items.push(HabitToday {
            cadence_label: cadence::label(&habit.schedule),
            habit,
            entry,
            quota_done,
            quota_target,
            streak,
        });
    }
    items.sort_by(|a, b| compare_habits(&a.habit, &b.habit));
    Ok(items)
}

pub fn complete(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    date: &str,
    count: Option<i64>,
    note: Option<String>,
) -> Result<HabitEntry> {
    let on_date = parse_required_date(date)?;
    let date_text = format_date(on_date);
    let now = db::now(clock);
    db::with_tx(conn, |tx| {
        let habit = require_habit(tx, id)?;
        let target = habit.target_count.max(1);
        let requested = count.unwrap_or(target);
        if requested < 1 {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "habit count must be at least one",
            ));
        }
        let value = requested.min(target);
        let note = validate::optional_text(note);
        tx.execute(
            "INSERT INTO habit_entries (id, habit_id, on_date, count, status, note, task_id, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'DONE', ?5, NULL, ?6, ?6) \
             ON CONFLICT(habit_id, on_date) DO UPDATE SET count = excluded.count, \
             status = 'DONE', note = excluded.note, updated_at = excluded.updated_at",
            params![
                uuid::Uuid::new_v4().to_string(),
                id,
                date_text,
                value,
                note,
                now
            ],
        )?;
        require_entry(tx, id, &date_text)
    })
}

pub fn skip(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    date: &str,
    note: Option<String>,
) -> Result<HabitEntry> {
    let on_date = parse_required_date(date)?;
    let date_text = format_date(on_date);
    let now = db::now(clock);
    db::with_tx(conn, |tx| {
        require_habit(tx, id)?;
        let note = validate::optional_text(note);
        tx.execute(
            "INSERT INTO habit_entries (id, habit_id, on_date, count, status, note, task_id, \
             created_at, updated_at) VALUES (?1, ?2, ?3, 0, 'SKIPPED', ?4, NULL, ?5, ?5) \
             ON CONFLICT(habit_id, on_date) DO UPDATE SET count = 0, \
             status = 'SKIPPED', note = excluded.note, updated_at = excluded.updated_at",
            params![uuid::Uuid::new_v4().to_string(), id, date_text, note, now],
        )?;
        require_entry(tx, id, &date_text)
    })
}

pub fn reopen(conn: &mut Connection, id: &str, date: &str) -> Result<()> {
    let on_date = parse_required_date(date)?;
    let date_text = format_date(on_date);
    db::with_tx(conn, |tx| {
        require_habit(tx, id)?;
        tx.execute(
            "DELETE FROM habit_entries WHERE habit_id = ?1 AND on_date = ?2",
            params![id, date_text],
        )?;
        Ok(())
    })
}

pub fn history(conn: &Connection, id: &str, from: &str, to: &str) -> Result<Vec<HabitEntry>> {
    require_habit(conn, id)?;
    let from_date = parse_required_date(from)?;
    let to_date = parse_required_date(to)?;
    if from_date > to_date {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "start date must be on or before end date",
        ));
    }
    entries_between(conn, id, from_date, to_date)
}

pub fn stats(
    conn: &Connection,
    clock: &dyn Clock,
    id: &str,
    date: Option<String>,
) -> Result<HabitStats> {
    let habit = require_habit(conn, id)?;
    let reference = match date {
        Some(value) => parse_required_date(&value)?,
        None => db::today(conn, clock)?,
    };
    let current_streak = streak(conn, &habit, reference)?;
    let (best_streak, _, _) = window_counts(conn, &habit, reference, BEST_WINDOW_DAYS)?;
    let (_, done_count, scheduled_count) =
        window_counts(conn, &habit, reference, RATE_WINDOW_DAYS)?;
    let completion_rate = if scheduled_count == 0 {
        0.0
    } else {
        done_count as f64 / scheduled_count as f64
    };
    Ok(HabitStats {
        current_streak,
        best_streak,
        completion_rate,
        done_count,
        scheduled_count,
    })
}

pub fn due_on(conn: &Connection, habit: &Habit, date: NaiveDate) -> Result<bool> {
    if !habit.active || !within_bounds(habit, date) {
        return Ok(false);
    }
    due_for_date(conn, habit, date)
}

fn due_for_date(conn: &Connection, habit: &Habit, date: NaiveDate) -> Result<bool> {
    if habit.schedule.kind != CadenceKind::Quota {
        return Ok(cadence::is_due_on(&habit.schedule, date));
    }
    let date_text = format_date(date);
    if entry_on(conn, &habit.id, &date_text)?.is_some() {
        return Ok(true);
    }
    let period_start = cadence::period_start(&habit.schedule, date);
    let from = period_start.max(start_bound(habit));
    let done = count_quota_done(conn, &habit.id, from, date)?;
    Ok(done < quota_target(habit))
}

fn insert_habit(tx: &Transaction<'_>, now: &str, input: HabitInput) -> Result<Habit> {
    let id = uuid::Uuid::new_v4().to_string();
    let schedule_json = serde_json::to_string(&input.schedule)?;
    tx.execute(
        "INSERT INTO habits (id, title, why, cue, area_id, schedule_json, anchor_on, target_count, \
         unit_label, time_of_day, sort_order, active, starts_on, ends_on, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?13, ?14, ?14)",
        params![
            id,
            input.title,
            input.why,
            input.cue,
            input.area_id,
            schedule_json,
            input.schedule.anchor_on,
            input.target_count,
            input.unit_label,
            input.time_of_day.as_str(),
            input.sort_order,
            input.starts_on,
            input.ends_on,
            now
        ],
    )?;
    get(tx, &id)
}

fn update_habit(tx: &Transaction<'_>, now: &str, id: &str, input: HabitInput) -> Result<Habit> {
    require_habit(tx, id)?;
    let schedule_json = serde_json::to_string(&input.schedule)?;
    tx.execute(
        "UPDATE habits SET title = ?2, why = ?3, cue = ?4, area_id = ?5, schedule_json = ?6, \
         anchor_on = ?7, target_count = ?8, unit_label = ?9, time_of_day = ?10, \
         sort_order = ?11, starts_on = ?12, ends_on = ?13, updated_at = ?14 WHERE id = ?1",
        params![
            id,
            input.title,
            input.why,
            input.cue,
            input.area_id,
            schedule_json,
            input.schedule.anchor_on,
            input.target_count,
            input.unit_label,
            input.time_of_day.as_str(),
            input.sort_order,
            input.starts_on,
            input.ends_on,
            now
        ],
    )?;
    get(tx, id)
}

fn validate_input(mut input: HabitInput) -> Result<HabitInput> {
    input.title = validate::required_title(&input.title)?;
    input.why = validate::optional_text(input.why);
    input.cue = validate::optional_text(input.cue);
    input.unit_label = validate::optional_text(input.unit_label);
    if input.target_count < 1 {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "target count must be at least one",
        ));
    }
    cadence::validate(&input.schedule)?;
    input.starts_on = validate::date_optional(input.starts_on)?;
    input.ends_on = validate::date_optional(input.ends_on)?;
    if let (Some(starts_on), Some(ends_on)) = (&input.starts_on, &input.ends_on) {
        let starts = parse_required_date(starts_on)?;
        let ends = parse_required_date(ends_on)?;
        if starts > ends {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "start date must be on or before end date",
            ));
        }
    }
    Ok(input)
}

fn active_habits(conn: &Connection) -> Result<Vec<Habit>> {
    let sql = format!("SELECT {HABIT_COLUMNS} FROM habits WHERE active = 1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], habit_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn require_habit(conn: &Connection, id: &str) -> Result<Habit> {
    get(conn, id)
}

fn habit_from_row(row: &Row<'_>) -> rusqlite::Result<Habit> {
    let schedule_json: String = row.get(5)?;
    let anchor_on: String = row.get(6)?;
    let mut schedule: Cadence = serde_json::from_str(&schedule_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(error))
    })?;
    if !anchor_on.is_empty() {
        schedule.anchor_on = anchor_on;
    }
    let time_text: String = row.get(9)?;
    let active_int: i64 = row.get(11)?;
    Ok(Habit {
        id: row.get(0)?,
        title: row.get(1)?,
        why: row.get(2)?,
        cue: row.get(3)?,
        area_id: row.get(4)?,
        schedule,
        target_count: row.get(7)?,
        unit_label: row.get(8)?,
        time_of_day: TimeOfDay::from_db(&time_text),
        sort_order: row.get(10)?,
        active: active_int != 0,
        starts_on: row.get(12)?,
        ends_on: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<HabitEntry> {
    let status_text: String = row.get(4)?;
    Ok(HabitEntry {
        id: row.get(0)?,
        habit_id: row.get(1)?,
        on_date: row.get(2)?,
        count: row.get(3)?,
        status: HabitEntryStatus::from_db(&status_text),
        note: row.get(5)?,
        task_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn entry_on(conn: &Connection, id: &str, date: &str) -> Result<Option<HabitEntry>> {
    let sql =
        format!("SELECT {ENTRY_COLUMNS} FROM habit_entries WHERE habit_id = ?1 AND on_date = ?2");
    Ok(conn
        .query_row(&sql, params![id, date], entry_from_row)
        .optional()?)
}

fn require_entry(conn: &Connection, id: &str, date: &str) -> Result<HabitEntry> {
    entry_on(conn, id, date)?.ok_or_else(|| AppError::internal("habit entry missing after write"))
}

fn entries_between(
    conn: &Connection,
    id: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<HabitEntry>> {
    let sql = format!(
        "SELECT {ENTRY_COLUMNS} FROM habit_entries WHERE habit_id = ?1 \
         AND on_date >= ?2 AND on_date <= ?3 ORDER BY on_date ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![id, format_date(from), format_date(to)],
        entry_from_row,
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn status_map(
    conn: &Connection,
    habit: &Habit,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<HashMap<NaiveDate, HabitEntryStatus>> {
    let mut map = HashMap::new();
    for entry in entries_between(conn, &habit.id, from, to)? {
        if let Some(date) = parse_date(&entry.on_date) {
            map.insert(date, entry.status);
        }
    }
    Ok(map)
}

fn count_quota_done(conn: &Connection, id: &str, from: NaiveDate, to: NaiveDate) -> Result<i64> {
    if from > to {
        return Ok(0);
    }
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM habit_entries WHERE habit_id = ?1 AND status = 'DONE' \
         AND on_date >= ?2 AND on_date <= ?3",
        params![id, format_date(from), format_date(to)],
        |row| row.get(0),
    )?)
}

fn quota_target(habit: &Habit) -> i64 {
    habit.schedule.quota_count.unwrap_or(1).max(1)
}

fn start_bound(habit: &Habit) -> NaiveDate {
    let anchor = cadence::anchor(&habit.schedule);
    match habit.starts_on.as_deref().and_then(parse_date) {
        Some(starts_on) => anchor.max(starts_on),
        None => anchor,
    }
}

fn end_bound(habit: &Habit) -> Option<NaiveDate> {
    habit.ends_on.as_deref().and_then(parse_date)
}

fn within_bounds(habit: &Habit, date: NaiveDate) -> bool {
    if date < start_bound(habit) {
        return false;
    }
    match end_bound(habit) {
        Some(ends_on) => date <= ends_on,
        None => true,
    }
}

fn streak(conn: &Connection, habit: &Habit, date: NaiveDate) -> Result<i64> {
    match habit.schedule.kind {
        CadenceKind::Quota => quota_streak(conn, habit, date),
        _ => fixed_streak(conn, habit, date),
    }
}

fn fixed_streak(conn: &Connection, habit: &Habit, date: NaiveDate) -> Result<i64> {
    let lower = start_bound(habit);
    let upper = match end_bound(habit) {
        Some(ends_on) => date.min(ends_on),
        None => date,
    };
    if upper < lower {
        return Ok(0);
    }
    let lookback = add_days(upper, -(MAX_ITERATIONS as i64));
    let statuses = status_map(conn, habit, lookback.max(lower), upper)?;
    let mut streak = 0;
    let mut cursor = upper;
    for _ in 0..MAX_ITERATIONS {
        if cursor < lower {
            break;
        }
        if cadence::is_due_on(&habit.schedule, cursor) {
            match statuses.get(&cursor) {
                Some(HabitEntryStatus::Done) => streak += 1,
                _ => break,
            }
        }
        cursor = add_days(cursor, -1);
    }
    Ok(streak)
}

fn quota_streak(conn: &Connection, habit: &Habit, date: NaiveDate) -> Result<i64> {
    let lower = start_bound(habit);
    let upper = match end_bound(habit) {
        Some(ends_on) => date.min(ends_on),
        None => date,
    };
    if upper < lower {
        return Ok(0);
    }
    let target = quota_target(habit);
    let mut period_start = cadence::period_start(&habit.schedule, upper);
    let mut streak = 0;
    for _ in 0..MAX_ITERATIONS {
        let period_end = cadence::period_end(&habit.schedule, period_start);
        if period_end < lower {
            break;
        }
        let from = period_start.max(lower);
        let to = period_end.min(upper);
        if count_quota_done(conn, &habit.id, from, to)? < target {
            break;
        }
        streak += 1;
        period_start = cadence::period_start(&habit.schedule, add_days(period_start, -1));
    }
    Ok(streak)
}

fn window_counts(
    conn: &Connection,
    habit: &Habit,
    reference: NaiveDate,
    days: i64,
) -> Result<(i64, i64, i64)> {
    match habit.schedule.kind {
        CadenceKind::Quota => quota_window(conn, habit, reference, days),
        _ => fixed_window(conn, habit, reference, days),
    }
}

fn fixed_window(
    conn: &Connection,
    habit: &Habit,
    reference: NaiveDate,
    days: i64,
) -> Result<(i64, i64, i64)> {
    let window_start = add_days(reference, -(days - 1));
    let lower = start_bound(habit);
    let upper = match end_bound(habit) {
        Some(ends_on) => reference.min(ends_on),
        None => reference,
    };
    let from = window_start.max(lower);
    if from > upper {
        return Ok((0, 0, 0));
    }
    let statuses = status_map(conn, habit, from, upper)?;
    let mut best = 0;
    let mut run = 0;
    let mut done = 0;
    let mut scheduled = 0;
    let mut cursor = from;
    for _ in 0..MAX_ITERATIONS {
        if cursor > upper {
            break;
        }
        if cadence::is_due_on(&habit.schedule, cursor) {
            scheduled += 1;
            if matches!(statuses.get(&cursor), Some(HabitEntryStatus::Done)) {
                done += 1;
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        cursor = add_days(cursor, 1);
    }
    Ok((best, done, scheduled))
}

fn quota_window(
    conn: &Connection,
    habit: &Habit,
    reference: NaiveDate,
    days: i64,
) -> Result<(i64, i64, i64)> {
    let window_start = add_days(reference, -(days - 1));
    let lower = start_bound(habit);
    let upper = match end_bound(habit) {
        Some(ends_on) => reference.min(ends_on),
        None => reference,
    };
    let from = window_start.max(lower);
    if from > upper {
        return Ok((0, 0, 0));
    }
    let target = quota_target(habit);
    let mut period_start = cadence::period_start(&habit.schedule, from);
    let mut best = 0;
    let mut run = 0;
    let mut done = 0;
    let mut scheduled = 0;
    for _ in 0..MAX_ITERATIONS {
        if period_start > upper {
            break;
        }
        let period_end = cadence::period_end(&habit.schedule, period_start);
        let from_date = period_start.max(from);
        let to_date = period_end.min(upper);
        if from_date <= to_date {
            scheduled += 1;
            if count_quota_done(conn, &habit.id, from_date, to_date)? >= target {
                done += 1;
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        period_start = cadence::period_start(&habit.schedule, add_days(period_end, 1));
    }
    Ok((best, done, scheduled))
}

fn parse_required_date(value: &str) -> Result<NaiveDate> {
    parse_date(value)
        .ok_or_else(|| AppError::hard(ErrorCode::InvalidDay, "date must use YYYY-MM-DD"))
}

fn time_rank(value: TimeOfDay) -> i64 {
    match value {
        TimeOfDay::Morning => 0,
        TimeOfDay::Afternoon => 1,
        TimeOfDay::Evening => 2,
        TimeOfDay::Anytime => 3,
    }
}

fn compare_habits(a: &Habit, b: &Habit) -> Ordering {
    time_rank(a.time_of_day)
        .cmp(&time_rank(b.time_of_day))
        .then_with(|| a.sort_order.cmp(&b.sort_order))
        .then_with(|| a.title.cmp(&b.title))
}

fn sort_habits(habits: &mut [Habit]) {
    habits.sort_by(compare_habits);
}
