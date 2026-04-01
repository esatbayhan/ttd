use std::io;
use std::path::PathBuf;
use std::{fs, mem};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ttd::store::{TaskId, TaskStore};
use ttd::tui::app::{AppAction, AppMode, AppState};
use ttd::tui::editor::{
    ConflictChoice, EditorMode, EditorSaveRequest, EditorSaveResult, EditorSaveTarget,
    EditorShortcut, SelectedTask,
};
use ttd::tui::render::render_frame;

#[test]
fn quick_entry_opens_with_a() {
    let mut app = AppState::new(AppMode::Main);

    assert!(app.editor.is_none());
    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));

    let editor = app.editor.as_ref().expect("quick entry should open");
    assert_eq!(editor.mode, EditorMode::QuickEntry);
    assert_eq!(editor.raw_line, "");
}

#[test]
fn editor_modal_routes_text_input_backspace_and_save_through_handle_key() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));
    assert_eq!(
        app.handle_key("C"),
        Some(AppAction::AppendToEditor("C".into()))
    );
    assert_eq!(
        app.handle_key("a"),
        Some(AppAction::AppendToEditor("a".into()))
    );
    assert_eq!(
        app.handle_key("l"),
        Some(AppAction::AppendToEditor("l".into()))
    );
    assert_eq!(
        app.handle_key("l"),
        Some(AppAction::AppendToEditor("l".into()))
    );
    assert_eq!(
        app.handle_key(" "),
        Some(AppAction::AppendToEditor(" ".into()))
    );
    assert_eq!(
        app.handle_key("M"),
        Some(AppAction::AppendToEditor("M".into()))
    );
    assert_eq!(
        app.handle_key("o"),
        Some(AppAction::AppendToEditor("o".into()))
    );
    assert_eq!(
        app.handle_key("m"),
        Some(AppAction::AppendToEditor("m".into()))
    );
    assert_eq!(
        app.handle_key("backspace"),
        Some(AppAction::BackspaceEditor)
    );
    assert_eq!(
        app.handle_key("m"),
        Some(AppAction::AppendToEditor("m".into()))
    );

    assert_eq!(app.editor.as_ref().unwrap().raw_line, "Call Mom");
    assert_eq!(app.handle_key("enter"), Some(AppAction::SubmitEditor));
}

#[test]
fn editor_helper_shortcuts_apply_metadata_through_handle_key() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));
    for key in ["P", "l", "a", "n", " ", "t", "r", "i", "p"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }

    assert_eq!(
        app.handle_key("ctrl+d"),
        Some(AppAction::OpenEditorShortcut(EditorShortcut::Due))
    );
    for key in ["2", "0", "2", "6", "-", "0", "4", "-", "0", "1"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditorShortcut(key.to_string()))
        );
    }
    assert_eq!(
        app.handle_key("enter"),
        Some(AppAction::ApplyEditorShortcut(EditorShortcut::Due))
    );
    assert_eq!(
        app.editor.as_ref().unwrap().due.as_deref(),
        Some("2026-04-01")
    );

    assert_eq!(
        app.handle_key("ctrl+s"),
        Some(AppAction::OpenEditorShortcut(EditorShortcut::Scheduled))
    );
    for key in ["2", "0", "2", "6", "-", "0", "3", "-", "3", "1"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditorShortcut(key.to_string()))
        );
    }
    assert_eq!(
        app.handle_key("enter"),
        Some(AppAction::ApplyEditorShortcut(EditorShortcut::Scheduled))
    );

    assert_eq!(
        app.handle_key("ctrl+t"),
        Some(AppAction::OpenEditorShortcut(EditorShortcut::Starting))
    );
    for key in ["2", "0", "2", "6", "-", "0", "3", "-", "3", "0"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditorShortcut(key.to_string()))
        );
    }
    assert_eq!(
        app.handle_key("enter"),
        Some(AppAction::ApplyEditorShortcut(EditorShortcut::Starting))
    );

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.scheduled.as_deref(), Some("2026-03-31"));
    assert_eq!(editor.starting.as_deref(), Some("2026-03-30"));
    assert!(editor.raw_line.contains("due:2026-04-01"));
    assert!(editor.raw_line.contains("scheduled:2026-03-31"));
    assert!(editor.raw_line.contains("starting:2026-03-30"));
}

