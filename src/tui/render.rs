use std::cell::Cell;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};

use super::app::{AppMode, AppState, FocusArea};
use super::editor::EditorMode;
use super::session::{SidebarItem, TuiSession};
use super::widgets::{render_help_bar, render_task_lines};

#[derive(Clone, Copy, Debug)]
pub struct Rects {
    pub sidebar: Rect,
    pub task_pane: Rect,
    pub sidebar_item_count: usize,
    pub sidebar_offset: usize,
    pub task_pane_inner_width: u16,
    pub visual_line_count: usize,
    pub pane_height: usize,
}

#[derive(Default)]
pub struct LayoutRects {
    inner: Cell<Option<Rects>>,
}

impl LayoutRects {
    pub fn set(&self, rects: Rects) {
        self.inner.set(Some(rects));
    }

    pub fn get(&self) -> Option<Rects> {
        self.inner.get()
    }
}

pub const EDITOR_MODAL_WIDTH: u16 = 68;

pub fn render_frame(frame: &mut Frame<'_>, app: &AppState) {
    match app.mode {
        AppMode::Welcome => render_welcome(frame, app),
        AppMode::Main => render_main(frame, app),
    }
}

pub fn render_session_frame(frame: &mut Frame<'_>, session: &TuiSession) {
    match session.app().mode {
        AppMode::Welcome => render_welcome(frame, session.app()),
        AppMode::Main => render_session_main(frame, session, None),
    }
}

