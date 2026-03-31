use ttd::parser::parse_task_line;
use ttd::query::{SmartFilter, filter_name, sort_tasks};

#[test]
fn inbox_contains_only_unscheduled_tasks() {
    let tasks = vec![
        parse_task_line("Call Mom"),
        parse_task_line("Prepare report scheduled:2026-03-29"),
        parse_task_line("Pay rent due:2026-03-29"),
        parse_task_line("Plan trip starting:2026-03-29"),
        parse_task_line("x 2026-03-29 Archive documents"),
    ];

    let inbox = filter_name(SmartFilter::Inbox, &tasks, "2026-03-29");

    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].description, "Call Mom");
}

#[test]
fn today_includes_due_today_and_overdue_items() {
    let tasks = vec![
        parse_task_line("Submit taxes due:2026-03-29"),
        parse_task_line("Book flights due:2026-03-20"),
        parse_task_line("Read book due:2026-04-10"),
    ];

    let today = filter_name(SmartFilter::Today, &tasks, "2026-03-29");

    assert_eq!(today.len(), 2);
}

#[test]
fn today_includes_due_today_even_if_starting_is_in_future() {
    let tasks = vec![
        parse_task_line("Prepare launch starting:2026-03-30 due:2026-03-29"),
        parse_task_line("Ship package scheduled:2026-03-29"),
    ];

    let today = filter_name(SmartFilter::Today, &tasks, "2026-03-29");

    assert_eq!(today.len(), 2);
    assert_eq!(
        today[0].description,
        "Prepare launch starting:2026-03-30 due:2026-03-29"
    );
    assert_eq!(today[1].description, "Ship package scheduled:2026-03-29");
}

#[test]
fn today_excludes_overdue_items_blocked_by_future_starting_date() {
    let tasks = vec![
        parse_task_line("Prepare launch starting:2026-03-30 due:2026-03-20"),
        parse_task_line("Ship package due:2026-03-20"),
    ];

    let today = filter_name(SmartFilter::Today, &tasks, "2026-03-29");

    assert_eq!(today.len(), 1);
    assert_eq!(today[0].description, "Ship package due:2026-03-20");
}

#[test]
fn scheduled_contains_open_scheduled_tasks() {
    let tasks = vec![
        parse_task_line("Prepare report scheduled:2026-03-29"),
        parse_task_line("x 2026-03-29 2026-03-20 Finished plan scheduled:2026-03-25"),
        parse_task_line("Call Mom"),
    ];

    let scheduled = filter_name(SmartFilter::Scheduled, &tasks, "2026-03-29");

    assert_eq!(scheduled.len(), 1);
    assert_eq!(
        scheduled[0].description,
        "Prepare report scheduled:2026-03-29"
    );
}

#[test]
fn upcoming_includes_future_starting_scheduled_and_due_dates() {
    let tasks = vec![
        parse_task_line("Future start starting:2026-03-30"),
        parse_task_line("Future scheduled scheduled:2026-04-01"),
        parse_task_line("Future due due:2026-04-02"),
        parse_task_line("Available now due:2026-03-29"),
        parse_task_line("x 2026-03-29 Completed due:2026-04-05"),
    ];

    let upcoming = filter_name(SmartFilter::Upcoming, &tasks, "2026-03-29");

    assert_eq!(upcoming.len(), 3);
    assert_eq!(upcoming[0].description, "Future start starting:2026-03-30");
    assert_eq!(
        upcoming[1].description,
        "Future scheduled scheduled:2026-04-01"
    );
    assert_eq!(upcoming[2].description, "Future due due:2026-04-02");
}

#[test]
fn done_contains_only_completed_tasks() {
    let tasks = vec![
        parse_task_line("Call Mom"),
        parse_task_line("x 2026-03-29 File taxes"),
        parse_task_line("x 2026-03-28 2026-03-20 Archive receipts"),
    ];

    let done = filter_name(SmartFilter::Done, &tasks, "2026-03-29");

    assert_eq!(done.len(), 2);
    assert_eq!(done[0].description, "File taxes");
    assert_eq!(done[1].description, "Archive receipts");
}

#[test]
fn sorting_prefers_actionable_then_priority_then_due_date() {
    let mut tasks = vec![
        parse_task_line("(B) Call Mom due:2026-03-30"),
        parse_task_line("(A) File taxes due:2026-04-01"),
        parse_task_line("(A) Buy milk due:2026-03-29"),
        parse_task_line("(A) Future task starting:2026-03-30 due:2026-03-29"),
    ];

    sort_tasks(&mut tasks, "2026-03-29");

    assert_eq!(tasks[0].description, "Buy milk due:2026-03-29");
    assert_eq!(tasks[1].description, "File taxes due:2026-04-01");
    assert_eq!(tasks[2].description, "Call Mom due:2026-03-30");
    assert_eq!(
        tasks[3].description,
        "Future task starting:2026-03-30 due:2026-03-29"
    );
}

#[test]
fn sorting_uses_deterministic_tie_breaker() {
    let mut tasks = vec![
        parse_task_line("Bravo"),
        parse_task_line("Alpha"),
        parse_task_line("Charlie"),
    ];

    sort_tasks(&mut tasks, "2026-03-29");

    assert_eq!(tasks[0].raw, "Alpha");
    assert_eq!(tasks[1].raw, "Bravo");
    assert_eq!(tasks[2].raw, "Charlie");
}

#[test]
fn sorting_keeps_open_tasks_ahead_of_done_tasks() {
    let mut tasks = vec![
        parse_task_line("x 2026-03-29 2026-03-20 Archive receipts"),
        parse_task_line("Open task"),
        parse_task_line("x 2026-03-28 File taxes"),
    ];

    sort_tasks(&mut tasks, "2026-03-29");

    assert!(!tasks[0].done);
    assert_eq!(tasks[0].description, "Open task");
    assert!(tasks[1].done);
    assert_eq!(tasks[1].completion_date.as_deref(), Some("2026-03-29"));
    assert!(tasks[2].done);
    assert_eq!(tasks[2].completion_date.as_deref(), Some("2026-03-28"));
}

#[test]
fn groups_projects_and_contexts_in_insertion_order() {
    let task = parse_task_line("Call Mom +Family @phone");

    assert_eq!(task.projects, vec!["Family"]);
    assert_eq!(task.contexts, vec!["phone"]);
}
