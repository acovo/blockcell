//! `/compact` command - Manually trigger conversation history compression.
//!
//! This command allows users to manually trigger the Layer 4 compact operation
//! instead of waiting for automatic triggering at 80% token budget threshold.

use crate::commands::slash_commands::{CommandContext, CommandResult, SlashCommand};

/// Compact command handler.
pub struct CompactCommand;

#[async_trait::async_trait]
impl SlashCommand for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "Manually trigger conversation history compression"
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        // Forward to runtime for processing
        // The actual compression happens in runtime.rs, not here
        // timeout_secs is not needed because ForwardToRuntime returns instantly
        CommandResult::ForwardToRuntime {
            transformed_content: "__COMPACT_REQUEST__".to_string(),
            original_command: "/compact".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_command_name() {
        let cmd = CompactCommand;
        assert_eq!(cmd.name(), "compact");
    }

    #[test]
    fn test_compact_command_description() {
        let cmd = CompactCommand;
        assert!(!cmd.description().is_empty());
    }

    #[tokio::test]
    async fn test_compact_command_execute() {
        let cmd = CompactCommand;
        let ctx = CommandContext::test_context();

        let result = cmd.execute("", &ctx).await;

        match result {
            CommandResult::ForwardToRuntime {
                transformed_content,
                original_command,
            } => {
                assert_eq!(transformed_content, "__COMPACT_REQUEST__");
                assert_eq!(original_command, "/compact");
            }
            _ => panic!("Expected ForwardToRuntime result"),
        }
    }

    #[tokio::test]
    async fn test_compact_command_with_args() {
        let cmd = CompactCommand;
        let ctx = CommandContext::test_context();

        // Args are currently ignored, but should not cause errors
        let result = cmd.execute("some args", &ctx).await;

        match result {
            CommandResult::ForwardToRuntime { .. } => {}
            _ => panic!("Expected ForwardToRuntime result"),
        }
    }
}