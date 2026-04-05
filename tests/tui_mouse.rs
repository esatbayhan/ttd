use std::fs;
use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ttd::tui::mouse::{DoubleClickTracker, MouseAction, resolve_mouse_action, resolve_scroll_action};
use ttd::tui::render::{LayoutRects, Rects, render_session_frame_with_layout};
use ttd::tui::app::FocusArea;
use ttd::tui::session::{SidebarItem, TuiSession};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-mouse-{}-{}", name, std::process::id()));
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

#[test]
fn layout_rects_are_populated_after_render() {
    let root = temp_path("layout-rects");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Call Mom +Family @phone\n").unwrap();

    let session = TuiSession::open(root, "2026-03-30").unwrap();
    let layout = LayoutRects::default();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render_session_frame_with_layout(frame, &session, &layout))
        .unwrap();

    let rects = layout.get().expect("layout rects should be populated after render");
    assert!(rects.sidebar.width > 0, "sidebar width should be non-zero");
    assert!(rects.task_pane.width > 0, "task_pane width should be non-zero");
}

#[test]
fn click_on_sidebar_item_returns_select_sidebar() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 5,
        sidebar_offset: 0,
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };
    let sidebar_items = vec![
        SidebarItem::SmartList(0),
        SidebarItem::SmartList(1),
        SidebarItem::Separator,
        SidebarItem::ProjectsHeader,
        SidebarItem::Project("+Family".to_string()),
    ];

    // y=1 is first item (y=0 is border)
    let action = resolve_mouse_action(5, 1, &rects, &sidebar_items);
    assert_eq!(action, Some(MouseAction::SelectSidebar(0)));

    // y=2 → second item
    let action = resolve_mouse_action(5, 2, &rects, &sidebar_items);
    assert_eq!(action, Some(MouseAction::SelectSidebar(1)));

    // Separator (index 2, y=3) → None
    let action = resolve_mouse_action(5, 3, &rects, &sidebar_items);
    assert_eq!(action, None);

    // Header (index 3, y=4) → None
    let action = resolve_mouse_action(5, 4, &rects, &sidebar_items);
    assert_eq!(action, None);

    // Project (index 4, y=5)
    let action = resolve_mouse_action(5, 5, &rects, &sidebar_items);
    assert_eq!(action, Some(MouseAction::SelectSidebar(4)));
}

#[test]
fn click_on_task_pane_returns_click_task_pane() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 3,
        sidebar_offset: 0,
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };
    let sidebar_items = vec![
        SidebarItem::SmartList(0),
        SidebarItem::SmartList(1),
        SidebarItem::Separator,
    ];

    // Inside task pane (x=30, y=1)
    let action = resolve_mouse_action(30, 1, &rects, &sidebar_items);
    assert_eq!(action, Some(MouseAction::ClickTaskPane { row: 0 }));

    // Border (y=0) → None
    let action = resolve_mouse_action(30, 0, &rects, &sidebar_items);
    assert_eq!(action, None);
}

#[test]
fn click_outside_both_panes_returns_none() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 3,
        sidebar_offset: 0,
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };
    let sidebar_items = vec![
        SidebarItem::SmartList(0),
        SidebarItem::SmartList(1),
        SidebarItem::Separator,
    ];

    // Help bar area (y=23)
    let action = resolve_mouse_action(5, 23, &rects, &sidebar_items);
    assert_eq!(action, None);
}

#[test]
fn scroll_in_task_pane() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 3,
        sidebar_offset: 0,
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };

    let action = resolve_scroll_action(30, 5, &rects, -1);
    assert_eq!(
        action,
        Some(MouseAction::Scroll {
            in_task_pane: true,
            delta: -1
        })
    );
}

#[test]
fn scroll_in_sidebar() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 3,
        sidebar_offset: 0,
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };

    let action = resolve_scroll_action(5, 5, &rects, 1);
    assert_eq!(
        action,
        Some(MouseAction::Scroll {
            in_task_pane: false,
            delta: 1
        })
    );
}

#[test]
fn scroll_outside_panes_returns_none() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 3,
        sidebar_offset: 0,
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };

    let action = resolve_scroll_action(5, 23, &rects, 1);
    assert_eq!(action, None);
}

