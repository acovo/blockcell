use super::*;

pub(crate) fn resolve_typed_workspace_scope(
    workspace: &std::path::Path,
    requested: &[String],
) -> Vec<crate::forked::WorkspaceScopeEntry> {
    requested
        .iter()
        .map(|raw| {
            let path = workspace.join(raw.trim_end_matches(['/', '\\']));
            if raw.ends_with(['/', '\\']) || path.is_dir() {
                crate::forked::WorkspaceScopeEntry::directory(path)
            } else {
                crate::forked::WorkspaceScopeEntry::file(path)
            }
        })
        .collect()
}

pub(crate) fn normalize_typed_agent_result(agent_type: &str, content: &str) -> String {
    const MAX_SUMMARY_CHARS: usize = 2_000;
    let trimmed = content.trim();
    let json_text = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    let parsed = serde_json::from_str::<serde_json::Value>(json_text).ok();
    let files_changed = parsed
        .as_ref()
        .and_then(|value| value.get("files_changed"))
        .filter(|value| value.is_array())
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let tests_run = parsed
        .as_ref()
        .and_then(|value| value.get("tests_run"))
        .filter(|value| value.is_array())
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let mut summary = parsed
        .as_ref()
        .and_then(|value| value.get("summary"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(trimmed)
        .to_string();
    if let Some(findings) = parsed
        .as_ref()
        .and_then(|value| value.get("findings"))
        .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
    {
        summary.push_str("\nfindings: ");
        summary.push_str(&findings.to_string());
    }
    if summary.chars().count() > MAX_SUMMARY_CHARS {
        summary = format!(
            "{}... [truncated typed-agent summary]",
            summary.chars().take(MAX_SUMMARY_CHARS).collect::<String>()
        );
    }
    let status = parsed
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("completed");

    serde_json::json!({
        "files_changed": files_changed,
        "summary": summary,
        "tests_run": tests_run,
        "status": status,
        "agent_type": agent_type,
    })
    .to_string()
}

// Additional AgentRuntime methods for typed agent support
impl AgentRuntime {
    /// Fork 模式执行（省略 subagent_type 触发）
    ///
    /// Sanitize prompt for fork_directive to prevent injection attacks.
    /// Truncates to max length and strips control characters that could
    /// break the directive format.
    pub(crate) fn sanitize_fork_prompt(prompt: &str) -> String {
        const MAX_FORK_PROMPT_LEN: usize = 4000;
        let sanitized: String = prompt
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .take(MAX_FORK_PROMPT_LEN)
            .collect::<String>()
            // 防止闭合标签注入：替换 </fork_directive> 避免提前终止指令
            .replace("</fork_directive>", "<\\/fork_directive>");
        if prompt.len() > MAX_FORK_PROMPT_LEN {
            format!("{}[...truncated]", sanitized)
        } else {
            sanitized
        }
    }

    /// 直接使用当前 Agent 的工具集执行一个轻量级的子任务，
    /// 不会触发 agent_type 路由。
    ///
    /// # Arguments
    /// * `prompt` - 任务描述/提示词
    ///
    /// # Returns
    /// * `Result<String>` - 执行结果字符串
    pub async fn execute_fork_mode(&self, prompt: String) -> Result<String> {
        use crate::forked::{
            run_forked_agent, CacheSafeParams, ForkedAgentParams, SubagentOverrides,
        };
        use blockcell_core::current_abort_token;
        use blockcell_core::types::ChatMessage;
        use uuid::Uuid;

        // 获取 parent session_id
        let parent_session_id = self
            .main_session_target
            .as_ref()
            .map(|t| t.session_key.clone())
            .unwrap_or_else(|| "internal:default".to_string());
        // 加载父对话历史（用于 fork 上下文继承）
        let parent_history = self
            .session_store
            .load(&parent_session_id)
            .unwrap_or_default();

        // 创建 ForkChild identity
        let fork_agent_id = format!(
            "fork-{}",
            Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("unknown")
        );
        let identity = AgentIdentity::fork_child(fork_agent_id.clone(), parent_session_id);

        // 获取当前 AbortToken 并创建 child token（用于链式取消）
        let child_abort_token = current_abort_token().map(|t| t.child()).unwrap_or_default();
        let abort_token_for_scope = child_abort_token.clone();

        // 在 ForkChild 上下文中执行（同时作用域化 AbortToken 和 AgentContext，
        // 确保 current_abort_token() 在子代理内部返回正确的 child token）
        scope_abort_token(
            abort_token_for_scope,
            scope_agent_context(identity.clone(), async {
                info!(
                    agent_id = %identity.agent_id,
                    role = "fork-child",
                    "Executing fork mode"
                );

                // 构建 Fork 消息
                let safe_prompt = Self::sanitize_fork_prompt(&prompt);
                let fork_messages = vec![
                    ChatMessage::system(
                        "You are a forked agent. Execute directly without spawning subagents.",
                    ),
                    ChatMessage::user(&format!(
                        "<fork_directive>\n\
                    RULES:\n\
                    1. Do NOT spawn sub-agents; execute directly.\n\
                    2. Do NOT converse; execute and report results.\n\
                    3. USE tools: Read, Grep, Glob, Bash (read-only).\n\
                    4. Keep report under 500 words.\n\
                    \n\
                    Task: {}",
                        safe_prompt
                    )),
                ];

                // 构建缓存安全参数，填入父对话历史以继承上下文
                let cache_safe_params = CacheSafeParams {
                    fork_context_messages: parent_history,
                    ..CacheSafeParams::default()
                };

                // 构建 SubagentOverrides，传递 AbortToken
                let overrides = SubagentOverrides {
                    abort_token: Some(child_abort_token),
                    ..Default::default()
                };

                let fork_disallowed = crate::forked::default_read_only_fork_disallowed_tools();

                // 构建 ForkedAgentParams（使用 builder 模式）
                let params = ForkedAgentParams::builder()
                    .provider_pool(self.provider_pool.clone())
                    .prompt_messages(fork_messages)
                    .cache_safe_params(cache_safe_params)
                    .fork_label("fork")
                    .max_turns(10)
                    .agent_type(None)
                    .disallowed_tools(fork_disallowed)
                    .one_shot(true)
                    .overrides(overrides)
                    .build()
                    .map_err(|e| {
                        blockcell_core::Error::Tool(format!(
                            "ForkedAgentParams build failed: {}",
                            e
                        ))
                    })?;

                // 执行 Fork Agent
                let result = run_forked_agent(params)
                    .await
                    .map_err(|e| blockcell_core::Error::Tool(format!("Fork failed: {}", e)))?;

                // 如果工具结果被截断，追加提示让用户知道
                let mut content = result
                    .final_content
                    .unwrap_or_else(|| "Fork completed with no output".to_string());
                if result.truncated {
                    tracing::warn!("[fork] 工具结果被截断，可能丢失部分信息");
                    content.push_str("\n\n[注意: 结果因长度限制被截断，可能丢失部分信息]");
                }
                Ok(content)
            }),
        ) // scope_agent_context + scope_abort_token
        .await
    }

    /// 启动类型化 Agent
    ///
    /// 基于 AgentTypeDefinition 启动一个专业化 Agent，
    /// 具有独立的工具集、权限模型和提示词模板。
    ///
    /// # Arguments
    /// * `agent_type` - Agent 类型标识符（如 "explore", "plan", "viper"）
    /// * `prompt` - 任务描述/提示词
    /// * `description` - 可选的任务描述（用于日志和状态显示）
    ///
    /// # Returns
    /// * `Result<String>` - task_id 字符串
    pub async fn spawn_typed_agent(
        &self,
        agent_type: &str,
        prompt: String,
        description: Option<String>,
        workspace_scope: Vec<String>,
    ) -> Result<String> {
        use crate::forked::{run_forked_agent, CacheSafeParams, ForkedAgentParams};
        use blockcell_core::types::ChatMessage;
        use uuid::Uuid;

        // 获取 Agent 类型定义（使用共享 registry，保留自定义类型）
        let def = self.agent_type_registry.get(agent_type).ok_or_else(|| {
            blockcell_core::Error::Tool(format!("Unknown agent type: {}", agent_type))
        })?;

        // 生成 task_id（作为 agent_id）
        let task_id = format!(
            "task-{}",
            Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("unknown")
        );

        let workspace_scope_entries =
            resolve_typed_workspace_scope(&self.paths.workspace(), &workspace_scope);
        if agent_type == "coder" {
            if workspace_scope_entries.is_empty() {
                return Err(blockcell_core::Error::Validation(
                    "coder agent requires a non-empty workspace_scope".to_string(),
                ));
            }
            let paths = workspace_scope_entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            self.task_manager
                .try_acquire_workspace_scope(&task_id, &paths)?;
        }

        // 获取 parent session_id
        let parent_session_id = self
            .main_session_target
            .as_ref()
            .map(|t| t.session_key.clone())
            .unwrap_or_else(|| "internal:default".to_string());
        let parent_system_prompt = self
            .context_builder
            .system_prompt_snapshot_reader()
            .get(&parent_session_id)
            .unwrap_or_default();

        // 创建 Typed identity（用于后续执行时设置上下文）
        let identity =
            AgentIdentity::typed(task_id.clone(), agent_type.to_string(), parent_session_id);

        info!(
            agent_id = %identity.agent_id,
            agent_type = agent_type,
            isolation = ?def.isolation,
            "Preparing typed agent spawn"
        );

        // 获取 channel 和 chat_id（从 main_session_target 或使用默认值）
        let (channel, chat_id) = self
            .main_session_target
            .as_ref()
            .map(|t| (t.channel.clone(), t.chat_id.clone()))
            .unwrap_or_else(|| ("internal".to_string(), "default".to_string()));

        // 获取父 agent_id，用于子agent结果送达时匹配 WebUI 的 selectedAgentId
        let parent_agent_id = self
            .main_session_target
            .as_ref()
            .and_then(|t| t.agent_id.clone());

        // 注册任务并原子性地标记为 Running（消除竞态条件）
        self.task_manager
            .create_and_start_task(
                &task_id,
                description.as_deref().unwrap_or(agent_type),
                &prompt,
                &channel,
                &chat_id,
                Some(&task_id),
                false,
                Some(agent_type),
                def.one_shot,
            )
            .await;

        // 发送开始进度
        self.task_manager
            .send_progress(crate::agent_progress::AgentProgress::Delta {
                task_id: task_id.clone(),
                tokens_added: 0,
                tools_added: 0,
                total_tokens: 0,
                total_tools: 0,
            })
            .await;

        // 克隆必需的运行时资源
        let _config = self.config.clone();
        let paths = self.paths.clone();
        let provider_pool = self.provider_pool.clone();
        let task_manager = self.task_manager.clone();
        let event_tx = self.event_tx.clone();
        let outbound_tx = self.outbound_tx.clone();
        let _system_event_emitter = self.system_event_emitter.clone();
        let system_prompt = def.system_prompt_template.clone();
        let disallowed_tools = def.disallowed_tools.clone();
        let max_turns = def.max_turns;
        let one_shot = def.one_shot;
        let tools = def.tools.clone();
        let model = def.model.clone();
        let cache_model = model.clone().unwrap_or_default();
        let skills = def.skills.clone();
        let mcp_servers = def.mcp_servers.clone();
        let initial_prompt = def.initial_prompt.clone();
        let background = def.background;
        let color = def.color.clone();
        let prompt_clone = if workspace_scope.is_empty() {
            prompt.clone()
        } else {
            format!(
                "{}\n\nAssigned workspace_scope (all writes must stay inside it):\n- {}",
                prompt,
                workspace_scope.join("\n- ")
            )
        };
        let identity_clone = identity.clone();
        let task_id_clone = task_id.clone();
        let agent_type_str = agent_type.to_string();
        let agent_type_for_log = agent_type_str.clone();
        let agent_type_for_label = agent_type_str.clone();
        // session_key 用于持久化子agent结果到 SessionStore
        let session_key_for_persist = self
            .main_session_target
            .as_ref()
            .map(|t| t.session_key.clone());
        // Create child AbortToken for chain cancellation
        let child_abort_token = self.abort_token.child();
        self.task_manager
            .register_abort_token(&task_id, child_abort_token.clone());

        // 检查是否需要 worktree 隔离（基于 AgentTypeDefinition.isolation 配置）
        let needs_worktree = self.requires_worktree(def);
        // 检查是否已在 worktree 中（避免嵌套 worktree）
        let already_in_worktree = self.is_in_worktree().await;
        let worktree_path = if needs_worktree && !already_in_worktree {
            match self.create_worktree(&task_id).await {
                Ok(path) => {
                    info!(task_id = %task_id, worktree = %path.display(), "Created worktree for typed agent");
                    Some(path)
                }
                Err(e) => {
                    warn!(task_id = %task_id, error = %e, "Failed to create worktree, proceeding in current directory");
                    None
                }
            }
        } else if needs_worktree && already_in_worktree {
            warn!(task_id = %task_id, "Already in worktree, skipping nested worktree creation");
            None
        } else {
            None
        };

        // Clone skill_mutex and memory_store for the spawned agent
        let skill_mutex_for_spawn = self.skill_mutex.clone();
        let memory_store_for_spawn = self.memory_store.clone();
        let memory_file_store_for_spawn = self.memory_file_store.clone();
        let skill_file_store_for_spawn = self.skill_file_store.clone();
        let skills_dir_for_spawn = self.paths.skills_dir();

        // 启动后台执行
        // 克隆 worktree_path 和 paths 给 guard task（用于 panic 时的 worktree 清理）
        let guard_worktree_path = worktree_path.clone();
        let guard_paths = paths.clone();
        let join_handle = tokio::spawn(async move {
            // Wrap in both AbortToken and AgentIdentity context for chain cancellation
            let result = scope_abort_token(
                child_abort_token,
                scope_agent_context(identity_clone.clone(), async {
                    info!(
                        agent_id = %identity_clone.agent_id,
                        agent_type = agent_type_for_log,
                        "Executing typed agent in background"
                    );

                    // 构建消息
                    let messages = vec![
                        ChatMessage::system(system_prompt.as_deref().unwrap_or(
                            "You are a specialized agent. Execute the task efficiently.",
                        )),
                        ChatMessage::user(&prompt_clone),
                    ];

                    // 构建缓存安全参数
                    let cache_safe_params = CacheSafeParams::new(parent_system_prompt, cache_model);

                    // 构建 ForkedAgentParams
                    // 使用 "typed" 作为静态 fork_label，实际的 agent_type 通过 agent_type() 方法设置
                    // 构建工具 schema（在 disallowed_tools 被 move 之前）
                    let tool_schemas = crate::forked::build_forked_tool_schemas(&disallowed_tools);

                    let mut builder = ForkedAgentParams::builder()
                        .provider_pool(provider_pool)
                        .prompt_messages(messages)
                        .cache_safe_params(cache_safe_params)
                        .fork_label("typed")
                        .agent_type(Some(agent_type_for_label))
                        .task_id(Some(task_id_clone.clone()))
                        .workspace_scope(workspace_scope_entries)
                        .disallowed_tools(disallowed_tools)
                        .one_shot(one_shot)
                        .tool_schemas(tool_schemas)
                        .tools(tools)
                        .model(model)
                        .skills(skills)
                        .mcp_servers(mcp_servers)
                        .initial_prompt(initial_prompt)
                        .background(background)
                        .color(color);

                    // 只有在有值时才设置 max_turns
                    if let Some(turns) = max_turns {
                        builder = builder.max_turns(turns);
                    }

                    // 设置工作目录（如果创建了 worktree）
                    if let Some(ref wt_path) = worktree_path {
                        builder = builder.working_dir(wt_path.clone());
                    }

                    // 设置 event_tx 用于转发子agent进度事件到父级
                    if let Some(ref tx) = event_tx {
                        builder = builder.event_tx(tx.clone());
                    }

                    // Pass skill_mutex and memory_store so typed agent can use skill and memory tools
                    builder = builder.skill_mutex(skill_mutex_for_spawn);
                    if let Some(store) = memory_store_for_spawn {
                        builder = builder.memory_store(store);
                    }
                    if let Some(store) = memory_file_store_for_spawn {
                        builder = builder.memory_file_store(store);
                    }
                    if let Some(store) = skill_file_store_for_spawn {
                        builder = builder.skill_file_store(store);
                    }
                    builder = builder.skills_dir(skills_dir_for_spawn);

                    let params = builder.build();

                    match params {
                        Ok(p) => run_forked_agent(p).await.map_err(|e| {
                            blockcell_core::Error::Tool(format!("Forked agent error: {}", e))
                        }),
                        Err(e) => Err(blockcell_core::Error::Tool(format!(
                            "ForkedAgentParams build failed: {}",
                            e
                        ))),
                    }
                }),
            )
            .await;

            // 处理执行结果
            match result {
                Ok(output) => {
                    let content = output
                        .final_content
                        .unwrap_or_else(|| "Task completed with no output".to_string());
                    let content = normalize_typed_agent_result(&agent_type_for_log, &content);
                    let was_cancelled = task_manager
                        .get_task(&task_id_clone)
                        .await
                        .is_some_and(|task| task.status == TaskStatus::Cancelled);
                    if was_cancelled {
                        info!(task_id = %task_id_clone, "Typed agent finished after cancellation; suppressing result delivery");
                        task_manager.unregister_abort_token(&task_id_clone);
                    } else {
                        task_manager.set_completed(&task_id_clone, &content).await;
                        info!(task_id = %task_id_clone, "Typed agent completed successfully");

                        // 将结果发送到 origin channel/chat_id，让用户看到输出
                        let session_store = SessionStore::new(paths.clone());
                        deliver_subagent_result_to_origin(
                            &channel,
                            &chat_id,
                            &content,
                            &task_id_clone,
                            parent_agent_id.as_deref(),
                            outbound_tx.clone(),
                            event_tx.clone(),
                            Some(&session_store),
                            session_key_for_persist.as_deref(),
                        )
                        .await;
                    }
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    let was_cancelled = task_manager
                        .get_task(&task_id_clone)
                        .await
                        .is_some_and(|task| task.status == TaskStatus::Cancelled);
                    if was_cancelled {
                        info!(task_id = %task_id_clone, "Typed agent stopped after cancellation");
                        task_manager.unregister_abort_token(&task_id_clone);
                    } else {
                        task_manager.set_failed(&task_id_clone, &err_msg).await;
                        warn!(task_id = %task_id_clone, error = %e, "Typed agent failed");

                        // 将失败信息也发送到 origin
                        let short_id = truncate_str(&task_id_clone, 8);
                        let failure_message = format!(
                            "\n❌ 后台任务失败: **{}** (ID: {})\n错误: {}",
                            agent_type_for_log, short_id, err_msg
                        );
                        let session_store = SessionStore::new(paths.clone());
                        deliver_subagent_result_to_origin(
                            &channel,
                            &chat_id,
                            &failure_message,
                            &task_id_clone,
                            parent_agent_id.as_deref(),
                            outbound_tx.clone(),
                            event_tx.clone(),
                            Some(&session_store),
                            session_key_for_persist.as_deref(),
                        )
                        .await;
                    }
                }
            }

            // 清理 worktree（如果创建了）
            // 注意：cleanup_worktree 需要在 AgentRuntime 上调用，这里我们直接使用 git 命令
            if let Some(ref wt_path) = worktree_path {
                let worktree_name =
                    format!("agent-{}", &task_id_clone[..16.min(task_id_clone.len())]);

                // Check for uncommitted changes before force-removing
                let status_result = tokio::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(wt_path)
                    .output()
                    .await;
                let has_uncommitted = status_result
                    .as_ref()
                    .is_ok_and(|o| o.status.success() && !o.stdout.is_empty());

                if has_uncommitted {
                    warn!(
                        worktree = %worktree_name,
                        "Worktree has uncommitted changes, preserving it for manual review"
                    );
                    // Don't remove worktree or branch — user may want to recover changes
                } else {
                    // Safe to remove: no uncommitted changes
                    let remove_result = tokio::process::Command::new("git")
                        .args(["worktree", "remove", &wt_path.display().to_string()])
                        .current_dir(paths.workspace())
                        .output()
                        .await;
                    if let Ok(output) = remove_result {
                        if !output.status.success() {
                            warn!(worktree = %worktree_name, "Failed to remove worktree");
                        }
                    } else {
                        warn!(worktree = %worktree_name, "Failed to remove worktree");
                    }
                    let branch_result = tokio::process::Command::new("git")
                        .args(["branch", "-D", &worktree_name])
                        .current_dir(paths.workspace())
                        .output()
                        .await;
                    if let Ok(output) = branch_result {
                        if output.status.success() {
                            info!(worktree = %worktree_name, "Cleaned up worktree and branch");
                        }
                    }
                }
            }
        });

        // Guard: if tokio::spawn fails (runtime shutdown) or task panics,
        // mark the task as Failed to prevent it from being stuck in Running state.
        // Also clean up worktree on panic since the main spawn closure's cleanup code won't run.
        let guard_task_manager = self.task_manager.clone();
        let guard_task_id = task_id.clone();
        tokio::spawn(async move {
            if let Err(e) = join_handle.await {
                if e.is_panic() {
                    warn!(task_id = %guard_task_id, "Typed agent task panicked");
                    guard_task_manager
                        .set_failed(&guard_task_id, "Agent task panicked")
                        .await;

                    // 清理 worktree（主 spawn 的清理代码因 panic 不会执行）
                    if let Some(ref wt_path) = guard_worktree_path {
                        let wt_name =
                            format!("agent-{}", &guard_task_id[..16.min(guard_task_id.len())]);
                        let status_result = tokio::process::Command::new("git")
                            .args(["status", "--porcelain"])
                            .current_dir(wt_path)
                            .output()
                            .await;
                        let has_uncommitted = status_result
                            .as_ref()
                            .is_ok_and(|o| o.status.success() && !o.stdout.is_empty());
                        if has_uncommitted {
                            warn!(worktree = %wt_name, "Worktree has uncommitted changes (panic), preserving for manual review");
                        } else {
                            let _ = tokio::process::Command::new("git")
                                .args(["worktree", "remove", &wt_path.display().to_string()])
                                .current_dir(guard_paths.workspace())
                                .output()
                                .await;
                            let _ = tokio::process::Command::new("git")
                                .args(["branch", "-D", &wt_name])
                                .current_dir(guard_paths.workspace())
                                .output()
                                .await;
                        }
                    }
                } else {
                    // Cancelled/aborted — this is normal (e.g. /tasks cancel), don't mark as failed
                    warn!(task_id = %guard_task_id, "Typed agent task was cancelled/aborted");
                }
            }
        });

        Ok(task_id)
    }
}
