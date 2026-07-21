//! 通道（Channels）配置类型
//!
//! 包含 WhatsApp, Telegram, QQ, NapCat, WeCom, 微信 等通道的配置定义。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_whatsapp_bridge_url")]
    pub bridge_url: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TelegramAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeishuAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub encrypt_key: String,
    #[serde(default)]
    pub verification_token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SlackAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub app_token: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default = "default_slack_poll_interval")]
    pub poll_interval_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscordAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DingTalkAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_key: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub robot_code: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LarkAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub encrypt_key: String,
    #[serde(default)]
    pub verification_token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeComAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_wecom_mode")]
    pub mode: String,
    #[serde(default)]
    pub corp_id: String,
    #[serde(default)]
    pub corp_secret: String,
    #[serde(default)]
    pub agent_id: i64,
    #[serde(default)]
    pub bot_id: String,
    #[serde(default)]
    pub bot_secret: String,
    #[serde(default)]
    pub callback_token: String,
    #[serde(default)]
    pub encoding_aes_key: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default = "default_wecom_poll_interval")]
    pub poll_interval_secs: u32,
    #[serde(default = "default_wecom_ws_url")]
    pub ws_url: String,
    #[serde(default = "default_wecom_ping_interval")]
    pub ping_interval_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_whatsapp_bridge_url")]
    pub bridge_url: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    /// Multi-account config map. Key is account_id.
    #[serde(default)]
    pub accounts: HashMap<String, WhatsAppAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bridge_url: default_whatsapp_bridge_url(),
            allow_from: Vec::new(),
            accounts: HashMap::new(),
            default_account_id: None,
        }
    }
}

fn default_whatsapp_bridge_url() -> String {
    "ws://localhost:3001".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub accounts: HashMap<String, TelegramAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub encrypt_key: String,
    #[serde(default)]
    pub verification_token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub accounts: HashMap<String, FeishuAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SlackConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub app_token: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default = "default_slack_poll_interval")]
    pub poll_interval_secs: u32,
    #[serde(default)]
    pub accounts: HashMap<String, SlackAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

fn default_slack_poll_interval() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscordConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub accounts: HashMap<String, DiscordAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

/// 钉钉 (DingTalk) channel configuration.
/// Uses DingTalk Stream SDK for real-time message reception.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DingTalkConfig {
    #[serde(default)]
    pub enabled: bool,
    /// DingTalk app key (AppKey from the developer console)
    #[serde(default)]
    pub app_key: String,
    /// DingTalk app secret (AppSecret from the developer console)
    #[serde(default)]
    pub app_secret: String,
    /// Optional: robot code for sending messages to users
    #[serde(default)]
    pub robot_code: String,
    /// Allowlist of sender user IDs. Empty = allow all.
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub accounts: HashMap<String, DingTalkAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

/// Lark (international Feishu) channel configuration.
/// Uses the same WebSocket long-connection protocol as Feishu,
/// but connects to open.larksuite.com instead of open.feishu.cn.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LarkConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub encrypt_key: String,
    #[serde(default)]
    pub verification_token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub accounts: HashMap<String, LarkAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

