# Weixin Bot Configuration Guide

Blockcell supports interacting with agents through Weixin iLink Bot API. The Weixin channel uses QR-code login to obtain a token, then receives messages through long polling; if you need multiple accounts, you can also configure `accounts` and `defaultAccountId`.

## 1. Log in to Weixin

Run the login command first:

```bash
blockcell channels login weixin
```

After running it:

1. A QR code will be shown in the terminal.
2. Scan it with Weixin.
3. Once login succeeds, the token will be saved automatically.

By default, it is saved to:

```bash
~/.blockcell/config.json5
```

If the QR code expires, just run the command again.

## 2. Configure Allowed Users

For security, it is recommended to configure an allowlist (`allowFrom`) so only specific Weixin users can interact with your bot.

The `sender_id` in Weixin messages is usually the identifier of the remote user account. After login, you can inspect the logs to confirm the actual user ID and then add it to the allowlist.

## 3. Set the Owner Binding

If you enable Weixin through `blockcell gateway`, you also need to bind an owner for this channel.

For example, to set a default owner for the whole Weixin channel:

```bash
blockcell channels owner set --channel weixin --agent default
```

If you skip this step, `gateway` will fail to start with an error like:

```text
Channel 'weixin' is enabled but has no owner agent.
```

If you configure multiple Weixin accounts, you can also bind an owner for one specific account. The `--account` value must be a real account ID that exists under `channels.weixin.accounts`, for example:

```bash
blockcell channels owner set --channel weixin --account bot1 --agent default
```

## 4. Configure Blockcell

In the Blockcell configuration file, such as `~/.blockcell/config.json5`, find the `channels.weixin` section and configure it.

### Single-account configuration

If you only use one Weixin account, keep the top-level `token`:

```json5
{
  "channelOwners": {
    "weixin": "default"
  },
  "channels": {
    "weixin": {
      "enabled": true,
      "token": "YOUR_TOKEN",
      "allowFrom": ["ALLOWED_WEIXIN_USER_ID"],
      "proxy": null
    }
  }
}
```

### Multi-account configuration

If you want to connect multiple Weixin accounts, add `defaultAccountId` and `accounts`:

```json5
{
  "channelOwners": {
    "weixin": "default"
  },
  "channels": {
    "weixin": {
      "enabled": true,
      "token": "MAIN_ACCOUNT_TOKEN",
      "allowFrom": ["ALLOWED_WEIXIN_USER_ID"],
      "proxy": null,
      "defaultAccountId": "bot1",
      "accounts": {
        "bot1": {
          "enabled": true,
          "token": "BOT1_TOKEN",
          "allowFrom": ["ALLOWED_WEIXIN_USER_ID"],
          "proxy": null
        },
        "bot2": {
          "enabled": true,
          "token": "BOT2_TOKEN",
          "allowFrom": ["ALLOWED_WEIXIN_USER_ID"],
          "proxy": null
        }
      }
    }
  }
}
```

### Configuration Options

- `enabled`: Whether to enable the Weixin channel.
- `token`: The iLink Bot token saved after QR-code login. In single-account setups, use this top-level field directly.
- `allowFrom`: List of allowed user IDs. If left empty `[]`, all received messages are accepted by default.
- `proxy`: Optional HTTP proxy address. Some network environments may require it.
- `accounts`: Account-level Weixin configuration. Once an account is enabled, you can bind different owners via `channelAccountOwners.weixin.<accountId>`.
- `defaultAccountId`: The default account ID to use when multiple accounts exist.

> If you enable the Weixin channel, you must configure an owner binding. Otherwise Gateway will refuse to start. Use `channelOwners.weixin` for a single account, or add `channelAccountOwners.weixin.<accountId>` for account-level bindings.

## 5. Start Blockcell

After login and configuration are complete, start the gateway:

```bash
blockcell gateway
```

Once it starts successfully, Weixin messages will flow into Blockcell's processing pipeline, and AI replies will be sent back to Weixin.

## 6. FAQ

### 1) The QR code does not scan

Run the login command again:

```bash
blockcell channels login weixin
```

### 2) Login succeeded, but nothing happens after startup

Check the following:

- `channels.weixin.enabled` is `true`
- `channels.weixin.token` has been saved
- `channelOwners.weixin` is configured
- `allowFrom` contains the current user if you are using a whitelist
- If you use account-level bindings, check that `channelAccountOwners.weixin.<accountId>` exists for the configured account

### 3) It says there is no owner agent

That means the Weixin channel is enabled, but no default owner has been configured yet.

Run:

```bash
blockcell channels owner set --channel weixin --agent default
```

## 7. Recommended Setup Flow

When setting up Weixin for the first time, the recommended order is:

```bash
blockcell channels login weixin
blockcell channels owner set --channel weixin --agent default
blockcell gateway
```

This is usually enough to get everything working.

## Summary

You can think of Blockcell Weixin integration as three steps:

1. **Log in with QR code**
2. **Save the token**
3. **Set the owner and start the gateway**

If you want to connect Weixin to your own automation workflow later, this is the most direct starting point.
