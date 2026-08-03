# 企业微信 (WeCom) 机器人配置指南

Blockcell 支持通过企业微信 (WeCom / WeChat Work) 自建应用与智能体进行交互。企业微信渠道支持 **轮询 (Polling)**、**回调 (Webhook)** 和 **长连接 (Long Connection / WebSocket)** 三种模式接收消息。

> **部署建议**：
> - **内网/本地开发**：推荐使用**轮询模式**，无需配置公网服务器和域名，开箱即用。
> - **生产环境**：推荐使用**回调模式 (Webhook)**，消息实时性更高，不消耗 API 调用额度。Gateway 已内置 `/webhook/wecom` 路由，支持 GET 验证和 POST 消息回调。
> - **智能机器人场景**：如果你使用企业微信智能机器人平台，推荐使用**长连接模式**，无需公网 Webhook，直接通过 `BotID + Secret` 建立 WebSocket 长连接接收消息。

> ⚠️ **重要：企业可信 IP**
> 回调模式下，企业微信的 API（发送消息等）只允许来自**企业可信 IP** 的请求。必须在管理后台将你的服务器公网 IP 加入白名单，否则发送消息时会报错 `60020: not allow to access from your ip`。
> 配置路径：管理后台 → 应用管理 → 你的应用 → **企业可信IP** → 添加服务器 IP。

---

## 前提条件（仅回调模式需要）

- Blockcell gateway 部署在**公网可访问**的服务器上（或通过 ngrok/frp 等内网穿透工具暴露）
- Webhook URL 格式：`https://your-domain.com/webhook/wecom`

---

## 1. 选择接入模式

你需要先确认自己接的是哪一类企业微信能力：

- **自建应用**
  - 使用 `corpId + corpSecret + agentId`
  - 接收方式：`webhook` 或 `polling`
- **智能机器人 / AI Bot**
  - 使用 `botId + botSecret`
  - 接收方式：`long_connection`

如果你当前参考的是企业微信智能机器人长连接文档，那么应该配置的是 **`long_connection`**，而不是传统 `corpId` 模式。

## 2. 创建企业微信应用 / 机器人

