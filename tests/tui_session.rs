use std::fs;
use std::path::PathBuf;

use ttd::tui::editor::{EditorState, SaveConflictState};
use ttd::tui::session::{SidebarItem, TuiSession};

fn write_standard_lists(root: &std::path::Path) {
    let lists_dir = root.join("lists.d");
    fs::create_dir_all(&lists_dir).unwrap();
    fs::write(
        lists_dir.join("inbox.list"),
        "---\nname: Inbox\norder: 1\n---\nno due\nno scheduled\nno starting\n",
    )
    .unwrap();
    fs::write(
        lists_dir.join("done.list"),
        "---\nname: Done\norder: 5\n---\ndone\n",
    )
    .unwrap();
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-session-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn welcome_session_does_not_create_placeholder_task_store_on_disk() {
    let placeholder =
        std::env::temp_dir().join(format!("ttd-welcome-placeholder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&placeholder);

    let session = TuiSession::welcome("2026-03-30");

    assert_eq!(session.app().mode, ttd::tui::app::AppMode::Welcome);
    assert!(!placeholder.exists());
}

#[test]
fn session_loads_snapshot_and_exposes_smart_filters() {
    let root = temp_path("load");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();
    fs::write(root.join("b.txt"), "(A) File taxes due:2026-03-30 +Admin\n").unwrap();
    fs::write(
        root.join("done.txt.d/c.txt"),
        "x 2026-03-29 Archive receipts +Admin @desk\n",
    )
    .unwrap();

    let session = TuiSession::open(root, "2026-03-30").unwrap();

    assert_eq!(
        session.active_sidebar_item(),
        SidebarItem::SmartList(0)
    );
    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(
        session.visible_tasks()[0].task.description,
        "Call Mom +Family @phone"
    );
    assert!(
        session
            .sidebar_items()
            .contains(&SidebarItem::SmartList(1))
    );
}

#[test]
fn session_discovers_project_and_context_sidebar_items() {
    let root = temp_path("dynamic-sidebar");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package +Errands @town\n").unwrap();

    let session = TuiSession::open(root, "2026-03-30").unwrap();

    assert!(
        session
            .sidebar_items()
            .contains(&SidebarItem::Project("+Errands".into()))
    );
    assert!(
        session
            .sidebar_items()
            .contains(&SidebarItem::Project("+Family".into()))
    );
    assert!(
        session
            .sidebar_items()
            .contains(&SidebarItem::Context("@phone".into()))
    );
    assert!(
        session
            .sidebar_items()
            .contains(&SidebarItem::Context("@town".into()))
    );
}

#[test]
fn selecting_project_or_context_filters_visible_tasks() {
    let root = temp_path("filter");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package +Errands @town\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));

    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(
        session.visible_tasks()[0].task.description,
        "Call Mom +Family @phone"
    );

    session.select_sidebar_item(SidebarItem::Context("@town".into()));

    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(
        session.visible_tasks()[0].task.description,
        "Ship package +Errands @town"
    );
}

#[test]
fn sidebar_focus_navigation_moves_between_smart_and_dynamic_items() {
    let root = temp_path("sidebar-nav");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package +Errands @town\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::Sidebar;

    session.dispatch_key("j").unwrap();
    session.dispatch_key("j").unwrap();

    assert_eq!(
        session.active_sidebar_item(),
        SidebarItem::Project("+Errands".into())
    );
    assert_eq!(
        session.visible_tasks()[0].task.description,
        "Ship package +Errands @town"
    );
}

#[test]
fn conflict_actions_can_reload_and_dismiss_session_dialog() {
    let root = temp_path("conflict-actions");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    session.app_mut().editor = Some(EditorState::quick_entry());
    session.app_mut().save_conflict = Some(SaveConflictState {
        external_raw: "External version".into(),
    });

    session.dispatch_key("r").unwrap();

    assert!(session.app().save_conflict.is_none());
    assert_eq!(
        session
            .app()
            .editor
            .as_ref()
            .expect("editor should stay open after reload")
            .raw_line,
        "External version"
    );

    session.app_mut().save_conflict = Some(SaveConflictState {
        external_raw: "Another external version".into(),
    });

    session.dispatch_key("esc").unwrap();

    assert!(session.app().save_conflict.is_none());
}

#[test]
fn quick_entry_creates_a_task_and_selects_it() {
    let root = temp_path("quick-entry");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    assert_eq!(
        session.selected_task().unwrap().task.description,
        "Call Mom"
    );

    session.dispatch_key("a").unwrap();
    for key in ["Z", "o", "o", " ", "v", "i", "s", "i", "t"] {
        session.dispatch_key(key).unwrap();
    }
    session.dispatch_key("enter").unwrap();

    let files = fs::read_dir(&root).unwrap().count();
    assert_eq!(files, 4); // a.txt, new task file, done.txt.d, lists.d
    assert_eq!(session.visible_tasks().len(), 2);
    assert_eq!(
        session.selected_task().unwrap().task.description,
        "Zoo visit"
    );
}

#[test]
fn quick_entry_selects_new_duplicate_task() {
    let root = temp_path("quick-entry-duplicate");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    assert_eq!(session.selected_task().unwrap().id.file_name(), "a.txt");

    session.dispatch_key("a").unwrap();
    for key in ["C", "a", "l", "l", " ", "M", "o", "m"] {
        session.dispatch_key(key).unwrap();
    }
    session.dispatch_key("enter").unwrap();

    let selected = session.selected_task().unwrap();
    assert_eq!(selected.task.description, "Call Mom");
    assert_ne!(selected.id.file_name(), "a.txt");
    assert!(root.join(selected.id.file_name()).exists());
}

#[test]
fn quick_entry_hidden_by_current_filter_preserves_previous_selection() {
    let root = temp_path("quick-entry-hidden-selection");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Plan taxes +Admin\n").unwrap();
    fs::write(root.join("b.txt"), "Call Mom +Admin\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Admin".into()));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("j").unwrap();
    let expected_file_name = session.selected_task().unwrap().id.file_name().to_string();
    let expected_description = session.selected_task().unwrap().task.description.clone();

    // Quick entry in a project view auto-appends the project tag, so the
    // new task appears in the current +Admin view and gets selected.
    session.dispatch_key("a").unwrap();
    for key in ["O", "t", "h", "e", "r", " ", "t", "a", "s", "k"] {
        session.dispatch_key(key).unwrap();
    }
    session.dispatch_key("enter").unwrap();

    assert_eq!(session.visible_tasks().len(), 3);
    let selected = session.selected_task().unwrap();
    assert!(
        selected.task.description.contains("Other task"),
        "newly created task should be selected"
    );
}

