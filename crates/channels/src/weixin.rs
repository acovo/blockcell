use base64::Engine;
use crate::account::weixin_account_id;
use blockcell_core::{Config, Error, InboundMessage, Result};
use reqwest::Client;
use reqwest::Proxy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

const WEIXIN_API_BASE: &str = "https://ilinkai.weixin.qq.com";

// ── API data structures ──

#[derive(Debug, Serialize)]
struct GetUpdatesRequest {
    get_updates_buf: String,
}

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    ret: Option<i32>,
    errcode: Option<i32>,
    msgs: Option<Vec<WeixinMessage>>,
    get_updates_buf: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WeixinMessage {
    message_type: i32,
    from_user_id: String,
    #[allow(dead_code)]
    to_user_id: String,
    context_token: Option<String>,
    item_list: Option<Vec<MessageItem>>,
}

#[derive(Debug, Deserialize)]
struct MessageItem {
    #[serde(rename = "type")]
    item_type: i32,
    text_item: Option<TextItem>,
    voice_item: Option<VoiceItem>,
    ref_msg: Option<RefMsg>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TextItem {
    text: String,
}

#[derive(Debug, Deserialize)]
struct VoiceItem {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RefMsg {
    title: Option<String>,
}

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    msg: SendMessageBody,
}

#[derive(Debug, Serialize)]
struct SendMessageBody {
    from_user_id: String,
    to_user_id: String,
    client_id: String,
    message_type: i32,
    message_state: i32,
    item_list: Vec<SendMessageItem>,
    context_token: String,
}

#[derive(Debug, Serialize)]
struct SendMessageItem {
    #[serde(rename = "type")]
    item_type: i32,
    text_item: TextItem,
}

// ── QR code login structs ──

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct QrCodeResponse {
    #[allow(dead_code)]
    qrcode: Option<String>,
    qrcode_img_content: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct QrCodeStatusResponse {
    status: Option<String>,
    bot_token: Option<String>,
    #[allow(dead_code)]
    ilink_bot_id: Option<String>,
    #[allow(dead_code)]
    baseurl: Option<String>,
    #[allow(dead_code)]
    ilink_user_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WeixinLoginQrCode {
    pub qrcode: String,
    pub qrcode_img_content: String,
}

#[derive(Debug, Clone)]
pub struct WeixinLoginStatus {
    pub status: String,
    pub bot_token: Option<String>,
    pub ilink_bot_id: Option<String>,
    pub baseurl: Option<String>,
    pub ilink_user_id: Option<String>,
}

// ── Helpers ──

fn generate_uin() -> String {
    let num: u32 = rand_u32();
    base64::engine::general_purpose::STANDARD.encode(num.to_string())
}

fn rand_u32() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let s = RandomState::new();
    let mut h = s.build_hasher();
    h.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    h.finish() as u32
}

fn generate_client_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rnd = rand_u32();
    format!("wcc-{}-{:08x}", ts, rnd)
}

// ── Channel implementation ──

pub struct WeixinChannel {
    config: Config,
    client: Client,
    inbound_tx: mpsc::Sender<InboundMessage>,
}

impl WeixinChannel {
    pub fn new(config: Config, inbound_tx: mpsc::Sender<InboundMessage>) -> Self {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(40))
            .connect_timeout(Duration::from_secs(10));

        if let Some(proxy_url) = config.channels.weixin.proxy.as_deref() {
            if !proxy_url.is_empty() {
                match Proxy::all(proxy_url) {
                    Ok(proxy) => {
                        builder = builder.proxy(proxy);
                        info!(proxy = %proxy_url, "Weixin: using proxy");
                    }
                    Err(e) => {
                        error!(error = %e, proxy = %proxy_url, "Weixin: invalid proxy, ignoring");
                    }
                }
            }
        }

        let client = builder.build().unwrap_or_else(|_| Client::new());

        Self {
            config,
            client,
            inbound_tx,
        }
    }

