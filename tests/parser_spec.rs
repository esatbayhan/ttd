use std::fs;
use std::path::Path;

use ttd::parser::{format_task, parse_task_line};

fn read_fixture_line(path: &str) -> String {
    let raw = fs::read_to_string(Path::new(path)).unwrap();

    raw.strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .or_else(|| raw.strip_suffix('\r'))
        .unwrap_or(&raw)
        .to_owned()
}

#[test]
fn parses_valid_due_fixture() {
    let raw = read_fixture_line("spec/examples/valid/tag-due.txt");
    let task = parse_task_line(&raw);

    assert!(!task.done);
    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-04-15"));
}

#[test]
fn parses_priority_only_fixture() {
    let raw = read_fixture_line("spec/examples/valid/priority-only.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.priority, Some('A'));
    assert_eq!(task.creation_date, None);
    assert_eq!(task.description, "Call Mom");
}

#[test]
fn parses_creation_date_without_priority_fixture() {
    let raw = read_fixture_line("spec/examples/valid/creation-date-no-priority.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.priority, None);
    assert_eq!(task.creation_date.as_deref(), Some("2024-01-15"));
    assert_eq!(task.description, "Call Mom");
}

#[test]
fn malformed_first_duplicate_key_blocks_second_key() {
    let raw = read_fixture_line(
        "spec/examples/edge-cases/malformed-first-key-blocks-valid-duplicate.txt",
    );
    let task = parse_task_line(&raw);

    assert_eq!(task.tags.get("due"), None);
    assert!(task.description.contains("due:2024-01-01"));
}

#[test]
fn malformed_first_due_blocks_valid_second_fixture() {
    let raw =
        read_fixture_line("spec/examples/invalid/malformed-first-due-key-blocks-valid-second.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.tags.get("due"), None);
    assert!(task.description.contains("due:next-week"));
    assert!(task.description.contains("due:2024-05-01"));
}

#[test]
fn invalid_completion_marker_stays_in_description() {
    let raw = read_fixture_line("spec/examples/invalid/completion-date-missing.txt");
    let task = parse_task_line(&raw);

    assert!(!task.done);
    assert!(task.description.starts_with("x "));
}

#[test]
fn uppercase_completion_marker_stays_in_description() {
    let raw = read_fixture_line("spec/examples/invalid/completion-marker-uppercase.txt");
    let task = parse_task_line(&raw);

    assert!(!task.done);
    assert_eq!(task.completion_date, None);
    assert_eq!(task.description, raw);
}

#[test]
fn extracts_metadata_after_tab_boundary() {
    let task = parse_task_line("Call Mom\t+Family\t@phone\tdue:2024-04-15");

    assert_eq!(
        task.description,
        "Call Mom\t+Family\t@phone\tdue:2024-04-15"
    );
    assert_eq!(task.projects, vec!["Family"]);
    assert_eq!(task.contexts, vec!["phone"]);
    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-04-15"));
}

#[test]
fn parses_completed_basic_fixture() {
    let raw = read_fixture_line("spec/examples/valid/done.txt.d/completed-basic.txt");
    let task = parse_task_line(&raw);

    assert!(task.done);
    assert_eq!(task.completion_date.as_deref(), Some("2024-03-01"));
    assert_eq!(task.creation_date, None);
    assert_eq!(task.description, "Call Mom");
}

#[test]
fn parses_completed_task_fixture_with_both_dates() {
    let raw = read_fixture_line("spec/examples/valid/done.txt.d/completed-with-both-dates.txt");
    let task = parse_task_line(&raw);

    assert!(task.done);
    assert_eq!(task.completion_date.as_deref(), Some("2024-01-15"));
    assert_eq!(task.creation_date.as_deref(), Some("2024-01-10"));
    assert_eq!(task.projects, vec!["TodoTxtTouch"]);
    assert_eq!(task.contexts, vec!["github"]);
}

#[test]
fn parses_completed_with_metadata_fixture() {
    let raw = read_fixture_line("spec/examples/edge-cases/done.txt.d/completed-with-metadata.txt");
    let task = parse_task_line(&raw);

    assert!(task.done);
    assert_eq!(task.completion_date.as_deref(), Some("2024-02-15"));
    assert_eq!(task.creation_date.as_deref(), Some("2024-01-01"));
    assert_eq!(task.projects, vec!["Shopping"]);
    assert_eq!(task.contexts, vec!["errands"]);
    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-02-15"));
}

#[test]
fn parses_scheduled_fixture() {
    let raw = read_fixture_line("spec/examples/valid/tag-scheduled.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.creation_date.as_deref(), Some("2024-03-01"));
    assert_eq!(
        task.tags.get("scheduled").map(String::as_str),
        Some("2024-03-20")
    );
    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-03-31"));
}

#[test]
fn parses_starting_fixture() {
    let raw = read_fixture_line("spec/examples/valid/tag-starting.txt");
    let task = parse_task_line(&raw);

    assert_eq!(
        task.tags.get("starting").map(String::as_str),
        Some("2024-06-01")
    );
    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-06-10"));
}

#[test]
fn parses_multiple_projects_and_contexts_fixture() {
    let raw = read_fixture_line("spec/examples/valid/multiple-projects-and-contexts.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.priority, Some('A'));
    assert_eq!(task.projects, vec!["Family", "PeaceLoveAndHappiness"]);
    assert_eq!(task.contexts, vec!["iphone", "phone"]);
}

#[test]
fn parses_multiple_tags_fixture() {
    let raw = read_fixture_line("spec/examples/valid/multiple-tags.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.priority, Some('A'));
    assert_eq!(task.creation_date.as_deref(), Some("2024-01-01"));
    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-06-01"));
    assert_eq!(
        task.tags.get("scheduled").map(String::as_str),
        Some("2024-05-15")
    );
    assert_eq!(
        task.tags.get("starting").map(String::as_str),
        Some("2024-03-01")
    );
}

#[test]
fn valid_duplicate_key_fixture_keeps_first_value() {
    let raw = read_fixture_line("spec/examples/edge-cases/duplicate-date-key.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.tags.get("due").map(String::as_str), Some("2024-04-15"));
    assert!(task.description.contains("due:2024-05-01"));
}

#[test]
fn plus_sign_inside_token_is_not_a_project() {
    let raw = read_fixture_line("spec/examples/edge-cases/plus-sign-in-math.txt");
    let task = parse_task_line(&raw);

    assert!(task.projects.is_empty());
    assert_eq!(task.description, raw);
}

#[test]
fn at_sign_inside_token_is_not_a_context() {
    let raw = read_fixture_line("spec/examples/edge-cases/email-contains-at-sign.txt");
    let task = parse_task_line(&raw);

    assert!(task.contexts.is_empty());
    assert_eq!(task.description, raw);
}

#[test]
fn multi_colon_token_is_not_a_tag() {
    let raw = read_fixture_line("spec/examples/edge-cases/time-with-two-colons-not-a-tag.txt");
    let task = parse_task_line(&raw);

    assert!(task.tags.is_empty());
    assert_eq!(task.description, raw);
}

#[test]
fn parses_project_at_start_of_description_fixture() {
    let raw = read_fixture_line("spec/examples/valid/project-at-start-of-description.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.projects, vec!["GarageSale"]);
    assert_eq!(task.description, raw);
}

#[test]
fn parses_context_at_start_of_description_fixture() {
    let raw = read_fixture_line("spec/examples/valid/context-at-start-of-description.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.contexts, vec!["phone"]);
    assert_eq!(task.description, raw);
}

#[test]
fn invalid_due_partial_date_stays_in_description() {
    let raw = read_fixture_line("spec/examples/invalid/due-key-partial-date.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.tags.get("due"), None);
    assert_eq!(task.description, raw);
}

#[test]
fn invalid_due_non_date_stays_in_description() {
    let raw = read_fixture_line("spec/examples/invalid/due-key-non-date-value.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.tags.get("due"), None);
    assert_eq!(task.description, raw);
}

#[test]
fn invalid_priority_fixture_stays_in_description() {
    let raw = read_fixture_line("spec/examples/invalid/priority-lowercase.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.priority, None);
    assert_eq!(task.description, raw);
}

#[test]
fn wrong_format_creation_date_stays_in_description() {
    let raw = read_fixture_line("spec/examples/invalid/creation-date-wrong-format.txt");
    let task = parse_task_line(&raw);

    assert_eq!(task.priority, Some('A'));
    assert_eq!(task.creation_date, None);
    assert_eq!(task.description, "01-15-2024 Call Mom");
}

#[test]
fn formatter_round_trips_normalized_open_task() {
    let task = parse_task_line("(A) 2024-01-15 Call Mom +Family due:2024-04-15");

    assert_eq!(
        format_task(&task),
        "(A) 2024-01-15 Call Mom +Family due:2024-04-15"
    );
}
