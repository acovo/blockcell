use super::*;

/// Lightweight handle for the agent tool that avoids circular Arc<Self> references.
///
/// Captures only the data needed by `execute_fork_mode` and `spawn_typed_agent`,
/// so `ToolContext` can hold this without owning the full `AgentRuntime`.
pub struct LightweightRuntimeHandle {
    provider_pool: Arc<ProviderPool>,
    /// Shared reference to main_session_target (updated by AgentRuntime on each message).
    /// Using Arc<RwLock> so the handle always sees the current value,
    /// not the stale None from initialization time.
    main_session_target: Arc<std::sync::RwLock<Option<MainSessionTarget>>>,
    task_manager: TaskManager,
    _config: Config,
    paths: Paths,
    event_tx: Option<broadcast::Sender<String>>,
    outbound_tx: Option<mpsc::Sender<OutboundMessage>>,
    _system_event_emitter: EventEmitterHandle,
    abort_token: AbortToken,
    /// Cached SessionStore to avoid creating a new one per fork call
    session_store: SessionStore,
    /// Agent type registry — 共享的 agent 类型定义
    agent_type_registry: crate::agent_types::AgentTypeRegistry,
    /// Skill 互斥锁手柄 — 通过 WriteGuard 提供，传递给 forked agent
    skill_mutex: blockcell_tools::SkillMutexHandle,
    /// Memory store — 共享的记忆存储，传递给 forked agent
    memory_store: Option<MemoryStoreHandle>,
    memory_file_store: Option<blockcell_tools::MemoryFileStoreHandle>,
    skill_file_store: Option<blockcell_tools::SkillFileStoreHandle>,
}

impl LightweightRuntimeHandle {
    pub fn from_runtime(runtime: &AgentRuntime) -> Self {
        Self {
            provider_pool: runtime.provider_pool.clone(),
            main_session_target: runtime.shared_session_target.clone(),
            task_manager: runtime.task_manager.clone(),
            _config: runtime.config.clone(),
            paths: runtime.paths.clone(),
            event_tx: runtime.event_tx.clone(),
            outbound_tx: runtime.outbound_tx.clone(),
            _system_event_emitter: runtime.system_event_emitter.clone(),
            abort_token: runtime.abort_token.clone(),
            session_store: SessionStore::new(runtime.paths.clone()),
            agent_type_registry: runtime.agent_type_registry.clone(),
            skill_mutex: runtime.skill_mutex.clone(),
            memory_store: runtime.memory_store.clone(),
            memory_file_store: runtime.memory_file_store.clone(),
            skill_file_store: runtime.skill_file_store.clone(),
        }
    }

    /// Read the current main_session_target from the shared reference.
    fn get_main_session_target(&self) -> Option<MainSessionTarget> {
        self.main_session_target
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// 检查 Agent 类型是否需要 worktree 隔离
    /// 基于 AgentTypeDefinition 中的 isolation 字段判断，而非硬编码类型名
    fn requires_worktree(def: &crate::agent_types::AgentTypeDefinition) -> bool {
        def.isolation == Some(crate::agent_types::IsolationMode::Worktree)
    }

    /// Detect if the current working directory is already inside a git worktree.
    async fn is_in_worktree(workspace: &std::path::Path) -> bool {
        let git_file = workspace.join(".git");
        if !tokio::fs::try_exists(&git_file).await.unwrap_or(false) {
            return false;
        }
        if let Ok(content) = tokio::fs::read_to_string(&git_file).await {
            content.starts_with("gitdir:")
        } else {
            false
        }
    }

    /// Create a git worktree for isolated agent execution.
    async fn create_worktree(
        workspace: &std::path::Path,
        task_id: &str,
    ) -> Result<std::path::PathBuf> {
        let worktree_name = format!("agent-{}", &task_id[..16.min(task_id.len())]);
        let worktree_path = workspace
            .join(".claude")
            .join("worktrees")
            .join(&worktree_name);

        let worktree_parent = worktree_path.parent().ok_or_else(|| {
            blockcell_core::Error::Other(format!(
                "Invalid worktree path: {}",
                worktree_path.display()
            ))
        })?;
        tokio::fs::create_dir_all(worktree_parent)
            .await
            .map_err(blockcell_core::Error::Io)?;

        let output = tokio::process::Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &worktree_name,
                &worktree_path.display().to_string(),
            ])
            .current_dir(workspace)
            .output()
            .await
            .map_err(blockcell_core::Error::Io)?;