1. 登录并访问 [企业微信管理后台](https://work.weixin.qq.com/)。
2. 切换到 **应用管理** 标签页。
3. 在 **自建** 应用区域，点击 **创建应用**。
4. 填写应用名称（如：Blockcell Bot）、应用 Logo 和应用介绍，选择可见范围（如：全员或指定部门），点击 **创建应用**。
5. 创建成功后，在应用详情页，复制并保存你的 **AgentId** 和 **Secret**（查看 Secret 需要在企业微信手机端确认）。

## 3. 获取企业 ID (CorpId)

1. 在企业微信管理后台，点击顶部菜单的 **我的企业**。
2. 滚动到底部的 **企业信息**，找到并复制 **企业 ID**。

## 4. 配置回调模式 (Webhook)

如果你希望使用回调模式（Webhook）而不是轮询模式，你需要配置接收消息服务器：

1. 在应用详情页，找到 **接收消息** 设置，点击 **设置 API 接收**。
2. **URL**：填写你的公网服务器 URL（如 `https://your-domain.com/webhook/wecom`）。
3. **Token**：点击 **随机获取**，并记录下来。
4. **EncodingAESKey**：点击 **随机获取**，并记录下来。
5. 点击 **保存**。企业微信会向该 URL 发送一个 `GET` 请求进行验证，Blockcell gateway 会自动完成 SHA1 签名验证并返回解密后的 `echostr`。

*（如果使用轮询模式，请跳过此步骤）。*

## 5. 长连接模式配置 (Long Connection)

如果你使用的是企业微信智能机器人长连接协议：

1. 在企业微信智能机器人平台获取：
   - `BotID`
   - `Secret`
2. 在 Blockcell 配置里设置：
   - `mode = "long_connection"`
   - `botId`
   - `botSecret`
3. 启动 `blockcell gateway`
4. 启动后 Blockcell 会主动连接：

```text
wss://openws.work.weixin.qq.com
```

5. 建连成功后，用户在企业微信里给机器人发消息，Blockcell 会收到：
   - `text`
   - `image`
   - `mixed`
   - `voice`
   - `file`

> 长连接模式**不需要**配置 `callbackToken`、`encodingAesKey`，也**不需要**公网 `webhook`。

## 6. 获取用户 ID（用于白名单）

企业微信的 `sender_id` 通常是企业内的账号（UserID）。
1. 在管理后台的 **通讯录** 中，点击对应的成员。
2. 在成员详情中找到 **账号**（如 `ZhangSan`）。

## 7. 配置 Blockcell

编辑 `~/.blockcell/config.json5`。

---

### 7.1 单 Agent 模式

整个企业微信渠道的消息统一路由给一个 Agent（最常见场景）。

**长连接模式（推荐）：**

```json5
{
  "channels": {
    "wecom": {
      "enabled": true,
      "mode": "long_connection",
      "botId": "aibXXXXXXXXXXXXXXXXXX",
      "botSecret": "你的智能机器人Secret"
    }
  },
  "channelOwners": {
    "wecom": "default"   // 消息路由给名为 "default" 的 agent
  }
}
```

**回调模式（自建应用 + 公网 Webhook）：**

```json5
{
  "channels": {
    "wecom": {
      "enabled": true,
      "mode": "webhook",
      "corpId": "ww1a2b3c4d5e6f7g8h",
      "corpSecret": "A1B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6",
      "agentId": 1000001,
      "callbackToken": "你的回调Token",
      "encodingAesKey": "你的回调AESKey（43位）"
    }
  },
  "channelOwners": {
    "wecom": "default"
  }
}
```

**轮询模式（内网/本地，无需公网）：**

```json5
{
  "channels": {
    "wecom": {
      "enabled": true,
      "mode": "polling",
      "corpId": "ww1a2b3c4d5e6f7g8h",
      "corpSecret": "A1B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6",
      "agentId": 1000001,
      "pollIntervalSecs": 30
    }
  },
  "channelOwners": {
    "wecom": "default"
  }
}
```

> ⚠️ **必须配置 `channelOwners`**，否则 Gateway 会因"enabled channel has no owner"而拒绝启动。

---

### 7.2 多 Agent 模式

通过 `accounts` 字段配置多个企业微信账号/机器人，每个账号可以路由到不同的 Agent。

```json5
{
  "channels": {
    "wecom": {
      "accounts": {
        // 账号一：AI 机器人，长连接，路由到 "ai" agent
        "aibot": {
          "enabled": true,
          "mode": "long_connection",
          "botId": "aibXXXXXXXXXXXXXXXXXX",
          "botSecret": "智能机器人Secret"
        },
        // 账号二：运营自建应用，回调模式，路由到 "ops" agent
        "ops-app": {
          "enabled": true,
          "mode": "webhook",
          "corpId": "ww1a2b3c4d5e6f7g8h",
          "corpSecret": "A1B2C3D4E5F6G7H8I9J0K1L2M3N4O5P6",
          "agentId": 1000001,
          "callbackToken": "OpsToken",
          "encodingAesKey": "OpsAESKey"
        }
      }
    }
  },
  // 未在 channelAccountOwners 中指定的账号，默认路由到此 agent
  "channelOwners": {
    "wecom": "default"
  },
  // 每个账号单独路由到指定 agent
  "channelAccountOwners": {
    "wecom": {
      "aibot":   "ai",
      "ops-app": "ops"
    }
  }
}
```

> **注意**：多账号模式下，每个账号的 `mode` 可以不同，长连接和回调可以并存。
> 路由优先级：`channelAccountOwners` > `channelOwners`（兜底）。

---

### 7.3 配置项说明

| 字段 | 适用模式 | 说明 |
|------|---------|------|
| `enabled` | 所有 | 是否启用此账号/渠道 |
| `mode` | 所有 | `webhook`、`polling`、`long_connection`。默认 `webhook` |
| `corpId` | webhook / polling | 企业 ID，在"我的企业"底部获取 |
| `corpSecret` | webhook / polling | 自建应用 Secret，在应用详情获取 |
| `agentId` | webhook / polling | 自建应用 AgentId（数字） |
| `botId` | long_connection | 智能机器人 BotID |
| `botSecret` | long_connection | 智能机器人 Secret |
| `callbackToken` | webhook | 回调消息验签 Token |
| `encodingAesKey` | webhook | 回调消息解密 Key（43 位） |
| `pollIntervalSecs` | polling | 轮询间隔秒数，默认 `10`，建议 `30` |
| `wsUrl` | long_connection | WebSocket 地址，默认 `wss://openws.work.weixin.qq.com` |
| `pingIntervalSecs` | long_connection | 心跳间隔秒数，默认 `30`，最小 `10` |
| `allowFrom` | 所有 | 白名单 UserID 列表，留空则允许所有人 |

## 8. 启动 Gateway

```bash
blockcell gateway
```

启动后，Gateway 会在本地监听端口（默认 `18790`）。

- **回调模式**：企业微信的 Webhook 路径为：

```
http://127.0.0.1:18790/webhook/wecom ← 本机地址（企业微信无法直接访问）
https://your-domain.com/webhook/wecom ← 需要通过 Nginx 反向代理暴露到公网
```

---

## 9. 网络配置：将 Webhook 暴露到公网 (回调模式)

企业微信服务器需要能主动访问你的 `/webhook/wecom` 端点，因此必须有公网可访问的 URL（企业微信要求 80 或 443 端口，推荐使用 HTTPS）。

### 方案一：Nginx 反向代理（推荐生产环境）

假设你的服务器公网域名为 `bot.example.com`，Blockcell gateway 运行在本机 `18790` 端口。

**Nginx 配置文件** `/etc/nginx/sites-available/blockcell`：

```nginx
server {
    listen 80;
    server_name bot.example.com;
    # 强制跳转 HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl;
    server_name bot.example.com;

    # SSL 证书（推荐使用 Let's Encrypt）
    ssl_certificate     /etc/letsencrypt/live/bot.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/bot.example.com/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;

    # 只代理企业微信 webhook 路径
    location /webhook/wecom {
        proxy_pass         http://127.0.0.1:18790/webhook/wecom;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
        proxy_read_timeout 30s;
    }
}
```

启用配置并重载 Nginx：

```bash
# 创建软链接
sudo ln -s /etc/nginx/sites-available/blockcell /etc/nginx/sites-enabled/

# 申请 SSL 证书（首次）
sudo certbot --nginx -d bot.example.com

# 测试配置并重载
sudo nginx -t
sudo systemctl reload nginx
```

配置完成后，在企业微信后台的“接收消息” URL 处填写：`https://bot.example.com/webhook/wecom`。

### 方案二：ngrok（本地开发 / 临时测试）

无需服务器，适合本地调试回调模式。

```bash
# 安装 ngrok
brew install ngrok

# 启动隧道（指向 Blockcell gateway 端口）
ngrok http 18790
```

ngrok 会输出类似：`Forwarding  https://abc123.ngrok-free.app -> http://localhost:18790`

在企业微信后台填写：`https://abc123.ngrok-free.app/webhook/wecom`。

> ⚠️ **注意**：免费版 ngrok 每次重启地址会变化，需要重新在企业微信后台更新 URL。

### 方案三：frp 内网穿透

适合没有公网 IP 但有一台公网服务器的场景。

**公网服务器** `frps.ini`：
```ini
[common]
bind_port = 7000
```

**本地机器** `frpc.ini`：
```ini
[common]
server_addr = your-public-server-ip
server_port = 7000

[blockcell-wecom]
type = http
local_ip = 127.0.0.1
local_port = 18790
custom_domains = bot.example.com
```

然后在公网服务器上用 Nginx 代理 frp 的 HTTP 端口，配置同方案一。

---

## 10. 交互方式

- **单聊**：在企业微信客户端（手机或 PC）中，找到 **工作台** -> 你的应用（Blockcell Bot），直接发送消息。
- **群聊**：
  1. 在企业微信手机端，进入任意内部群聊。
  2. 点击右上角群设置 -> **添加群机器人** -> **选择已有的机器人**（如果在应用管理中开启了“防骚扰/机器人”相关功能），或者直接将自建应用添加到群聊（这取决于你的企业微信版本和配置）。
  3. **注意**：企业微信对自建应用在群聊中的表现有限制。如果通过应用发消息到群，通常需要用户的行为触发（且发往群的接口是 `appchat/send`，需要预先创建 `chatid`）。Blockcell 会自动识别以 `wr` 开头的群聊 ID 并使用 `appchat` 接口回复。

## 11. 测试与验证

### 长连接模式检查点

启动 `blockcell gateway` 后，如果配置正确，通常应能在日志中看到类似信息：

```text
WeCom channel started (long_connection mode)
Connected to WeCom long connection WebSocket
WeCom long connection subscribed successfully
```

如果你在企业微信里发消息后后端完全没反应，优先检查：

- `mode` 是否真的是 `long_connection`
- `botId` / `botSecret` 是否填写正确
- `channelOwners.wecom` 是否已配置
- 是否真的启动的是你当前编译出来的 `./target/debug/blockcell gateway`
- 日志里是否出现了 `subscribed successfully`

### 连通性测试（无需任何参数）

```bash
# 直接访问，应返回 200 OK，body 为 "ok"
curl -i http://127.0.0.1:18790/webhook/wecom

# 通过公网域名（需 Nginx 代理）
curl -i https://your-domain.com/webhook/wecom
```

### 模拟企业微信 URL 验证（GET + 签名参数）

企业微信在后台保存回调 URL 时，会发送带签名的 GET 请求。可用以下命令模拟：

```bash
# 不配置 callbackToken 时（签名验证跳过），直接返回 echostr 的值
curl -i "http://127.0.0.1:18790/webhook/wecom?msg_signature=abc&timestamp=1234567890&nonce=test&echostr=hello123"
# 期望响应：200 OK，body = hello123
```

### 模拟企业微信消息回调（POST）

```bash
curl -i -X POST http://127.0.0.1:18790/webhook/wecom \
  -H 'Content-Type: text/xml' \
  -d '<xml>
  <ToUserName><![CDATA[ww企业ID]]></ToUserName>
  <FromUserName><![CDATA[ZhangSan]]></FromUserName>
  <CreateTime>1409735669</CreateTime>
  <MsgType><![CDATA[text]]></MsgType>
  <Content><![CDATA[你好]]></Content>
  <MsgId>1234567890</MsgId>
  <AgentID>1000001</AgentID>
</xml>'
# 期望响应：200 OK，body = success
```

> **说明**：
> - 若 `callbackToken` 为空，POST 消息不做签名验证（适合内网测试）。
> - 若 `callbackToken` 已配置，企业微信发来的请求会携带 `msg_signature` 参数，Blockcell 会自动验签（SHA1）。
> - 企业微信实际发送的消息体是 AES-256-CBC 加密的，Blockcell 会使用 `encodingAesKey` 自动解密（支持 WeCom 的 PKCS7-32 填充方式）。

---

## 12. 注意事项

- **消息长度限制**：企业微信文本消息最大长度为 2048 字符，超长消息 Blockcell 会自动切片并分条发送。
- **API 频率限制**：企业微信的 **API 调用频率限制非常严格**（例如：发送消息接口通常是 10000次/月，获取 access_token 是 2000次/月）。
  - 如果使用**轮询模式**，请确保配置了合适的 `pollIntervalSecs`（建议 10-30 秒）。
  - 如果使用**回调模式**，则接收消息不消耗 API 调用额度，仅发送消息消耗额度。
  - Blockcell 内部实现了 Token 的缓存和复用（提前 5 分钟刷新）以尽量减少 API 调用。
- **消息安全**：回调模式下，企业微信发来的所有消息默认是加密的，Blockcell 会利用你配置的 `encodingAesKey` 自动解密，并使用 `callbackToken` 进行安全验签。
- **长连接媒体消息**：`image`、`voice`、`file` 会通过 `url + aeskey` 下载并解密到本地 `~/.blockcell/workspace/media/`。

---

## 13. 常见错误排查

| 错误 | 原因 | 解决方法 |
|------|------|----------|
| `GET signature verification failed` | `callbackToken` 配置错误，或 URL 参数被中间件二次解码 | 确认 `callbackToken` 与管理后台一致 |
| `WeCom — no corp_id configured` | 你实际上在用长连接模式，但旧版本 banner 仍按传统模式理解配置 | 升级到包含长连接 banner 修复的版本；长连接模式应显示 `bot_id` |
| 企业微信发消息后端无任何日志 | 长连接未连上，或 `botId` / `botSecret` 配错 | 检查启动日志中是否出现 `Connected to WeCom long connection WebSocket` 和 `subscribed successfully` |
| `60020: not allow to access from your ip` | 服务器 IP 未加入企业可信 IP 白名单 | 管理后台 → 应用 → **企业可信IP** → 添加服务器 IP |
| `81013: user & party & tag all invalid` | 回复目标用户 ID 错误（误用了企业 ID 作为收件人） | 已修复，Blockcell 使用 `FromUserName` 作为回复目标 |
| `40014: invalid access_token` | `corpSecret` 配置错误，或 token 已过期 | 确认 `corpSecret` 正确；Blockcell 会自动刷新 token |
