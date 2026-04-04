use std::io;

use crate::parser::{is_date, parse_task_line};
use crate::store::{EditSaveOutcome, TaskId, TaskStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorMode {
    QuickEntry,
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTask {
    pub id: TaskId,
    pub normalized_raw: String,
    pub original_raw: String,
}

impl SelectedTask {
    pub fn new(id: TaskId, normalized_raw: impl Into<String>) -> Self {
        let normalized_raw = normalized_raw.into();
        Self {
            id,
            normalized_raw: normalized_raw.clone(),
            original_raw: normalized_raw,
        }
    }

    pub fn with_original_raw(
        id: TaskId,
        normalized_raw: impl Into<String>,
        original_raw: impl Into<String>,
    ) -> Self {
        Self {
            id,
            normalized_raw: normalized_raw.into(),
            original_raw: original_raw.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorShortcut {
    Due,
    Scheduled,
    Starting,
}

impl EditorShortcut {
    fn tag_key(self) -> &'static str {
        match self {
            Self::Due => "due",
            Self::Scheduled => "scheduled",
            Self::Starting => "starting",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Due => "Due",
            Self::Scheduled => "Scheduled",
            Self::Starting => "Starting",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorShortcutState {
    pub shortcut: EditorShortcut,
    pub input: String,
    pub cursor_pos: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    pub mode: EditorMode,
    pub raw_line: String,
    pub cursor_pos: usize,
    pub original_raw: Option<String>,
    pub due: Option<String>,
    pub scheduled: Option<String>,
    pub starting: Option<String>,
    pub task_id: Option<TaskId>,
    pub shortcut: Option<EditorShortcutState>,
}

impl EditorState {
    pub fn quick_entry() -> Self {
        Self::build(EditorMode::QuickEntry, String::new(), None, None)
    }

    pub fn set_suffix(&mut self, suffix: &str) {
        self.raw_line = format!(" {suffix}");
        self.cursor_pos = 0;
        self.refresh_helpers_from_raw();
    }

    pub fn edit(task: &SelectedTask) -> Self {
        Self::build(
            EditorMode::Edit,
            task.normalized_raw.clone(),
            Some(task.original_raw.clone()),
            Some(task.id.clone()),
        )
    }

    pub fn set_raw_line(&mut self, raw_line: impl Into<String>) {
        self.raw_line = raw_line.into();
        self.cursor_pos = self.raw_line.chars().count();
        self.refresh_helpers_from_raw();
    }

    pub fn set_due(&mut self, due: Option<&str>) {
        self.set_metadata_tag("due", due);
    }

    pub fn set_scheduled(&mut self, scheduled: Option<&str>) {
        self.set_metadata_tag("scheduled", scheduled);
    }

    pub fn set_starting(&mut self, starting: Option<&str>) {
        self.set_metadata_tag("starting", starting);
    }

    pub fn append_raw_char(&mut self, value: &str) {
        let byte_pos = char_to_byte_pos(&self.raw_line, self.cursor_pos);
        self.raw_line.insert_str(byte_pos, value);
        self.cursor_pos += value.chars().count();
        self.refresh_helpers_from_raw();
    }

    pub fn backspace_raw(&mut self) -> bool {
        if self.cursor_pos == 0 {
            return false;
        }
        let byte_start = char_to_byte_pos(&self.raw_line, self.cursor_pos - 1);
        let byte_end = char_to_byte_pos(&self.raw_line, self.cursor_pos);
        self.raw_line.replace_range(byte_start..byte_end, "");
        self.cursor_pos -= 1;
        self.refresh_helpers_from_raw();
        true
    }

    pub fn move_cursor_left(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_right(&mut self) -> bool {
        if self.cursor_pos < self.raw_line.chars().count() {
            self.cursor_pos += 1;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_up(&mut self, wrap_width: usize) -> bool {
        if wrap_width == 0 {
            return false;
        }
        if self.cursor_pos >= wrap_width {
            self.cursor_pos -= wrap_width;
            true
        } else if self.cursor_pos > 0 {
            self.cursor_pos = 0;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_down(&mut self, wrap_width: usize) -> bool {
        if wrap_width == 0 {
            return false;
        }
        let char_count = self.raw_line.chars().count();
        if self.cursor_pos + wrap_width <= char_count {
            self.cursor_pos += wrap_width;
            true
        } else if self.cursor_pos < char_count {
            self.cursor_pos = char_count;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.raw_line.chars().count();
    }

    pub fn open_shortcut(&mut self, shortcut: EditorShortcut) {
        let input = match shortcut {
            EditorShortcut::Due => self.due.clone(),
            EditorShortcut::Scheduled => self.scheduled.clone(),
            EditorShortcut::Starting => self.starting.clone(),
        }
        .unwrap_or_default();

        let cursor_pos = input.chars().count();
        self.shortcut = Some(EditorShortcutState {
            shortcut,
            input,
            cursor_pos,
            error: None,
        });
    }

    pub fn append_shortcut_char(&mut self, value: &str) {
        if let Some(shortcut) = self.shortcut.as_mut() {
            let byte_pos = char_to_byte_pos(&shortcut.input, shortcut.cursor_pos);
            shortcut.input.insert_str(byte_pos, value);
            shortcut.cursor_pos += value.chars().count();
            shortcut.error = None;
        }
    }

    pub fn backspace_shortcut(&mut self) -> bool {
        if let Some(shortcut) = self.shortcut.as_mut() {
            if shortcut.cursor_pos == 0 {
                return false;
            }
            let byte_start = char_to_byte_pos(&shortcut.input, shortcut.cursor_pos - 1);
            let byte_end = char_to_byte_pos(&shortcut.input, shortcut.cursor_pos);
            shortcut.input.replace_range(byte_start..byte_end, "");
            shortcut.cursor_pos -= 1;
            shortcut.error = None;
            true
        } else {
            false
        }
    }

    pub fn move_shortcut_cursor_left(&mut self) -> bool {
        if let Some(shortcut) = self.shortcut.as_mut() {
            if shortcut.cursor_pos > 0 {
                shortcut.cursor_pos -= 1;
                return true;
            }
        }
        false
    }

    pub fn move_shortcut_cursor_right(&mut self) -> bool {
        if let Some(shortcut) = self.shortcut.as_mut() {
            if shortcut.cursor_pos < shortcut.input.chars().count() {
                shortcut.cursor_pos += 1;
                return true;
            }
        }
        false
    }

    pub fn apply_shortcut(&mut self) -> Option<ShortcutApplyOutcome> {
        let shortcut = self.shortcut.take()?;
        let value = if shortcut.input.trim().is_empty() {
            None
        } else {
            Some(shortcut.input.as_str())
        };

        if let Some(value) = value
            && !is_date_token(value)
        {
            let shortcut_kind = shortcut.shortcut;
            self.shortcut = Some(EditorShortcutState {
                error: Some("Use YYYY-MM-DD for helper dates".into()),
                ..shortcut
            });
            return Some(ShortcutApplyOutcome::Rejected(shortcut_kind));
        }

        self.set_metadata_tag(shortcut.shortcut.tag_key(), value);
        Some(ShortcutApplyOutcome::Applied(shortcut.shortcut))
    }

    pub fn cancel_shortcut(&mut self) {
        self.shortcut = None;
    }

    fn build(
        mode: EditorMode,
        raw_line: String,
        original_raw: Option<String>,
        task_id: Option<TaskId>,
    ) -> Self {
        let cursor_pos = raw_line.chars().count();
        let mut state = Self {
            mode,
            raw_line,
            cursor_pos,
            original_raw,
            due: None,
            scheduled: None,
            starting: None,
            task_id,
            shortcut: None,
        };
        state.refresh_helpers_from_raw();
        state
    }

    fn set_metadata_tag(&mut self, key: &str, value: Option<&str>) {
        self.raw_line = upsert_tag(&self.raw_line, key, value);
        self.refresh_helpers_from_raw();
    }

    fn refresh_helpers_from_raw(&mut self) {
        let task = parse_task_line(&self.raw_line);
        self.due = task.tags.get("due").cloned();
        self.scheduled = task.tags.get("scheduled").cloned();
        self.starting = task.tags.get("starting").cloned();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutApplyOutcome {
    Applied(EditorShortcut),
    Rejected(EditorShortcut),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    ReloadExternal,
    OverwriteExternal,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveConflictState {
    pub external_raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorSaveRequest {
    Create {
        raw: String,
    },
    Update {
        id: TaskId,
        original_raw: String,
        raw: String,
        overwrite_conflict: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorSaveResult {
    Saved,
    Conflict { external_raw: String },
}

pub trait EditorSaveTarget {
    fn save_editor(&mut self, request: EditorSaveRequest) -> io::Result<EditorSaveResult>;
}

impl EditorSaveTarget for TaskStore {
    fn save_editor(&mut self, request: EditorSaveRequest) -> io::Result<EditorSaveResult> {
        match request {
            EditorSaveRequest::Create { raw } => {
                self.create_task(&raw)?;
                Ok(EditorSaveResult::Saved)
            }
            EditorSaveRequest::Update {
                id,
                original_raw,
                raw,
                overwrite_conflict,
            } => match self.save_edited_task(&id, &original_raw, &raw, overwrite_conflict)? {
                EditSaveOutcome::Saved => Ok(EditorSaveResult::Saved),
                EditSaveOutcome::Conflict { external_raw } => {
                    Ok(EditorSaveResult::Conflict { external_raw })
                }
            },
        }
    }
}

fn upsert_tag(raw_line: &str, key: &str, value: Option<&str>) -> String {
    let prefix = format!("{key}:");
    let mut tokens = raw_line
        .split_whitespace()
        .filter(|token| !token.starts_with(&prefix))
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if let Some(value) = value {
        tokens.push(format!("{key}:{value}"));
    }

    tokens.join(" ")
}

fn is_date_token(token: &str) -> bool {
    is_date(token)
}

fn char_to_byte_pos(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}
