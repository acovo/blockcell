//! 内存与进化系统配置类型
//!
//! 包含 MemoryVector, Layer1-7, MemorySystem,
//!  SelfImprove, Evolution 等配置定义。

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryVectorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default = "super::default_memory_vector_table")]
    pub table: String,
}

impl Default for MemoryVectorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: String::new(),
            model: String::new(),
            uri: None,
            table: super::default_memory_vector_table(),
        }
    }
}

// === Layer 1: 工具结果持久化配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer1Config {
    /// 单个工具结果的最大字符数（超过此值触发持久化，通过 ResponseCacheConfig 在运行时生效）
    #[serde(default = "default_l1_max_result_size")]
    pub max_result_size_chars: usize,
    #[serde(default = "default_l1_max_per_message")]
    pub max_tool_results_per_message_chars: usize,
    #[serde(
        default = "default_l1_preview_size",
        alias = "preview_size_bytes",
        alias = "previewSizeBytes"
    )]
    pub preview_size_chars: usize,
    #[serde(default = "default_l1_max_replacement")]
    pub max_replacement_entries: usize,
    #[serde(default = "default_l1_cache_max")]
    pub cache_max_per_session: usize,
    /// 可缓存最小字符数（低于此数不缓存）
    #[serde(default = "default_cacheable_min_chars")]
    pub cacheable_min_chars: usize,
}

fn default_l1_max_result_size() -> usize {
    50_000
}
fn default_l1_max_per_message() -> usize {
    150_000
}
fn default_l1_preview_size() -> usize {
    2_000
}
fn default_l1_max_replacement() -> usize {
    1_000
}
fn default_l1_cache_max() -> usize {
    10
}
fn default_cacheable_min_chars() -> usize {
    800
}

impl Default for Layer1Config {
    fn default() -> Self {
        Self {
            max_result_size_chars: 50_000,
            max_tool_results_per_message_chars: 150_000,
            preview_size_chars: 2_000,
            max_replacement_entries: 1_000,
            cache_max_per_session: 10,
            cacheable_min_chars: 800,
        }
    }
}

// === Layer 2: 时间触发 MicroCompact 配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer2Config {
    #[serde(default = "super::default_true")]
    pub enabled: bool,
    #[serde(default = "default_l2_gap_threshold")]
    pub gap_threshold_minutes: u32,
    #[serde(default = "default_l2_keep_recent")]
    pub keep_recent: u32,
}

fn default_l2_gap_threshold() -> u32 {
    60
}
fn default_l2_keep_recent() -> u32 {
    5
}

impl Default for Layer2Config {
    fn default() -> Self {
        Self {
            enabled: true,
            gap_threshold_minutes: 60,
            keep_recent: 5,
        }
    }
}

// === Layer 3: Session Memory 提取配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer3Config {
    #[serde(default = "default_l3_init_tokens")]
    pub minimum_message_tokens_to_init: usize,
    #[serde(default = "default_l3_update_tokens")]
    pub minimum_tokens_between_update: usize,
    #[serde(default = "default_l3_tool_calls")]
    pub tool_calls_between_updates: usize,
    #[serde(default = "default_l3_wait_timeout")]
    pub extraction_wait_timeout_ms: u64,
    #[serde(default = "default_l3_stale_threshold")]
    pub extraction_stale_threshold_ms: u64,
    #[serde(default = "default_l3_max_section")]
    pub max_section_length: usize,
    #[serde(default = "default_l3_max_total_tokens")]
    pub max_total_session_memory_tokens: usize,
}

fn default_l3_init_tokens() -> usize {
    10_000
}
fn default_l3_update_tokens() -> usize {
    5_000
}
fn default_l3_tool_calls() -> usize {
    3
}
fn default_l3_wait_timeout() -> u64 {
    15_000
}
fn default_l3_stale_threshold() -> u64 {
    60_000
}
fn default_l3_max_section() -> usize {
    2_000
}
fn default_l3_max_total_tokens() -> usize {
    12_000
}

impl Default for Layer3Config {
    fn default() -> Self {
        Self {
            minimum_message_tokens_to_init: 10_000,
            minimum_tokens_between_update: 5_000,
            tool_calls_between_updates: 3,
            extraction_wait_timeout_ms: 15_000,
            extraction_stale_threshold_ms: 60_000,
            max_section_length: 2_000,
            max_total_session_memory_tokens: 12_000,
        }
    }
}

