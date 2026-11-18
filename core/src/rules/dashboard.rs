use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection};

use crate::clock::{add_days, format_date, parse_date, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    Band, DashboardCarried, DashboardDay, DashboardHabit, DashboardSection, DashboardView,
    HorizonKind, Slot,
};
use crate::rules::{habit, horizon};

const SERIES_CAP: i64 = 60;

fn range_kind(range: &str) -> Option<HorizonKind> {
    match range {
        "WEEK" => Some(HorizonKind::Week),
        "MONTH" => Some(HorizonKind::Month),
        "QUARTER" => Some(HorizonKind::Quarter),
        "YEAR" => Some(HorizonKind::Year),
        _ => None,
    }
}

fn bounds(range: &str, today: NaiveDate, week_starts_on: i64) -> Result<(NaiveDate, NaiveDate)> {
    if range == "ALL" {
        return Ok((NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(), today));
    }
    if range == "DAY" {
        return Ok((today, today));
    }
    let kind = range_kind(range)
        .ok_or_else(|| AppError::hard(ErrorCode::InvalidSetting, "unsupported dashboard range"))?;
    let (start, end, _) = horizon::period_bounds(kind, None, today, week_starts_on)?;
    let start = start
        .as_deref()
        .and_then(parse_date)
        .ok_or_else(|| AppError::internal("dashboard period start missing"))?;
    let end = end
        .as_deref()
        .and_then(parse_date)
        .ok_or_else(|| AppError::internal("dashboard period end missing"))?;
    Ok((start, end))
}

fn day_list(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let capped_start = if (end - start).num_days() + 1 > SERIES_CAP {
        add_days(end, -(SERIES_CAP - 1))
    } else {
        start
    };
    let mut days = Vec::new();
    let mut cursor = capped_start;
    while cursor <= end {
        days.push(cursor);
        cursor = add_days(cursor, 1);
    }
    days
}

fn slot_label(slot: Slot) -> &'static str {
    match slot {
        Slot::One => "Main",
        Slot::Three => "Next",
        Slot::Batch => "Small",
        Slot::Extra => "More",
    }
}

