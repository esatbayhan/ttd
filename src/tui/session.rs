use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;

use crate::parser::parse_task_line;
use crate::query::sort_tasks;
use crate::refresh::SnapshotIndex;
use crate::store::{Snapshot, StoredTask, TaskId, TaskStore};
use crate::task::Task;
use crate::tui::app::{AppAction, AppMode, AppState, FocusArea};
use crate::tui::editor::{ConflictChoice, EditorState, SelectedTask};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarItem {
    SmartList(usize),
    Separator,
    ProjectsHeader,
    Project(String),
    ContextsHeader,
    Context(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogicalTaskKey {
    priority: Option<char>,
    creation_date: Option<String>,
    description: String,
    projects: Vec<String>,
    contexts: Vec<String>,
    tags: BTreeMap<String, String>,
}

impl LogicalTaskKey {
    fn from_task(task: &Task) -> Self {
        Self {
            priority: task.priority,
            creation_date: task.creation_date.clone(),
            description: task.description.clone(),
            projects: task.projects.clone(),
            contexts: task.contexts.clone(),
            tags: task.tags.clone(),
        }
    }

    fn matches(&self, task: &Task) -> bool {
        self.priority == task.priority
            && self.creation_date == task.creation_date
            && self.description == task.description
            && self.projects == task.projects
            && self.contexts == task.contexts
            && self.tags == task.tags
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectionTarget {
    task_id: Option<TaskId>,
    file_name: Option<String>,
    raw: Option<String>,
    logical: Option<LogicalTaskKey>,
    fallback_index: Option<usize>,
}

impl SelectionTarget {
    fn from_stored(stored: &StoredTask, index: Option<usize>) -> Self {
        Self {
            task_id: Some(stored.id.clone()),
            file_name: Some(stored.id.file_name().to_string()),
            raw: Some(stored.task.raw.clone()),
            logical: Some(LogicalTaskKey::from_task(&stored.task)),
            fallback_index: index,
        }
    }

    fn from_editor(editor: &EditorState, fallback_index: Option<usize>) -> Self {
        let parsed = parse_task_line(&editor.raw_line);
        Self {
            task_id: editor.task_id.clone(),
            file_name: editor.task_id.as_ref().map(|id| id.file_name().to_string()),
            raw: Some(editor.raw_line.clone()),
            logical: Some(LogicalTaskKey::from_task(&parsed)),
            fallback_index: if editor.task_id.is_some() {
                fallback_index
            } else {
                None
            },
        }
    }
}

pub struct TuiSession {
    app: AppState,
    store: Option<TaskStore>,
    today: String,
    snapshot: Snapshot,
    smart_lists: Vec<crate::smartlist::SmartList>,
    sidebar_items: Vec<SidebarItem>,
    active_sidebar_item: SidebarItem,
    visible_tasks: Vec<StoredTask>,
    visible_groups: Vec<crate::smartlist::TaskGroup>,
    selected_task_index: Option<usize>,
    fs_index: Option<SnapshotIndex>,
}

impl TuiSession {
    pub fn from_launch_mode(
        launch_mode: crate::bootstrap::LaunchMode,
        today: &str,
    ) -> io::Result<Self> {
        match launch_mode {
            crate::bootstrap::LaunchMode::Welcome => Ok(Self::welcome(today)),
            crate::bootstrap::LaunchMode::Main(config) => Self::open(config.task_dir, today),
        }
    }

    pub fn welcome(today: &str) -> Self {
        Self {
            app: AppState::new(AppMode::Welcome),
            store: None,
            today: today.to_string(),
            snapshot: Snapshot {
                open_tasks: Vec::new(),
                done_tasks: Vec::new(),
            },
            smart_lists: Vec::new(),
            sidebar_items: Vec::new(),
            active_sidebar_item: SidebarItem::SmartList(0),
            visible_tasks: Vec::new(),
            visible_groups: Vec::new(),
            selected_task_index: None,
            fs_index: None,
        }
    }

    pub fn open(root: PathBuf, today: &str) -> io::Result<Self> {
        let store = TaskStore::open(root)?;
        let snapshot = store.load_all()?;
        let fs_index = Some(store.snapshot_index()?);
        let smart_lists = crate::smartlist::load_all(&store.lists_dir());
        let default_sidebar = SidebarItem::SmartList(0);
        let mut session = Self {
            app: AppState::new(AppMode::Main),
            store: Some(store),
            today: today.to_string(),
            snapshot,
            smart_lists,
            sidebar_items: Vec::new(),
            active_sidebar_item: default_sidebar,
            visible_tasks: Vec::new(),
            visible_groups: Vec::new(),
            selected_task_index: None,
            fs_index,
        };
        session.rebuild();
        Ok(session)
    }

    pub fn sidebar_items(&self) -> &[SidebarItem] {
        &self.sidebar_items
    }

    pub fn app(&self) -> &AppState {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut AppState {
        &mut self.app
    }

    pub fn active_sidebar_item(&self) -> SidebarItem {
        self.active_sidebar_item.clone()
    }

    pub fn visible_tasks(&self) -> &[StoredTask] {
        &self.visible_tasks
    }

    pub fn visible_groups(&self) -> &[crate::smartlist::TaskGroup] {
        &self.visible_groups
    }

    pub fn smart_lists(&self) -> &[crate::smartlist::SmartList] {
        &self.smart_lists
    }

    pub fn smart_list_for_active(&self) -> Option<&crate::smartlist::SmartList> {
        match &self.active_sidebar_item {
            SidebarItem::SmartList(index) => self.smart_lists.get(*index),
            _ => None,
        }
    }

    pub fn selected_task(&self) -> Option<&StoredTask> {
        self.selected_task_index
            .and_then(|index| self.visible_tasks.get(index))
    }

    pub fn select_sidebar_item(&mut self, item: SidebarItem) {
        self.active_sidebar_item = item;
        self.rebuild_visible_tasks();
    }

    pub fn refresh(&mut self) -> io::Result<()> {
        let selected = self.current_selection_target();
        self.snapshot = self.store()?.load_all()?;
        self.fs_index = Some(self.store()?.snapshot_index()?);
        self.rebuild();
        self.reselect_task(selected);
        Ok(())
    }

    pub fn dispatch_key(&mut self, key: &str) -> io::Result<()> {
        let Some(action) = self.app.handle_key(key) else {
            return Ok(());
        };

        match action {
            AppAction::MoveDown => self.move_selection(1),
            AppAction::MoveUp => self.move_selection(-1),
            AppAction::MoveTop => self.move_to_edge(true),
            AppAction::MoveBottom => self.move_to_edge(false),
            other => self.apply_action(other)?,
        }

        Ok(())
    }

    pub fn dispatch_key_with_paths(
        &mut self,
        key: &str,
        paths: &crate::config::ConfigPaths,
    ) -> io::Result<()> {
        let Some(action) = self.app.handle_key(key) else {
            return Ok(());
        };

        match action {
            AppAction::SubmitWelcomePath(path) => {
                if path.trim().is_empty() {
                    return Ok(());
                }

                let task_dir = PathBuf::from(path.trim());
                crate::config::validate_task_dir(&task_dir)?;
                crate::config::AppConfig {
                    task_dir: task_dir.clone(),
                }
                .save(paths)?;
                *self = Self::open(task_dir, &self.today)?;
            }
            AppAction::MoveDown => self.move_selection(1),
            AppAction::MoveUp => self.move_selection(-1),
            AppAction::MoveTop => self.move_to_edge(true),
            AppAction::MoveBottom => self.move_to_edge(false),
            other => self.apply_action(other)?,
        }

        Ok(())
    }

    fn rebuild(&mut self) {
        self.sidebar_items = build_sidebar_items(&self.smart_lists, &self.snapshot);
        if !self.sidebar_items.contains(&self.active_sidebar_item) {
            self.active_sidebar_item = self
                .sidebar_items
                .iter()
                .find(|item| {
                    matches!(
                        item,
                        SidebarItem::SmartList(_)
                            | SidebarItem::Project(_)
                            | SidebarItem::Context(_)
                    )
                })
                .cloned()
                .unwrap_or(SidebarItem::SmartList(0));
        }
        self.rebuild_visible_tasks();
    }

    fn rebuild_visible_tasks(&mut self) {
        self.visible_tasks = apply_search_filter(
            filter_snapshot(
                &self.snapshot,
                &self.active_sidebar_item,
                &self.today,
                &self.smart_lists,
            ),
            &self.app.search_query,
        );
        self.visible_groups = if let Some(smart_list) = self.smart_list_for_active() {
            crate::smartlist::group(smart_list, &self.visible_tasks)
        } else {
            vec![crate::smartlist::TaskGroup {
                label: String::new(),
                tasks: self.visible_tasks.clone(),
            }]
        };
        self.selected_task_index = (!self.visible_tasks.is_empty()).then_some(0);
        self.sync_selected_task();
    }

    fn apply_action(&mut self, action: AppAction) -> io::Result<()> {
        match action {
            AppAction::AppendToSearch(_) | AppAction::BackspaceSearch | AppAction::Cancel
                if self.app.mode == AppMode::Main =>
            {
                let wanted = self.current_selection_target();
                self.rebuild();
                self.reselect_task(wanted);
            }
            AppAction::NextSearchResult => {
                self.move_search_result(1);
            }
            AppAction::PreviousSearchResult => {
                self.move_search_result(-1);
            }
            AppAction::OpenSelected if self.app.confirm_delete => {
                let wanted = self.current_selection_target();
                if let Some(task_id) = self.selected_task().map(|stored| stored.id.clone()) {
                    self.store_mut()?.delete_task(&task_id)?;
                    self.app.confirm_delete = false;
                    self.refresh_to_target(wanted)?;
                }
            }
            AppAction::SubmitEditor => {
                let editor = self.app.editor.clone();
                let previous_snapshot = self.snapshot.clone();
                {
                    let app = &mut self.app;
                    let store = self
                        .store
                        .as_mut()
                        .ok_or_else(|| io::Error::other("session store is not initialized"))?;
                    app.save_editor(store)?;
                }
                if self.app.save_conflict.is_none() {
                    let next_snapshot = self.store()?.load_all()?;
                    let wanted = editor.as_ref().and_then(|editor| {
                        if editor.task_id.is_none() {
                            created_task_target(
                                &previous_snapshot,
                                &next_snapshot,
                                self.selected_task_index,
                            )
                            .or_else(|| {
                                Some(SelectionTarget::from_editor(
                                    editor,
                                    self.selected_task_index,
                                ))
                            })
                        } else {
                            Some(SelectionTarget::from_editor(
                                editor,
                                self.selected_task_index,
                            ))
                        }
                    });
                    self.replace_snapshot_and_reselect(next_snapshot, wanted);
                }
            }
            AppAction::ToggleDone => {
                let wanted = self.current_selection_target();
                if let Some(task_id) = self
                    .selected_task()
                    .filter(|stored| !stored.task.done)
                    .map(|stored| stored.id.clone())
                {
                    let today = self.today.clone();
                    self.store_mut()?.mark_done(&task_id, &today)?;
                    self.refresh_to_target(wanted)?;
                }
            }
            AppAction::RestoreCompleted => {
                let wanted = self.current_selection_target();
                if let Some(task_id) = self
                    .selected_task()
                    .filter(|stored| stored.task.done)
                    .map(|stored| stored.id.clone())
                {
                    self.store_mut()?.restore_task(&task_id)?;
                    self.refresh_to_target(wanted)?;
                }
            }
            AppAction::Refresh => {
                self.refresh()?;
            }
            AppAction::ResolveConflict(choice) => {
                {
                    let app = &mut self.app;
                    let store = self
                        .store
                        .as_mut()
                        .ok_or_else(|| io::Error::other("session store is not initialized"))?;
                    app.resolve_save_conflict(choice, store)?;
                }
                if choice == ConflictChoice::OverwriteExternal && self.app.save_conflict.is_none() {
                    self.refresh()?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn move_selection(&mut self, delta: isize) {
        match self.app.focus {
            FocusArea::Sidebar => self.move_sidebar(delta),
            FocusArea::TaskList => self.move_task_list(delta),
        }
    }

    fn move_to_edge(&mut self, top: bool) {
        match self.app.focus {
            FocusArea::Sidebar => {
                let selectable = self.selectable_sidebar_indices();
                let target = if top {
                    selectable.first().copied()
                } else {
                    selectable.last().copied()
                };
                if let Some(index) = target {
                    self.select_sidebar_item(self.sidebar_items[index].clone());
                }
            }
            FocusArea::TaskList => {
                if self.visible_tasks.is_empty() {
                    self.selected_task_index = None;
                } else {
                    self.selected_task_index =
                        Some(if top { 0 } else { self.visible_tasks.len() - 1 });
                }
                self.sync_selected_task();
            }
        }
    }

    fn move_sidebar(&mut self, delta: isize) {
        let selectable = self.selectable_sidebar_indices();
        if selectable.is_empty() {
            return;
        }

        let current = selectable
            .iter()
            .position(|index| self.sidebar_items[*index] == self.active_sidebar_item)
            .unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, selectable.len().saturating_sub(1) as isize);
        self.select_sidebar_item(self.sidebar_items[selectable[next as usize]].clone());
    }

    fn move_task_list(&mut self, delta: isize) {
        if self.visible_tasks.is_empty() {
            self.selected_task_index = None;
            self.sync_selected_task();
            return;
        }

        let current = self.selected_task_index.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.visible_tasks.len().saturating_sub(1) as isize);
        self.selected_task_index = Some(next as usize);
        self.sync_selected_task();
    }

    fn move_search_result(&mut self, delta: isize) {
        if self.app.search_query.is_empty() || self.visible_tasks.is_empty() {
            return;
        }

        let len = self.visible_tasks.len() as isize;
        let current = self.selected_task_index.unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len);
        self.selected_task_index = Some(next as usize);
        self.sync_selected_task();
    }

    fn selectable_sidebar_indices(&self) -> Vec<usize> {
        self.sidebar_items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                SidebarItem::ProjectsHeader
                | SidebarItem::ContextsHeader
                | SidebarItem::Separator => None,
                _ => Some(index),
            })
            .collect()
    }

    fn sync_selected_task(&mut self) {
        self.app.selected_task = self.selected_task_index.and_then(|index| {
            self.visible_tasks.get(index).map(|stored| {
                SelectedTask::with_original_raw(
                    stored.id.clone(),
                    stored.task.raw.clone(),
                    stored.task.raw.clone(),
                )
            })
        });
    }

    fn current_selection_target(&self) -> Option<SelectionTarget> {
        self.selected_task()
            .map(|stored| SelectionTarget::from_stored(stored, self.selected_task_index))
    }

    fn refresh_to_target(&mut self, wanted: Option<SelectionTarget>) -> io::Result<()> {
        self.snapshot = self.store()?.load_all()?;
        self.fs_index = Some(self.store()?.snapshot_index()?);
        self.rebuild();
        self.reselect_task(wanted);
        Ok(())
    }

    fn replace_snapshot_and_reselect(
        &mut self,
        snapshot: Snapshot,
        wanted: Option<SelectionTarget>,
    ) {
        self.snapshot = snapshot;
        self.rebuild();
        self.reselect_task(wanted);
    }

    fn reselect_task(&mut self, wanted: Option<SelectionTarget>) {
        self.selected_task_index = wanted
            .as_ref()
            .and_then(|target| {
                target.task_id.as_ref().and_then(|id| {
                    self.visible_tasks
                        .iter()
                        .position(|stored| stored.id == *id)
                })
            })
            .or_else(|| {
                wanted.as_ref().and_then(|target| {
                    target.file_name.as_ref().and_then(|file_name| {
                        self.visible_tasks
                            .iter()
                            .position(|stored| stored.id.file_name() == file_name)
                    })
                })
            })
            .or_else(|| {
                wanted.as_ref().and_then(|target| {
                    target.raw.as_ref().and_then(|raw| {
                        self.visible_tasks
                            .iter()
                            .position(|stored| stored.task.raw == *raw)
                    })
                })
            })
            .or_else(|| {
                wanted.as_ref().and_then(|target| {
                    target.logical.as_ref().and_then(|logical| {
                        self.visible_tasks
                            .iter()
                            .position(|stored| logical.matches(&stored.task))
                    })
                })
            })
            .or_else(|| {
                wanted.as_ref().and_then(|target| {
                    target
                        .fallback_index
                        .filter(|_| !self.visible_tasks.is_empty())
                        .map(|index| index.min(self.visible_tasks.len() - 1))
                })
            })
            .or_else(|| (!self.visible_tasks.is_empty()).then_some(0));
        self.sync_selected_task();
    }

    pub fn can_auto_refresh(&self) -> bool {
        self.app.mode == AppMode::Main
            && self.store.is_some()
            && self.app.editor.is_none()
            && self.app.save_conflict.is_none()
            && !self.app.confirm_delete
    }

    pub fn poll_refresh(&mut self) -> io::Result<bool> {
        if !self.can_auto_refresh() {
            return Ok(false);
        }

        let current = self.store()?.snapshot_index()?;
        let changed = match &self.fs_index {
            Some(previous) => previous.has_changes(&current),
            None => true,
        };

        if changed {
            self.refresh()?;
        }

        Ok(changed)
    }

    fn store(&self) -> io::Result<&TaskStore> {
        self.store
            .as_ref()
            .ok_or_else(|| io::Error::other("session store is not initialized"))
    }

    fn store_mut(&mut self) -> io::Result<&mut TaskStore> {
        self.store
            .as_mut()
            .ok_or_else(|| io::Error::other("session store is not initialized"))
    }
}

fn created_task_target(
    before: &Snapshot,
    after: &Snapshot,
    fallback_index: Option<usize>,
) -> Option<SelectionTarget> {
    after
        .open_tasks
        .iter()
        .chain(after.done_tasks.iter())
        .find(|candidate| !snapshot_contains_task(before, &candidate.id))
        .map(|stored| SelectionTarget::from_stored(stored, fallback_index))
}

fn snapshot_contains_task(snapshot: &Snapshot, wanted: &TaskId) -> bool {
    snapshot
        .open_tasks
        .iter()
        .chain(snapshot.done_tasks.iter())
        .any(|stored| stored.id == *wanted)
}

fn build_sidebar_items(
    smart_lists: &[crate::smartlist::SmartList],
    snapshot: &Snapshot,
) -> Vec<SidebarItem> {
    let mut items: Vec<SidebarItem> = smart_lists
        .iter()
        .enumerate()
        .map(|(index, _)| SidebarItem::SmartList(index))
        .collect();

    if !items.is_empty() {
        items.push(SidebarItem::Separator);
    }

    items.push(SidebarItem::ProjectsHeader);

    let mut projects = BTreeSet::new();
    let mut contexts = BTreeSet::new();
    for stored in snapshot.open_tasks.iter().chain(snapshot.done_tasks.iter()) {
        for project in &stored.task.projects {
            projects.insert(format!("+{project}"));
        }
        for context in &stored.task.contexts {
            contexts.insert(format!("@{context}"));
        }
    }

    items.extend(projects.into_iter().map(SidebarItem::Project));
    items.push(SidebarItem::Separator);
    items.push(SidebarItem::ContextsHeader);
    items.extend(contexts.into_iter().map(SidebarItem::Context));
    items
}

fn filter_snapshot(
    snapshot: &Snapshot,
    active: &SidebarItem,
    today: &str,
    smart_lists: &[crate::smartlist::SmartList],
) -> Vec<StoredTask> {
    match active {
        SidebarItem::SmartList(index) => {
            if let Some(smart_list) = smart_lists.get(*index) {
                if smart_list.parse_error.is_some() {
                    return Vec::new();
                }
                let all_tasks: Vec<StoredTask> = if needs_done_tasks(smart_list) {
                    snapshot
                        .open_tasks
                        .iter()
                        .chain(snapshot.done_tasks.iter())
                        .cloned()
                        .collect()
                } else {
                    snapshot.open_tasks.clone()
                };
                crate::smartlist::evaluate(smart_list, &all_tasks, today)
            } else {
                Vec::new()
            }
        }
        SidebarItem::Project(project) => {
            let ordered = ordered_tasks(snapshot, today);
            ordered
                .into_iter()
                .filter(|stored| {
                    stored
                        .task
                        .projects
                        .iter()
                        .any(|value| value == project.strip_prefix('+').unwrap_or(project))
                })
                .collect()
        }
        SidebarItem::Context(context) => {
            let ordered = ordered_tasks(snapshot, today);
            ordered
                .into_iter()
                .filter(|stored| {
                    stored
                        .task
                        .contexts
                        .iter()
                        .any(|value| value == context.strip_prefix('@').unwrap_or(context))
                })
                .collect()
        }
        SidebarItem::ProjectsHeader | SidebarItem::ContextsHeader | SidebarItem::Separator => {
            Vec::new()
        }
    }
}

fn needs_done_tasks(list: &crate::smartlist::SmartList) -> bool {
    list.blocks.iter().any(|block| {
        block
            .conditions
            .iter()
            .any(|c| matches!(c, crate::smartlist::Condition::DoneFilter { done: true }))
    })
}

fn apply_search_filter(tasks: Vec<StoredTask>, query: &str) -> Vec<StoredTask> {
    if query.is_empty() {
        return tasks;
    }

    let query_lower = query.to_lowercase();
    tasks
        .into_iter()
        .filter(|stored| stored.task.raw.to_lowercase().contains(&query_lower))
        .collect()
}

fn ordered_tasks(snapshot: &Snapshot, today: &str) -> Vec<StoredTask> {
    let mut tasks = snapshot
        .open_tasks
        .iter()
        .chain(snapshot.done_tasks.iter())
        .map(|stored| stored.task.clone())
        .collect::<Vec<_>>();
    sort_tasks(&mut tasks, today);

    let mut used = vec![false; snapshot.open_tasks.len() + snapshot.done_tasks.len()];
    let stored_tasks = snapshot
        .open_tasks
        .iter()
        .chain(snapshot.done_tasks.iter())
        .cloned()
        .collect::<Vec<_>>();

    tasks
        .into_iter()
        .filter_map(|task| {
            stored_tasks
                .iter()
                .enumerate()
                .find(|(index, stored)| !used[*index] && stored.task.raw == task.raw)
                .map(|(index, stored)| {
                    used[index] = true;
                    stored.clone()
                })
        })
        .collect()
}
