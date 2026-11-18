use rusqlite::{params, Connection};

use crate::db;
use crate::error::Result;
use crate::model::{BudgetView, Slot, Status, Task};

pub const MIN_RATIO: f64 = 0.5;
pub const MAX_RATIO: f64 = 3.0;
pub const MIN_MULTIPLIER: f64 = 0.8;
pub const MAX_MULTIPLIER: f64 = 2.5;
pub const MIN_SAMPLES: usize = 5;
pub const SAMPLE_LIMIT: i64 = 20;

pub struct SampleSet {
    pub ratios: Vec<f64>,
    pub no_time: i64,
}

pub fn round_to_five(value: f64) -> i64 {
    ((value / 5.0).round() * 5.0) as i64
}

pub fn calibrated_estimate(estimate: i64, multiplier: f64) -> i64 {
    let value = estimate as f64 * multiplier;
    round_to_five(value).max(5)
}

pub fn samples(conn: &Connection) -> Result<SampleSet> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.estimate_mins, COALESCE(SUM(s.accumulated_secs), 0) AS secs, \
         COALESCE(t.completed_at, t.updated_at) AS stamp \
         FROM tasks t JOIN sessions s ON s.task_id = t.id \
         AND s.kind IN ('FOCUS', 'MANUAL') AND s.ended_at IS NOT NULL \
         WHERE (t.status = 'DONE' OR (t.status = 'DROPPED' AND t.closed_reason = 'SPLIT')) \
         AND t.estimate_mins IS NOT NULL AND t.estimate_mins > 15 \
         GROUP BY t.id HAVING secs > 0 ORDER BY stamp DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![SAMPLE_LIMIT], |row| {
        Ok((row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
    })?;
    let mut ratios = Vec::new();
    for row in rows {
        let (estimate, secs) = row?;
        if estimate <= 0 || secs <= 0 {
            continue;
        }
        let ratio = (secs as f64 / 60.0 / estimate as f64).clamp(MIN_RATIO, MAX_RATIO);
        ratios.push(ratio);
    }
    let no_time: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks t WHERE t.status = 'DONE' \
         AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.task_id = t.id \
         AND s.kind IN ('FOCUS', 'MANUAL') AND s.accumulated_secs > 0)",
        [],
        |row| row.get(0),
    )?;
    Ok(SampleSet { ratios, no_time })
}

pub fn effective_multiplier(conn: &Connection) -> Result<f64> {
    let sample_set = samples(conn)?;
    if sample_set.ratios.len() < MIN_SAMPLES {
        return Ok(db::settings(conn)?.default_multiplier);
    }
    let mut ratios = sample_set.ratios;
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = ratios.len() / 2;
    let median = if ratios.len() % 2 == 0 {
        (ratios[mid - 1] + ratios[mid]) / 2.0
    } else {
        ratios[mid]
    };
    let clamped = median.clamp(MIN_MULTIPLIER, MAX_MULTIPLIER);
    Ok((clamped * 100.0).round() / 100.0)
}

pub fn multiplier_for_day(conn: &Connection, day: &str) -> Result<f64> {
    if let Some(row) = db::get_day(conn, day)? {
        if let Some(locked) = row.multiplier_at_lock {
            return Ok(locked);
        }
    }
    effective_multiplier(conn)
}

pub fn plan_minutes(tasks: &[Task], multiplier: f64) -> i64 {
    let mut total = 0;
    for task in tasks {
        if task.status != Status::Ready {
            continue;
        }
        if matches!(
            task.slot,
            Some(Slot::One) | Some(Slot::Three) | Some(Slot::Batch)
        ) {
            if let Some(estimate) = task.estimate_mins {
                total += calibrated_estimate(estimate, multiplier);
            }
        }
    }
    total
}

pub fn budget_mins(conn: &Connection, day: &str) -> Result<i64> {
    if let Some(row) = db::get_day(conn, day)? {
        if row.plan_locked_at.is_some() {
            if let Some(locked) = row.budget_at_lock {
                return Ok(locked);
            }
        }
    }
    Ok(db::settings(conn)?.daily_focus_budget_mins)
}

pub fn day_budget(conn: &Connection, day: &str) -> Result<BudgetView> {
    let multiplier = multiplier_for_day(conn, day)?;
    let tasks = db::list_planned(conn, day)?;
    Ok(BudgetView {
        planned_mins: plan_minutes(&tasks, multiplier),
        budget_mins: budget_mins(conn, day)?,
        multiplier,
    })
}

pub fn today_sessions_for_task(conn: &Connection, task_id: &str, day: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(accumulated_secs), 0) FROM sessions WHERE task_id = ?1 AND day = ?2 \
         AND kind IN ('FOCUS', 'MANUAL')",
        params![task_id, day],
        |row| row.get(0),
    )?)
}

pub fn task_extended_today(conn: &Connection, task_id: &str, day: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE task_id = ?1 AND day = ?2 AND extended = 1",
        params![task_id, day],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}
