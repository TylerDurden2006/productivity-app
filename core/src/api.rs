use std::path::Path;

use rusqlite::Connection;

use crate::clock::{format_date, parse_rfc3339, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    Area, AreaInput, Band, ClarifyInput, CurrentTask, DailyView, DashboardView, Day, DoState,
    DoView, Goal, GoalInput, Habit, HabitEntry, HabitInput, HabitStats, HabitToday, HorizonKind,
    HorizonView, IfFull, LockResult, Mode, Note, PlanAssignment, PlanProposal, PlanView,
    QuickAddInput, ReactivateInput, ReconcileReport, Recurrence, RecurrenceInput, RecurrencePatch,
    RecurrenceView, ResolveInput, SessionKind, SessionView, Settings, SettingsPatch, Slot,
    SplitChild, StartupStatus, Stats, Task, TaskPatch, TimeOfDay, TriageInput, TryItem,
    TryItemInput, WrapupView,
};
use crate::rules::{
    calibration, clarify, daily, dashboard, habit, horizon, notes, plan, ranking, recurrence,
    reorder, review, session, try_shelf, validate, wrapup,
};

pub fn startup_status(conn: &Connection) -> StartupStatus {
    match db::integrity_ok(conn) {
        Ok(true) => StartupStatus {
            ok: true,
            error: None,
        },
        Ok(false) => StartupStatus {
            ok: false,
            error: Some("The database failed its integrity check.".to_string()),
        },
        Err(error) => StartupStatus {
            ok: false,
            error: Some(error.message),
        },
    }
}

pub fn settings(conn: &Connection) -> Result<Settings> {
    db::settings(conn)
}

pub fn update_settings(
    conn: &mut Connection,
    clock: &dyn Clock,
    patch: SettingsPatch,
) -> Result<Settings> {
    db::with_tx(conn, |tx| {
        let current = db::settings(tx)?;
        let rollover = patch.day_rollover_hour.unwrap_or(current.day_rollover_hour);
        if !(0..=23).contains(&rollover) {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "rollover hour must be between 0 and 23",
            ));
        }
        let budget = patch
            .daily_focus_budget_mins
            .unwrap_or(current.daily_focus_budget_mins);
        if budget <= 0 {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "daily budget must be positive",
            ));
        }
        let ready_cap = patch.ready_cap.unwrap_or(current.ready_cap);
        if ready_cap < 0 {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "ready cap cannot be negative",
            ));
        }
        let focus_block = patch.focus_block_mins.unwrap_or(current.focus_block_mins);
        let batch_block = patch.batch_block_mins.unwrap_or(current.batch_block_mins);
        if focus_block <= 0 || batch_block <= 0 {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "block lengths must be positive",
            ));
        }
        let multiplier = patch
            .default_multiplier
            .unwrap_or(current.default_multiplier);
        if multiplier <= 0.0 {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "default multiplier must be positive",
            ));
        }
        let week_starts_on = patch.week_starts_on.unwrap_or(current.week_starts_on);
        if !(0..=6).contains(&week_starts_on) {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "week start must be a weekday from 0 to 6",
            ));
        }
        let auto_carry_enabled = patch
            .auto_carry_enabled
            .unwrap_or(current.auto_carry_enabled);
        let dashboard_default_range = patch
            .dashboard_default_range
            .unwrap_or_else(|| current.dashboard_default_range.clone());
        let allowed_ranges = ["DAY", "WEEK", "MONTH", "QUARTER", "YEAR", "ALL"];
        if !allowed_ranges.contains(&dashboard_default_range.to_uppercase().as_str()) {
            return Err(AppError::hard(
                ErrorCode::InvalidSetting,
                "dashboard range is not supported",
            ));
        }
        let notes_autosave = patch.notes_autosave.unwrap_or(current.notes_autosave);
        let notify_block_end = patch.notify_block_end.unwrap_or(current.notify_block_end);
        let planning_reminder_enabled = patch
            .planning_reminder_enabled
            .unwrap_or(current.planning_reminder_enabled);
        let planning_reminder_time = match patch.planning_reminder_time {
            Some(value) => value,
            None => current.planning_reminder_time.clone(),
        };
        let reduced_motion = patch.reduced_motion.unwrap_or(current.reduced_motion);
        let last_review = match patch.last_weekly_review_at {
            Some(None) => None,
            Some(Some(value)) => {
                if parse_rfc3339(&value).is_none() {
                    return Err(AppError::hard(
                        ErrorCode::InvalidSetting,
                        "review timestamp is invalid",
                    ));
                }
                Some(value)
            }
            None => current.last_weekly_review_at,
        };
        tx.execute(
            "UPDATE settings SET day_rollover_hour = ?1, daily_focus_budget_mins = ?2, ready_cap = ?3, \
             focus_block_mins = ?4, batch_block_mins = ?5, default_multiplier = ?6, \
             last_weekly_review_at = ?7, week_starts_on = ?8, auto_carry_enabled = ?9, \
             dashboard_default_range = ?10, notes_autosave = ?11, notify_block_end = ?12, \
             planning_reminder_enabled = ?13, planning_reminder_time = ?14, reduced_motion = ?15 \
             WHERE id = 1",
            rusqlite::params![
                rollover,
                budget,
                ready_cap,
                focus_block,
                batch_block,
                multiplier,
                last_review,
                week_starts_on,
                auto_carry_enabled as i64,
                dashboard_default_range.to_uppercase(),
                notes_autosave as i64,
                notify_block_end as i64,
                planning_reminder_enabled as i64,
                planning_reminder_time,
                reduced_motion as i64
            ],
        )?;
        let _ = clock;
        db::settings(tx)
    })
}

