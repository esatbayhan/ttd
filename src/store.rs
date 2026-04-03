use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::parser::{is_date, parse_task_line};
use crate::refresh::SnapshotIndex;
use crate::task::Task;

const DONE_DIR: &str = "done.txt.d";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskId {
    pub path: PathBuf,
    pub line_index: usize,
}

impl TaskId {
    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct StoredTask {
    pub id: TaskId,
    pub task: Task,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub open_tasks: Vec<StoredTask>,
    pub done_tasks: Vec<StoredTask>,
}

pub struct TaskStore {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditSaveOutcome {
    Saved,
    Conflict { external_raw: String },
}

#[derive(Clone, Copy)]
enum ExpectedLocation {
    Open,
    Done,
    Either,
}

struct PlannedWrite {
    destination: PathBuf,
    contents: String,
}

struct NormalizationPlan {
    source: PathBuf,
    target: PathBuf,
    writes: Vec<PlannedWrite>,
    remove_source: bool,
}

impl TaskStore {
    pub fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(root.join(DONE_DIR))?;
        Ok(Self { root })
    }

    pub fn snapshot_index(&self) -> io::Result<SnapshotIndex> {
        let root_index = SnapshotIndex::scan(&self.root)?;
        let done_index = SnapshotIndex::scan(&self.done_dir())?;
        Ok(root_index.merge(&done_index))
    }

    pub fn load_all(&self) -> io::Result<Snapshot> {
        Ok(Snapshot {
            open_tasks: self.load_dir(&self.root)?,
            done_tasks: self.load_dir(&self.done_dir())?,
        })
    }

    pub fn create_task(&self, raw: &str) -> io::Result<TaskId> {
        ensure_single_task_input(raw)?;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        for attempt in 0..1024 {
            let path = self
                .root
                .join(format!("task-{}-{nonce}-{attempt}.txt", process::id()));
            if path.exists() {
                continue;
            }

            fs::write(&path, with_trailing_newline(raw))?;
            return Ok(TaskId {
                path,
                line_index: 0,
            });
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate a unique task path",
        ))
    }

    pub fn update_task(&self, id: &TaskId, raw: &str) -> io::Result<()> {
        ensure_single_task_input(raw)?;
        let source = self.validate_task_id(id, ExpectedLocation::Either)?;
        let plan = self.build_normalization_plan_from_content(
            &source,
            id.line_index,
            &fs::read_to_string(&source)?,
        )?;
        let path = self.materialize_normalization(plan)?;
        fs::write(path, with_trailing_newline(raw))
    }