// === Layer 4: Full Compact + 恢复配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer4Config {
    #[serde(default = "default_l4_threshold_ratio")]
    pub compact_threshold_ratio: f64,
    #[serde(default = "default_l4_keep_recent")]
    pub keep_recent_messages: usize,
    #[serde(default = "default_l4_max_output")]
    pub max_output_tokens: usize,
    #[serde(default = "default_l4_file_recovery")]
    pub max_file_recovery_tokens: usize,
    #[serde(default = "default_l4_single_file")]
    pub max_single_file_tokens: usize,
    #[serde(default = "default_l4_max_files")]
    pub max_files_to_recover: usize,
    #[serde(default = "default_l4_skill_recovery")]
    pub max_skill_recovery_tokens: usize,
    #[serde(default = "default_l4_session_memory_recovery")]
    pub max_session_memory_recovery_tokens: usize,
    #[serde(default = "default_l4_tracker_summary")]
    pub tracker_summary_chars: usize,
}

fn default_l4_threshold_ratio() -> f64 {
    0.8
}
fn default_l4_keep_recent() -> usize {
    2
}
fn default_l4_max_output() -> usize {
    12_000
}
fn default_l4_file_recovery() -> usize {
    50_000
}
fn default_l4_single_file() -> usize {
    5_000
}
fn default_l4_max_files() -> usize {
    5
}
fn default_l4_skill_recovery() -> usize {
    25_000
}
fn default_l4_session_memory_recovery() -> usize {
    12_000
}
fn default_l4_tracker_summary() -> usize {
    2_000
}

impl Default for Layer4Config {
    fn default() -> Self {
        Self {
            compact_threshold_ratio: 0.8,
            keep_recent_messages: 2,
            max_output_tokens: 12_000,
            max_file_recovery_tokens: 50_000,
            max_single_file_tokens: 5_000,
            max_files_to_recover: 5,
            max_skill_recovery_tokens: 25_000,
            max_session_memory_recovery_tokens: 12_000,
            tracker_summary_chars: 2_000,
        }
    }
}

// === Layer 5: Auto Memory 提取 + 注入配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer5Config {
    #[serde(default = "default_l5_min_messages")]
    pub min_messages_for_extraction: usize,
    #[serde(default = "default_l5_cooldown")]
    pub extraction_cooldown_messages: usize,
    #[serde(default = "default_l5_max_file_tokens")]
    pub max_memory_file_tokens: usize,
    #[serde(default = "default_l5_injection_max")]
    pub injection_max_tokens: usize,
    /// 提取时间冷却阈值（秒），距离上次提取需经过此时间
    #[serde(default = "default_l5_time_cooldown_secs")]
    pub extraction_time_cooldown_secs: u64,
    /// 内容变化阈值（字符数），内容变化需超过此值才触发提取
    #[serde(default = "default_l5_content_change_threshold")]
    pub content_change_threshold: usize,
    /// 后台提取 pending marker 过期阈值（毫秒）
    /// 独立于 Layer 3 的 extraction_stale_threshold_ms，避免长任务被短阈值误判
    #[serde(default = "default_l5_extraction_stale_threshold_ms")]
    pub extraction_stale_threshold_ms: u64,
}

fn default_l5_min_messages() -> usize {
    15
}
fn default_l5_cooldown() -> usize {
    5
}
fn default_l5_max_file_tokens() -> usize {
    4_000
}
fn default_l5_injection_max() -> usize {
    4_000
}
fn default_l5_time_cooldown_secs() -> u64 {
    300
}
fn default_l5_content_change_threshold() -> usize {
    500
}
fn default_l5_extraction_stale_threshold_ms() -> u64 {
    300_000 // 5 分钟，比 Layer 3 的 1 分钟更长，适合长任务
}

impl Default for Layer5Config {
    fn default() -> Self {
        Self {
            min_messages_for_extraction: 15,
            extraction_cooldown_messages: 5,
            max_memory_file_tokens: 4_000,
            injection_max_tokens: 4_000,
            extraction_time_cooldown_secs: 300,
            content_change_threshold: 500,
            extraction_stale_threshold_ms: 300_000,
        }
    }
}

