use chrono::NaiveDate;
use rusqlite::Transaction;

use crate::clock::{add_days, format_date, Clock};
use crate::db;
use crate::error::{AppError, ErrorCode, Result};
use crate::model::{
    ClarifyInput, IfFull, ReactivateInput, SplitChild, Status, Task, TaskPatch, TriageInput,
    TriageTarget,
};
use crate::rules::{calibration, validate};

pub const SOMEDAY_REVIEW_DAYS: i64 = 14;

pub fn capture(tx: &Transaction<'_>, clock: &dyn Clock, title: &str) -> Result<Task> {
    let title = validate::required_title(title)?;
    let now = db::now(clock);
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        first_step: None,
        done_when: None,
        estimate_mins: None,
        status: Status::Inbox,
        value: 3,
        pinned: false,
        deadline_on: None,
        waiting_on: None,
        review_on: None,
        breadcrumb: None,
        cue: None,
        parent_id: None,
        closed_reason: None,
        rollover_count: 0,
        planned_for: None,
        slot: None,
        slot_pos: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        recurrence_id: None,
        occurrence_on: None,
    };
    db::insert_task_row(tx, &task)?;
    db::insert_event(tx, &task.created_at, Some(&task.id), "CREATED", None, None)?;
    Ok(task)
}

pub fn prepare_ready(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    input: ClarifyInput,
) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    if task.status != Status::Inbox {
        return Err(AppError::invalid_transition(
            "only inbox tasks can be clarified",
        ));
    }
    let title = validate::required_title(&input.title)?;
    let first_step = input
        .first_step
        .as_deref()
        .ok_or_else(|| AppError::missing("first step is required"))?;
    let first_step = validate::required_first_step(first_step)?;
    let estimate = input
        .estimate_mins
        .ok_or_else(|| AppError::missing("estimate is required"))?;
    let estimate = validate::estimate(estimate)?;
    let value = validate::value_scale(input.value.unwrap_or(3))?;
    let deadline = validate::date_optional(input.deadline_on)?;

    let before = serde_json::to_string(&task)?;
    task.title = title;
    task.first_step = Some(first_step);
    task.done_when = validate::optional_text(input.done_when);
    task.estimate_mins = Some(estimate);
    task.value = value;
    task.deadline_on = deadline;
    task.status = Status::Ready;
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(id),
        "CLARIFIED",
        None,
        Some(&before),
    )?;
    Ok(task)
}

fn boulder_gate(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    estimate: i64,
    override_reason: Option<String>,
) -> Result<()> {
    let multiplier = calibration::effective_multiplier(tx)?;
    if (estimate as f64 * multiplier).round() <= 120.0 {
        return Ok(());
    }
    let reason = validate::override_reason(override_reason);
    let Some(reason) = reason else {
        return Err(AppError::soft(
            ErrorCode::Boulder,
            "This task looks bigger than two hours once calibrated.",
        )
        .with_hint("Split it into smaller tasks, or keep it with a short reason."));
    };
    db::insert_event(
        tx,
        &db::now(clock),
        None,
        "OVERRIDE",
        Some(&format!("BOULDER: {reason}")),
        None,
    )?;
    Ok(())
}

fn ready_gate(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    task: &mut Task,
    if_full: Option<IfFull>,
    today: NaiveDate,
) -> Result<()> {
    if task.estimate_mins == Some(15) {
        return Ok(());
    }
    let settings = db::settings(tx)?;
    if db::ready_count(tx)? < settings.ready_cap {
        return Ok(());
    }
    if if_full == Some(IfFull::Someday) {
        task.status = Status::Someday;
        task.review_on = Some(format_date(add_days(today, SOMEDAY_REVIEW_DAYS)));
        task.updated_at = db::now(clock);
        return Ok(());
    }
    Err(AppError::soft(
        ErrorCode::ReadyFull,
        format!(
            "Ready is full ({} of {}).",
            db::ready_count(tx)?,
            settings.ready_cap
        ),
    )
    .with_hint("Send it to Someday, or finish something first."))
}

pub fn clarify(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    input: ClarifyInput,
    if_full: Option<IfFull>,
    override_reason: Option<String>,
) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    if task.status != Status::Inbox {
        return Err(AppError::invalid_transition(
            "only inbox tasks can be clarified",
        ));
    }
    let title = validate::required_title(&input.title)?;
    let first_step = input
        .first_step
        .as_deref()
        .ok_or_else(|| AppError::missing("first step is required"))?;
    let first_step = validate::required_first_step(first_step)?;
    let estimate = input
        .estimate_mins
        .ok_or_else(|| AppError::missing("estimate is required"))?;
    let estimate = validate::estimate(estimate)?;
    let value = validate::value_scale(input.value.unwrap_or(3))?;
    let deadline = validate::date_optional(input.deadline_on)?;
    boulder_gate(tx, clock, estimate, override_reason)?;

    let before = serde_json::to_string(&task)?;
    let today = db::today(tx, clock)?;
    task.title = title;
    task.first_step = Some(first_step);
    task.done_when = validate::optional_text(input.done_when);
    task.estimate_mins = Some(estimate);
    task.value = value;
    task.deadline_on = deadline;
    task.status = Status::Ready;
    task.updated_at = db::now(clock);
    ready_gate(tx, clock, &mut task, if_full, today)?;
    db::write_task_row(tx, &task)?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(id),
        "CLARIFIED",
        None,
        Some(&before),
    )?;
    Ok(task)
}

