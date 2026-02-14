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

// Mayor signal names
pub const SIGNAL_REGISTER_AGENT: &str = "register_agent";
pub const SIGNAL_UNREGISTER_AGENT: &str = "unregister_agent";
pub const SIGNAL_AGENT_STATUS_UPDATE: &str = "agent_status_update";
pub const SIGNAL_CONVOY_CLOSED: &str = "convoy_closed";
pub const SIGNAL_MAYOR_STOP: &str = "mayor_stop";

// Mayor signal payloads

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAgentSignal {
    pub agent_id: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusUpdateSignal {
    pub agent_id: String,
    pub status: String,
    pub current_work: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvoyClosedSignal {
    pub convoy_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MayorState {
    pub active_convoys: Vec<String>,
    pub agents: Vec<MayorAgentEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MayorAgentEntry {
    pub agent_id: String,
    pub role: String,
    pub status: String,
    pub current_work: Option<String>,
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
    #[serde(default)]
    pub hook: Option<HookSignal>,
}

// Refinery signal names
pub const SIGNAL_REFINERY_ENQUEUE: &str = "refinery_enqueue";
pub const SIGNAL_REFINERY_DEQUEUE: &str = "refinery_dequeue";
pub const SIGNAL_REFINERY_STOP: &str = "refinery_stop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineryEnqueueSignal {
    pub work_item_id: String,
    pub branch: String,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineryDequeueSignal {
    pub work_item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineryEntry {
    pub work_item_id: String,
    pub branch: String,
    pub priority: u8,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineryState {
    pub queue: Vec<RefineryEntry>,
    pub processed: Vec<RefineryEntry>,
}

// Rig signal names
pub const SIGNAL_RIG_PARK: &str = "rig_park";
pub const SIGNAL_RIG_UNPARK: &str = "rig_unpark";
pub const SIGNAL_RIG_DOCK: &str = "rig_dock";
pub const SIGNAL_RIG_UNDOCK: &str = "rig_undock";
pub const SIGNAL_RIG_REGISTER_AGENT: &str = "rig_register_agent";
pub const SIGNAL_RIG_UNREGISTER_AGENT: &str = "rig_unregister_agent";
pub const SIGNAL_RIG_STOP: &str = "rig_stop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigAgentEntry {
    pub agent_id: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigState {
    pub name: String,
    pub git_url: String,
    pub status: String,
    pub agents: Vec<RigAgentEntry>,
    pub polecats: Vec<String>,
    pub crew: Vec<String>,
    pub has_witness: bool,
    pub has_refinery: bool,
}

// Polecat signal names
pub const SIGNAL_POLECAT_HEARTBEAT: &str = "polecat_heartbeat";
pub const SIGNAL_POLECAT_DONE: &str = "polecat_done";
pub const SIGNAL_POLECAT_STUCK: &str = "polecat_stuck";
pub const SIGNAL_POLECAT_KILL: &str = "polecat_kill";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolecatDoneSignal {
    pub branch: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolecatState {
    pub name: String,
    pub rig: String,
    pub work_item_id: String,
    pub status: String,
    pub branch: String,
    pub worktree_path: String,
}

// Hook signal names
pub const SIGNAL_HOOK: &str = "hook";
pub const SIGNAL_HOOK_CLEAR: &str = "hook_clear";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSignal {
    pub work_item_id: String,
    pub title: String,
    pub molecule_id: Option<String>,
    pub current_step: Option<String>,
}