#[test]
fn uppercase_d_s_and_t_remain_typable_in_raw_editor_line() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));
    for key in ["D", "S", "T"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }

    assert_eq!(app.editor.as_ref().unwrap().raw_line, "DST");
    assert_eq!(
        app.handle_key("ctrl+d"),
        Some(AppAction::OpenEditorShortcut(EditorShortcut::Due))
    );
}

#[test]
fn invalid_helper_input_stays_visible_and_does_not_mutate_raw_line() {
    let mut app = AppState::new(AppMode::Main);

    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));
    for key in ["P", "l", "a", "n"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }

    assert_eq!(
        app.handle_key("ctrl+d"),
        Some(AppAction::OpenEditorShortcut(EditorShortcut::Due))
    );
    for key in ["n", "o", "t", "-", "a", "-", "d", "a", "t", "e"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditorShortcut(key.to_string()))
        );
    }
    assert_eq!(
        app.handle_key("enter"),
        Some(AppAction::RejectEditorShortcut(EditorShortcut::Due))
    );

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.due, None);
    assert_eq!(editor.raw_line, "Plan");
    let shortcut = editor.shortcut.as_ref().expect("shortcut should stay open");
    assert_eq!(shortcut.input, "not-a-date");
    assert_eq!(
        shortcut.error.as_deref(),
        Some("Use YYYY-MM-DD for helper dates")
    );

    let text = buffer_text(&render(&app));
    assert!(text.contains("Use YYYY-MM-DD for helper dates"));
}

#[test]
fn conflicting_save_requires_explicit_resolution_through_handle_key() {
    let mut app = AppState::new(AppMode::Main);
    app.selected_task = Some(SelectedTask::new(
        TaskId {
            path: PathBuf::from("/tmp/inbox.txt"),
            line_index: 0,
        },
        "(A) Call Mom due:2026-03-31",
    ));

    assert_eq!(app.handle_key("e"), Some(AppAction::EditTask));
    assert_eq!(
        app.handle_key(" "),
        Some(AppAction::AppendToEditor(" ".into()))
    );
    assert_eq!(
        app.handle_key("@"),
        Some(AppAction::AppendToEditor("@".into()))
    );
    assert_eq!(
        app.handle_key("h"),
        Some(AppAction::AppendToEditor("h".into()))
    );
    assert_eq!(
        app.handle_key("o"),
        Some(AppAction::AppendToEditor("o".into()))
    );
    assert_eq!(
        app.handle_key("m"),
        Some(AppAction::AppendToEditor("m".into()))
    );
    assert_eq!(app.handle_key("enter"), Some(AppAction::SubmitEditor));

    let mut saver = ConflictSaver::default();
    app.save_editor(&mut saver).unwrap();

    assert_eq!(saver.requests.len(), 1);
    assert!(app.editor.is_some());

    let conflict = app
        .save_conflict
        .as_ref()
        .expect("conflict should require explicit resolution");
    assert_eq!(conflict.external_raw, "(A) Call Mom due:2026-03-30");
    assert_eq!(
        app.editor.as_ref().unwrap().raw_line,
        "(A) Call Mom due:2026-03-31 @hom"
    );
}

#[test]
fn edit_modal_starts_from_selected_tasks_normalized_raw_line() {
    let mut app = AppState::new(AppMode::Main);
    app.selected_task = Some(SelectedTask::new(
        TaskId {
            path: PathBuf::from("/tmp/inbox.txt"),
            line_index: 0,
        },
        "(B) Plan trip due:2026-04-05",
    ));

    assert_eq!(app.handle_key("e"), Some(AppAction::EditTask));

    let editor = app.editor.as_ref().expect("edit modal should open");
    assert_eq!(editor.mode, EditorMode::Edit);
    assert_eq!(editor.raw_line, "(B) Plan trip due:2026-04-05");
    assert_eq!(
        editor.original_raw.as_deref(),
        Some("(B) Plan trip due:2026-04-05")
    );
}

#[test]
fn metadata_helpers_are_stored_in_editor_state() {
    let mut app = AppState::new(AppMode::Main);
    app.handle_key("a");

    assert_eq!(
        app.handle_key("ctrl+d"),
        Some(AppAction::OpenEditorShortcut(EditorShortcut::Due))
    );
    for key in ["2", "0", "2", "6", "-", "0", "4", "-", "0", "1"] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditorShortcut(key.to_string()))
        );
    }
    assert_eq!(
        app.handle_key("enter"),
        Some(AppAction::ApplyEditorShortcut(EditorShortcut::Due))
    );

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.due.as_deref(), Some("2026-04-01"));
    assert!(editor.raw_line.contains("due:2026-04-01"));
}

