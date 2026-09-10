use std::{path::Path, process::Command};

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandContext {
    pub shell: String,
    pub cwd: String,
    pub os: String,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitContext>,
}

#[derive(Debug, Serialize)]
pub struct GitContext {
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub dirty: bool,
}

impl CommandContext {
    pub fn collect(shell: &str, cwd: &Path) -> Self {
        Self {
            shell: shell.to_owned(),
            cwd: cwd.display().to_string(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            git: collect_git(cwd),
        }
    }
}

fn collect_git(cwd: &Path) -> Option<GitContext> {
    let root = git_output(cwd, &["rev-parse", "--show-toplevel"])?;
    let branch = git_output(cwd, &["branch", "--show-current"]).filter(|value| !value.is_empty());
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| !output.stdout.is_empty());

    Some(GitContext {
        root,
        branch,
        dirty,
    })
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_repo_omits_git_context() {
        let dir = tempfile::tempdir().unwrap();
        let context = CommandContext::collect("zsh", dir.path());
        assert!(context.git.is_none());
    }
}
