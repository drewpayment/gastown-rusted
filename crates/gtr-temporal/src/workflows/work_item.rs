use temporalio_sdk::{WfContext, WfExitValue};

pub async fn work_item_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let args = ctx.get_args();
    let title = if let Some(payload) = args.first() {
        serde_json::from_slice::<String>(&payload.data)
            .unwrap_or_else(|_| "untitled".into())
    } else {
        "untitled".into()
    };
    tracing::info!("WorkItem workflow started: {}", title);
    Ok(WfExitValue::Normal(format!("WorkItem created: {}", title)))
}
