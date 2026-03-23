# 23. 微信接入使用说明


`blockcell` 现在已经支持微信接入了。你可以把它理解成：

- 扫码登录微信机器人
- 自动保存 token
- 启动后接收微信消息
- 把消息转给 blockcell 里的 Agent 处理
- 再把回复发回微信

如果你只是想快速用起来，照着下面几步做就行。

## 一、先完成微信登录

先运行登录命令：

```bash
blockcell channels login weixin
```

运行后会发生三件事：

1. 终端里会显示二维码。
2. 用微信扫码。
3. 确认登录后，系统会自动把 token 保存下来。

默认保存到这个位置：

```bash
~/.blockcell/config.json5
```

如果二维码过期了，直接重新执行一次上面的命令就行。

## 二、把微信设成默认 owner

微信接入成功后，还需要给它指定一个默认的 owner agent。

如果不设置，启动 `gateway` 时会报错：

```text
Channel 'weixin' is enabled but has no owner agent.
```

直接执行：

```bash
blockcell channels owner set --channel weixin --agent default
```

这一步的意思是：

- `weixin` 这条消息通道交给 `default` 这个 Agent 处理
- 启动时就不会再提示没有 owner 了

如果你有自己的 Agent，也可以把 `default` 换成你的 Agent 名称。

## 三、启动 blockcell

登录和 owner 设置完成后，就可以启动了：

```bash
blockcell gateway
```

或者你平时用的是 agent 模式，也可以直接启动：

```bash
blockcell agent
```

启动成功后，微信消息就会进入 blockcell 的处理流程。

## 四、配置文件长什么样

你一般不需要手写太多配置，但了解一下会更安心。

微信相关配置主要在 `channels.weixin` 下面：

```json5
{
  channels: {
    weixin: {
      enabled: true,
      token: "你的 token",
      allowFrom: [],
      proxy: null
    }
  },
  channelOwners: {
    weixin: "default"
  }
}
```

你只要记住两点：

- `token` 是登录后自动保存的
- `channelOwners.weixin` 一定要有值

## 五、常见问题

### 1）提示没有 owner agent

说明微信通道已经启用了，但是还没有设置默认处理人。

执行：

```bash
blockcell channels owner set --channel weixin --agent default
```

### 2）二维码扫不上

直接重新执行登录命令：

```bash
blockcell channels login weixin
```

如果二维码已经过期，重新生成一次就可以。

### 3）登录成功了，但启动后没反应

可以先确认这几项：

- `channels.weixin.enabled` 是否为 `true`
- `channels.weixin.token` 是否已经保存
- `channelOwners.weixin` 是否已设置

## 六、推荐的标准流程

第一次接入微信时，建议按这个顺序操作：

```bash
blockcell channels login weixin
blockcell channels owner set --channel weixin --agent default
blockcell gateway
```

这样基本就能跑通。

## 七、小结

你可以把 blockcell 的微信接入理解为三步：

1. **扫码登录**
2. **保存 token**
3. **设置 owner 并启动 gateway**

对新手来说，最重要的就是记住：

- 先 `login weixin`
- 再 `owner set`
- 最后 `gateway`

