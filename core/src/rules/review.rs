use chrono::{Duration, SecondsFormat};
use rusqlite::{params, Transaction};

use crate::clock::Clock;
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{DayMins, RolloverStat, Stats, Task, WinNote};
use crate::rules::{calibration, plan};

pub const UNDO_WINDOW_MINUTES: i64 = 10;
pub const UNDOABLE_KINDS: [&str; 6] = [
    "DONE",
    "WAITING",
    "SOMEDAY",
    "DROPPED",
    "PLANNED",
    "UNPLANNED",
];

fn nothing_to_undo() -> AppError {
    AppError::hard(ErrorCode::NothingToUndo, "nothing to undo")
}

pub fn undo_last(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<Task> {
    if let Some(latest) = db::latest_event(tx)? {
        if matches!(
            latest.kind.as_str(),
            "SPLIT" | "SWAPPED" | "LOCKED" | "WRAPUP"
        ) {
            return Err(nothing_to_undo());
        }
    }
    let cutoff = (clock.now_utc() - Duration::minutes(UNDO_WINDOW_MINUTES))
        .to_rfc3339_opts(SecondsFormat::Secs, true);
    let event =
        db::find_undoable_event(tx, &cutoff, &UNDOABLE_KINDS)?.ok_or_else(nothing_to_undo)?;
    let task_id = event.task_id.clone().ok_or_else(nothing_to_undo)?;
    let Some(before_json) = event.before_json.as_deref() else {
        return Err(nothing_to_undo());
    };
    let mut snapshot: Task = serde_json::from_str(before_json).map_err(|_| nothing_to_undo())?;
    if snapshot.recurrence_id.is_some() {
        return Err(AppError::hard(
            ErrorCode::NothingToUndo,
            "recurring task changes cannot be undone",
        ));
    }
    if db::get_task(tx, &task_id)?.is_none() {
        return Err(nothing_to_undo());
    }
    if let (Some(day), Some(slot), Some(pos)) = (
        snapshot.planned_for.clone(),
        snapshot.slot,
        snapshot.slot_pos,
    ) {
        let conflicts: i64 = tx.query_row(
            "SELECT COUNT(*) FROM tasks WHERE id <> ?1 AND planned_for = ?2 AND slot = ?3 \
             AND slot_pos = ?4 AND status = 'READY'",
            params![snapshot.id, day, slot.as_str(), pos],
            |row| row.get(0),
        )?;
        if conflicts > 0 {
            return Err(nothing_to_undo());
        }
    }
    snapshot.updated_at = db::now(clock);
    db::write_task_row(tx, &snapshot)?;
    tx.execute("DELETE FROM events WHERE id = ?1", params![event.id])?;
    Ok(snapshot)
}

pub fn stats(conn: &rusqlite::Connection, clock: &dyn Clock) -> Result<Stats> {
    let sample_set = calibration::samples(conn)?;
    let multiplier = calibration::effective_multiplier(conn)?;
    let focus_mins_last_14: Vec<DayMins> = plan::week_minutes(conn, clock)?;
    let sql = format!(
        "SELECT {} FROM tasks WHERE rollover_count > 0 ORDER BY rollover_count DESC, updated_at DESC LIMIT 5",
        db::TASK_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], db::task_from_row)?;
    let mut most_rolled_over = Vec::new();
    for row in rows {
        let task = row?;
        most_rolled_over.push(RolloverStat {
            id: task.id,
            title: task.title,
            rollover_count: task.rollover_count,
        });
    }
    let mut stmt = conn.prepare(
        "SELECT day, win_note FROM days WHERE win_note IS NOT NULL ORDER BY day DESC LIMIT 5",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(WinNote {
            day: row.get(0)?,
            note: row.get(1)?,
        })
    })?;
    let recent_win_notes = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Stats {
        multiplier,
        sample_count: sample_set.ratios.len() as i64,
        no_time_data_count: sample_set.no_time,
        focus_mins_last_14,
        most_rolled_over,
        recent_win_notes,
    })
}

pub fn mark_weekly_review(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<()> {
    tx.execute(
        "UPDATE settings SET last_weekly_review_at = ?1 WHERE id = 1",
        params![db::now(clock)],
    )?;
    Ok(())
}
