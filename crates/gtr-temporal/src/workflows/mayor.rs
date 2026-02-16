use futures_util::StreamExt;
use temporalio_sdk::{WfContext, WfExitValue};

use crate::signals::*;

pub async fn mayor_wf(ctx: WfContext) -> Result<WfExitValue<String>, anyhow::Error> {
    let mut active_convoys: Vec<String> = vec![];
    let mut agents: Vec<MayorAgentEntry> = vec![];
    let mut polecat_reports: Vec<PolecatReportSignal> = vec![];

    let mut register_ch = ctx.make_signal_channel(SIGNAL_REGISTER_AGENT);
    let mut unregister_ch = ctx.make_signal_channel(SIGNAL_UNREGISTER_AGENT);
    let mut status_ch = ctx.make_signal_channel(SIGNAL_AGENT_STATUS_UPDATE);
    let mut convoy_closed_ch = ctx.make_signal_channel(SIGNAL_CONVOY_CLOSED);
    let mut add_convoy_ch = ctx.make_signal_channel(SIGNAL_ADD_WORK_ITEM);
    let mut stop_ch = ctx.make_signal_channel(SIGNAL_MAYOR_STOP);
    let mut report_ch = ctx.make_signal_channel(SIGNAL_POLECAT_REPORT);

    tracing::info!("Mayor workflow started");

    loop {
        tokio::select! {
            Some(signal) = register_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<RegisterAgentSignal>(&payload.data) {
                        if !agents.iter().any(|a| a.agent_id == data.agent_id) {
                            tracing::info!("Mayor: registered agent {} ({})", data.agent_id, data.role);
                            agents.push(MayorAgentEntry {
                                agent_id: data.agent_id,
                                role: data.role,
                                status: "idle".to_string(),
                                current_work: None,
                            });
                        }
                    }
                }
            }
            Some(signal) = unregister_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(agent_id) = serde_json::from_slice::<String>(&payload.data) {
                        agents.retain(|a| a.agent_id != agent_id);
                        tracing::info!("Mayor: unregistered agent {agent_id}");
                    }
                }
            }
            Some(signal) = status_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<AgentStatusUpdateSignal>(&payload.data) {
                        if let Some(agent) = agents.iter_mut().find(|a| a.agent_id == data.agent_id) {
                            agent.status = data.status;
                            agent.current_work = data.current_work;
                            tracing::info!("Mayor: agent {} status → {}", data.agent_id, agent.status);
                        }
                    }
                }
            }
            Some(signal) = add_convoy_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(convoy_id) = serde_json::from_slice::<String>(&payload.data) {
                        if !active_convoys.contains(&convoy_id) {
                            active_convoys.push(convoy_id.clone());
                            tracing::info!("Mayor: tracking convoy {convoy_id}");
                        }
                    }
                }
            }
            Some(signal) = convoy_closed_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(data) = serde_json::from_slice::<ConvoyClosedSignal>(&payload.data) {
                        active_convoys.retain(|c| *c != data.convoy_id);
                        tracing::info!("Mayor: convoy {} closed", data.convoy_id);
                    }
                }
            }
            Some(signal) = report_ch.next() => {
                if let Some(payload) = signal.input.first() {
                    if let Ok(report) = serde_json::from_slice::<PolecatReportSignal>(&payload.data) {
                        tracing::info!(
                            "Mayor: polecat report — {} ({}) status={} exit={}{}",
                            report.polecat_id, report.work_item_id, report.status, report.exit_reason,
                            report.summary.as_ref().map(|s| format!(" summary={}", &s[..s.len().min(100)])).unwrap_or_default()
                        );
                        if let Some(agent) = agents.iter_mut().find(|a| a.agent_id == report.polecat_id) {
                            agent.status = report.status.clone();
                            agent.current_work = Some(report.work_item_id.clone());
                        }
                        polecat_reports.push(report);
                    }
                }
            }
            Some(_) = stop_ch.next() => {
                tracing::info!("Mayor stopping — {} agents, {} convoys, {} reports", agents.len(), active_convoys.len(), polecat_reports.len());
                return Ok(WfExitValue::Normal(
                    serde_json::to_string(&MayorState {
                        active_convoys,
                        agents,
                        polecat_reports,
                    })?
                ));
            }
        }
    }
}
