use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use super::app::{AppMode, AppState, FocusArea};
use super::editor::EditorMode;
use super::session::{SidebarItem, TuiSession};
use super::widgets::{render_help_bar, render_task_lines};

const SIDEBAR_ENTRIES: [&str; 7] = [
    "Inbox",
    "Today",
    "Scheduled",
    "Upcoming",
    "Done",
    "Projects",
    "Contexts",
];

pub fn render_frame(frame: &mut Frame<'_>, app: &AppState) {
    match app.mode {
        AppMode::Welcome => render_welcome(frame, app),
        AppMode::Main => render_main(frame, app),
    }
}

pub fn render_session_frame(frame: &mut Frame<'_>, session: &TuiSession) {
    match session.app().mode {
        AppMode::Welcome => render_welcome(frame, session.app()),
        AppMode::Main => render_session_main(frame, session),
    }
}

fn render_welcome(frame: &mut Frame<'_>, app: &AppState) {
    let text = format!(
        "Welcome to ttd\n\nManage your todo.txt.d directory from one terminal UI.\n\nPath: {}",
        app.welcome_input
    );
    let widget =
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Welcome"));
    frame.render_widget(widget, frame.area());
}

fn render_main(frame: &mut Frame<'_>, app: &AppState) {
    let items = SIDEBAR_ENTRIES
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let style = if index == 0 {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(*label).style(style)
        })
        .collect::<Vec<_>>();

    let task_content = if app.search_active {
        vec![
            Line::raw("Tasks"),
            Line::raw(""),
            Line::raw("Search: active"),
        ]
    } else {
        vec![Line::raw("Tasks")]
    };

    render_main_shell(frame, app, items, task_content, "Filters", "Tasks", None);
    render_overlays(frame, app);
}

fn render_session_main(frame: &mut Frame<'_>, session: &TuiSession) {
    let app = session.app();

    // Pre-compute task pane width for hanging-indent word wrap
    let task_pane_inner_width = {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(frame.area());
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(24)])
            .split(outer[0]);
        chunks[1].width.saturating_sub(2)
    };

    let sidebar = session
        .sidebar_items()
        .iter()
        .map(|item| {
            let style = match item {
                _ if *item == session.active_sidebar_item() => {
                    Style::default().add_modifier(Modifier::BOLD)
                }
                SidebarItem::ProjectsHeader
                | SidebarItem::ContextsHeader
                | SidebarItem::Separator => Style::default().add_modifier(Modifier::DIM),
                _ => Style::default(),
            };
            ListItem::new(sidebar_label(item)).style(style)
        })
        .collect::<Vec<_>>();

    let sidebar_title = active_filter_title(&session.active_sidebar_item());
    let tasks_title = format!("{} ({})", sidebar_title, session.visible_tasks().len());

    let mut task_lines: Vec<Line> = Vec::new();

    let mut selected_line_index: Option<usize> = None;
    let mut line_count: usize = 0;

    if session.visible_tasks().is_empty() {
        task_lines.push(Line::raw("No tasks in this view"));
    } else {
        if !app.search_query.is_empty() {
            task_lines.push(Line::raw(format!("Search: {}", app.search_query)));
            task_lines.push(Line::raw(""));
            line_count += 2;
        } else if app.search_active {
            task_lines.push(Line::raw("Search: "));
            task_lines.push(Line::raw(""));
            line_count += 2;
        }

        let task_count = session.visible_tasks().len();
        for (i, stored) in session.visible_tasks().iter().enumerate() {
            let is_selected = app
                .selected_task
                .as_ref()
                .is_some_and(|selected| selected.id == stored.id);
            let lines = render_task_lines(&stored.task, is_selected, task_pane_inner_width);
            line_count += lines.len();
            task_lines.extend(lines);

            if is_selected {
                selected_line_index = Some(line_count - 1);
            }

            if i < task_count - 1 {
                let sep_width = (task_pane_inner_width as usize).saturating_sub(4);
                task_lines.push(Line::from(Span::styled(
                    format!("  {}", "─".repeat(sep_width)),
                    Style::default().fg(Color::DarkGray),
                )));
                line_count += 1;
            }
        }
    }

    render_main_shell(
        frame,
        app,
        sidebar,
        task_lines,
        &sidebar_title,
        &tasks_title,
        selected_line_index,
    );
    render_overlays(frame, app);
}

fn render_main_shell(
    frame: &mut Frame<'_>,
    app: &AppState,
    sidebar: Vec<ListItem<'_>>,
    task_content: Vec<Line<'_>>,
    sidebar_title: &str,
    tasks_title: &str,
    selected_line_index: Option<usize>,
) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(24)])
        .split(outer[0]);

    frame.render_widget(
        List::new(sidebar).block(panel(sidebar_title, app.focus == FocusArea::Sidebar)),
        chunks[0],
    );

    let pane_height = chunks[1].height.saturating_sub(2) as usize;
    let inner_width = chunks[1].width.saturating_sub(2);
    let scroll_offset =
        compute_scroll_offset(&task_content, selected_line_index, inner_width, pane_height);

    frame.render_widget(
        Paragraph::new(task_content)
            .block(panel(tasks_title, app.focus == FocusArea::TaskList))
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset, 0)),
        chunks[1],
    );

    frame.render_widget(render_help_bar(app), outer[1]);
}

