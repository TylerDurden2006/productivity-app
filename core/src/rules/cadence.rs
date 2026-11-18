use chrono::{Datelike, NaiveDate};

use crate::clock::{add_days, parse_date, weekday_bit};
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{Cadence, CadenceKind, QuotaPeriod};

#[allow(clippy::collapsible_match)]
pub fn validate(cadence: &Cadence) -> Result<NaiveDate> {
    let anchor = parse_date(&cadence.anchor_on)
        .ok_or_else(|| AppError::hard(ErrorCode::InvalidSetting, "cadence needs a start date"))?;
    if cadence.interval_n < 1 {
        return Err(AppError::hard(
            ErrorCode::InvalidSetting,
            "cadence interval must be at least one",
        ));
    }
    match cadence.kind {
        CadenceKind::Weekly => {
            if cadence.weekdays_mask < 1 || cadence.weekdays_mask > 127 {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "choose at least one weekday",
                ));
            }
        }
        CadenceKind::MonthlyDay => {
            let day = cadence.month_day.unwrap_or(0);
            if !(1..=31).contains(&day) {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "month day must be between 1 and 31",
                ));
            }
        }
        CadenceKind::MonthlyNthWeekday => {
            let nth = cadence.nth_weekday.unwrap_or(0);
            let weekday = cadence.weekday.unwrap_or(-1);
            if !(-1..=4).contains(&nth) || nth == 0 || !(0..=6).contains(&weekday) {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "choose a monthly weekday pattern",
                ));
            }
        }
        CadenceKind::Yearly => {
            let month = cadence.month.unwrap_or(0);
            let day = cadence.month_day.unwrap_or(0);
            if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "choose a month and day",
                ));
            }
        }
        CadenceKind::CustomDates => {
            if cadence.dates.is_empty() {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "choose at least one custom date",
                ));
            }
            for date in &cadence.dates {
                if parse_date(date).is_none() {
                    return Err(AppError::hard(
                        ErrorCode::InvalidSetting,
                        "custom dates must use YYYY-MM-DD",
                    ));
                }
            }
        }
        CadenceKind::Quota => {
            let count = cadence.quota_count.unwrap_or(0);
            if count < 1 {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "quota must be at least one",
                ));
            }
            if cadence.quota_period.is_none() {
                return Err(AppError::hard(
                    ErrorCode::InvalidSetting,
                    "choose a quota period",
                ));
            }
        }
        _ => {}
    }
    Ok(anchor)
}

pub fn anchor(cadence: &Cadence) -> NaiveDate {
    parse_date(&cadence.anchor_on)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid fallback date"))
}

pub fn is_due_on(cadence: &Cadence, date: NaiveDate) -> bool {
    let start = anchor(cadence);
    if date < start {
        return false;
    }
    match cadence.kind {
        CadenceKind::Daily => true,
        CadenceKind::Weekly => {
            if cadence.weekdays_mask & weekday_bit(date) as i64 == 0 {
                return false;
            }
            week_difference(start, date).rem_euclid(cadence.interval_n.max(1)) == 0
        }
        CadenceKind::EveryNDays => {
            (date - start)
                .num_days()
                .rem_euclid(cadence.interval_n.max(1))
                == 0
        }
        CadenceKind::EveryNWeeks => {
            if cadence.weekdays_mask & weekday_bit(date) as i64 == 0 {
                return false;
            }
            week_difference(start, date).rem_euclid(cadence.interval_n.max(1)) == 0
        }
        CadenceKind::MonthlyDay => {
            date.day() as i64 == clamped_day(date.year(), date.month(), cadence.month_day)
        }
        CadenceKind::MonthlyNthWeekday => {
            let weekday = cadence.weekday.unwrap_or(0);
            let nth = cadence.nth_weekday.unwrap_or(1);
            date.weekday().num_days_from_monday() as i64 == weekday
                && (if nth == -1 {
                    add_days(date, 7).month() != date.month()
                } else {
                    weekday_ordinal(date) == nth
                })
        }
        CadenceKind::Yearly => {
            let month = cadence.month.unwrap_or(1);
            let day = clamped_day(date.year(), month as u32, cadence.month_day);
            date.month() as i64 == month && date.day() as i64 == day
        }
        CadenceKind::CustomDates => cadence
            .dates
            .iter()
            .filter_map(|value| parse_date(value))
            .any(|value| value == date),
        CadenceKind::Quota => true,
        CadenceKind::OneOff => date == start,
    }
}

