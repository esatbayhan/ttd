use std::fs;
use std::path::PathBuf;

use ttd::store::{TaskId, TaskStore};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-store-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn loading_reads_open_and_done_directories() {
    let root = temp_path("load");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("open.txt"), "(A) Call Mom\n").unwrap();
    fs::write(
        root.join("done.txt.d/closed.txt"),
        "x 2024-03-01 Filed taxes\n",
    )
    .unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    assert_eq!(snapshot.open_tasks.len(), 1);
    assert_eq!(snapshot.done_tasks.len(), 1);
    assert_eq!(snapshot.open_tasks[0].task.priority, Some('A'));
    assert!(snapshot.done_tasks[0].task.done);
}

#[test]
fn updating_multiline_source_normalizes_to_single_task_file() {
    let root = temp_path("normalize");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("batch.txt"), "Call Mom\nBuy milk\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();
    let task = snapshot.open_tasks[0].clone();

    store.update_task(&task.id, "(A) Call Mom +Family").unwrap();

    let rewritten = fs::read_to_string(root.join(task.id.file_name())).unwrap();
    assert_eq!(rewritten, "(A) Call Mom +Family\n");

    let sibling = fs::read_to_string(root.join("batch-line-1.txt")).unwrap();
    assert_eq!(sibling, "Buy milk\n");
}

#[test]
fn normalizing_file_with_leading_blank_line_removes_original_source_file() {
    let root = temp_path("normalize-leading-blank");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("batch.txt"), "\nCall Mom\nBuy milk\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    store
        .update_task(&snapshot.open_tasks[0].id, "(A) Call Mom +Family")
        .unwrap();

    assert!(!root.join("batch.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("batch-line-1.txt")).unwrap(),
        "(A) Call Mom +Family\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("batch-line-2.txt")).unwrap(),
        "Buy milk\n"
    );
}

#[test]
fn marking_done_moves_file_into_done_directory() {
    let root = temp_path("done");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    store
        .mark_done(&snapshot.open_tasks[0].id, "2026-03-29")
        .unwrap();

    assert!(!root.join("todo.txt").exists());
    assert!(root.join("done.txt.d/todo.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("done.txt.d/todo.txt")).unwrap(),
        "x 2026-03-29 Call Mom\n"
    );
}

#[test]
fn mark_done_and_restore_preserve_priority_metadata() {
    let root = temp_path("priority-roundtrip");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "(A) Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    store
        .mark_done(&snapshot.open_tasks[0].id, "2026-03-29")
        .unwrap();

    assert_eq!(
        fs::read_to_string(root.join("done.txt.d/todo.txt")).unwrap(),
        "x 2026-03-29 Call Mom pri:A\n"
    );

    let done_snapshot = store.load_all().unwrap();
    store.restore_task(&done_snapshot.done_tasks[0].id).unwrap();

    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "(A) Call Mom\n"
    );
}

#[test]
fn marking_done_rejects_done_task_ids() {
    let root = temp_path("done-id");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("done.txt.d/todo.txt"), "x 2026-03-29 Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store
        .mark_done(&snapshot.done_tasks[0].id, "2026-03-30")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("done.txt.d/todo.txt")).unwrap(),
        "x 2026-03-29 Call Mom\n"
    );
}

#[test]
fn marking_done_refuses_to_overwrite_existing_done_file() {
    let root = temp_path("done-overwrite");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();
    fs::write(
        root.join("done.txt.d/todo.txt"),
        "x 2026-03-28 Existing done copy\n",
    )
    .unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store
        .mark_done(&snapshot.open_tasks[0].id, "2026-03-29")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Call Mom\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("done.txt.d/todo.txt")).unwrap(),
        "x 2026-03-28 Existing done copy\n"
    );
}

#[test]
fn marking_done_rejects_invalid_completion_date() {
    let root = temp_path("done-invalid-date");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store
        .mark_done(&snapshot.open_tasks[0].id, "not-a-date")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Call Mom\n"
    );
    assert!(!root.join("done.txt.d/todo.txt").exists());
}

