use std::io;

use crate::config::{AppConfig, ConfigPaths, validate_task_dir};

#[derive(Debug)]
pub enum LaunchMode {
    Welcome,
    Main(AppConfig),
}

impl LaunchMode {
    pub fn from_disk(paths: &ConfigPaths) -> io::Result<Self> {
        match AppConfig::load(paths) {
            Err(error) if is_structural_config_error(&error) => Ok(Self::Welcome),
            Ok(config) => match validate_task_dir(&config.task_dir) {
                Ok(()) => Ok(Self::Main(config)),
                Err(error) if is_structural_config_error(&error) => Ok(Self::Welcome),
                Err(error) => Err(error),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::Welcome),
            Err(error) => Err(error),
        }
    }
}

fn is_structural_config_error(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::InvalidInput)
}

#[cfg(test)]
mod tests {
    use super::is_structural_config_error;
    use std::io;

    #[test]
    fn invalid_input_is_treated_as_structural_config_error() {
        let error = io::Error::new(io::ErrorKind::InvalidInput, "bad task dir");

        assert!(is_structural_config_error(&error));
    }

    #[test]
    fn permission_denied_is_not_treated_as_structural_config_error() {
        let error = io::Error::new(io::ErrorKind::PermissionDenied, "cannot write");

        assert!(!is_structural_config_error(&error));
    }
}
