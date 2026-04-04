use std::fs;
use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ttd::store::TaskId;
use ttd::tui::app::{AppAction, AppMode, AppState, FocusArea};
use ttd::tui::editor::{EditorState, SaveConflictState, SelectedTask};
use ttd::tui::events::normalize_key;
use ttd::tui::render::{compute_scroll_offset, render_frame, render_session_frame};
use ttd::tui::session::TuiSession;
use ttd::tui::widgets::{help_bar_text, task_line_text};

fn render(app: &AppState) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render_frame(frame, app)).unwrap();

    terminal.backend().buffer().clone()
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("")
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-render-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

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

fn render_text(session: &TuiSession) -> String {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, session))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn welcome_screen_renders_when_no_directory_is_configured() {
    let app = AppState::new(AppMode::Welcome);

    let buffer = render(&app);

    assert!(buffer.content().iter().any(|cell| cell.symbol() == "W"));
    let text = buffer_text(&buffer);
    assert!(text.contains("Welcome to ttd"));
    assert!(text.contains("todo.txt.d"));
    assert!(text.contains("Path"));
}

#[test]
fn main_shell_renders_sidebar_entries() {
    let app = AppState::new(AppMode::Main);

    let text = buffer_text(&render(&app));

    for label in ["Projects", "Contexts"] {
        assert!(text.contains(label), "missing label: {label}");
    }
}

#[test]
fn main_render_shows_sidebar_labels_and_real_task_rows() {
    let root = temp_path("real-rows");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();

    let session = TuiSession::open(root, "2026-03-30").unwrap();
    let text = render_text(&session);

    assert!(text.contains("Inbox"));
    assert!(text.contains("PROJECTS"));
    assert!(text.contains("+Family"));
    assert!(text.contains("@phone"));
    assert!(text.contains("Call Mom +Family @phone"));
}

#[test]
fn session_render_preserves_search_delete_editor_and_conflict_states() {
    let root = temp_path("stateful-render");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    session.app_mut().search_active = true;
    session.app_mut().confirm_delete = true;
    session.app_mut().editor = Some(EditorState::quick_entry());
    session.app_mut().save_conflict = Some(SaveConflictState {
        external_raw: "External version".into(),
    });

    let text = render_text(&session);

    assert!(text.contains("Search:"));
    assert!(text.contains("Quick Entry"));
    assert!(text.contains("Save Conflict"));
}

#[test]
fn session_render_shows_selected_task_and_updates_after_navigation() {
    let root = temp_path("selected-task");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package +Errands @town\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-30").unwrap();
    session.app_mut().focus = FocusArea::TaskList;

    let initial = render_text(&session);
    assert!(initial.contains("> Call Mom +Family @phone"));
    assert!(!initial.contains("> Ship package +Errands @town"));

    session.dispatch_key("j").unwrap();

    let moved = render_text(&session);
    assert!(moved.contains("> Ship package +Errands @town"));
    assert!(!moved.contains("> Call Mom +Family @phone"));
}

#[test]
fn session_render_shows_active_search_query_and_matching_rows() {
    let root = temp_path("render-search");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("b.txt"), "Email Alex\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.dispatch_key("/").unwrap();
    for key in ["C", "a", "l", "l"] {
        session.dispatch_key(key).unwrap();
    }

    let text = render_text(&session);
    assert!(text.contains("Search: Call"));
    assert!(text.contains("Call Mom"));
    assert!(!text.contains("Email Alex"));
}

#[test]
fn tab_rotates_focus_between_sidebar_and_list() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.focus, FocusArea::Sidebar);

    app.handle_key("tab");
    assert_eq!(app.focus, FocusArea::TaskList);

    app.handle_key("tab");
    assert_eq!(app.focus, FocusArea::Sidebar);
}

#[test]
fn h_and_l_move_focus_horizontally() {
    let mut app = AppState::new(AppMode::Main);

    app.handle_key("l");
    assert_eq!(app.focus, FocusArea::TaskList);

    app.handle_key("l");
    assert_eq!(app.focus, FocusArea::Sidebar);

    app.handle_key("h");
    assert_eq!(app.focus, FocusArea::TaskList);
}

#[test]
fn slash_enters_search_mode_and_escape_clears_it() {
    let mut app = AppState::new(AppMode::Main);

    assert!(!app.search_active);

    let action = app.handle_key("/");
    assert_eq!(action, Some(AppAction::EnterSearch));
    assert!(app.search_active);

    let action = app.handle_key("esc");
    assert_eq!(action, Some(AppAction::Cancel));
    assert!(!app.search_active);
}

