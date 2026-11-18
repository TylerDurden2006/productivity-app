use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::clock::Clock;
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    EnergyLevel, SocialMode, TimeOfDay, TryCost, TryEffort, TryItem, TryItemInput, TryStatus,
};
use crate::rules::validate;

const TRY_COLUMNS: &str = "id, title, notes, area_id, category, link, effort, cost, social_mode, \
energy, status, scheduled_for, scheduled_day_part, linked_task_id, rating, reflection, tried_at, \
created_at, updated_at";

fn item_from_row(row: &Row<'_>) -> rusqlite::Result<TryItem> {
    let effort_text: String = row.get(6)?;
    let cost_text: String = row.get(7)?;
    let social_text: String = row.get(8)?;
    let energy_text: String = row.get(9)?;
    let status_text: String = row.get(10)?;
    let day_part_text: Option<String> = row.get(12)?;
    Ok(TryItem {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        area_id: row.get(3)?,
        category: row.get(4)?,
        link: row.get(5)?,
        effort: TryEffort::from_db(&effort_text),
        cost: TryCost::from_db(&cost_text),
        social_mode: SocialMode::from_db(&social_text),
        energy: EnergyLevel::from_db(&energy_text),
        status: TryStatus::from_db(&status_text),
        scheduled_for: row.get(11)?,
        scheduled_day_part: day_part_text.map(|text| TimeOfDay::from_db(&text)),
        linked_task_id: row.get(13)?,
        rating: row.get(14)?,
        reflection: row.get(15)?,
        tried_at: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn require_item(conn: &Connection, id: &str) -> Result<TryItem> {
    let sql = format!("SELECT {TRY_COLUMNS} FROM try_items WHERE id = ?1");
    conn.query_row(&sql, params![id], item_from_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("try item"))
}

fn insert_item(conn: &Connection, item: &TryItem) -> Result<()> {
    conn.execute(
        "INSERT INTO try_items (id, title, notes, area_id, category, link, effort, cost, \
         social_mode, energy, status, scheduled_for, scheduled_day_part, linked_task_id, rating, \
         reflection, tried_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
         ?18, ?19)",
        params![
            item.id,
            item.title,
            item.notes,
            item.area_id,
            item.category,
            item.link,
            item.effort.as_str(),
            item.cost.as_str(),
            item.social_mode.as_str(),
            item.energy.as_str(),
            item.status.as_str(),
            item.scheduled_for,
            item.scheduled_day_part.map(|part| part.as_str()),
            item.linked_task_id,
            item.rating,
            item.reflection,
            item.tried_at,
            item.created_at,
            item.updated_at
        ],
    )?;
    Ok(())
}

fn write_item(conn: &Connection, item: &TryItem) -> Result<()> {
    conn.execute(
        "UPDATE try_items SET title = ?2, notes = ?3, area_id = ?4, category = ?5, link = ?6, \
         effort = ?7, cost = ?8, social_mode = ?9, energy = ?10, status = ?11, scheduled_for = ?12, \
         scheduled_day_part = ?13, linked_task_id = ?14, rating = ?15, reflection = ?16, \
         tried_at = ?17, updated_at = ?18 WHERE id = ?1",
        params![
            item.id,
            item.title,
            item.notes,
            item.area_id,
            item.category,
            item.link,
            item.effort.as_str(),
            item.cost.as_str(),
            item.social_mode.as_str(),
            item.energy.as_str(),
            item.status.as_str(),
            item.scheduled_for,
            item.scheduled_day_part.map(|part| part.as_str()),
            item.linked_task_id,
            item.rating,
            item.reflection,
            item.tried_at,
            item.updated_at
        ],
    )?;
    Ok(())
}

fn merged_text(current: Option<String>, input: Option<String>) -> Option<String> {
    match input {
        Some(value) => validate::optional_text(Some(value)),
        None => current,
    }
}

fn schedule_date(value: &str) -> Result<String> {
    let trimmed = value.trim();
    match NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        Ok(date) if date.format("%Y-%m-%d").to_string() == trimmed => Ok(trimmed.to_string()),
        _ => Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "date must use YYYY-MM-DD",
        )),
    }
}

fn checked_rating(value: Option<i64>) -> Result<Option<i64>> {
    match value {
        Some(value) if !(1..=5).contains(&value) => Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "rating must be between 1 and 5",
        )),
        other => Ok(other),
    }
}