pub fn plan_view(conn: &Connection, clock: &dyn Clock, day: Option<String>) -> Result<PlanView> {
    plan::view(conn, clock, day)
}

pub fn daily_view(conn: &Connection, clock: &dyn Clock, day: Option<String>) -> Result<DailyView> {
    daily::view(conn, clock, day)
}

pub fn horizon_view(
    conn: &Connection,
    clock: &dyn Clock,
    kind: HorizonKind,
    period_start: Option<String>,
) -> Result<HorizonView> {
    horizon::view(conn, clock, kind, period_start)
}

pub fn create_goal(conn: &mut Connection, clock: &dyn Clock, input: GoalInput) -> Result<Goal> {
    horizon::create_goal(conn, clock, input)
}

fn goal_band_order(requested: Band) -> Vec<Band> {
    match requested {
        Band::Focus => vec![Band::Focus, Band::Secondary, Band::Commitment, Band::More],
        Band::Secondary => vec![Band::Secondary, Band::Commitment, Band::More],
        Band::Commitment => vec![Band::Commitment, Band::More],
        Band::More => vec![Band::More],
    }
}

pub fn create_goal_quick(
    conn: &mut Connection,
    clock: &dyn Clock,
    input: GoalInput,
) -> Result<Goal> {
    for band in goal_band_order(input.band) {
        let attempt = GoalInput {
            band,
            ..input.clone()
        };
        match horizon::create_goal(conn, clock, attempt) {
            Ok(goal) => return Ok(goal),
            Err(error) if error.code == ErrorCode::SlotFull => continue,
            Err(error) => return Err(error),
        }
    }
    Err(AppError::hard(ErrorCode::SlotFull, "that band is full"))
}

pub fn update_goal(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: GoalInput,
) -> Result<Goal> {
    horizon::update_goal(conn, clock, id, input)
}

pub fn complete_goal(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<Goal> {
    horizon::complete_goal(conn, clock, id)
}

pub fn reopen_goal(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<Goal> {
    horizon::reopen_goal(conn, clock, id)
}

pub fn link_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    task_id: &str,
    goal_id: &str,
) -> Result<()> {
    horizon::link_task(conn, clock, task_id, goal_id)
}

pub fn unlink_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    task_id: &str,
    goal_id: &str,
) -> Result<()> {
    let _ = clock;
    horizon::unlink_task(conn, task_id, goal_id)
}

pub fn set_horizon_note(
    conn: &mut Connection,
    clock: &dyn Clock,
    kind: HorizonKind,
    period_start: &str,
    intent: Option<String>,
    review_note: Option<String>,
) -> Result<()> {
    horizon::set_note(conn, clock, kind, period_start, intent, review_note)
}