pub fn render_session_frame_with_layout(
    frame: &mut Frame<'_>,
    session: &TuiSession,
    layout: &LayoutRects,
) {
    match session.app().mode {
        AppMode::Welcome => render_welcome(frame, session.app()),
        AppMode::Main => render_session_main(frame, session, Some(layout)),
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
    let items = vec![
        ListItem::new("Projects").style(Style::default().add_modifier(Modifier::DIM)),
        ListItem::new("Contexts").style(Style::default().add_modifier(Modifier::DIM)),
    ];

    let task_content = if app.search_active {
        vec![
            Line::raw("Tasks"),
            Line::raw(""),
            Line::raw("Search: active"),
        ]
    } else {
        vec![Line::raw("Tasks")]
    };

    render_main_shell(frame, app, items, task_content, "Filters", "Tasks", None, None, 0);
    render_overlays(frame, app);
}

fn render_session_main(frame: &mut Frame<'_>, session: &TuiSession, layout: Option<&LayoutRects>) {
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
                SidebarItem::SmartList(index)
                    if session
                        .smart_lists()
                        .get(*index)
                        .is_some_and(|l| l.parse_error.is_some()) =>
                {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM)
                }
                _ if *item == session.active_sidebar_item() => {
                    Style::default().add_modifier(Modifier::BOLD)
                }
                SidebarItem::ProjectsHeader
                | SidebarItem::ContextsHeader
                | SidebarItem::Separator => Style::default().add_modifier(Modifier::DIM),
                _ => Style::default(),
            };
            ListItem::new(sidebar_label(item, session.smart_lists())).style(style)
        })
        .collect::<Vec<_>>();

    let selected_sidebar_index = session
        .sidebar_items()
        .iter()
        .position(|item| *item == session.active_sidebar_item());

    let sidebar_title = active_filter_title(&session.active_sidebar_item(), session.smart_lists());
    let tasks_title = format!("{} ({})", sidebar_title, session.visible_tasks().len());

    let mut task_lines: Vec<Line> = Vec::new();

    let mut selected_line_index: Option<usize> = None;
    let mut line_count: usize = 0;

    if !app.search_query.is_empty() {
        task_lines.push(Line::raw(format!("Search: {}", app.search_query)));
        task_lines.push(Line::raw(""));
        line_count += 2;
    } else if app.search_active {
        task_lines.push(Line::raw("Search: "));
        task_lines.push(Line::raw(""));
        line_count += 2;
    }

    let groups = session.visible_groups();
    let show_group_headers =
        groups.len() > 1 || groups.first().is_some_and(|g| !g.label.is_empty());

    if session.visible_tasks().is_empty() {
        if let Some(error) = session
            .smart_list_for_active()
            .and_then(|l| l.parse_error.as_ref())
        {
            task_lines.push(Line::from(Span::styled(
                format!("Parse error: {error}"),
                Style::default().fg(Color::Yellow),
            )));
        } else {
            task_lines.push(Line::raw("No tasks in this view"));
        }
    } else if show_group_headers {
        for (gi, group) in groups.iter().enumerate() {
            if !group.label.is_empty() {
                if gi > 0 {
                    let sep_width = (task_pane_inner_width as usize).saturating_sub(2);
                    task_lines.push(Line::from(Span::styled(
                        format!(" {}", "─".repeat(sep_width)),
                        Style::default().fg(Color::DarkGray),
                    )));
                    line_count += 1;
                }
                task_lines.push(Line::from(Span::styled(
                    format!(" {}", group.label),
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                line_count += 1;
            }

            for (i, stored) in group.tasks.iter().enumerate() {
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

                if i < group.tasks.len() - 1 {
                    task_lines.push(Line::raw(""));
                    line_count += 1;
                }
            }
        }
    } else {
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

    // Pre-compute scroll offset: use manual override if set, otherwise auto-follow selection.
    let scroll_offset = if let Some(override_offset) = session.task_scroll_offset_override() {
        override_offset
    } else {
        // Estimate pane dimensions for scroll calculation (matches render_main_shell layout).
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(frame.area());
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(24)])
            .split(outer[0]);
        let pane_height = chunks[1].height.saturating_sub(2) as usize;
        let inner_width = chunks[1].width.saturating_sub(2);
        compute_scroll_offset(&task_lines, selected_line_index, inner_width, pane_height)
    };

    render_main_shell(
        frame,
        app,
        sidebar,
        task_lines,
        &sidebar_title,
        &tasks_title,
        selected_sidebar_index,
        layout,
        scroll_offset,
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
    selected_sidebar_index: Option<usize>,
    layout: Option<&LayoutRects>,
    scroll_offset: u16,
) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(24)])
        .split(outer[0]);

    let sidebar_item_count = sidebar.len();
    let mut list_state = ListState::default().with_selected(selected_sidebar_index);
    frame.render_stateful_widget(
        List::new(sidebar)
            .block(panel(sidebar_title, app.focus == FocusArea::Sidebar))
            .scroll_padding(1),
        chunks[0],
        &mut list_state,
    );

    let sidebar_visible_height = chunks[0].height.saturating_sub(2) as usize;
    if sidebar_item_count > sidebar_visible_height {
        let mut scrollbar_state =
            ScrollbarState::new(sidebar_item_count.saturating_sub(sidebar_visible_height))
                .position(list_state.offset());
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            chunks[0].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }

    let pane_height = chunks[1].height.saturating_sub(2) as usize;
    let inner_width = chunks[1].width.saturating_sub(2);

    let visual_line_count = if inner_width > 0 {
        Paragraph::new(task_content.clone())
            .wrap(Wrap { trim: false })
            .line_count(inner_width) as usize
    } else {
        0
    };

    if let Some(layout) = layout {
        layout.set(Rects {
            sidebar: chunks[0],
            task_pane: chunks[1],
            sidebar_item_count,
            sidebar_offset: list_state.offset(),
            task_pane_inner_width: inner_width,
            visual_line_count,
            pane_height,
        });
    }

    frame.render_widget(
        Paragraph::new(task_content)
            .block(panel(tasks_title, app.focus == FocusArea::TaskList))
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset, 0)),
        chunks[1],
    );

    if visual_line_count > pane_height {
        let mut task_scrollbar_state =
            ScrollbarState::new(visual_line_count.saturating_sub(pane_height))
                .position(scroll_offset as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            chunks[1].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut task_scrollbar_state,
        );
    }

    frame.render_widget(render_help_bar(app), outer[1]);
}