pub fn reactivate(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    input: Option<ReactivateInput>,
    if_full: Option<IfFull>,
) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    if !matches!(task.status, Status::Waiting | Status::Someday) {
        return Err(AppError::invalid_transition(
            "only waiting or someday tasks can be reactivated",
        ));
    }
    if let Some(input) = input {
        if let Some(first_step) = input.first_step {
            task.first_step = Some(validate::required_first_step(&first_step)?);
        }
        if let Some(done_when) = input.done_when {
            task.done_when = validate::optional_text(Some(done_when));
        }
        if let Some(estimate) = input.estimate_mins {
            task.estimate_mins = Some(validate::estimate(estimate)?);
        }
        if let Some(value) = input.value {
            task.value = validate::value_scale(value)?;
        }
        if let Some(deadline) = input.deadline_on {
            task.deadline_on = validate::date_optional(Some(deadline))?;
        }
    }
    if task.first_step.is_none() {
        return Err(AppError::missing("add a first step before reactivating"));
    }
    if task.estimate_mins.is_none() {
        return Err(AppError::missing("add an estimate before reactivating"));
    }
    let before = serde_json::to_string(&task)?;
    let today = db::today(tx, clock)?;
    task.status = Status::Ready;
    task.waiting_on = None;
    task.review_on = None;
    task.closed_reason = None;
    task.updated_at = db::now(clock);
    ready_gate(tx, clock, &mut task, if_full, today)?;
    db::write_task_row(tx, &task)?;
    db::insert_event(
        tx,
        &db::now(clock),
        Some(id),
        "REACTIVATED",
        None,
        Some(&before),
    )?;
    Ok(task)
}

pub fn split(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    children: Vec<SplitChild>,
) -> Result<Vec<Task>> {
    let mut task = db::require_task(tx, id)?;
    if task.status != Status::Ready {
        return Err(AppError::invalid_transition(
            "only ready tasks can be split",
        ));
    }
    if !(2..=5).contains(&children.len()) {
        return Err(AppError::missing("split needs two to five children"));
    }
    let now = db::now(clock);
    let before = serde_json::to_string(&task)?;
    let mut created = Vec::new();
    for child in children {
        let title = validate::required_title(&child.title)?;
        let first_step = validate::required_first_step(&child.first_step)?;
        let estimate = validate::estimate(child.estimate_mins)?;
        let child_task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            first_step: Some(first_step),
            done_when: None,
            estimate_mins: Some(estimate),
            status: Status::Ready,
            value: task.value,
            pinned: false,
            deadline_on: task.deadline_on.clone(),
            waiting_on: None,
            review_on: None,
            breadcrumb: None,
            cue: None,
            parent_id: Some(task.id.clone()),
            closed_reason: None,
            rollover_count: 0,
            planned_for: None,
            slot: None,
            slot_pos: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: None,
            recurrence_id: None,
            occurrence_on: None,
        };
        db::insert_task_row(tx, &child_task)?;
        created.push(child_task);
    }
    task.status = Status::Dropped;
    task.closed_reason = Some(crate::model::ClosedReason::Split);
    task.planned_for = None;
    task.slot = None;
    task.slot_pos = None;
    task.updated_at = now.clone();
    db::write_task_row(tx, &task)?;
    if let Some(recurrence_id) = &task.recurrence_id {
        tx.execute(
            "UPDATE recurrences SET active = 0, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![recurrence_id, now],
        )?;
    }
    let child_ids = created
        .iter()
        .map(|child| child.id.clone())
        .collect::<Vec<_>>()
        .join(",");
    db::insert_event(tx, &now, Some(id), "SPLIT", Some(&child_ids), Some(&before))?;
    Ok(created)
}