pub fn areas(conn: &Connection) -> Result<Vec<Area>> {
    horizon::list_areas(conn)
}

pub fn create_area(conn: &mut Connection, clock: &dyn Clock, input: AreaInput) -> Result<Area> {
    horizon::create_area(conn, clock, input)
}

pub fn habits(conn: &Connection) -> Result<Vec<Habit>> {
    habit::list(conn)
}

pub fn create_habit(conn: &mut Connection, clock: &dyn Clock, input: HabitInput) -> Result<Habit> {
    habit::create(conn, clock, input)
}

pub fn update_habit(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: HabitInput,
) -> Result<Habit> {
    habit::update(conn, clock, id, input)
}

pub fn archive_habit(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<()> {
    habit::archive(conn, clock, id)
}

pub fn habits_today(
    conn: &Connection,
    clock: &dyn Clock,
    day: Option<String>,
) -> Result<Vec<HabitToday>> {
    habit::today(conn, clock, day)
}

pub fn complete_habit(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    day: &str,
    count: Option<i64>,
    note: Option<String>,
) -> Result<HabitEntry> {
    habit::complete(conn, clock, id, day, count, note)
}

pub fn skip_habit(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    day: &str,
    note: Option<String>,
) -> Result<HabitEntry> {
    habit::skip(conn, clock, id, day, note)
}

pub fn reopen_habit(conn: &mut Connection, id: &str, day: &str) -> Result<()> {
    habit::reopen(conn, id, day)
}

pub fn habit_history(conn: &Connection, id: &str, from: &str, to: &str) -> Result<Vec<HabitEntry>> {
    habit::history(conn, id, from, to)
}

pub fn habit_stats(
    conn: &Connection,
    clock: &dyn Clock,
    id: &str,
    day: Option<String>,
) -> Result<HabitStats> {
    habit::stats(conn, clock, id, day)
}

pub fn try_items(conn: &Connection) -> Result<Vec<TryItem>> {
    try_shelf::list(conn)
}

pub fn create_try_item(
    conn: &mut Connection,
    clock: &dyn Clock,
    input: TryItemInput,
) -> Result<TryItem> {
    try_shelf::create(conn, clock, input)
}

pub fn update_try_item(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: TryItemInput,
) -> Result<TryItem> {
    try_shelf::update(conn, clock, id, input)
}

pub fn schedule_try_item(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    day: &str,
    day_part: TimeOfDay,
) -> Result<TryItem> {
    try_shelf::schedule(conn, clock, id, day, day_part)
}

pub fn unschedule_try_item(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<TryItem> {
    try_shelf::unschedule(conn, clock, id)
}

pub fn mark_try_item_tried(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    rating: Option<i64>,
    reflection: Option<String>,
) -> Result<TryItem> {
    try_shelf::mark_tried(conn, clock, id, rating, reflection)
}

pub fn skip_try_item(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<TryItem> {
    try_shelf::skip_item(conn, clock, id)
}

pub fn set_try_item_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    task_id: Option<String>,
) -> Result<TryItem> {
    try_shelf::set_linked_task(conn, clock, id, task_id)
}

fn task_time_today(conn: &Connection, task_id: &str, day: &str) -> Result<(i64, bool)> {
    let (count, secs): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(accumulated_secs), 0) \
         FROM sessions WHERE task_id = ?1 AND day = ?2",
        rusqlite::params![task_id, day],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(((secs + 59) / 60, count > 0))
}

pub fn do_view(conn: &Connection, clock: &dyn Clock) -> Result<DoView> {
    let today = db::today(conn, clock)?;
    let day = format_date(today);
    let planned = db::list_planned(conn, &day)?;
    let active = db::active_session(conn, clock)?;
    let mut ordered = planned.clone();
    ordered.sort_by(|a, b| {
        ranking::slot_rank(a.slot.unwrap_or(Slot::Extra))
            .cmp(&ranking::slot_rank(b.slot.unwrap_or(Slot::Extra)))
            .then_with(|| a.slot_pos.unwrap_or(0).cmp(&b.slot_pos.unwrap_or(0)))
            .then_with(|| ranking::compare_tasks(a, b, today))
    });
    let current_id = active
        .as_ref()
        .and_then(|value| value.task_id.clone())
        .filter(|id| ordered.iter().any(|task| &task.id == id))
        .or_else(|| ordered.first().map(|task| task.id.clone()));
    let current = match current_id.as_ref() {
        Some(id) => ordered
            .iter()
            .find(|task| &task.id == id)
            .map(|task| {
                let (logged_today_mins, has_session_today) = task_time_today(conn, &task.id, &day)?;
                Ok::<CurrentTask, AppError>(CurrentTask {
                    task: task.clone(),
                    slot: task.slot.unwrap_or(Slot::Extra),
                    logged_today_mins,
                    has_session_today,
                })
            })
            .transpose()?,
        None => None,
    };
    let others: Vec<CurrentTask> = ordered
        .iter()
        .filter(|task| Some(&task.id) != current_id.as_ref())
        .map(|task| {
            let (logged_today_mins, has_session_today) = task_time_today(conn, &task.id, &day)?;
            Ok(CurrentTask {
                task: task.clone(),
                slot: task.slot.unwrap_or(Slot::Extra),
                logged_today_mins,
                has_session_today,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let done_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE status = 'DONE' AND substr(completed_at, 1, 10) = ?1",
        rusqlite::params![day],
        |row| row.get(0),
    )?;
    let day_row = db::day_or_default(conn, &day)?;
    let state = if planned.is_empty() {
        DoState::AllDone
    } else {
        DoState::Working
    };
    Ok(DoView {
        state,
        current,
        remaining_count: planned.len() as i64,
        done_count,
        session: active.map(|value| session::view(&value)),
        others,
        win_note: day_row.win_note,
    })
}

pub fn wrapup_view(conn: &Connection, clock: &dyn Clock) -> Result<WrapupView> {
    wrapup::view(conn, clock)
}

pub fn stats_view(conn: &Connection, clock: &dyn Clock) -> Result<Stats> {
    review::stats(conn, clock)
}

pub fn mode(conn: &Connection, clock: &dyn Clock) -> Result<Mode> {
    plan::mode(conn, clock)
}

pub fn default_target(conn: &Connection, clock: &dyn Clock) -> Result<String> {
    plan::default_target(conn, clock)
}

pub fn capture_task(conn: &mut Connection, clock: &dyn Clock, title: &str) -> Result<Task> {
    db::with_tx(conn, |tx| clarify::capture(tx, clock, title))
}

pub fn add_task(conn: &mut Connection, clock: &dyn Clock, input: QuickAddInput) -> Result<Task> {
    db::with_tx(conn, |tx| {
        let captured = clarify::capture(tx, clock, &input.title)?;
        let first_step = if captured.title.len() >= 3 {
            captured.title.clone()
        } else {
            "Start".to_string()
        };
        let ready = clarify::prepare_ready(
            tx,
            clock,
            &captured.id,
            ClarifyInput {
                title: captured.title.clone(),
                first_step: Some(first_step),
                done_when: None,
                estimate_mins: Some(input.estimate_mins),
                value: input.value,
                deadline_on: None,
            },
        )?;
        plan::assign_quick(tx, clock, &input.day, &ready.id, input.slot)
    })
}

pub fn clarify_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: ClarifyInput,
    if_full: Option<IfFull>,
    override_reason: Option<String>,
) -> Result<Task> {
    db::with_tx(conn, |tx| {
        clarify::clarify(tx, clock, id, input, if_full, override_reason)
    })
}

pub fn reactivate_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: Option<ReactivateInput>,
    if_full: Option<IfFull>,
) -> Result<Task> {
    db::with_tx(conn, |tx| {
        clarify::reactivate(tx, clock, id, input, if_full)
    })
}

pub fn split_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    children: Vec<SplitChild>,
) -> Result<Vec<Task>> {
    db::with_tx(conn, |tx| clarify::split(tx, clock, id, children))
}

pub fn update_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    patch: TaskPatch,
) -> Result<Task> {
    db::with_tx(conn, |tx| clarify::update(tx, clock, id, patch))
}

