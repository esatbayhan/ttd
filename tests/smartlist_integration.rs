use std::fs;
use std::path::PathBuf;
use ttd::tui::session::{SidebarItem, TuiSession};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-smartlist-integ-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

fn setup_with_spec_examples(name: &str) -> (PathBuf, TuiSession) {
    let root = temp_path(name);
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::create_dir_all(root.join("lists.d")).unwrap();

    // Tasks
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();
    fs::write(root.join("b.txt"), "(A) File taxes due:2026-03-30 +Admin\n").unwrap();
    fs::write(
        root.join("c.txt"),
        "(B) Prepare report scheduled:2026-03-30 +Work @desk\n",
    )
    .unwrap();
    fs::write(
        root.join("d.txt"),
        "Plan vacation starting:2026-04-15 due:2026-05-01\n",
    )
    .unwrap();
    fs::write(
        root.join("done.txt.d/e.txt"),
        "x 2026-03-29 Archive receipts +Admin @desk\n",
    )
    .unwrap();

    // List files (from spec examples)
    fs::write(
        root.join("lists.d/today.list"),
        "---\nname: Today\nicon: 📅\norder: 1\n---\ndue <= today\nOR\nscheduled <= today\n\nsort by priority desc\nsort by due asc\n",
    )
    .unwrap();
    fs::write(
        root.join("lists.d/inbox.list"),
        "---\nname: Inbox\nicon: 📥\norder: 2\n---\nno due\nno scheduled\nno starting\n\nsort by priority desc\nsort by creation_date asc\n",
    )
    .unwrap();
    fs::write(
        root.join("lists.d/upcoming.list"),
        "---\nname: Upcoming\nicon: 📆\norder: 3\n---\ndue > today\nOR\nscheduled > today\nOR\nstarting > today\n\nsort by due asc\ngroup by due\n",
    )
    .unwrap();

    let session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    (root, session)
}

#[test]
fn sidebar_shows_smart_lists_in_order() {
    let (_root, session) = setup_with_spec_examples("sidebar-order");
    let items = session.sidebar_items();

    // First three items should be SmartList(0), SmartList(1), SmartList(2)
    // ordered by the `order` field in their frontmatter: Today(1), Inbox(2), Upcoming(3)
    assert_eq!(items[0], SidebarItem::SmartList(0));
    assert_eq!(items[1], SidebarItem::SmartList(1));
    assert_eq!(items[2], SidebarItem::SmartList(2));
    assert_eq!(items[3], SidebarItem::Separator);

    // Confirm names are in the right order
    let lists = session.smart_lists();
    assert_eq!(lists[0].name, "Today");
    assert_eq!(lists[1].name, "Inbox");
    assert_eq!(lists[2].name, "Upcoming");
}

#[test]
fn today_list_shows_due_and_scheduled_today() {
    let (_root, mut session) = setup_with_spec_examples("today-list");

    // Select the Today smart list (index 0)
    session.select_sidebar_item(SidebarItem::SmartList(0));

    let tasks = session.visible_tasks();
    assert_eq!(tasks.len(), 2, "Expected 2 tasks matching today's date");

    // Both tasks have dates matching today; both should be present
    let descriptions: Vec<&str> = tasks.iter().map(|t| t.task.description.as_str()).collect();
    assert!(
        descriptions.iter().any(|d| d.contains("File taxes")),
        "Expected 'File taxes' in today tasks, got: {:?}",
        descriptions
    );
    assert!(
        descriptions.iter().any(|d| d.contains("Prepare report")),
        "Expected 'Prepare report' in today tasks, got: {:?}",
        descriptions
    );

    // Sorted by priority desc (string order): 'B' > 'A' alphabetically, so (B) Prepare report first
    assert_eq!(tasks[0].task.priority, Some('B'));
    assert!(
        tasks[0].task.description.contains("Prepare report"),
        "First task (priority desc) should be 'Prepare report' (B > A), got: {}",
        tasks[0].task.description
    );
    assert_eq!(tasks[1].task.priority, Some('A'));
    assert!(
        tasks[1].task.description.contains("File taxes"),
        "Second task (priority desc) should be 'File taxes' (A), got: {}",
        tasks[1].task.description
    );
}