/// QQ Official Bot channel configuration.
/// Uses Tencent's official QQ Bot API with OAuth2 authentication
/// and a Discord-like WebSocket gateway protocol.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QQConfig {
    #[serde(default)]
    pub enabled: bool,
    /// QQ Bot App ID
    #[serde(default)]
    pub app_id: String,
    /// QQ Bot App Secret
    #[serde(default)]
    pub app_secret: String,
    /// API environment: production or sandbox
    #[serde(default)]
    pub environment: String,
    /// Connection mode: "websocket" (default, no public IP needed) or "webhook" (requires public URL)
    #[serde(default)]
    pub mode: String,
    /// Allowlist of user IDs. Empty = allow all.
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub accounts: HashMap<String, QQAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QQAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// NapCatQQ channel configuration.
/// Implements OneBot 11 protocol with WebSocket client/server and HTTP client/server support.
/// NapCatQQ is a community-driven QQ bot protocol implementation.
///
/// # Connection Modes
///
/// | Mode | BlockCell Role | NapCatQQ Config Key | Description |
/// |------|----------------|---------------------|-------------|
/// | `ws-client` | WebSocket Client | `websocketServers` | BlockCell connects to NapCatQQ WS server |
/// | `ws-server` | WebSocket Server | `websocketClients` | NapCatQQ connects to BlockCell WS server |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NapCatConfig {
    /// Whether the channel is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Connection mode:
    /// - "ws-client": BlockCell connects to NapCatQQ WebSocket server (default)
    /// - "ws-server": NapCatQQ connects to BlockCell WebSocket server
    #[serde(default = "default_napcat_mode")]
    pub mode: String,

    // =========================================================================
    // WebSocket Client Mode Configuration
    // =========================================================================
    /// NapCatQQ WebSocket URL (ws-client mode).
    /// Example: "ws://127.0.0.1:3001"
    #[serde(default)]
    pub ws_url: String,

    // =========================================================================
    // WebSocket Server Mode Configuration
    // =========================================================================
    /// WebSocket server host (ws-server mode).
    /// Default: "127.0.0.1"
    #[serde(default = "default_napcat_server_host")]
    pub server_host: String,
    /// WebSocket server port (ws-server mode).
    /// Default: 8080
    #[serde(default = "default_napcat_server_port")]
    pub server_port: u16,
    /// WebSocket server path (ws-server mode).
    /// Default: "/onebot/v11/ws"
    #[serde(default = "default_napcat_server_path")]
    pub server_path: String,

    // =========================================================================
    // Authentication & Access Control
    // =========================================================================
    /// Access token for authentication.
    /// Must match the token configured in NapCatQQ.
    #[serde(default)]
    pub access_token: String,
    /// Allowlist of user IDs (QQ numbers). Empty = allow all.
    /// Supports: specific QQ numbers, or "*" for all.
    #[serde(default)]
    pub allow_from: Vec<String>,
    /// Allowlist of group IDs. Empty = allow all groups.
    /// When specified, only messages from these groups are processed.
    #[serde(default)]
    pub allow_groups: Vec<String>,
    /// Blocklist of user IDs. Takes precedence over allow_from.
    #[serde(default)]
    pub block_from: Vec<String>,

    /// Group message response mode.
    /// - "none": Do not respond to any group messages
    /// - "at_only": Only respond when bot is @mentioned
    /// - "all": Respond to all group messages (default)
    #[serde(default = "default_group_response_mode")]
    pub group_response_mode: String,

    // =========================================================================
    // Connection Settings
    // =========================================================================
    /// Heartbeat interval in seconds.
    /// Default: 30
    #[serde(default = "default_napcat_heartbeat_interval")]
    pub heartbeat_interval_secs: u32,
    /// Reconnect delay in seconds (exponential backoff base).
    /// Default: 5
    #[serde(default = "default_napcat_reconnect_delay")]
    pub reconnect_delay_secs: u32,

    // =========================================================================
    // Multi-Account & Admin
    // =========================================================================
    /// Multi-account configuration.
    #[serde(default)]
    pub accounts: HashMap<String, NapCatAccountConfig>,
    /// Default account ID for outbound messages.
    #[serde(default)]
    pub default_account_id: Option<String>,
    /// Admin operation permissions configuration.
    #[serde(default)]
    pub admin_permissions: NapCatAdminPermissions,

    // =========================================================================
    // Media Auto-Download Configuration
    // =========================================================================
    /// Whether to automatically download media (images, voice, video, files)
    /// when receiving messages. Default: true.
    /// When enabled, media will be downloaded before the message reaches LLM,
    /// and the local path will be attached to the message.
    #[serde(default = "default_auto_download_media")]
    pub auto_download_media: bool,

    /// Directory to save downloaded media (relative to workspace).
    /// Default: "downloads"
    #[serde(default = "default_media_download_dir")]
    pub media_download_dir: String,

    /// Maximum file size for auto-download in bytes.
    /// Files larger than this will not be auto-downloaded.
    /// Default: 10MB (10 * 1024 * 1024 = 10485760)
    #[serde(default = "default_max_auto_download_size")]
    pub max_auto_download_size: u64,
}

fn default_auto_download_media() -> bool {
    true
}

fn default_media_download_dir() -> String {
    "downloads".to_string()
}

fn default_max_auto_download_size() -> u64 {
    10 * 1024 * 1024 // 10MB
}