pub fn triage_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: TriageInput,
) -> Result<Task> {
    db::with_tx(conn, |tx| clarify::triage(tx, clock, id, input))
}

pub fn restore_task(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<Task> {
    db::with_tx(conn, |tx| clarify::restore(tx, clock, id))
}

pub fn set_pinned(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    pinned: bool,
) -> Result<Task> {
    db::with_tx(conn, |tx| clarify::set_pinned(tx, clock, id, pinned))
}

pub fn assign_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    id: &str,
    slot: Slot,
    override_reason: Option<String>,
) -> Result<Task> {
    db::with_tx(conn, |tx| {
        plan::assign(tx, clock, day, id, slot, override_reason)
    })
}

pub fn assign_task_quick(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    id: &str,
    slot: Slot,
) -> Result<Task> {
    db::with_tx(conn, |tx| plan::assign_quick(tx, clock, day, id, slot))
}

pub fn unassign_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    override_reason: Option<String>,
) -> Result<Task> {
    db::with_tx(conn, |tx| plan::unassign(tx, clock, id, override_reason))
}

pub fn swap_plan(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    out_id: &str,
    in_id: &str,
    reason: Option<String>,
) -> Result<()> {
    db::with_tx(conn, |tx| plan::swap(tx, clock, day, out_id, in_id, reason))
}