// === Layer 6: Dream 整合配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer6Config {
    /// 是否启用 Dream 整合
    #[serde(default = "super::default_true")]
    pub enabled: bool,
    /// 检查间隔（秒）
    #[serde(default = "default_l6_check_interval")]
    pub check_interval_secs: u64,
    /// 时间门控阈值（小时）
    #[serde(default = "default_l6_time_gate_hours")]
    pub time_gate_threshold_hours: u64,
    /// 会话门控阈值
    #[serde(default = "default_l6_session_gate")]
    pub session_gate_threshold: usize,
    /// Dream 执行超时（秒）
    #[serde(default = "default_l6_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_l6_check_interval() -> u64 {
    600 // 10 分钟
}
fn default_l6_time_gate_hours() -> u64 {
    24
}
fn default_l6_session_gate() -> usize {
    5
}
fn default_l6_timeout_secs() -> u64 {
    300 // 5 分钟
}

impl Default for Layer6Config {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_secs: 600,
            time_gate_threshold_hours: 24,
            session_gate_threshold: 5,
            timeout_secs: 300,
        }
    }
}

// === Layer 7: Forked Agent 配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer7Config {
    /// 是否启用 Forked Agent
    #[serde(default = "super::default_true")]
    pub enabled: bool,
    /// 最大轮次
    #[serde(default = "default_l7_max_turns")]
    pub max_turns: usize,
    /// 执行超时（秒）
    #[serde(default = "default_l7_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_l7_max_turns() -> usize {
    10
}
fn default_l7_timeout_secs() -> u64 {
    120 // 2 分钟
}

impl Default for Layer7Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_turns: 10,
            timeout_secs: 120,
        }
    }
}

// === 熔断器配置（用户配置结构，会被转换为运行时的 CircuitBreakerConfig） ===
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerSettings {
    pub max_failures: u64,
    pub reset_timeout_secs: u64,
    #[serde(skip)]
    configured: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CircuitBreakerSettingsWire {
    #[serde(default = "default_cb_max_failures")]
    max_failures: u64,
    #[serde(default = "default_cb_reset_timeout_secs")]
    reset_timeout_secs: u64,
}

fn default_cb_max_failures() -> u64 {
    3
}
fn default_cb_reset_timeout_secs() -> u64 {
    60
}

impl Default for CircuitBreakerSettings {
    fn default() -> Self {
        Self {
            max_failures: 3,
            reset_timeout_secs: 60,
            configured: false,
        }
    }
}

impl<'de> Deserialize<'de> for CircuitBreakerSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CircuitBreakerSettingsWire::deserialize(deserializer)?;
        Ok(Self {
            max_failures: wire.max_failures,
            reset_timeout_secs: wire.reset_timeout_secs,
            configured: true,
        })
    }
}

impl CircuitBreakerSettings {
    /// 返回用户配置文件中是否显式出现了 circuitBreaker。
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    fn has_default_values(&self) -> bool {
        self.max_failures == default_cb_max_failures()
            && self.reset_timeout_secs == default_cb_reset_timeout_secs()
    }

    fn is_implicit_default(&self) -> bool {
        !self.configured && self.has_default_values()
    }
}

// === 监控配置 ===
// TODO: 接入运行时行为 — 当前仅定义配置结构体，尚未在业务路径中使用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringConfig {
    #[serde(default = "default_monitoring_enabled")]
    pub enabled: bool,
}

fn default_monitoring_enabled() -> bool {
    true
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// === 压缩通知配置 ===
// TODO: 接入运行时行为 — 当前仅定义配置结构体，尚未在业务路径中使用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactNotificationConfig {
    #[serde(default = "default_compact_notify_enabled")]
    pub enabled: bool,
}

fn default_compact_notify_enabled() -> bool {
    true
}

impl Default for CompactNotificationConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// === 7 层记忆系统配置 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySystemConfig {
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,
    #[serde(default = "super::default_true")]
    pub auto_memory_enabled: bool,
    #[serde(default = "super::default_true")]
    pub compact_enabled: bool,
    #[serde(default)]
    pub layer1: Layer1Config,
    #[serde(default)]
    pub layer2: Layer2Config,
    #[serde(default)]
    pub layer3: Layer3Config,
    #[serde(default)]
    pub layer4: Layer4Config,
    #[serde(default)]
    pub layer5: Layer5Config,
    #[serde(default)]
    pub layer6: Layer6Config,
    #[serde(default)]
    pub layer7: Layer7Config,
    #[serde(
        default,
        skip_serializing_if = "CircuitBreakerSettings::is_implicit_default"
    )]
    pub circuit_breaker: CircuitBreakerSettings,
    #[doc(hidden)]
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[doc(hidden)]
    #[serde(default)]
    pub compact_notification: CompactNotificationConfig,
}

fn default_token_budget() -> usize {
    100_000
}

