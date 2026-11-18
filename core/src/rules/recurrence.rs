use chrono::NaiveDate;
use rusqlite::{params, Transaction};

use crate::clock::{add_days, format_date, parse_date, weekday_bit, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{Freq, Recurrence, RecurrenceInput, RecurrencePatch, RecurrenceView, Task};
use crate::rules::validate;

pub fn next_on_or_after(today: NaiveDate, recurrence: &Recurrence) -> NaiveDate {
    let starts = parse_date(&recurrence.starts_on).unwrap_or(today);
    if today <= starts {
        return starts;
    }
    match recurrence.freq {
        Freq::Daily => today,
        Freq::Weekly => {
            for offset in 0..=6 {
                let candidate = add_days(today, offset);
                if weekday_bit(candidate) & (recurrence.weekdays_mask as u8) != 0 {
                    return candidate;
                }
            }
            today
        }
        Freq::EveryNDays => {
            let interval = recurrence.interval_n.max(1);
            let diff = (today - starts).num_days();
            let remainder = diff % interval;
            if remainder == 0 {
                today
            } else {
                add_days(today, interval - remainder)
            }
        }
    }
}

pub fn next_after(date: NaiveDate, recurrence: &Recurrence) -> NaiveDate {
    match recurrence.freq {
        Freq::Daily => add_days(date, 1),
        Freq::Weekly => {
            for offset in 1..=7 {
                let candidate = add_days(date, offset);
                if weekday_bit(candidate) & (recurrence.weekdays_mask as u8) != 0 {
                    return candidate;
                }
            }
            add_days(date, 7)
        }
        Freq::EveryNDays => add_days(date, recurrence.interval_n.max(1)),
    }
}

pub fn ensure_occurrence(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    recurrence: &Recurrence,
    today: NaiveDate,
) -> Result<Option<Task>> {
    if !recurrence.active {
        return Ok(None);
    }
    if db::find_occurrence(tx, &recurrence.id)?.is_some() {
        return Ok(None);
    }
    let latest = db::latest_occurrence_on(tx, &recurrence.id)?
        .as_deref()
        .and_then(parse_date);
    let due = match latest {
        Some(date) if date >= today => next_after(date, recurrence),
        _ => next_on_or_after(today, recurrence),
    };
    create_occurrence(tx, clock, recurrence, today, due)
}

pub fn ensure_next_after(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    recurrence: &Recurrence,
    today: NaiveDate,
) -> Result<Option<Task>> {
    if !recurrence.active {
        return Ok(None);
    }
    if db::find_occurrence(tx, &recurrence.id)?.is_some() {
        return Ok(None);
    }
    let due = next_after(today, recurrence);
    create_occurrence(tx, clock, recurrence, today, due)
}

fn create_occurrence(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    recurrence: &Recurrence,
    today: NaiveDate,
    due: NaiveDate,
) -> Result<Option<Task>> {
    if due > add_days(today, 1) {
        return Ok(None);
    }
    let now = db::now(clock);
    let id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO tasks (id, title, first_step, done_when, estimate_mins, status, value, pinned, \
         deadline_on, waiting_on, review_on, breadcrumb, cue, parent_id, closed_reason, rollover_count, \
         planned_for, slot, slot_pos, recurrence_id, occurrence_on, created_at, updated_at, completed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'READY', ?6, 0, NULL, NULL, NULL, NULL, ?7, NULL, NULL, 0, \
         NULL, NULL, NULL, ?8, ?9, ?10, ?10, NULL)",
        params![
            id,
            recurrence.title,
            recurrence.first_step,
            recurrence.done_when,
            recurrence.estimate_mins,
            recurrence.value,
            recurrence.cue,
            recurrence.id,
            format_date(due),
            now
        ],
    )?;
    db::get_task(tx, &id)
}

pub fn materialize_all(tx: &Transaction<'_>, clock: &dyn Clock, today: NaiveDate) -> Result<()> {
    for recurrence in db::list_recurrences(tx)? {
        if recurrence.active {
            ensure_occurrence(tx, clock, &recurrence, today)?;
        }
    }
    Ok(())
}