pub fn pull_extra(conn: &mut Connection, clock: &dyn Clock, id: &str) -> Result<Task> {
    db::with_tx(conn, |tx| plan::pull_extra(tx, clock, id))
}

pub fn lock_plan(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    override_reason: Option<String>,
) -> Result<LockResult> {
    db::with_tx(conn, |tx| plan::lock(tx, clock, day, override_reason))
}

pub fn suggest_plan(conn: &Connection, clock: &dyn Clock, day: &str) -> Result<PlanProposal> {
    plan::suggest(conn, clock, day)
}

pub fn apply_plan(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    assignments: &[PlanAssignment],
) -> Result<Vec<String>> {
    db::with_tx(conn, |tx| plan::apply(tx, clock, day, assignments))
}

pub fn complete_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    manual_mins: Option<i64>,
) -> Result<Task> {
    db::with_tx(conn, |tx| session::complete(tx, clock, id, manual_mins))
}

pub fn drop_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    reason: Option<String>,
) -> Result<Task> {
    db::with_tx(conn, |tx| session::drop_task(tx, clock, id, reason))
}

pub fn undo_last(conn: &mut Connection, clock: &dyn Clock) -> Result<Task> {
    db::with_tx(conn, |tx| review::undo_last(tx, clock))
}

pub fn resolve_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    input: ResolveInput,
) -> Result<Task> {
    db::with_tx(conn, |tx| wrapup::resolve(tx, clock, id, input))
}

pub fn start_session(
    conn: &mut Connection,
    clock: &dyn Clock,
    kind: SessionKind,
    task_id: Option<String>,
) -> Result<SessionView> {
    db::with_tx(conn, |tx| {
        session::start(tx, clock, kind, task_id.as_deref()).map(|value| session::view(&value))
    })
}

pub fn pause_session(conn: &mut Connection, clock: &dyn Clock) -> Result<SessionView> {
    db::with_tx(conn, |tx| {
        session::pause(tx, clock).map(|value| session::view(&value))
    })
}

pub fn resume_session(conn: &mut Connection, clock: &dyn Clock) -> Result<SessionView> {
    db::with_tx(conn, |tx| {
        session::resume(tx, clock).map(|value| session::view(&value))
    })
}

pub fn stop_session(
    conn: &mut Connection,
    clock: &dyn Clock,
    breadcrumb: Option<String>,
) -> Result<SessionView> {
    db::with_tx(conn, |tx| {
        session::stop(tx, clock, breadcrumb).map(|value| session::view(&value))
    })
}

pub fn extend_session(conn: &mut Connection, clock: &dyn Clock) -> Result<SessionView> {
    db::with_tx(conn, |tx| {
        session::extend(tx, clock).map(|value| session::view(&value))
    })
}

pub fn heartbeat(conn: &mut Connection, clock: &dyn Clock) -> Result<()> {
    db::with_tx(conn, |tx| session::heartbeat(tx, clock))
}

