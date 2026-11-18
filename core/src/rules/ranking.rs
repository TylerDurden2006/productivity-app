use std::cmp::Ordering;

use chrono::NaiveDate;

use crate::clock::{add_days, parse_date};
use crate::model::{Slot, Task};

fn urgency_tier(task: &Task, today: NaiveDate) -> i64 {
    let Some(deadline) = task.deadline_on.as_deref().and_then(parse_date) else {
        return 2;
    };
    if deadline <= add_days(today, 2) {
        0
    } else if deadline <= add_days(today, 14) {
        1
    } else {
        2
    }
}

pub fn compare_tasks(a: &Task, b: &Task, today: NaiveDate) -> Ordering {
    b.pinned
        .cmp(&a.pinned)
        .then_with(|| urgency_tier(a, today).cmp(&urgency_tier(b, today)))
        .then_with(|| b.value.cmp(&a.value))
        .then_with(|| {
            a.estimate_mins
                .unwrap_or(i64::MAX)
                .cmp(&b.estimate_mins.unwrap_or(i64::MAX))
        })
        .then_with(|| a.created_at.cmp(&b.created_at))
        .then_with(|| a.id.cmp(&b.id))
}

pub fn compare_quick(a: &Task, b: &Task) -> Ordering {
    b.pinned
        .cmp(&a.pinned)
        .then_with(|| match (&a.deadline_on, &b.deadline_on) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| a.created_at.cmp(&b.created_at))
        .then_with(|| a.id.cmp(&b.id))
}

pub fn sort_deep(tasks: &mut [Task], today: NaiveDate) {
    tasks.sort_by(|a, b| compare_tasks(a, b, today));
}

pub fn sort_quick(tasks: &mut [Task]) {
    tasks.sort_by(compare_quick);
}

pub fn sort_due(tasks: &mut [Task]) {
    tasks.sort_by(|a, b| {
        a.occurrence_on
            .cmp(&b.occurrence_on)
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub fn slot_rank(slot: Slot) -> i64 {
    match slot {
        Slot::One => 1,
        Slot::Three => 2,
        Slot::Batch => 3,
        Slot::Extra => 4,
    }
}