#[test]
fn search_mode_owns_text_input_and_suppresses_normal_shortcuts() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("/"), Some(AppAction::EnterSearch));
    assert_eq!(
        app.handle_key("a"),
        Some(AppAction::AppendToSearch("a".into()))
    );
    assert_eq!(app.search_query, "a");
    assert_eq!(
        app.handle_key("j"),
        Some(AppAction::AppendToSearch("j".into()))
    );
    assert_eq!(app.search_query, "aj");
    assert_eq!(
        app.handle_key("backspace"),
        Some(AppAction::BackspaceSearch)
    );
    assert_eq!(app.search_query, "a");
    assert_eq!(app.handle_key("tab"), None);
}

#[test]
fn delete_confirmation_owns_input_until_cancelled() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("D"), Some(AppAction::ConfirmDelete));
    assert!(app.confirm_delete);
    assert_eq!(app.handle_key("j"), None);
    assert_eq!(app.handle_key("a"), None);
    assert_eq!(app.handle_key("escape"), Some(AppAction::Cancel));
    assert!(!app.confirm_delete);
}

#[test]
fn delete_confirmation_accepts_enter_to_confirm() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("D"), Some(AppAction::ConfirmDelete));
    assert_eq!(app.handle_key("enter"), Some(AppAction::OpenSelected));
}

#[test]
fn welcome_screen_collects_path_input_and_submits() {
    let mut app = AppState::new(AppMode::Welcome);

    for key in ["/", "t", "m", "p"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToInput(key.into()))
        );
    }

    assert_eq!(app.welcome_input, "/tmp");
    assert_eq!(
        app.handle_key("enter"),
        Some(AppAction::SubmitWelcomePath("/tmp".into()))
    );
}

#[test]
fn welcome_screen_backspace_removes_last_character() {
    let mut app = AppState::new(AppMode::Welcome);

    app.handle_key("a");
    app.handle_key("b");

    assert_eq!(app.handle_key("backspace"), Some(AppAction::BackspaceInput));
    assert_eq!(app.welcome_input, "a");
}

#[test]
fn handle_key_consumes_normalized_aliases() {
    let mut welcome = AppState::new(AppMode::Welcome);
    welcome.handle_key("a");
    welcome.handle_key("b");
    assert_eq!(
        welcome.handle_key("return"),
        Some(AppAction::SubmitWelcomePath("ab".into()))
    );

    let mut main = AppState::new(AppMode::Main);
    assert_eq!(main.handle_key("tab"), Some(AppAction::FocusNext));
    assert_eq!(main.focus, FocusArea::TaskList);
    assert_eq!(main.handle_key("escape"), None);
    assert_eq!(main.handle_key("/"), Some(AppAction::EnterSearch));
    assert_eq!(main.handle_key("bs"), Some(AppAction::BackspaceSearch));
    assert_eq!(main.handle_key("escape"), Some(AppAction::Cancel));
}

#[test]
fn navigation_and_task_shortcuts_are_routed_to_actions() {
    let cases = [
        ("j", Some(AppAction::MoveDown)),
        ("k", Some(AppAction::MoveUp)),
        ("gg", Some(AppAction::MoveTop)),
        ("G", Some(AppAction::MoveBottom)),
        ("enter", Some(AppAction::OpenSelected)),
        ("a", Some(AppAction::AddTask)),
        ("e", Some(AppAction::EditTask)),
        ("x", Some(AppAction::ToggleDone)),
        ("r", Some(AppAction::RestoreCompleted)),
        ("R", Some(AppAction::Refresh)),
        ("n", Some(AppAction::NextSearchResult)),
        ("N", Some(AppAction::PreviousSearchResult)),
    ];

    for (key, expected) in cases {
        let mut app = AppState::new(AppMode::Main);
        if key == "e" {
            app.selected_task = Some(SelectedTask::new(
                TaskId {
                    path: PathBuf::from("/tmp/inbox.txt"),
                    line_index: 0,
                },
                "Call Mom",
            ));
        }
        assert_eq!(app.handle_key(key), expected, "key: {key}");
    }
}

#[test]
fn delete_shortcut_enters_confirmation_state_instead_of_deleting_immediately() {
    let mut app = AppState::new(AppMode::Main);

    assert!(!app.confirm_delete);
    assert_eq!(app.handle_key("D"), Some(AppAction::ConfirmDelete));
    assert!(app.confirm_delete);
    assert_eq!(app.handle_key("esc"), Some(AppAction::Cancel));
    assert!(!app.confirm_delete);
}

