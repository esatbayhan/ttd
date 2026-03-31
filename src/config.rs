use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub task_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub root: PathBuf,
    pub config_file: PathBuf,
}

impl ConfigPaths {
    pub fn from_root(root: PathBuf) -> Self {
        let config_file = root.join("config.txt");
        Self { root, config_file }
    }

    pub fn discover() -> io::Result<Self> {
        if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
            return Ok(Self::from_root(PathBuf::from(config_home).join("ttd")));
        }

        if let Some(home) = env::var_os("HOME") {
            return Ok(Self::from_root(PathBuf::from(home).join(".config/ttd")));
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "HOME or XDG_CONFIG_HOME must be set to resolve config paths",
        ))
    }
}

impl AppConfig {
    pub fn save(&self, paths: &ConfigPaths) -> io::Result<()> {
        fs::create_dir_all(&paths.root)?;
        fs::write(&paths.config_file, self.task_dir.display().to_string())
    }

    pub fn load(paths: &ConfigPaths) -> io::Result<Self> {
        let task_dir = fs::read_to_string(&paths.config_file)?;
        let task_dir = task_dir
            .strip_suffix("\r\n")
            .or_else(|| task_dir.strip_suffix('\n'))
            .unwrap_or(&task_dir);

        if task_dir.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "config file is empty",
            ));
        }

        if task_dir.contains(['\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "config file must contain exactly one path line",
            ));
        }

        Ok(Self {
            task_dir: PathBuf::from(task_dir),
        })
    }
}

pub fn validate_task_dir(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path.join("done.txt.d"))?;
        return Ok(());
    }

    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task dir is not a directory",
        ));
    }

    fs::create_dir_all(path.join("done.txt.d"))
}
