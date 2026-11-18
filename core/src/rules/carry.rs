use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rusqlite::{params, Transaction};

use crate::clock::{format_date, Clock};
use crate::db;
use crate::error::Result;
use crate::model::{Band, HorizonKind, Slot, Status, Task};
use crate::rules::horizon;

fn band_name(band: Band) -> &'static str {
    band.as_str()
}

fn slots() -> Vec<Slot> {
    vec![Slot::One, Slot::Three, Slot::Batch, Slot::Extra]
}

fn capacity(slot: Slot, len: usize) -> usize {
    if slot == Slot::Extra {
        len
    } else {
        slot.capacity() as usize
    }
}

fn source_tasks(tx: &Transaction<'_>, today: &str) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {} FROM tasks WHERE status = 'READY' AND recurrence_id IS NULL \
         AND planned_for IS NOT NULL AND planned_for < ?1 AND slot IS NOT NULL \
         ORDER BY planned_for ASC, CASE slot WHEN 'ONE' THEN 0 WHEN 'THREE' THEN 1 \
         WHEN 'BATCH' THEN 2 ELSE 3 END ASC, slot_pos ASC, id ASC",
        db::TASK_COLUMNS
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![today], db::task_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn daily(tx: &Transaction<'_>, clock: &dyn Clock) -> Result<Vec<String>> {
    let settings = db::settings(tx)?;
    if !settings.auto_carry_enabled {
        return Ok(Vec::new());
    }
    let today = db::today(tx, clock)?;
    let today_text = format_date(today);
    let sources = source_tasks(tx, &today_text)?;
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let targets = db::list_plan_all(tx, &today_text)?;
    let now = db::now(clock);

    let source_ids = sources
        .iter()
        .map(|task| task.id.clone())
        .collect::<HashSet<_>>();
    let mut source_bands: HashMap<Slot, Vec<String>> = HashMap::new();
    let mut old_day: HashMap<String, String> = HashMap::new();
    for task in &sources {
        if let Some(slot) = task.slot {
            source_bands.entry(slot).or_default().push(task.id.clone());
            old_day.insert(
                task.id.clone(),
                task.planned_for.clone().unwrap_or_default(),
            );
        }
    }
    let mut target_bands: HashMap<Slot, Vec<String>> = HashMap::new();
    let mut sorted_targets = targets.clone();
    sorted_targets.sort_by_key(|task| task.slot_pos.unwrap_or(i64::MAX));
    for task in sorted_targets
        .iter()
        .filter(|task| task.status == Status::Ready)
    {
        if let Some(slot) = task.slot {
            target_bands.entry(slot).or_default().push(task.id.clone());
        }
    }

    let mut overflow: Vec<String> = Vec::new();
    let mut assignments: Vec<(String, Slot, i64)> = Vec::new();
    for slot in slots() {
        let mut ordered = Vec::new();
        ordered.append(&mut overflow);
        if let Some(mut source) = source_bands.remove(&slot) {
            ordered.append(&mut source);
        }
        if let Some(mut target) = target_bands.remove(&slot) {
            ordered.append(&mut target);
        }
        let keep = capacity(slot, ordered.len());
        for (index, id) in ordered.drain(..).enumerate() {
            if index < keep {
                assignments.push((id, slot, (index + 1) as i64));
            } else {
                overflow.push(id);
            }
        }
    }

    tx.execute(
        "UPDATE tasks SET planned_for = NULL, slot = NULL, slot_pos = NULL \
         WHERE planned_for = ?1 AND slot IS NOT NULL AND status = 'READY'",
        params![today_text],
    )?;

    for (id, slot, position) in &assignments {
        if source_ids.contains(id) {
            tx.execute(
                "UPDATE tasks SET planned_for = ?2, first_planned_for = COALESCE(first_planned_for, planned_for), \
                 rollover_count = rollover_count + 1, slot = ?3, slot_pos = ?4, updated_at = ?5 \
                 WHERE id = ?1",
                params![id, today_text, slot.as_str(), position, now],
            )?;
            if let Some(from) = old_day.get(id) {
                db::insert_event(tx, &now, Some(id), "ROLLED_OVER", Some(from), None)?;
            }
        } else {
            tx.execute(
                "UPDATE tasks SET planned_for = ?4, slot = ?2, slot_pos = ?3 \
                 WHERE id = ?1",
                params![id, slot.as_str(), position, today_text],
            )?;
        }
    }

    Ok(assignments
        .into_iter()
        .filter(|(id, _, _)| source_ids.contains(id))
        .map(|(id, _, _)| id)
        .collect())
}

struct GoalItem {
    id: String,
    band: Band,
}

fn goal_items(
    tx: &Transaction<'_>,
    kind: HorizonKind,
    period_start: &str,
    before: bool,
) -> Result<Vec<GoalItem>> {
    let comparison = if before { "<" } else { "=" };
    let sql = format!(
        "SELECT id, band FROM goals WHERE horizon_kind = ?1 \
         AND period_start {comparison} ?2 AND status = 'OPEN' \
         ORDER BY CASE band WHEN 'FOCUS' THEN 0 WHEN 'SECONDARY' THEN 1 \
         WHEN 'COMMITMENT' THEN 2 ELSE 3 END ASC, position ASC, created_at ASC, id ASC"
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![kind.as_str(), period_start], |row| {
        Ok(GoalItem {
            id: row.get(0)?,
            band: Band::from_db(&row.get::<_, String>(1)?),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn carry_kind(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    kind: HorizonKind,
    today: NaiveDate,
    week_starts_on: i64,
) -> Result<()> {
    if kind == HorizonKind::LongTerm {
        return Ok(());
    }
    let (Some(period_start), _, _) = horizon::period_bounds(kind, None, today, week_starts_on)?
    else {
        return Ok(());
    };
    let sources = goal_items(tx, kind, &period_start, true)?;
    if sources.is_empty() {
        return Ok(());
    }
    let targets = goal_items(tx, kind, &period_start, false)?;
    let now = db::now(clock);
    let source_ids = sources
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut source_bands: HashMap<Band, Vec<String>> = HashMap::new();
    for item in &sources {
        source_bands
            .entry(item.band)
            .or_default()
            .push(item.id.clone());
    }
    let mut target_bands: HashMap<Band, Vec<String>> = HashMap::new();
    for item in &targets {
        target_bands
            .entry(item.band)
            .or_default()
            .push(item.id.clone());
    }

    let mut overflow: Vec<String> = Vec::new();
    let mut assignments: Vec<(String, Band, i64)> = Vec::new();
    for band in [Band::Focus, Band::Secondary, Band::Commitment, Band::More] {
        let mut ordered = Vec::new();
        ordered.append(&mut overflow);
        if let Some(mut source) = source_bands.remove(&band) {
            ordered.append(&mut source);
        }
        if let Some(mut target) = target_bands.remove(&band) {
            ordered.append(&mut target);
        }
        let keep = if band == Band::More {
            ordered.len()
        } else {
            band.capacity() as usize
        };
        for (index, id) in ordered.drain(..).enumerate() {
            if index < keep {
                assignments.push((id, band, (index + 1) as i64));
            } else {
                overflow.push(id);
            }
        }
    }

    for (id, band, position) in assignments {
        if source_ids.contains(&id) {
            tx.execute(
                "UPDATE goals SET period_start = ?2, \
                 first_period_start = COALESCE(first_period_start, period_start), \
                 carry_count = carry_count + 1, band = ?3, position = ?4, updated_at = ?5 \
                 WHERE id = ?1",
                params![id, period_start, band_name(band), position, now],
            )?;
        } else {
            tx.execute(
                "UPDATE goals SET band = ?2, position = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, band_name(band), position, now],
            )?;
        }
    }
    Ok(())
}

pub fn horizons(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    settings: &crate::model::Settings,
    today: NaiveDate,
) -> Result<()> {
    if !settings.auto_carry_enabled {
        return Ok(());
    }
    for kind in [
        HorizonKind::Week,
        HorizonKind::Month,
        HorizonKind::Quarter,
        HorizonKind::Year,
    ] {
        carry_kind(tx, clock, kind, today, settings.week_starts_on)?;
    }
    Ok(())
}
