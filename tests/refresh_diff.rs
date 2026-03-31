use std::collections::BTreeSet;
use std::path::PathBuf;

use ttd::refresh::{RefreshAction, SnapshotIndex};

#[test]
fn diff_detects_created_updated_and_deleted_files() {
    let before = SnapshotIndex::from_entries([
        (PathBuf::from("a.txt"), 10_u64, 100_u64),
        (PathBuf::from("b.txt"), 10_u64, 100_u64),
    ]);
    let after = SnapshotIndex::from_entries([
        (PathBuf::from("b.txt"), 12_u64, 120_u64),
        (PathBuf::from("c.txt"), 5_u64, 50_u64),
    ]);

    let actions: BTreeSet<RefreshAction> = before.diff(&after).into_iter().collect();

    assert!(actions.contains(&RefreshAction::Deleted(PathBuf::from("a.txt"))));
    assert!(actions.contains(&RefreshAction::Updated(PathBuf::from("b.txt"))));
    assert!(actions.contains(&RefreshAction::Created(PathBuf::from("c.txt"))));
}

#[cfg(unix)]
#[test]
fn diff_preserves_non_utf8_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let non_utf8 = PathBuf::from(OsString::from_vec(vec![0x66, 0x6f, 0x80, 0x6f]));
    let before = SnapshotIndex::from_entries([(non_utf8.clone(), 1_u64, 1_u64)]);
    let after = SnapshotIndex::from_entries([(non_utf8.clone(), 2_u64, 2_u64)]);

    let actions = before.diff(&after);

    assert_eq!(actions, vec![RefreshAction::Updated(non_utf8)]);
}

#[test]
fn scan_builds_index_from_directory_contents() {
    let dir = std::env::temp_dir().join(format!("ttd-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("a.txt"), "Call Mom\n").unwrap();
    std::fs::write(dir.join("b.txt"), "Ship package\n").unwrap();
    std::fs::write(dir.join("not-a-task.md"), "notes\n").unwrap();

    let index = SnapshotIndex::scan(&dir).unwrap();

    assert!(index.has_path(&dir.join("a.txt")));
    assert!(index.has_path(&dir.join("b.txt")));
    assert!(!index.has_path(&dir.join("not-a-task.md")));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn has_changes_returns_false_for_identical_indices() {
    let a = SnapshotIndex::from_entries([
        (PathBuf::from("a.txt"), 10_u64, 100_u64),
        (PathBuf::from("b.txt"), 20_u64, 200_u64),
    ]);
    let b = SnapshotIndex::from_entries([
        (PathBuf::from("a.txt"), 10_u64, 100_u64),
        (PathBuf::from("b.txt"), 20_u64, 200_u64),
    ]);

    assert!(!a.has_changes(&b));
}

#[test]
fn has_changes_returns_true_after_file_mutation() {
    let before = SnapshotIndex::from_entries([(PathBuf::from("a.txt"), 10_u64, 100_u64)]);
    let after = SnapshotIndex::from_entries([(PathBuf::from("a.txt"), 12_u64, 110_u64)]);

    assert!(before.has_changes(&after));
}

#[test]
fn has_changes_detects_new_file() {
    let before = SnapshotIndex::from_entries([(PathBuf::from("a.txt"), 10_u64, 100_u64)]);
    let after = SnapshotIndex::from_entries([
        (PathBuf::from("a.txt"), 10_u64, 100_u64),
        (PathBuf::from("b.txt"), 5_u64, 50_u64),
    ]);

    assert!(before.has_changes(&after));
}

#[test]
fn has_changes_detects_deleted_file() {
    let before = SnapshotIndex::from_entries([
        (PathBuf::from("a.txt"), 10_u64, 100_u64),
        (PathBuf::from("b.txt"), 5_u64, 50_u64),
    ]);
    let after = SnapshotIndex::from_entries([(PathBuf::from("a.txt"), 10_u64, 100_u64)]);

    assert!(before.has_changes(&after));
}

#[test]
fn merge_combines_two_indices() {
    let a = SnapshotIndex::from_entries([(PathBuf::from("root/a.txt"), 10_u64, 100_u64)]);
    let b = SnapshotIndex::from_entries([(PathBuf::from("done/b.txt"), 5_u64, 50_u64)]);

    let merged = a.merge(&b);

    assert!(merged.has_path(&PathBuf::from("root/a.txt")));
    assert!(merged.has_path(&PathBuf::from("done/b.txt")));
}
