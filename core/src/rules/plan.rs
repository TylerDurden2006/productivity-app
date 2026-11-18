use std::collections::HashSet;

use chrono::NaiveDate;
use rusqlite::{params, Connection, Transaction};

use crate::clock::{add_days, format_date, parse_date, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    Banners, BudgetView, DayMins, LockResult, Mode, PlanAssignment, PlanProposal, PlanSlots,
    PlanView, Slot, Status, Task,
};
use crate::rules::{calibration, ranking, recurrence, validate};

fn target_date(conn: &Connection, clock: &dyn Clock, day: &str) -> Result<NaiveDate> {
    let today = db::today(conn, clock)?;
    let parsed =
        parse_date(day).ok_or_else(|| AppError::hard(ErrorCode::InvalidDay, "invalid day"))?;
    if parsed != today && parsed != add_days(today, 1) {
        return Err(AppError::hard(
            ErrorCode::InvalidDay,
            "you can only plan today or tomorrow",
        ));
    }
    Ok(parsed)
}

pub fn default_target(conn: &Connection, clock: &dyn Clock) -> Result<String> {
    let today = db::today(conn, clock)?;
    let day = db::day_or_default(conn, &format_date(today))?;
    if day.plan_locked_at.is_none() && day.wrapup_done_at.is_none() {
        Ok(format_date(today))
    } else {
        Ok(format_date(add_days(today, 1)))
    }
}

pub fn mode(conn: &Connection, clock: &dyn Clock) -> Result<Mode> {
    let today = db::today(conn, clock)?;
    let day = db::day_or_default(conn, &format_date(today))?;
    if day.plan_locked_at.is_some() && day.wrapup_done_at.is_none() {
        Ok(Mode::Do)
    } else {
        Ok(Mode::Plan)
    }
}

fn locked_reason(
    conn: &Connection,
    day: &str,
    override_reason: Option<String>,
) -> Result<Option<(String, bool)>> {
    let row = db::day_or_default(conn, day)?;
    if row.plan_locked_at.is_none() {
        return Ok(validate::override_reason(override_reason).map(|reason| (reason, false)));
    }
    match validate::override_reason(override_reason) {
        Some(reason) => Ok(Some((reason, true))),
        None => Err(
            AppError::soft(ErrorCode::PlanLocked, "today's plan is locked.")
                .with_hint("Give a short reason to change a locked plan."),
        ),
    }
}

fn lowest_free(planned: &[Task], slot: Slot) -> i64 {
    if slot == Slot::Extra {
        let max = planned
            .iter()
            .filter(|task| task.slot == Some(Slot::Extra) && task.status == Status::Ready)
            .filter_map(|task| task.slot_pos)
            .max()
            .unwrap_or(0);
        return max + 1;
    }
    let used: HashSet<i64> = planned
        .iter()
        .filter(|task| task.slot == Some(slot) && task.status == Status::Ready)
        .filter_map(|task| task.slot_pos)
        .collect();
    for pos in 1..=slot.capacity() {
        if !used.contains(&pos) {
            return pos;
        }
    }
    slot.capacity() + 1
}

fn write_override(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    reason: Option<&str>,
    gate: &str,
) -> Result<()> {
    if let Some(reason) = reason {
        db::insert_event(
            tx,
            &db::now(clock),
            None,
            "OVERRIDE",
            Some(&format!("{gate}: {reason}")),
            None,
        )?;
    }
    Ok(())
}

