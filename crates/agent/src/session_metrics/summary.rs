//! Metrics Summary - Snapshot and formatting for CLI output.

use super::memory::get_memory_metrics;
use super::circuit_breaker::{
    get_compact_circuit_breaker,
    get_memory_extraction_circuit_breaker,
    get_dream_circuit_breaker,
    reset_all_circuit_breakers,
    CircuitState,
};
use serde::Serialize;

/// Summary of a single circuit breaker.
#[derive(Debug, Serialize)]
pub struct CircuitBreakerSummary {
    /// Current state of the circuit breaker.
    pub state: CircuitState,
    /// Number of consecutive failures.
    pub failures: u64,
}

/// Snapshot of all memory metrics.
#[derive(Debug, Serialize)]
pub struct MetricsSummary {
    pub layer1: Layer1Summary,
    pub layer2: Layer2Summary,
    pub layer3: Layer3Summary,
    pub layer4: Layer4Summary,
    pub layer5: Layer5Summary,
    pub layer6: Layer6Summary,
    pub layer7: Layer7Summary,
    /// Layer 4: Compact circuit breaker (快速恢复 - 用户同步操作)
    pub compact_circuit_breaker: CircuitBreakerSummary,
    /// Layer 5: Memory Extraction circuit breaker (中等恢复 - 后台异步操作)
    pub memory_extraction_circuit_breaker: CircuitBreakerSummary,
    /// Layer 6: Dream Consolidation circuit breaker (慢恢复 - 定时后台任务)
    pub dream_circuit_breaker: CircuitBreakerSummary,
}

#[derive(Debug, Serialize)]
pub struct Layer1Summary {
    pub persisted_count: u64,
    pub total_original_size: u64,
    pub total_preview_size: u64,
    pub budget_exceeded_count: u64,
    pub average_compression: f64,
    // 新增字段
    pub max_tool_results: u64,
    pub preview_size_limit: u64,
    pub current_stored_results: u64,
}

#[derive(Debug, Serialize)]
pub struct Layer2Summary {
    pub trigger_count: u64,
    pub cleared_count: u64,
    pub kept_count: u64,
    // 新增字段
    pub gap_threshold_minutes: u64,
    pub keep_recent: u64,
    pub last_trigger_timestamp: u64,
}

#[derive(Debug, Serialize)]
pub struct Layer3Summary {
    pub extraction_count: u64,
    pub load_count: u64,
    pub current_size: u64,
    // 新增字段
    pub max_total_tokens: u64,
    pub max_section_length: u64,
    pub last_extraction_timestamp: u64,
    pub section_count: u64,
}

#[derive(Debug, Serialize)]
pub struct Layer4Summary {
    pub compact_count: u64,
    pub auto_compact_count: u64,
    pub manual_compact_count: u64,
    pub failed_count: u64,
    pub consecutive_failures: u64,
    pub average_compression_ratio: f64,
    pub cache_hit_rate: f64,
    // 新增字段
    pub token_budget: u64,
    pub threshold_ratio: f64,
    pub threshold_tokens: u64,
    pub current_tokens: u64,
    pub remaining_tokens: u64,
    pub usage_percentage: f64,
    pub last_compact_timestamp: u64,
    pub total_recovery_budget: u64,
}

