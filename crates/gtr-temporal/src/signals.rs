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

// Agent signal names
pub const SIGNAL_AGENT_ASSIGN: &str = "agent_assign";
pub const SIGNAL_AGENT_MAIL: &str = "agent_mail";
pub const SIGNAL_AGENT_NUDGE: &str = "agent_nudge";
pub const SIGNAL_AGENT_STOP: &str = "agent_stop";
pub const SIGNAL_AGENT_UNASSIGN: &str = "agent_unassign";

// Convoy signal names
pub const SIGNAL_ADD_WORK_ITEM: &str = "add_work_item";
pub const SIGNAL_ITEM_DONE: &str = "item_done";
pub const SIGNAL_CANCEL_CONVOY: &str = "cancel_convoy";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddWorkItemSignal {
    pub work_item_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDoneSignal {
    pub work_item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvoyState {
    pub id: String,
    pub title: String,
    pub status: String,
    pub work_items: Vec<String>,
    pub completed_items: Vec<String>,
}

// Agent signal payloads

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAssignSignal {
    pub work_item_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMailSignal {
    pub from: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNudgeSignal {
    pub from: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailEntry {
    pub from: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub id: String,
    pub role: String,
    pub status: String,
    pub current_work: Option<String>,
    pub inbox: Vec<MailEntry>,
}