#[test]
fn dispatch_mouse_sidebar_click_switches_view_and_focus() {
    let root = temp_path("mouse-sidebar");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Buy milk +Family\n").unwrap();
    fs::write(root.join("b.txt"), "File taxes +Admin\n").unwrap();

    let mut session = TuiSession::open(root, "2026-04-04").unwrap();
    assert_eq!(session.active_sidebar_item(), SidebarItem::SmartList(0));

    let admin_index = session
        .sidebar_items()
        .iter()
        .position(|item| *item == SidebarItem::Project("+Admin".to_string()))
        .expect("+Admin should be in sidebar");

    session.dispatch_mouse_sidebar(admin_index);

    assert_eq!(
        session.active_sidebar_item(),
        SidebarItem::Project("+Admin".to_string())
    );
    assert_eq!(session.app().focus, FocusArea::Sidebar);
}

#[test]
fn dispatch_mouse_task_click_selects_task_and_focuses_pane() {
    let root = temp_path("mouse-task");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "First task\n").unwrap();
    fs::write(root.join("b.txt"), "Second task\n").unwrap();

    let mut session = TuiSession::open(root, "2026-04-04").unwrap();
    assert_eq!(session.app().focus, FocusArea::Sidebar);

    session.dispatch_mouse_task_select(1);

    assert_eq!(session.app().focus, FocusArea::TaskList);
    let selected = session.selected_task().unwrap();
    assert_eq!(session.visible_tasks()[1].id, selected.id);
}

#[test]
fn scroll_wheel_changes_task_scroll_offset() {
    let root = temp_path("mouse-scroll");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    for i in 0..30 {
        fs::write(root.join(format!("{i:02}.txt")), format!("Task number {i}\n")).unwrap();
    }

    let mut session = TuiSession::open(root, "2026-04-04").unwrap();

    assert_eq!(session.task_scroll_offset(), 0);

    // 30 tasks, each ~2 lines (task + separator) = ~59 visual lines
    session.apply_task_scroll(3, 60, 20);
    assert_eq!(session.task_scroll_offset(), 3);

    session.apply_task_scroll(-2, 60, 20);
    assert_eq!(session.task_scroll_offset(), 1);

    // Clamp at 0
    session.apply_task_scroll(-10, 60, 20);
    assert_eq!(session.task_scroll_offset(), 0);
}

#[test]
fn double_click_tracker_detects_double_click() {
    let mut tracker = DoubleClickTracker::new();
    assert!(!tracker.record(0)); // first click
    assert!(tracker.record(0));  // double click
    assert!(!tracker.record(0)); // reset, so single click again
}

#[test]
fn double_click_tracker_rejects_different_task() {
    let mut tracker = DoubleClickTracker::new();
    assert!(!tracker.record(0));
    assert!(!tracker.record(1)); // different task, not a double click
}

#[test]
fn sidebar_click_with_scroll_offset_maps_correctly() {
    let rects = Rects {
        sidebar: Rect::new(0, 0, 24, 22),
        task_pane: Rect::new(24, 0, 56, 22),
        sidebar_item_count: 10,
        sidebar_offset: 3, // scrolled down by 3
        task_pane_inner_width: 54,
        visual_line_count: 0,
        pane_height: 20,
        task_scroll_offset: 0,
    };
    let sidebar_items: Vec<SidebarItem> = (0..10)
        .map(|i| SidebarItem::SmartList(i))
        .collect();

    // Click on first visible row (y=1) with offset 3 → item index 3
    let action = resolve_mouse_action(5, 1, &rects, &sidebar_items);
    assert_eq!(action, Some(MouseAction::SelectSidebar(3)));
}

#[test]
fn scroll_clamps_to_max_offset() {
    // 50 visual lines, pane shows 20 → max offset is 30
    let root = temp_path("mouse-clamp-max");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Task\n").unwrap();

    let mut session = TuiSession::open(root, "2026-04-04").unwrap();

    session.apply_task_scroll(100, 50, 20);
    assert_eq!(session.task_scroll_offset(), 30);

    // Further scrolling stays clamped
    session.apply_task_scroll(10, 50, 20);
    assert_eq!(session.task_scroll_offset(), 30);
}

#[test]
fn mouse_scroll_task_pane_clamps_to_zero() {
    let root = temp_path("mouse-clamp");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    fs::write(root.join("a.txt"), "Only task\n").unwrap();

    let mut session = TuiSession::open(root, "2026-04-04").unwrap();

    session.apply_task_scroll(-5, 10, 20);
    assert_eq!(session.task_scroll_offset(), 0);
}

#[test]
fn task_click_on_empty_list_does_nothing() {
    let root = temp_path("mouse-empty");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    write_standard_lists(&root);
    // No task files — empty list

    let mut session = TuiSession::open(root, "2026-04-04").unwrap();

    let result = session.task_index_for_visual_row(0, 54);
    assert_eq!(result, None);

    // dispatch_mouse_task_select with out-of-bounds should be safe
    session.dispatch_mouse_task_select(5);
    assert!(session.selected_task().is_none());
}
