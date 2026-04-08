//! # /quit 和 /exit 命令
//!
//! 退出交互模式（仅 CLI 模式可用）。

use crate::commands::slash_commands::*;

/// /quit 命令 - 仅在 CLI 模式可用
pub struct QuitCommand;

#[async_trait::async_trait]
impl SlashCommand for QuitCommand {
    fn name(&self) -> &str {
        "quit"
    }

    fn description(&self) -> &str {
        "Exit interactive mode (CLI only)"
    }

    fn available_channels(&self) -> Option<Vec<&'static str>> {
        Some(vec!["cli"]) // 仅 CLI 可用
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        // 返回 ExitRequested，由 CLI 层处理退出逻辑
        CommandResult::ExitRequested
    }
}

/// /exit 命令 - quit 的别名
pub struct ExitCommand;

#[async_trait::async_trait]
impl SlashCommand for ExitCommand {
    fn name(&self) -> &str {
        "exit"
    }

    fn description(&self) -> &str {
        "Exit interactive mode (CLI only)"
    }

    fn available_channels(&self) -> Option<Vec<&'static str>> {
        Some(vec!["cli"])
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        // 调用 QuitCommand 的实现
        QuitCommand.execute(args, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quit_command_cli() {
        let cmd = QuitCommand;
        let ctx = CommandContext {
            source: CommandSource::cli("test-chat".to_string()),
            ..Default::default()
        };

        let result = cmd.execute("", &ctx).await;
        assert!(matches!(result, CommandResult::ExitRequested));
    }

    #[tokio::test]
    async fn test_quit_command_channel_restriction() {
        let mut handler = crate::commands::slash_commands::SlashCommandHandler::new();
        handler.register(QuitCommand);

        // WebSocket 渠道应该返回提示
        let ctx_ws = CommandContext {
            source: CommandSource::websocket("test-chat".to_string()),
            ..Default::default()
        };
        let result = handler.try_handle("/quit", &ctx_ws).await;

        if let CommandResult::Handled(response) = result {
            assert!(response.content.contains("仅"));
        } else {
            panic!("Expected Handled response for channel restriction");
        }
    }

    #[tokio::test]
    async fn test_exit_command_alias() {
        let cmd = ExitCommand;
        let ctx = CommandContext {
            source: CommandSource::cli("test-chat".to_string()),
            ..Default::default()
        };

        let result = cmd.execute("", &ctx).await;
        assert!(matches!(result, CommandResult::ExitRequested));
    }
}