use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- ID types ---

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkItemId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConvoyId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

// --- Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    Pending,
    InProgress,
    Blocked,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvoyStatus {
    Queued,
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Mayor,
    Witness,
    Refinery,
    Polecat,
    Crew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRuntime {
    Claude,
    Human,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Offline,
}

// --- Structs ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub title: String,
    pub description: String,
    pub status: WorkItemStatus,
    pub priority: Priority,
    pub assignee: Option<AgentId>,
    pub depends_on: Vec<WorkItemId>,
    pub blocks: Vec<WorkItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convoy {
    pub id: ConvoyId,
    pub status: ConvoyStatus,
    pub items: Vec<WorkItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: AgentId,
    pub name: String,
    pub role: AgentRole,
    pub runtime: AgentRuntime,
    pub status: AgentStatus,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_status_serde_roundtrip() {
        let json = serde_json::to_string(&WorkItemStatus::InProgress).unwrap();
        assert_eq!(json, r#""in_progress""#);
        let parsed: WorkItemStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkItemStatus::InProgress);
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::P0 < Priority::P1);
        assert!(Priority::P1 < Priority::P2);
        assert!(Priority::P2 < Priority::P3);
    }

    #[test]
    fn work_item_construction_and_serde() {
        let item = WorkItem {
            id: WorkItemId("hq-nn6.2".into()),
            title: "Core Types".into(),
            description: "Create core type definitions".into(),
            status: WorkItemStatus::Pending,
            priority: Priority::P1,
            assignee: Some(AgentId("slit".into())),
            depends_on: vec![WorkItemId("hq-nn6.1".into())],
            blocks: vec![],
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: WorkItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, item.id);
        assert_eq!(parsed.priority, Priority::P1);
        assert_eq!(parsed.assignee.unwrap(), AgentId("slit".into()));
    }

    #[test]
    fn agent_config_with_metadata() {
        let mut meta = HashMap::new();
        meta.insert("worktree".into(), "/polecats/slit".into());
        let cfg = AgentConfig {
            id: AgentId("slit".into()),
            name: "Slit".into(),
            role: AgentRole::Polecat,
            runtime: AgentRuntime::Claude,
            status: AgentStatus::Working,
            capabilities: vec!["rust".into(), "types".into()],
            metadata: meta,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, AgentRole::Polecat);
        assert_eq!(parsed.runtime, AgentRuntime::Claude);
        assert_eq!(parsed.metadata.get("worktree").unwrap(), "/polecats/slit");
    }
}
