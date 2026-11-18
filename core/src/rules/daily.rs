use rusqlite::Connection;

use crate::clock::Clock;
use crate::db;
use crate::error::Result;
use crate::model::{DailyView, HabitEntryStatus};
use crate::rules::{habit, plan};

pub fn view(conn: &Connection, clock: &dyn Clock, day: Option<String>) -> Result<DailyView> {
    let target = match day {
        Some(value) => value,
        None => db::today_string(conn, clock)?,
    };
    let plan_view = plan::view(conn, clock, Some(target.clone()))?;
    let habits = habit::today(conn, clock, Some(target.clone()))?;
    let habit_total = habits.len() as i64;
    let habit_done = habits
        .iter()
        .filter(|entry| {
            entry
                .entry
                .as_ref()
                .is_some_and(|item| item.status == HabitEntryStatus::Done)
        })
        .count() as i64;
    let completed = db::list_done_for_day(conn, &target)?;

    Ok(DailyView {
        day: target,
        locked: plan_view.locked,
        plan: plan_view.plan,
        budget: plan_view.budget,
        habits,
        habit_total,
        habit_done,
        completed,
    })
}
