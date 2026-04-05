# 微信（Weixin）机器人配置指南

Blockcell 支持通过微信 iLink Bot API 与智能体进行交互。微信渠道使用扫码登录的方式获取 token，启动后通过长轮询接收消息；如果你需要多账号接入，也可以通过 `accounts` 和 `defaultAccountId` 进行配置。

## 1. 登录微信

先运行登录命令：

```bash
blockcell channels login weixin
```

运行后会发生三件事：

1. 终端里会显示二维码。
2. 用微信扫码登录。
3. 登录成功后，系统会自动保存 token。

默认保存到：

```bash
~/.blockcell/config.json5
```

如果二维码过期了，直接重新执行一次上面的命令即可。

## 2. 配置允许访问的用户

为了安全起见，建议配置白名单（`allowFrom`），只允许指定的微信用户与你的机器人交互。

微信消息里的 `sender_id` 一般就是对端用户的微信账号标识。你可以先登录后观察日志，确认实际收到的用户 ID，再把它加入白名单。

## 3. 设置 owner 绑定

如果你通过 `blockcell gateway` 启用微信，还需要为这个渠道设置 owner。

例如给整条微信渠道设置默认 owner：

```bash
blockcell channels owner set --channel weixin --agent default
```

如果不设置，启动 `gateway` 时会报错：

```text
Channel 'weixin' is enabled but has no owner agent.
```

如果你配置了多个微信账号，还可以给某个具体账号单独绑定 owner。`--account` 需要填写 `channels.weixin.accounts` 里真实存在的账号 ID，例如：

```bash
blockcell channels owner set --channel weixin --account bot1 --agent default
```

## 4. 配置 Blockcell

在 Blockcell 的配置文件（例如 `~/.blockcell/config.json5`）中，找到 `channels.weixin` 部分并配置。

### 单账号配置

如果你只使用一个微信账号，保持顶层 `token` 即可：

```json5
{
  "channelOwners": {
    "weixin": "default"
  },
  "channels": {
    "weixin": {
      "enabled": true,
      "token": "你的 token",
      "allowFrom": ["允许的微信用户ID"],
      "proxy": null
    }
  }
}
```

### 多账号配置

如果你要接入多个微信账号，可以再加上 `defaultAccountId` 和 `accounts`：

```json5
{
  "channelOwners": {
    "weixin": "default"
  },
  "channels": {
    "weixin": {
      "enabled": true,
      "token": "主账号 token",
      "allowFrom": ["允许的微信用户ID"],
      "proxy": null,
      "defaultAccountId": "bot1",
      "accounts": {
        "bot1": {
          "enabled": true,
          "token": "BOT1_TOKEN",
          "allowFrom": ["允许的微信用户ID"],
          "proxy": null
        },
        "bot2": {
          "enabled": true,
          "token": "BOT2_TOKEN",
          "allowFrom": ["允许的微信用户ID"],
          "proxy": null
        }
      }
    }
  }
}
```

### 配置项说明

- `enabled`：是否启用微信渠道。
- `token`：扫码登录后自动保存的 iLink Bot token。单账号场景下直接使用这一项。
- `allowFrom`：允许访问的用户 ID 列表（字符串数组）。如果留空 `[]`，则默认允许所有收到的消息。
- `proxy`：HTTP 代理地址（可选）。在某些网络环境下可能需要配置。
- `accounts`：微信账号级配置。账号启用后，可以配合 `channelAccountOwners.weixin.<accountId>` 分别绑定不同 owner。
- `defaultAccountId`：当存在多个账号时，默认使用的账号 ID。

> 如果你启用了微信渠道，必须配置 owner 绑定，否则 Gateway 会拒绝启动。单账号场景下用 `channelOwners.weixin`，多账号场景下可以再配 `channelAccountOwners.weixin.<accountId>`。

## 5. 启动 Blockcell

完成登录和配置后，启动 gateway：

```bash
blockcell gateway
```

启动成功后，微信消息就会进入 blockcell 的处理流程，AI 的回复也会发回微信。

## 6. 常见问题

### 1）二维码扫不上

重新执行登录命令即可：

```bash
blockcell channels login weixin
```

### 2）登录成功了，但启动后没反应

可以检查下面几项：

- `channels.weixin.enabled` 是否为 `true`
- `channels.weixin.token` 是否已经保存
- `channelOwners.weixin` 是否已设置
- `allowFrom` 是否把当前用户加入白名单
- 如果使用了多账号，`channelAccountOwners.weixin.<accountId>` 是否已配置

### 3）提示没有 owner agent

说明微信通道已经启用了，但是还没有设置默认处理人。

执行：

```bash
blockcell channels owner set --channel weixin --agent default
```

## 7. 推荐的标准流程

第一次接入微信时，建议按这个顺序操作：

```bash
blockcell channels login weixin
blockcell channels owner set --channel weixin --agent default
blockcell gateway
```

这样基本就能跑通。

## 小结

你可以把 blockcell 的微信接入理解为三步：

1. **扫码登录**
2. **保存 token**
3. **设置 owner 并启动 gateway**

如果你后续要把微信接入到自己的自动化工作流里，这套方式是最直接的起点。
