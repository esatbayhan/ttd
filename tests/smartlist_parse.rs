use std::path::Path;
use ttd::smartlist::{
    parse_list, CompareOp, Condition, DateField, Direction, Directive, Field,
    PriorityOp, TextOp, TextField,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn path(s: &str) -> &Path {
    Path::new(s)
}

fn make_content(frontmatter: &str, body: &str) -> String {
    format!("---\n{frontmatter}---\n{body}")
}

// ---------------------------------------------------------------------------
// Frontmatter tests
// ---------------------------------------------------------------------------

#[test]
fn parses_frontmatter_with_all_fields() {
    let content = make_content("name: Today\nicon: 📅\norder: 1\n", "");
    let list = parse_list(&content, path("lists.d/today.list"));
    assert_eq!(list.name, "Today");
    assert_eq!(list.icon, Some("📅".to_string()));
    assert_eq!(list.order, Some(1));
    assert!(list.parse_error.is_none());
}

#[test]
fn missing_name_falls_back_to_filename_stem() {
    let content = make_content("icon: 🗂\n", "");
    let list = parse_list(&content, path("lists.d/inbox.list"));
    assert_eq!(list.name, "inbox");
    assert!(list.parse_error.is_none());
}

#[test]
fn unknown_frontmatter_keys_are_ignored() {
    let content = make_content("name: Test\nunknown_key: whatever\nanother: value\n", "");
    let list = parse_list(&content, path("test.list"));
    assert_eq!(list.name, "Test");
    assert!(list.parse_error.is_none());
}

#[test]
fn missing_frontmatter_delimiters_set_parse_error() {
    let content = "name: Test\ndue <= today\n";
    let list = parse_list(content, path("test.list"));
    assert!(list.parse_error.is_some());
}

#[test]
fn empty_body_produces_no_filter_blocks() {
    let content = make_content("name: Empty\n", "");
    let list = parse_list(&content, path("empty.list"));
    assert!(list.blocks.is_empty());
    assert!(list.sort_directives.is_empty());
    assert!(list.group_directives.is_empty());
}

// ---------------------------------------------------------------------------
// Filter body tests
// ---------------------------------------------------------------------------

#[test]
fn parses_date_comparison_with_today() {
    let content = make_content("name: Today\n", "due <= today\n");
    let list = parse_list(&content, path("today.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::DateComparison {
            field: DateField::Due,
            op: CompareOp::Lte,
            offset: 0,
        }
    );
}

#[test]
fn parses_date_comparison_with_offset() {
    let content = make_content("name: Upcoming\n", "due <= today + 7\n");
    let list = parse_list(&content, path("upcoming.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::DateComparison {
            field: DateField::Due,
            op: CompareOp::Lte,
            offset: 7,
        }
    );
}

#[test]
fn parses_date_comparison_with_negative_offset() {
    let content = make_content("name: Overdue\n", "due < today - 14\n");
    let list = parse_list(&content, path("overdue.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::DateComparison {
            field: DateField::Due,
            op: CompareOp::Lt,
            offset: -14,
        }
    );
}

#[test]
fn parses_date_offset_without_spaces() {
    let content = make_content("name: Soon\n", "due <= today+7\n");
    let list = parse_list(&content, path("soon.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::DateComparison {
            field: DateField::Due,
            op: CompareOp::Lte,
            offset: 7,
        }
    );
}

#[test]
fn parses_priority_comparison() {
    let content = make_content("name: High Priority\n", "priority above C\n");
    let list = parse_list(&content, path("highpri.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::PriorityComparison {
            op: PriorityOp::Above,
            letter: 'C',
        }
    );
}

#[test]
fn parses_text_match() {
    let content = make_content("name: Work\n", "project includes Work\n");
    let list = parse_list(&content, path("work.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::TextMatch {
            field: TextField::Project,
            op: TextOp::Includes,
            text: "Work".to_string(),
        }
    );
}

#[test]
fn parses_existence_conditions() {
    let content = make_content("name: No Dates\n", "no due\nno scheduled\nno starting\n");
    let list = parse_list(&content, path("nodates.list"));
    assert_eq!(list.blocks.len(), 1);
    let conds = &list.blocks[0].conditions;
    assert_eq!(conds.len(), 3);
    assert_eq!(
        conds[0],
        Condition::Existence {
            field: Field::Due,
            present: false,
        }
    );
    assert_eq!(
        conds[1],
        Condition::Existence {
            field: Field::Scheduled,
            present: false,
        }
    );
    assert_eq!(
        conds[2],
        Condition::Existence {
            field: Field::Starting,
            present: false,
        }
    );
}

#[test]
fn parses_done_and_not_done() {
    let done_content = make_content("name: Done\n", "done\n");
    let done_list = parse_list(&done_content, path("done.list"));
    assert_eq!(done_list.blocks.len(), 1);
    assert_eq!(
        done_list.blocks[0].conditions[0],
        Condition::DoneFilter { done: true }
    );

    let not_done_content = make_content("name: Active\n", "not done\n");
    let not_done_list = parse_list(&not_done_content, path("active.list"));
    assert_eq!(not_done_list.blocks.len(), 1);
    assert_eq!(
        not_done_list.blocks[0].conditions[0],
        Condition::DoneFilter { done: false }
    );
}

#[test]
fn parses_or_blocks() {
    let body = "due <= today\nOR\nscheduled <= today\n";
    let content = make_content("name: Actionable\n", body);
    let list = parse_list(&content, path("actionable.list"));
    assert_eq!(list.blocks.len(), 2);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::DateComparison {
            field: DateField::Due,
            op: CompareOp::Lte,
            offset: 0,
        }
    );
    assert_eq!(
        list.blocks[1].conditions[0],
        Condition::DateComparison {
            field: DateField::Scheduled,
            op: CompareOp::Lte,
            offset: 0,
        }
    );
}

