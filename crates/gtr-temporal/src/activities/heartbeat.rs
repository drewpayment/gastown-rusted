use serde::{Deserialize, Serialize};
use temporalio_sdk::{ActContext, ActivityError};

use crate::pty;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatInput {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatOutput {
    pub agent_id: String,
    pub alive: bool,
    pub pid: Option<u32>,
}

pub async fn check_agent_alive(
    _ctx: ActContext,
    input: HeartbeatInput,
) -> Result<HeartbeatOutput, ActivityError> {
    let alive = pty::is_alive(&input.agent_id);
    let pid = pty::read_pid(&input.agent_id).map(|p| p.as_raw() as u32);

    Ok(HeartbeatOutput {
        agent_id: input.agent_id,
        alive,
        pid,
    })
}
