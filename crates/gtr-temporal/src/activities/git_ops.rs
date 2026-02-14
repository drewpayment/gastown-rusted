use std::path::Path;

use serde::{Deserialize, Serialize};
use temporalio_sdk::ActContext;
use temporalio_sdk::ActivityError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum GitOperation {
    #[serde(rename = "clone")]
    Clone { url: String, dest: String },
    #[serde(rename = "checkout")]
    Checkout { repo_path: String, branch: String, create: bool },
    #[serde(rename = "commit")]
    Commit { repo_path: String, message: String },
    #[serde(rename = "push")]
    Push { repo_path: String, remote: String, branch: String },
    #[serde(rename = "worktree_add")]
    WorktreeAdd { repo_path: String, path: String, branch: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitResult {
    pub op: String,
    pub success: bool,
    pub message: String,
}

pub async fn git_operation(_ctx: ActContext, op: GitOperation) -> Result<GitResult, ActivityError> {
    // Run blocking git2 operations on a blocking thread
    let result = tokio::task::spawn_blocking(move || run_git_op(op))
        .await
        .map_err(|e| ActivityError::Retryable {
                source: anyhow::anyhow!("join error: {e}"),
                explicit_delay: None,
            })?;

    result
}

fn run_git_op(op: GitOperation) -> Result<GitResult, ActivityError> {
    match op {
        GitOperation::Clone { url, dest } => {
            tracing::info!("git clone {url} -> {dest}");
            git2::Repository::clone(&url, &dest).map_err(|e| {
                ActivityError::NonRetryable(anyhow::anyhow!("clone failed: {e}"))
            })?;
            Ok(GitResult {
                op: "clone".into(),
                success: true,
                message: format!("Cloned {url} to {dest}"),
            })
        }
        GitOperation::Checkout {
            repo_path,
            branch,
            create,
        } => {
            tracing::info!("git checkout {branch} in {repo_path} (create: {create})");
            let repo = open_repo(&repo_path)?;

            if create {
                // Create branch from HEAD
                let head = repo.head().map_err(git_err)?;
                let commit = head.peel_to_commit().map_err(git_err)?;
                repo.branch(&branch, &commit, false).map_err(git_err)?;
            }

            // Checkout
            let refname = format!("refs/heads/{branch}");
            let obj = repo
                .revparse_single(&refname)
                .map_err(git_err)?;
            repo.checkout_tree(&obj, None).map_err(git_err)?;
            repo.set_head(&refname).map_err(git_err)?;

            Ok(GitResult {
                op: "checkout".into(),
                success: true,
                message: format!("Checked out {branch}"),
            })
        }
        GitOperation::Commit {
            repo_path,
            message,
        } => {
            tracing::info!("git commit in {repo_path}: {message}");
            let repo = open_repo(&repo_path)?;

            let mut index = repo.index().map_err(git_err)?;
            index
                .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .map_err(git_err)?;
            index.write().map_err(git_err)?;
            let tree_oid = index.write_tree().map_err(git_err)?;
            let tree = repo.find_tree(tree_oid).map_err(git_err)?;

            let sig = repo.signature().unwrap_or_else(|_| {
                git2::Signature::now("gtr", "gtr@gastownrusted.dev").unwrap()
            });

            let parent = repo
                .head()
                .ok()
                .and_then(|h| h.peel_to_commit().ok());
            let parents: Vec<&git2::Commit> = parent.iter().collect();

            let oid = repo
                .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)
                .map_err(git_err)?;

            Ok(GitResult {
                op: "commit".into(),
                success: true,
                message: format!("Committed {}", &oid.to_string()[..8]),
            })
        }
        GitOperation::Push {
            repo_path,
            remote,
            branch,
        } => {
            tracing::info!("git push {remote} {branch} in {repo_path}");
            let repo = open_repo(&repo_path)?;
            let mut remote = repo.find_remote(&remote).map_err(git_err)?;
            let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
            remote.push(&[&refspec], None).map_err(git_err)?;

            Ok(GitResult {
                op: "push".into(),
                success: true,
                message: format!("Pushed {branch}"),
            })
        }
        GitOperation::WorktreeAdd {
            repo_path,
            path,
            branch,
        } => {
            tracing::info!("git worktree add {path} {branch} in {repo_path}");
            let repo = open_repo(&repo_path)?;

            // Create branch from HEAD if it doesn't exist
            let head = repo.head().map_err(git_err)?;
            let commit = head.peel_to_commit().map_err(git_err)?;
            let branch_ref = match repo.find_branch(&branch, git2::BranchType::Local) {
                Ok(b) => b,
                Err(_) => repo.branch(&branch, &commit, false).map_err(git_err)?,
            };

            let reference = branch_ref.into_reference();
            repo.worktree(
                &branch,
                Path::new(&path),
                Some(
                    git2::WorktreeAddOptions::new()
                        .reference(Some(&reference)),
                ),
            )
            .map_err(git_err)?;

            Ok(GitResult {
                op: "worktree_add".into(),
                success: true,
                message: format!("Created worktree at {path} on branch {branch}"),
            })
        }
    }
}

fn open_repo(path: &str) -> Result<git2::Repository, ActivityError> {
    git2::Repository::open(path)
        .map_err(|e| ActivityError::NonRetryable(anyhow::anyhow!("open repo: {e}")))
}

fn git_err(e: git2::Error) -> ActivityError {
    ActivityError::NonRetryable(anyhow::anyhow!("git error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_clone_op() {
        let op = GitOperation::Clone {
            url: "https://github.com/foo/bar.git".into(),
            dest: "/tmp/bar".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"op\":\"clone\""));
        let parsed: GitOperation = serde_json::from_str(&json).unwrap();
        match parsed {
            GitOperation::Clone { url, dest } => {
                assert_eq!(url, "https://github.com/foo/bar.git");
                assert_eq!(dest, "/tmp/bar");
            }
            _ => panic!("expected Clone"),
        }
    }

    #[test]
    fn serde_checkout_op() {
        let op = GitOperation::Checkout {
            repo_path: "/repo".into(),
            branch: "feature/x".into(),
            create: true,
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: GitOperation = serde_json::from_str(&json).unwrap();
        match parsed {
            GitOperation::Checkout { branch, create, .. } => {
                assert_eq!(branch, "feature/x");
                assert!(create);
            }
            _ => panic!("expected Checkout"),
        }
    }

    #[test]
    fn serde_worktree_op() {
        let op = GitOperation::WorktreeAdd {
            repo_path: "/repo".into(),
            path: "/work/feat".into(),
            branch: "feat".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"op\":\"worktree_add\""));
    }
}