pub fn assign(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    day: &str,
    task_id: &str,
    slot: Slot,
    override_reason: Option<String>,
) -> Result<Task> {
    let target = target_date(tx, clock, day)?;
    let day = format_date(target);
    db::ensure_day(tx, &day)?;
    let locked = locked_reason(tx, &day, override_reason)?;
    let reason = locked.as_ref().map(|(reason, _)| reason.clone());
    let mut overrode = locked.as_ref().map(|(_, used)| *used).unwrap_or(false);

    let mut task = db::require_task(tx, task_id)?;
    if task.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "only ready tasks can be planned",
        ));
    }
    if task.recurrence_id.is_some() {
        return Err(AppError::invalid_transition(
            "recurring tasks stay in the due strip",
        ));
    }
    if task.planned_for.is_some() {
        return Err(AppError::invalid_transition("task is already planned"));
    }
    let planned = db::list_planned(tx, &day)?;
    let all_planned = db::list_plan_all(tx, &day)?;
    let count = all_planned
        .iter()
        .filter(|item| item.slot == Some(slot) && item.status == Status::Ready)
        .count();
    if count as i64 >= slot.capacity() {
        return Err(AppError::hard(
            ErrorCode::SlotFull,
            "that slot is already full",
        ));
    }
    if task.rollover_count >= 3 {
        if reason.is_none() {
            return Err(AppError::soft(
                ErrorCode::StuckTask,
                "this task has rolled over three times.",
            )
            .with_hint("Shrink it, or plan it with a short reason."));
        }
        overrode = true;
    }

    let multiplier = calibration::multiplier_for_day(tx, &day)?;
    let mut after = planned.clone();
    let mut candidate = task.clone();
    candidate.slot = Some(slot);
    after.push(candidate);
    let planned_mins = calibration::plan_minutes(&after, multiplier);
    if planned_mins > calibration::budget_mins(tx, &day)? {
        if reason.is_none() {
            return Err(AppError::soft(
                ErrorCode::BudgetExceeded,
                format!(
                    "That would plan {planned_mins} minutes against a {} minute budget.",
                    calibration::budget_mins(tx, &day)?
                ),
            ));
        }
        overrode = true;
    }

    let before = serde_json::to_string(&task)?;
    let pos = lowest_free(&all_planned, slot);
    task.planned_for = Some(day.clone());
    task.slot = Some(slot);
    task.slot_pos = Some(pos);
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    tx.execute(
        "UPDATE tasks SET first_planned_for = COALESCE(first_planned_for, ?2) WHERE id = ?1",
        rusqlite::params![task_id, day],
    )?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(task_id),
        "PLANNED",
        Some(&day),
        Some(&before),
    )?;
    if overrode {
        write_override(tx, clock, reason.as_deref(), "PLAN_GATE")?;
    }
    Ok(task)
}

fn quick_slots(requested: Slot) -> Vec<Slot> {
    match requested {
        Slot::One => vec![Slot::One, Slot::Three, Slot::Batch, Slot::Extra],
        Slot::Three => vec![Slot::Three, Slot::Batch, Slot::Extra],
        Slot::Batch => vec![Slot::Batch, Slot::Extra],
        Slot::Extra => vec![Slot::Extra],
    }
}

pub fn assign_quick(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    day: &str,
    task_id: &str,
    requested_slot: Slot,
) -> Result<Task> {
    let target = target_date(tx, clock, day)?;
    let day = format_date(target);
    db::ensure_day(tx, &day)?;

    let mut task = db::require_task(tx, task_id)?;
    if task.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "only ready tasks can be planned",
        ));
    }
    if task.recurrence_id.is_some() {
        return Err(AppError::invalid_transition(
            "recurring tasks stay in the due strip",
        ));
    }
    if task.planned_for.is_some() {
        return Err(AppError::invalid_transition("task is already planned"));
    }

    let all_planned = db::list_plan_all(tx, &day)?;
    let mut slot = None;
    for candidate in quick_slots(requested_slot) {
        let count = all_planned
            .iter()
            .filter(|item| item.slot == Some(candidate) && item.status == Status::Ready)
            .count();
        if (count as i64) < candidate.capacity() {
            slot = Some(candidate);
            break;
        }
    }
    let slot = slot.ok_or_else(|| AppError::hard(ErrorCode::SlotFull, "that slot is full"))?;

    let before = serde_json::to_string(&task)?;
    let pos = lowest_free(&all_planned, slot);
    task.planned_for = Some(day.clone());
    task.slot = Some(slot);
    task.slot_pos = Some(pos);
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    tx.execute(
        "UPDATE tasks SET first_planned_for = COALESCE(first_planned_for, ?2) WHERE id = ?1",
        params![task_id, day],
    )?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(task_id),
        "PLANNED",
        Some(&day),
        Some(&before),
    )?;
    Ok(task)
}