pub fn reconcile(conn: &mut Connection, clock: &dyn Clock) -> Result<ReconcileReport> {
    db::with_tx(conn, |tx| session::reconcile(tx, clock))
}

pub fn carry_all(conn: &mut Connection, clock: &dyn Clock) -> Result<Vec<Task>> {
    db::with_tx(conn, |tx| wrapup::carry_all(tx, clock))
}

pub fn complete_wrapup(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    win_note: Option<String>,
) -> Result<Day> {
    db::with_tx(conn, |tx| wrapup::complete(tx, clock, day, win_note))
}

pub fn reopen_wrapup(conn: &mut Connection, clock: &dyn Clock, day: Option<String>) -> Result<Day> {
    db::with_tx(conn, |tx| wrapup::reopen(tx, clock, day.as_deref()))
}

pub fn set_win_note(conn: &mut Connection, clock: &dyn Clock, note: Option<String>) -> Result<Day> {
    db::with_tx(conn, |tx| wrapup::set_win_note(tx, clock, note))
}

pub fn create_recurrence(
    conn: &mut Connection,
    clock: &dyn Clock,
    input: RecurrenceInput,
) -> Result<Recurrence> {
    db::with_tx(conn, |tx| recurrence::create(tx, clock, input))
}

pub fn update_recurrence(
    conn: &mut Connection,
    clock: &dyn Clock,
    id: &str,
    patch: RecurrencePatch,
) -> Result<Recurrence> {
    db::with_tx(conn, |tx| recurrence::update(tx, clock, id, patch))
}

pub fn delete_recurrence(conn: &mut Connection, id: &str) -> Result<()> {
    db::with_tx(conn, |tx| recurrence::delete_series(tx, id))
}

pub fn recurrences(conn: &Connection, clock: &dyn Clock) -> Result<Vec<RecurrenceView>> {
    recurrence::list_views(conn, clock)
}

pub fn mark_weekly_review(conn: &mut Connection, clock: &dyn Clock) -> Result<()> {
    db::with_tx(conn, |tx| review::mark_weekly_review(tx, clock))
}

pub fn export_json(conn: &Connection, clock: &dyn Clock) -> Result<serde_json::Value> {
    crate::export::export_json(conn, clock)
}

pub fn backup_to(conn: &Connection, path: &Path) -> Result<()> {
    crate::export::backup(conn, path)
}

pub fn validate_database(path: &Path) -> Result<()> {
    crate::export::validate_database(path)
}

pub fn budget(conn: &Connection, day: &str) -> Result<crate::model::BudgetView> {
    calibration::day_budget(conn, day)
}

pub fn parse_day(value: &str) -> Result<chrono::NaiveDate> {
    crate::clock::parse_date(value)
        .ok_or_else(|| AppError::hard(ErrorCode::InvalidDay, "invalid day"))
}

pub fn clean_estimate(value: i64) -> Result<i64> {
    validate::estimate(value)
}

pub fn note(conn: &Connection, scope: &str, period_start: &str) -> Result<Note> {
    notes::get(conn, scope, period_start)
}

pub fn save_note(
    conn: &mut Connection,
    clock: &dyn Clock,
    scope: &str,
    period_start: &str,
    body_markdown: &str,
) -> Result<Note> {
    notes::save(conn, clock, scope, period_start, body_markdown)
}

pub fn dashboard_view(
    conn: &Connection,
    clock: &dyn Clock,
    range: &str,
    date: Option<String>,
) -> Result<DashboardView> {
    dashboard::view(conn, clock, range, date)
}

pub fn reorder_task(
    conn: &mut Connection,
    clock: &dyn Clock,
    day: &str,
    id: &str,
    target_slot: Slot,
    target_index: i64,
) -> Result<()> {
    reorder::task(conn, clock, day, id, target_slot, target_index)
}

pub fn reorder_goal(
    conn: &mut Connection,
    kind: HorizonKind,
    period_start: &str,
    id: &str,
    target_band: Band,
    target_index: i64,
) -> Result<()> {
    reorder::goal(conn, kind, period_start, id, target_band, target_index)
}