#[test]
fn restoring_moves_task_back_to_open_directory() {
    let root = temp_path("restore");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("done.txt.d/todo.txt"), "x 2026-03-29 Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    store.restore_task(&snapshot.done_tasks[0].id).unwrap();

    assert!(root.join("todo.txt").exists());
    assert!(!root.join("done.txt.d/todo.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Call Mom\n"
    );
}

#[test]
fn restoring_rejects_open_task_ids() {
    let root = temp_path("open-id");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store.restore_task(&snapshot.open_tasks[0].id).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Call Mom\n"
    );
}

#[test]
fn restoring_refuses_to_overwrite_existing_open_file() {
    let root = temp_path("restore-overwrite");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Existing open copy\n").unwrap();
    fs::write(root.join("done.txt.d/todo.txt"), "x 2026-03-29 Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store.restore_task(&snapshot.done_tasks[0].id).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Existing open copy\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("done.txt.d/todo.txt")).unwrap(),
        "x 2026-03-29 Call Mom\n"
    );
}

#[test]
fn deleting_task_removes_only_target_file() {
    let root = temp_path("delete");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("first.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("second.txt"), "Buy milk\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    store.delete_task(&snapshot.open_tasks[0].id).unwrap();

    assert!(!root.join("first.txt").exists());
    assert!(root.join("second.txt").exists());
}

#[test]
fn invalid_task_id_does_not_partially_normalize_multiline_file() {
    let root = temp_path("invalid-id");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("batch.txt"), "Call Mom\nBuy milk\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let invalid_id = TaskId {
        path: root.join("batch.txt"),
        line_index: 99,
    };

    let error = store.delete_task(&invalid_id).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        fs::read_to_string(root.join("batch.txt")).unwrap(),
        "Call Mom\nBuy milk\n"
    );
    assert!(!root.join("batch-line-1.txt").exists());
}

#[test]
fn deleting_task_rejects_ids_from_other_store_roots() {
    let root = temp_path("delete-foreign-root");
    let foreign_root = temp_path("delete-foreign-other");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::create_dir_all(foreign_root.join("done.txt.d")).unwrap();
    fs::write(foreign_root.join("todo.txt"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let foreign_store = TaskStore::open(foreign_root.clone()).unwrap();
    let snapshot = foreign_store.load_all().unwrap();

    let error = store.delete_task(&snapshot.open_tasks[0].id).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(foreign_root.join("todo.txt").exists());
}

#[test]
fn deleting_task_rejects_non_txt_ids() {
    let root = temp_path("delete-non-txt");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("notes.md"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let invalid_id = TaskId {
        path: root.join("notes.md"),
        line_index: 0,
    };

    let error = store.delete_task(&invalid_id).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("notes.md")).unwrap(),
        "Call Mom\n"
    );
}

#[test]
fn normalization_failure_leaves_original_multitask_file_unchanged() {
    let root = temp_path("normalize-atomicity");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("batch.txt"), "Call Mom\n\nBuy milk\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();
    fs::create_dir(root.join("batch-line-2.txt")).unwrap();

    let error = store
        .update_task(&snapshot.open_tasks[0].id, "(A) Call Mom +Family")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::IsADirectory);
    assert_eq!(
        fs::read_to_string(root.join("batch.txt")).unwrap(),
        "Call Mom\n\nBuy milk\n"
    );
    assert!(root.join("batch-line-2.txt").is_dir());
}

#[test]
fn update_task_rejects_multiline_input() {
    let root = temp_path("update-multiline-input");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store
        .update_task(&snapshot.open_tasks[0].id, "Call Mom\nBuy milk")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Call Mom\n"
    );
}

#[test]
fn update_task_rejects_blank_input() {
    let root = temp_path("update-blank-input");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();

    let store = TaskStore::open(root.clone()).unwrap();
    let snapshot = store.load_all().unwrap();

    let error = store
        .update_task(&snapshot.open_tasks[0].id, "   ")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("todo.txt")).unwrap(),
        "Call Mom\n"
    );
}

#[test]
fn snapshot_index_covers_root_and_done_directories() {
    let root = temp_path("snapshot-index");
    std::fs::create_dir_all(root.join("done.txt.d")).unwrap();
    std::fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    std::fs::write(root.join("done.txt.d/b.txt"), "x 2026-03-29 Ship pkg\n").unwrap();

    let store = ttd::store::TaskStore::open(root.clone()).unwrap();
    let index = store.snapshot_index().unwrap();

    assert!(index.has_path(&root.join("a.txt")));
    assert!(index.has_path(&root.join("done.txt.d/b.txt")));
}
