use std::path::PathBuf;

/// Unified error type for the gtr system.
#[derive(Debug, thiserror::Error)]
pub enum GtrError {
    #[error("config not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("config parse error: {0}")]
    ConfigParse(String),

    #[error("invalid state transition: {0}")]
    InvalidTransition(String),

    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("work item not found: {0}")]
    WorkItemNotFound(String),

    #[error("convoy not found: {0}")]
    ConvoyNotFound(String),

    #[error("temporal error: {0}")]
    Temporal(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