pub fn unassign(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    task_id: &str,
    override_reason: Option<String>,
) -> Result<Task> {
    let mut task = db::require_task(tx, task_id)?;
    let Some(day) = task.planned_for.clone() else {
        return Err(AppError::invalid_transition("task is not planned"));
    };
    if task.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "only open planned tasks can be unassigned",
        ));
    }
    let locked = locked_reason(tx, &day, override_reason)?;
    let before = serde_json::to_string(&task)?;
    task.planned_for = None;
    task.slot = None;
    task.slot_pos = None;
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(task_id),
        "UNPLANNED",
        Some(&day),
        Some(&before),
    )?;
    if let Some((reason, true)) = locked {
        write_override(tx, clock, Some(&reason), "PLAN_LOCKED")?;
    }
    Ok(task)
}

pub fn swap(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    day: &str,
    out_id: &str,
    in_id: &str,
    reason: Option<String>,
) -> Result<()> {
    let target = target_date(tx, clock, day)?;
    let day = format_date(target);
    db::ensure_day(tx, &day)?;
    let mut out = db::require_task(tx, out_id)?;
    let mut incoming = db::require_task(tx, in_id)?;
    if out.planned_for.as_deref() != Some(day.as_str()) || out.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "the outgoing task is not open on that day",
        ));
    }
    if incoming.status != Status::Ready || incoming.planned_for.is_some() {
        return Err(AppError::invalid_transition(
            "the incoming task is not an unplanned ready task",
        ));
    }
    if incoming.recurrence_id.is_some() {
        return Err(AppError::invalid_transition(
            "recurring tasks stay in the due strip",
        ));
    }
    let locked = locked_reason(tx, &day, reason.clone())?;
    let clean_reason = locked.as_ref().map(|(value, _)| value.clone());
    let mut overrode = locked.as_ref().map(|(_, used)| *used).unwrap_or(false);
    if incoming.rollover_count >= 3 {
        if clean_reason.is_none() {
            return Err(AppError::soft(
                ErrorCode::StuckTask,
                "this task has rolled over three times.",
            ));
        }
        overrode = true;
    }
    let Some(slot) = out.slot else {
        return Err(AppError::invalid_transition("task is not planned"));
    };
    let pos = out.slot_pos.unwrap_or(1);
    let planned = db::list_planned(tx, &day)?;
    let multiplier = calibration::multiplier_for_day(tx, &day)?;
    let mut after: Vec<Task> = planned
        .into_iter()
        .filter(|task| task.id != out.id)
        .collect();
    let mut candidate = incoming.clone();
    candidate.status = Status::Ready;
    candidate.slot = Some(slot);
    candidate.slot_pos = Some(pos);
    after.push(candidate);
    let planned_mins = calibration::plan_minutes(&after, multiplier);
    if planned_mins > calibration::budget_mins(tx, &day)? {
        if clean_reason.is_none() {
            return Err(AppError::soft(
                ErrorCode::BudgetExceeded,
                "that swap would go over budget",
            ));
        }
        overrode = true;
    }
    let before_out = serde_json::to_string(&out)?;
    let before_in = serde_json::to_string(&incoming)?;
    let now = db::now(clock);
    out.planned_for = None;
    out.slot = None;
    out.slot_pos = None;
    out.updated_at = now.clone();
    db::write_task_row(tx, &out)?;
    incoming.planned_for = Some(day.clone());
    incoming.slot = Some(slot);
    incoming.slot_pos = Some(pos);
    incoming.updated_at = now.clone();
    db::write_task_row(tx, &incoming)?;
    db::insert_event(
        tx,
        &now,
        Some(out_id),
        "SWAPPED",
        Some(in_id),
        Some(&before_out),
    )?;
    db::insert_event(
        tx,
        &now,
        Some(in_id),
        "SWAPPED",
        Some(out_id),
        Some(&before_in),
    )?;
    if overrode {
        write_override(tx, clock, clean_reason.as_deref(), "PLAN_GATE")?;
    }
    Ok(())
}