fn render_overlays(frame: &mut Frame<'_>, app: &AppState) {
    if let Some(editor) = app.editor.as_ref() {
        let modal = centered_rect(frame.area(), 68, 12);
        let title = match editor.mode {
            EditorMode::QuickEntry => "Quick Entry",
            EditorMode::Edit => "Edit Task",
        };
        let shortcut_text = if let Some(shortcut) = editor.shortcut.as_ref() {
            let error_text = shortcut
                .error
                .as_deref()
                .map(|error| format!("\n{error}"))
                .unwrap_or_default();
            format!(
                "\n\n{} date: {}\nEnter apply | Esc cancel helper{}",
                shortcut.shortcut.label(),
                shortcut.input,
                error_text
            )
        } else {
            "\n\nEnter save | Esc cancel\nctrl+d due\nctrl+s scheduled\nctrl+t starting".to_string()
        };
        let helper_text = format!(
            "due: {}\nscheduled: {}\nstarting: {}{}",
            editor.due.as_deref().unwrap_or("-"),
            editor.scheduled.as_deref().unwrap_or("-"),
            editor.starting.as_deref().unwrap_or("-"),
            shortcut_text
        );
        let text = format!("{}\n\n{}", editor.raw_line, helper_text);
        frame.render_widget(Clear, modal);
        frame.render_widget(
            Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false }),
            modal,
        );

        // Render cursor
        let inner_width = modal.width.saturating_sub(2) as usize;
        if inner_width > 0 {
            if let Some(shortcut) = editor.shortcut.as_ref() {
                // Cursor is on the shortcut input line
                let raw_visual_rows = visual_line_count(&editor.raw_line, inner_width);
                // Lines between raw_line and shortcut: empty, due, scheduled, starting, empty = 5
                let shortcut_base_row = raw_visual_rows + 5;
                let prefix_len = shortcut.shortcut.label().len() + " date: ".len();
                let col_in_line = prefix_len + shortcut.cursor_pos;
                let cursor_row = shortcut_base_row + col_in_line / inner_width;
                let cursor_col = col_in_line % inner_width;
                frame.set_cursor_position((
                    modal.x + 1 + cursor_col as u16,
                    modal.y + 1 + cursor_row as u16,
                ));
            } else {
                // Cursor is on the raw_line
                let cursor_row = editor.cursor_pos / inner_width;
                let cursor_col = editor.cursor_pos % inner_width;
                frame.set_cursor_position((
                    modal.x + 1 + cursor_col as u16,
                    modal.y + 1 + cursor_row as u16,
                ));
            }
        }
    }

    if app.save_conflict.is_some() {
        let dialog = centered_rect(frame.area(), 52, 7);
        frame.render_widget(Clear, dialog);
        frame.render_widget(
            Paragraph::new(
                "Conflict detected\nr reload external version\no overwrite external version\nc cancel and keep local draft",
            )
            .block(Block::default().borders(Borders::ALL).title("Save Conflict")),
            dialog,
        );
    }
}

fn sidebar_label(item: &SidebarItem) -> String {
    match item {
        SidebarItem::Smart(filter) => format!("{filter:?}"),
        SidebarItem::Separator => "──────────────────────".to_string(),
        SidebarItem::ProjectsHeader => "PROJECTS".to_string(),
        SidebarItem::Project(value) => format!("  {value}"),
        SidebarItem::ContextsHeader => "CONTEXTS".to_string(),
        SidebarItem::Context(value) => format!("  {value}"),
    }
}

fn active_filter_title(item: &SidebarItem) -> String {
    match item {
        SidebarItem::Smart(filter) => format!("{filter:?}"),
        SidebarItem::Project(value) => value.clone(),
        SidebarItem::Context(value) => value.clone(),
        SidebarItem::ProjectsHeader => "Projects".to_string(),
        SidebarItem::ContextsHeader => "Contexts".to_string(),
        SidebarItem::Separator => "Filters".to_string(),
    }
}

fn panel(title: &str, focused: bool) -> Block<'_> {
    let title = if focused {
        format!("{title} *")
    } else {
        title.to_string()
    };

    Block::default().borders(Borders::ALL).title(title)
}

pub fn compute_scroll_offset(
    lines: &[Line<'_>],
    selected_line_index: Option<usize>,
    inner_width: u16,
    pane_height: usize,
) -> u16 {
    let Some(sel_idx) = selected_line_index else {
        return 0;
    };
    if inner_width == 0 || pane_height == 0 || sel_idx >= lines.len() {
        return 0;
    }
    let prefix = lines[..=sel_idx].to_vec();
    let visual_row_end = Paragraph::new(prefix)
        .wrap(Wrap { trim: false })
        .line_count(inner_width);
    if visual_row_end > pane_height {
        (visual_row_end - pane_height) as u16
    } else {
        0
    }
}

fn centered_rect(area: ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area)[1];

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(vertical)[1]
}

fn visual_line_count(text: &str, width: usize) -> usize {
    if text.is_empty() || width == 0 {
        return 1;
    }
    let char_count = text.chars().count();
    (char_count + width - 1) / width
}
