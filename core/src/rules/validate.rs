use crate::clock::parse_date;
use crate::error::{AppError, ErrorCode, Result};

pub const ESTIMATES: [i64; 5] = [15, 30, 60, 90, 120];

pub fn required_title(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::missing("title is required"));
    }
    Ok(trimmed.to_string())
}

pub fn required_first_step(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.len() < 3 {
        return Err(AppError::missing(
            "first step must be at least 3 characters",
        ));
    }
    Ok(trimmed.to_string())
}

pub fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub fn estimate(value: i64) -> Result<i64> {
    if ESTIMATES.contains(&value) {
        Ok(value)
    } else {
        Err(AppError::hard(
            ErrorCode::InvalidEstimate,
            "estimate must be 15, 30, 60, 90, or 120 minutes",
        ))
    }
}

pub fn value_scale(value: i64) -> Result<i64> {
    if (1..=5).contains(&value) {
        Ok(value)
    } else {
        Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "value must be between 1 and 5",
        ))
    }
}

pub fn date_optional(value: Option<String>) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if parse_date(trimmed).is_none() {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "date must use YYYY-MM-DD",
                ));
            }
            Ok(Some(trimmed.to_string()))
        }
    }
}

pub fn date_required(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if parse_date(trimmed).is_none() {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "date must use YYYY-MM-DD",
        ));
    }
    Ok(trimmed.to_string())
}

pub fn override_reason(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        if trimmed.len() >= 3 {
            Some(trimmed)
        } else {
            None
        }
    })
}

pub fn weekday_mask(value: i64) -> Result<i64> {
    if (1..=127).contains(&value) {
        Ok(value)
    } else {
        Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "pick at least one weekday",
        ))
    }
}