    pub(crate) fn save_edited_task(
        &self,
        id: &TaskId,
        original_raw: &str,
        raw: &str,
        overwrite_conflict: bool,
    ) -> io::Result<EditSaveOutcome> {
        ensure_single_task_input(raw)?;
        let source = self.validate_task_id(id, ExpectedLocation::Either)?;
        let lock = OpenOptions::new().read(true).write(true).open(&source)?;
        lock.try_lock().map_err(|_| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "task file is locked by another process",
            )
        })?;

        let current_content = fs::read_to_string(&source)?;
        let current_raw = find_task_line(&current_content, id.line_index)?;
        if !overwrite_conflict && current_raw != original_raw {
            return Ok(EditSaveOutcome::Conflict {
                external_raw: current_raw,
            });
        }

        let plan =
            self.build_normalization_plan_from_content(&source, id.line_index, &current_content)?;
        let path = self.materialize_normalization(plan)?;
        let parsed = parse_task_line(raw);
        let is_done_destination = parsed.done;
        let destination = edited_task_destination(self, &path, is_done_destination)?;

        if destination == path {
            fs::write(path, with_trailing_newline(raw))?;
        } else {
            ensure_destination_available(&destination)?;
            fs::write(&destination, with_trailing_newline(raw))?;
            fs::remove_file(path)?;
        }
        Ok(EditSaveOutcome::Saved)
    }

    pub fn mark_done(&self, id: &TaskId, completion_date: &str) -> io::Result<()> {
        let source = self.validate_task_id(id, ExpectedLocation::Open)?;
        let plan = self.build_normalization_plan(&source, id.line_index)?;
        let destination = self
            .done_dir()
            .join(file_name(&plan.target).map(PathBuf::from)?);
        ensure_destination_available(&destination)?;

        let source = self.materialize_normalization(plan)?;
        let line = read_single_task_line(&source)?;
        let updated = format_done_line(&line, completion_date)?;

        fs::write(&destination, with_trailing_newline(&updated))?;
        fs::remove_file(source)
    }

    pub fn mark_done_by_name(&self, file_name: &str, completion_date: &str) -> io::Result<()> {
        let id = TaskId {
            path: self.root.join(file_name),
            line_index: 0,
        };
        self.mark_done(&id, completion_date)
    }

    pub fn restore_task(&self, id: &TaskId) -> io::Result<()> {
        let source = self.validate_task_id(id, ExpectedLocation::Done)?;
        let plan = self.build_normalization_plan(&source, id.line_index)?;
        let destination = self.root.join(file_name(&plan.target).map(PathBuf::from)?);
        ensure_destination_available(&destination)?;

        let source = self.materialize_normalization(plan)?;
        let line = read_single_task_line(&source)?;
        let updated = format_restored_line(&line)?;

        fs::write(&destination, with_trailing_newline(&updated))?;
        fs::remove_file(source)
    }

    pub fn delete_task(&self, id: &TaskId) -> io::Result<()> {
        let source = self.validate_task_id(id, ExpectedLocation::Either)?;
        let plan = self.build_normalization_plan(&source, id.line_index)?;
        let path = self.materialize_normalization(plan)?;
        fs::remove_file(path)
    }

    fn done_dir(&self) -> PathBuf {
        self.root.join(DONE_DIR)
    }

    pub fn lists_dir(&self) -> PathBuf {
        self.root.join("lists.d")
    }

    fn load_dir(&self, dir: &Path) -> io::Result<Vec<StoredTask>> {
        let mut entries = Vec::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }

            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("txt") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            for (line_index, line) in content.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }

                entries.push(StoredTask {
                    id: TaskId {
                        path: path.clone(),
                        line_index,
                    },
                    task: parse_task_line(line),
                });
            }
        }

        entries.sort_by(|left, right| {
            left.id
                .path
                .cmp(&right.id.path)
                .then(left.id.line_index.cmp(&right.id.line_index))
        });

        Ok(entries)
    }

    fn validate_task_id(&self, id: &TaskId, expected: ExpectedLocation) -> io::Result<PathBuf> {
        let parent = id.path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "task path did not have a parent directory",
            )
        })?;
        let done_dir = self.done_dir();

        let parent_ok = match expected {
            ExpectedLocation::Open => parent == self.root,
            ExpectedLocation::Done => parent == done_dir,
            ExpectedLocation::Either => parent == self.root || parent == done_dir,
        };

        if !parent_ok {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "task id does not belong to this store or expected state",
            ));
        }

        if id.path.extension().and_then(|value| value.to_str()) != Some("txt") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "task ids must point to .txt task files",
            ));
        }

        Ok(id.path.clone())
    }

    fn build_normalization_plan(
        &self,
        source: &Path,
        line_index: usize,
    ) -> io::Result<NormalizationPlan> {
        self.build_normalization_plan_from_content(source, line_index, &fs::read_to_string(source)?)
    }

    fn build_normalization_plan_from_content(
        &self,
        source: &Path,
        line_index: usize,
        content: &str,
    ) -> io::Result<NormalizationPlan> {
        let tasks = task_lines(content);

        if tasks.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "task file did not contain any task lines",
            ));
        }

        if !tasks
            .iter()
            .any(|(task_line_index, _)| *task_line_index == line_index)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "task line index does not exist in source file",
            ));
        }

        if tasks.len() == 1 {
            return Ok(NormalizationPlan {
                source: source.to_path_buf(),
                target: source.to_path_buf(),
                writes: Vec::new(),
                remove_source: false,
            });
        }

        let mut writes = Vec::with_capacity(tasks.len());
        let mut target = None;
        let mut source_is_reused = false;

        for (task_line_index, line) in tasks {
            let destination = normalized_path(source, task_line_index)?;
            if destination == source {
                source_is_reused = true;
            }
            if task_line_index == line_index {
                target = Some(destination.clone());
            }

            writes.push(PlannedWrite {
                destination,
                contents: with_trailing_newline(&line),
            });
        }

        Ok(NormalizationPlan {
            source: source.to_path_buf(),
            target: target.ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "task line index does not exist")
            })?,
            writes,
            remove_source: !source_is_reused,
        })
    }

    fn materialize_normalization(&self, plan: NormalizationPlan) -> io::Result<PathBuf> {
        if plan.writes.is_empty() {
            return Ok(plan.target);
        }

        preflight_normalization(&plan)?;

        let staged_dir = self.create_staging_dir()?;
        let mut staged = Vec::with_capacity(plan.writes.len());

        for (index, write) in plan.writes.iter().enumerate() {
            let staged_path = staged_dir.join(index.to_string());
            fs::write(&staged_path, &write.contents)?;
            staged.push(staged_path);
        }

        let commit_result = (|| -> io::Result<()> {
            for (write, staged_path) in plan.writes.iter().zip(staged.iter()) {
                if write.destination == plan.source {
                    let contents = fs::read_to_string(staged_path)?;
                    fs::write(&write.destination, contents)?;
                } else {
                    fs::rename(staged_path, &write.destination)?;
                }
            }

            if plan.remove_source {
                fs::remove_file(&plan.source)?;
            }

            Ok(())
        })();

        let _ = fs::remove_dir_all(&staged_dir);
        commit_result?;
        Ok(plan.target)
    }

    fn create_staging_dir(&self) -> io::Result<PathBuf> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = self
            .root
            .join(format!(".ttd-store-tmp-{}-{nonce}", process::id()));
        fs::create_dir(&path)?;
        Ok(path)
    }
}

