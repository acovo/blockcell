use super::*;

fn temp_test_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "blockcell-{}-{}",
        label,
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

#[test]
fn test_dream_state_default() {
    let state = DreamState::default();
    assert!(state.last_consolidation_time.is_none());
    assert_eq!(state.current_session_count, 0);
    assert!(!state.is_consolidating);
    assert!(state.consolidating_started_at.is_none());
}

#[test]
fn test_dream_state_increment() {
    let mut state = DreamState::default();
    state.increment_session_count();
    assert_eq!(state.current_session_count, 1);
}

#[tokio::test]
async fn test_dream_prompt_prefers_relative_memory_paths() {
    let root = temp_test_dir("dream-relative-prompt");
    let memory_dir = root.join(".dream_staging").join("run").join("memory");
    let consolidator = DreamConsolidator::new(&root).await.unwrap();

    let prompt = consolidator.build_consolidation_prompt(&memory_dir, &[]);

    assert!(prompt.contains("当前工作目录就是记忆目录"));
    assert!(prompt.contains("相对路径"));
    assert!(prompt.contains("不要加 `memory/` 前缀"));
    assert!(prompt.contains("grep: path=\"reference.md\""));
    assert!(prompt.contains("glob: path=\".\""));
    assert!(prompt.contains("只能读取 list_dir/glob 已确认存在的文件"));
    assert!(prompt.contains("不要猜测 memory.md"));
    assert!(!prompt.contains("grep/glob: path=\".\""));
    assert!(prompt.contains("list_dir"));
    assert!(prompt.contains("read_file"));
    assert!(!prompt.contains(".dream_staging"));

    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[test]
fn test_dream_forked_agent_turn_budget_allows_recovery_from_exploration() {
    const { assert!(DREAM_FORKED_AGENT_MAX_TURNS >= 16) };
}

#[test]
fn test_noop_dream_advances_session_cursor_without_incrementing_count() {
    let mut state = DreamState {
        last_consolidation_time: Some(1),
        last_session_count: 5,
        current_session_count: 10,
        consolidation_count: 3,
        is_consolidating: false,
        consolidating_started_at: None,
    };
    let stats = DreamStats::default();

    apply_successful_dream_state(&mut state, &stats, 10, 99);

    assert_eq!(state.last_consolidation_time, Some(99));
    assert_eq!(state.last_session_count, 10);
    assert_eq!(state.consolidation_count, 3);
}

#[test]
fn test_changed_dream_advances_session_cursor_and_count() {
    let mut state = DreamState {
        last_consolidation_time: Some(1),
        last_session_count: 5,
        current_session_count: 10,
        consolidation_count: 3,
        is_consolidating: false,
        consolidating_started_at: None,
    };
    let stats = DreamStats {
        memories_updated: 1,
        ..DreamStats::default()
    };

    apply_successful_dream_state(&mut state, &stats, 10, 99);

    assert_eq!(state.last_consolidation_time, Some(99));
    assert_eq!(state.last_session_count, 10);
    assert_eq!(state.consolidation_count, 4);
}

#[test]
fn test_truncated_forked_agent_result_is_consolidation_failure() {
    let agent_result = blockcell_agent::forked::ForkedAgentResult {
        messages: vec![],
        total_usage: blockcell_core::UsageMetrics::default(),
        files_modified: vec![],
        final_content: Some("still working".to_string()),
        truncated: true,
        had_tool_error: false,
    };

    let err = validate_dream_agent_result(&agent_result).expect_err("truncated must fail");

    assert!(err.contains("truncated"));
}

#[test]
fn test_completed_forked_agent_result_is_consolidation_success() {
    let agent_result = blockcell_agent::forked::ForkedAgentResult {
        messages: vec![],
        total_usage: blockcell_core::UsageMetrics::default(),
        files_modified: vec![],
        final_content: Some("done".to_string()),
        truncated: false,
        had_tool_error: false,
    };

    assert!(validate_dream_agent_result(&agent_result).is_ok());
}

#[tokio::test]
async fn test_dream_release_does_not_delete_replaced_lock() {
    let root = temp_test_dir("dream-lock-owner");
    let consolidator = DreamConsolidator::new(&root).await.unwrap();
    consolidator
        .acquire_lock()
        .await
        .expect("acquire dream lock");
    let lock_path = root.join(LOCK_FILE_NAME);
    tokio::fs::write(&lock_path, format!("{}:{}", std::process::id(), u64::MAX))
        .await
        .expect("replace lock ownership");

    consolidator.release_lock().await.expect("release old lock");

    assert!(
        lock_path.exists(),
        "an old owner must not delete a replacement lock"
    );
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test]
async fn test_dream_staging_commit_applies_updates_atomically() {
    let root = temp_test_dir("dream-staging-commit");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("project.md"), "old")
        .await
        .unwrap();
    tokio::fs::write(staging.join("project.md"), "new")
        .await
        .unwrap();
    tokio::fs::write(staging.join("new.md"), "created")
        .await
        .unwrap();

    let pre = snapshot_memory_tree(&real).await.unwrap();
    let stats = commit_staged_memory(&real, &pre, &staging).await.unwrap();

    assert_eq!(
        tokio::fs::read_to_string(real.join("project.md"))
            .await
            .unwrap(),
        "new"
    );
    assert_eq!(
        tokio::fs::read_to_string(real.join("new.md"))
            .await
            .unwrap(),
        "created"
    );
    assert_eq!(stats.memories_updated, 1);
    assert_eq!(stats.memories_created, 1);
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_staging_commit_rejects_conflicting_real_file() {
    let root = temp_test_dir("dream-staging-conflict");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("project.md"), "old")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();
    tokio::fs::write(real.join("project.md"), "concurrent")
        .await
        .unwrap();
    tokio::fs::write(staging.join("project.md"), "new")
        .await
        .unwrap();

    let err = commit_staged_memory(&real, &pre, &staging)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("changed during dream staging"));
    assert_eq!(
        tokio::fs::read_to_string(real.join("project.md"))
            .await
            .unwrap(),
        "concurrent"
    );
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_staging_commit_rejects_non_markdown_change() {
    let root = temp_test_dir("dream-staging-non-md");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("project.md"), "old")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();
    tokio::fs::write(staging.join("project.md"), "old")
        .await
        .unwrap();
    tokio::fs::write(staging.join("notes.json"), "{}")
        .await
        .unwrap();

    let err = commit_staged_memory(&real, &pre, &staging)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("unsupported file"));
    assert!(!real.join("notes.json").exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_staging_commit_rolls_back_written_files_when_later_write_fails() {
    let root = temp_test_dir("dream-staging-rollback");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("a.md"), "old-a").await.unwrap();
    tokio::fs::write(real.join("b.md"), "old-b").await.unwrap();
    tokio::fs::write(staging.join("a.md"), "new-a")
        .await
        .unwrap();
    tokio::fs::write(staging.join("b.md"), "new-b")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();

    let err = commit_staged_memory_with_injected_write_failure_for_test(
        &real,
        &pre,
        &staging,
        Path::new("b.md"),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("injected write failure"));
    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "old-a"
    );
    assert_eq!(
        tokio::fs::read_to_string(real.join("b.md")).await.unwrap(),
        "old-b"
    );
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_staging_commit_removes_created_files_when_later_write_fails() {
    let root = temp_test_dir("dream-staging-rollback-created");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("b.md"), "old-b").await.unwrap();
    tokio::fs::write(staging.join("a-new.md"), "created")
        .await
        .unwrap();
    tokio::fs::write(staging.join("b.md"), "new-b")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();

    let err = commit_staged_memory_with_injected_write_failure_for_test(
        &real,
        &pre,
        &staging,
        Path::new("b.md"),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("injected write failure"));
    assert!(!real.join("a-new.md").exists());
    assert_eq!(
        tokio::fs::read_to_string(real.join("b.md")).await.unwrap(),
        "old-b"
    );
    assert!(!root.join(".dream_commit_backup").exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_staging_commit_can_roll_back_after_successful_write() {
    let root = temp_test_dir("dream-staging-post-commit-rollback");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("a.md"), "old-a").await.unwrap();
    tokio::fs::write(real.join("b.md"), "old-b").await.unwrap();
    tokio::fs::write(staging.join("a.md"), "new-a")
        .await
        .unwrap();
    tokio::fs::write(staging.join("b.md"), "old-b")
        .await
        .unwrap();
    tokio::fs::write(staging.join("b-new.md"), "created")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();

    let commit = commit_staged_memory_transaction(&real, &pre, &staging)
        .await
        .unwrap();

    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "new-a"
    );
    assert_eq!(
        tokio::fs::read_to_string(real.join("b-new.md"))
            .await
            .unwrap(),
        "created"
    );

    commit.rollback().await.unwrap();

    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "old-a"
    );
    assert_eq!(
        tokio::fs::read_to_string(real.join("b.md")).await.unwrap(),
        "old-b"
    );
    assert!(!real.join("b-new.md").exists());
    assert!(!root.join(".dream_commit_backup").exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_recovers_stale_commit_backup_from_manifest() {
    let root = temp_test_dir("dream-staging-manifest-rollback");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("a.md"), "old-a").await.unwrap();
    tokio::fs::write(staging.join("a.md"), "new-a")
        .await
        .unwrap();
    tokio::fs::write(staging.join("created.md"), "created")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();

    let _commit = commit_staged_memory_transaction(&real, &pre, &staging)
        .await
        .unwrap();

    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "new-a"
    );
    assert!(real.join("created.md").exists());
    assert!(root.join(DREAM_COMMIT_BACKUP_DIR).exists());

    recover_dream_commit_backups(&root, &real, true)
        .await
        .unwrap();

    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "old-a"
    );
    assert!(!real.join("created.md").exists());
    assert!(!root.join(DREAM_COMMIT_BACKUP_DIR).exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_cleans_finalized_commit_backup_without_rollback() {
    let root = temp_test_dir("dream-staging-manifest-finalized");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("a.md"), "old-a").await.unwrap();
    tokio::fs::write(staging.join("a.md"), "new-a")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();

    let _commit = commit_staged_memory_transaction(&real, &pre, &staging)
        .await
        .unwrap();

    recover_dream_commit_backups(&root, &real, false)
        .await
        .unwrap();

    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "new-a"
    );
    assert!(!root.join(DREAM_COMMIT_BACKUP_DIR).exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_releases_lock_when_commit_backup_recovery_fails() {
    let root = temp_test_dir("dream-recovery-fails-release-lock");
    tokio::fs::write(root.join(DREAM_COMMIT_BACKUP_DIR), "not a directory")
        .await
        .unwrap();

    let mut consolidator = DreamConsolidator::new(&root).await.unwrap();

    let err = consolidator.dream(5).await.unwrap_err();

    assert!(matches!(err, DreamError::Io(_)));
    assert!(!root.join(LOCK_FILE_NAME).exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_final_state_save_failure_helper_rolls_back_pending_commit() {
    let root = temp_test_dir("dream-staging-final-state-rollback");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("a.md"), "old-a").await.unwrap();
    tokio::fs::write(staging.join("a.md"), "new-a")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();
    let commit = commit_staged_memory_transaction(&real, &pre, &staging)
        .await
        .unwrap();
    let mut pending = Some(commit);

    rollback_pending_memory_commit(&mut pending).await;

    assert!(pending.is_none());
    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "old-a"
    );
    assert!(!root.join(DREAM_COMMIT_BACKUP_DIR).exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_dream_check_gates_rolls_back_stale_commit_backup() {
    let root = temp_test_dir("dream-staging-gate-stale-rollback");
    let real = root.join("memory");
    let staging = root.join(".dream_staging").join("run").join("memory");
    tokio::fs::create_dir_all(&real).await.unwrap();
    tokio::fs::create_dir_all(&staging).await.unwrap();
    tokio::fs::write(real.join("a.md"), "old-a").await.unwrap();
    tokio::fs::write(staging.join("a.md"), "new-a")
        .await
        .unwrap();
    let pre = snapshot_memory_tree(&real).await.unwrap();
    let _commit = commit_staged_memory_transaction(&real, &pre, &staging)
        .await
        .unwrap();

    let mut state = DreamState {
        current_session_count: 10,
        is_consolidating: true,
        consolidating_started_at: Some(0),
        ..Default::default()
    };

    let result = check_gates(&mut state, &root, &ConsolidatorConfig::default()).await;

    assert_eq!(result, GateCheckResult::Passed);
    assert!(!state.is_consolidating);
    assert_eq!(
        tokio::fs::read_to_string(real.join("a.md")).await.unwrap(),
        "old-a"
    );
    assert!(!root.join(DREAM_COMMIT_BACKUP_DIR).exists());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[cfg(unix)]
#[tokio::test]
async fn test_dream_cleans_state_and_lock_when_staging_prepare_fails() {
    let root = temp_test_dir("dream-staging-prepare-fails");
    let memory = root.join("memory");
    tokio::fs::create_dir_all(&memory).await.unwrap();
    std::os::unix::fs::symlink(root.join("missing-target"), memory.join("bad-link")).unwrap();

    let mut consolidator = DreamConsolidator::new(&root).await.unwrap();
    consolidator.state.current_session_count = 10;

    let err = consolidator.dream(5).await.unwrap_err();

    assert!(err.to_string().contains("symlink is not allowed"));
    assert!(!root.join(LOCK_FILE_NAME).exists());
    let persisted = DreamState::load(&root).await.unwrap();
    assert!(!persisted.is_consolidating);
    assert!(persisted.consolidating_started_at.is_none());
    let _ = tokio::fs::remove_dir_all(&root).await;
}

#[tokio::test]
async fn test_check_gates_time_failed() {
    let mut state = DreamState {
        last_consolidation_time: Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        ),
        last_session_count: 0,
        current_session_count: 10,
        consolidation_count: 1,
        is_consolidating: false,
        consolidating_started_at: None,
    };

    let result = check_gates(
        &mut state,
        Path::new("/config"),
        &ConsolidatorConfig::default(),
    )
    .await;
    assert_eq!(result, GateCheckResult::TimeGateFailed);
}

#[tokio::test]
async fn test_check_gates_session_failed() {
    let mut state = DreamState {
        last_consolidation_time: Some(0), // 很久以前
        last_session_count: 0,
        current_session_count: 3, // 少于阈值 5
        consolidation_count: 1,
        is_consolidating: false,
        consolidating_started_at: None,
    };

    let result = check_gates(
        &mut state,
        Path::new("/config"),
        &ConsolidatorConfig::default(),
    )
    .await;
    assert_eq!(result, GateCheckResult::SessionGateFailed);
}

#[tokio::test]
async fn test_check_gates_lock_failed_active() {
    // is_consolidating=true 且 consolidating_started_at 在阈值内 → 仍为活跃整合
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut state = DreamState {
        last_consolidation_time: Some(0),
        last_session_count: 0,
        current_session_count: 10,
        consolidation_count: 1,
        is_consolidating: true,              // 正在整合
        consolidating_started_at: Some(now), // 刚开始
    };

    let result = check_gates(
        &mut state,
        Path::new("/config"),
        &ConsolidatorConfig::default(),
    )
    .await;
    assert_eq!(result, GateCheckResult::LockGateFailed);
}

#[tokio::test]
async fn test_check_gates_stale_consolidating_auto_recover() {
    // is_consolidating=true 但 consolidating_started_at 超过阈值 → 自动清除 stale 标记
    let stale_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_sub(CONSOLIDATING_STALE_THRESHOLD_SECS + 100);
    let mut state = DreamState {
        last_consolidation_time: Some(0),
        last_session_count: 0,
        current_session_count: 10,
        consolidation_count: 1,
        is_consolidating: true,
        consolidating_started_at: Some(stale_time), // 超时
    };

    let result = check_gates(
        &mut state,
        Path::new("/config"),
        &ConsolidatorConfig::default(),
    )
    .await;
    // stale 标记被清除后，应继续检查时间和会话门控
    assert_eq!(result, GateCheckResult::Passed);
    // 内存中状态已清除
    assert!(!state.is_consolidating);
    assert!(state.consolidating_started_at.is_none());
}

#[tokio::test]
async fn test_check_gates_passed() {
    let mut state = DreamState {
        last_consolidation_time: Some(0), // 很久以前
        last_session_count: 0,
        current_session_count: 10, // 超过阈值 5
        consolidation_count: 1,
        is_consolidating: false,
        consolidating_started_at: None,
    };

    let result = check_gates(
        &mut state,
        Path::new("/config"),
        &ConsolidatorConfig::default(),
    )
    .await;
    assert_eq!(result, GateCheckResult::Passed);
}

// ========== 核心路径测试 ==========

#[test]
fn test_gathered_signal_creation() {
    use std::time::SystemTime;

    let signal = GatheredSignal {
        title: "User Preferences".to_string(),
        content: "User prefers dark mode".to_string(),
        importance: 8,
        source_time: SystemTime::now(),
    };

    assert_eq!(signal.title, "User Preferences");
    assert_eq!(signal.importance, 8);
}

#[test]
fn test_dream_state_serialization() {
    let state = DreamState {
        last_consolidation_time: Some(1234567890),
        last_session_count: 5,
        current_session_count: 10,
        consolidation_count: 3,
        is_consolidating: false,
        consolidating_started_at: None,
    };

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: DreamState = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.last_consolidation_time, Some(1234567890));
    assert_eq!(deserialized.current_session_count, 10);
}

#[test]
fn test_gate_check_result_variants() {
    // 确保所有变体都能正确创建和比较
    assert_eq!(
        GateCheckResult::TimeGateFailed,
        GateCheckResult::TimeGateFailed
    );
    assert_eq!(
        GateCheckResult::SessionGateFailed,
        GateCheckResult::SessionGateFailed
    );
    assert_eq!(
        GateCheckResult::LockGateFailed,
        GateCheckResult::LockGateFailed
    );
    assert_eq!(GateCheckResult::Passed, GateCheckResult::Passed);
}

#[test]
fn test_dream_config_defaults() {
    assert_eq!(TIME_GATE_THRESHOLD_HOURS, 24);
    assert_eq!(SESSION_GATE_THRESHOLD, 5);
    assert_eq!(SESSION_MEMORY_EXPIRY_DAYS, 7);
    assert_eq!(MAX_SESSIONS_TO_PROCESS, 10);
    assert_eq!(CONSOLIDATING_STALE_THRESHOLD_SECS, 3600);
}

/// 测试：DreamState 与 agent 侧 DreamStateData 的 JSON schema 一致性
///
/// 验证两个独立定义的结构体序列化/反序列化结果完全一致，
/// 防止字段名、类型或 serde 属性不匹配导致跨 crate 数据丢失。
/// 长期方案：将共享结构体移至 blockcell-core crate。
#[test]
fn test_dream_state_schema_consistency_with_agent_side() {
    use blockcell_agent::dream_state::DreamStateData;

    // 构造一个包含所有字段的完整实例
    let scheduler_state = DreamState {
        last_consolidation_time: Some(1234567890),
        last_session_count: 10,
        current_session_count: 15,
        consolidation_count: 3,
        is_consolidating: true,
        consolidating_started_at: Some(1234567800),
    };

    // 序列化 scheduler 侧结构体
    let scheduler_json = serde_json::to_value(&scheduler_state).unwrap();

    // 用 agent 侧结构体反序列化
    let agent_state: DreamStateData = serde_json::from_value(scheduler_json.clone()).unwrap();

    // 验证所有字段值一致
    assert_eq!(agent_state.last_consolidation_time, Some(1234567890u64));
    assert_eq!(agent_state.last_session_count, 10);
    assert_eq!(agent_state.current_session_count, 15);
    assert_eq!(agent_state.consolidation_count, 3);
    assert!(agent_state.is_consolidating);
    assert_eq!(agent_state.consolidating_started_at, Some(1234567800u64));

    // 反向：用 agent 侧结构体序列化，scheduler 侧反序列化
    let agent_json = serde_json::to_value(&agent_state).unwrap();
    let restored: DreamState = serde_json::from_value(agent_json.clone()).unwrap();
    assert_eq!(restored.last_consolidation_time, Some(1234567890));
    assert_eq!(restored.last_session_count, 10);
    assert_eq!(restored.current_session_count, 15);
    assert_eq!(restored.consolidation_count, 3);
    assert!(restored.is_consolidating);
    assert_eq!(restored.consolidating_started_at, Some(1234567800));

    // 验证 JSON key 集合完全一致
    let scheduler_keys: std::collections::BTreeSet<String> = scheduler_json
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let agent_keys: std::collections::BTreeSet<String> =
        agent_json.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        scheduler_keys, agent_keys,
        "scheduler 和 agent 侧 DreamState 的 JSON key 集合不一致"
    );
}