#[test]
fn main_shell_renders_delete_confirmation_message() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("D");

    let text = buffer_text(&render(&app));

    assert!(text.contains("confirm"));
    assert!(text.contains("cancel"));
}

#[test]
fn session_render_shows_delete_confirmation_for_selected_task() {
    let root = temp_path("render-delete");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.app_mut().focus = FocusArea::TaskList;
    session.dispatch_key("D").unwrap();

    let text = render_text(&session);
    assert!(text.contains("confirm"));
    assert!(text.contains("cancel"));
}

#[test]
fn normalize_key_leaves_supported_tokens_stable() {
    for key in ["tab", "enter", "backspace", "esc", "gg", "G", "/", "j", "R"] {
        assert_eq!(normalize_key(key), key);
    }
}

#[test]
fn normalize_key_maps_common_aliases() {
    let aliases = [
        ("return", "enter"),
        ("escape", "esc"),
        ("bs", "backspace"),
        ("del", "backspace"),
    ];

    for (raw, normalized) in aliases {
        assert_eq!(normalize_key(raw), normalized);
    }
}

#[test]
fn q_returns_quit_action_at_root_level() {
    let mut app = AppState::new(AppMode::Main);
    let action = app.handle_key("q");
    assert_eq!(action, Some(AppAction::Quit));
    assert!(app.should_quit);
}

#[test]
fn esc_is_noop_at_root_level() {
    let mut app = AppState::new(AppMode::Main);
    let action = app.handle_key("esc");
    assert_eq!(action, None);
    assert!(!app.should_quit);
}

#[test]
fn q_does_not_quit_during_search() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("/");
    let action = app.handle_key("q");
    assert_eq!(action, Some(AppAction::AppendToSearch("q".into())));
    assert!(!app.should_quit);
}

#[test]
fn q_does_not_quit_during_editor() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("a");
    let action = app.handle_key("q");
    assert_eq!(action, Some(AppAction::AppendToEditor("q".into())));
    assert!(!app.should_quit);
}

#[test]
fn empty_editor_submit_cancels_instead_of_submitting() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("a");
    assert!(app.editor.is_some());
    let action = app.handle_key("enter");
    assert_eq!(action, Some(AppAction::Cancel));
    assert!(app.editor.is_none());
}

#[test]
fn whitespace_only_editor_submit_cancels() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("a");
    app.handle_key(" ");
    app.handle_key(" ");
    let action = app.handle_key("enter");
    assert_eq!(action, Some(AppAction::Cancel));
    assert!(app.editor.is_none());
}

#[test]
fn nonempty_editor_submit_returns_submit_action() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("a");
    app.handle_key("H");
    app.handle_key("i");
    let action = app.handle_key("enter");
    assert_eq!(action, Some(AppAction::SubmitEditor));
}

#[test]
fn help_bar_shows_navigation_keys_at_default_state() {
    let app = AppState::new(AppMode::Main);
    let text = help_bar_text(&app);
    assert!(text.contains("j/k"), "should contain j/k nav");
    assert!(text.contains("nav"), "should contain nav label");
    assert!(text.contains("h/l"), "should contain h/l focus");
    assert!(text.contains("focus"), "should contain focus label");
}

#[test]
fn help_bar_shows_root_shortcuts_at_default_state() {
    let app = AppState::new(AppMode::Main);
    let text = help_bar_text(&app);
    assert!(text.contains("a"));
    assert!(text.contains("add"));
    assert!(text.contains("q"));
    assert!(text.contains("quit"));
}

#[test]
fn help_bar_shows_search_shortcuts_during_search() {
    let mut app = AppState::new(AppMode::Main);
    app.search_active = true;
    let text = help_bar_text(&app);
    assert!(text.contains("esc"));
    assert!(text.contains("cancel"));
    assert!(text.contains("n"));
    assert!(text.contains("next"));
}

#[test]
fn help_bar_shows_editor_shortcuts_when_editor_is_open() {
    let mut app = AppState::new(AppMode::Main);
    app.editor = Some(EditorState::quick_entry());
    let text = help_bar_text(&app);
    assert!(text.contains("enter"));
    assert!(text.contains("save"));
    assert!(text.contains("ctrl+d"));
}