impl Default for MemorySystemConfig {
    fn default() -> Self {
        Self {
            token_budget: 100_000,
            auto_memory_enabled: true,
            compact_enabled: true,
            layer1: Layer1Config::default(),
            layer2: Layer2Config::default(),
            layer3: Layer3Config::default(),
            layer4: Layer4Config::default(),
            layer5: Layer5Config::default(),
            layer6: Layer6Config::default(),
            layer7: Layer7Config::default(),
            circuit_breaker: CircuitBreakerSettings::default(),
            monitoring: MonitoringConfig::default(),
            compact_notification: CompactNotificationConfig::default(),
        }
    }
}

impl MemorySystemConfig {
    /// Validate configuration and return a list of warnings.
    ///
    /// Warnings are non-fatal — the system will still start, but the user
    /// should be informed about potentially problematic settings.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // token_budget range check
        if self.token_budget < 20_000 {
            warnings.push(format!(
                "memorySystem.tokenBudget = {} is very low (min recommended: 20,000). \
                 This may cause frequent compaction.",
                self.token_budget
            ));
        }
        if self.token_budget > 500_000 {
            warnings.push(format!(
                "memorySystem.tokenBudget = {} is very high (max recommended: 500,000). \
                 This may cause excessive memory usage.",
                self.token_budget
            ));
        }

        // L1: max_result_size_chars <= max_tool_results_per_message_chars
        if self.layer1.max_result_size_chars > self.layer1.max_tool_results_per_message_chars {
            warnings.push(format!(
                "layer1.maxResultSizeChars ({}) > layer1.maxToolResultsPerMessageChars ({}). \
                 Single result exceeds per-message budget.",
                self.layer1.max_result_size_chars, self.layer1.max_tool_results_per_message_chars
            ));
        }

        // L1: preview_size_chars 不超过 max_result_size_chars，防止 fallback 截断路径下溢
        // 注意：validate 为 &self，无法就地 clamp；超值时仅发 warning，运行时应做 saturating_sub 防御
        if self.layer1.preview_size_chars > self.layer1.max_result_size_chars {
            warnings.push(format!(
                "layer1.previewSizeChars ({}) > layer1.maxResultSizeChars ({}). \
                 Preview exceeds inline truncation budget; runtime uses saturating_sub to guard.",
                self.layer1.preview_size_chars, self.layer1.max_result_size_chars
            ));
        }

        // L4: compact_threshold_ratio range
        if self.layer4.compact_threshold_ratio < 0.5 {
            warnings.push(format!(
                "layer4.compactThresholdRatio = {:.2} is below 0.5. \
                 This may cause premature compaction.",
                self.layer4.compact_threshold_ratio
            ));
        }
        if self.layer4.compact_threshold_ratio > 0.95 {
            warnings.push(format!(
                "layer4.compactThresholdRatio = {:.2} is above 0.95. \
                 This may leave insufficient room for compaction.",
                self.layer4.compact_threshold_ratio
            ));
        }

        // L4: recovery budget check
        let total_recovery = self.layer4.max_file_recovery_tokens
            + self.layer4.max_skill_recovery_tokens
            + self.layer4.max_session_memory_recovery_tokens;
        let budget_95pct = (self.token_budget as f64 * 0.95) as usize;
        if total_recovery > budget_95pct {
            warnings.push(format!(
                "L4 recovery total ({} + {} + {} = {}) exceeds 95% of tokenBudget ({}). \
                 Recovery may consume too much of the budget.",
                self.layer4.max_file_recovery_tokens,
                self.layer4.max_skill_recovery_tokens,
                self.layer4.max_session_memory_recovery_tokens,
                total_recovery,
                budget_95pct
            ));
        }

        // L4: single file × max files <= file recovery budget
        let max_file_total = self.layer4.max_single_file_tokens * self.layer4.max_files_to_recover;
        if max_file_total > self.layer4.max_file_recovery_tokens {
            warnings.push(format!(
                "layer4.maxSingleFileTokens ({}) × layer4.maxFilesToRecover ({}) = {} \
                 exceeds layer4.maxFileRecoveryTokens ({}). \
                 Some files may not be recovered.",
                self.layer4.max_single_file_tokens,
                self.layer4.max_files_to_recover,
                max_file_total,
                self.layer4.max_file_recovery_tokens
            ));
        }