#[test]
fn inbox_list_shows_tasks_without_dates() {
    let (_root, mut session) = setup_with_spec_examples("inbox-list");

    // Select the Inbox smart list (index 1)
    session.select_sidebar_item(SidebarItem::SmartList(1));

    let tasks = session.visible_tasks();
    assert_eq!(tasks.len(), 1, "Expected 1 task with no date fields");
    assert!(
        tasks[0].task.description.contains("Call Mom"),
        "Inbox task should be 'Call Mom', got: {}",
        tasks[0].task.description
    );
}

#[test]
fn upcoming_list_shows_future_tasks() {
    let (_root, mut session) = setup_with_spec_examples("upcoming-list");

    // Select the Upcoming smart list (index 2)
    session.select_sidebar_item(SidebarItem::SmartList(2));

    let tasks = session.visible_tasks();
    let descriptions: Vec<&str> = tasks.iter().map(|t| t.task.description.as_str()).collect();

    // Plan vacation has due:2026-05-01 which is > today (2026-03-30)
    assert!(
        descriptions.iter().any(|d| d.contains("Plan vacation")),
        "Expected 'Plan vacation' in upcoming tasks, got: {:?}",
        descriptions
    );
}

#[test]
fn default_selection_is_first_smart_list() {
    let (_root, session) = setup_with_spec_examples("default-selection");

    assert_eq!(
        session.active_sidebar_item(),
        SidebarItem::SmartList(0),
        "Default active sidebar item should be SmartList(0)"
    );
}

#[test]
fn no_list_files_means_empty_smart_section() {
    let root = temp_path("no-lists");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    // Intentionally do NOT create lists.d or any .list files
    fs::write(root.join("a.txt"), "Some task +Project\n").unwrap();

    let session = TuiSession::open(root, "2026-03-30").unwrap();
    let items = session.sidebar_items();

    // With no smart lists, the first item should be ProjectsHeader (no separator before it)
    assert_eq!(
        items[0],
        SidebarItem::ProjectsHeader,
        "First sidebar item should be ProjectsHeader when no smart lists exist"
    );
    assert_eq!(
        session.smart_lists().len(),
        0,
        "Should have no smart lists loaded"
    );
}

#[test]
fn upcoming_list_has_groups() {
    let (_root, mut session) = setup_with_spec_examples("upcoming-groups");

    // Select the Upcoming smart list (index 2) which has `group by due`
    session.select_sidebar_item(SidebarItem::SmartList(2));

    let groups = session.visible_groups();

    // There should be at least one group
    assert!(
        !groups.is_empty(),
        "Expected at least one group for Upcoming list"
    );

    // All non-empty groups should have non-empty labels (date labels)
    for group in groups {
        if !group.tasks.is_empty() {
            assert!(
                !group.label.is_empty(),
                "Expected a date label for group containing tasks"
            );
        }
    }

    // Plan vacation has due:2026-05-01, so there should be a group with that label
    let has_date_label = groups.iter().any(|g| g.label.contains("2026-05-01"));
    assert!(
        has_date_label,
        "Expected group with label '2026-05-01' for Plan vacation task"
    );
}

#[test]
fn malformed_list_shows_in_sidebar_with_error() {
    let root = temp_path("malformed-list");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::create_dir_all(root.join("lists.d")).unwrap();

    fs::write(root.join("a.txt"), "Some task\n").unwrap();

    // A bad.list with no frontmatter delimiters
    fs::write(
        root.join("lists.d/bad.list"),
        "due <= today\nno scheduled\n",
    )
    .unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();

    // The bad list should still appear in the sidebar
    let items = session.sidebar_items();
    let has_smart_list = items.iter().any(|i| matches!(i, SidebarItem::SmartList(_)));
    assert!(
        has_smart_list,
        "Malformed list should still appear in sidebar as a SmartList item"
    );

    // The smart list should have a parse_error set
    let lists = session.smart_lists();
    assert_eq!(lists.len(), 1, "Should have exactly one smart list loaded");
    assert!(
        lists[0].parse_error.is_some(),
        "Malformed list should have parse_error set, got: {:?}",
        lists[0].parse_error
    );

    // When selected, it should show no tasks (parse_error causes empty results)
    session.select_sidebar_item(SidebarItem::SmartList(0));
    let tasks = session.visible_tasks();
    assert_eq!(
        tasks.len(),
        0,
        "Malformed list should show no tasks when selected"
    );
}
