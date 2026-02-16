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
    #[serde(rename = "rebase")]
    Rebase { repo_path: String, branch: String, onto: String },
    #[serde(rename = "merge")]
    Merge { repo_path: String, branch: String },
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
            // Shell out to system git for push — inherits SSH agent, ~/.ssh/config,
            // credential helpers, etc. git2's push requires explicit SSH callbacks.
            tracing::info!("git push {remote} {branch} in {repo_path}");
            let output = std::process::Command::new("git")
                .args(["push", &remote, &branch])
                .current_dir(&repo_path)
                .output()
                .map_err(|e| {
                    ActivityError::NonRetryable(anyhow::anyhow!("git push spawn failed: {e}"))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                    "git push failed: {stderr}"
                )));
            }

            Ok(GitResult {
                op: "push".into(),
                success: true,
                message: format!("Pushed {branch} to {remote}"),
            })
        }
        GitOperation::WorktreeAdd {
            repo_path,
            path,
            branch,
        } => {
            tracing::info!("git worktree add {path} {branch} in {repo_path}");
            let wt_name = branch.replace('/', "-");
            let worktree_path = Path::new(&path);

            // Clean up stale worktree from a previous run if it exists
            if worktree_path.exists() {
                tracing::info!("Removing stale worktree dir: {path}");
                std::fs::remove_dir_all(worktree_path).map_err(|e| {
                    ActivityError::NonRetryable(anyhow::anyhow!(
                        "failed to remove stale worktree {path}: {e}"
                    ))
                })?;
            }
            // Also remove stale worktree metadata from the bare repo
            let wt_meta = Path::new(&repo_path).join("worktrees").join(&wt_name);
            if wt_meta.exists() {
                tracing::info!("Removing stale worktree metadata: {}", wt_meta.display());
                let _ = std::fs::remove_dir_all(&wt_meta);
            }

            let repo = open_repo(&repo_path)?;

            // Ensure parent directory of worktree path exists
            if let Some(parent) = worktree_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    ActivityError::NonRetryable(anyhow::anyhow!(
                        "failed to create worktree parent dir {}: {e}",
                        parent.display()
                    ))
                })?;
            }

            // Delete stale branch if it exists (from a previous failed run)
            // so we get a fresh branch from HEAD
            if let Ok(mut old_branch) = repo.find_branch(&branch, git2::BranchType::Local) {
                let _ = old_branch.delete();
            }

            // Create branch from HEAD
            let head = repo.head().map_err(git_err)?;
            let commit = head.peel_to_commit().map_err(git_err)?;
            let branch_ref = repo.branch(&branch, &commit, false).map_err(git_err)?;

            let reference = branch_ref.into_reference();
            // Worktree name must be flat (no slashes) — git2 creates
            // .repo.git/worktrees/<name>/ and slashes cause mkdir failures.
            repo.worktree(
                &wt_name,
                worktree_path,
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
        GitOperation::Rebase {
            repo_path,
            branch,
            onto,
        } => {
            tracing::info!("git rebase {branch} onto {onto} in {repo_path}");
            let repo = open_repo(&repo_path)?;

            let branch_ref = format!("refs/heads/{branch}");
            let onto_ref = format!("refs/heads/{onto}");

            let branch_annotated = repo
                .find_annotated_commit(
                    repo.revparse_single(&branch_ref)
                        .map_err(git_err)?
                        .id(),
                )
                .map_err(git_err)?;
            let onto_annotated = repo
                .find_annotated_commit(
                    repo.revparse_single(&onto_ref)
                        .map_err(git_err)?
                        .id(),
                )
                .map_err(git_err)?;

            let mut rebase = repo
                .rebase(Some(&branch_annotated), Some(&onto_annotated), None, None)
                .map_err(git_err)?;

            let sig = repo.signature().unwrap_or_else(|_| {
                git2::Signature::now("gtr", "gtr@gastownrusted.dev").unwrap()
            });

            // Apply each rebase operation
            while rebase.next().is_some() {
                if let Err(e) = rebase.commit(None, &sig, None) {
                    rebase.abort().ok();
                    return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                        "rebase conflict on {branch} onto {onto}: {e}"
                    )));
                }
            }
            rebase.finish(None).map_err(git_err)?;

            Ok(GitResult {
                op: "rebase".into(),
                success: true,
                message: format!("Rebased {branch} onto {onto}"),
            })
        }
        GitOperation::Merge {
            repo_path,
            branch,
        } => {
            tracing::info!("git merge {branch} in {repo_path}");
            let repo = open_repo(&repo_path)?;

            // Resolve branch to annotated commit
            let branch_ref = format!("refs/heads/{branch}");
            let branch_oid = repo
                .revparse_single(&branch_ref)
                .map_err(git_err)?
                .id();
            let annotated = repo
                .find_annotated_commit(branch_oid)
                .map_err(git_err)?;

            // Perform merge analysis
            let (analysis, _) = repo.merge_analysis(&[&annotated]).map_err(git_err)?;

            if analysis.is_up_to_date() {
                return Ok(GitResult {
                    op: "merge".into(),
                    success: true,
                    message: format!("{branch} already up to date"),
                });
            }

            if analysis.is_fast_forward() {
                // Fast-forward: just move HEAD
                let mut reference = repo.find_reference("HEAD").map_err(git_err)?;
                reference
                    .set_target(branch_oid, &format!("merge: fast-forward {branch}"))
                    .map_err(git_err)?;
                repo.checkout_head(Some(
                    git2::build::CheckoutBuilder::new().force(),
                ))
                .map_err(git_err)?;
            } else {
                // Normal merge
                repo.merge(&[&annotated], None, None).map_err(git_err)?;

                // Check for conflicts
                let index = repo.index().map_err(git_err)?;
                if index.has_conflicts() {
                    repo.cleanup_state().map_err(git_err)?;
                    return Err(ActivityError::NonRetryable(anyhow::anyhow!(
                        "merge conflict merging {branch}"
                    )));
                }

                // Commit the merge
                let mut index = repo.index().map_err(git_err)?;
                let tree_oid = index.write_tree().map_err(git_err)?;
                let tree = repo.find_tree(tree_oid).map_err(git_err)?;
                let sig = repo.signature().unwrap_or_else(|_| {
                    git2::Signature::now("gtr", "gtr@gastownrusted.dev").unwrap()
                });
                let head_commit = repo.head().map_err(git_err)?.peel_to_commit().map_err(git_err)?;
                let branch_commit = repo.find_commit(branch_oid).map_err(git_err)?;

                repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &format!("Merge branch '{branch}'"),
                    &tree,
                    &[&head_commit, &branch_commit],
                )
                .map_err(git_err)?;

                repo.cleanup_state().map_err(git_err)?;
            }

            Ok(GitResult {
                op: "merge".into(),
                success: true,
                message: format!("Merged {branch}"),
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

    #[test]
    fn serde_rebase_op() {
        let op = GitOperation::Rebase {
            repo_path: "/repo".into(),
            branch: "feature/x".into(),
            onto: "main".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"op\":\"rebase\""));
        let parsed: GitOperation = serde_json::from_str(&json).unwrap();
        match parsed {
            GitOperation::Rebase { branch, onto, .. } => {
                assert_eq!(branch, "feature/x");
                assert_eq!(onto, "main");
            }
            _ => panic!("expected Rebase"),
        }
    }

    #[test]
    fn serde_merge_op() {
        let op = GitOperation::Merge {
            repo_path: "/repo".into(),
            branch: "feature/y".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"op\":\"merge\""));
        let parsed: GitOperation = serde_json::from_str(&json).unwrap();
        match parsed {
            GitOperation::Merge { branch, .. } => {
                assert_eq!(branch, "feature/y");
            }
            _ => panic!("expected Merge"),
        }
    }
}