pub fn next_on_or_after(cadence: &Cadence, from: NaiveDate) -> NaiveDate {
    let start = anchor(cadence);
    if from <= start {
        return start;
    }
    if cadence.kind == CadenceKind::CustomDates {
        let mut dates = cadence
            .dates
            .iter()
            .filter_map(|value| parse_date(value))
            .collect::<Vec<_>>();
        dates.sort_unstable();
        return dates.into_iter().find(|date| *date >= from).unwrap_or(from);
    }
    if cadence.kind == CadenceKind::OneOff {
        return start;
    }
    let mut candidate = from;
    for _ in 0..=(366 * 5) {
        if is_due_on(cadence, candidate) {
            return candidate;
        }
        candidate = add_days(candidate, 1);
    }
    from
}

pub fn next_after(cadence: &Cadence, date: NaiveDate) -> NaiveDate {
    next_on_or_after(cadence, add_days(date, 1))
}

pub fn period_start(cadence: &Cadence, date: NaiveDate) -> NaiveDate {
    match cadence.quota_period.unwrap_or(QuotaPeriod::Week) {
        QuotaPeriod::Week => add_days(date, -weekday_index(date)),
        QuotaPeriod::Month => NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date),
    }
}

pub fn period_end(cadence: &Cadence, date: NaiveDate) -> NaiveDate {
    let start = period_start(cadence, date);
    match cadence.quota_period.unwrap_or(QuotaPeriod::Week) {
        QuotaPeriod::Week => add_days(start, 6),
        QuotaPeriod::Month => {
            let next = if date.month() == 12 {
                NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
            };
            next.and_then(|value| value.pred_opt()).unwrap_or(date)
        }
    }
}

pub fn label(cadence: &Cadence) -> String {
    match cadence.kind {
        CadenceKind::Daily => "Daily".into(),
        CadenceKind::Weekly => weekdays_label(cadence.weekdays_mask, cadence.interval_n),
        CadenceKind::EveryNDays => {
            if cadence.interval_n == 1 {
                "Every day".into()
            } else {
                format!("Every {} days", cadence.interval_n)
            }
        }
        CadenceKind::EveryNWeeks => {
            if cadence.interval_n == 1 {
                weekdays_label(cadence.weekdays_mask, 1)
            } else {
                format!(
                    "Every {} weeks on {}",
                    cadence.interval_n,
                    weekdays_label(cadence.weekdays_mask, 1)
                )
            }
        }
        CadenceKind::MonthlyDay => {
            format!("Monthly on the {}", ordinal(cadence.month_day.unwrap_or(1)))
        }
        CadenceKind::MonthlyNthWeekday => {
            let nth = cadence.nth_weekday.unwrap_or(1);
            let weekday = weekday_name(cadence.weekday.unwrap_or(0));
            if nth == -1 {
                format!("Last {weekday} of the month")
            } else {
                format!("{} {weekday} of the month", ordinal(nth))
            }
        }
        CadenceKind::Yearly => format!(
            "Every year on {}/{}",
            cadence.month.unwrap_or(1),
            cadence.month_day.unwrap_or(1)
        ),
        CadenceKind::CustomDates => format!("{} custom dates", cadence.dates.len()),
        CadenceKind::Quota => {
            let period = match cadence.quota_period.unwrap_or(QuotaPeriod::Week) {
                QuotaPeriod::Week => "week",
                QuotaPeriod::Month => "month",
            };
            format!("{} times a {period}", cadence.quota_count.unwrap_or(1))
        }
        CadenceKind::OneOff => "One time".into(),
    }
}

fn clamped_day(year: i32, month: u32, day: Option<i64>) -> i64 {
    let max = days_in_month(year, month);
    day.unwrap_or(1).clamp(1, max)
}

fn days_in_month(year: i32, month: u32) -> i64 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|date| date.pred_opt())
        .map(|date| date.day() as i64)
        .unwrap_or(28)
}

fn weekday_index(date: NaiveDate) -> i64 {
    date.weekday().num_days_from_monday() as i64
}

