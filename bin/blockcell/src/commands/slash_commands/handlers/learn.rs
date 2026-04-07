//! # /learn 命令
//!
//! 学习新技能。
//!
//! 注：此命令需要在 CLI/Gateway 层特殊处理，将消息转发给 AgentRuntime。

use crate::commands::slash_commands::*;

/// /learn 命令 - 学习新技能
///
/// 注意：此命令会调用 LLM，消耗 Token。
/// 由于需要将消息转发给 AgentRuntime，此命令在处理器中仅做参数验证，
/// 实际的消息转发逻辑在 CLI/Gateway 层实现。
pub struct LearnCommand;

#[async_trait::async_trait]
impl SlashCommand for LearnCommand {
    fn name(&self) -> &str {
        "learn"
    }

    fn description(&self) -> &str {
        "Learn a new skill by description (uses LLM)"
    }

    fn timeout_secs(&self) -> u64 {
        120 // 学习技能需要更长超时
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let description = args.trim();

        if description.is_empty() {
            return CommandResult::Handled(CommandResponse::text(
                "  Usage: /learn <skill description>\n".to_string(),
            ));
        }

        // 返回 NotACommand，让消息被转发给 AgentRuntime
        // CLI/Gateway 层会检测到这是 /learn 命令并特殊处理
        CommandResult::NotACommand
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_learn_command_empty() {
        let cmd = LearnCommand;
        let ctx = CommandContext::test_context();

        let result = cmd.execute("", &ctx).await;
        assert!(matches!(result, CommandResult::Handled(_)));

        if let CommandResult::Handled(response) = result {
            assert!(response.content.contains("Usage"));
        }
    }

    #[tokio::test]
    async fn test_learn_command_with_description() {
        let cmd = LearnCommand;
        let ctx = CommandContext::test_context();

        let result = cmd.execute("data analysis skill", &ctx).await;
        // /learn 返回 NotACommand，让消息被转发给 AgentRuntime
        assert!(matches!(result, CommandResult::NotACommand));
    }
}