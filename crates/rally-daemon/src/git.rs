use std::path::{Path, PathBuf};
use std::process::Command;

use compact_str::CompactString;
use tracing::debug;

pub struct GitInfo {
    pub project_root: PathBuf,
    pub branch: Option<CompactString>,
}

/// Discover git repo root and current branch from a working directory.
/// Returns None if the path is not inside a git repository.
pub fn discover(cwd: &Path) -> Option<GitInfo> {
    let root_output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !root_output.status.success() {
        debug!(cwd = %cwd.display(), "not a git repository");
        return None;
    }

    let root_str = String::from_utf8_lossy(&root_output.stdout);
    let project_root = PathBuf::from(root_str.trim());

    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()?;

    let branch = if branch_output.status.success() {
        let b = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();
        if b == "HEAD" {
            None
        } else {
            Some(CompactString::from(b))
        }
    } else {
        None
    };

    Some(GitInfo {
        project_root,
        branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_git_repo_from_subdirectory() {
        // This test runs from within the rally repo itself
        let cwd = std::env::current_dir().unwrap();
        let info = discover(&cwd).expect("should find git repo");
        assert!(info.project_root.exists());
        assert!(info.project_root.join(".git").exists());
        assert!(info.branch.is_some());
    }

    #[test]
    fn returns_none_for_non_repo() {
        let info = discover(Path::new("/tmp"));
        assert!(info.is_none());
    }
}