        // L3 vs L4: session memory consistency
        if self.layer3.max_total_session_memory_tokens
            != self.layer4.max_session_memory_recovery_tokens
        {
            warnings.push(format!(
                "layer3.maxTotalSessionMemoryTokens ({}) != \
                 layer4.maxSessionMemoryRecoveryTokens ({}). \
                 Consider aligning these values.",
                self.layer3.max_total_session_memory_tokens,
                self.layer4.max_session_memory_recovery_tokens
            ));
        }

        // L5: injection_max_tokens should be reasonable
        if self.layer5.injection_max_tokens > self.layer5.max_memory_file_tokens * 4 {
            warnings.push(format!(
                "layer5.injectionMaxTokens ({}) > 4 × layer5.maxMemoryFileTokens ({}). \
                 Injection budget may be too large relative to file size limit.",
                self.layer5.injection_max_tokens, self.layer5.max_memory_file_tokens
            ));
        }

        warnings
    }
}

/// Global system-prompt section budgets. Values are token ceilings and the
/// allocator also enforces `total` across all sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBudgetConfig {
    #[serde(default = "default_prompt_total")]
    pub total: usize,
    #[serde(default = "default_prompt_rules")]
    pub rules: usize,
    #[serde(default = "default_prompt_user_profile")]
    pub user_profile: usize,
    #[serde(default = "default_prompt_retrieved")]
    pub retrieved: usize,
    #[serde(default = "default_prompt_active_skill")]
    pub active_skill: usize,
    #[serde(default = "default_prompt_session_recovery")]
    pub session_recovery: usize,
}

fn default_prompt_total() -> usize {
    12_000
}

fn default_prompt_rules() -> usize {
    2_000
}

fn default_prompt_user_profile() -> usize {
    800
}

fn default_prompt_retrieved() -> usize {
    3_000
}

fn default_prompt_active_skill() -> usize {
    4_000
}

fn default_prompt_session_recovery() -> usize {
    2_000
}

