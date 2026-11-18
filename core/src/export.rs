use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

use crate::clock::Clock;
use crate::db;
use crate::error::{AppError, ErrorCode, Result};

pub fn export_json(conn: &Connection, clock: &dyn Clock) -> Result<serde_json::Value> {
    let settings = db::settings(conn)?;
    let recurrences = db::list_recurrences(conn)?;
    let tasks = db::all_tasks(conn)?;
    let sessions = db::list_sessions_since(conn, "", clock)?;
    let days = db::all_days(conn)?;
    let events = db::all_events(conn)?;
    let areas = table_json(conn, "areas")?;
    let goals = table_json(conn, "goals")?;
    let task_goals = table_json(conn, "task_goals")?;
    let horizon_notes = table_json(conn, "horizon_notes")?;
    let goal_checkins = table_json(conn, "goal_checkins")?;
    let habits = table_json(conn, "habits")?;
    let habit_entries = table_json(conn, "habit_entries")?;
    let try_items = table_json(conn, "try_items")?;
    Ok(json!({
        "version": 3,
        "schema_version": db::CURRENT_SCHEMA_VERSION,
        "exported_at": db::now(clock),
        "settings": settings,
        "recurrences": recurrences,
        "tasks": tasks,
        "sessions": sessions,
        "days": days,
        "events": events,
        "areas": areas,
        "goals": goals,
        "task_goals": task_goals,
        "horizon_notes": horizon_notes,
        "goal_checkins": goal_checkins,
        "habits": habits,
        "habit_entries": habit_entries,
        "try_items": try_items,
    }))
}

pub fn backup(conn: &Connection, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if destination.exists() {
        std::fs::remove_file(destination)?;
    }
    let escaped = destination.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))?;
    Ok(())
}

pub fn validate_database(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "backup file does not exist",
        ));
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if check != "ok" {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "backup failed its integrity check",
        ));
    }
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if !(1..=db::CURRENT_SCHEMA_VERSION).contains(&version) {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "backup has an unsupported schema version",
        ));
    }
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
         AND name IN ('settings', 'tasks', 'sessions', 'days', 'events', 'recurrences')",
        [],
        |row| row.get(0),
    )?;
    if tables != 6 {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "backup is missing Flow tables",
        ));
    }
    Ok(())
}

fn table_json(conn: &Connection, table: &str) -> Result<Vec<Value>> {
    let sql = format!("SELECT * FROM {table}");
    let mut stmt = conn.prepare(&sql)?;
    let columns = stmt
        .column_names()
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let rows = stmt.query_map([], |row| {
        let mut object = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => Value::from(value),
                ValueRef::Real(value) => Value::from(value),
                ValueRef::Text(value) => Value::from(String::from_utf8_lossy(value).to_string()),
                ValueRef::Blob(value) => Value::from(value.to_vec()),
            };
            object.insert(column.clone(), value);
        }
        Ok(Value::Object(object))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn restore_file(source: &Path, destination: &Path) -> Result<()> {
    validate_database(source)?;
    std::fs::copy(source, destination)?;
    Ok(())
}
