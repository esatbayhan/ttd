use std::collections::{BTreeMap, HashSet};

use crate::task::Task;

pub fn parse_task_line(raw: &str) -> Task {
    let mut done = false;
    let mut completion_date = None;
    let mut priority = None;
    let mut creation_date = None;
    let mut description_start = 0;

    if let Some((date, next)) = consume_completed_prefix(raw) {
        done = true;
        completion_date = Some(date.to_owned());
        description_start = next;

        if let Some((date, next)) = consume_date(raw, description_start) {
            creation_date = Some(date.to_owned());
            description_start = next;
        }
    } else {
        if let Some((value, next)) = consume_priority(raw) {
            priority = Some(value);
            description_start = next;
        }

        if let Some((date, next)) = consume_date(raw, description_start) {
            creation_date = Some(date.to_owned());
            description_start = next;
        }
    }

    let description = raw[description_start..].to_owned();
    let (projects, contexts, tags) = extract_description_metadata(&description);

    Task {
        done,
        completion_date,
        priority,
        creation_date,
        description,
        projects,
        contexts,
        tags,
        raw: raw.to_owned(),
    }
}

pub fn format_task(task: &Task) -> String {
    let mut parts = Vec::new();

    if task.done {
        parts.push("x".to_owned());
        if let Some(completion_date) = &task.completion_date {
            parts.push(completion_date.clone());
        }
    } else if let Some(priority) = task.priority {
        parts.push(format!("({priority})"));
    }

    if let Some(creation_date) = &task.creation_date {
        parts.push(creation_date.clone());
    }

    if task.description.is_empty() {
        return parts.join(" ");
    }

    if parts.is_empty() {
        return task.description.clone();
    }

    format!("{} {}", parts.join(" "), task.description)
}

fn consume_completed_prefix(raw: &str) -> Option<(&str, usize)> {
    if !raw.starts_with("x ") {
        return None;
    }

    consume_date(raw, 2)
}

fn consume_priority(raw: &str) -> Option<(char, usize)> {
    let bytes = raw.as_bytes();

    if bytes.len() < 3 || bytes[0] != b'(' || bytes[2] != b')' || !bytes[1].is_ascii_uppercase() {
        return None;
    }

    if bytes.len() == 3 {
        return Some((bytes[1] as char, 3));
    }

    if bytes[3] == b' ' {
        return Some((bytes[1] as char, 4));
    }

    None
}

fn consume_date(raw: &str, start: usize) -> Option<(&str, usize)> {
    let candidate = raw.get(start..start + 10)?;
    if !is_date(candidate) {
        return None;
    }

    if raw.len() == start + 10 {
        return Some((candidate, start + 10));
    }

    if raw.as_bytes().get(start + 10) == Some(&b' ') {
        return Some((candidate, start + 11));
    }

    None
}

fn extract_description_metadata(
    description: &str,
) -> (Vec<String>, Vec<String>, BTreeMap<String, String>) {
    let mut projects = Vec::new();
    let mut contexts = Vec::new();
    let mut tags = BTreeMap::new();
    let mut seen_keys = HashSet::new();

    let mut index = 0;
    while index < description.len() {
        let at_boundary = index == 0 || description.as_bytes()[index - 1].is_ascii_whitespace();
        if !at_boundary {
            index += 1;
            continue;
        }

        let end = find_token_end(description, index);
        let token = &description[index..end];

        if let Some(project) = token.strip_prefix('+') {
            if !project.is_empty() {
                projects.push(project.to_owned());
            }
        } else if let Some(context) = token.strip_prefix('@') {
            if !context.is_empty() {
                contexts.push(context.to_owned());
            }
        } else if let Some((key, value)) = split_tag_token(token) {
            let first_occurrence = seen_keys.insert(key.to_owned());
            if first_occurrence && tag_value_is_valid(key, value) {
                tags.insert(key.to_owned(), value.to_owned());
            }
        }

        index = end + 1;
    }

    (projects, contexts, tags)
}

fn find_token_end(description: &str, start: usize) -> usize {
    let remainder = &description[start..];
    let end = remainder
        .find(|value: char| value.is_ascii_whitespace())
        .unwrap_or(remainder.len());

    start + end
}

fn split_tag_token(token: &str) -> Option<(&str, &str)> {
    let (key, value) = token.split_once(':')?;
    if key.is_empty() || value.is_empty() || key.contains(':') || value.contains(':') {
        return None;
    }

    Some((key, value))
}

fn tag_value_is_valid(key: &str, value: &str) -> bool {
    if matches!(key, "due" | "scheduled" | "starting") {
        return is_date(value);
    }

    true
}

pub fn is_date(token: &str) -> bool {
    token.len() == 10
        && token.chars().enumerate().all(|(index, value)| match index {
            4 | 7 => value == '-',
            _ => value.is_ascii_digit(),
        })
}