#[test]
fn parses_sort_and_group_directives() {
    let body = "due <= today\nsort by due asc\ngroup by priority desc\n";
    let content = make_content("name: Sorted\n", body);
    let list = parse_list(&content, path("sorted.list"));
    assert_eq!(list.sort_directives.len(), 1);
    assert_eq!(
        list.sort_directives[0],
        Directive {
            field: Field::Due,
            direction: Direction::Asc,
        }
    );
    assert_eq!(list.group_directives.len(), 1);
    assert_eq!(
        list.group_directives[0],
        Directive {
            field: Field::Priority,
            direction: Direction::Desc,
        }
    );
}

#[test]
fn unrecognized_filter_lines_are_silently_skipped() {
    let body = "due <= today\nthis is nonsense\nfoo bar baz\nscheduled <= today\n";
    let content = make_content("name: Test\n", body);
    let list = parse_list(&content, path("test.list"));
    // Only valid conditions are kept; no panic or error
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(list.blocks[0].conditions.len(), 2);
    assert!(list.parse_error.is_none());
}

#[test]
fn comments_and_blank_lines_are_ignored() {
    let body = "# This is a comment\n\ndue <= today\n\n# Another comment\nscheduled <= today\n";
    let content = make_content("name: Test\n", body);
    let list = parse_list(&content, path("test.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(list.blocks[0].conditions.len(), 2);
}

#[test]
fn has_existence_form_parses_correctly() {
    let content = make_content("name: Has Due\n", "has due\n");
    let list = parse_list(&content, path("hasdue.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::Existence {
            field: Field::Due,
            present: true,
        }
    );
}

#[test]
fn text_excludes_operator_parses_correctly() {
    let content = make_content("name: Not Work\n", "project excludes Work\n");
    let list = parse_list(&content, path("notwork.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::TextMatch {
            field: TextField::Project,
            op: TextOp::Excludes,
            text: "Work".to_string(),
        }
    );
}

#[test]
fn priority_eq_and_below_parse_correctly() {
    let eq_content = make_content("name: Priority A\n", "priority = A\n");
    let eq_list = parse_list(&eq_content, path("pria.list"));
    assert_eq!(eq_list.blocks.len(), 1);
    assert_eq!(
        eq_list.blocks[0].conditions[0],
        Condition::PriorityComparison {
            op: PriorityOp::Eq,
            letter: 'A',
        }
    );

    let below_content = make_content("name: Below C\n", "priority below C\n");
    let below_list = parse_list(&below_content, path("belowc.list"));
    assert_eq!(below_list.blocks.len(), 1);
    assert_eq!(
        below_list.blocks[0].conditions[0],
        Condition::PriorityComparison {
            op: PriorityOp::Below,
            letter: 'C',
        }
    );
}

#[test]
fn today_negative_offset_without_spaces() {
    let content = make_content("name: Recent\n", "due <= today-7\n");
    let list = parse_list(&content, path("recent.list"));
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(
        list.blocks[0].conditions[0],
        Condition::DateComparison {
            field: DateField::Due,
            op: CompareOp::Lte,
            offset: -7,
        }
    );
}

#[test]
fn crlf_line_endings_are_handled() {
    let content = "---\r\nname: CRLF Test\r\n---\r\ndue <= today\r\nscheduled <= today\r\n";
    let list = parse_list(content, path("crlf.list"));
    assert_eq!(list.name, "CRLF Test");
    assert!(list.parse_error.is_none());
    assert_eq!(list.blocks.len(), 1);
    assert_eq!(list.blocks[0].conditions.len(), 2);
}

#[test]
fn sort_directive_defaults_to_asc() {
    let content = make_content("name: Sorted\n", "due <= today\nsort by due\n");
    let list = parse_list(&content, path("sorted.list"));
    assert_eq!(list.sort_directives.len(), 1);
    assert_eq!(
        list.sort_directives[0],
        Directive {
            field: Field::Due,
            direction: Direction::Asc,
        }
    );
}
