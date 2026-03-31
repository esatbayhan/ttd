use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefreshAction {
    Created(PathBuf),
    Updated(PathBuf),
    Deleted(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotIndex(BTreeMap<PathBuf, (u64, u64)>);

impl SnapshotIndex {
    pub fn from_entries<P, const N: usize>(entries: [(P, u64, u64); N]) -> Self
    where
        P: AsRef<Path>,
    {
        let mut index = BTreeMap::new();
        for (path, len, stamp) in entries {
            index.insert(path.as_ref().to_path_buf(), (len, stamp));
        }
        Self(index)
    }

    pub fn scan(dir: &Path) -> io::Result<Self> {
        let mut index = BTreeMap::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("txt") {
                continue;
            }
            let meta = entry.metadata()?;
            let len = meta.len();
            let mtime = meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            index.insert(path, (len, mtime));
        }
        Ok(Self(index))
    }

    pub fn has_path(&self, path: &Path) -> bool {
        self.0.contains_key(path)
    }

    pub fn has_changes(&self, other: &Self) -> bool {
        self.0 != other.0
    }

    pub fn merge(&self, other: &Self) -> Self {
        let mut combined = self.0.clone();
        combined.extend(other.0.iter().map(|(k, v)| (k.clone(), *v)));
        Self(combined)
    }

    pub fn diff(&self, other: &Self) -> Vec<RefreshAction> {
        let mut actions = BTreeSet::new();

        for path in self.0.keys() {
            if !other.0.contains_key(path) {
                actions.insert(RefreshAction::Deleted(path.clone()));
            }
        }

        for (path, meta) in &other.0 {
            match self.0.get(path) {
                None => {
                    actions.insert(RefreshAction::Created(path.clone()));
                }
                Some(current) if current != meta => {
                    actions.insert(RefreshAction::Updated(path.clone()));
                }
                _ => {}
            }
        }

        actions.into_iter().collect()
    }
}