#[test]
fn help_bar_shows_delete_confirm_shortcuts() {
    let mut app = AppState::new(AppMode::Main);
    app.confirm_delete = true;
    let text = help_bar_text(&app);
    assert!(text.contains("enter"));
    assert!(text.contains("confirm"));
    assert!(text.contains("esc"));
    assert!(text.contains("cancel"));
}

#[test]
fn help_bar_shows_conflict_shortcuts() {
    let mut app = AppState::new(AppMode::Main);
    app.save_conflict = Some(SaveConflictState {
        external_raw: "conflict".into(),
    });
    let text = help_bar_text(&app);
    assert!(text.contains("r"));
    assert!(text.contains("reload"));
    assert!(text.contains("o"));
    assert!(text.contains("overwrite"));
}

#[test]
fn session_render_includes_help_bar() {
    let root = temp_path("help-bar");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();

    let session = TuiSession::open(root, "2026-03-31").unwrap();
    let text = render_text(&session);

    assert!(text.contains("add"), "help bar should contain 'add'");
    assert!(text.contains("quit"), "help bar should contain 'quit'");
}

#[test]
fn task_line_strips_date_tags_and_shows_them_below() {
    let task = ttd::parser::parse_task_line("Buy groceries +Personal @home due:2026-04-01");
    let text = task_line_text(&task, true);
    assert!(text.contains("Buy groceries"));
    assert!(text.contains("+Personal"));
    assert!(text.contains("@home"));
    assert!(text.contains("due:"));
    assert!(text.contains("2026-04-01"));
    let lines: Vec<&str> = text.lines().collect();
    assert!(!lines[0].contains("due:2026-04-01"));
}

#[test]
fn task_line_with_no_date_tags_is_single_line() {
    let task = ttd::parser::parse_task_line("Call dentist @phone");
    let text = task_line_text(&task, false);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(text.contains("Call dentist"));
    assert!(text.contains("@phone"));
}

#[test]
fn task_line_shows_multiple_date_tags() {
    let task =
        ttd::parser::parse_task_line("Write report +Work due:2026-03-31 starting:2026-03-28");
    let text = task_line_text(&task, false);
    assert!(text.contains("due:"));
    assert!(text.contains("2026-03-31"));
    assert!(text.contains("start:"));
    assert!(text.contains("2026-03-28"));
    let lines: Vec<&str> = text.lines().collect();
    assert!(!lines[0].contains("due:2026-03-31"));
    assert!(!lines[0].contains("starting:2026-03-28"));
}

#[test]
fn task_line_shows_creation_date_as_tag_card() {
    let task =
        ttd::parser::parse_task_line("2026-03-20 Buy groceries +Personal @home due:2026-04-01");
    let text = task_line_text(&task, true);
    assert!(text.contains("created: 2026-03-20"));
    assert!(text.contains("due: 2026-04-01"));
}

#[test]
fn task_line_omits_creation_date_when_absent() {
    let task = ttd::parser::parse_task_line("Buy groceries +Personal @home");
    let text = task_line_text(&task, false);
    assert!(!text.contains("created:"));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn session_render_shows_divider_lines_between_tasks() {
    let root = temp_path("dividers");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("b.txt"), "Ship package\n").unwrap();

    let session = TuiSession::open(root, "2026-03-31").unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let mut divider_found = false;
    for row in 0..24u16 {
        let mut row_text = String::new();
        for col in 25..80u16 {
            row_text.push_str(buf[(col, row)].symbol());
        }
        if row_text.contains("──────") {
            divider_found = true;
            break;
        }
    }
    assert!(divider_found, "should find divider line in task pane area");
}

#[test]
fn session_render_wraps_long_task_descriptions() {
    let root = temp_path("word-wrap");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(
        root.join("a.txt"),
        "This is a very long task description that should definitely wrap to the next line in the TUI\n",
    )
    .unwrap();

    let session = TuiSession::open(root, "2026-03-31").unwrap();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let mut first_row = None;
    let mut wrap_row = None;
    for row in 0..24u16 {
        let mut row_text = String::new();
        for col in 25..80u16 {
            row_text.push_str(buf[(col, row)].symbol());
        }
        if row_text.contains("This is") {
            first_row = Some(row);
        }
        if row_text.contains("wrap") && first_row.is_some() {
            wrap_row = Some(row);
        }
    }
    assert!(first_row.is_some(), "should find start of task text");
    assert!(
        wrap_row.is_some(),
        "wrapped text should appear on a subsequent row"
    );
    assert!(
        wrap_row.unwrap() > first_row.unwrap(),
        "wrap should be on a later row"
    );
}

