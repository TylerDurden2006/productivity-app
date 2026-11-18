use rusqlite::{params, Connection, OptionalExtension};

use crate::clock::Clock;
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::Note;

const SCOPES: [&str; 6] = ["DAY", "WEEK", "MONTH", "QUARTER", "YEAR", "LONG_TERM"];

fn validate_scope(scope: &str) -> Result<String> {
    let value = scope.trim().to_uppercase();
    if !SCOPES.contains(&value.as_str()) {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "unsupported note scope",
        ));
    }
    Ok(value)
}

fn period_key(scope: &str, period_start: &str) -> Result<String> {
    if scope == "LONG_TERM" {
        return Ok(String::new());
    }
    let value = period_start.trim();
    if value.is_empty() {
        return Err(AppError::hard(
            ErrorCode::InvalidDay,
            "note period start is required",
        ));
    }
    Ok(value.to_string())
}

pub fn get(conn: &Connection, scope: &str, period_start: &str) -> Result<Note> {
    let scope = validate_scope(scope)?;
    let period = period_key(&scope, period_start)?;
    let row = conn
        .query_row(
            "SELECT body_markdown, updated_at FROM horizon_notes \
             WHERE horizon_kind = ?1 AND period_start = ?2",
            params![scope, period],
            |row| {
                Ok(Note {
                    scope: scope.clone(),
                    period_start: period.clone(),
                    body_markdown: row.get(0)?,
                    updated_at: row.get(1)?,
                })
            },
        )
        .optional()?;
    Ok(row.unwrap_or(Note {
        scope,
        period_start: period,
        body_markdown: String::new(),
        updated_at: None,
    }))
}

pub fn save(
    conn: &mut Connection,
    clock: &dyn Clock,
    scope: &str,
    period_start: &str,
    body_markdown: &str,
) -> Result<Note> {
    let scope = validate_scope(scope)?;
    let period = period_key(&scope, period_start)?;
    let now = db::now(clock);
    db::with_tx(conn, |tx| {
        tx.execute(
            "INSERT INTO horizon_notes (horizon_kind, period_start, body_markdown, updated_at) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(horizon_kind, period_start) DO UPDATE SET \
             body_markdown = excluded.body_markdown, updated_at = excluded.updated_at",
            params![scope, period, body_markdown, now],
        )?;
        Ok(Note {
            scope: scope.clone(),
            period_start: period.clone(),
            body_markdown: body_markdown.to_string(),
            updated_at: Some(now.clone()),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, FixedClock};
    use chrono::{TimeZone, Utc};

    fn clock() -> FixedClock {
        FixedClock::utc(
            Utc.with_ymd_and_hms(2026, 10, 23, 9, 0, 0)
                .single()
                .unwrap(),
        )
    }

    #[test]
    fn empty_notes_are_returned_as_blank() {
        let conn = db::open_memory().unwrap();
        let note = get(&conn, "DAY", "2026-10-23").unwrap();
        assert_eq!(note.body_markdown, "");
    }

    #[test]
    fn notes_upsert_by_scope_and_period() {
        let mut conn = db::open_memory().unwrap();
        save(&mut conn, &clock(), "WEEK", "2026-10-19", "# Week").unwrap();
        save(&mut conn, &clock(), "WEEK", "2026-10-19", "# Changed").unwrap();
        let note = get(&conn, "WEEK", "2026-10-19").unwrap();
        assert_eq!(note.body_markdown, "# Changed");
    }
}
