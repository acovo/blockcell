# BlueClaw / BlockCell v0.1.7 Release Notes

发布日期：2026-06-28

v0.1.7 是一次偏向生产可控性、安全边界和运行稳定性的版本。它在 v0.1.6 的 Ghost Native 学习、多智能体和统一命令体系之上，补上了模型路由、工具执行策略、预算控制、生命周期 Hook、审计防篡改和一批关键安全修复。

## 重点更新

### 1. ModelRouter 智能路由与自动降级

- 支持 `manual`、`cost_optimized`、`quality_first`、`latency_first` 路由策略。
- `cost_optimized` 可让短上下文优先使用低成本模型，复杂长上下文回到主力模型。
- 主对话 LLM 调用支持连接阶段自动降级：首选 Provider 在流式输出开始前失败时，可切换到下一个可用 Provider。
- 流式输出开始后不会跨模型拼接，避免部分输出混入另一模型结果。

### 2. Tool Policy、预算与审计

- 新增 Tool Policy，支持工具名 glob、渠道条件、路径条件、`allow` / `ask` / `deny`、规则组继承和 simulation mode。
- 新增全局 Token / 成本预算控制，按会话跟踪 LLM 用量，避免异常循环或超长任务失控消耗。
- 审计日志新增 SHA-256 hash chain 校验，并记录会话、Provider 调用和预算事件。
- 新增 `blockcell audit verify` 对审计链完整性进行验证。

### 3. Hook 生命周期事件

- 新增 `~/.blockcell/hooks.yaml`。
- 支持 `session_start`、`user_prompt`、`pre_tool_use`、`post_tool_use`、`agent_stop`。
- 支持 `{tool_name}`、`{session_id}`、`{cwd}`、`{command}`、`{file_path}`、`{result}` 等模板变量。
- Hook 失败、超时或返回非 0 不阻断主流程，适合审计、格式化、通知和外部日志接入。

### 4. MCP 按需发现与模型预设更新

- 当允许的 MCP 工具数量较大时，默认只向模型暴露 `mcp_search_tools`，减少 system prompt 膨胀。
- 远端 MCP 工具保持可执行，但先通过搜索发现再使用。
- 更新 2026-06 Provider 与默认模型预设，包括 DeepSeek、OpenAI、Anthropic、Gemini、Ollama、GLM、Qwen 等模型族。

### 5. WebUI 与运行性能优化

- WebUI 页面改用 `React.lazy` + `Suspense` 分块加载。
- Gateway 事件广播改为轻量路由结构，避免复制 content/token 等大字段。
- Token 估算、流式读取、配置热重载、Cron sync 和 SQLite 记忆操作路径减少重复计算、读盘和 Tokio worker 阻塞。

## 安全与稳定性修复

- 普通 Gateway API 不再接受 URL `?token=`，避免 token 写入访问日志或 Referer；请改用 `Authorization: Bearer <token>`。
- WebSocket / outbound 广播增加会话隔离，确认请求绑定连接上下文，避免跨会话泄漏或误批准。
- 修复 `http_request` SSRF、脚本执行路径逃逸、危险 `rm` 绕过、超时孤儿进程、symlink 写入逃逸和文件上传/读取大小限制问题。
- 修复 Ghost review、Dream、Session Memory、Auto Memory、Compact、skill evolution、core evolution 中多处并发、原子性、事务、恢复、路径和注入问题。
- 修复 CLI Unicode 光标编辑、Home/End/Delete 键处理、UTF-8 截断 panic、Windows 原子写入和路径兼容性问题。

## 升级注意

- 如果你的集成仍通过普通 HTTP API 的 `?token=` 传 token，需要改为 Bearer 头：

```bash
curl -H "Authorization: Bearer YOUR_TOKEN" http://localhost:18790/v1/...
```

- WebSocket 和文件下载/serve 入口仍保留必要的 query token 兼容。
- 建议检查 `~/.blockcell/config.json5` 中的 `gateway.apiToken`，公网部署必须使用稳定且足够复杂的 token。
- 如果启用了大量 MCP 工具，模型可能会先调用 `mcp_search_tools` 发现工具，这是预期行为。

## 相关文档

- [CHANGELOG.md](CHANGELOG.md)
- [ModelRouter 智能路由与自动降级](docs/28_model_router.md)
- [Hook 生命周期事件系统](docs/29_hook_system.md)
- [Gateway 模式](docs/08_gateway_mode.md)
- [CLI 参考](docs/17_cli_reference.md)
