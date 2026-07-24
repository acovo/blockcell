use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventScope {
    Global,
    MainSession,
    Channel {
        channel: String,
        chat_id: String,
    },
    Session {
        channel: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        chat_id: String,
        session_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryPolicy {
    pub immediate: bool,
    pub include_in_summary: bool,
    pub notify_user: bool,
    pub persist: bool,
    pub max_delay_seconds: Option<u64>,
}

impl Default for DeliveryPolicy {
    fn default() -> Self {
        Self {
            immediate: false,
            include_in_summary: true,
            notify_user: true,
            persist: true,
            max_delay_seconds: None,
        }
    }
}

impl DeliveryPolicy {
    pub fn critical() -> Self {
        Self {
            immediate: true,
            ..Self::default()
        }
    }

    pub fn silent() -> Self {
        Self {
            immediate: false,
            include_in_summary: false,
            notify_user: false,
            persist: true,
            max_delay_seconds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemEvent {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub scope: EventScope,
    pub priority: EventPriority,
    pub title: String,
    pub summary: String,
    pub details: Value,
    pub created_at_ms: i64,
    pub correlation_id: Option<String>,
    pub dedup_key: Option<String>,
    pub delivery: DeliveryPolicy,
    pub delivered: bool,
    pub acked: bool,
}

impl SystemEvent {
    pub fn new_main_session(
        kind: impl Into<String>,
        source: impl Into<String>,
        priority: EventPriority,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("evt_{}", Uuid::new_v4()),
            kind: kind.into(),
            source: source.into(),
            scope: EventScope::MainSession,
            priority,
            title: title.into(),
            summary: summary.into(),
            details: Value::Null,
            created_at_ms: Utc::now().timestamp_millis(),
            correlation_id: None,
            dedup_key: None,
            delivery: DeliveryPolicy::default(),
            delivered: false,
            acked: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryScope {
    MainSession,
    Channel {
        channel: String,
        chat_id: String,
    },
    Session {
        channel: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        chat_id: String,
        session_key: String,
    },
    User {
        user_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryCategory {
    Task,
    Cron,
    Ghost,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryItem {
    pub id: String,
    pub scope: SummaryScope,
    pub category: SummaryCategory,
    pub title: String,
    pub body: String,
    pub source_event_ids: Vec<String>,
    pub created_at_ms: i64,
    pub priority: EventPriority,
    pub merge_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub title: String,
    pub items: Vec<SummaryItem>,
    pub compact_text: String,
}
