use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use expectrl::{ControlCode, Expect, Regex, spawn};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-e2e-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn first_run_welcome_flow_persists_config_and_shows_real_task_view() {
    let root = temp_path("welcome");
    let config_home = root.join("config-home");
    let task_dir = root.join("todo.txt.d");
    fs::create_dir_all(&config_home).unwrap();
    fs::create_dir_all(task_dir.join("done.txt.d")).unwrap();
    fs::write(task_dir.join("a.txt"), "Call Mom +Family\n").unwrap();

    let mut process = spawn(format!(
        "env HOME={} XDG_CONFIG_HOME={} {}",
        root.join("home").display(),
        config_home.display(),
        env!("CARGO_BIN_EXE_ttd")
    ))
    .unwrap();

    process.expect(Regex("Welcome.*to.*ttd")).unwrap();
    process.send(format!("{}\r", task_dir.display())).unwrap();
    process.expect(Regex("Call.*Mom.*\\+Family")).unwrap();
    process.send(ControlCode::EndOfText).unwrap();

    let config_path = config_home.join("ttd/config.txt");
    assert_eq!(
        fs::read_to_string(config_path).unwrap(),
        task_dir.display().to_string()
    );
}

#[test]
fn live_tui_search_shows_the_active_query() {
    let root = temp_path("search-live");
    let config_home = root.join("config-home");
    let home = root.join("home");
    let task_dir = root.join("todo.txt.d");
    fs::create_dir_all(&config_home).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(task_dir.join("done.txt.d")).unwrap();
    fs::write(task_dir.join("a.txt"), "Call Mom\n").unwrap();
    fs::write(task_dir.join("b.txt"), "Email Alex\n").unwrap();
    fs::create_dir_all(config_home.join("ttd")).unwrap();
    fs::write(
        config_home.join("ttd/config.txt"),
        task_dir.display().to_string(),
    )
    .unwrap();

    let mut process = spawn(format!(
        "env HOME={} XDG_CONFIG_HOME={} {}",
        home.display(),
        config_home.display(),
        env!("CARGO_BIN_EXE_ttd")
    ))
    .unwrap();

    process.expect(Regex("Call.*Mom")).unwrap();
    process.send("/").unwrap();
    process.expect(Regex("Search:")).unwrap();
    process.send(ControlCode::EndOfText).unwrap();
}

#[test]
fn live_tui_delete_removes_the_selected_task_file() {
    let root = temp_path("delete-live");
    let config_home = root.join("config-home");
    let home = root.join("home");
    let task_dir = root.join("todo.txt.d");
    fs::create_dir_all(&config_home).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(task_dir.join("done.txt.d")).unwrap();
    fs::write(task_dir.join("a.txt"), "Alpha task\n").unwrap();
    fs::write(task_dir.join("b.txt"), "Beta task\n").unwrap();
    fs::create_dir_all(config_home.join("ttd")).unwrap();
    fs::write(
        config_home.join("ttd/config.txt"),
        task_dir.display().to_string(),
    )
    .unwrap();

    let mut process = spawn(format!(
        "env HOME={} XDG_CONFIG_HOME={} {}",
        home.display(),
        config_home.display(),
        env!("CARGO_BIN_EXE_ttd")
    ))
    .unwrap();

    process.expect(Regex("Alpha.*task")).unwrap();
    process.send("D\r").unwrap();

    for _ in 0..20 {
        if !task_dir.join("a.txt").exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    assert!(!task_dir.join("a.txt").exists());
    assert!(task_dir.join("b.txt").exists());
    process.send(ControlCode::EndOfText).unwrap();
}