fn default_napcat_mode() -> String {
    "ws-client".to_string()
}

fn default_napcat_server_host() -> String {
    "127.0.0.1".to_string()
}

fn default_napcat_server_port() -> u16 {
    13005 // NapCatQQ client 默认连接 ws://localhost:13005
}

fn default_napcat_server_path() -> String {
    "/".to_string() // NapCatQQ client 默认连接 ws://localhost:13005，路径为 /
}

fn default_napcat_heartbeat_interval() -> u32 {
    30
}

fn default_napcat_reconnect_delay() -> u32 {
    5
}

fn default_group_response_mode() -> String {
    "all".to_string()
}

/// NapCatQQ admin operation permissions configuration.
/// Controls who can execute management operations via LLM tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NapCatAdminPermissions {
    /// Allowed admin user IDs (QQ numbers).
    /// Supports: specific QQ numbers, or "*" for all users.
    /// Inherits from allow_from if empty.
    #[serde(default)]
    pub allowed_admins: Vec<String>,

    /// Allowed group IDs for admin operations.
    /// Only users in these groups can execute admin operations.
    /// Empty = all groups allowed.
    #[serde(default)]
    pub allowed_groups: Vec<String>,

    /// Default policy: "allow" or "deny".
    /// Default: "deny" - users not in allowed_admins are denied.
    #[serde(default = "default_admin_policy")]
    pub default_policy: String,

    /// Tool-specific permission overrides.
    /// Key: tool name (e.g., "napcat_set_group_kick")
    /// Value: permission configuration for that tool.
    #[serde(default)]
    pub tool_overrides: HashMap<String, ToolPermissionOverride>,

    /// Tools that require confirmation before execution.
    #[serde(default)]
    pub require_confirmation: Vec<String>,
}

fn default_admin_policy() -> String {
    "deny".to_string()
}

impl Default for NapCatAdminPermissions {
    fn default() -> Self {
        Self {
            allowed_admins: Vec::new(),
            allowed_groups: Vec::new(),
            default_policy: default_admin_policy(),
            tool_overrides: HashMap::new(),
            require_confirmation: Vec::new(),
        }
    }
}

/// Tool-specific permission override configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionOverride {
    /// Override allowed admin user IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_admins: Option<Vec<String>>,

    /// Override allowed group IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_groups: Option<Vec<String>>,

    /// Override default policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_policy: Option<String>,

    /// Whether this tool requires confirmation.
    #[serde(default)]
    pub require_confirmation: bool,

    /// Required role: "owner", "admin", or "member".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_role: Option<String>,
}

impl Default for NapCatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_napcat_mode(),
            ws_url: String::new(),
            server_host: default_napcat_server_host(),
            server_port: default_napcat_server_port(),
            server_path: default_napcat_server_path(),
            access_token: String::new(),
            allow_from: Vec::new(),
            allow_groups: Vec::new(),
            block_from: Vec::new(),
            group_response_mode: default_group_response_mode(),
            heartbeat_interval_secs: default_napcat_heartbeat_interval(),
            reconnect_delay_secs: default_napcat_reconnect_delay(),
            accounts: HashMap::new(),
            default_account_id: None,
            admin_permissions: NapCatAdminPermissions::default(),
            auto_download_media: true,
            media_download_dir: default_media_download_dir(),
            max_auto_download_size: default_max_auto_download_size(),
        }
    }
}

/// NapCatQQ account configuration for multi-account support.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NapCatAccountConfig {
    /// Whether this account is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Connection mode for this account.
    /// Overrides parent NapCatConfig.mode if set.
    #[serde(default)]
    pub mode: Option<String>,
    /// WebSocket URL for this account (ws-client mode).
    /// Overrides parent NapCatConfig.ws_url if set.
    #[serde(default)]
    pub ws_url: Option<String>,
    /// Access token for this account.
    /// Overrides parent NapCatConfig.access_token if set.
    #[serde(default)]
    pub access_token: Option<String>,
    /// Allowlist of user IDs for this account.
    /// Overrides parent NapCatConfig.allow_from if set.
    #[serde(default)]
    pub allow_from: Option<Vec<String>>,
    /// Allowlist of group IDs for this account.
    /// Overrides parent NapCatConfig.allow_groups if set.
    #[serde(default)]
    pub allow_groups: Option<Vec<String>>,
    /// Blocklist of user IDs for this account.
    /// Overrides parent NapCatConfig.block_from if set.
    #[serde(default)]
    pub block_from: Option<Vec<String>>,
    /// WebSocket server configuration for this account (ws-server mode).
    #[serde(default)]
    pub server_host: Option<String>,
    #[serde(default)]
    pub server_port: Option<u16>,
    #[serde(default)]
    pub server_path: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeixinAccountConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub proxy: Option<String>,
}