    pub async fn run_loop(self: Arc<Self>, mut shutdown: tokio::sync::broadcast::Receiver<()>) {
        let token = self.config.channels.weixin.token.trim();
        if token.is_empty() {
            warn!("Weixin: token is empty, channel will not start");
            return;
        }

        info!("Weixin: starting message polling loop");

        let mut get_updates_buf = String::new();
        let mut continuous_failures: u32 = 0;
        const MAX_CONTINUOUS_FAILURES: u32 = 3;

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Weixin: shutdown signal received");
                    break;
                }
                result = self.get_updates(&get_updates_buf) => {
                    match result {
                        Ok(response) => {
                            continuous_failures = 0;

                            if let Some(new_buf) = response.get_updates_buf {
                                if !new_buf.is_empty() {
                                    get_updates_buf = new_buf;
                                }
                            }

                            let ret = response.ret.unwrap_or(0);
                            let errcode = response.errcode.unwrap_or(0);
                            if ret != 0 || errcode != 0 {
                                warn!(ret, errcode, "Weixin: getupdates returned non-zero code");
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                continue;
                            }

                            if let Some(msgs) = response.msgs {
                                for msg in msgs {
                                    if msg.message_type != 1 {
                                        continue;
                                    }
                                    if let Err(e) = self.handle_message(&msg).await {
                                        error!(error = %e, "Weixin: failed to handle message");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            continuous_failures += 1;
                            if continuous_failures >= MAX_CONTINUOUS_FAILURES {
                                error!(
                                    error = %e,
                                    failures = continuous_failures,
                                    "Weixin: max continuous failures reached, backing off 30s"
                                );
                                tokio::time::sleep(Duration::from_secs(30)).await;
                                continuous_failures = 0;
                            } else {
                                warn!(
                                    error = %e,
                                    failures = continuous_failures,
                                    "Weixin: getupdates failed, retrying in 2s"
                                );
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                }
            }
        }

        info!("Weixin: polling loop stopped");
    }

    async fn get_updates(&self, sync_buf: &str) -> Result<GetUpdatesResponse> {
        let token = &self.config.channels.weixin.token;
        let body = GetUpdatesRequest {
            get_updates_buf: sync_buf.to_string(),
        };

        let resp = self
            .client
            .post(format!("{}/ilink/bot/getupdates", WEIXIN_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .header("AuthorizationType", "ilink_bot_token")
            .header("X-WECHAT-UIN", generate_uin())
            .json(&body)
            .timeout(Duration::from_secs(35))
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Weixin getupdates request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Channel(format!(
                "Weixin getupdates HTTP {}: {}",
                status, text
            )));
        }

        resp.json::<GetUpdatesResponse>()
            .await
            .map_err(|e| Error::Channel(format!("Weixin getupdates parse error: {}", e)))
    }

    async fn handle_message(&self, msg: &WeixinMessage) -> Result<()> {
        let from = &msg.from_user_id;
        let context_token = msg.context_token.clone().unwrap_or_default();

        if !self.is_allowed(from) {
            debug!(from = %from, "Weixin: message from non-allowed user, ignoring");
            return Ok(());
        }

        let mut text_parts: Vec<String> = Vec::new();

        if let Some(items) = &msg.item_list {
            for item in items {
                match item.item_type {
                    1 => {
                        // Text message
                        if let Some(text_item) = &item.text_item {
                            if !text_item.text.is_empty() {
                                text_parts.push(text_item.text.clone());
                            }
                        }
                    }
                    3 => {
                        // Voice message — use transcribed text
                        if let Some(voice_item) = &item.voice_item {
                            if let Some(text) = &voice_item.text {
                                if !text.is_empty() {
                                    text_parts.push(format!("[语音] {}", text));
                                }
                            }
                        }
                    }
                    _ => {
                        debug!(item_type = item.item_type, "Weixin: unsupported item type");
                    }
                }

                // Append referenced message if present
                if let Some(ref_msg) = &item.ref_msg {
                    if let Some(title) = &ref_msg.title {
                        if !title.is_empty() {
                            text_parts.push(format!("[引用] {}", title));
                        }
                    }
                }
            }
        }

        let content = text_parts.join("\n");
        if content.is_empty() {
            debug!(from = %from, "Weixin: empty message content, ignoring");
            return Ok(());
        }

        let account_id = weixin_account_id(&self.config);

        info!(from = %from, len = content.len(), "Weixin: received message");

        let inbound = InboundMessage {
            channel: "weixin".to_string(),
            account_id,
            sender_id: from.to_string(),
            chat_id: from.to_string(),
            content,
            media: vec![],
            metadata: serde_json::json!({ "context_token": context_token }),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };

        self.inbound_tx
            .send(inbound)
            .await
            .map_err(|e| Error::Channel(format!("Weixin: failed to send inbound: {}", e)))?;

        Ok(())
    }

    fn is_allowed(&self, from: &str) -> bool {
        let allow_from = &self.config.channels.weixin.allow_from;
        if allow_from.is_empty() {
            return true;
        }
        allow_from.iter().any(|allowed| allowed == from)
    }
}

// ── Public send functions ──

pub async fn send_message(
    config: &Config,
    chat_id: &str,
    text: &str,
) -> Result<()> {
    let token = &config.channels.weixin.token;
    if token.is_empty() {
        return Err(Error::Channel("Weixin: token not configured".to_string()));
    }

    let client = build_send_client(config)?;

    // Split long messages at ~2000 chars to avoid potential API limits
    let chunks = split_message(text, 2000);
    for chunk in &chunks {
        send_text_chunk(&client, token, chat_id, chunk, "").await?;
        if chunks.len() > 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    Ok(())
}

pub async fn send_message_with_context(
    config: &Config,
    chat_id: &str,
    text: &str,
    context_token: &str,
) -> Result<()> {
    let token = &config.channels.weixin.token;
    if token.is_empty() {
        return Err(Error::Channel("Weixin: token not configured".to_string()));
    }

    let client = build_send_client(config)?;

    let chunks = split_message(text, 2000);
    for chunk in &chunks {
        send_text_chunk(&client, token, chat_id, chunk, context_token).await?;
        if chunks.len() > 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    Ok(())
}

pub async fn fetch_login_qrcode(config: &Config) -> Result<WeixinLoginQrCode> {
    let client = build_client(config, Duration::from_secs(15))?;

    let resp = client
        .get(format!("{}/ilink/bot/get_bot_qrcode?bot_type=3", WEIXIN_API_BASE))
        .header("X-WECHAT-UIN", generate_uin())
        .send()
        .await
        .map_err(|e| Error::Channel(format!("Weixin get_bot_qrcode failed: {}", e)))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::Channel(format!(
            "Weixin get_bot_qrcode HTTP {}: {}",
            status, text
        )));
    }

    let payload = resp
        .json::<QrCodeResponse>()
        .await
        .map_err(|e| Error::Channel(format!("Weixin get_bot_qrcode parse error: {}", e)))?;

    let qrcode = payload
        .qrcode
        .unwrap_or_default()
        .trim()
        .to_string();
    let qrcode_img_content = payload
        .qrcode_img_content
        .unwrap_or_default()
        .trim()
        .to_string();

    if qrcode.is_empty() || qrcode_img_content.is_empty() {
        return Err(Error::Channel(
            "Weixin get_bot_qrcode returned an empty qrcode payload".to_string(),
        ));
    }

    Ok(WeixinLoginQrCode {
        qrcode,
        qrcode_img_content,
    })
}

pub async fn poll_login_status(config: &Config, qrcode: &str) -> Result<WeixinLoginStatus> {
    let client = build_client(config, Duration::from_secs(35))?;

    let resp = client
        .get(format!("{}/ilink/bot/get_qrcode_status", WEIXIN_API_BASE))
        .query(&[("qrcode", qrcode)])
        .header("iLink-App-ClientVersion", "1")
        .header("X-WECHAT-UIN", generate_uin())
        .send()
        .await
        .map_err(|e| Error::Channel(format!("Weixin get_qrcode_status failed: {}", e)))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::Channel(format!(
            "Weixin get_qrcode_status HTTP {}: {}",
            status, text
        )));
    }

    let payload = resp
        .json::<QrCodeStatusResponse>()
        .await
        .map_err(|e| Error::Channel(format!("Weixin get_qrcode_status parse error: {}", e)))?;

    Ok(WeixinLoginStatus {
        status: payload.status.unwrap_or_else(|| "unknown".to_string()),
        bot_token: payload.bot_token,
        ilink_bot_id: payload.ilink_bot_id,
        baseurl: payload.baseurl,
        ilink_user_id: payload.ilink_user_id,
    })
}

fn build_client(config: &Config, timeout: Duration) -> Result<Client> {
    let mut builder = Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10));

    if let Some(proxy_url) = config.channels.weixin.proxy.as_deref() {
        if !proxy_url.is_empty() {
            match Proxy::all(proxy_url) {
                Ok(proxy) => {
                    builder = builder.proxy(proxy);
                }
                Err(e) => {
                    warn!(error = %e, "Weixin: invalid proxy for send client");
                }
            }
        }
    }

    builder
        .build()
        .map_err(|e| Error::Channel(format!("Weixin: failed to build send client: {}", e)))
}

