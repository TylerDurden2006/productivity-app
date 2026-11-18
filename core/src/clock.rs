use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, NaiveDate, NaiveDateTime, Timelike, Utc,
};
use std::sync::Mutex;

pub trait Clock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
    fn logical_today(&self, rollover_hour: u32) -> NaiveDate;
}

pub fn logical_day_from(local: NaiveDateTime, rollover_hour: u32) -> NaiveDate {
    let date = local.date();
    if local.hour() < rollover_hour {
        date.pred_opt().unwrap_or(date)
    } else {
        date
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn logical_today(&self, rollover_hour: u32) -> NaiveDate {
        logical_day_from(Local::now().naive_local(), rollover_hour)
    }
}

pub struct FixedClock {
    now: Mutex<DateTime<Utc>>,
    offset: FixedOffset,
}

impl FixedClock {
    pub fn new(now: DateTime<Utc>, offset_seconds: i32) -> Self {
        Self {
            now: Mutex::new(now),
            offset: FixedOffset::east_opt(offset_seconds)
                .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap()),
        }
    }

    pub fn utc(now: DateTime<Utc>) -> Self {
        Self::new(now, 0)
    }

    pub fn set(&self, now: DateTime<Utc>) {
        *self.now.lock().unwrap_or_else(|e| e.into_inner()) = now;
    }

    pub fn advance(&self, duration: Duration) {
        let mut guard = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *guard += duration;
    }

    pub fn offset(&self) -> FixedOffset {
        self.offset
    }
}

impl Clock for FixedClock {
    fn now_utc(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn logical_today(&self, rollover_hour: u32) -> NaiveDate {
        logical_day_from(
            self.now_utc().with_timezone(&self.offset).naive_local(),
            rollover_hour,
        )
    }
}

pub fn today_string(date: NaiveDate) -> String {
    format_date(date)
}

pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub fn add_days(date: NaiveDate, days: i64) -> NaiveDate {
    if days >= 0 {
        date.checked_add_days(chrono::Days::new(days as u64))
            .unwrap_or(date)
    } else {
        date.checked_sub_days(chrono::Days::new((-days) as u64))
            .unwrap_or(date)
    }
}

pub fn now_rfc3339(clock: &dyn Clock) -> String {
    clock
        .now_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn weekday_bit(date: NaiveDate) -> u8 {
    1u8 << date.weekday().num_days_from_monday()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn rollover_hour_boundary() {
        let before = NaiveDate::from_ymd_opt(2026, 5, 5)
            .unwrap()
            .and_hms_opt(3, 59, 0)
            .unwrap();
        let after = NaiveDate::from_ymd_opt(2026, 5, 5)
            .unwrap()
            .and_hms_opt(4, 0, 0)
            .unwrap();
        assert_eq!(
            logical_day_from(before, 4),
            NaiveDate::from_ymd_opt(2026, 5, 4).unwrap()
        );
        assert_eq!(
            logical_day_from(after, 4),
            NaiveDate::from_ymd_opt(2026, 5, 5).unwrap()
        );
    }

    #[test]
    fn fixed_offsets_crossing_dst() {
        let instant = Utc.with_ymd_and_hms(2026, 3, 29, 3, 30, 0).unwrap();
        let winter = FixedClock::new(instant, 3600);
        let summer = FixedClock::new(instant, 7200);
        assert_eq!(
            winter.logical_today(4),
            NaiveDate::from_ymd_opt(2026, 3, 29).unwrap()
        );
        assert_eq!(
            summer.logical_today(4),
            NaiveDate::from_ymd_opt(2026, 3, 29).unwrap()
        );
        let late = Utc.with_ymd_and_hms(2026, 3, 29, 2, 30, 0).unwrap();
        let winter_late = FixedClock::new(late, 3600);
        let summer_late = FixedClock::new(late, 7200);
        assert_eq!(
            winter_late.logical_today(4),
            NaiveDate::from_ymd_opt(2026, 3, 28).unwrap()
        );
        assert_eq!(
            summer_late.logical_today(4),
            NaiveDate::from_ymd_opt(2026, 3, 29).unwrap()
        );
    }
}