impl Default for PromptBudgetConfig {
    fn default() -> Self {
        Self {
            total: default_prompt_total(),
            rules: default_prompt_rules(),
            user_profile: default_prompt_user_profile(),
            retrieved: default_prompt_retrieved(),
            active_skill: default_prompt_active_skill(),
            session_recovery: default_prompt_session_recovery(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRecallMode {
    Chat,
    General,
    Skill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecallConfig {
    #[serde(default = "super::default_true")]
    pub chat: bool,
    #[serde(default = "super::default_true")]
    pub general: bool,
    #[serde(default = "super::default_true")]
    pub skill: bool,
    #[serde(default)]
    pub internal: bool,
}

impl MemoryRecallConfig {
    pub fn allows(&self, mode: MemoryRecallMode, channel: &str) -> bool {
        const INTERNAL_CHANNELS: [&str; 4] = ["ghost", "cron", "system", "subagent"];
        if INTERNAL_CHANNELS.contains(&channel) && !self.internal {
            return false;
        }
        match mode {
            MemoryRecallMode::Chat => self.chat,
            MemoryRecallMode::General => self.general,
            MemoryRecallMode::Skill => self.skill,
        }
    }
}

impl Default for MemoryRecallConfig {
    fn default() -> Self {
        Self {
            chat: true,
            general: true,
            skill: true,
            internal: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfig {
    #[serde(default)]
    pub vector: MemoryVectorConfig,
    /// 7 层记忆系统阈值配置
    #[serde(default)]
    pub memory_system: MemorySystemConfig,
    /// Global prompt allocation for fixed rules and retrieved context.
    #[serde(default)]
    pub prompt_budget: PromptBudgetConfig,
    /// Recall policy by interaction mode and internal channel.
    #[serde(default)]
    pub memory_recall: MemoryRecallConfig,
}

/// Self-Improve 配置 — Nudge + Review 子系统
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SelfImproveConfig {
    /// Nudge 配置
    #[serde(default)]
    pub nudge: SelfImproveNudgeConfig,
    /// Review 配置
    #[serde(default)]
    pub review: SelfImproveReviewConfig,
}

/// 进化服务配置（对应 blockcell_skills::EvolutionServiceConfig）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionConfig {
    /// 触发进化所需的连续错误次数（默认 1）
    #[serde(default = "default_evolution_error_threshold")]
    pub error_threshold: u32,
    /// 错误统计的时间窗口（分钟，默认 30）
    #[serde(default = "default_evolution_error_window_minutes")]
    pub error_window_minutes: u32,
    /// 是否启用自动进化（默认 true）
    #[serde(default = "default_evolution_enabled")]
    pub enabled: bool,
    /// 每个阶段失败后的最大重试次数（默认 3）
    #[serde(default = "default_evolution_max_retries")]
    pub max_retries: u32,
    /// LLM 调用超时时间（秒，默认 300）
    #[serde(default = "default_evolution_llm_timeout_secs")]
    pub llm_timeout_secs: u64,
    /// 回滚冷却期时长（分钟，默认 60）
    #[serde(default = "default_evolution_cooldown_minutes")]
    pub cooldown_minutes: u32,
}

fn default_evolution_error_threshold() -> u32 {
    1
}
fn default_evolution_error_window_minutes() -> u32 {
    30
}
fn default_evolution_enabled() -> bool {
    true
}
fn default_evolution_max_retries() -> u32 {
    3
}
fn default_evolution_llm_timeout_secs() -> u64 {
    300
}
fn default_evolution_cooldown_minutes() -> u32 {
    60
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            error_threshold: default_evolution_error_threshold(),
            error_window_minutes: default_evolution_error_window_minutes(),
            enabled: default_evolution_enabled(),
            max_retries: default_evolution_max_retries(),
            llm_timeout_secs: default_evolution_llm_timeout_secs(),
            cooldown_minutes: default_evolution_cooldown_minutes(),
        }
    }
}

/// Self-Improve Nudge 配置 — Skill 和 Memory 使用独立阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfImproveNudgeConfig {
    /// Skill nudge 软阈值 (默认: 5 次工具迭代)
    #[serde(default = "default_skill_soft_threshold")]
    pub skill_soft_threshold: u32,
    /// Skill nudge 硬阈值 (默认: 10 次工具迭代)
    #[serde(default = "default_skill_hard_threshold")]
    pub skill_hard_threshold: u32,
    /// Memory nudge 软阈值 (默认: 3 次用户轮次)
    #[serde(default = "default_memory_soft_threshold")]
    pub memory_soft_threshold: u32,
    /// Memory nudge 硬阈值 (默认: 6 次用户轮次)
    #[serde(default = "default_memory_hard_threshold")]
    pub memory_hard_threshold: u32,
    /// 是否启用 nudge (默认: true)
    #[serde(default = "super::default_true")]
    pub enabled: bool,
    /// 最小 nudge 间隔秒数 (默认: 300)
    #[serde(default = "default_min_nudge_interval_secs")]
    pub min_nudge_interval_secs: u64,
}

fn default_skill_soft_threshold() -> u32 {
    5
}
fn default_skill_hard_threshold() -> u32 {
    10
}
fn default_memory_soft_threshold() -> u32 {
    3
}
fn default_memory_hard_threshold() -> u32 {
    6
}
fn default_min_nudge_interval_secs() -> u64 {
    300
}

impl Default for SelfImproveNudgeConfig {
    fn default() -> Self {
        Self {
            skill_soft_threshold: 5,
            skill_hard_threshold: 10,
            memory_soft_threshold: 3,
            memory_hard_threshold: 6,
            enabled: true,
            min_nudge_interval_secs: 300,
        }
    }
}

/// Self-Improve Review 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfImproveReviewConfig {
    /// 是否启用 Review (默认: true)
    #[serde(default = "super::default_true")]
    pub enabled: bool,
    /// Review 最大轮次 (默认: 8)
    #[serde(default = "default_max_review_rounds")]
    pub max_rounds: u32,
}

fn default_max_review_rounds() -> u32 {
    8
}

impl Default for SelfImproveReviewConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_rounds: 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 previewSizeBytes（旧 camelCase 配置）能正确反序列化到 preview_size_chars
    #[test]
    fn test_preview_size_bytes_camel_case_alias() {
        let json = r#"{"previewSizeBytes": 4096}"#;
        let config: Layer1Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.preview_size_chars, 4096);
    }

    /// 测试 preview_size_bytes（旧 snake_case 配置）能正确反序列化
    #[test]
    fn test_preview_size_bytes_snake_case_alias() {
        let json = r#"{"preview_size_bytes": 3000}"#;
        let config: Layer1Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.preview_size_chars, 3000);
    }

    /// 测试新字段名 previewSizeChars（camelCase，与 rename_all 一致）正常工作
    #[test]
    fn test_preview_size_chars_current_name() {
        let json = r#"{"previewSizeChars": 5000}"#;
        let config: Layer1Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.preview_size_chars, 5000);
    }

    /// 测试默认值
    #[test]
    fn test_preview_size_default() {
        let config: Layer1Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config.preview_size_chars, 2000);
    }
}
