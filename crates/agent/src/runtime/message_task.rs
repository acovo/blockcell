use super::*;

/// Extract the first JSON object from potentially markdown-wrapped LLM output.
/// Handles ```json...```, ```...```, `<tool_call>` XML with `<parameter=argv>`,
/// bare `{...}` objects, and bare `[...]` arrays (wrapped as `{"argv":[...]}`).
#[allow(dead_code)]
pub(crate) fn extract_json_from_text(text: &str) -> String {
    // Try ```json ... ``` blocks first
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    // Try ``` ... ``` blocks containing an object or array
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.starts_with('{') || candidate.starts_with('[') {
                if candidate.starts_with('[') {
                    return format!("{{\"argv\": {}}}", candidate);
                }
                return candidate.to_string();
            }
        }
    }
    // Handle <tool_call> XML: extract argv from <parameter=argv>...</parameter>
    if text.contains("<parameter=argv>") {
        if let Some(start) = text.find("<parameter=argv>") {
            let after = &text[start + 16..];
            let end_tag = after.find("</parameter>").unwrap_or(after.len());
            let content = after[..end_tag].trim();
            if content.starts_with('[') {
                return format!("{{\"argv\": {}}}", content);
            }
            if content.starts_with('{') {
                return content.to_string();
            }
        }
    }
    // Fall back to first { ... } span
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end >= start {
                return text[start..=end].to_string();
            }
        }
    }
    // Handle bare JSON arrays (wrap as {"argv": [...]})
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            if end >= start {
                return format!("{{\"argv\": {}}}", &text[start..=end]);
            }
        }
    }
    text.trim().to_string()
}

#[allow(dead_code)]
pub(crate) fn build_script_skill_summary_prompt(
    user_question: &str,
    skill_name: &str,
    method_name: &str,
    skill_md: &str,
    script_output: &str,
) -> String {
    crate::skill_summary::SkillSummaryFormatter::build_prompt(
        user_question,
        skill_name,
        Some(method_name),
        skill_md,
        script_output,
    )
}