pub fn pull_extra(tx: &Transaction<'_>, clock: &dyn Clock, task_id: &str) -> Result<Task> {
    let today = db::today(tx, clock)?;
    let day = format_date(today);
    db::ensure_day(tx, &day)?;
    let mut task = db::require_task(tx, task_id)?;
    if task.status != Status::Ready || task.recurrence_id.is_some() {
        return Err(AppError::invalid_transition(
            "only an unplanned ready task can be pulled in",
        ));
    }
    if task.planned_for.is_some() {
        return Err(AppError::invalid_transition("task is already planned"));
    }
    let planned = db::list_plan_all(tx, &day)?;
    let before = serde_json::to_string(&task)?;
    let pos = lowest_free(&planned, Slot::Extra);
    task.planned_for = Some(day.clone());
    task.slot = Some(Slot::Extra);
    task.slot_pos = Some(pos);
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    tx.execute(
        "UPDATE tasks SET first_planned_for = COALESCE(first_planned_for, ?2) WHERE id = ?1",
        rusqlite::params![task_id, day],
    )?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(task_id),
        "PLANNED",
        Some(&day),
        Some(&before),
    )?;
    Ok(task)
}

pub fn lock(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    day: &str,
    override_reason: Option<String>,
) -> Result<LockResult> {
    let target = target_date(tx, clock, day)?;
    let day = format_date(target);
    db::ensure_day(tx, &day)?;
    let existing = db::day_or_default(tx, &day)?;
    if existing.plan_locked_at.is_some() {
        return Err(AppError::soft(
            ErrorCode::PlanLocked,
            "that plan is already locked",
        ));
    }
    let planned = db::list_planned(tx, &day)?;
    if planned.is_empty() {
        return Err(AppError::invalid_transition(
            "plan at least one task before locking",
        ));
    }
    let multiplier = calibration::effective_multiplier(tx)?;
    let settings = db::settings(tx)?;
    let budget = settings.daily_focus_budget_mins;
    let planned_mins = calibration::plan_minutes(&planned, multiplier);
    let reason = validate::override_reason(override_reason);
    let mut overrode = false;
    if planned_mins > budget {
        if reason.is_none() {
            return Err(AppError::soft(
                ErrorCode::BudgetExceeded,
                format!("The plan is over budget ({planned_mins} against {budget} minutes)."),
            ));
        }
        overrode = true;
    }
    let now = db::now(clock);
    tx.execute(
        "UPDATE days SET plan_locked_at = ?2, multiplier_at_lock = ?3, budget_at_lock = ?4 \
         WHERE day = ?1",
        params![day, now, multiplier, budget],
    )?;
    db::insert_event(tx, &now, None, "LOCKED", Some(&day), None)?;
    if overrode {
        write_override(tx, clock, reason.as_deref(), "BUDGET_EXCEEDED")?;
    }
    let no_one = !planned.iter().any(|task| task.slot == Some(Slot::One));
    Ok(LockResult {
        locked_at: now,
        no_one,
    })
}