#[test]
fn search_filters_visible_tasks_within_the_active_sidebar_view() {
    let root = temp_path("search-active-view");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package +Errands\n").unwrap();
    fs::write(root.join("c.txt"), "Call Dad +Family\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));

    session.dispatch_key("/").unwrap();
    for key in ["C", "a", "l", "l", " ", "D", "a", "d"] {
        session.dispatch_key(key).unwrap();
    }

    assert!(session.app().search_active);
    assert_eq!(session.app().search_query, "Call Dad");
    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(session.visible_tasks()[0].task.raw, "Call Dad +Family");
}

#[test]
fn clearing_search_restores_the_unfiltered_active_view() {
    let root = temp_path("search-clear");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();
    fs::write(root.join("b.txt"), "Call Dad +Family\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));
    session.dispatch_key("/").unwrap();
    session.dispatch_key("M").unwrap();
    session.dispatch_key("o").unwrap();
    session.dispatch_key("m").unwrap();

    assert_eq!(session.visible_tasks().len(), 1);

    session.dispatch_key("backspace").unwrap();
    session.dispatch_key("backspace").unwrap();
    session.dispatch_key("backspace").unwrap();

    assert_eq!(session.visible_tasks().len(), 2);
}

#[test]
fn search_next_and_previous_cycle_between_matches_only() {
    let root = temp_path("search-next-prev");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("b.txt"), "Email Alex\n").unwrap();
    fs::write(root.join("c.txt"), "Call Dad\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("/").unwrap();
    for key in ["C", "a", "l", "l"] {
        session.dispatch_key(key).unwrap();
    }

    assert_eq!(session.visible_tasks().len(), 2);
    let first = session.selected_task().unwrap().task.raw.clone();
    let second = if first == "Call Mom" {
        "Call Dad"
    } else {
        "Call Mom"
    };

    session.dispatch_key("esc").unwrap();
    session.dispatch_key("n").unwrap();
    assert_eq!(session.selected_task().unwrap().task.raw, second);

    session.dispatch_key("N").unwrap();
    assert_eq!(session.selected_task().unwrap().task.raw, first);
}

#[test]
fn toggle_done_moves_selected_open_task_into_done_view() {
    let root = temp_path("done");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("x").unwrap();
    session.select_sidebar_item(SidebarItem::SmartList(1));

    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(session.visible_tasks()[0].task.description, "Call Mom");
    assert!(!root.join("a.txt").exists());
}

#[test]
fn delete_confirmation_removes_the_selected_open_task() {
    let root = temp_path("delete-open");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-31").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;

    session.dispatch_key("D").unwrap();
    assert!(session.app().confirm_delete);

    session.dispatch_key("enter").unwrap();

    assert!(!root.join("a.txt").exists());
    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(session.selected_task().unwrap().task.raw, "Ship package");
    assert!(!session.app().confirm_delete);
}

#[test]
fn delete_confirmation_removes_the_selected_done_task() {
    let root = temp_path("delete-done");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("done.txt.d/a.txt"), "x 2026-03-30 Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-31").unwrap();
    session.select_sidebar_item(SidebarItem::SmartList(1));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;

    session.dispatch_key("D").unwrap();
    session.dispatch_key("enter").unwrap();

    assert!(!root.join("done.txt.d/a.txt").exists());
    assert!(session.visible_tasks().is_empty());
}

#[test]
fn restore_moves_selected_done_task_back_to_open_view() {
    let root = temp_path("restore");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("done.txt.d/a.txt"), "x 2026-03-29 Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::SmartList(1));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("r").unwrap();

    assert!(root.join("a.txt").exists());
    assert!(!root.join("done.txt.d/a.txt").exists());
}

#[test]
fn toggling_done_preserves_selection_in_project_view() {
    let root = temp_path("done-project-selection");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();
    fs::write(root.join("b.txt"), "Plan reunion +Family\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));
    assert_eq!(
        session.selected_task().unwrap().task.description,
        "Call Mom +Family"
    );

    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("x").unwrap();

    let selected = session.selected_task().unwrap();
    assert_eq!(selected.task.description, "Call Mom +Family");
    assert!(selected.task.done);
}

#[test]
fn toggling_done_preserves_selected_duplicate_file_in_project_view() {
    let root = temp_path("done-project-duplicate-selection");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();
    fs::write(root.join("b.txt"), "Call Mom +Family\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("j").unwrap();
    assert_eq!(session.selected_task().unwrap().id.file_name(), "b.txt");

    session.dispatch_key("x").unwrap();

    let selected = session.selected_task().unwrap();
    assert_eq!(selected.id.file_name(), "b.txt");
    assert!(selected.task.done);
}

#[test]
fn restoring_done_task_preserves_selection_in_project_view() {
    let root = temp_path("restore-project-selection");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Alpha +Family\n").unwrap();
    fs::write(root.join("done.txt.d/b.txt"), "x 2026-03-29 Zulu +Family\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("j").unwrap();
    assert_eq!(
        session.selected_task().unwrap().task.description,
        "Zulu +Family"
    );

    session.dispatch_key("r").unwrap();

    let selected = session.selected_task().unwrap();
    assert_eq!(selected.task.description, "Zulu +Family");
    assert!(!selected.task.done);
}

#[test]
fn restoring_done_preserves_selected_duplicate_file_in_project_view() {
    let root = temp_path("restore-project-duplicate-selection");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();
    fs::write(
        root.join("done.txt.d/b.txt"),
        "x 2026-03-29 Call Mom +Family\n",
    )
    .unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Family".into()));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("j").unwrap();
    assert_eq!(session.selected_task().unwrap().id.file_name(), "b.txt");

    session.dispatch_key("r").unwrap();

    let selected = session.selected_task().unwrap();
    assert_eq!(selected.id.file_name(), "b.txt");
    assert!(!selected.task.done);
}

#[test]
fn editing_open_task_to_done_moves_it_into_done_view_coherently() {
    let root = temp_path("edit-open-to-done");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("e").unwrap();
    session
        .app_mut()
        .editor
        .as_mut()
        .expect("editor should open")
        .set_raw_line("x 2026-03-30 Call Mom");
    session.dispatch_key("enter").unwrap();

    assert!(!root.join("a.txt").exists());
    assert!(root.join("done.txt.d/a.txt").exists());

    session.select_sidebar_item(SidebarItem::SmartList(1));
    assert_eq!(session.visible_tasks().len(), 1);
    assert!(session.visible_tasks()[0].task.done);

    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("r").unwrap();
    assert!(root.join("a.txt").exists());
    assert!(!root.join("done.txt.d/a.txt").exists());
}

#[test]
fn editing_done_task_to_open_moves_it_back_into_open_view_coherently() {
    let root = temp_path("edit-done-to-open");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("done.txt.d/a.txt"), "x 2026-03-29 Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::SmartList(1));
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    session.dispatch_key("e").unwrap();
    session
        .app_mut()
        .editor
        .as_mut()
        .expect("editor should open")
        .set_raw_line("Call Mom");
    session.dispatch_key("enter").unwrap();

    assert!(root.join("a.txt").exists());
    assert!(!root.join("done.txt.d/a.txt").exists());
    session.select_sidebar_item(SidebarItem::SmartList(0));
    assert_eq!(session.visible_tasks().len(), 1);
    assert!(!session.visible_tasks()[0].task.done);
}

#[test]
fn manual_refresh_picks_up_external_file_changes_and_preserves_selection() {
    let root = temp_path("refresh");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;
    fs::write(root.join("b.txt"), "Ship package +Errands\n").unwrap();

    session.dispatch_key("R").unwrap();

    assert_eq!(session.visible_tasks().len(), 2);
    assert_eq!(
        session.selected_task().unwrap().task.description,
        "Call Mom +Family"
    );
}

#[test]
fn refresh_recovers_when_active_project_filter_disappears() {
    let root = temp_path("refresh-project-filter-disappears");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family\n").unwrap();
    fs::write(root.join("b.txt"), "Plan trip +Travel\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Project("+Travel".into()));
    assert_eq!(session.visible_tasks().len(), 1);

    fs::remove_file(root.join("b.txt")).unwrap();
    session.dispatch_key("R").unwrap();

    assert_eq!(
        session.active_sidebar_item(),
        SidebarItem::SmartList(0)
    );
    assert!(
        !session
            .sidebar_items()
            .contains(&SidebarItem::Project("+Travel".into()))
    );
    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(
        session.visible_tasks()[0].task.description,
        "Call Mom +Family"
    );
}

#[test]
fn sidebar_items_include_separators_between_groups() {
    let root = temp_path("sidebar-separators");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();

    let session = TuiSession::open(root, "2026-03-30").unwrap();

    let items = session.sidebar_items();
    assert!(
        items.contains(&SidebarItem::Separator),
        "sidebar should contain separators"
    );

    // Separator should appear between Done and ProjectsHeader
    let done_pos = items
        .iter()
        .position(|item| *item == SidebarItem::SmartList(1))
        .unwrap();
    let projects_header_pos = items
        .iter()
        .position(|item| *item == SidebarItem::ProjectsHeader)
        .unwrap();
    assert_eq!(
        items[done_pos + 1],
        SidebarItem::Separator,
        "separator should be between Done and Projects header"
    );
    assert_eq!(done_pos + 2, projects_header_pos);
}

#[test]
fn refresh_recovers_when_active_context_filter_disappears() {
    let root = temp_path("refresh-context-filter-disappears");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom @phone\n").unwrap();
    fs::write(root.join("b.txt"), "Book table @town\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.select_sidebar_item(SidebarItem::Context("@town".into()));
    assert_eq!(session.visible_tasks().len(), 1);

    fs::remove_file(root.join("b.txt")).unwrap();
    session.dispatch_key("R").unwrap();

    assert_eq!(
        session.active_sidebar_item(),
        SidebarItem::SmartList(0)
    );
    assert!(
        !session
            .sidebar_items()
            .contains(&SidebarItem::Context("@town".into()))
    );
    assert_eq!(session.visible_tasks().len(), 1);
    assert_eq!(
        session.visible_tasks()[0].task.description,
        "Call Mom @phone"
    );
}

#[test]
fn can_auto_refresh_is_false_when_editor_is_open() {
    let root = temp_path("auto-refresh-editor");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    assert!(session.can_auto_refresh());

    session.dispatch_key("a").unwrap();
    assert!(!session.can_auto_refresh());

    session.dispatch_key("esc").unwrap();
    assert!(session.can_auto_refresh());
}

#[test]
fn can_auto_refresh_is_false_when_delete_confirm_is_active() {
    let root = temp_path("auto-refresh-delete");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    session.app_mut().focus = ttd::tui::app::FocusArea::TaskList;

    session.dispatch_key("D").unwrap();
    assert!(!session.can_auto_refresh());

    session.dispatch_key("esc").unwrap();
    assert!(session.can_auto_refresh());
}

#[test]
fn can_auto_refresh_is_false_in_welcome_mode() {
    let session = TuiSession::welcome("2026-03-30");
    assert!(!session.can_auto_refresh());
}

#[test]
fn poll_refresh_picks_up_externally_created_file() {
    let root = temp_path("poll-created");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    assert_eq!(session.visible_tasks().len(), 1);

    fs::write(root.join("b.txt"), "Ship package\n").unwrap();
    let changed = session.poll_refresh().unwrap();

    assert!(changed);
    assert_eq!(session.visible_tasks().len(), 2);
}

#[test]
fn poll_refresh_picks_up_externally_deleted_file() {
    let root = temp_path("poll-deleted");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    assert_eq!(session.visible_tasks().len(), 2);

    fs::remove_file(root.join("b.txt")).unwrap();
    let changed = session.poll_refresh().unwrap();

    assert!(changed);
    assert_eq!(session.visible_tasks().len(), 1);
}

#[test]
fn poll_refresh_is_noop_when_nothing_changed() {
    let root = temp_path("poll-noop");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    let changed = session.poll_refresh().unwrap();

    assert!(!changed);
    assert_eq!(session.visible_tasks().len(), 1);
}

#[test]
fn poll_refresh_is_noop_when_editor_is_open() {
    let root = temp_path("poll-editor-open");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root.clone(), "2026-03-30").unwrap();
    session.dispatch_key("a").unwrap(); // opens editor

    fs::write(root.join("b.txt"), "Ship package\n").unwrap();
    let changed = session.poll_refresh().unwrap();

    assert!(!changed);
    assert_eq!(session.visible_tasks().len(), 1);
}
