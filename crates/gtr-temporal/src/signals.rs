use serde::{Deserialize, Serialize};

// WorkItem signal names
pub const SIGNAL_ASSIGN: &str = "assign";
pub const SIGNAL_START: &str = "start";
pub const SIGNAL_COMPLETE: &str = "complete";
pub const SIGNAL_FAIL: &str = "fail";
pub const SIGNAL_CLOSE: &str = "close";
pub const SIGNAL_RELEASE: &str = "release";
pub const SIGNAL_HEARTBEAT: &str = "heartbeat";
pub const SIGNAL_ESCALATE: &str = "escalate";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignSignal {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatSignal {
    pub progress: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailSignal {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemState {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assigned_to: Option<String>,
}