/// Free async function that runs a user message in the background.
/// Each message gets its own AgentRuntime so the main loop stays responsive.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_message_task(
    config: Config,
    paths: Paths,
    provider_pool: Arc<ProviderPool>,
    tool_registry: ToolRegistry,
    task_manager: TaskManager,
    outbound_tx: Option<mpsc::Sender<OutboundMessage>>,
    confirm_tx: Option<mpsc::Sender<ConfirmRequest>>,
    memory_store: Option<MemoryStoreHandle>,
    capability_registry: Option<CapabilityRegistryHandle>,
    core_evolution: Option<CoreEvolutionHandle>,
    event_tx: Option<broadcast::Sender<String>>,
    agent_id: Option<String>,
    event_emitter: EventEmitterHandle,
    steering: SteeringChannel,
    steering_sender: SteeringSender,
    msg: InboundMessage,
    task_id: String,
    abort_token: AbortToken,
) {
    // 注意：任务已通过 create_and_start_task 标记为 Running，无需再调用 set_running
    let checkpoint_manager = crate::checkpoint::CheckpointManager::new(&paths.workspace());
    let resumed_task_id = msg
        .metadata
        .get("resumed_task_id")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let completion_receipt_id = msg
        .metadata
        .get(blockcell_core::message_receipt::MESSAGE_RECEIPT_ID)
        .and_then(|value| value.as_str())
        .map(str::to_string);

    // 发送开始进度
    task_manager
        .send_progress(crate::agent_progress::AgentProgress::Delta {
            task_id: task_id.clone(),
            tokens_added: 0,
            tools_added: 0,
            total_tokens: 0,
            total_tools: 0,
        })
        .await;

    let mut runtime = match AgentRuntime::new(config, paths, provider_pool, tool_registry) {
        Ok(r) => r,
        Err(e) => {
            if let Some(receipt_id) = completion_receipt_id.as_deref() {
                blockcell_core::message_receipt::complete_message_receipt(
                    receipt_id,
                    Err(e.to_string()),
                );
            }
            task_manager.set_failed(&task_id, &format!("{}", e)).await;
            if let Some(tx) = &outbound_tx {
                let mut outbound =
                    OutboundMessage::new(&msg.channel, &msg.chat_id, &format!("❌ {}", e));
                outbound.account_id = msg.account_id.clone();
                let _ = tx.send(outbound).await;
            }
            return;
        }
    };

    // Wire up channels
    if let Some(tx) = outbound_tx.clone() {
        runtime.set_outbound(tx);
    }
    if let Some(tx) = confirm_tx {
        runtime.set_confirm(tx);
    }
    runtime.set_task_manager(task_manager.clone());
    runtime.set_agent_id(agent_id.clone());
    runtime.set_event_emitter(event_emitter);
    runtime.set_steering_channel(steering, steering_sender);
    if let Some(store) = memory_store {
        runtime.set_memory_store(store);
    }
    if let Err(e) = runtime.init_memory_file_store() {
        tracing::warn!(error = %e, "Failed to initialize file memory store");
    }
    if let Err(e) = runtime.init_skill_file_store() {
        tracing::warn!(error = %e, "Failed to initialize skill file store");
    }
    if let Some(registry) = capability_registry {
        runtime.set_capability_registry(registry);
    }
    if let Some(core_evo) = core_evolution {
        runtime.set_core_evolution(core_evo);
    }
    if let Some(tx) = event_tx.clone() {
        runtime.set_event_tx(tx);
    }
    // Set abort token from parent (enables graceful cancellation)
    runtime.set_abort_token(abort_token);

    // 初始化 runtime handle（必须在 set_abort_token 之后，确保 handle 捕获正确的 abort_token）
    runtime.init_runtime_handle();
    runtime.wire_evolution_deploy_callback();

    let error_channel = msg.channel.clone();
    let error_chat_id = msg.chat_id.clone();

    match runtime.process_message(msg).await {
        Ok(response) => {
            debug!(task_id = %task_id, response_len = response.len(), "Message task completed");
            // Mark message tasks as completed so they appear in /tasks.
            // The periodic cleanup loop will evict them after the grace period.
            // This way users can see recently completed tasks via /tasks.
            task_manager.set_completed(&task_id, &response).await;
            if let Err(e) = checkpoint_manager.mark_completed(&task_id) {
                warn!(task_id = %task_id, error = %e, "Failed to mark message checkpoint completed");
            }
            if let Some(resumed_task_id) = resumed_task_id.as_deref() {
                if let Err(e) = checkpoint_manager.mark_completed(resumed_task_id) {
                    warn!(task_id = %resumed_task_id, error = %e, "Failed to mark resumed checkpoint completed");
                }
            }
            if let Some(receipt_id) = completion_receipt_id.as_deref() {
                blockcell_core::message_receipt::complete_message_receipt(receipt_id, Ok(()));
            }
        }
        Err(e) => {
            let err_msg = format!("{}", e);
            error!(task_id = %task_id, error = %e, "Message task failed");
            if let Some(ref event_tx) = event_tx {
                let _ = event_tx.send(
                    serde_json::json!({
                        "type": "error",
                        "channel": error_channel,
                        "agent_id": agent_id.clone().unwrap_or_else(|| "default".to_string()),
                        "chat_id": error_chat_id,
                        "task_id": task_id.clone(),
                        "message": err_msg,
                    })
                    .to_string(),
                );
            }
            // Keep failed tasks briefly for visibility, then let tick cleanup handle them
            task_manager.set_failed(&task_id, &err_msg).await;
            if let Some(receipt_id) = completion_receipt_id.as_deref() {
                blockcell_core::message_receipt::complete_message_receipt(receipt_id, Err(err_msg));
            }
        }
    }
}
