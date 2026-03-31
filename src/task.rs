use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub done: bool,
    pub completion_date: Option<String>,
    pub priority: Option<char>,
    pub creation_date: Option<String>,
    pub description: String,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
    pub tags: BTreeMap<String, String>,
    pub raw: String,
}