pub fn view(
    conn: &Connection,
    clock: &dyn Clock,
    range: &str,
    date: Option<String>,
) -> Result<DashboardView> {
    let range = range.trim().to_uppercase();
    if !["DAY", "WEEK", "MONTH", "QUARTER", "YEAR", "ALL"].contains(&range.as_str()) {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "unsupported dashboard range",
        ));
    }
    let settings = db::settings(conn)?;
    let today = match date.as_deref().and_then(parse_date) {
        Some(value) => value,
        None => db::today(conn, clock)?,
    };
    let (start, end) = bounds(&range, today, settings.week_starts_on)?;
    let start_text = format_date(start);
    let end_text = format_date(end);
    let days = day_list(start, end);

    let (daily_planned, daily_done): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
         FROM tasks WHERE planned_for BETWEEN ?1 AND ?2",
        params![start_text, end_text],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let kind = range_kind(&range);
    let period_start = match kind {
        Some(value) => horizon::period_bounds(value, None, today, settings.week_starts_on)?
            .0
            .unwrap_or_default(),
        None => String::new(),
    };
    let (horizon_planned, horizon_done): (i64, i64) = if let Some(value) = kind {
        conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
             FROM goals WHERE horizon_kind = ?1 AND period_start = ?2 AND status <> 'DROPPED'",
            params![value.as_str(), period_start],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
    } else if range == "ALL" {
        conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
             FROM goals WHERE status <> 'DROPPED'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
    } else {
        (0, 0)
    };

    let planned = daily_planned + horizon_planned;
    let done = daily_done + horizon_done;
    let left = planned - done;
    let completion_rate = if planned > 0 {
        (done as f64 / planned as f64) * 100.0
    } else {
        0.0
    };

    let mut day_counts: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT planned_for, COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
             FROM tasks WHERE planned_for BETWEEN ?1 AND ?2 GROUP BY planned_for",
        )?;
        let rows = stmt.query_map(params![start_text, end_text], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, i64>(1)?, row.get::<_, i64>(2)?),
            ))
        })?;
        for row in rows {
            let (day, counts) = row?;
            day_counts.insert(day, counts);
        }
    }

    let mut focus_by_day: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT day, COALESCE(SUM(accumulated_secs), 0) FROM sessions \
             WHERE ended_at IS NOT NULL AND day BETWEEN ?1 AND ?2 GROUP BY day",
        )?;
        let rows = stmt.query_map(params![start_text, end_text], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (day, secs) = row?;
            focus_by_day.insert(day, (secs + 59) / 60);
        }
    }

    let days_series = days
        .iter()
        .map(|day| {
            let key = format_date(*day);
            let (planned, done) = day_counts.get(&key).copied().unwrap_or((0, 0));
            DashboardDay {
                day: key.clone(),
                planned,
                done,
                left: planned - done,
                focus_mins: focus_by_day.get(&key).copied().unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();
    let focus_mins = days_series.iter().map(|day| day.focus_mins).sum::<i64>();

    let mut sections = Vec::new();
    for (slot, band, label) in [
        (Slot::One, Band::Focus, "Main"),
        (Slot::Three, Band::Secondary, "Next"),
        (Slot::Batch, Band::Commitment, "Small"),
        (Slot::Extra, Band::More, "More"),
    ] {
        let (task_planned, task_done): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
             FROM tasks WHERE slot = ?1 AND planned_for BETWEEN ?2 AND ?3",
            params![slot.as_str(), start_text, end_text],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let (goal_planned, goal_done): (i64, i64) = match kind {
            Some(value) => conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
                 FROM goals WHERE band = ?1 AND horizon_kind = ?2 AND period_start = ?3 \
                 AND status <> 'DROPPED'",
                params![band.as_str(), value.as_str(), period_start],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?,
            None if range == "ALL" => conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status = 'DONE' THEN 1 ELSE 0 END), 0) \
                 FROM goals WHERE band = ?1 AND status <> 'DROPPED'",
                params![band.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?,
            None => (0, 0),
        };
        let planned = task_planned + goal_planned;
        let done = task_done + goal_done;
        sections.push(DashboardSection {
            label: label.into(),
            planned,
            done,
            left: planned - done,
        });
    }
    let _ = slot_label;

    let mut habits: Vec<DashboardHabit> = Vec::new();
    let mut habit_due = 0;
    let mut habit_done = 0;
    for day in &days_series {
        let due = habit::today(conn, clock, Some(day.day.clone()))?;
        for entry in due {
            habit_due += 1;
            let title = entry.habit.title.clone();
            let index = habits.iter().position(|row| row.title == title);
            let row = if let Some(index) = index {
                &mut habits[index]
            } else {
                habits.push(DashboardHabit {
                    title,
                    due: 0,
                    done: 0,
                    rate: 0.0,
                });
                habits.last_mut().expect("just pushed")
            };
            row.due += 1;
            if entry
                .entry
                .as_ref()
                .is_some_and(|value| value.status == crate::model::HabitEntryStatus::Done)
            {
                row.done += 1;
                habit_done += 1;
            }
        }
    }
    for row in &mut habits {
        row.rate = if row.due > 0 {
            (row.done as f64 / row.due as f64) * 100.0
        } else {
            0.0
        };
    }
    habits.sort_by(|a, b| b.due.cmp(&a.due).then_with(|| a.title.cmp(&b.title)));

    let priority_start = if range == "ALL" { today } else { start };
    let priority_start_text = format_date(priority_start);
    let (overdue_tasks, carried_tasks): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN status = 'READY' AND first_planned_for < ?1 THEN 1 ELSE 0 END), 0), \
         COALESCE(SUM(CASE WHEN status = 'READY' AND rollover_count > 0 THEN 1 ELSE 0 END), 0) \
         FROM tasks",
        params![priority_start_text],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (overdue_goals, carried_goals): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN status = 'OPEN' AND first_period_start < ?1 THEN 1 ELSE 0 END), 0), \
         COALESCE(SUM(CASE WHEN status = 'OPEN' AND carry_count > 0 THEN 1 ELSE 0 END), 0) \
         FROM goals",
        params![priority_start_text],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let overdue = overdue_tasks + overdue_goals;
    let carried = carried_tasks + carried_goals;

    let mut carried_items: Vec<DashboardCarried> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT title, rollover_count FROM tasks WHERE status = 'READY' AND rollover_count > 0 \
             ORDER BY rollover_count DESC, updated_at DESC LIMIT 5",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DashboardCarried {
                title: row.get(0)?,
                kind: "Task".into(),
                count: row.get(1)?,
            })
        })?;
        carried_items.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
    }
    {
        let mut stmt = conn.prepare(
            "SELECT title, carry_count FROM goals WHERE status = 'OPEN' AND carry_count > 0 \
             ORDER BY carry_count DESC, updated_at DESC LIMIT 5",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DashboardCarried {
                title: row.get(0)?,
                kind: "Horizon task".into(),
                count: row.get(1)?,
            })
        })?;
        carried_items.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
    }
    carried_items.sort_by_key(|item| std::cmp::Reverse(item.count));
    carried_items.truncate(5);

    let (try_scheduled, try_tried): (i64, i64) = conn.query_row(
        "SELECT \
         COALESCE(SUM(CASE WHEN scheduled_for BETWEEN ?1 AND ?2 THEN 1 ELSE 0 END), 0), \
         COALESCE(SUM(CASE WHEN tried_at BETWEEN ?1 AND ?2 THEN 1 ELSE 0 END), 0) \
         FROM try_items",
        params![start_text, end_text],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    Ok(DashboardView {
        range,
        start_date: start_text,
        end_date: end_text,
        planned,
        done,
        left,
        daily_planned,
        daily_done,
        horizon_planned,
        horizon_done,
        carried,
        overdue,
        habit_due,
        habit_done,
        focus_mins,
        completion_rate,
        days: days_series,
        sections,
        habits,
        carried_items,
        try_scheduled,
        try_tried,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api, db, FixedClock};
    use chrono::{TimeZone, Utc};

    #[test]
    fn empty_dashboard_is_zeroed() {
        let conn = db::open_memory().unwrap();
        let clock = FixedClock::utc(
            Utc.with_ymd_and_hms(2026, 10, 23, 9, 0, 0)
                .single()
                .unwrap(),
        );
        let view = view(&conn, &clock, "WEEK", None).unwrap();
        assert_eq!(view.planned, 0);
        assert_eq!(view.days.len(), 7);
    }

    #[test]
    fn dashboard_counts_done_and_left() {
        let mut conn = db::open_memory().unwrap();
        let clock = FixedClock::utc(
            Utc.with_ymd_and_hms(2026, 10, 23, 9, 0, 0)
                .single()
                .unwrap(),
        );
        let task = api::capture_task(&mut conn, &clock, "Task").unwrap();
        let task = api::clarify_task(
            &mut conn,
            &clock,
            &task.id,
            crate::model::ClarifyInput {
                title: "Task".into(),
                first_step: Some("Start".into()),
                done_when: None,
                estimate_mins: Some(30),
                value: Some(3),
                deadline_on: None,
            },
            None,
            None,
        )
        .unwrap();
        let today = db::today_string(&conn, &clock).unwrap();
        api::assign_task(
            &mut conn,
            &clock,
            &today,
            &task.id,
            crate::model::Slot::One,
            None,
        )
        .unwrap();
        let view = view(&conn, &clock, "DAY", None).unwrap();
        assert_eq!(view.planned, 1);
        assert_eq!(view.left, 1);
    }
}