#[derive(Debug, Serialize)]
pub struct Layer5Summary {
    pub extraction_count: u64,
    pub user_memories: u64,
    pub project_memories: u64,
    pub feedback_memories: u64,
    pub reference_memories: u64,
    pub total_bytes_written: u64,
    // 新增字段
    pub min_messages: u64,
    pub cooldown_messages: u64,
    pub max_file_tokens: u64,
    pub last_extraction_timestamp: u64,
    pub user_bytes: u64,
    pub project_bytes: u64,
    pub feedback_bytes: u64,
    pub reference_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct Layer6Summary {
    pub dream_count: u64,
    pub memories_created: u64,
    pub memories_updated: u64,
    pub memories_deleted: u64,
    pub sessions_pruned: u64,
    // 新增字段
    pub dream_interval_hours: u64,
    pub last_dream_timestamp: u64,
    pub sessions_processed: u64,
    pub consolidation_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct Layer7Summary {
    pub spawned_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub tool_denied_count: u64,
    pub total_tokens_used: u64,
    pub total_turns_used: u64,
    // 新增字段
    pub max_turns: u64,
    pub active_count: u64,
    pub avg_completion_time_ms: f64,
    pub avg_tokens_per_agent: f64,
    pub avg_turns: f64,
    pub success_rate: f64,
}

/// Get a snapshot of all memory metrics.
pub fn get_metrics_summary() -> MetricsSummary {
    let m = get_memory_metrics();
    let compact_cb = get_compact_circuit_breaker();
    let extraction_cb = get_memory_extraction_circuit_breaker();
    let dream_cb = get_dream_circuit_breaker();

    MetricsSummary {
        layer1: Layer1Summary {
            persisted_count: m.layer1.persisted_count(),
            total_original_size: m.layer1.total_original_size(),
            total_preview_size: m.layer1.total_preview_size(),
            budget_exceeded_count: m.layer1.budget_exceeded_count(),
            average_compression: m.layer1.average_compression(),
            max_tool_results: m.layer1.max_tool_results(),
            preview_size_limit: m.layer1.preview_size_limit(),
            current_stored_results: m.layer1.current_stored_results(),
        },
        layer2: Layer2Summary {
            trigger_count: m.layer2.trigger_count(),
            cleared_count: m.layer2.cleared_count(),
            kept_count: m.layer2.kept_count(),
            gap_threshold_minutes: m.layer2.gap_threshold_minutes(),
            keep_recent: m.layer2.keep_recent(),
            last_trigger_timestamp: m.layer2.last_trigger_timestamp(),
        },
        layer3: Layer3Summary {
            extraction_count: m.layer3.extraction_count(),
            load_count: m.layer3.load_count(),
            current_size: m.layer3.current_size(),
            max_total_tokens: m.layer3.max_total_tokens(),
            max_section_length: m.layer3.max_section_length(),
            last_extraction_timestamp: m.layer3.last_extraction_timestamp(),
            section_count: m.layer3.section_count(),
        },
        layer4: Layer4Summary {
            compact_count: m.layer4.compact_count(),
            auto_compact_count: m.layer4.auto_compact_count(),
            manual_compact_count: m.layer4.manual_compact_count(),
            failed_count: m.layer4.compact_failed_count(),
            consecutive_failures: m.layer4.consecutive_failures(),
            average_compression_ratio: m.layer4.average_compression_ratio(),
            cache_hit_rate: m.layer4.cache_hit_rate(),
            token_budget: m.layer4.token_budget(),
            threshold_ratio: m.layer4.threshold_ratio(),
            threshold_tokens: m.layer4.threshold_tokens(),
            current_tokens: m.layer4.current_tokens(),
            remaining_tokens: m.layer4.remaining_tokens(),
            usage_percentage: m.layer4.usage_percentage(),
            last_compact_timestamp: m.layer4.last_compact_timestamp(),
            total_recovery_budget: m.layer4.total_recovery_budget(),
        },
        layer5: Layer5Summary {
            extraction_count: m.layer5.extraction_count(),
            user_memories: m.layer5.user_memories(),
            project_memories: m.layer5.project_memories(),
            feedback_memories: m.layer5.feedback_memories(),
            reference_memories: m.layer5.reference_memories(),
            total_bytes_written: m.layer5.total_bytes_written(),
            min_messages: m.layer5.min_messages(),
            cooldown_messages: m.layer5.cooldown_messages(),
            max_file_tokens: m.layer5.max_file_tokens(),
            last_extraction_timestamp: m.layer5.last_extraction_timestamp(),
            user_bytes: m.layer5.user_bytes(),
            project_bytes: m.layer5.project_bytes(),
            feedback_bytes: m.layer5.feedback_bytes(),
            reference_bytes: m.layer5.reference_bytes(),
        },
        layer6: Layer6Summary {
            dream_count: m.layer6.dream_count(),
            memories_created: m.layer6.memories_created(),
            memories_updated: m.layer6.memories_updated(),
            memories_deleted: m.layer6.memories_deleted(),
            sessions_pruned: m.layer6.sessions_pruned(),
            dream_interval_hours: m.layer6.dream_interval_hours(),
            last_dream_timestamp: m.layer6.last_dream_timestamp(),
            sessions_processed: m.layer6.sessions_processed(),
            consolidation_rate: m.layer6.consolidation_rate(),
        },
        layer7: Layer7Summary {
            spawned_count: m.layer7.spawned_count(),
            completed_count: m.layer7.completed_count(),
            failed_count: m.layer7.failed_count(),
            tool_denied_count: m.layer7.tool_denied_count(),
            total_tokens_used: m.layer7.total_tokens_used(),
            total_turns_used: m.layer7.total_turns_used(),
            max_turns: m.layer7.max_turns(),
            active_count: m.layer7.active_count(),
            avg_completion_time_ms: m.layer7.avg_completion_time_ms(),
            avg_tokens_per_agent: m.layer7.avg_tokens_per_agent(),
            avg_turns: m.layer7.avg_turns(),
            success_rate: m.layer7.success_rate(),
        },
        compact_circuit_breaker: CircuitBreakerSummary {
            state: compact_cb.state(),
            failures: compact_cb.failure_count(),
        },
        memory_extraction_circuit_breaker: CircuitBreakerSummary {
            state: extraction_cb.state(),
            failures: extraction_cb.failure_count(),
        },
        dream_circuit_breaker: CircuitBreakerSummary {
            state: dream_cb.state(),
            failures: dream_cb.failure_count(),
        },
    }
}

/// Reset all metrics to zero.
pub fn reset_metrics() {
    let m = get_memory_metrics();

    // Reset all layer metrics
    m.reset();

    // Reset all circuit breakers
    reset_all_circuit_breakers();

    tracing::info!(
        target: "blockcell.session_metrics",
        "All metrics counters and circuit breakers have been reset"
    );
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format metrics as a markdown table for CLI output.
pub fn format_metrics_table(summary: &MetricsSummary, layer_filter: Option<u8>) -> String {
    let mut output = String::new();

    output.push_str("```\n");
    output.push_str("╔═══════════════════════════════════════════════════════════════╗\n");
    output.push_str("║              BlockCell Memory Metrics Summary                 ║\n");
    output.push_str("╠═══════════════════════════════════════════════════════════════╣\n");

    // Layer 1
    if layer_filter.is_none() || layer_filter == Some(1) {
        output.push_str("║                                                               ║\n");
        output.push_str("║  📁 Layer 1: Tool Result Storage\n");
        output.push_str(&format!(
            "║  ├─ Persisted: {} files | Budget exceeded: {}\n",
            summary.layer1.persisted_count,
            summary.layer1.budget_exceeded_count
        ));
        output.push_str(&format!(
            "║  ├─ Size: {} → {} preview ({:.1}% compression)\n",
            format_bytes(summary.layer1.total_original_size),
            format_bytes(summary.layer1.total_preview_size),
            summary.layer1.average_compression * 100.0
        ));
        output.push_str(&format!(
            "║  └─ Limits: max_results={}, preview_limit={}\n",
            summary.layer1.max_tool_results,
            format_bytes(summary.layer1.preview_size_limit)
        ));
    }

    // Layer 2
    if layer_filter.is_none() || layer_filter == Some(2) {
        output.push_str("║                                                               ║\n");
        output.push_str("║  ⚡ Layer 2: Micro Compact\n");
        output.push_str(&format!(
            "║  ├─ Triggered: {} times | Cleared: {} | Kept: {}\n",
            summary.layer2.trigger_count,
            summary.layer2.cleared_count,
            summary.layer2.kept_count
        ));
        output.push_str(&format!(
            "║  └─ Config: gap={}min, keep_recent={}\n",
            summary.layer2.gap_threshold_minutes,
            summary.layer2.keep_recent
        ));
    }

    // Layer 3
    if layer_filter.is_none() || layer_filter == Some(3) {
        output.push_str("║                                                               ║\n");
        output.push_str("║  📝 Layer 3: Session Memory\n");
        output.push_str(&format!(
            "║  ├─ Extractions: {} | Loads: {}\n",
            summary.layer3.extraction_count,
            summary.layer3.load_count
        ));
        output.push_str(&format!(
            "║  ├─ Current: {} ({} sections)\n",
            format_bytes(summary.layer3.current_size),
            summary.layer3.section_count
        ));
        output.push_str(&format!(
            "║  └─ Limits: max_total={:.0}K tokens, max_section={} chars\n",
            summary.layer3.max_total_tokens as f64 / 1000.0,
            summary.layer3.max_section_length
        ));
    }

    // Layer 4 - 重点层
    if layer_filter.is_none() || layer_filter == Some(4) {
        let success_rate = if summary.layer4.compact_count > 0 {
            1.0 - (summary.layer4.failed_count as f64 / summary.layer4.compact_count as f64)
        } else {
            1.0
        };

        output.push_str("║                                                               ║\n");
        output.push_str("║  🗜️  Layer 4: Full Compact\n");
        // Token Budget 信息
        output.push_str(&format!(
            "║  ├─ Token Budget: {}\n",
            summary.layer4.token_budget
        ));
        output.push_str(&format!(
            "║  │   ├─ Threshold: {} ({:.0}%)\n",
            summary.layer4.threshold_tokens,
            summary.layer4.threshold_ratio * 100.0
        ));
        output.push_str(&format!(
            "║  │   ├─ Current: {} ({:.1}%)\n",
            summary.layer4.current_tokens,
            summary.layer4.usage_percentage
        ));
        output.push_str(&format!(
            "║  │   └─ Remaining: {}\n",
            summary.layer4.remaining_tokens
        ));
        output.push_str(&format!(
            "║  ├─ Compacts: {} (auto: {}, manual: {})\n",
            summary.layer4.compact_count,
            summary.layer4.auto_compact_count,
            summary.layer4.manual_compact_count
        ));
        output.push_str(&format!(
            "║  ├─ Failed: {} ({:.1}%)\n",
            summary.layer4.failed_count,
            (1.0 - success_rate) * 100.0
        ));
        output.push_str(&format!(
            "║  ├─ Avg compression: {:.1}%\n",
            summary.layer4.average_compression_ratio * 100.0
        ));
        output.push_str(&format!(
            "║  └─ Cache hit rate: {:.1}%\n",
            summary.layer4.cache_hit_rate * 100.0
        ));
    }

    // Layer 5
    if layer_filter.is_none() || layer_filter == Some(5) {
        output.push_str("║                                                               ║\n");
        output.push_str("║  🧠 Layer 5: Memory Extraction\n");
        output.push_str(&format!(
            "║  ├─ Extractions: {}\n",
            summary.layer5.extraction_count
        ));
        output.push_str(&format!(
            "║  ├─ Memories: user({})/project({})/feedback({})/ref({})\n",
            summary.layer5.user_memories,
            summary.layer5.project_memories,
            summary.layer5.feedback_memories,
            summary.layer5.reference_memories
        ));
        output.push_str(&format!(
            "║  ├─ Storage: {} total\n",
            format_bytes(summary.layer5.total_bytes_written)
        ));
        output.push_str(&format!(
            "║  └─ Config: min_msg={}, cooldown={}, max_file={:.0}K tokens\n",
            summary.layer5.min_messages,
            summary.layer5.cooldown_messages,
            summary.layer5.max_file_tokens as f64 / 1000.0
        ));
    }

    // Layer 6
    if layer_filter.is_none() || layer_filter == Some(6) {
        output.push_str("║                                                               ║\n");
        output.push_str("║  💤 Layer 6: Auto Dream\n");
        output.push_str(&format!(
            "║  ├─ Dream runs: {}\n",
            summary.layer6.dream_count
        ));
        output.push_str(&format!(
            "║  ├─ Memories: +{}/~{}/-{}\n",
            summary.layer6.memories_created,
            summary.layer6.memories_updated,
            summary.layer6.memories_deleted
        ));
        output.push_str(&format!(
            "║  ├─ Sessions processed: {} | Pruned: {}\n",
            summary.layer6.sessions_processed,
            summary.layer6.sessions_pruned
        ));
        output.push_str(&format!(
            "║  └─ Consolidation rate: {:.1}\n",
            summary.layer6.consolidation_rate
        ));
    }

    // Layer 7
    if layer_filter.is_none() || layer_filter == Some(7) {
        output.push_str("║                                                               ║\n");
        output.push_str("║  🤖 Layer 7: Forked Agent\n");
        output.push_str(&format!(
            "║  ├─ Spawned: {} | Active: {} | Completed: {} | Failed: {}\n",
            summary.layer7.spawned_count,
            summary.layer7.active_count,
            summary.layer7.completed_count,
            summary.layer7.failed_count
        ));
        output.push_str(&format!(
            "║  ├─ Success rate: {:.1}%\n",
            summary.layer7.success_rate * 100.0
        ));
        output.push_str(&format!(
            "║  ├─ Avg tokens: {:.0} | Avg turns: {:.1}\n",
            summary.layer7.avg_tokens_per_agent,
            summary.layer7.avg_turns
        ));
        output.push_str(&format!(
            "║  ├─ Avg duration: {:.1}s\n",
            summary.layer7.avg_completion_time_ms / 1000.0
        ));
        output.push_str(&format!(
            "║  └─ Tool denied: {}\n",
            summary.layer7.tool_denied_count
        ));
    }

    // Circuit Breakers (三层熔断器)
    output.push_str("║                                                               ║\n");
    output.push_str("║  🔌 Circuit Breakers (多层熔断器)\n");

    // 格式化单个熔断器状态
    let format_cb = |cb: &CircuitBreakerSummary, name: &str| -> String {
        let (icon, state_text, desc) = match cb.state {
            CircuitState::Open => ("○", "OPEN", "熔断中"),
            CircuitState::HalfOpen => ("◐", "HALF_OPEN", "半开"),
            CircuitState::Closed => ("●", "CLOSED", "正常"),
        };
        format!("║    {} {}: {} {} (失败: {})\n", icon, name, state_text, desc, cb.failures)
    };

    // Layer 4: Compact 熔断器
    output.push_str(&format_cb(&summary.compact_circuit_breaker, "L4-Compact"));
    // Layer 5: Memory Extraction 熔断器
    output.push_str(&format_cb(&summary.memory_extraction_circuit_breaker, "L5-Extract"));
    // Layer 6: Dream 熔断器
    output.push_str(&format_cb(&summary.dream_circuit_breaker, "L6-Dream"));

    output.push_str("║                                                               ║\n");
    output.push_str("╚═══════════════════════════════════════════════════════════════╝\n");
    output.push_str("```\n");

    output
}