        if !output.status.success() {
            return Err(blockcell_core::Error::Other(format!(
                "Failed to create worktree: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        tracing::info!(
            "Created worktree at {} with branch {}",
            worktree_path.display(),
            worktree_name
        );
        Ok(worktree_path)
    }

    /// Clean up a git worktree after agent task completion.
    /// 检查未提交更改，避免 --force 丢失工作。
    async fn cleanup_worktree(workspace: &std::path::Path, task_id: &str) {
        let worktree_name = format!("agent-{}", &task_id[..16.min(task_id.len())]);
        let worktree_path = workspace
            .join(".claude")
            .join("worktrees")
            .join(&worktree_name);

        // 检查是否有未提交的更改
        let status_result = tokio::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&worktree_path)
            .output()
            .await;
        let has_uncommitted = status_result
            .as_ref()
            .is_ok_and(|o| o.status.success() && !o.stdout.is_empty());

        if has_uncommitted {
            tracing::warn!(
                worktree = %worktree_name,
                "Worktree has uncommitted changes, preserving it for manual review"
            );
            return;
        }

        // 安全移除：无未提交更改
        let output = tokio::process::Command::new("git")
            .args(["worktree", "remove", &worktree_path.display().to_string()])
            .current_dir(workspace)
            .output()
            .await;

        if let Ok(output) = output {
            if !output.status.success() {
                tracing::warn!("Failed to remove worktree: {}", worktree_name);
            }
        } else {
            tracing::warn!("Failed to remove worktree: {}", worktree_name);
        }

        let output = tokio::process::Command::new("git")
            .args(["branch", "-D", &worktree_name])
            .current_dir(workspace)
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                tracing::info!("Cleaned up worktree and branch {}", worktree_name);
            }
        }
    }
}