#[test]
fn session_render_scrolls_to_keep_selected_task_visible() {
    let root = temp_path("scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    for i in 0..20 {
        fs::write(
            root.join(format!("{:02}.txt", i)),
            format!("Task number {i}\n"),
        )
        .unwrap();
    }

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.app_mut().focus = FocusArea::TaskList;

    for _ in 0..19 {
        session.dispatch_key("j").unwrap();
    }

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let mut found_last = false;
    for row in 0..24u16 {
        let mut row_text = String::new();
        for col in 25..80u16 {
            row_text.push_str(buf[(col, row)].symbol());
        }
        if row_text.contains("Task number 19") {
            found_last = true;
            break;
        }
    }
    assert!(
        found_last,
        "last task should be visible after scrolling down"
    );
}

#[test]
fn compute_scroll_offset_accounts_for_word_wrap() {
    use ratatui::text::Line;

    // 3 lines, each 30 chars wide, displayed in a 10-char-wide pane
    // With word wrap, "aaaa..." (no spaces) breaks mid-word every 10 chars = 3 visual rows each
    let lines: Vec<Line> = vec![
        Line::raw("a".repeat(30)), // 3 visual rows
        Line::raw("b".repeat(30)), // 3 visual rows
        Line::raw("c".repeat(30)), // 3 visual rows, selected
    ];

    // Select last line (index 2), pane height 5
    // Total visual rows up to selected = 9, offset = 9 - 5 = 4
    let offset = compute_scroll_offset(&lines, Some(2), 10, 5);
    assert_eq!(offset, 4);
}

#[test]
fn compute_scroll_offset_no_scroll_when_fits() {
    use ratatui::text::Line;

    let lines: Vec<Line> = vec![Line::raw("short"), Line::raw("also short")];
    let offset = compute_scroll_offset(&lines, Some(1), 40, 10);
    assert_eq!(offset, 0);
}

#[test]
fn compute_scroll_offset_handles_word_boundary_wrapping() {
    use ratatui::text::Line;

    // "hello world" at width 8: "hello" (5) + " " (1) + "world" (5) = 11 total
    // Word wrap: "hello " fits in 8, "world" wraps to next line = 2 visual rows
    let lines: Vec<Line> = vec![
        Line::raw("hello world"),
        Line::raw("foo bar baz"), // "foo bar" fits (7), "baz" wraps = 2 visual rows
    ];

    // Select line 1, pane height 2
    // Lines up to index 1: 2 + 2 = 4 visual rows, offset = 4 - 2 = 2
    let offset = compute_scroll_offset(&lines, Some(1), 8, 2);
    assert_eq!(offset, 2);
}