#[test]
fn save_dispatches_quick_entry_through_task_store() {
    let root = temp_path("quick-entry-store");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();

    let mut app = AppState::new(AppMode::Main);
    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));
    for key in [
        "C", "a", "l", "l", " ", "M", "o", "m", " ", "+", "F", "a", "m", "i", "l", "y",
    ] {
        assert_eq!(
            app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }
    assert_eq!(app.handle_key("enter"), Some(AppAction::SubmitEditor));

    let mut store = ttd::store::TaskStore::open(root.clone()).unwrap();
    app.save_editor(&mut store).unwrap();

    assert!(app.editor.is_none());

    let snapshot = store.load_all().unwrap();
    assert_eq!(snapshot.open_tasks.len(), 1);
    assert_eq!(snapshot.open_tasks[0].task.raw, "Call Mom +Family");
}

#[test]
fn conflict_choices_reload_overwrite_or_keep_local_draft() {
    let task_id = TaskId {
        path: PathBuf::from("/tmp/inbox.txt"),
        line_index: 0,
    };
    let selected = SelectedTask::new(task_id.clone(), "Call Mom due:2026-03-31");

    let mut cancel_app = AppState::new(AppMode::Main);
    cancel_app.selected_task = Some(selected.clone());
    cancel_app.handle_key("e");
    for key in [" ", "@", "h", "o", "m", "e"] {
        assert_eq!(
            cancel_app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }
    assert_eq!(
        cancel_app.handle_key("enter"),
        Some(AppAction::SubmitEditor)
    );
    let mut cancel_saver = ScriptedSaver::new(vec![EditorSaveResult::Conflict {
        external_raw: "Call Mom due:2026-03-30".into(),
    }]);
    cancel_app.save_editor(&mut cancel_saver).unwrap();
    assert_eq!(
        cancel_app.handle_key("c"),
        Some(AppAction::ResolveConflict(ConflictChoice::Cancel))
    );
    cancel_app
        .resolve_save_conflict(ConflictChoice::Cancel, &mut cancel_saver)
        .unwrap();
    assert!(cancel_app.save_conflict.is_none());
    assert_eq!(
        cancel_app.editor.as_ref().unwrap().raw_line,
        "Call Mom due:2026-03-31 @home"
    );

    let mut reload_app = AppState::new(AppMode::Main);
    reload_app.selected_task = Some(selected.clone());
    reload_app.handle_key("e");
    for key in [" ", "@", "w", "o", "r", "k"] {
        assert_eq!(
            reload_app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }
    assert_eq!(
        reload_app.handle_key("enter"),
        Some(AppAction::SubmitEditor)
    );
    let mut reload_saver = ScriptedSaver::new(vec![EditorSaveResult::Conflict {
        external_raw: "Call Mom due:2026-03-30".into(),
    }]);
    reload_app.save_editor(&mut reload_saver).unwrap();
    assert_eq!(
        reload_app.handle_key("r"),
        Some(AppAction::ResolveConflict(ConflictChoice::ReloadExternal))
    );
    reload_app
        .resolve_save_conflict(ConflictChoice::ReloadExternal, &mut reload_saver)
        .unwrap();
    assert!(reload_app.save_conflict.is_none());
    assert_eq!(
        reload_app.editor.as_ref().unwrap().raw_line,
        "Call Mom due:2026-03-30"
    );
    assert_eq!(
        reload_app.editor.as_ref().unwrap().original_raw.as_deref(),
        Some("Call Mom due:2026-03-30")
    );

    let mut overwrite_app = AppState::new(AppMode::Main);
    overwrite_app.selected_task = Some(selected);
    overwrite_app.handle_key("e");
    for key in [" ", "@", "e", "r", "r", "a", "n", "d", "s"] {
        assert_eq!(
            overwrite_app.handle_key(key),
            Some(AppAction::AppendToEditor(key.to_string()))
        );
    }
    assert_eq!(
        overwrite_app.handle_key("enter"),
        Some(AppAction::SubmitEditor)
    );
    let mut overwrite_saver = ScriptedSaver::new(vec![
        EditorSaveResult::Conflict {
            external_raw: "Call Mom due:2026-03-30".into(),
        },
        EditorSaveResult::Saved,
    ]);
    overwrite_app.save_editor(&mut overwrite_saver).unwrap();
    assert_eq!(
        overwrite_app.handle_key("o"),
        Some(AppAction::ResolveConflict(
            ConflictChoice::OverwriteExternal
        ))
    );
    overwrite_app
        .resolve_save_conflict(ConflictChoice::OverwriteExternal, &mut overwrite_saver)
        .unwrap();
    assert!(overwrite_app.save_conflict.is_none());
    assert!(overwrite_app.editor.is_none());

    let requests = overwrite_saver.take_requests();
    assert_eq!(requests.len(), 2);
    match &requests[1] {
        EditorSaveRequest::Update {
            id,
            overwrite_conflict,
            raw,
            ..
        } => {
            assert_eq!(id, &task_id);
            assert!(*overwrite_conflict);
            assert_eq!(raw, "Call Mom due:2026-03-31 @errands");
        }
        request => panic!("expected update request, got {request:?}"),
    }
}

#[test]
fn render_shows_editor_modal_and_conflict_prompt_when_active() {
    let mut app = AppState::new(AppMode::Main);
    assert_eq!(app.handle_key("a"), Some(AppAction::AddTask));
    for key in ["d", "r", "a", "f", "t", " ", "t", "a", "s", "k"] {
        let _ = app.handle_key(key);
    }

    let text = buffer_text(&render(&app));
    assert!(text.contains("Quick Entry"));
    assert!(text.contains("draft task"));
    assert!(text.contains("enter save"));
    assert!(text.contains("ctrl+d due"));
    assert!(text.contains("ctrl+s sched"));
    assert!(text.contains("ctrl+t start"));

    let mut saver = ConflictSaver::default();
    app.selected_task = Some(SelectedTask::new(
        TaskId {
            path: PathBuf::from("/tmp/conflict.txt"),
            line_index: 0,
        },
        "Draft task",
    ));
    app.editor = Some(ttd::tui::editor::EditorState::edit(
        app.selected_task.as_ref().unwrap(),
    ));
    for key in [" ", "u", "p", "d", "a", "t", "e", "d"] {
        let _ = app.handle_key(key);
    }
    assert_eq!(app.handle_key("enter"), Some(AppAction::SubmitEditor));
    app.save_editor(&mut saver).unwrap();

    let conflict_text = buffer_text(&render(&app));
    assert!(conflict_text.contains("r reload"));
    assert!(conflict_text.contains("o overwrite"));
    assert!(conflict_text.contains("c cancel"));
}

#[test]
fn task_store_edit_save_fails_immediately_when_file_is_locked() {
    let root = temp_path("locked-edit-save");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    let task_path = root.join("todo.txt");
    fs::write(&task_path, "Call Mom\n").unwrap();

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&task_path)
        .unwrap();
    file.lock().unwrap();

    let mut store = TaskStore::open(root.clone()).unwrap();
    let result = store.save_editor(EditorSaveRequest::Update {
        id: TaskId {
            path: task_path.clone(),
            line_index: 0,
        },
        original_raw: "Call Mom".into(),
        raw: "Call Dad".into(),
        overwrite_conflict: false,
    });

    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);

    file.unlock().unwrap();
    assert_eq!(fs::read_to_string(&task_path).unwrap(), "Call Mom\n");
}

#[derive(Default)]
struct ConflictSaver {
    requests: Vec<EditorSaveRequest>,
}

impl EditorSaveTarget for ConflictSaver {
    fn save_editor(&mut self, request: EditorSaveRequest) -> io::Result<EditorSaveResult> {
        self.requests.push(request);
        Ok(EditorSaveResult::Conflict {
            external_raw: "(A) Call Mom due:2026-03-30".into(),
        })
    }
}

struct ScriptedSaver {
    requests: Vec<EditorSaveRequest>,
    results: Vec<EditorSaveResult>,
}

impl ScriptedSaver {
    fn new(results: Vec<EditorSaveResult>) -> Self {
        Self {
            requests: Vec::new(),
            results,
        }
    }

    fn take_requests(&mut self) -> Vec<EditorSaveRequest> {
        mem::take(&mut self.requests)
    }
}

impl EditorSaveTarget for ScriptedSaver {
    fn save_editor(&mut self, request: EditorSaveRequest) -> io::Result<EditorSaveResult> {
        self.requests.push(request);
        Ok(self.results.remove(0))
    }
}

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
    path.push(format!("ttd-tui-editor-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}
