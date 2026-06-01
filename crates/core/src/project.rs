use crate::error::{CoreError, CoreResult};
use std::path::{Path, PathBuf};

/// Resolved root of an aiken project (directory containing aiken.toml).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot(PathBuf);

impl ProjectRoot {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Walk upward from `start` looking for an `aiken.toml`. Returns the
    /// directory containing it, or `ProjectNotFound` if none found.
    pub fn discover(start: impl AsRef<Path>) -> CoreResult<Self> {
        let start = start.as_ref();
        let mut current: Option<&Path> = if start.is_file() {
            start.parent()
        } else {
            Some(start)
        };

        while let Some(dir) = current {
            if dir.join("aiken.toml").is_file() {
                return Ok(Self(dir.to_path_buf()));
            }
            current = dir.parent();
        }

        Err(CoreError::ProjectNotFound {
            path: start.display().to_string(),
        })
    }
}

/// Project context passed to tools.
#[derive(Debug, Clone)]
pub struct Project {
    pub root: ProjectRoot,
}

impl Project {
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discover_finds_aiken_toml_in_parent() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("aiken.toml"), "name = \"test\"\n").unwrap();
        let nested = root.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let found = ProjectRoot::discover(&nested).unwrap();
        assert_eq!(found.as_path(), root);
    }

    #[test]
    fn discover_errors_when_missing() {
        let tmp = tempdir().unwrap();
        let err = ProjectRoot::discover(tmp.path()).unwrap_err();
        assert!(matches!(err, CoreError::ProjectNotFound { .. }));
    }
}
