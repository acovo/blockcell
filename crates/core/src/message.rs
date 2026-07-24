use serde::{Deserialize, Serialize};

use crate::session_key::build_session_key;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub sender_id: String,
    pub chat_id: String,
    pub content: String,
    #[serde(default)]
    pub media: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub timestamp_ms: i64,
}

impl InboundMessage {
    pub fn session_key(&self) -> String {
        let chat_id = match self.account_id.as_deref() {
            Some(account_id) => format!(
                "account:{}:{}{}",
                account_id.len(),
                account_id,
                self.chat_id
            ),
            None => self.chat_id.clone(),
        };
        build_session_key(&self.channel, &chat_id)
    }

    pub fn cli(content: &str) -> Self {
        Self {
            channel: "cli".to_string(),
            account_id: None,
            sender_id: "user".to_string(),
            chat_id: "default".to_string(),
            content: content.to_string(),
            media: vec![],
            metadata: serde_json::Value::Null,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn system(content: &str, origin_channel: &str, origin_chat_id: &str) -> Self {
        Self {
            channel: "system".to_string(),
            account_id: None,
            sender_id: "system".to_string(),
            chat_id: format!("{}:{}", origin_channel, origin_chat_id),
            content: content.to_string(),
            media: vec![],
            metadata: serde_json::Value::Null,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub chat_id: String,
    pub content: String,
    /// LLM 推理/思考内容（如 DeepSeek thinking mode）。
    /// 渠道可用此在 content 前插入换行分隔后拼接显示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub media: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// When true, `outbound_to_ws_bridge` should NOT forward this as
    /// `message_done` to WebSocket clients — the runtime already sent
    /// `message_done` directly via `event_tx`. Prevents duplicate messages.
    #[serde(default)]
    pub skip_ws_echo: bool,
}

impl OutboundMessage {
    pub fn new(channel: &str, chat_id: &str, content: &str) -> Self {
        Self {
            channel: channel.to_string(),
            account_id: None,
            chat_id: chat_id.to_string(),
            content: content.to_string(),
            reasoning_content: None,
            reply_to: None,
            media: vec![],
            metadata: serde_json::Value::Null,
            skip_ws_echo: false,
        }
    }

    /// 返回完整显示内容，将 reasoning（如有）拼接到 content 前面，
    /// 用换行分隔。**仅用于 CLI 打印 / debug / UI 展示辅助**，
    /// 不要用于 Telegram/Slack 等外部渠道发送（渠道应直接用 content）。
    pub fn display_content(&self) -> String {
        match &self.reasoning_content {
            Some(r) if !r.is_empty() => format!("{}\n{}", r, self.content),
            _ => self.content.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_id_roundtrip() {
        let inbound = InboundMessage {
            channel: "telegram".to_string(),
            account_id: Some("default".to_string()),
            sender_id: "u1".to_string(),
            chat_id: "c1".to_string(),
            content: "hello".to_string(),
            media: vec![],
            metadata: serde_json::json!({"k":"v"}),
            timestamp_ms: 1,
        };
        let json = serde_json::to_string(&inbound).expect("serialize inbound");
        let restored: InboundMessage = serde_json::from_str(&json).expect("deserialize inbound");
        assert_eq!(restored.account_id.as_deref(), Some("default"));
        assert_ne!(restored.session_key(), "telegram:c1");

        let mut other_account = restored.clone();
        other_account.account_id = Some("secondary".to_string());
        assert_ne!(restored.session_key(), other_account.session_key());

        let mut legacy = restored.clone();
        legacy.account_id = None;
        assert_eq!(legacy.session_key(), "telegram:c1");

        let mut outbound = OutboundMessage::new("telegram", "c1", "ok");
        outbound.account_id = Some("default".to_string());
        let out_json = serde_json::to_string(&outbound).expect("serialize outbound");
        let out_restored: OutboundMessage =
            serde_json::from_str(&out_json).expect("deserialize outbound");
        assert_eq!(out_restored.account_id.as_deref(), Some("default"));
    }
}
