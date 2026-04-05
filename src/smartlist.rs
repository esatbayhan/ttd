use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateField {
    Due,
    Scheduled,
    Starting,
    CreationDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextField {
    Project,
    Context,
    Description,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    Due,
    Scheduled,
    Starting,
    CreationDate,
    Priority,
    Project,
    Context,
    Description,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriorityOp {
    Eq,
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOp {
    Includes,
    Excludes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    DateComparison {
        field: DateField,
        op: CompareOp,
        offset: i32,
    },
    PriorityComparison {
        op: PriorityOp,
        letter: char,
    },
    TextMatch {
        field: TextField,
        op: TextOp,
        text: String,
    },
    Existence {
        field: Field,
        present: bool,
    },
    DoneFilter {
        done: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterBlock {
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    pub field: Field,
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartList {
    pub name: String,
    pub icon: Option<String>,
    pub order: Option<i32>,
    pub source_path: PathBuf,
    pub parse_error: Option<String>,
    pub blocks: Vec<FilterBlock>,
    pub sort_directives: Vec<Directive>,
    pub group_directives: Vec<Directive>,
}

fn parse_date_offset(value: &str) -> Option<i32> {
    let value = value.trim();
    if value == "today" {
        return Some(0);
    }
    // Strip "today" prefix then parse optional offset
    let rest = value.strip_prefix("today")?;
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(0);
    }
    if let Some(rest) = rest.strip_prefix('+') {
        let num: i32 = rest.trim().parse().ok()?;
        Some(num)
    } else if let Some(rest) = rest.strip_prefix('-') {
        let num: i32 = rest.trim().parse().ok()?;
        Some(-num)
    } else {
        None
    }
}

fn parse_date_field(s: &str) -> Option<DateField> {
    match s {
        "due" => Some(DateField::Due),
        "scheduled" => Some(DateField::Scheduled),
        "starting" => Some(DateField::Starting),
        "creation_date" => Some(DateField::CreationDate),
        _ => None,
    }
}

fn parse_field(s: &str) -> Option<Field> {
    match s {
        "due" => Some(Field::Due),
        "scheduled" => Some(Field::Scheduled),
        "starting" => Some(Field::Starting),
        "creation_date" => Some(Field::CreationDate),
        "priority" => Some(Field::Priority),
        "project" => Some(Field::Project),
        "context" => Some(Field::Context),
        "description" => Some(Field::Description),
        "done" => Some(Field::Done),
        _ => None,
    }
}

fn parse_text_field(s: &str) -> Option<TextField> {
    match s {
        "project" => Some(TextField::Project),
        "context" => Some(TextField::Context),
        "description" => Some(TextField::Description),
        _ => None,
    }
}

fn parse_compare_op(s: &str) -> Option<CompareOp> {
    match s {
        "=" => Some(CompareOp::Eq),
        "<" => Some(CompareOp::Lt),
        "<=" => Some(CompareOp::Lte),
        ">" => Some(CompareOp::Gt),
        ">=" => Some(CompareOp::Gte),
        _ => None,
    }
}

fn parse_condition(line: &str) -> Option<Condition> {
    let line = line.trim();

    // done / not done — check before "no" prefix
    if line == "done" {
        return Some(Condition::DoneFilter { done: true });
    }
    if line == "not done" {
        return Some(Condition::DoneFilter { done: false });
    }

    // existence: "has <field>" or "no <field>"
    if let Some(rest) = line.strip_prefix("has ") {
        if let Some(field) = parse_field(rest.trim()) {
            return Some(Condition::Existence {
                field,
                present: true,
            });
        }
        return None;
    }
    if let Some(rest) = line.strip_prefix("no ") {
        if let Some(field) = parse_field(rest.trim()) {
            return Some(Condition::Existence {
                field,
                present: false,
            });
        }
        return None;
    }

    // comparison: field op value
    // Split into at most 3 parts: field, operator, value
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 3 {
        return None;
    }
    let field_str = parts[0];
    let op_str = parts[1];
    let value_str = parts[2];

    // date comparison
    if let (Some(date_field), Some(comp_op), Some(offset)) = (
        parse_date_field(field_str),
        parse_compare_op(op_str),
        parse_date_offset(value_str),
    ) {
        return Some(Condition::DateComparison {
            field: date_field,
            op: comp_op,
            offset,
        });
    }

    // priority comparison
    if field_str == "priority" {
        let priority_op = match op_str {
            "=" => Some(PriorityOp::Eq),
            "above" => Some(PriorityOp::Above),
            "below" => Some(PriorityOp::Below),
            _ => None,
        };
        let letter_str = value_str.trim();
        let first_char = if letter_str.len() == 1 {
            letter_str.chars().next()
        } else {
            None
        };
        if let (Some(pop), Some(c)) = (priority_op, first_char)
            && c.is_ascii_uppercase()
        {
            return Some(Condition::PriorityComparison {
                op: pop,
                letter: c,
            });
        }
    }

    // text comparison
    let text_op = match op_str {
        "includes" => Some(TextOp::Includes),
        "excludes" => Some(TextOp::Excludes),
        _ => None,
    };
    if let (Some(text_field), Some(top)) = (parse_text_field(field_str), text_op) {
        return Some(Condition::TextMatch {
            field: text_field,
            op: top,
            text: value_str.to_string(),
        });
    }

    None
}

fn parse_directive(line: &str) -> Option<(bool, Directive)> {
    // Returns (is_sort, Directive) or None
    let line = line.trim();
    let (is_sort, rest) = if let Some(r) = line.strip_prefix("sort by ") {
        (true, r)
    } else if let Some(r) = line.strip_prefix("group by ") {
        (false, r)
    } else {
        return None;
    };

    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    let field_str = parts[0];
    let direction = if parts.len() == 2 {
        match parts[1].trim() {
            "asc" => Direction::Asc,
            "desc" => Direction::Desc,
            _ => Direction::Asc,
        }
    } else {
        Direction::Asc
    };

    let field = parse_field(field_str)?;
    Some((is_sort, Directive { field, direction }))
}

pub fn parse_list(content: &str, source_path: &Path) -> SmartList {
    // Normalize line endings
    let content = content.replace("\r\n", "\n");

    // Default name from filename stem
    let default_name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    // Split into lines and find frontmatter delimiters
    let lines: Vec<&str> = content.lines().collect();

    // Find first and second "---" lines
    let first_dash = lines.iter().position(|l| l.trim() == "---");
    let second_dash = first_dash.and_then(|start| {
        lines[start + 1..]
            .iter()
            .position(|l| l.trim() == "---")
            .map(|rel| start + 1 + rel)
    });

    let (parse_error, name, icon, order, body_lines) =
        match (first_dash, second_dash) {
            (Some(start), Some(end)) => {
                let frontmatter_lines = &lines[start + 1..end];
                let body_lines = &lines[end + 1..];

                let mut name: Option<String> = None;
                let mut icon: Option<String> = None;
                let mut order: Option<i32> = None;

                for fm_line in frontmatter_lines {
                    if let Some(colon_pos) = fm_line.find(':') {
                        let key = fm_line[..colon_pos].trim();
                        let value = fm_line[colon_pos + 1..].trim();
                        match key {
                            "name" => name = Some(value.to_string()),
                            "icon" => icon = Some(value.to_string()),
                            "order" => order = value.parse().ok(),
                            _ => {} // unknown keys ignored
                        }
                    }
                }

                let resolved_name = name.unwrap_or_else(|| default_name.clone());
                (None, resolved_name, icon, order, body_lines.to_vec())
            }
            _ => {
                // No valid frontmatter delimiters
                let err = "missing frontmatter delimiters".to_string();
                (Some(err), default_name, None, None, lines.as_slice().to_vec())
            }
        };

    // Split body by "OR" lines
    let mut raw_blocks: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in &body_lines {
        if line.trim() == "OR" {
            raw_blocks.push(current);
            current = Vec::new();
        } else {
            current.push(line);
        }
    }
    raw_blocks.push(current);

    let mut blocks: Vec<FilterBlock> = Vec::new();
    let mut sort_directives: Vec<Directive> = Vec::new();
    let mut group_directives: Vec<Directive> = Vec::new();

    for raw_block in raw_blocks {
        let mut conditions: Vec<Condition> = Vec::new();

        for line in raw_block {
            let trimmed = line.trim();
            // Skip blank lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Try directive first
            if trimmed.starts_with("sort by ") || trimmed.starts_with("group by ") {
                if let Some((is_sort, directive)) = parse_directive(trimmed) {
                    if is_sort {
                        sort_directives.push(directive);
                    } else {
                        group_directives.push(directive);
                    }
                }
                continue;
            }

            // Try condition
            if let Some(condition) = parse_condition(trimmed) {
                conditions.push(condition);
            }
            // Unrecognized lines silently skipped
        }

        // Only add a block if it has conditions (empty blocks from trailing OR etc are skipped)
        if !conditions.is_empty() {
            blocks.push(FilterBlock { conditions });
        }
    }

    SmartList {
        name,
        icon,
        order,
        source_path: source_path.to_path_buf(),
        parse_error,
        blocks,
        sort_directives,
        group_directives,
    }
}

// ── Date arithmetic ──────────────────────────────────────────────────────────

fn julian_day(year: i32, month: u32, day: u32) -> i32 {
    let a = (14 - month as i32) / 12;
    let y = year + 4800 - a;
    let m = month as i32 + 12 * a - 3;
    day as i32 + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045
}

fn from_julian_day(jdn: i32) -> (i32, u32, u32) {
    let a = jdn + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (146097 * b) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let day = (e - (153 * m + 2) / 5 + 1) as u32;
    let month = (m + 3 - 12 * (m / 10)) as u32;
    let year = 100 * b + d - 4800 + m / 10;
    (year, month, day)
}

pub fn add_days_to_date(date_str: &str, days: i32) -> String {
    let year: i32 = date_str[0..4].parse().unwrap_or(0);
    let month: u32 = date_str[5..7].parse().unwrap_or(1);
    let day: u32 = date_str[8..10].parse().unwrap_or(1);
    let jdn = julian_day(year, month, day) + days;
    let (y, mo, d) = from_julian_day(jdn);
    format!("{:04}-{:02}-{:02}", y, mo, d)
}

// ── Condition evaluation ──────────────────────────────────────────────────────

use crate::store::StoredTask;
use crate::task::Task;

fn get_date_field_value<'a>(task: &'a Task, field: &DateField) -> Option<&'a str> {
    match field {
        DateField::Due => task.tags.get("due").map(|s| s.as_str()),
        DateField::Scheduled => task.tags.get("scheduled").map(|s| s.as_str()),
        DateField::Starting => task.tags.get("starting").map(|s| s.as_str()),
        DateField::CreationDate => task.creation_date.as_deref(),
    }
}

fn eval_condition(cond: &Condition, task: &Task, today: &str) -> bool {
    match cond {
        Condition::DoneFilter { done } => task.done == *done,

        Condition::Existence { field, present } => {
            let has = match field {
                Field::Due => task.tags.contains_key("due"),
                Field::Scheduled => task.tags.contains_key("scheduled"),
                Field::Starting => task.tags.contains_key("starting"),
                Field::CreationDate => task.creation_date.is_some(),
                Field::Priority => task.priority.is_some(),
                Field::Project => !task.projects.is_empty(),
                Field::Context => !task.contexts.is_empty(),
                Field::Description => !task.description.is_empty(),
                Field::Done => task.done,
            };
            has == *present
        }

        Condition::DateComparison { field, op, offset } => {
            let task_date = match get_date_field_value(task, field) {
                Some(d) => d,
                None => return false,
            };
            let target = add_days_to_date(today, *offset);
            match op {
                CompareOp::Eq => task_date == target,
                CompareOp::Lt => task_date < target.as_str(),
                CompareOp::Lte => task_date <= target.as_str(),
                CompareOp::Gt => task_date > target.as_str(),
                CompareOp::Gte => task_date >= target.as_str(),
            }
        }

        Condition::PriorityComparison { op, letter } => {
            let task_priority = match task.priority {
                Some(p) => p,
                None => return false,
            };
            match op {
                PriorityOp::Eq => task_priority == *letter,
                // "above" means alphabetically earlier (A is above B)
                PriorityOp::Above => task_priority < *letter,
                // "below" means alphabetically later
                PriorityOp::Below => task_priority > *letter,
            }
        }

        Condition::TextMatch { field, op, text } => {
            let needle = text.to_lowercase();
            let matches = match field {
                TextField::Project => task
                    .projects
                    .iter()
                    .any(|p| p.to_lowercase().contains(&needle)),
                TextField::Context => task
                    .contexts
                    .iter()
                    .any(|c| c.to_lowercase().contains(&needle)),
                TextField::Description => task.description.to_lowercase().contains(&needle),
            };
            match op {
                TextOp::Includes => matches,
                TextOp::Excludes => !matches,
            }
        }
    }
}

fn has_done_filter(list: &SmartList) -> bool {
    list.blocks.iter().any(|block| {
        block
            .conditions
            .iter()
            .any(|c| matches!(c, Condition::DoneFilter { .. }))
    })
}

// ── Sort helpers ──────────────────────────────────────────────────────────────

fn task_sort_key(task: &Task, field: &Field) -> Option<String> {
    match field {
        Field::Due => task.tags.get("due").cloned(),
        Field::Scheduled => task.tags.get("scheduled").cloned(),
        Field::Starting => task.tags.get("starting").cloned(),
        Field::CreationDate => task.creation_date.clone(),
        Field::Priority => task.priority.map(|c| c.to_string()),
        Field::Project => task.projects.first().cloned(),
        Field::Context => task.contexts.first().cloned(),
        Field::Description => Some(task.description.clone()),
        Field::Done => Some(if task.done { "1" } else { "0" }.to_string()),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn evaluate(list: &SmartList, tasks: &[StoredTask], today: &str) -> Vec<StoredTask> {
    if list.blocks.is_empty() {
        return Vec::new();
    }

    let implied_not_done = !has_done_filter(list);

    let mut matched: Vec<StoredTask> = tasks
        .iter()
        .filter(|st| {
            // Apply implied "not done" pre-filter
            if implied_not_done && st.task.done {
                return false;
            }
            // DNF: match if ANY block has ALL conditions true
            list.blocks.iter().any(|block| {
                block
                    .conditions
                    .iter()
                    .all(|cond| eval_condition(cond, &st.task, today))
            })
        })
        .cloned()
        .collect();

    // Apply sort directives (first directive = highest precedence)
    matched.sort_by(|a, b| {
        for directive in &list.sort_directives {
            let ka = task_sort_key(&a.task, &directive.field);
            let kb = task_sort_key(&b.task, &directive.field);

            // Items with values sort before items without
            let ord = match (&ka, &kb) {
                (Some(va), Some(vb)) => va.cmp(vb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            let ord = if directive.direction == Direction::Desc {
                ord.reverse()
            } else {
                ord
            };

            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });

    matched
}

pub fn filter_only(list: &SmartList, tasks: &[StoredTask], today: &str) -> Vec<StoredTask> {
    if list.blocks.is_empty() {
        return Vec::new();
    }

    let implied_not_done = !has_done_filter(list);

    tasks
        .iter()
        .filter(|st| {
            if implied_not_done && st.task.done {
                return false;
            }
            list.blocks.iter().any(|block| {
                block
                    .conditions
                    .iter()
                    .all(|cond| eval_condition(cond, &st.task, today))
            })
        })
        .cloned()
        .collect()
}

pub fn sort_by_directives(tasks: &mut [StoredTask], directives: &[Directive]) {
    tasks.sort_by(|a, b| {
        for directive in directives {
            let ka = task_sort_key(&a.task, &directive.field);
            let kb = task_sort_key(&b.task, &directive.field);

            let ord = match (&ka, &kb) {
                (Some(va), Some(vb)) => va.cmp(vb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            let ord = if directive.direction == Direction::Desc {
                ord.reverse()
            } else {
                ord
            };

            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

pub fn group_by_directives(directives: &[Directive], tasks: &[StoredTask]) -> Vec<TaskGroup> {
    let directive = match directives.first() {
        Some(d) => d,
        None => {
            return vec![TaskGroup {
                label: String::new(),
                tasks: tasks.to_vec(),
            }];
        }
    };

    let mut labeled: Vec<(String, StoredTask)> = Vec::new();
    let mut no_value: Vec<StoredTask> = Vec::new();
    let fallback_label = format!("No {}", field_display_name(&directive.field));

    for st in tasks {
        match task_group_key(&st.task, &directive.field) {
            Some(key) => labeled.push((key, st.clone())),
            None => no_value.push(st.clone()),
        }
    }

    let mut group_map: std::collections::BTreeMap<String, Vec<StoredTask>> =
        std::collections::BTreeMap::new();
    for (key, st) in labeled {
        group_map.entry(key).or_default().push(st);
    }

    let field_prefix = capitalize(field_display_name(&directive.field));
    let mut groups: Vec<TaskGroup> = group_map
        .into_iter()
        .map(|(value, tasks)| TaskGroup {
            label: format!("{field_prefix}: {value}"),
            tasks,
        })
        .collect();

    groups.sort_by(|a, b| match directive.direction {
        Direction::Asc => a.label.cmp(&b.label),
        Direction::Desc => b.label.cmp(&a.label),
    });

    if !no_value.is_empty() {
        groups.push(TaskGroup {
            label: fallback_label,
            tasks: no_value,
        });
    }

    groups
}

// ── Grouping ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TaskGroup {
    pub label: String,
    pub tasks: Vec<StoredTask>,
}

fn task_group_key(task: &Task, field: &Field) -> Option<String> {
    match field {
        Field::Due => task.tags.get("due").cloned(),
        Field::Scheduled => task.tags.get("scheduled").cloned(),
        Field::Starting => task.tags.get("starting").cloned(),
        Field::CreationDate => task.creation_date.clone(),
        Field::Priority => task.priority.map(|c| c.to_string()),
        Field::Project => task.projects.first().cloned(),
        Field::Context => task.contexts.first().cloned(),
        Field::Description => Some(task.description.clone()),
        Field::Done => Some(if task.done { "Done" } else { "Not done" }.to_string()),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

pub fn field_display_name(field: &Field) -> &'static str {
    match field {
        Field::Due => "due",
        Field::Scheduled => "scheduled",
        Field::Starting => "starting",
        Field::CreationDate => "creation date",
        Field::Priority => "priority",
        Field::Project => "project",
        Field::Context => "context",
        Field::Description => "description",
        Field::Done => "done",
    }
}

pub fn group(list: &SmartList, tasks: &[StoredTask]) -> Vec<TaskGroup> {
    group_by_directives(&list.group_directives, tasks)
}

pub fn load_all(lists_dir: &Path) -> Vec<SmartList> {
    let entries = match fs::read_dir(lists_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut lists: Vec<SmartList> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("list") {
                return None;
            }
            let content = fs::read_to_string(&path).ok()?;
            Some(parse_list(&content, &path))
        })
        .collect();

    lists.sort_by(|a, b| {
        match (a.order, b.order) {
            (Some(ao), Some(bo)) => ao.cmp(&bo).then_with(|| a.name.cmp(&b.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });

    lists
}