pub fn triage(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    input: TriageInput,
) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    if task.recurrence_id.is_some() {
        return Err(AppError::invalid_transition(
            "recurring occurrences cannot be triaged",
        ));
    }
    let now = db::now(clock);
    let before = serde_json::to_string(&task)?;
    match input.to {
        TriageTarget::Waiting => {
            if !matches!(task.status, Status::Inbox | Status::Ready | Status::Someday) {
                return Err(AppError::invalid_transition(
                    "that task cannot move to waiting",
                ));
            }
            let waiting_on = validate::optional_text(input.waiting_on)
                .ok_or_else(|| AppError::missing("waiting tasks need who or what they wait on"))?;
            let review_on = validate::date_optional(input.review_on)?
                .ok_or_else(|| AppError::missing("waiting tasks need a review date"))?;
            task.status = Status::Waiting;
            task.waiting_on = Some(waiting_on);
            task.review_on = Some(review_on);
            task.planned_for = None;
            task.slot = None;
            task.slot_pos = None;
            task.updated_at = now.clone();
            db::write_task_row(tx, &task)?;
            db::insert_event(
                tx,
                &now,
                Some(id),
                "WAITING",
                task.waiting_on.as_deref(),
                Some(&before),
            )?;
        }
        TriageTarget::Someday => {
            if !matches!(task.status, Status::Inbox | Status::Ready | Status::Waiting) {
                return Err(AppError::invalid_transition(
                    "that task cannot move to someday",
                ));
            }
            let review_on = validate::date_optional(input.review_on)?
                .ok_or_else(|| AppError::missing("someday tasks need a review date"))?;
            task.status = Status::Someday;
            task.review_on = Some(review_on);
            task.waiting_on = None;
            task.planned_for = None;
            task.slot = None;
            task.slot_pos = None;
            task.updated_at = now.clone();
            db::write_task_row(tx, &task)?;
            db::insert_event(tx, &now, Some(id), "SOMEDAY", None, Some(&before))?;
        }
        TriageTarget::Dropped => {
            if !matches!(
                task.status,
                Status::Inbox | Status::Ready | Status::Waiting | Status::Someday
            ) {
                return Err(AppError::invalid_transition(
                    "that task cannot be dropped from triage",
                ));
            }
            task.status = Status::Dropped;
            task.closed_reason = Some(crate::model::ClosedReason::NotDoing);
            task.planned_for = None;
            task.slot = None;
            task.slot_pos = None;
            task.updated_at = now.clone();
            db::write_task_row(tx, &task)?;
            db::insert_event(
                tx,
                &now,
                Some(id),
                "DROPPED",
                task.breadcrumb.as_deref(),
                Some(&before),
            )?;
        }
    }
    Ok(task)
}

pub fn restore(tx: &Transaction<'_>, clock: &dyn Clock, id: &str) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    if task.status != Status::Dropped
        || task.closed_reason != Some(crate::model::ClosedReason::NotDoing)
    {
        return Err(AppError::invalid_transition(
            "only tasks dropped as not doing can be restored",
        ));
    }
    task.status = Status::Inbox;
    task.closed_reason = None;
    task.completed_at = None;
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    Ok(task)
}

pub fn update(tx: &Transaction<'_>, clock: &dyn Clock, id: &str, patch: TaskPatch) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    if matches!(task.status, Status::Done | Status::Dropped) {
        return Err(AppError::invalid_transition(
            "closed tasks cannot be edited",
        ));
    }
    if let Some(title) = patch.title {
        task.title = validate::required_title(&title)?;
    }
    if let Some(first_step) = patch.first_step {
        task.first_step = match first_step {
            Some(value) => Some(validate::required_first_step(&value)?),
            None => None,
        };
    }
    if let Some(done_when) = patch.done_when {
        task.done_when = validate::optional_text(done_when);
    }
    if let Some(estimate) = patch.estimate_mins {
        task.estimate_mins = Some(validate::estimate(estimate)?);
    }
    if let Some(value) = patch.value {
        task.value = validate::value_scale(value)?;
    }
    if let Some(deadline) = patch.deadline_on {
        task.deadline_on = validate::date_optional(deadline)?;
    }
    if let Some(cue) = patch.cue {
        task.cue = validate::optional_text(cue);
    }
    if let Some(breadcrumb) = patch.breadcrumb {
        task.breadcrumb = validate::optional_text(breadcrumb);
    }
    if let Some(waiting_on) = patch.waiting_on {
        task.waiting_on = validate::optional_text(waiting_on);
    }
    if let Some(review_on) = patch.review_on {
        task.review_on = validate::date_optional(review_on)?;
    }
    if task.status == Status::Ready {
        if task.first_step.is_none() {
            return Err(AppError::missing("ready tasks need a first step"));
        }
        if task.estimate_mins.is_none() {
            return Err(AppError::missing("ready tasks need an estimate"));
        }
    }
    if task.slot == Some(crate::model::Slot::Batch) && task.estimate_mins != Some(15) {
        return Err(AppError::hard(
            ErrorCode::BatchRequiresMicro,
            "batch tasks must be 15 minutes",
        ));
    }
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    Ok(task)
}

pub fn set_pinned(tx: &Transaction<'_>, clock: &dyn Clock, id: &str, pinned: bool) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    task.pinned = pinned;
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    Ok(task)
}

pub fn set_cue(
    tx: &Transaction<'_>,
    clock: &dyn Clock,
    id: &str,
    cue: Option<String>,
) -> Result<Task> {
    let mut task = db::require_task(tx, id)?;
    task.cue = validate::optional_text(cue);
    task.updated_at = db::now(clock);
    db::write_task_row(tx, &task)?;
    Ok(task)
}
