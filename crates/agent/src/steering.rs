use blockcell_core::{build_session_key, InboundMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct ActiveConversationKey {
    pub agent_id: String,
    pub session_key: String,
}

impl ActiveConversationKey {
    pub fn from_message(agent_id: &str, msg: &InboundMessage) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            session_key: msg.session_key(),
        }
    }

    pub fn from_channel_chat(agent_id: &str, channel: &str, chat_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            session_key: build_session_key(channel, chat_id),
        }
    }
}

pub type SteeringSessionKey = ActiveConversationKey;

pub type SteeringRegistry = Arc<Mutex<HashMap<SteeringSessionKey, SteeringSender>>>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SteeringMessage {
    pub content: String,
    pub channel: String,
    pub chat_id: String,
}

pub struct SteeringChannel {
    rx: mpsc::Receiver<SteeringMessage>,
}

#[derive(Clone)]
pub struct SteeringSender {
    tx: mpsc::Sender<SteeringMessage>,
}

impl SteeringChannel {
    pub fn new(buffer_size: usize) -> (Self, SteeringSender) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (Self { rx }, SteeringSender { tx })
    }

    pub fn try_recv(&mut self) -> Option<SteeringMessage> {
        self.rx.try_recv().ok()
    }

    pub fn drain(&mut self) -> Vec<SteeringMessage> {
        let mut messages = Vec::new();
        while let Ok(message) = self.rx.try_recv() {
            messages.push(message);
        }
        messages
    }

    pub fn has_pending(&self) -> bool {
        !self.rx.is_empty()
    }
}

impl SteeringSender {
    pub async fn send(
        &self,
        message: SteeringMessage,
    ) -> std::result::Result<(), mpsc::error::SendError<SteeringMessage>> {
        self.tx.send(message).await
    }

    pub fn try_send(
        &self,
        message: SteeringMessage,
    ) -> std::result::Result<(), mpsc::error::TrySendError<SteeringMessage>> {
        self.tx.try_send(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockcell_core::InboundMessage;

    fn inbound(channel: &str, account_id: Option<&str>, chat_id: &str) -> InboundMessage {
        InboundMessage {
            channel: channel.to_string(),
            account_id: account_id.map(str::to_string),
            sender_id: "user".to_string(),
            chat_id: chat_id.to_string(),
            content: "hello".to_string(),
            media: vec![],
            metadata: serde_json::Value::Null,
            timestamp_ms: 1,
        }
    }

    #[test]
    fn active_conversation_key_separates_channels_with_same_chat_id() {
        let ws = inbound("ws", None, "shared-chat");
        let telegram = inbound("telegram", None, "shared-chat");

        assert_ne!(
            ActiveConversationKey::from_message("default", &ws),
            ActiveConversationKey::from_message("default", &telegram)
        );
    }

    #[test]
    fn active_conversation_key_separates_accounts_with_same_channel_and_chat_id() {
        let primary = inbound("telegram", Some("primary"), "shared-chat");
        let secondary = inbound("telegram", Some("secondary"), "shared-chat");

        assert_ne!(
            ActiveConversationKey::from_message("default", &primary),
            ActiveConversationKey::from_message("default", &secondary)
        );
    }

    fn message(content: &str) -> SteeringMessage {
        SteeringMessage {
            content: content.to_string(),
            channel: "ws".to_string(),
            chat_id: "chat-1".to_string(),
        }
    }

    #[test]
    fn drain_returns_pending_messages_in_send_order() {
        let (mut channel, sender) = SteeringChannel::new(4);

        sender.try_send(message("first")).expect("send first");
        sender.try_send(message("second")).expect("send second");

        assert!(channel.has_pending());
        let drained = channel.drain();

        assert_eq!(drained, vec![message("first"), message("second")]);
        assert!(!channel.has_pending());
        assert!(channel.drain().is_empty());
    }

    #[test]
    fn try_recv_returns_none_when_empty() {
        let (mut channel, _sender) = SteeringChannel::new(1);

        assert_eq!(channel.try_recv(), None);
        assert!(!channel.has_pending());
    }

    #[tokio::test]
    async fn send_reports_closed_channel() {
        let (channel, sender) = SteeringChannel::new(1);
        drop(channel);

        let result = sender.send(message("closed")).await;

        assert!(result.is_err());
    }
}