pub fn suggest(conn: &Connection, clock: &dyn Clock, day: &str) -> Result<PlanProposal> {
    let target = target_date(conn, clock, day)?;
    let day = format_date(target);
    let row = db::day_or_default(conn, &day)?;
    if row.plan_locked_at.is_some() {
        return Err(AppError::soft(ErrorCode::PlanLocked, "that plan is locked"));
    }
    let multiplier = calibration::effective_multiplier(conn)?;
    let settings = db::settings(conn)?;
    let budget = settings.daily_focus_budget_mins;
    let existing = db::list_planned(conn, &day)?;
    let mut assignments = Vec::new();
    let mut used: HashSet<String> = existing.iter().map(|task| task.id.clone()).collect();
    let mut working = existing.clone();

    let today = db::today(conn, clock)?;
    let mut deep = db::list_ready_candidates(conn)?;
    ranking::sort_deep(&mut deep, today);
    let mut quick = db::list_quick_candidates(conn)?;
    ranking::sort_quick(&mut quick);

    let one_taken = existing.iter().any(|task| task.slot == Some(Slot::One));
    if !one_taken {
        for task in deep.iter().filter(|task| task.rollover_count < 3) {
            let estimate = task.estimate_mins.unwrap_or(0);
            let calibrated = calibration::calibrated_estimate(estimate, multiplier);
            if calibrated > 120 {
                continue;
            }
            if calibration::plan_minutes(
                &[working.clone(), vec![task_with_slot(task, Slot::One)]].concat(),
                multiplier,
            ) > budget
            {
                break;
            }
            working.push(task_with_slot(task, Slot::One));
            used.insert(task.id.clone());
            assignments.push(PlanAssignment {
                id: task.id.clone(),
                slot: Slot::One,
            });
            break;
        }
    }

    let mut three_room = 3usize.saturating_sub(
        existing
            .iter()
            .filter(|task| task.slot == Some(Slot::Three))
            .count(),
    );
    for task in deep.iter().filter(|task| task.rollover_count < 3) {
        if three_room == 0 {
            break;
        }
        if used.contains(&task.id) {
            continue;
        }
        let next = [working.clone(), vec![task_with_slot(task, Slot::Three)]].concat();
        if calibration::plan_minutes(&next, multiplier) > budget {
            continue;
        }
        working.push(task_with_slot(task, Slot::Three));
        used.insert(task.id.clone());
        assignments.push(PlanAssignment {
            id: task.id.clone(),
            slot: Slot::Three,
        });
        three_room -= 1;
    }

    let batch_count = working
        .iter()
        .filter(|task| task.slot == Some(Slot::Batch))
        .count();
    let mut batch_room = 5usize.saturating_sub(batch_count);
    for task in quick.iter() {
        if batch_room == 0 {
            break;
        }
        if used.contains(&task.id) {
            continue;
        }
        let next = [working.clone(), vec![task_with_slot(task, Slot::Batch)]].concat();
        if calibration::plan_minutes(&next, multiplier) > budget {
            continue;
        }
        used.insert(task.id.clone());
        working.push(task_with_slot(task, Slot::Batch));
        assignments.push(PlanAssignment {
            id: task.id.clone(),
            slot: Slot::Batch,
        });
        batch_room -= 1;
    }

    let planned_mins = calibration::plan_minutes(&working, multiplier);
    Ok(PlanProposal {
        assignments,
        planned_mins,
        multiplier,
    })
}

fn task_with_slot(task: &Task, slot: Slot) -> Task {
    let mut clone = task.clone();
    clone.slot = Some(slot);
    clone
}

