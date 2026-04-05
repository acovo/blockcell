# 23. Weixin Integration Guide

`blockcell` now supports Weixin integration. You can think of it as:

- QR-code login for a Weixin bot
- Automatic token saving
- Receiving Weixin messages after startup
- Sending those messages to blockcell Agents for processing
- Sending the replies back to Weixin

If you just want to get started quickly, follow the steps below.

## 1. Log in to Weixin first

Run the login command:

```bash
blockcell channels login weixin
```

After running it:

1. A QR code will appear in the terminal.
2. Scan it with Weixin.
3. After confirming the login, the token will be saved automatically.

By default, it is saved to:

```bash
~/.blockcell/config.json5
```

If the QR code expires, just run the same command again.

## 2. Set Weixin as the default owner

After Weixin is connected, you still need to assign a default owner agent.

If you do not set it, `gateway` will fail to start with:

```text
Channel 'weixin' is enabled but has no owner agent.
```

Run:

```bash
blockcell channels owner set --channel weixin --agent default
```

This means:

- the `weixin` channel is handled by the `default` Agent
- startup will no longer complain about a missing owner

If you have your own Agent, you can replace `default` with that Agent's name.

If you want to bind a specific Weixin account to its own owner, you can also use `--account <ACCOUNT_ID>`:

```bash
blockcell channels owner set --channel weixin --account bot1 --agent default
```

## 3. Start blockcell

After login and owner setup are complete, start it with:

```bash
blockcell gateway
```

Once it starts successfully, Weixin messages will enter blockcell's processing pipeline.

## 4. What the config file looks like

You usually do not need to hand-write much config, but it helps to know how it works.

The Weixin-related config lives under `channels.weixin`:

```json5
{
  channels: {
    weixin: {
      enabled: true,
      token: "YOUR_TOKEN",
      allowFrom: [],
      proxy: null
    }
  },
  channelOwners: {
    weixin: "default"
  }
}
```

Keep these points in mind:

- `token` is saved automatically after login
- `channelOwners.weixin` must be set
- If you only have one Weixin account, keeping the top-level `token` is enough; `accounts` and `defaultAccountId` are mainly for multi-account setups

## 5. FAQ

### 1) The QR code does not scan

Run the login command again:

```bash
blockcell channels login weixin
```

If the QR code has already expired, just generate a new one.

### 2) Login succeeded, but nothing happens after startup

Check the following:

- `channels.weixin.enabled` is `true`
- `channels.weixin.token` has been saved
- `channelOwners.weixin` is configured
- If you are using a whitelist, `allowFrom` contains the current user
- If you are using account-level bindings, make sure `channelAccountOwners.weixin` has the expected account entry

### 3) It says there is no owner agent

That means the Weixin channel is enabled, but no default owner has been configured yet.

Run:

```bash
blockcell channels owner set --channel weixin --agent default
```

## 6. Recommended setup flow

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
