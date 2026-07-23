//! Gateway 内部使用的纯工具函数：进度节流、进度条格式化、常量时间比较、
//! workspace 相对路径校验，以及活动模型/Provider
//! 解析等。均为从 `gateway.rs` 抽离的独立函数，不依赖 GatewayState，不改变行为。

use blockcell_core::Config;
use std::collections::HashMap;

pub(super) fn new_confirm_request_id() -> String {
    format!("confirm_{}", uuid::Uuid::new_v4().simple())
}

pub(super) fn retain_active_progress_throttle_entries<F>(
    forwarded: &mut HashMap<String, u8>,
    mut status_for_task: F,
) where
    F: FnMut(&str) -> Option<blockcell_agent::task_manager::TaskStatus>,
{
    forwarded.retain(|task_id, _| {
        status_for_task(task_id)
            .as_ref()
            .is_some_and(|status| !blockcell_agent::task_manager::is_terminal_status(status))
    });
}

pub(super) fn update_stage_progress_throttle(
    forwarded: &mut HashMap<String, u8>,
    task_id: &str,
    percent: u8,
    threshold: u8,
    task_status: Option<&blockcell_agent::task_manager::TaskStatus>,
) -> bool {
    let terminal_or_missing = task_status
        .map(blockcell_agent::task_manager::is_terminal_status)
        .unwrap_or(true);
    if terminal_or_missing {
        forwarded.remove(task_id);
        return percent >= 100 && task_status.is_some();
    }

    let should_forward = match forwarded.get(task_id) {
        Some(&last) => percent.abs_diff(last) >= threshold,
        None => true,
    } || percent >= 100;

    if percent >= 100 {
        forwarded.remove(task_id);
    } else if should_forward {
        forwarded.insert(task_id.to_string(), percent);
    }

    should_forward
}

/// 格式化进度条（10 格宽度，用于 channel 进度转发）
/// 使用四舍五入避免 1-9% 显示为空进度条
pub(super) fn format_progress_bar(percent: u8) -> String {
    let clamped = percent.min(100);
    // 四舍五入：(clamped * 10 + 50) / 100，5% 显示 1 格
    let filled = ((clamped as usize * 10 + 50) / 100).min(10);
    let empty = 10 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

pub(super) fn secure_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.as_bytes().iter().zip(b.as_bytes().iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub(super) fn validate_workspace_relative_path(path: &str) -> Result<std::path::PathBuf, String> {
    if path.trim().is_empty() {
        return Err("path is required".to_string());
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return Err("absolute paths are not allowed".to_string());
    }
    let mut normalized = std::path::PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(s) => normalized.push(s),
            std::path::Component::ParentDir => {
                return Err("path traversal (..) is not allowed".to_string());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("invalid path".to_string());
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("invalid path".to_string());
    }
    Ok(normalized)
}

pub(super) fn primary_pool_entry(config: &Config) -> Option<&blockcell_core::config::ModelEntry> {
    config
        .agents
        .defaults
        .model_pool
        .iter()
        .min_by(|a, b| a.priority.cmp(&b.priority).then(b.weight.cmp(&a.weight)))
}

pub(super) fn active_model_and_provider(config: &Config) -> (String, Option<String>, &'static str) {
    if let Some(entry) = primary_pool_entry(config) {
        return (
            entry.model.clone(),
            Some(entry.provider.clone()),
            "modelPool",
        );
    }

    (
        config.agents.defaults.model.clone(),
        config.agents.defaults.provider.clone(),
        "agents.defaults",
    )
}