pub fn apply(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    day: &str,
    assignments: &[PlanAssignment],
) -> Result<Vec<String>> {
    let mut applied = Vec::new();
    for assignment in assignments {
        match assign(tx, clock, day, &assignment.id, assignment.slot, None) {
            Ok(task) => applied.push(task.id),
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::SlotFull
                        | ErrorCode::BudgetExceeded
                        | ErrorCode::StuckTask
                        | ErrorCode::InvalidTransition
                        | ErrorCode::BatchRequiresMicro
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(applied)
}

fn weekly_review_due(conn: &Connection, clock: &dyn Clock) -> Result<bool> {
    let settings = db::settings(conn)?;
    let Some(last) = settings.last_weekly_review_at else {
        return Ok(true);
    };
    let Some(last_at) = crate::clock::parse_rfc3339(&last) else {
        return Ok(true);
    };
    Ok(clock.now_utc() - last_at >= chrono::Duration::days(7))
}

pub fn week_minutes(conn: &Connection, clock: &dyn Clock) -> Result<Vec<DayMins>> {
    let today = db::today(conn, clock)?;
    let from = add_days(today, -13);
    let sessions = db::list_sessions_since(conn, &format_date(from), clock)?;
    let mut totals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for session in sessions {
        if session.kind == crate::model::SessionKind::Focus {
            let mins = (session.elapsed_secs + 59) / 60;
            *totals.entry(session.day).or_insert(0) += mins;
        }
    }
    Ok((0..14)
        .map(|offset| {
            let day = format_date(add_days(from, offset));
            let mins = totals.get(&day).copied().unwrap_or(0);
            DayMins { day, mins }
        })
        .collect())
}

pub fn view(conn: &Connection, clock: &dyn Clock, requested: Option<String>) -> Result<PlanView> {
    let today = db::today(conn, clock)?;
    let target = match requested {
        Some(day) => format_date(target_date(conn, clock, &day)?),
        None => default_target(conn, clock)?,
    };
    let all_planned = db::list_plan_all(conn, &target)?;
    let open_planned = db::list_planned(conn, &target)?;
    let mut one = None;
    let mut three = Vec::new();
    let mut batch = Vec::new();
    let mut extra = Vec::new();
    for task in &all_planned {
        match task.slot {
            // `one` is a single option, so prefer the open task when a
            // completed task also holds the Main slot for the day.
            Some(Slot::One) if one.is_none() || task.status == Status::Ready => {
                one = Some(task.clone());
            }
            Some(Slot::One) => {}
            Some(Slot::Three) => three.push(task.clone()),
            Some(Slot::Batch) => batch.push(task.clone()),
            Some(Slot::Extra) => extra.push(task.clone()),
            None => {}
        }
    }

    let mut deep_candidates = db::list_ready_candidates(conn)?;
    ranking::sort_deep(&mut deep_candidates, today);
    let mut quick_candidates = db::list_quick_candidates(conn)?;
    ranking::sort_quick(&mut quick_candidates);
    let day_row = db::day_or_default(conn, &target)?;
    let multiplier = calibration::multiplier_for_day(conn, &target)?;
    let planned_mins = calibration::plan_minutes(&open_planned, multiplier);
    let mut due_for_review = db::list_review_due(conn, &target)?;
    ranking::sort_deep(&mut due_for_review, today);
    let mut stuck = db::list_stuck(conn)?;
    ranking::sort_deep(&mut stuck, today);
    let mut recurring = db::list_due_recurring(conn, &target)?;
    ranking::sort_due(&mut recurring);
    let mut dropped = db::list_recently_dropped(conn, 20)?;
    dropped.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let mut inbox = db::list_inbox(conn)?;
    for task in &mut inbox {
        task.updated_at = task.updated_at.clone();
    }

    Ok(PlanView {
        target_day: target.clone(),
        locked: day_row.plan_locked_at.is_some(),
        deep_candidates,
        quick_candidates,
        inbox,
        plan: PlanSlots {
            one,
            three,
            batch,
            extra,
        },
        budget: BudgetView {
            planned_mins,
            budget_mins: calibration::budget_mins(conn, &target)?,
            multiplier,
        },
        ready_count: db::ready_count(conn)?,
        ready_cap: db::settings(conn)?.ready_cap,
        banners: Banners {
            due_for_review,
            stuck,
            weekly_review_due: weekly_review_due(conn, clock)?,
        },
        dropped,
        recurring,
        recurrences: recurrence::list_views(conn, clock)?,
    })
}
