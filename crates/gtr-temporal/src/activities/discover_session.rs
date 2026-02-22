use serde::{Deserialize, Serialize};
use temporalio_sdk::{ActContext, ActivityError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverSessionInput {
    pub work_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverSessionOutput {
    pub session_id: Option<String>,
}

/// Sanitize a path the same way Claude Code does: replace / with -
fn sanitize_path(path: &str) -> String {
    path.replace('/', "-")
}

pub async fn discover_session_id(
    _ctx: ActContext,
    input: DiscoverSessionInput,
) -> Result<DiscoverSessionOutput, ActivityError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());

    // Claude Code stores sessions at ~/.claude/projects/-{sanitized-path}/
    let sanitized = sanitize_path(&input.work_dir);
    let sessions_dir = std::path::PathBuf::from(&home)
        .join(".claude")
        .join("projects")
        .join(&sanitized);

    if !sessions_dir.exists() {
        return Ok(DiscoverSessionOutput { session_id: None });
    }

    // Find the most recent .jsonl file (by modification time)
    let mut newest: Option<(std::time::SystemTime, String)> = None;

    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        if newest.as_ref().map_or(true, |(t, _)| modified > *t) {
                            newest = Some((modified, stem));
                        }
                    }
                }
            }
        }
    }

    Ok(DiscoverSessionOutput {
        session_id: newest.map(|(_, id)| id),
    })
}
