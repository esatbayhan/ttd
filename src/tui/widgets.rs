use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::app::AppState;
use crate::task::Task;

pub fn help_bar_text(app: &AppState) -> String {
    help_bar_entries(app)
        .iter()
        .map(|(key, desc)| format!("{key} {desc}"))
        .collect::<Vec<_>>()
        .join(" │ ")
}

pub fn render_help_bar(app: &AppState) -> Paragraph<'static> {
    let entries = help_bar_entries(app);
    let mut spans = Vec::new();
    for (i, (key, desc)) in entries.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(Span::styled(
            key.to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {desc}"),
            Style::default().fg(Color::Gray),
        ));
    }
    Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray))
}

fn help_bar_entries(app: &AppState) -> Vec<(&'static str, &'static str)> {
    if app.save_conflict.is_some() {
        return vec![("r", "reload"), ("o", "overwrite"), ("c", "cancel")];
    }
    if app.picker.is_some() {
        return vec![
            ("j/k", "nav"),
            ("enter", "select"),
            ("esc", "cancel"),
        ];
    }
    if app.editor.as_ref().is_some_and(|e| e.shortcut.is_some()) {
        return vec![("enter", "apply"), ("esc", "cancel")];
    }
    if app.editor.is_some() {
        return vec![
            ("enter", "save"),
            ("esc", "cancel"),
            ("ctrl+d", "due"),
            ("ctrl+s", "sched"),
            ("ctrl+t", "start"),
        ];
    }
    if app.confirm_delete {
        return vec![("enter", "confirm"), ("esc", "cancel")];
    }
    if app.search_active {
        return vec![("esc", "cancel"), ("n", "next"), ("N", "prev")];
    }
    vec![
        ("j/k", "nav"),
        ("h/l", "focus"),
        ("a", "add"),
        ("e", "edit"),
        ("x", "toggle done"),
        ("D", "delete"),
        ("s", "sort"),
        ("o", "group"),
        ("r", "reverse"),
        ("/", "search"),
        ("q", "quit"),
    ]
}

pub fn task_line_text(task: &Task, is_selected: bool) -> String {
    let marker = if is_selected {
        "> "
    } else if task.done {
        "\u{2713} "
    } else {
        "  "
    };
    let description = strip_date_tags(&task.description);
    let mut result = format!("{marker}{description}");
    let tag_line = build_tag_line(task);
    if !tag_line.is_empty() {
        result.push('\n');
        result.push_str("    ");
        result.push_str(&tag_line);
    }
    result
}

pub fn render_task_lines<'a>(task: &Task, is_selected: bool, wrap_width: u16) -> Vec<Line<'a>> {
    let marker = if is_selected {
        "> "
    } else if task.done {
        "\u{2713} "
    } else {
        "  "
    };
    let description = strip_date_tags(&task.description);
    let mut lines = wrap_colored_description(marker, &description, wrap_width);
    let tag_spans = build_tag_spans(task);
    if !tag_spans.is_empty() {
        let mut spans = vec![Span::raw("    ")];
        spans.extend(tag_spans);
        lines.push(Line::from(spans));
    }
    if task.done {
        let dim = Style::default().add_modifier(Modifier::DIM);
        lines = lines
            .into_iter()
            .map(|line| {
                Line::from(
                    line.spans
                        .into_iter()
                        .map(|span| Span::styled(span.content, dim))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
    }
    lines
}

fn strip_date_tags(description: &str) -> String {
    description
        .split_whitespace()
        .filter(|token| {
            !token.starts_with("due:")
                && !token.starts_with("scheduled:")
                && !token.starts_with("starting:")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn wrap_colored_description(marker: &str, description: &str, width: u16) -> Vec<Line<'static>> {
    let width = width as usize;
    let indent = "  ";
    let prefix_len = marker.len();
    let words: Vec<&str> = description.split_whitespace().collect();

    if words.is_empty() {
        return vec![Line::from(Span::raw(marker.to_string()))];
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = vec![Span::raw(marker.to_string())];
    let mut col = prefix_len;

    for word in &words {
        let need_space = col > prefix_len;
        let extra = if need_space { 1 } else { 0 };

        if col + extra + word.len() > width && col > prefix_len {
            result.push(Line::from(spans));
            spans = vec![Span::raw(indent.to_string())];
            col = prefix_len;
        } else if need_space {
            spans.push(Span::raw(" "));
            col += 1;
        }

        // If the word is longer than the remaining space on this line,
        // split it into chunks so each continuation line gets the
        // hanging indent instead of wrapping flush-left.
        let remaining = width.saturating_sub(col);
        if word.len() > remaining && width > prefix_len {
            let chars: Vec<char> = word.chars().collect();
            let mut pos = 0;

            // Fill the rest of the current line
            if remaining > 0 {
                let end = remaining.min(chars.len());
                spans.push(color_word(&chars[..end].iter().collect::<String>()));
                pos = end;
            }

            // Each subsequent chunk gets its own indented line
            let chunk_size = width - prefix_len;
            while pos < chars.len() {
                result.push(Line::from(spans));
                spans = vec![Span::raw(indent.to_string())];
                let end = (pos + chunk_size).min(chars.len());
                spans.push(color_word(&chars[pos..end].iter().collect::<String>()));
                col = prefix_len + (end - pos);
                pos = end;
            }
        } else {
            spans.push(color_word(word));
            col += word.len();
        }
    }

    result.push(Line::from(spans));
    result
}

fn color_word(word: &str) -> Span<'static> {
    if word.starts_with('+') {
        Span::styled(word.to_string(), Style::default().fg(Color::Cyan))
    } else if word.starts_with('@') {
        Span::styled(word.to_string(), Style::default().fg(Color::Green))
    } else {
        Span::raw(word.to_string())
    }
}

fn build_tag_line(task: &Task) -> String {
    let mut parts = Vec::new();
    if let Some(due) = task.tags.get("due") {
        parts.push(format!("due: {due}"));
    }
    if let Some(scheduled) = task.tags.get("scheduled") {
        parts.push(format!("sched: {scheduled}"));
    }
    if let Some(starting) = task.tags.get("starting") {
        parts.push(format!("start: {starting}"));
    }
    if let Some(created) = task.creation_date.as_deref() {
        parts.push(format!("created: {created}"));
    }
    parts.join("  ")
}

fn build_tag_spans(task: &Task) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if let Some(due) = task.tags.get("due") {
        spans.push(Span::styled(
            format!("due: {due}"),
            Style::default().fg(Color::Yellow),
        ));
    }
    if let Some(scheduled) = task.tags.get("scheduled") {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("sched: {scheduled}"),
            Style::default().fg(Color::Cyan),
        ));
    }
    if let Some(starting) = task.tags.get("starting") {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("start: {starting}"),
            Style::default().fg(Color::Green),
        ));
    }
    if let Some(created) = task.creation_date.as_deref() {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("created: {created}"),
            Style::default().fg(Color::Magenta),
        ));
    }
    spans
}
