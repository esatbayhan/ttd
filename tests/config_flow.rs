use std::fs;
use std::io;
use std::path::PathBuf;

use ttd::bootstrap::LaunchMode;
use ttd::config::{AppConfig, ConfigPaths, validate_task_dir};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ttd-tests-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

#[test]
fn missing_config_enters_welcome_mode() {
    let root = temp_path("welcome");
    fs::create_dir_all(&root).unwrap();
    let paths = ConfigPaths::from_root(root.clone());

    let mode = LaunchMode::from_disk(&paths).unwrap();

    assert!(matches!(mode, LaunchMode::Welcome));
}

#[test]
fn validation_accepts_existing_todo_directory() {
    let root = temp_path("validate");
    fs::create_dir_all(root.join("todo.txt.d/done.txt.d")).unwrap();

    validate_task_dir(&root.join("todo.txt.d")).unwrap();
}

#[test]
fn persisted_config_loads_main_mode() {
    let root = temp_path("config");
    fs::create_dir_all(root.join("todo.txt.d/done.txt.d")).unwrap();
    let paths = ConfigPaths::from_root(root.clone());

    AppConfig {
        task_dir: root.join("todo.txt.d"),
    }
    .save(&paths)
    .unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    match mode {
        LaunchMode::Main(config) => {
            assert_eq!(config.task_dir, root.join("todo.txt.d"));
        }
        LaunchMode::Welcome => panic!("expected persisted config to enter main mode"),
    }
}

#[test]
fn invalid_config_falls_back_to_welcome_mode() {
    let root = temp_path("invalid");
    fs::create_dir_all(&root).unwrap();
    let task_file = root.join("todo.txt.d");
    fs::write(&task_file, "not a directory").unwrap();
    let paths = ConfigPaths::from_root(root.clone());

    AppConfig {
        task_dir: task_file,
    }
    .save(&paths)
    .unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    assert!(matches!(mode, LaunchMode::Welcome));
}

#[test]
fn empty_config_falls_back_to_welcome_mode() {
    let root = temp_path("empty-config");
    fs::create_dir_all(&root).unwrap();
    let paths = ConfigPaths::from_root(root.clone());
    fs::write(&paths.config_file, "").unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    assert!(matches!(mode, LaunchMode::Welcome));
}

#[test]
fn newline_only_config_falls_back_to_welcome_mode() {
    let root = temp_path("newline-only-config");
    fs::create_dir_all(&root).unwrap();
    let paths = ConfigPaths::from_root(root.clone());
    fs::write(&paths.config_file, "\n").unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    assert!(matches!(mode, LaunchMode::Welcome));
}

#[test]
fn crlf_only_config_falls_back_to_welcome_mode() {
    let root = temp_path("crlf-only-config");
    fs::create_dir_all(&root).unwrap();
    let paths = ConfigPaths::from_root(root.clone());
    fs::write(&paths.config_file, "\r\n").unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    assert!(matches!(mode, LaunchMode::Welcome));
}

#[test]
fn persisted_path_with_trailing_newline_loads_main_mode() {
    let root = temp_path("config-lf");
    fs::create_dir_all(root.join("todo.txt.d/done.txt.d")).unwrap();
    let paths = ConfigPaths::from_root(root.clone());
    fs::write(
        &paths.config_file,
        format!("{}\n", root.join("todo.txt.d").display()),
    )
    .unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    match mode {
        LaunchMode::Main(config) => {
            assert_eq!(config.task_dir, root.join("todo.txt.d"));
        }
        LaunchMode::Welcome => panic!("expected config with trailing newline to enter main mode"),
    }
}

#[test]
fn malformed_multiline_config_falls_back_to_welcome_mode() {
    let cases = ["\n\n", "/path\n\n", "/path\nextra", "/path\r\n\r\n"];

    for (index, contents) in cases.into_iter().enumerate() {
        let root = temp_path(&format!("multiline-{}", index));
        fs::create_dir_all(&root).unwrap();
        let paths = ConfigPaths::from_root(root.clone());
        fs::write(&paths.config_file, contents).unwrap();

        let mode = LaunchMode::from_disk(&paths).unwrap();

        assert!(
            matches!(mode, LaunchMode::Welcome),
            "contents: {contents:?}"
        );
    }
}

#[test]
fn persisted_path_with_trailing_crlf_loads_main_mode() {
    let root = temp_path("config-crlf");
    fs::create_dir_all(root.join("todo.txt.d/done.txt.d")).unwrap();
    let paths = ConfigPaths::from_root(root.clone());
    fs::write(
        &paths.config_file,
        format!("{}\r\n", root.join("todo.txt.d").display()),
    )
    .unwrap();

    let mode = LaunchMode::from_disk(&paths).unwrap();

    match mode {
        LaunchMode::Main(config) => {
            assert_eq!(config.task_dir, root.join("todo.txt.d"));
        }
        LaunchMode::Welcome => panic!("expected config with trailing crlf to enter main mode"),
    }
}

#[cfg(unix)]
#[test]
fn operational_validation_error_is_returned() {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    struct PermissionReset<'a> {
        path: &'a Path,
        mode: u32,
    }

    impl Drop for PermissionReset<'_> {
        fn drop(&mut self) {
            let _ = fs::set_permissions(self.path, fs::Permissions::from_mode(self.mode));
        }
    }

    let root = temp_path("permission-denied");
    fs::create_dir_all(&root).unwrap();
    let locked_parent = root.join("locked");
    fs::create_dir_all(&locked_parent).unwrap();
    let reset = PermissionReset {
        path: &locked_parent,
        mode: 0o755,
    };

    let paths = ConfigPaths::from_root(root.clone());
    AppConfig {
        task_dir: locked_parent.join("todo.txt.d"),
    }
    .save(&paths)
    .unwrap();

    fs::set_permissions(&locked_parent, fs::Permissions::from_mode(0o555)).unwrap();

    let error = LaunchMode::from_disk(&paths).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    drop(reset);
}