#[test]
fn sidebar_scrollbar_appears_when_items_overflow() {
    let root = temp_path("sidebar-scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    // Create enough projects to overflow a short terminal (10 rows tall)
    for i in 0..20 {
        fs::write(
            root.join(format!("proj{i:02}.txt")),
            format!("Task {i} +Project{i}\n"),
        )
        .unwrap();
    }

    let session = TuiSession::open(root, "2026-04-01").unwrap();

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    // The scrollbar track character "│" or thumb "█" should appear
    // in the rightmost column of the sidebar area (column 23, inside border).
    let buf = terminal.backend().buffer();
    let sidebar_right_col = 23u16;
    let mut found_thumb = false;
    for row in 1..9u16 {
        let symbol = buf[(sidebar_right_col, row)].symbol();
        if symbol == "█" || symbol == "▐" || symbol == "░" {
            found_thumb = true;
            break;
        }
    }
    assert!(found_thumb, "expected scrollbar thumb in sidebar right edge");
}

#[test]
fn task_pane_scrollbar_appears_when_tasks_overflow() {
    let root = temp_path("task-scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    for i in 0..20 {
        fs::write(
            root.join(format!("{i:02}.txt")),
            format!("Task number {i}\n"),
        )
        .unwrap();
    }

    let mut session = TuiSession::open(root, "2026-04-01").unwrap();
    session.app_mut().focus = FocusArea::TaskList;

    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    // The scrollbar should appear on the rightmost column of the task pane
    // (column 79, inside the right border).
    let buf = terminal.backend().buffer();
    let task_pane_right_col = 79u16;
    let mut found_thumb = false;
    for row in 1..11u16 {
        let symbol = buf[(task_pane_right_col, row)].symbol();
        if symbol == "█" || symbol == "▐" || symbol == "░" {
            found_thumb = true;
            break;
        }
    }
    assert!(found_thumb, "expected scrollbar thumb in task pane right edge");
}

#[test]
fn session_render_scroll_keeps_selected_visible_in_narrow_terminal() {
    let root = temp_path("narrow-scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    // Create tasks with long descriptions that will wrap in a narrow terminal
    for i in 0..10 {
        let long_text = format!(
            "Task {i} with a very long description that should definitely wrap in a narrow terminal window aaa bbb ccc ddd eee fff"
        );
        fs::write(root.join(format!("{:02}.txt", i)), format!("{long_text}\n")).unwrap();
    }

    let mut session = TuiSession::open(root, "2026-03-31").unwrap();
    session.app_mut().focus = FocusArea::TaskList;

    // Navigate to the last task
    for _ in 0..9 {
        session.dispatch_key("j").unwrap();
    }

    // Render in a narrow terminal (50 wide, 15 tall)
    let backend = TestBackend::new(50, 15);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let mut found_marker = false;
    for row in 0..15u16 {
        let mut row_text = String::new();
        for col in 25..50u16 {
            row_text.push_str(buf[(col, row)].symbol());
        }
        if row_text.contains(">") && row_text.contains("Task 9") {
            found_marker = true;
            break;
        }
    }
    assert!(
        found_marker,
        "selected task marker '>' for Task 9 should be visible in narrow terminal"
    );
}

#[test]
fn sidebar_scrollbar_hidden_when_items_fit() {
    let root = temp_path("sidebar-no-scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Only task +Proj @ctx\n").unwrap();

    let session = TuiSession::open(root, "2026-04-01").unwrap();

    // Tall terminal — sidebar items easily fit
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let sidebar_right_col = 23u16;
    for row in 1..29u16 {
        let symbol = buf[(sidebar_right_col, row)].symbol();
        assert!(
            symbol != "█" && symbol != "▐" && symbol != "░",
            "unexpected scrollbar thumb at row {row} when content fits"
        );
    }
}

#[test]
fn task_pane_scrollbar_hidden_when_tasks_fit() {
    let root = temp_path("task-no-scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Only task\n").unwrap();

    let session = TuiSession::open(root, "2026-04-01").unwrap();

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let task_pane_right_col = 79u16;
    for row in 1..29u16 {
        let symbol = buf[(task_pane_right_col, row)].symbol();
        assert!(
            symbol != "█" && symbol != "▐" && symbol != "░",
            "unexpected scrollbar thumb at row {row} when tasks fit"
        );
    }
}

#[test]
fn scrollbar_thumb_reaches_bottom_when_last_task_selected() {
    let root = temp_path("scroll-bottom");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    for i in 0..30 {
        fs::write(
            root.join(format!("{i:02}.txt")),
            format!("Task number {i}\n"),
        )
        .unwrap();
    }

    let mut session = TuiSession::open(root, "2026-04-01").unwrap();
    session.app_mut().focus = FocusArea::TaskList;

    // Navigate to the very last task
    for _ in 0..29 {
        session.dispatch_key("j").unwrap();
    }

    let backend = TestBackend::new(80, 14);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame(frame, &session))
        .unwrap();

    let buf = terminal.backend().buffer();
    let task_pane_right_col = 79u16;
    let pane_bottom = 12u16; // 14 - 2 (borders) = 12 inner rows, last inner row is 12

    // Dump the scrollbar column for debugging
    let mut col_dump = String::new();
    for row in 0..14u16 {
        let sym = buf[(task_pane_right_col, row)].symbol();
        col_dump.push_str(&format!("row {row:2}: '{sym}'\n"));
    }

    // The row just above the bottom border end-symbol should be thumb, not track
    // With default ratatui scrollbar: row 1 = ▲, rows 2..11 = track/thumb, row 12 = ▼
    // When at bottom, the thumb should extend to the row just above ▼
    let end_symbol_row = (1..13u16)
        .rev()
        .find(|&r| buf[(task_pane_right_col, r)].symbol() == "▼");
    let thumb_bottom = end_symbol_row.map(|r| r - 1).unwrap_or(pane_bottom);

    let symbol_above_end = buf[(task_pane_right_col, thumb_bottom)].symbol();
    assert!(
        symbol_above_end == "█" || symbol_above_end == "▐" || symbol_above_end == "░",
        "scrollbar thumb should reach the bottom when last task is selected.\n\
         Row {thumb_bottom} symbol: '{symbol_above_end}'\n\
         Column dump:\n{col_dump}"
    );
}

