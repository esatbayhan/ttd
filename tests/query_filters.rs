use ttd::parser::parse_task_line;
use ttd::query::sort_tasks;

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