fn build_send_client(config: &Config) -> Result<Client> {
    build_client(config, Duration::from_secs(15))
}

async fn send_text_chunk(
    client: &Client,
    token: &str,
    to_user_id: &str,
    text: &str,
    context_token: &str,
) -> Result<()> {
    let body = SendMessageRequest {
        msg: SendMessageBody {
            from_user_id: String::new(),
            to_user_id: to_user_id.to_string(),
            client_id: generate_client_id(),
            message_type: 2,
            message_state: 2,
            item_list: vec![SendMessageItem {
                item_type: 1,
                text_item: TextItem {
                    text: text.to_string(),
                },
            }],
            context_token: context_token.to_string(),
        },
    };

    let resp = client
        .post(format!("{}/ilink/bot/sendmessage", WEIXIN_API_BASE))
        .header("Authorization", format!("Bearer {}", token))
        .header("AuthorizationType", "ilink_bot_token")
        .header("X-WECHAT-UIN", generate_uin())
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Channel(format!("Weixin sendmessage failed: {}", e)))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::Channel(format!(
            "Weixin sendmessage HTTP {}: {}",
            status, text
        )));
    }

    debug!(to = %to_user_id, "Weixin: message sent");
    Ok(())
}

fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to split at newline boundary
        let split_at = remaining[..max_len]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(max_len);

        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_uin_is_base64() {
        let uin = generate_uin();
        assert!(!uin.is_empty());
        // Should be valid base64
        assert!(base64::engine::general_purpose::STANDARD
            .decode(&uin)
            .is_ok());
    }

    #[test]
    fn test_generate_client_id_format() {
        let id = generate_client_id();
        assert!(id.starts_with("wcc-"));
        let parts: Vec<&str> = id.splitn(3, '-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "wcc");
        // timestamp part should be numeric
        assert!(parts[1].chars().all(|c| c.is_ascii_digit()));
        // hex part
        assert!(parts[2].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_split_message_short() {
        let chunks = split_message("hello", 100);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_split_message_long() {
        let text = "line1\nline2\nline3\nline4";
        let chunks = split_message(text, 12);
        assert!(chunks.len() >= 2);
        let joined: String = chunks.join("");
        assert_eq!(joined, text);
    }

    #[test]
    fn test_send_message_request_serialization() {
        let req = SendMessageRequest {
            msg: SendMessageBody {
                from_user_id: String::new(),
                to_user_id: "wxid_test".to_string(),
                client_id: "wcc-123-abcd".to_string(),
                message_type: 2,
                message_state: 2,
                item_list: vec![SendMessageItem {
                    item_type: 1,
                    text_item: TextItem {
                        text: "hello".to_string(),
                    },
                }],
                context_token: "ctx_123".to_string(),
            },
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["msg"]["message_type"], 2);
        assert_eq!(json["msg"]["to_user_id"], "wxid_test");
        assert_eq!(json["msg"]["item_list"][0]["type"], 1);
        assert_eq!(json["msg"]["item_list"][0]["text_item"]["text"], "hello");
    }

    #[test]
    fn test_get_updates_response_deserialization() {
        let json = r#"{
            "ret": 0,
            "errcode": 0,
            "msgs": [
                {
                    "message_type": 1,
                    "from_user_id": "wxid_abc",
                    "to_user_id": "bot_id",
                    "context_token": "ctx_xyz",
                    "item_list": [
                        {
                            "type": 1,
                            "text_item": { "text": "hello" }
                        }
                    ]
                }
            ],
            "get_updates_buf": "new_buf"
        }"#;

        let resp: GetUpdatesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.ret, Some(0));
        assert_eq!(resp.errcode, Some(0));
        let msgs = resp.msgs.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, 1);
        assert_eq!(msgs[0].from_user_id, "wxid_abc");
        assert_eq!(msgs[0].context_token.as_deref(), Some("ctx_xyz"));
        let items = msgs[0].item_list.as_ref().unwrap();
        assert_eq!(items[0].item_type, 1);
        assert_eq!(items[0].text_item.as_ref().unwrap().text, "hello");
        assert_eq!(resp.get_updates_buf.as_deref(), Some("new_buf"));
    }
}
