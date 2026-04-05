use super::editor::{
    ConflictChoice, EditorSaveRequest, EditorSaveResult, EditorSaveTarget, EditorShortcut,
    EditorState, SaveConflictState, SelectedTask, ShortcutApplyOutcome,
};
use super::events::normalize_key;
use super::render::EDITOR_MODAL_WIDTH;
use crate::smartlist::Field;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Sort,
    Group,
}

#[derive(Debug, Clone)]
pub struct PickerState {
    pub kind: PickerKind,
    pub items: Vec<Field>,
    pub selected_index: usize,
}

impl PickerState {
    pub fn new(kind: PickerKind) -> Self {
        Self {
            kind,
            items: vec![
                Field::Priority,
                Field::Due,
                Field::Scheduled,
                Field::Starting,
                Field::CreationDate,
            ],
            selected_index: 0,
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.items.len() - 1 {
            self.selected_index += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn selected_field(&self) -> &Field {
        &self.items[self.selected_index]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Welcome,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Sidebar,
    TaskList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Quit,
    MoveDown,
    MoveUp,
    MoveTop,
    MoveBottom,
    FocusNext,
    FocusPrev,
    OpenSelected,
    AddTask,
    EditTask,
    ToggleDone,
    Refresh,
    ConfirmDelete,
    EnterSearch,
    AppendToSearch(String),
    BackspaceSearch,
    NextSearchResult,
    PreviousSearchResult,
    Cancel,
    AppendToInput(String),
    BackspaceInput,
    SubmitWelcomePath(String),
    AppendToEditor(String),
    BackspaceEditor,
    OpenEditorShortcut(EditorShortcut),
    AppendToEditorShortcut(String),
    BackspaceEditorShortcut,
    ApplyEditorShortcut(EditorShortcut),
    RejectEditorShortcut(EditorShortcut),
    SubmitEditor,
    ResolveConflict(ConflictChoice),
    OpenSortPicker,
    OpenGroupPicker,
    DeactivateSort,
    DeactivateGroup,
    ReverseSort,
    PickerSelect,
}

pub struct AppState {
    pub mode: AppMode,
    pub focus: FocusArea,
    pub search_active: bool,
    pub search_query: String,
    pub confirm_delete: bool,
    pub welcome_input: String,
    pub selected_task: Option<SelectedTask>,
    pub editor: Option<EditorState>,
    pub save_conflict: Option<SaveConflictState>,
    pub should_quit: bool,
    pub picker: Option<PickerState>,
}

impl AppState {
    pub fn new(mode: AppMode) -> Self {
        Self {
            mode,
            focus: FocusArea::Sidebar,
            search_active: false,
            search_query: String::new(),
            confirm_delete: false,
            welcome_input: String::new(),
            selected_task: None,
            editor: None,
            save_conflict: None,
            should_quit: false,
            picker: None,
        }
    }

    pub fn handle_key(&mut self, key: &str) -> Option<AppAction> {
        let key = normalize_key(key);

        match self.mode {
            AppMode::Welcome => self.handle_welcome_key(key),
            AppMode::Main => self.handle_main_key(key),
        }
    }

    fn handle_welcome_key(&mut self, key: &str) -> Option<AppAction> {
        match key {
            "enter" => Some(AppAction::SubmitWelcomePath(self.welcome_input.clone())),
            "backspace" => {
                if self.welcome_input.pop().is_some() {
                    Some(AppAction::BackspaceInput)
                } else {
                    None
                }
            }
            _ if is_text_input(key) => {
                self.welcome_input.push_str(key);
                Some(AppAction::AppendToInput(key.to_string()))
            }
            _ => None,
        }
    }

    fn handle_main_key(&mut self, key: &str) -> Option<AppAction> {
        if self.save_conflict.is_some() {
            return self.handle_conflict_key(key);
        }

        if self.editor.is_some() {
            return self.handle_editor_key(key);
        }

        if self.picker.is_some() {
            return self.handle_picker_key(key);
        }

        if self.confirm_delete {
            return match key {
                "enter" => Some(AppAction::OpenSelected),
                "esc" => {
                    self.confirm_delete = false;
                    Some(AppAction::Cancel)
                }
                _ => None,
            };
        }

        if self.search_active {
            return match key {
                "esc" => {
                    self.search_active = false;
                    Some(AppAction::Cancel)
                }
                "backspace" => {
                    self.search_query.pop();
                    Some(AppAction::BackspaceSearch)
                }
                _ if is_text_input(key) => {
                    self.search_query.push_str(key);
                    Some(AppAction::AppendToSearch(key.to_string()))
                }
                _ => None,
            };
        }

        match key {
            "s" => {
                self.picker = Some(PickerState::new(PickerKind::Sort));
                Some(AppAction::OpenSortPicker)
            }
            "S" => Some(AppAction::DeactivateSort),
            "o" => {
                self.picker = Some(PickerState::new(PickerKind::Group));
                Some(AppAction::OpenGroupPicker)
            }
            "O" => Some(AppAction::DeactivateGroup),
            "r" => Some(AppAction::ReverseSort),
            "q" => {
                self.should_quit = true;
                Some(AppAction::Quit)
            }
            "tab" => {
                self.focus = toggle_focus(self.focus);
                Some(AppAction::FocusNext)
            }
            "h" | "left" => {
                self.focus = toggle_focus(self.focus);
                Some(AppAction::FocusPrev)
            }
            "l" | "right" => {
                self.focus = toggle_focus(self.focus);
                Some(AppAction::FocusNext)
            }
            "/" => {
                self.search_active = true;
                self.search_query.clear();
                Some(AppAction::EnterSearch)
            }
            "j" | "down" => Some(AppAction::MoveDown),
            "k" | "up" => Some(AppAction::MoveUp),
            "gg" => Some(AppAction::MoveTop),
            "G" => Some(AppAction::MoveBottom),
            "enter" => Some(AppAction::OpenSelected),
            "a" => {
                self.editor = Some(EditorState::quick_entry());
                Some(AppAction::AddTask)
            }
            "e" => {
                let selected = self.selected_task.as_ref()?;
                self.editor = Some(EditorState::edit(selected));
                Some(AppAction::EditTask)
            }
            "x" => Some(AppAction::ToggleDone),
            "D" => {
                self.confirm_delete = true;
                Some(AppAction::ConfirmDelete)
            }
            "R" => Some(AppAction::Refresh),
            "n" => Some(AppAction::NextSearchResult),
            "N" => Some(AppAction::PreviousSearchResult),
            _ => None,
        }
    }

    fn handle_editor_key(&mut self, key: &str) -> Option<AppAction> {
        let editor = self.editor.as_mut()?;

        if editor.shortcut.is_some() {
            return match key {
                "esc" => {
                    editor.cancel_shortcut();
                    Some(AppAction::Cancel)
                }
                "enter" => editor.apply_shortcut().map(|result| match result {
                    ShortcutApplyOutcome::Applied(shortcut) => {
                        AppAction::ApplyEditorShortcut(shortcut)
                    }
                    ShortcutApplyOutcome::Rejected(shortcut) => {
                        AppAction::RejectEditorShortcut(shortcut)
                    }
                }),
                "backspace" => {
                    if editor.backspace_shortcut() {
                        Some(AppAction::BackspaceEditorShortcut)
                    } else {
                        None
                    }
                }
                "left" => {
                    editor.move_shortcut_cursor_left();
                    None
                }
                "right" => {
                    editor.move_shortcut_cursor_right();
                    None
                }
                _ if is_text_input(key) => {
                    editor.append_shortcut_char(key);
                    Some(AppAction::AppendToEditorShortcut(key.to_string()))
                }
                _ => None,
            };
        }

        match key {
            "esc" => {
                self.editor = None;
                Some(AppAction::Cancel)
            }
            "enter" => {
                if editor.raw_line.trim().is_empty() {
                    self.editor = None;
                    Some(AppAction::Cancel)
                } else {
                    Some(AppAction::SubmitEditor)
                }
            }
            "backspace" => {
                if editor.backspace_raw() {
                    Some(AppAction::BackspaceEditor)
                } else {
                    None
                }
            }
            "left" => {
                editor.move_cursor_left();
                None
            }
            "right" => {
                editor.move_cursor_right();
                None
            }
            "up" => {
                let inner_width = (EDITOR_MODAL_WIDTH - 2) as usize;
                editor.move_cursor_up(inner_width);
                None
            }
            "down" => {
                let inner_width = (EDITOR_MODAL_WIDTH - 2) as usize;
                editor.move_cursor_down(inner_width);
                None
            }
            "home" => {
                editor.move_cursor_home();
                None
            }
            "end" => {
                editor.move_cursor_end();
                None
            }
            "ctrl+d" => {
                editor.open_shortcut(EditorShortcut::Due);
                Some(AppAction::OpenEditorShortcut(EditorShortcut::Due))
            }
            "ctrl+s" => {
                editor.open_shortcut(EditorShortcut::Scheduled);
                Some(AppAction::OpenEditorShortcut(EditorShortcut::Scheduled))
            }
            "ctrl+t" => {
                editor.open_shortcut(EditorShortcut::Starting);
                Some(AppAction::OpenEditorShortcut(EditorShortcut::Starting))
            }
            _ if is_text_input(key) => {
                editor.append_raw_char(key);
                Some(AppAction::AppendToEditor(key.to_string()))
            }
            _ => None,
        }
    }

    fn handle_picker_key(&mut self, key: &str) -> Option<AppAction> {
        let picker = self.picker.as_mut()?;
        match key {
            "j" | "down" => { picker.move_down(); None }
            "k" | "up" => { picker.move_up(); None }
            "enter" => Some(AppAction::PickerSelect),
            "esc" => { self.picker = None; Some(AppAction::Cancel) }
            _ => None,
        }
    }

    fn handle_conflict_key(&mut self, key: &str) -> Option<AppAction> {
        match key {
            "r" => Some(AppAction::ResolveConflict(ConflictChoice::ReloadExternal)),
            "o" => Some(AppAction::ResolveConflict(
                ConflictChoice::OverwriteExternal,
            )),
            "c" | "esc" => Some(AppAction::ResolveConflict(ConflictChoice::Cancel)),
            _ => None,
        }
    }

    pub fn save_editor<T: EditorSaveTarget>(&mut self, target: &mut T) -> std::io::Result<()> {
        let Some(editor) = self.editor.as_ref() else {
            return Ok(());
        };

        let request = match (&editor.task_id, &editor.original_raw) {
            (Some(id), Some(original_raw)) => EditorSaveRequest::Update {
                id: id.clone(),
                original_raw: original_raw.clone(),
                raw: editor.raw_line.clone(),
                overwrite_conflict: false,
            },
            _ => EditorSaveRequest::Create {
                raw: editor.raw_line.clone(),
            },
        };

        match target.save_editor(request)? {
            EditorSaveResult::Saved => {
                self.editor = None;
                self.save_conflict = None;
            }
            EditorSaveResult::Conflict { external_raw } => {
                self.save_conflict = Some(SaveConflictState { external_raw });
            }
        }

        Ok(())
    }

    pub fn resolve_save_conflict<T: EditorSaveTarget>(
        &mut self,
        choice: ConflictChoice,
        target: &mut T,
    ) -> std::io::Result<()> {
        let Some(conflict) = self.save_conflict.clone() else {
            return Ok(());
        };

        match choice {
            ConflictChoice::Cancel => {
                self.save_conflict = None;
            }
            ConflictChoice::ReloadExternal => {
                if let Some(editor) = self.editor.as_mut() {
                    editor.set_raw_line(conflict.external_raw.clone());
                    editor.original_raw = Some(conflict.external_raw);
                }
                self.save_conflict = None;
            }
            ConflictChoice::OverwriteExternal => {
                let Some(editor) = self.editor.as_ref() else {
                    self.save_conflict = None;
                    return Ok(());
                };

                let Some(task_id) = editor.task_id.clone() else {
                    self.save_conflict = None;
                    return Ok(());
                };

                let request = EditorSaveRequest::Update {
                    id: task_id,
                    original_raw: editor
                        .original_raw
                        .clone()
                        .unwrap_or_else(|| conflict.external_raw.clone()),
                    raw: editor.raw_line.clone(),
                    overwrite_conflict: true,
                };

                match target.save_editor(request)? {
                    EditorSaveResult::Saved => {
                        self.editor = None;
                        self.save_conflict = None;
                    }
                    EditorSaveResult::Conflict { external_raw } => {
                        self.save_conflict = Some(SaveConflictState { external_raw });
                    }
                }
            }
        }

        Ok(())
    }
}

fn toggle_focus(focus: FocusArea) -> FocusArea {
    match focus {
        FocusArea::Sidebar => FocusArea::TaskList,
        FocusArea::TaskList => FocusArea::Sidebar,
    }
}

fn is_text_input(key: &str) -> bool {
    !key.is_empty() && key.chars().count() == 1
}