#[async_trait::async_trait]
impl blockcell_tools::RuntimeHandle for LightweightRuntimeHandle {
    async fn execute_fork_mode(&self, prompt: String) -> Result<String> {
        use crate::forked::{
            run_forked_agent, CacheSafeParams, ForkedAgentParams, SubagentOverrides,
        };
        use blockcell_core::current_abort_token;
        use blockcell_core::types::ChatMessage;
        use uuid::Uuid;

        let parent_session_id = self
            .get_main_session_target()
            .as_ref()
            .map(|t| t.session_key.clone())
            .unwrap_or_else(|| "internal:default".to_string());

        // 加载父对话历史（用于 fork 上下文继承）
        let parent_history = self
            .session_store
            .load(&parent_session_id)
            .unwrap_or_default();

        let fork_agent_id = format!(
            "fork-{}",
            Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("unknown")
        );
        let identity = AgentIdentity::fork_child(fork_agent_id.clone(), parent_session_id);

        let child_abort_token = current_abort_token().map(|t| t.child()).unwrap_or_default();

        scope_abort_token(
            child_abort_token.clone(),
            scope_agent_context(identity.clone(), async {
                info!(
                    agent_id = %identity.agent_id,
                    role = "fork-child",
                    "Executing fork mode"
                );

                let safe_prompt = AgentRuntime::sanitize_fork_prompt(&prompt);
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

                let cache_safe_params = CacheSafeParams {
                    fork_context_messages: parent_history,
                    ..CacheSafeParams::default()
                };
                let overrides = SubagentOverrides {
                    abort_token: Some(child_abort_token),
                    ..Default::default()
                };

                let fork_disallowed = crate::forked::default_read_only_fork_disallowed_tools();
                let mut builder = ForkedAgentParams::builder()
                    .provider_pool(self.provider_pool.clone())
                    .prompt_messages(fork_messages)
                    .cache_safe_params(cache_safe_params)
                    .fork_label("fork")
                    .max_turns(10)
                    .agent_type(None)
                    .disallowed_tools(fork_disallowed.clone())
                    .one_shot(true)
                    .overrides(overrides);

                // 传递 event_tx 用于转发 fork agent 进度事件到父级
                if let Some(ref tx) = self.event_tx {
                    builder = builder.event_tx(tx.clone());
                }

                // 传递 progress_tx 用于转发工具调用事件到外部渠道
                if let Some(tx) = self.task_manager.progress_tx() {
                    builder = builder.progress_tx(tx);
                }

                // 传递 skill_mutex 和 memory_store，使 fork agent 可以使用技能和记忆工具
                builder = builder.skill_mutex(self.skill_mutex.clone());
                if let Some(ref store) = self.memory_store {
                    builder = builder.memory_store(store.clone());
                }
                if let Some(ref store) = self.memory_file_store {
                    builder = builder.memory_file_store(store.clone());
                }
                if let Some(ref store) = self.skill_file_store {
                    builder = builder.skill_file_store(store.clone());
                }
                builder = builder.skills_dir(self.paths.skills_dir());

                // 构建并传递工具 schema，让 LLM 知道可以调用哪些工具
                let tool_schemas = crate::forked::build_forked_tool_schemas(&fork_disallowed);
                builder = builder.tool_schemas(tool_schemas);

                let params = builder.build().map_err(|e| {
                    blockcell_core::Error::Tool(format!("ForkedAgentParams build failed: {}", e))
                })?;

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
        )
        .await
    }

    async fn spawn_typed_agent(
        &self,
        agent_type: &str,
        prompt: String,
        description: Option<String>,
        workspace_scope: Vec<String>,
    ) -> Result<String> {
        use crate::forked::{
            run_forked_agent, CacheSafeParams, ForkedAgentParams, SubagentOverrides,
        };
        use blockcell_core::types::ChatMessage;
        use uuid::Uuid;

        // 使用共享 registry（保留自定义类型）
        let def = self.agent_type_registry.get(agent_type).ok_or_else(|| {
            blockcell_core::Error::Tool(format!("Unknown agent type: {}", agent_type))
        })?;

        let task_id = format!(
            "task-{}",
            Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("unknown")
        );
        let workspace = self.paths.workspace();
        let workspace_scope_entries = resolve_typed_workspace_scope(&workspace, &workspace_scope);
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

        let parent_session_id = self
            .get_main_session_target()
            .as_ref()
            .map(|t| t.session_key.clone())
            .unwrap_or_else(|| "internal:default".to_string());

        let identity =
            AgentIdentity::typed(task_id.clone(), agent_type.to_string(), parent_session_id);

        let (channel, chat_id) = self
            .get_main_session_target()
            .as_ref()
            .map(|t| (t.channel.clone(), t.chat_id.clone()))
            .unwrap_or_else(|| ("internal".to_string(), "default".to_string()));

        let parent_agent_id = self
            .get_main_session_target()
            .as_ref()
            .and_then(|t| t.agent_id.clone());

        // 原子性地创建并标记为 Running（消除竞态条件）
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

        self.task_manager
            .send_progress(crate::agent_progress::AgentProgress::Delta {
                task_id: task_id.clone(),
                tokens_added: 0,
                tools_added: 0,
                total_tokens: 0,
                total_tools: 0,
            })
            .await;

        let provider_pool = self.provider_pool.clone();
        let task_manager = self.task_manager.clone();
        let event_tx = self.event_tx.clone();
        let outbound_tx = self.outbound_tx.clone();
        let system_prompt = def.system_prompt_template.clone();
        let disallowed_tools = def.disallowed_tools.clone();
        let max_turns = def.max_turns;
        let one_shot = def.one_shot;
        let tools = def.tools.clone();
        let model = def.model.clone();
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
        let agent_type_for_log = agent_type.to_string();
        let agent_type_for_label = agent_type.to_string();
        // session_key 用于持久化子agent结果到 SessionStore
        let session_key_for_persist = self
            .get_main_session_target()
            .as_ref()
            .map(|t| t.session_key.clone());
        let paths_for_persist = self.paths.clone();
        // Create child AbortToken for chain cancellation
        let child_abort_token = self.abort_token.child();
        self.task_manager
            .register_abort_token(&task_id, child_abort_token.clone());
        // Clone skill_mutex and memory_store for the spawned agent
        let skill_mutex_for_spawn = self.skill_mutex.clone();
        let memory_store_for_spawn = self.memory_store.clone();
        let memory_file_store_for_spawn = self.memory_file_store.clone();
        let skill_file_store_for_spawn = self.skill_file_store.clone();
        let skills_dir_for_spawn = self.paths.skills_dir();

        // Worktree isolation support
        // 检查是否需要 worktree 隔离（基于 AgentTypeDefinition.isolation 配置）
        let needs_worktree = Self::requires_worktree(def);
        let already_in_worktree = Self::is_in_worktree(&workspace).await;
        let worktree_path = if needs_worktree && !already_in_worktree {
            match Self::create_worktree(&workspace, &task_id).await {
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

        // 克隆给 guard task（用于 panic 时的 worktree 清理）
        let guard_worktree_path = worktree_path.clone();
        let guard_workspace = workspace.clone();
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

                    let messages = vec![
                        ChatMessage::system(system_prompt.as_deref().unwrap_or(
                            "You are a specialized agent. Execute the task efficiently.",
                        )),
                        ChatMessage::user(&prompt_clone),
                    ];

                    let cache_safe_params = CacheSafeParams::default();

                    // Build SubagentOverrides with AbortToken from context
                    let overrides = SubagentOverrides {
                        abort_token: blockcell_core::current_abort_token(),
                        working_dir: worktree_path.clone(),
                        ..Default::default()
                    };

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
                        .overrides(overrides)
                        .tools(tools)
                        .model(model)
                        .skills(skills)
                        .mcp_servers(mcp_servers)
                        .initial_prompt(initial_prompt)
                        .background(background)
                        .color(color);

                    if let Some(turns) = max_turns {
                        builder = builder.max_turns(turns);
                    }

                    // 设置 event_tx 用于转发子agent进度事件到父级
                    if let Some(ref tx) = event_tx {
                        builder = builder.event_tx(tx.clone());
                    }

                    // 设置 progress_tx 用于转发工具调用事件到外部渠道
                    if let Some(tx) = task_manager.progress_tx() {
                        builder = builder.progress_tx(tx);
                    }

                    // 传递 skill_mutex 和 memory_store，使 typed agent 可以使用技能和记忆工具
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

                    // 传递工具 schema，让 LLM 知道可以调用哪些工具
                    builder = builder.tool_schemas(tool_schemas);

                    match builder.build() {
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
                        let session_store = SessionStore::new(paths_for_persist.clone());
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
                        let session_store = SessionStore::new(paths_for_persist.clone());
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

            // Cleanup worktree if created
            if worktree_path.is_some() {
                Self::cleanup_worktree(&workspace, &task_id_clone).await;
            }
        });

        // Guard: if tokio::spawn fails (runtime shutdown) or task panics,
        // mark the task as Failed to prevent it from being stuck in Running state.
        // Also clean up worktree on panic since the main spawn closure's cleanup code won't run.
        let guard_task_manager = self.task_manager.clone();
        let guard_task_id = task_id.clone();
        tokio::spawn(async move {
            if let Err(e) = join_handle.await {
                // Only mark as Failed if the task panicked.
                // Cancellation (e.g. abort token) is intentional and should not
                // overwrite the already-set Cancelled state.
                if e.is_panic() {
                    warn!(task_id = %guard_task_id, error = %e, "Typed agent task panicked");
                    guard_task_manager
                        .set_failed(&guard_task_id, &format!("Task panicked: {}", e))
                        .await;

                    // 清理 worktree（主 spawn 的清理代码因 panic 不会执行）
                    if guard_worktree_path.is_some() {
                        Self::cleanup_worktree(&guard_workspace, &guard_task_id).await;
                    }
                } else {
                    warn!(task_id = %guard_task_id, "Typed agent task was cancelled (not a panic)");
                }
            }
        });

        Ok(task_id)
    }
}