fn render_overlays(frame: &mut Frame<'_>, app: &AppState) {
    if let Some(editor) = app.editor.as_ref() {
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
                "\n\n{} date: {}{}",
                shortcut.shortcut.label(),
                shortcut.input,
                error_text
            )
        } else {
            String::new()
        };
        let helper_text = format!(
            "due: {}\nscheduled: {}\nstarting: {}{}",
            editor.due.as_deref().unwrap_or("-"),
            editor.scheduled.as_deref().unwrap_or("-"),
            editor.starting.as_deref().unwrap_or("-"),
            shortcut_text
        );

        // The modal grows with the input text up to a cap, then scrolls.
        let modal_width = EDITOR_MODAL_WIDTH;
        let inner_width = (modal_width - 2) as usize; // subtract left+right borders
        let helper_line_count = helper_text.lines().count() as u16;
        let raw_visual_rows = if inner_width > 0 {
            let char_count = editor.raw_line.chars().count();
            (char_count / inner_width + 1).max(1) as u16
        } else {
            1u16
        };
        let max_input_rows = 10u16;
        let input_rows = raw_visual_rows.clamp(1, max_input_rows);
        let chrome = 2 + 1; // borders (top+bottom) + blank separator line
        let desired_height = input_rows + helper_line_count + chrome;
        let max_height = frame.area().height.saturating_sub(2);
        // Keep the editor taller than the conflict dialog (height 7) so its
        // title stays visible when both overlays are stacked.
        let min_height = 9u16;
        let mut modal_height = desired_height.max(min_height).min(max_height);
        // Ensure the vertical remainder is even so centered_rect grows
        // symmetrically (1 row top + 1 row bottom) instead of alternating.
        let remainder = frame.area().height.saturating_sub(modal_height);
        if remainder % 2 != 0 {
            modal_height = (modal_height + 1).min(max_height);
        }

        let modal = centered_rect(frame.area(), modal_width, modal_height);

        // Render modal border and clear the area underneath.
        let block = Block::default().borders(Borders::ALL).title(title);
        let modal_inner = block.inner(modal);
        frame.render_widget(Clear, modal);
        frame.render_widget(block, modal);

        // Split inner area: scrollable input (top), blank separator, helpers (bottom).
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(helper_line_count),
            ])
            .split(modal_inner);
        let input_area = chunks[0];
        let helper_area = chunks[2];

        // Character-wrap the raw_line so the cursor arithmetic
        // (cursor_pos / width, cursor_pos % width) stays correct,
        // then scroll to keep the cursor row visible.
        let raw_display = char_wrap_text(&editor.raw_line, inner_width);
        let cursor_row = if inner_width > 0 {
            editor.cursor_pos / inner_width
        } else {
            0
        };
        let input_height = input_area.height as usize;
        let scroll_offset = if input_height > 0 && cursor_row >= input_height {
            cursor_row - input_height + 1
        } else {
            0
        };

        frame.render_widget(
            Paragraph::new(raw_display)
                .wrap(Wrap { trim: false })
                .scroll((scroll_offset as u16, 0)),
            input_area,
        );

        frame.render_widget(
            Paragraph::new(helper_text.as_str()).wrap(Wrap { trim: false }),
            helper_area,
        );

        // Render cursor
        if inner_width > 0 {
            if let Some(shortcut) = editor.shortcut.as_ref() {
                // Shortcut input sits on line 4 of helper_text
                // (due=0, scheduled=1, starting=2, blank=3, shortcut=4).
                let shortcut_row = 4u16;
                let prefix_len = shortcut.shortcut.label().len() + " date: ".len();
                let col_in_line = prefix_len + shortcut.cursor_pos;
                let cursor_col = col_in_line % inner_width;
                let extra_rows = (col_in_line / inner_width) as u16;
                frame.set_cursor_position((
                    helper_area.x + cursor_col as u16,
                    helper_area.y + shortcut_row + extra_rows,
                ));
            } else {
                // Cursor on the raw_line, adjusted for scroll.
                let visible_row = cursor_row - scroll_offset;
                let cursor_col = editor.cursor_pos % inner_width;
                frame.set_cursor_position((
                    input_area.x + cursor_col as u16,
                    input_area.y + visible_row as u16,
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

fn sidebar_label(item: &SidebarItem, smart_lists: &[crate::smartlist::SmartList]) -> String {
    match item {
        SidebarItem::SmartList(index) => {
            if let Some(list) = smart_lists.get(*index) {
                let icon = list.icon.as_deref().unwrap_or("\u{25c6}"); // ◆ default
                format!("{icon} {}", list.name)
            } else {
                "?".to_string()
            }
        }
        SidebarItem::Separator => "──────────────────────".to_string(),
        SidebarItem::ProjectsHeader => "PROJECTS".to_string(),
        SidebarItem::Project(value) => format!("  {value}"),
        SidebarItem::ContextsHeader => "CONTEXTS".to_string(),
        SidebarItem::Context(value) => format!("  {value}"),
    }
}

fn active_filter_title(item: &SidebarItem, smart_lists: &[crate::smartlist::SmartList]) -> String {
    match item {
        SidebarItem::SmartList(index) => smart_lists
            .get(*index)
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
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

/// Pre-wrap text at exact character boundaries so that the visual layout
/// matches the simple `cursor_pos / width` cursor calculation.  Each
/// resulting logical line is at most `width` characters, so the
/// Paragraph's word-wrapper will never split it further.
fn char_wrap_text(text: &str, width: usize) -> String {
    if width == 0 || text.is_empty() {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len() + chars.len() / width);
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && i % width == 0 {
            result.push('\n');
        }
        result.push(ch);
    }
    result
}