fn preflight_normalization(plan: &NormalizationPlan) -> io::Result<()> {
    for write in &plan.writes {
        if write.destination == plan.source {
            continue;
        }

        if write.destination.exists() {
            let kind = if write.destination.is_dir() {
                io::ErrorKind::IsADirectory
            } else {
                io::ErrorKind::AlreadyExists
            };
            return Err(io::Error::new(
                kind,
                format!(
                    "normalization destination already exists: {}",
                    write.destination.display()
                ),
            ));
        }
    }

    Ok(())
}

fn ensure_destination_available(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let kind = if path.is_dir() {
        io::ErrorKind::IsADirectory
    } else {
        io::ErrorKind::AlreadyExists
    };

    Err(io::Error::new(
        kind,
        format!("destination already exists: {}", path.display()),
    ))
}

fn ensure_single_task_input(raw: &str) -> io::Result<()> {
    if raw.contains(['\n', '\r']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task updates must contain exactly one task line",
        ));
    }

    if raw.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task updates must not be blank",
        ));
    }

    Ok(())
}

fn task_lines(content: &str) -> Vec<(usize, String)> {
    content
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            if line.trim().is_empty() {
                None
            } else {
                Some((line_index, line.to_owned()))
            }
        })
        .collect()
}

fn read_single_task_line(path: &Path) -> io::Result<String> {
    task_lines(&fs::read_to_string(path)?)
        .into_iter()
        .next()
        .map(|(_, line)| line)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "task file was empty"))
}

fn find_task_line(content: &str, line_index: usize) -> io::Result<String> {
    task_lines(content)
        .into_iter()
        .find(|(task_line_index, _)| *task_line_index == line_index)
        .map(|(_, line)| line)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "task line index does not exist in source file",
            )
        })
}

fn normalized_path(source: &Path, line_index: usize) -> io::Result<PathBuf> {
    if line_index == 0 {
        return Ok(source.to_path_buf());
    }

    let parent = source.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "task file did not have a parent directory",
        )
    })?;

    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "task file had invalid UTF-8")
        })?;

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("txt");
    Ok(parent.join(format!("{stem}-line-{line_index}.{extension}")))
}

fn file_name(path: &Path) -> io::Result<OsString> {
    path.file_name().map(OsString::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "task file did not have a file name",
        )
    })
}

fn edited_task_destination(store: &TaskStore, path: &Path, is_done: bool) -> io::Result<PathBuf> {
    let file_name = file_name(path)?;
    let done_dir = store.done_dir();
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "task file did not have a parent directory",
        )
    })?;

    if is_done {
        if parent == done_dir {
            Ok(path.to_path_buf())
        } else {
            Ok(done_dir.join(file_name))
        }
    } else if parent == done_dir {
        Ok(store.root.join(file_name))
    } else {
        Ok(path.to_path_buf())
    }
}

fn with_trailing_newline(raw: &str) -> String {
    format!("{}\n", raw.trim_end_matches(['\r', '\n']))
}

fn format_done_line(raw: &str, completion_date: &str) -> io::Result<String> {
    if !is_date_token(completion_date) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "completion date must use YYYY-MM-DD format",
        ));
    }

    let task = parse_task_line(raw);
    if task.done {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot mark an already completed task as done",
        ));
    }

    let description_raw = if task.priority.is_some() {
        // Strip "(X) " prefix from raw
        let after_priority = if raw.len() > 4 && raw.starts_with('(') {
            &raw[4..]
        } else {
            &raw[3..]
        };
        after_priority.to_string()
    } else {
        raw.to_string()
    };

    if let Some(priority) = task.priority {
        Ok(format!(
            "x {completion_date} {description_raw} pri:{priority}"
        ))
    } else {
        Ok(format!("x {completion_date} {description_raw}"))
    }
}

fn format_restored_line(raw: &str) -> io::Result<String> {
    let task = parse_task_line(raw);
    if !task.done {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot restore an open task",
        ));
    }

    let mut stripped = strip_completed_prefix(raw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "completed task line was missing completion metadata",
        )
    })?;

    // Restore priority from pri:X tag if present
    if let Some(pri_value) = task.tags.get("pri")
        && pri_value.len() == 1
        && pri_value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
    {
        stripped = stripped
            .split_whitespace()
            .filter(|token| !token.starts_with("pri:"))
            .collect::<Vec<_>>()
            .join(" ");
        stripped = format!("({}) {stripped}", pri_value);
    }

    Ok(stripped)
}

fn strip_completed_prefix(raw: &str) -> Option<String> {
    if !raw.starts_with("x ") {
        return None;
    }

    let remainder = &raw[2..];
    let (_date, tail) = remainder.split_once(' ')?;
    if !is_date_token(&remainder[..10.min(remainder.len())]) {
        return None;
    }

    Some(tail.to_owned())
}

fn is_date_token(token: &str) -> bool {
    is_date(token)
}
