use std::cmp::Ordering;

use crate::task::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartFilter {
    Inbox,
    Today,
    Scheduled,
    Upcoming,
    Done,
}

pub fn filter_name<'a>(filter: SmartFilter, tasks: &'a [Task], today: &str) -> Vec<&'a Task> {
    tasks
        .iter()
        .filter(|task| match filter {
            SmartFilter::Inbox => is_open(task) && !has_any_date(task),
            SmartFilter::Today => is_open(task) && is_today_task(task, today),
            SmartFilter::Scheduled => is_open(task) && task.tags.contains_key("scheduled"),
            SmartFilter::Upcoming => {
                is_open(task)
                    && task.tags.iter().any(|(key, value)| {
                        matches!(key.as_str(), "due" | "scheduled" | "starting")
                            && value.as_str() > today
                    })
            }
            SmartFilter::Done => task.done,
        })
        .collect()
}

pub fn sort_tasks(tasks: &mut [Task], today: &str) {
    tasks.sort_by(|left, right| {
        compare_done_status(left, right)
            .then_with(|| compare_done_completion_date(left, right))
            .then_with(|| actionable_rank(left, today).cmp(&actionable_rank(right, today)))
            .then_with(|| compare_priority(left.priority, right.priority))
            .then_with(|| compare_optional_date(left.tags.get("due"), right.tags.get("due")))
            .then_with(|| {
                compare_optional_date(left.tags.get("scheduled"), right.tags.get("scheduled"))
            })
            .then_with(|| {
                compare_optional_date(left.creation_date.as_ref(), right.creation_date.as_ref())
            })
            .then_with(|| left.raw.cmp(&right.raw))
    });
}

fn is_open(task: &Task) -> bool {
    !task.done
}

fn has_any_date(task: &Task) -> bool {
    task.tags.contains_key("due")
        || task.tags.contains_key("scheduled")
        || task.tags.contains_key("starting")
}

fn is_today_task(task: &Task, today: &str) -> bool {
    task.tags.iter().any(|(key, value)| {
        matches!(key.as_str(), "due" | "scheduled")
            && (value.as_str() == today || (value.as_str() < today && is_actionable(task, today)))
    })
}

fn is_actionable(task: &Task, today: &str) -> bool {
    !matches!(task.tags.get("starting"), Some(starting) if starting.as_str() > today)
}

fn actionable_rank(task: &Task, today: &str) -> u8 {
    if is_actionable(task, today) { 0 } else { 1 }
}

fn compare_done_status(left: &Task, right: &Task) -> Ordering {
    match (left.done, right.done) {
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn compare_done_completion_date(left: &Task, right: &Task) -> Ordering {
    match (left.done, right.done) {
        (true, true) => match (
            left.completion_date.as_ref(),
            right.completion_date.as_ref(),
        ) {
            (Some(left), Some(right)) => right.cmp(left),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        _ => Ordering::Equal,
    }
}

fn compare_priority(left: Option<char>, right: Option<char>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_optional_date(left: Option<&String>, right: Option<&String>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