pub fn create(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    input: RecurrenceInput,
) -> Result<Recurrence> {
    let title = validate::required_title(&input.title)?;
    let first_step = validate::required_first_step(&input.first_step)?;
    let done_when = validate::optional_text(input.done_when);
    let estimate = validate::estimate(input.estimate_mins)?;
    let value = validate::value_scale(input.value.unwrap_or(3))?;
    let cue = validate::optional_text(input.cue);
    let starts_on = validate::date_required(&input.starts_on)?;
    let interval_n = match input.freq {
        Freq::EveryNDays => input.interval_n.unwrap_or(1),
        _ => input.interval_n.unwrap_or(1),
    };
    if !(1..=365).contains(&interval_n) {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "interval must be between 1 and 365 days",
        ));
    }
    let weekdays_mask = match input.freq {
        Freq::Weekly => validate::weekday_mask(input.weekdays_mask.unwrap_or(127))?,
        _ => 127,
    };

    let now = db::now(clock);
    let id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO recurrences (id, title, first_step, done_when, estimate_mins, value, cue, freq, \
         interval_n, weekdays_mask, starts_on, active, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?12)",
        params![
            id,
            title,
            first_step,
            done_when,
            estimate,
            value,
            cue,
            input.freq.as_str(),
            interval_n,
            weekdays_mask,
            starts_on,
            now
        ],
    )?;

    let recurrence = db::require_recurrence(tx, &id)?;
    let today = db::today(tx, clock)?;
    ensure_occurrence(tx, clock, &recurrence, today)?;
    Ok(recurrence)
}

pub fn update(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    patch: RecurrencePatch,
) -> Result<Recurrence> {
    let current = db::require_recurrence(tx, id)?;
    let title = match patch.title {
        Some(value) => validate::required_title(&value)?,
        None => current.title.clone(),
    };
    let first_step = match patch.first_step {
        Some(value) => validate::required_first_step(&value)?,
        None => current.first_step.clone(),
    };
    let done_when = match patch.done_when {
        Some(value) => validate::optional_text(value),
        None => current.done_when.clone(),
    };
    let estimate = match patch.estimate_mins {
        Some(value) => validate::estimate(value)?,
        None => current.estimate_mins,
    };
    let value_scale = match patch.value {
        Some(value) => validate::value_scale(value)?,
        None => current.value,
    };
    let cue = match patch.cue {
        Some(value) => validate::optional_text(value),
        None => current.cue.clone(),
    };
    let freq = patch.freq.unwrap_or(current.freq);
    let interval_n = patch.interval_n.unwrap_or(current.interval_n);
    if !(1..=365).contains(&interval_n) {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "interval must be between 1 and 365 days",
        ));
    }
    let weekdays_mask = match freq {
        Freq::Weekly => {
            validate::weekday_mask(patch.weekdays_mask.unwrap_or(current.weekdays_mask))?
        }
        _ => current.weekdays_mask,
    };
    let starts_on = match patch.starts_on {
        Some(value) => validate::date_required(&value)?,
        None => current.starts_on.clone(),
    };
    let active = patch.active.unwrap_or(current.active);
    let now = db::now(clock);
    tx.execute(
        "UPDATE recurrences SET title = ?2, first_step = ?3, done_when = ?4, estimate_mins = ?5, \
         value = ?6, cue = ?7, freq = ?8, interval_n = ?9, weekdays_mask = ?10, starts_on = ?11, \
         active = ?12, updated_at = ?13 WHERE id = ?1",
        params![
            id,
            title,
            first_step,
            done_when,
            estimate,
            value_scale,
            cue,
            freq.as_str(),
            interval_n,
            weekdays_mask,
            starts_on,
            if active { 1 } else { 0 },
            now
        ],
    )?;
    let updated = db::require_recurrence(tx, id)?;
    let today = db::today(tx, clock)?;
    if !active {
        tx.execute(
            "DELETE FROM tasks WHERE recurrence_id = ?1 AND status = 'READY' AND occurrence_on > ?2",
            params![id, format_date(today)],
        )?;
    } else if !current.active {
        ensure_occurrence(tx, clock, &updated, today)?;
    }
    Ok(updated)
}

pub fn delete_series(tx: &Transaction<'_>, id: &str) -> Result<()> {
    db::require_recurrence(tx, id)?;
    tx.execute(
        "UPDATE recurrences SET active = 0 WHERE id = ?1",
        params![id],
    )?;
    tx.execute(
        "DELETE FROM tasks WHERE recurrence_id = ?1 AND status = 'READY'",
        params![id],
    )?;
    Ok(())
}

pub fn list_views(conn: &rusqlite::Connection, clock: &dyn Clock) -> Result<Vec<RecurrenceView>> {
    let today = db::today(conn, clock)?;
    let mut views = Vec::new();
    for recurrence in db::list_recurrences(conn)? {
        let open = db::find_occurrence(conn, &recurrence.id)?;
        let next_due_on = match &open {
            Some(task) => task.occurrence_on.clone(),
            None if recurrence.active => Some(format_date(next_on_or_after(today, &recurrence))),
            None => None,
        };
        views.push(RecurrenceView {
            has_open_occurrence: open.is_some(),
            recurrence,
            next_due_on,
        });
    }
    Ok(views)
}