/// 微信 (WeChat) iLink Bot channel configuration.
/// Uses long-polling based message reception via iLink Bot API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeixinConfig {
    #[serde(default)]
    pub enabled: bool,
    /// iLink Bot API token (Bearer token)
    #[serde(default)]
    pub token: String,
    /// Allowlist of sender user IDs. Empty = allow all.
    #[serde(default)]
    pub allow_from: Vec<String>,
    /// HTTP proxy for API requests
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub accounts: HashMap<String, WeixinAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

/// 企业微信 (WeCom / WeChat Work) channel configuration.
/// Supports both callback mode (webhook) and polling mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeComConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_wecom_mode")]
    pub mode: String,
    /// Enterprise corp ID (企业ID)
    #[serde(default)]
    pub corp_id: String,
    /// Application secret (应用Secret)
    #[serde(default)]
    pub corp_secret: String,
    /// Application agent ID (应用AgentId)
    #[serde(default)]
    pub agent_id: i64,
    /// Long connection bot_id (智能机器人 BotID)
    #[serde(default)]
    pub bot_id: String,
    /// Long connection secret (智能机器人 Secret)
    #[serde(default)]
    pub bot_secret: String,
    /// Callback token for message verification (企业微信回调Token)
    #[serde(default)]
    pub callback_token: String,
    /// AES key for message decryption (EncodingAESKey)
    #[serde(default)]
    pub encoding_aes_key: String,
    /// Allowlist of sender user IDs. Empty = allow all.
    #[serde(default)]
    pub allow_from: Vec<String>,
    /// Polling interval in seconds (used when callback is not configured). Default: 10.
    #[serde(default = "default_wecom_poll_interval")]
    pub poll_interval_secs: u32,
    /// Long connection websocket url.
    #[serde(default = "default_wecom_ws_url")]
    pub ws_url: String,
    /// Long connection ping interval in seconds. Default: 30.
    #[serde(default = "default_wecom_ping_interval")]
    pub ping_interval_secs: u32,
    #[serde(default)]
    pub accounts: HashMap<String, WeComAccountConfig>,
    #[serde(default)]
    pub default_account_id: Option<String>,
}

fn default_wecom_mode() -> String {
    "webhook".to_string()
}

fn default_wecom_poll_interval() -> u32 {
    10
}

fn default_wecom_ws_url() -> String {
    "wss://openws.work.weixin.qq.com".to_string()
}

fn default_wecom_ping_interval() -> u32 {
    30
}

impl Default for WeComConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_wecom_mode(),
            corp_id: String::new(),
            corp_secret: String::new(),
            agent_id: 0,
            bot_id: String::new(),
            bot_secret: String::new(),
            callback_token: String::new(),
            encoding_aes_key: String::new(),
            allow_from: Vec::new(),
            poll_interval_secs: default_wecom_poll_interval(),
            ws_url: default_wecom_ws_url(),
            ping_interval_secs: default_wecom_ping_interval(),
            accounts: HashMap::new(),
            default_account_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    #[serde(default)]
    pub whatsapp: WhatsAppConfig,
    #[serde(default)]
    pub telegram: TelegramConfig,
    #[serde(default)]
    pub feishu: FeishuConfig,
    #[serde(default)]
    pub slack: SlackConfig,
    #[serde(default)]
    pub discord: DiscordConfig,
    #[serde(default)]
    pub dingtalk: DingTalkConfig,
    #[serde(default)]
    pub wecom: WeComConfig,
    #[serde(default)]
    pub lark: LarkConfig,
    #[serde(default)]
    pub qq: QQConfig,
    /// NapCatQQ channel configuration (OneBot 11 protocol).
    #[serde(default)]
    pub napcat: NapCatConfig,
    #[serde(default)]
    pub weixin: WeixinConfig,
}