pub fn list(conn: &Connection) -> Result<Vec<TryItem>> {
    let sql = format!("SELECT {TRY_COLUMNS} FROM try_items ORDER BY created_at ASC, id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], item_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<TryItem> {
    require_item(conn, id)
}

pub fn create(conn: &mut Connection, clock: &dyn Clock, input: TryItemInput) -> Result<TryItem> {
    db::with_tx(conn, |tx| create_in(tx, clock, input))
}

fn create_in(tx: &Transaction<'_>, clock: &dyn Clock, input: TryItemInput) -> Result<TryItem> {
    let now = db::now(clock);
    let item = TryItem {
        id: uuid::Uuid::new_v4().to_string(),
        title: validate::required_title(&input.title)?,
        notes: validate::optional_text(input.notes),
        area_id: validate::optional_text(input.area_id),
        category: validate::optional_text(input.category),
        link: validate::optional_text(input.link),
        effort: input.effort.unwrap_or(TryEffort::Medium),
        cost: input.cost.unwrap_or(TryCost::Low),
        social_mode: input.social_mode.unwrap_or(SocialMode::Either),
        energy: input.energy.unwrap_or(EnergyLevel::Medium),
        status: TryStatus::Idea,
        scheduled_for: None,
        scheduled_day_part: None,
        linked_task_id: None,
        rating: None,
        reflection: None,
        tried_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    insert_item(tx, &item)?;
    Ok(item)
}

pub fn update(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: TryItemInput,
) -> Result<TryItem> {
    db::with_tx(conn, |tx| update_in(tx, clock, id, input))
}

fn update_in(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    input: TryItemInput,
) -> Result<TryItem> {
    let mut item = require_item(tx, id)?;
    item.title = validate::required_title(&input.title)?;
    item.notes = merged_text(item.notes, input.notes);
    item.area_id = merged_text(item.area_id, input.area_id);
    item.category = merged_text(item.category, input.category);
    item.link = merged_text(item.link, input.link);
    if let Some(effort) = input.effort {
        item.effort = effort;
    }
    if let Some(cost) = input.cost {
        item.cost = cost;
    }
    if let Some(social_mode) = input.social_mode {
        item.social_mode = social_mode;
    }
    if let Some(energy) = input.energy {
        item.energy = energy;
    }
    item.updated_at = db::now(clock);
    write_item(tx, &item)?;
    Ok(item)
}

pub fn schedule(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    date: &str,
    day_part: TimeOfDay,
) -> Result<TryItem> {
    db::with_tx(conn, |tx| schedule_in(tx, clock, id, date, day_part))
}

fn schedule_in(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    date: &str,
    day_part: TimeOfDay,
) -> Result<TryItem> {
    let date = schedule_date(date)?;
    let mut item = require_item(tx, id)?;
    item.status = TryStatus::Scheduled;
    item.scheduled_for = Some(date);
    item.scheduled_day_part = Some(day_part);
    item.updated_at = db::now(clock);
    write_item(tx, &item)?;
    Ok(item)
}

pub fn unschedule(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<TryItem> {
    db::with_tx(conn, |tx| unschedule_in(tx, clock, id))
}

fn unschedule_in(tx: &Transaction<'_>, clock: &dyn Clock, id: &str) -> Result<TryItem> {
    let mut item = require_item(tx, id)?;
    item.status = TryStatus::Idea;
    item.scheduled_for = None;
    item.scheduled_day_part = None;
    item.updated_at = db::now(clock);
    write_item(tx, &item)?;
    Ok(item)
}

pub fn mark_tried(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    rating: Option<i64>,
    reflection: Option<String>,
) -> Result<TryItem> {
    db::with_tx(conn, |tx| mark_tried_in(tx, clock, id, rating, reflection))
}

fn mark_tried_in(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    rating: Option<i64>,
    reflection: Option<String>,
) -> Result<TryItem> {
    let rating = checked_rating(rating)?;
    let reflection = validate::optional_text(reflection);
    let now = db::now(clock);
    let mut item = require_item(tx, id)?;
    item.status = TryStatus::Tried;
    item.tried_at = Some(now.clone());
    item.rating = rating;
    item.reflection = reflection;
    item.updated_at = now;
    write_item(tx, &item)?;
    Ok(item)
}

pub fn skip_item(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<TryItem> {
    db::with_tx(conn, |tx| skip_in(tx, clock, id))
}

fn skip_in(tx: &Transaction<'_>, clock: &dyn Clock, id: &str) -> Result<TryItem> {
    let mut item = require_item(tx, id)?;
    item.status = TryStatus::Skipped;
    item.updated_at = db::now(clock);
    write_item(tx, &item)?;
    Ok(item)
}

pub fn set_linked_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    task_id: Option<String>,
) -> Result<TryItem> {
    db::with_tx(conn, |tx| set_linked_task_in(tx, clock, id, task_id))
}

fn set_linked_task_in(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    task_id: Option<String>,
) -> Result<TryItem> {
    let linked_task_id = match task_id {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(AppError::not_found("task"));
            }
            db::require_task(tx, trimmed)?;
            Some(trimmed.to_string())
        }
        None => None,
    };
    let mut item = require_item(tx, id)?;
    item.linked_task_id = linked_task_id;
    item.updated_at = db::now(clock);
    write_item(tx, &item)?;
    Ok(item)
}
