use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;

use crate::parser::parse_task_line;
use crate::query::sort_tasks;
use crate::refresh::SnapshotIndex;
use crate::smartlist::{Directive, Direction};
use crate::store::{Snapshot, StoredTask, TaskId, TaskStore};
use crate::task::Task;
use crate::tui::app::{AppAction, AppMode, AppState, FocusArea};
use crate::tui::editor::{ConflictChoice, EditorState, SelectedTask};
use crate::tui::widgets::render_task_lines;
use ratatui::widgets::{Paragraph, Wrap};

pub struct ViewOverrides {
    pub sort: Option<Directive>,
    pub group: Option<Directive>,
    pub reversed: bool,
}

impl ViewOverrides {
    pub fn new() -> Self {
        Self { sort: None, group: None, reversed: false }
    }
    pub fn clear(&mut self) {
        self.sort = None;
        self.group = None;
        self.reversed = false;
    }
    pub fn has_sort_override(&self) -> bool {
        self.sort.is_some() || self.reversed
    }
    pub fn has_group_override(&self) -> bool {
        self.group.is_some()
    }
}

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
    task_scroll_override: Option<u16>,
    fs_index: Option<SnapshotIndex>,
    view_overrides: ViewOverrides,
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
            task_scroll_override: None,
            fs_index: None,
            view_overrides: ViewOverrides::new(),
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
            task_scroll_override: None,
            fs_index,
            view_overrides: ViewOverrides::new(),
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

    pub fn view_overrides(&self) -> &ViewOverrides {
        &self.view_overrides
    }

    pub fn override_indicator(&self) -> String {
        let mut parts = Vec::new();

        let sort_directives = self.effective_sort_directives();
        if self.view_overrides.has_sort_override() {
            if let Some(d) = sort_directives.first() {
                let arrow = match d.direction {
                    crate::smartlist::Direction::Asc => "\u{2191}",  // ↑
                    crate::smartlist::Direction::Desc => "\u{2193}", // ↓
                };
                parts.push(format!(
                    "[sort: {} {}]",
                    crate::smartlist::field_display_name(&d.field),
                    arrow
                ));
            }
        }

        if self.view_overrides.has_group_override() {
            let group_directives = self.effective_group_directives();
            if let Some(d) = group_directives.first() {
                parts.push(format!(
                    "[group: {}]",
                    crate::smartlist::field_display_name(&d.field),
                ));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!(" {}", parts.join(" "))
        }
    }

    pub fn set_sort_override(&mut self, directive: Directive) {
        let selected = self.current_selection_target();
        self.view_overrides.sort = Some(directive);
        self.view_overrides.reversed = false;
        self.rebuild_visible_tasks();
        self.reselect_task(selected);
    }

    pub fn set_group_override(&mut self, directive: Directive) {
        let selected = self.current_selection_target();
        self.view_overrides.group = Some(directive);
        self.rebuild_visible_tasks();
        self.reselect_task(selected);
    }

    pub fn clear_sort_override(&mut self) {
        let selected = self.current_selection_target();
        self.view_overrides.sort = None;
        self.view_overrides.reversed = false;
        self.rebuild_visible_tasks();
        self.reselect_task(selected);
    }

    pub fn clear_group_override(&mut self) {
        let selected = self.current_selection_target();
        self.view_overrides.group = None;
        self.rebuild_visible_tasks();
        self.reselect_task(selected);
    }

    pub fn toggle_reverse_sort(&mut self) {
        let selected = self.current_selection_target();
        self.view_overrides.reversed = !self.view_overrides.reversed;
        self.rebuild_visible_tasks();
        self.reselect_task(selected);
    }

    pub fn selected_task(&self) -> Option<&StoredTask> {
        self.selected_task_index
            .and_then(|index| self.visible_tasks.get(index))
    }

    pub fn task_scroll_offset_override(&self) -> Option<u16> {
        self.task_scroll_override
    }

    pub fn task_scroll_offset(&self) -> u16 {
        self.task_scroll_override.unwrap_or(0)
    }

    /// Map a visual row (0-based, relative to inner task pane area) to a task
    /// index. Accounts for scroll offset, search bar, group headers, text
    /// wrapping, and separator lines between tasks.
    pub fn task_index_for_visual_row(
        &self,
        visual_row: usize,
        task_pane_inner_width: u16,
    ) -> Option<usize> {
        if self.visible_tasks.is_empty() {
            return None;
        }

        let scroll = self.task_scroll_override.unwrap_or(0) as usize;
        let absolute_row = visual_row + scroll;

        let mut current_line = 0;

        // Account for search bar
        if self.app.search_active || !self.app.search_query.is_empty() {
            current_line += 2; // search line + blank line
        }

        let groups = self.visible_groups();
        let show_group_headers =
            groups.len() > 1 || groups.first().is_some_and(|g| !g.label.is_empty());

        let mut task_flat_index = 0;

        if show_group_headers {
            for (gi, group) in groups.iter().enumerate() {
                if !group.label.is_empty() {
                    if gi > 0 {
                        current_line += 1; // separator before group
                    }
                    current_line += 1; // group header
                }
                for (i, stored) in group.tasks.iter().enumerate() {
                    let task_start = current_line;
                    let line_count = visual_line_count_for_task(
                        &stored.task,
                        task_pane_inner_width,
                    );
                    current_line += line_count;
                    if absolute_row >= task_start && absolute_row < current_line {
                        return Some(task_flat_index);
                    }
                    if i < group.tasks.len() - 1 {
                        current_line += 1; // blank line between tasks
                    }
                    task_flat_index += 1;
                }
            }
        } else {
            for (i, stored) in self.visible_tasks.iter().enumerate() {
                let task_start = current_line;
                let line_count = visual_line_count_for_task(
                    &stored.task,
                    task_pane_inner_width,
                );
                current_line += line_count;
                if absolute_row >= task_start && absolute_row < current_line {
                    return Some(i);
                }
                if i < self.visible_tasks.len() - 1 {
                    current_line += 1; // separator
                }
            }
        }

        None
    }

    pub fn apply_task_scroll(&mut self, delta: isize, visual_line_count: usize, pane_height: usize) {
        let max_offset = visual_line_count.saturating_sub(pane_height);
        let current = self.task_scroll_override.unwrap_or(0) as isize;
        let new_offset = (current + delta).clamp(0, max_offset as isize) as u16;
        self.task_scroll_override = Some(new_offset);
    }

    pub fn apply_sidebar_scroll(&mut self, delta: isize) {
        self.move_sidebar(delta);
    }

    pub fn select_sidebar_item(&mut self, item: SidebarItem) {
        self.active_sidebar_item = item;
        self.view_overrides.clear();
        self.rebuild_visible_tasks();
    }

    pub fn dispatch_mouse_sidebar(&mut self, sidebar_index: usize) {
        if let Some(item) = self.sidebar_items.get(sidebar_index).cloned() {
            self.app.focus = FocusArea::Sidebar;
            self.select_sidebar_item(item);
        }
    }

    pub fn dispatch_mouse_task_select(&mut self, task_index: usize) {
        if task_index < self.visible_tasks.len() {
            self.app.focus = FocusArea::TaskList;
            self.selected_task_index = Some(task_index);
            let scroll = self.task_scroll_override;
            self.sync_selected_task();
            self.task_scroll_override = scroll;
        }
    }

    pub fn dispatch_mouse_task_edit(&mut self) -> io::Result<()> {
        if let Some(action) = self.app.handle_key("e") {
            self.apply_action(action)?;
        }
        Ok(())
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

    fn effective_sort_directives(&self) -> Vec<Directive> {
        let base = if let Some(ref sort) = self.view_overrides.sort {
            vec![sort.clone()]
        } else if let Some(smart_list) = self.smart_list_for_active() {
            smart_list.sort_directives.clone()
        } else {
            Vec::new()
        };
        if self.view_overrides.reversed && !base.is_empty() {
            let mut flipped = base;
            flipped[0].direction = match flipped[0].direction {
                Direction::Asc => Direction::Desc,
                Direction::Desc => Direction::Asc,
            };
            flipped
        } else {
            base
        }
    }

    fn effective_group_directives(&self) -> Vec<Directive> {
        if let Some(ref group) = self.view_overrides.group {
            vec![group.clone()]
        } else if let Some(smart_list) = self.smart_list_for_active() {
            smart_list.group_directives.clone()
        } else {
            Vec::new()
        }
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
        self.task_scroll_override = None;
        let mut tasks = apply_search_filter(
            filter_snapshot(
                &self.snapshot,
                &self.active_sidebar_item,
                &self.today,
                &self.smart_lists,
            ),
            &self.app.search_query,
        );
        let sort_directives = self.effective_sort_directives();
        if !sort_directives.is_empty() {
            crate::smartlist::sort_by_directives(&mut tasks, &sort_directives);
        }
        if self.view_overrides.reversed && sort_directives.is_empty() {
            tasks.reverse();
        }
        self.visible_tasks = tasks;
        let group_directives = self.effective_group_directives();
        self.visible_groups = crate::smartlist::group_by_directives(&group_directives, &self.visible_tasks);
        // When a sort override is active, re-sort tasks within each group and
        // reorder the groups themselves so that sorting is visible even when
        // grouping is enabled.
        let has_groups = self.visible_groups.len() > 1
            || self.visible_groups.first().is_some_and(|g| !g.label.is_empty());
        if has_groups && !sort_directives.is_empty() {
            for group in &mut self.visible_groups {
                crate::smartlist::sort_by_directives(&mut group.tasks, &sort_directives);
            }
            // If sort and group share the same field, reorder groups by sort direction.
            if let (Some(sd), Some(gd)) = (sort_directives.first(), group_directives.first()) {
                if sd.field == gd.field {
                    self.visible_groups.sort_by(|a, b| match sd.direction {
                        Direction::Asc => a.label.cmp(&b.label),
                        Direction::Desc => b.label.cmp(&a.label),
                    });
                }
            }
        }
        // Rebuild visible_tasks to match the visual (grouped) order so that
        // j/k selection follows what the user sees on screen.
        if has_groups {
            self.visible_tasks = self
                .visible_groups
                .iter()
                .flat_map(|g| g.tasks.iter().cloned())
                .collect();
        }
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
                if let Some(stored) = self.selected_task() {
                    let task_id = stored.id.clone();
                    if stored.task.done {
                        self.store_mut()?.restore_task(&task_id)?;
                    } else {
                        let today = self.today.clone();
                        self.store_mut()?.mark_done(&task_id, &today)?;
                    }
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
            AppAction::AddTask => {
                let suffix = match &self.active_sidebar_item {
                    SidebarItem::Project(value) | SidebarItem::Context(value) => {
                        Some(value.clone())
                    }
                    _ => None,
                };
                if let Some(suffix) = suffix {
                    if let Some(editor) = self.app.editor.as_mut() {
                        editor.set_suffix(&suffix);
                    }
                }
            }
            AppAction::PickerSelect => {
                if let Some(picker) = self.app.picker.take() {
                    let directive = crate::smartlist::Directive {
                        field: picker.selected_field().clone(),
                        direction: crate::smartlist::Direction::Asc,
                    };
                    match picker.kind {
                        crate::tui::app::PickerKind::Sort => {
                            self.view_overrides.sort = Some(directive);
                            self.view_overrides.reversed = false;
                        }
                        crate::tui::app::PickerKind::Group => {
                            self.view_overrides.group = Some(directive);
                        }
                    }
                    let wanted = self.current_selection_target();
                    self.rebuild_visible_tasks();
                    self.reselect_task(wanted);
                }
            }
            AppAction::DeactivateSort => {
                if self.view_overrides.has_sort_override() {
                    self.view_overrides.sort = None;
                    self.view_overrides.reversed = false;
                    let wanted = self.current_selection_target();
                    self.rebuild_visible_tasks();
                    self.reselect_task(wanted);
                }
            }
            AppAction::DeactivateGroup => {
                if self.view_overrides.has_group_override() {
                    self.view_overrides.group = None;
                    let wanted = self.current_selection_target();
                    self.rebuild_visible_tasks();
                    self.reselect_task(wanted);
                }
            }
            AppAction::ReverseSort => {
                self.view_overrides.reversed = !self.view_overrides.reversed;
                let wanted = self.current_selection_target();
                self.rebuild_visible_tasks();
                self.reselect_task(wanted);
            }
            AppAction::OpenSortPicker | AppAction::OpenGroupPicker => {}
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
        self.task_scroll_override = None;
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
                crate::smartlist::filter_only(smart_list, &all_tasks, today)
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

/// Compute the true visual line count for a task, matching what
/// `Paragraph::wrap` produces at render time.
fn visual_line_count_for_task(task: &Task, width: u16) -> usize {
    if width == 0 {
        return 1;
    }
    let lines = render_task_lines(task, false, width);
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .line_count(width)
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