fn week_difference(start: NaiveDate, date: NaiveDate) -> i64 {
    let start_week = add_days(start, -weekday_index(start));
    let date_week = add_days(date, -weekday_index(date));
    (date_week - start_week).num_days() / 7
}

fn weekday_ordinal(date: NaiveDate) -> i64 {
    let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
    let offset = weekday_index(date) - weekday_index(first);
    let offset = if offset < 0 { offset + 7 } else { offset };
    (offset / 7) + 1
}

fn weekdays_label(mask: i64, interval: i64) -> String {
    let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let selected = names
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();
    let joined = if selected.is_empty() {
        "no days".to_string()
    } else if selected.len() == 7 {
        "Every day".to_string()
    } else {
        selected.join(", ")
    };
    if interval > 1 {
        format!("Every {interval} weeks: {joined}")
    } else {
        joined
    }
}

fn weekday_name(index: i64) -> &'static str {
    match index {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        _ => "Sunday",
    }
}

fn ordinal(value: i64) -> String {
    let suffix = match value % 100 {
        11..=13 => "th",
        _ => match value % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{value}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cadence(kind: CadenceKind, anchor_on: &str) -> Cadence {
        Cadence {
            kind,
            anchor_on: anchor_on.into(),
            ..Cadence::default()
        }
    }

    fn date(value: &str) -> NaiveDate {
        parse_date(value).unwrap()
    }

    #[test]
    fn every_37_days_is_anchored() {
        let cadence = Cadence {
            kind: CadenceKind::EveryNDays,
            interval_n: 37,
            ..cadence(CadenceKind::EveryNDays, "2026-01-01")
        };
        assert!(is_due_on(&cadence, date("2026-01-01")));
        assert!(is_due_on(&cadence, date("2026-02-07")));
        assert!(!is_due_on(&cadence, date("2026-02-08")));
        assert_eq!(
            next_on_or_after(&cadence, date("2026-01-02")),
            date("2026-02-07")
        );
    }

    #[test]
    fn weekdays_and_interval_work() {
        let cadence = Cadence {
            kind: CadenceKind::EveryNWeeks,
            interval_n: 2,
            weekdays_mask: 0b0010101,
            ..cadence(CadenceKind::EveryNWeeks, "2026-01-05")
        };
        assert!(is_due_on(&cadence, date("2026-01-05")));
        assert!(!is_due_on(&cadence, date("2026-01-12")));
        assert!(is_due_on(&cadence, date("2026-01-19")));
    }

    #[test]
    fn month_end_is_clamped() {
        let cadence = Cadence {
            kind: CadenceKind::MonthlyDay,
            month_day: Some(31),
            ..cadence(CadenceKind::MonthlyDay, "2026-01-31")
        };
        assert!(is_due_on(&cadence, date("2026-02-28")));
        assert!(is_due_on(&cadence, date("2026-04-30")));
        assert!(!is_due_on(&cadence, date("2026-04-29")));
    }

    #[test]
    fn last_weekday_is_found() {
        let cadence = Cadence {
            kind: CadenceKind::MonthlyNthWeekday,
            nth_weekday: Some(-1),
            weekday: Some(4),
            ..cadence(CadenceKind::MonthlyNthWeekday, "2026-01-01")
        };
        assert!(is_due_on(&cadence, date("2026-01-30")));
        assert!(!is_due_on(&cadence, date("2026-01-23")));
    }

    #[test]
    fn custom_dates_are_sorted() {
        let cadence = Cadence {
            kind: CadenceKind::CustomDates,
            dates: vec!["2026-03-03".into(), "2026-01-17".into()],
            ..cadence(CadenceKind::CustomDates, "2026-01-01")
        };
        assert_eq!(
            next_on_or_after(&cadence, date("2026-02-01")),
            date("2026-03-03")
        );
    }

    #[test]
    fn labels_are_human_readable() {
        let cadence = Cadence {
            kind: CadenceKind::Quota,
            quota_count: Some(3),
            quota_period: Some(QuotaPeriod::Week),
            ..cadence(CadenceKind::Quota, "2026-01-01")
        };
        assert_eq!(label(&cadence), "3 times a week");
    }
}
