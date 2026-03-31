use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-cli-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn add_command_writes_a_task_file() {
    let root = temp_path("add");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_ttd"))
        .arg("add")
        .arg("Call Mom +Family")
        .env("TTD_TASK_DIR", &root)
        .status()
        .unwrap();

    assert!(status.success());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
}

#[test]
fn search_command_prints_matching_lines() {
    let root = temp_path("search");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("b.txt"), "Buy milk\n").unwrap();
    fs::write(
        root.join("done.txt.d/completed.txt"),
        "x 2026-03-29 Call Mom from archive\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ttd"))
        .args(["search", "Mom"])
        .env("TTD_TASK_DIR", &root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Call Mom"));
    assert!(!stdout.contains("archive"));
}

#[test]
fn list_command_prints_open_tasks_only() {
    let root = temp_path("list");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("open.txt"), "Call Mom\n").unwrap();
    fs::write(root.join("done.txt.d/done.txt"), "x 2026-03-29 Buy milk\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ttd"))
        .arg("list")
        .env("TTD_TASK_DIR", &root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Call Mom"));
    assert!(!stdout.contains("Buy milk"));
}

#[test]
fn done_command_marks_task_done_by_file_name() {
    let root = temp_path("done");
    fs::create_dir_all(root.join("done.txt.d")).unwrap();
    fs::write(root.join("todo.txt"), "Call Mom\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ttd"))
        .args(["done", "todo.txt"])
        .env("TTD_TASK_DIR", &root)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!root.join("todo.txt").exists());
    let done_path = root.join("done.txt.d/todo.txt");
    assert!(done_path.exists());
    let done_contents = fs::read_to_string(done_path).unwrap();
    assert!(done_contents.starts_with("x "));
    assert!(done_contents.contains("Call Mom"));
}

#[test]
fn running_without_a_subcommand_enters_welcome_mode_when_config_is_missing() {
    let root = temp_path("welcome");
    let config_home = root.join("config-home");
    fs::create_dir_all(&config_home).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ttd"))
        .env("TTD_TUI_RENDER_ONCE", "1")
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Welcome to ttd"));
    assert!(stdout.contains("todo.txt.d"));
    assert!(!stdout.contains("welcome mode"));
}

#[test]
fn running_without_a_subcommand_enters_main_mode_when_config_exists() {
    let root = temp_path("main");
    let config_home = root.join("config-home");
    let task_dir = root.join("todo.txt.d");
    fs::create_dir_all(task_dir.join("done.txt.d")).unwrap();
    fs::create_dir_all(config_home.join("ttd")).unwrap();
    fs::write(
        config_home.join("ttd/config.txt"),
        format!("{}\n", task_dir.display()),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ttd"))
        .env("TTD_TUI_RENDER_ONCE", "1")
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", &config_home)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Inbox"));
    assert!(!stdout.contains("main mode"));
}
