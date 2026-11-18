use rusqlite::{params, Connection, Transaction};

use crate::clock::Clock;
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{Band, HorizonKind, Slot, Status};

fn slot_capacity(slot: Slot) -> usize {
    if slot == Slot::Extra {
        usize::MAX
    } else {
        slot.capacity() as usize
    }
}

pub fn task(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    task_id: &str,
    target_slot: Slot,
    target_index: i64,
) -> Result<()> {
    db::with_tx(conn, |tx| {
        let all = db::list_plan_all(tx, day)?;
        let moving = all
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("task"))?;
        if moving.planned_for.as_deref() != Some(day) {
            return Err(AppError::invalid_transition(
                "task is not planned for that day",
            ));
        }
        if moving.status != Status::Ready {
            return Err(AppError::invalid_transition(
                "only open tasks can be reordered",
            ));
        }
        let source_slot = moving.slot.ok_or_else(|| AppError::missing("task slot"))?;
        let mut ordered = all
            .into_iter()
            .filter(|task| task.id != task_id)
            .collect::<Vec<_>>();
        if source_slot != target_slot {
            // Completed tasks stay on the day for the history list, but they
            // no longer occupy one of the section's open slots.
            let count = ordered
                .iter()
                .filter(|task| task.slot == Some(target_slot) && task.status == Status::Ready)
                .count();
            if count >= slot_capacity(target_slot) {
                return Err(AppError::hard(ErrorCode::SlotFull, "that section is full"));
            }
        }
        let target_open = ordered
            .iter()
            .enumerate()
            .filter(|(_, task)| task.slot == Some(target_slot) && task.status == Status::Ready)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let index = target_index.max(0) as usize;
        let insert_at = if index < target_open.len() {
            target_open[index]
        } else if let Some(last) = target_open.last().copied() {
            last + 1
        } else {
            ordered
                .iter()
                .position(|task| task.slot == Some(target_slot))
                .unwrap_or(ordered.len())
        };
        let mut task = moving;
        task.slot = Some(target_slot);
        ordered.insert(insert_at.min(ordered.len()), task);

        tx.execute(
            "UPDATE tasks SET planned_for = NULL, slot = NULL, slot_pos = NULL \
             WHERE planned_for = ?1 AND slot IS NOT NULL AND status = 'READY'",
            params![day],
        )?;
        for slot in [Slot::One, Slot::Three, Slot::Batch, Slot::Extra] {
            let ids = ordered
                .iter()
                .filter(|task| task.slot == Some(slot) && task.status == Status::Ready)
                .map(|task| task.id.clone())
                .collect::<Vec<_>>();
            for (index, id) in ids.into_iter().enumerate() {
                tx.execute(
                    "UPDATE tasks SET planned_for = ?4, slot = ?2, slot_pos = ?3 WHERE id = ?1",
                    params![id, slot.as_str(), (index + 1) as i64, day],
                )?;
            }
        }
        let _ = db::now(clock);
        Ok(())
    })
}

pub fn goal(
    conn: &mut Connection,
    kind: HorizonKind,
    period_start: &str,
    goal_id: &str,
    target_band: Band,
    target_index: i64,
) -> Result<()> {
    db::with_tx(conn, |tx| {
        let mut ordered = goals_for_period(tx, kind, period_start)?;
        let moving_index = ordered
            .iter()
            .position(|item| item.0 == goal_id)
            .ok_or_else(|| AppError::not_found("horizon task"))?;
        let (_, source_band) = ordered.remove(moving_index);
        if source_band != target_band {
            let count = ordered.iter().filter(|item| item.1 == target_band).count();
            let capacity = if target_band == Band::More {
                usize::MAX
            } else {
                target_band.capacity() as usize
            };
            if count >= capacity {
                return Err(AppError::hard(ErrorCode::SlotFull, "that section is full"));
            }
        }
        let target_indices = ordered
            .iter()
            .enumerate()
            .filter(|(_, item)| item.1 == target_band)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let index = target_index.max(0) as usize;
        let insert_at = if index < target_indices.len() {
            target_indices[index]
        } else if let Some(last) = target_indices.last().copied() {
            last + 1
        } else {
            ordered
                .iter()
                .position(|item| item.1 == target_band)
                .unwrap_or(ordered.len())
        };
        ordered.insert(
            insert_at.min(ordered.len()),
            (goal_id.to_string(), target_band),
        );
        for (position, (id, band)) in ordered.iter().enumerate() {
            tx.execute(
                "UPDATE goals SET band = ?2, position = ?3 WHERE id = ?1",
                params![id, band.as_str(), (position + 1) as i64],
            )?;
        }
        Ok(())
    })
}

fn goals_for_period(
    tx: &Transaction<'_>,
    kind: HorizonKind,
    period_start: &str,
) -> Result<Vec<(String, Band)>> {
    let mut stmt = tx.prepare(
        "SELECT id, band FROM goals WHERE horizon_kind = ?1 AND period_start = ?2 \
         AND status = 'OPEN' ORDER BY CASE band WHEN 'FOCUS' THEN 0 WHEN 'SECONDARY' THEN 1 \
         WHEN 'COMMITMENT' THEN 2 ELSE 3 END ASC, position ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![kind.as_str(), period_start], |row| {
        Ok((row.get(0)?, Band::from_db(&row.get::<_, String>(1)?)))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
