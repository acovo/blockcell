//! Ghost（自进化/学习）相关配置类型。
//!
//! 包含 GhostLearningConfig、GhostConfig。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GhostLearningConfig {
    /// 旧版总开关。未配置 captureEnabled 时作为捕获开关的兼容来源。
    #[serde(default = "default_ghost_learning_enabled")]
    pub enabled: bool,
    /// 旧版影子模式。新三开关未显式配置时等价于：捕获开启、写入影子目录、召回关闭。
    #[serde(default = "default_ghost_learning_shadow_mode")]
    pub shadow_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_enabled: Option<bool>,
    #[serde(default = "default_ghost_turn_review_interval")]
    pub turn_review_interval: u32,
    #[serde(default = "default_ghost_method_tool_threshold")]
    pub method_tool_threshold: u32,
    #[serde(default = "default_ghost_recall_max_items")]
    pub recall_max_items: u32,
    #[serde(default = "default_ghost_recall_token_budget")]
    pub recall_token_budget: u32,
}

fn default_ghost_learning_enabled() -> bool {
    true
}

fn default_ghost_learning_shadow_mode() -> bool {
    true
}

fn default_ghost_turn_review_interval() -> u32 {
    6
}

fn default_ghost_method_tool_threshold() -> u32 {
    3
}

fn default_ghost_recall_max_items() -> u32 {
    4
}

fn default_ghost_recall_token_budget() -> u32 {
    1200
}

impl Default for GhostLearningConfig {
    fn default() -> Self {
        Self {
            enabled: default_ghost_learning_enabled(),
            shadow_mode: default_ghost_learning_shadow_mode(),
            capture_enabled: None,
            write_enabled: None,
            recall_enabled: None,
            turn_review_interval: default_ghost_turn_review_interval(),
            method_tool_threshold: default_ghost_method_tool_threshold(),
            recall_max_items: default_ghost_recall_max_items(),
            recall_token_budget: default_ghost_recall_token_budget(),
        }
    }
}

impl GhostLearningConfig {
    pub fn capture_enabled(&self) -> bool {
        self.capture_enabled.unwrap_or(self.enabled)
    }

    /// true 表示写入正式知识文件；false 表示仅写入 shadow 命名空间。
    pub fn write_enabled(&self) -> bool {
        self.write_enabled
            .unwrap_or(self.enabled && !self.shadow_mode)
    }

    pub fn recall_enabled(&self) -> bool {
        self.recall_enabled
            .unwrap_or(self.enabled && !self.shadow_mode)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GhostConfig {
    #[serde(default = "default_ghost_enabled")]
    pub enabled: bool,
    /// If None, uses the default agent model.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_ghost_schedule")]
    pub schedule: String,
    #[serde(default = "default_max_syncs")]
    pub max_syncs_per_day: u32,
    #[serde(default = "default_auto_social")]
    pub auto_social: bool,
    #[serde(default)]
    pub learning: GhostLearningConfig,
}

fn default_ghost_enabled() -> bool {
    false
}

fn default_ghost_schedule() -> String {
    "0 */4 * * *".to_string() // Every 4 hours
}

fn default_max_syncs() -> u32 {
    10
}

fn default_auto_social() -> bool {
    true
}

impl Default for GhostConfig {
    fn default() -> Self {
        Self {
            enabled: default_ghost_enabled(),
            model: None,
            schedule: default_ghost_schedule(),
            max_syncs_per_day: default_max_syncs(),
            auto_social: default_auto_social(),
            learning: GhostLearningConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_shadow_mode_maps_to_capture_without_write_or_recall() {
        let config: GhostLearningConfig = json5::from_str(
            r#"{
                enabled: true,
                shadowMode: true
            }"#,
        )
        .unwrap();

        assert!(config.capture_enabled());
        assert!(!config.write_enabled());
        assert!(!config.recall_enabled());
    }

    #[test]
    fn explicit_learning_switches_override_legacy_shadow_mode() {
        let config: GhostLearningConfig = json5::from_str(
            r#"{
                enabled: true,
                shadowMode: true,
                captureEnabled: false,
                writeEnabled: true,
                recallEnabled: true
            }"#,
        )
        .unwrap();

        assert!(!config.capture_enabled());
        assert!(config.write_enabled());
        assert!(config.recall_enabled());
    }
}
