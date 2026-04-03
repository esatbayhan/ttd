use std::path::PathBuf;

use ttd::parser::parse_task_line;
use ttd::smartlist::{evaluate, group, parse_list};
use ttd::store::{StoredTask, TaskId};

fn stored(raw: &str) -> StoredTask {
    StoredTask {
        id: TaskId {
            path: PathBuf::from(format!(
                "{}.txt",
                raw.split_whitespace().next().unwrap_or("task")
            )),
            line_index: 0,
        },
        task: parse_task_line(raw),
    }
}

fn make_list(body: &str) -> ttd::smartlist::SmartList {
    let content = format!("---\nname: Test\n---\n{}", body);
    parse_list(&content, &PathBuf::from("test.list"))
}

// ── Evaluate tests ────────────────────────────────────────────────────────────

#[test]
fn today_list_matches_due_or_scheduled_today() {
    // Two blocks OR'd: (due = today) OR (scheduled = today)
    // Sort by priority asc
    let list = make_list(
        "due = today\nsort by priority asc\nOR\nscheduled = today\n",
    );

    let today = "2026-04-03";
    let tasks = vec![
        stored("(A) Task A due:2026-04-03"),
        stored("(B) Task B scheduled:2026-04-03"),
        stored("(C) Task C due:2026-04-10"),   // not today
        stored("Task D"),                       // no dates
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 2);
    let descriptions: Vec<&str> = result
        .iter()
        .map(|st| st.task.description.as_str())
        .collect();
    // Priority A < B so A comes first
    assert!(descriptions[0].contains("Task A"));
    assert!(descriptions[1].contains("Task B"));
}

#[test]
fn inbox_list_matches_tasks_without_dates() {
    // AND: no due AND no scheduled AND no starting
    let list = make_list("no due\nno scheduled\nno starting\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("Task inbox"),
        stored("Task with due due:2026-04-03"),
        stored("Task with scheduled scheduled:2026-04-03"),
        stored("Task with starting starting:2026-04-03"),
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 1);
    assert!(result[0].task.description.contains("Task inbox"));
}

#[test]
fn priority_above_filters_correctly() {
    // priority above C → only A and B match
    let list = make_list("priority above C\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("(A) Task A"),
        stored("(B) Task B"),
        stored("(C) Task C"),
        stored("(D) Task D"),
        stored("Task no priority"),
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 2);
    let priorities: Vec<char> = result
        .iter()
        .filter_map(|st| st.task.priority)
        .collect();
    assert!(priorities.contains(&'A'));
    assert!(priorities.contains(&'B'));
}

#[test]
fn text_match_is_case_insensitive() {
    // project includes work should match +Work and +WORK
    let list = make_list("project includes work\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("Task one +Work"),
        stored("Task two +WORK"),
        stored("Task three +home"),
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 2);
}

#[test]
fn done_filter_includes_completed_tasks() {
    let list = make_list("done\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("x 2026-04-01 Completed task"),
        stored("Open task"),
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 1);
    assert!(result[0].task.done);
}

#[test]
fn not_done_implied_when_no_done_filter_present() {
    // No done filter → only open tasks are considered
    let list = make_list("has description\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("x 2026-04-01 Completed task"),
        stored("Open task"),
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 1);
    assert!(!result[0].task.done);
}

#[test]
fn empty_list_matches_nothing() {
    let list = make_list(""); // no conditions → no blocks

    let today = "2026-04-03";
    let tasks = vec![stored("Some task"), stored("Another task")];

    let result = evaluate(&list, &tasks, today);
    assert!(result.is_empty());
}

#[test]
fn sort_by_due_asc() {
    let list = make_list("has due\nsort by due asc\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("Task C due:2026-04-10"),
        stored("Task A due:2026-04-03"),
        stored("Task B due:2026-04-05"),
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 3);
    let due_dates: Vec<&str> = result
        .iter()
        .map(|st| st.task.tags.get("due").map(|s| s.as_str()).unwrap_or(""))
        .collect();
    assert_eq!(due_dates, vec!["2026-04-03", "2026-04-05", "2026-04-10"]);
}

#[test]
fn date_offset_arithmetic_works() {
    // due <= today + 7 means tasks due within the next week
    let list = make_list("due <= today + 7\n");

    let today = "2026-04-03";
    let tasks = vec![
        stored("Task soon due:2026-04-10"),   // exactly +7 days → included
        stored("Task later due:2026-04-11"),  // +8 days → excluded
        stored("Task today due:2026-04-03"),  // today → included
    ];

    let result = evaluate(&list, &tasks, today);
    assert_eq!(result.len(), 2);
    let descriptions: Vec<&str> = result
        .iter()
        .map(|st| st.task.description.as_str())
        .collect();
    assert!(descriptions.iter().any(|d| d.contains("Task soon")));
    assert!(descriptions.iter().any(|d| d.contains("Task today")));
}

// ── Group tests ───────────────────────────────────────────────────────────────

#[test]
fn group_by_priority_creates_labeled_groups() {
    let list = make_list("group by priority asc\n");

    let tasks = vec![
        stored("(A) Task one"),
        stored("(B) Task two"),
        stored("(A) Task three"),
    ];

    let groups = group(&list, &tasks);
    // Should have two groups: A and B
    assert_eq!(groups.len(), 2);
    let a_group = groups.iter().find(|g| g.label == "Priority: A").expect("A group");
    let b_group = groups.iter().find(|g| g.label == "Priority: B").expect("B group");
    assert_eq!(a_group.tasks.len(), 2);
    assert_eq!(b_group.tasks.len(), 1);
}

#[test]
fn group_by_due_creates_date_groups() {
    let list = make_list("group by due asc\n");

    let tasks = vec![
        stored("Task A due:2026-04-03"),
        stored("Task B due:2026-04-10"),
        stored("Task C due:2026-04-03"),
    ];

    let groups = group(&list, &tasks);
    assert_eq!(groups.len(), 2);
    let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(labels, vec!["Due: 2026-04-03", "Due: 2026-04-10"]);
    assert_eq!(groups[0].tasks.len(), 2);
    assert_eq!(groups[1].tasks.len(), 1);
}

#[test]
fn no_group_directive_returns_single_unnamed_group() {
    let list = make_list("has description\n");

    let tasks = vec![stored("Task A"), stored("Task B")];

    let groups = group(&list, &tasks);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].label, "");
    assert_eq!(groups[0].tasks.len(), 2);
}

#[test]
fn tasks_without_group_field_value_go_to_end() {
    let list = make_list("group by priority asc\n");

    let tasks = vec![
        stored("(A) Task with priority"),
        stored("Task without priority"),
    ];

    let groups = group(&list, &tasks);
    assert_eq!(groups.len(), 2);
    // First group: A (has priority)
    assert_eq!(groups[0].label, "Priority: A");
    // Last group: fallback
    assert_eq!(groups[1].label, "No priority");
    assert_eq!(groups[1].tasks.len(), 1);
}

#[test]
fn group_by_desc_reverses_group_order() {
    let list = make_list("group by priority desc\n");

    let tasks = vec![
        stored("(A) Task one"),
        stored("(B) Task two"),
        stored("(C) Task three"),
    ];

    let groups = group(&list, &tasks);
    assert_eq!(groups.len(), 3);
    let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
    // Desc: C > B > A
    assert_eq!(labels, vec!["Priority: C", "Priority: B", "Priority: A"]);
}
