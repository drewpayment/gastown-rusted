use temporalio_sdk::{WfContext, WfExitValue, WorkflowResult};

/// Hello-world workflow proving Temporal SDK integration.
/// Returns a greeting string.
pub async fn work_item_wf(_ctx: WfContext) -> WorkflowResult<String> {
    Ok(WfExitValue::Normal(
        "Hello from gtr work_item_wf".to_string(),
    ))
}
