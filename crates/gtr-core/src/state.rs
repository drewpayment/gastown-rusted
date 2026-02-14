/// Canonical status strings used across all workflows.
/// These are the wire-format values that Temporal signals carry.
pub mod status {
    pub const PENDING: &str = "pending";
    pub const ASSIGNED: &str = "assigned";
    pub const IN_PROGRESS: &str = "in_progress";
    pub const DONE: &str = "done";
    pub const FAILED: &str = "failed";
    pub const CLOSED: &str = "closed";
    pub const IDLE: &str = "idle";
    pub const WORKING: &str = "working";
    pub const STOPPED: &str = "stopped";
    pub const OPEN: &str = "open";
    pub const QUEUED: &str = "queued";
    pub const VALIDATING: &str = "validating";
    pub const MERGING: &str = "merging";
    pub const MERGED: &str = "merged";
    pub const OPERATIONAL: &str = "operational";
    pub const PARKED: &str = "parked";
    pub const DOCKED: &str = "docked";
    pub const STUCK: &str = "stuck";
    pub const ZOMBIE: &str = "zombie";
}

/// Canonical agent roles.
pub mod roles {
    pub const MAYOR: &str = "mayor";
    pub const DEACON: &str = "deacon";
    pub const WITNESS: &str = "witness";
    pub const REFINERY: &str = "refinery";
    pub const POLECAT: &str = "polecat";
    pub const CREW: &str = "crew";
    pub const DOG: &str = "dog";
    pub const BOOT: &str = "boot";
}

/// Workflow ID conventions — ensures singleton workflows have deterministic IDs.
pub fn mayor_workflow_id() -> String {
    "mayor".to_string()
}

pub fn witness_workflow_id(rig: &str) -> String {
    format!("{rig}-witness")
}

pub fn refinery_workflow_id(rig: &str) -> String {
    format!("{rig}-refinery")
}

pub fn patrol_workflow_id() -> String {
    "patrol".to_string()
}

pub fn boot_workflow_id() -> String {
    "boot".to_string()
}

pub fn rig_workflow_id(rig: &str) -> String {
    format!("rig-{rig}")
}

pub fn polecat_workflow_id(rig: &str, name: &str) -> String {
    format!("{rig}-polecat-{name}")
}

pub fn dog_workflow_id(name: &str) -> String {
    format!("dog-{name}")
}

pub fn crew_workflow_id(rig: &str, name: &str) -> String {
    format!("{rig}-crew-{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_id_conventions() {
        assert_eq!(mayor_workflow_id(), "mayor");
        assert_eq!(witness_workflow_id("gt"), "gt-witness");
        assert_eq!(refinery_workflow_id("gt"), "gt-refinery");
        assert_eq!(rig_workflow_id("gt"), "rig-gt");
        assert_eq!(polecat_workflow_id("gt", "nux"), "gt-polecat-nux");
        assert_eq!(dog_workflow_id("alpha"), "dog-alpha");
        assert_eq!(crew_workflow_id("gt", "drew"), "gt-crew-drew");
    }
}
