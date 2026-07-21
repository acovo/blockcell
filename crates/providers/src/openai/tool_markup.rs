//! OpenAI Provider 的「文本工具标记」解析器。
//!
//! 把自研 tool-markup 协议的解析逻辑（文本 <tool_call> 标记识别、
//! 非标准块解析、参数提取等）从 `openai.rs` 中分离出来，作为
//! `OpenAIProvider` 的关联函数集中维护，便于单测与阅读。
//! 这些函数全部为纯函数（不持有实例状态）。

use serde_json::{Map, Value};
use tracing::warn;

use blockcell_core::types::ToolCallRequest;

use super::OpenAIProvider;

impl OpenAIProvider {
    /// Build a text description of tools to inject into the system prompt.
    pub(super) fn build_tools_prompt(tools: &[Value]) -> String {
        crate::prompt_utils::build_tools_prompt(tools)
    }

    /// Parse text-based tool call blocks from the response content.
    /// Handles multiple formats:
    /// - `{"name":"...","arguments":{...}}`
    /// - `[TOOL_CALL]{tool => "...", args => {...}}[/TOOL_CALL]`
    ///
    /// Returns (remaining_text, parsed_tool_calls).
    pub(super) fn parse_function_parameter_tool_block(
        block: &str,
        call_index: u64,
    ) -> Option<ToolCallRequest> {
        let trimmed = block.trim();
        let lower = trimmed.to_lowercase();

        let func_start = lower.find("<function=")?;
        let after_func = &trimmed[func_start + "<function=".len()..];
        let func_end = after_func.find('>')?;
        let tool_name = after_func[..func_end].trim().to_string();
        if tool_name.is_empty() {
            return None;
        }

        let body = &after_func[func_end + 1..];
        let body_lower = body.to_lowercase();
        let body_end = body_lower.find("</function>").unwrap_or(body.len());
        let params_str = &body[..body_end];

        let mut args = serde_json::Map::new();
        let mut scan = params_str;

        loop {
            let scan_lower = scan.to_lowercase();
            let Some(param_start) = scan_lower.find("<parameter=") else {
                break;
            };

            let after_param = &scan[param_start + "<parameter=".len()..];
            let Some(param_name_end) = after_param.find('>') else {
                break;
            };

            let param_name = after_param[..param_name_end].trim().to_string();
            if param_name.is_empty() {
                scan = &after_param[param_name_end + 1..];
                continue;
            }

            let value_str = &after_param[param_name_end + 1..];
            let value_lower = value_str.to_lowercase();
            let Some(close_idx) = value_lower.find("</parameter>") else {
                break;
            };

            let raw_value = value_str[..close_idx].trim();
            let json_val = serde_json::from_str::<Value>(raw_value)
                .unwrap_or_else(|_| Value::String(raw_value.to_string()));
            args.insert(param_name, json_val);

            scan = &value_str[close_idx + "</parameter>".len()..];
        }

        Some(ToolCallRequest {
            id: format!("text_call_{}", call_index),
            name: tool_name,
            arguments: Value::Object(args),
            thought_signature: None,
        })
    }

    pub(super) fn strip_wrapping_quotes(input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.len() >= 2 {
            let bytes = trimmed.as_bytes();
            let first = bytes[0];
            let last = bytes[trimmed.len() - 1];
            if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
                return trimmed[1..trimmed.len() - 1].to_string();
            }
        }
        trimmed.to_string()
    }

    pub(super) fn parse_native_scalar_value(input: &str) -> Value {
        let trimmed = input.trim();
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            return parsed;
        }

        let unquoted = Self::strip_wrapping_quotes(trimmed);
        if unquoted.eq_ignore_ascii_case("true") {
            return Value::Bool(true);
        }
        if unquoted.eq_ignore_ascii_case("false") {
            return Value::Bool(false);
        }
        if unquoted.eq_ignore_ascii_case("null") {
            return Value::Null;
        }
        if let Ok(parsed) = unquoted.parse::<i64>() {
            return serde_json::json!(parsed);
        }
        if let Ok(parsed) = unquoted.parse::<f64>() {
            return serde_json::json!(parsed);
        }

        Value::String(unquoted)
    }

    pub(super) fn parse_parameter_block_arguments(raw: &str) -> Option<Map<String, Value>> {
        let mut args = Map::new();
        let mut scan = raw;

        loop {
            let scan_lower = scan.to_lowercase();
            let Some(param_start) = scan_lower.find("<parameter=") else {
                break;
            };

            let after_param = &scan[param_start + "<parameter=".len()..];
            let Some(param_name_end) = after_param.find('>') else {
                break;
            };

            let param_name = after_param[..param_name_end].trim().to_string();
            if param_name.is_empty() {
                scan = &after_param[param_name_end + 1..];
                continue;
            }

            let value_str = &after_param[param_name_end + 1..];
            let value_lower = value_str.to_lowercase();
            let Some(close_idx) = value_lower.find("</parameter>") else {
                break;
            };

            let raw_value = value_str[..close_idx].trim();
            args.insert(param_name, Self::parse_native_scalar_value(raw_value));
            scan = &value_str[close_idx + "</parameter>".len()..];
        }

        if args.is_empty() {
            None
        } else {
            Some(args)
        }
    }

    pub(super) fn parse_loose_argument_map(raw: &str) -> Option<Map<String, Value>> {
        if raw.contains("://")
            && !raw.trim_start().starts_with('{')
            && !raw.contains("=>")
            && !raw.contains(',')
            && !raw.starts_with("--")
        {
            return None;
        }

        let trimmed = raw
            .trim()
            .trim_start_matches('{')
            .trim_end_matches('}')
            .trim();
        if trimmed.is_empty() {
            return Some(Map::new());
        }

        let mut args = Map::new();
        for pair in trimmed.split(',') {
            let entry = pair.trim();
            if entry.is_empty() {
                continue;
            }

            let parsed = entry
                .split_once("=>")
                .or_else(|| entry.split_once(':'))
                .or_else(|| entry.split_once('='));
            let (raw_key, raw_value) = parsed?;

            let key = Self::strip_wrapping_quotes(raw_key.trim());
            if key.is_empty() {
                return None;
            }

            args.insert(key, Self::parse_native_scalar_value(raw_value.trim()));
        }

        Some(args)
    }

    pub(super) fn single_required_string_param_name(
        tool_name: &str,
        tools: &[Value],
    ) -> Option<String> {
        tools.iter().find_map(|tool| {
            let function = tool.get("function")?;
            let name = function.get("name")?.as_str()?;
            if name != tool_name {
                return None;
            }

            let parameters = function.get("parameters")?;
            let required = parameters.get("required")?.as_array()?;
            if required.len() != 1 {
                return None;
            }
            let param_name = required.first()?.as_str()?.to_string();
            let param_type = parameters
                .get("properties")
                .and_then(|p| p.get(&param_name))
                .and_then(|p| p.get("type"))
                .and_then(|t| t.as_str());
            if param_type == Some("string") {
                Some(param_name)
            } else {
                None
            }
        })
    }

    pub(super) fn parse_native_tool_arguments(
        tool_name: &str,
        raw_arguments: &str,
        tools: &[Value],
    ) -> std::result::Result<Value, String> {
        let trimmed = raw_arguments.trim();
        if trimmed.is_empty() {
            return Ok(Value::Object(Map::new()));
        }

        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            return Ok(parsed);
        }

        if let Some(args) = Self::parse_parameter_block_arguments(trimmed) {
            return Ok(Value::Object(args));
        }

        if let Some(args) = Self::parse_loose_argument_map(trimmed) {
            return Ok(Value::Object(args));
        }

        let dash_args = Self::parse_dash_args(trimmed);
        if dash_args.as_object().is_some_and(|map| !map.is_empty()) {
            return Ok(dash_args);
        }

        if let Some(param_name) = Self::single_required_string_param_name(tool_name, tools) {
            let value = Self::strip_wrapping_quotes(trimmed);
            if !value.is_empty() {
                let mut args = Map::new();
                args.insert(param_name, Value::String(value));
                return Ok(Value::Object(args));
            }
        }

        Err("unrecognized native tool-call arguments".to_string())
    }

    pub(super) fn normalize_text_tool_markup(content: &str) -> String {
        content
            .replace("<｜DSML｜", "<")
            .replace("</｜DSML｜", "</")
            .replace("<|DSML|", "<")
            .replace("</|DSML|", "</")
    }

    pub(super) fn strip_text_tool_wrappers(content: &str) -> String {
        let mut remaining = content.to_string();
        for tag in ["<tool_calls>", "</tool_calls>", "<tools>", "</tools>"] {
            remaining = remaining.replace(tag, "");
        }
        remaining
    }

    pub(super) fn text_tool_marker_starts() -> &'static [&'static str] {
        &[
            "<｜DSML｜",
            "</｜DSML｜",
            "<|DSML|",
            "</|DSML|",
            "<tool_call",
            "</tool_call",
            "[tool_call]",
            "[/tool_call]",
            "<minimax:tool_call",
            "</minimax:tool_call",
            "<tool_calls",
            "</tool_calls",
            "<invoke",
            "</invoke",
            "<parameter",
            "</parameter",
            "[called:",
        ]
    }

    pub(super) fn starts_with_text_tool_marker(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::text_tool_marker_starts()
            .iter()
            .any(|marker| lower.starts_with(&marker.to_lowercase()))
    }

    pub(super) fn find_text_tool_marker_start(text: &str) -> Option<usize> {
        text.char_indices()
            .map(|(idx, _)| idx)
            .find(|idx| Self::starts_with_text_tool_marker(&text[*idx..]))
    }

    pub(super) fn text_tool_marker_candidate_suffix_len(text: &str) -> usize {
        text.char_indices()
            .map(|(idx, _)| idx)
            .filter_map(|idx| {
                let suffix = &text[idx..];
                let suffix_lower = suffix.to_lowercase();
                Self::text_tool_marker_starts()
                    .iter()
                    .map(|marker| marker.to_lowercase())
                    .any(|marker| suffix.len() < marker.len() && marker.starts_with(&suffix_lower))
                    .then_some(suffix.len())
            })
            .max()
            .unwrap_or(0)
    }

    pub(super) fn text_tool_markup_delta_closes(delta: &str) -> bool {
        let normalized = Self::normalize_text_tool_markup(delta);
        let lower = normalized.to_lowercase();
        lower.contains("</tool_call>")
            || lower.contains("[/tool_call]")
            || lower.contains("</tool_calls>")
            || lower.contains("</minimax:tool_call>")
            || lower.contains("</invoke>")
    }

    pub(super) fn parse_text_tool_calls(content: &str) -> (String, Vec<ToolCallRequest>) {
        let normalized_content = Self::normalize_text_tool_markup(content);
        let content = normalized_content.as_str();
        let mut tool_calls = Vec::new();
        let mut remaining = String::new();
        let mut rest = content;
        let mut call_index = 0u64;

        // Pass 1: extract <tool_call>...</tool_call> blocks
        loop {
            if let Some(start) = rest.find("<tool_call>") {
                remaining.push_str(&rest[..start]);
                let after_tag = &rest[start + "<tool_call>".len()..];
                if let Some(end) = after_tag.find("</tool_call>") {
                    let block = after_tag[..end].trim();
                    if let Ok(val) = serde_json::from_str::<Value>(block) {
                        let name = val
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let arguments = val
                            .get("arguments")
                            .cloned()
                            .unwrap_or(Value::Object(serde_json::Map::new()));
                        tool_calls.push(ToolCallRequest {
                            id: format!("text_call_{}", call_index),
                            name,
                            arguments,
                            thought_signature: None,
                        });
                        call_index += 1;
                    } else if let Some(tc) =
                        Self::parse_function_parameter_tool_block(block, call_index)
                    {
                        tool_calls.push(tc);
                        call_index += 1;
                    } else {
                        warn!(block_len = block.len(), "Failed to parse tool_call JSON");
                        remaining.push_str(
                            &rest[start..start + "<tool_call>".len() + end + "</tool_call>".len()],
                        );
                    }
                    rest = &after_tag[end + "</tool_call>".len()..];
                } else {
                    remaining.push_str(&rest[start..]);
                    break;
                }
            } else {
                remaining.push_str(rest);
                break;
            }
        }

        // Pass 2: extract [TOOL_CALL]...[/TOOL_CALL] blocks from remaining
        // Some models (e.g. xminimaxm25) use this format with non-JSON arrow syntax.
        if tool_calls.is_empty() {
            let mut pass2_remaining = String::new();
            let mut rest2 = remaining.as_str();
            loop {
                let lower = rest2.to_lowercase();
                if let Some(start) = lower.find("[tool_call]") {
                    pass2_remaining.push_str(&rest2[..start]);
                    let after_tag = &rest2[start + "[tool_call]".len()..];
                    let after_lower = after_tag.to_lowercase();
                    if let Some(end) = after_lower.find("[/tool_call]") {
                        let block = after_tag[..end].trim();
                        if let Some(tc) = Self::parse_nonstandard_tool_block(block, call_index) {
                            tool_calls.push(tc);
                            call_index += 1;
                        } else {
                            warn!(block_len = block.len(), "Failed to parse [TOOL_CALL] block");
                        }
                        rest2 = &after_tag[end + "[/tool_call]".len()..];
                    } else {
                        // No closing tag — try to parse what's left
                        let block = after_tag.trim();
                        if let Some(tc) = Self::parse_nonstandard_tool_block(block, call_index) {
                            tool_calls.push(tc);
                        }
                        break;
                    }
                } else {
                    pass2_remaining.push_str(rest2);
                    break;
                }
            }
            remaining = pass2_remaining;
        }

        // Pass 3: extract <minimax:tool_call> / [Called: name] ... </minimax:tool_call> blocks.
        // Format observed from xminimaxm25:
        //   [Called: exec]\n<parameter name="command">...</parameter>\n</invoke>\n</minimax:tool_call>
        // Also handles bare [Called: name] with following <parameter> tags (no closing tag).
        if tool_calls.is_empty() {
            let mut pass3_remaining = String::new();
            let mut rest3 = remaining.as_str();
            loop {
                // Look for [Called: <name>] prefix
                let lower3 = rest3.to_lowercase();
                if let Some(called_start) = lower3.find("[called:") {
                    pass3_remaining.push_str(&rest3[..called_start]);
                    let after_called = &rest3[called_start + "[called:".len()..];
                    // Extract tool name up to ']'
                    if let Some(bracket_end) = after_called.find(']') {
                        let tool_name = after_called[..bracket_end].trim().to_string();
                        let after_bracket = &after_called[bracket_end + 1..];
                        // Find end of this block: </minimax:tool_call> or </invoke>
                        let lower_after = after_bracket.to_lowercase();
                        let block_end = lower_after
                            .find("</minimax:tool_call>")
                            .or_else(|| lower_after.find("</invoke>"));
                        let (params_str, consumed) = if let Some(end) = block_end {
                            let tag_len = if lower_after[end..].starts_with("</minimax:tool_call>")
                            {
                                "</minimax:tool_call>".len()
                            } else {
                                "</invoke>".len()
                            };
                            (&after_bracket[..end], end + tag_len)
                        } else {
                            (after_bracket, after_bracket.len())
                        };
                        // Parse <parameter name="key">value</parameter> pairs
                        let mut args = serde_json::Map::new();
                        let mut scan = params_str;
                        loop {
                            let sl = scan.to_lowercase();
                            if let Some(p_start) = sl.find("<parameter") {
                                let after_p = &scan[p_start + "<parameter".len()..];
                                // Extract name="..."
                                if let Some(name_start) = after_p.find("name=\"") {
                                    let after_name = &after_p[name_start + "name=\"".len()..];
                                    if let Some(name_end) = after_name.find('"') {
                                        let param_name = after_name[..name_end].to_string();
                                        // Find > then value then </parameter>
                                        if let Some(gt) = after_p.find('>') {
                                            let value_str = &after_p[gt + 1..];
                                            let vl = value_str.to_lowercase();
                                            if let Some(close) = vl.find("</parameter>") {
                                                let value = value_str[..close].to_string();
                                                args.insert(
                                                    param_name,
                                                    serde_json::Value::String(value),
                                                );
                                                scan = &value_str[close + "</parameter>".len()..];
                                                continue;
                                            }
                                        }
                                    }
                                }
                                // Couldn't parse this parameter, skip past it
                                scan = &scan[p_start + "<parameter".len()..];
                            } else {
                                break;
                            }
                        }
                        if !tool_name.is_empty() {
                            tool_calls.push(ToolCallRequest {
                                id: format!("text_call_{}", call_index),
                                name: tool_name,
                                arguments: serde_json::Value::Object(args),
                                thought_signature: None,
                            });
                            call_index += 1;
                        }
                        rest3 = &after_called[bracket_end + 1 + consumed..];
                    } else {
                        pass3_remaining.push_str(&rest3[called_start..]);
                        break;
                    }
                } else {
                    pass3_remaining.push_str(rest3);
                    break;
                }
            }
            remaining = pass3_remaining;
        }

        // Pass 4: extract <invoke name="tool_name">...<parameter name="key">value</parameter>...</invoke>
        // with optional </minimax:tool_call> wrapper.
        // Format observed from xminimaxm25:
        //   <invoke name="list_skills">\n</invoke>\n</minimax:tool_call>
        //   <invoke name="exec">\n<parameter name="command">ls -la</parameter>\n</invoke>\n</minimax:tool_call>
        if tool_calls.is_empty() {
            let mut pass4_remaining = String::new();
            let mut rest4 = remaining.as_str();
            loop {
                let lower4 = rest4.to_lowercase();
                if let Some(invoke_start) = lower4.find("<invoke") {
                    pass4_remaining.push_str(&rest4[..invoke_start]);
                    let after_invoke = &rest4[invoke_start + "<invoke".len()..];
                    // Extract name="..." from the <invoke> tag
                    if let Some(name_attr_start) = after_invoke.find("name=\"") {
                        let after_name = &after_invoke[name_attr_start + "name=\"".len()..];
                        if let Some(name_end) = after_name.find('"') {
                            let tool_name = after_name[..name_end].trim().to_string();
                            // Find the > that closes the <invoke ...> tag
                            let tag_content_start =
                                &after_invoke[name_attr_start + "name=\"".len() + name_end + 1..];
                            if let Some(gt_pos) = tag_content_start.find('>') {
                                let body = &tag_content_start[gt_pos + 1..];
                                // Find </invoke> end
                                let body_lower = body.to_lowercase();
                                let invoke_end = body_lower.find("</invoke>");
                                let (params_str, after_body) = if let Some(end) = invoke_end {
                                    (&body[..end], &body[end + "</invoke>".len()..])
                                } else {
                                    (body, "")
                                };
                                // Parse <parameter name="key">value</parameter> pairs
                                let mut args = serde_json::Map::new();
                                let mut scan = params_str;
                                loop {
                                    let sl = scan.to_lowercase();
                                    if let Some(p_start) = sl.find("<parameter") {
                                        let after_p = &scan[p_start + "<parameter".len()..];
                                        if let Some(ns) = after_p.find("name=\"") {
                                            let an = &after_p[ns + "name=\"".len()..];
                                            if let Some(ne) = an.find('"') {
                                                let param_name = an[..ne].to_string();
                                                if let Some(gt) = after_p.find('>') {
                                                    let value_str = &after_p[gt + 1..];
                                                    let vl = value_str.to_lowercase();
                                                    if let Some(close) = vl.find("</parameter>") {
                                                        let value = value_str[..close].to_string();
                                                        // Try to parse as JSON value (number, bool, etc.)
                                                        let json_val =
                                                            serde_json::from_str::<Value>(&value)
                                                                .unwrap_or(Value::String(value));
                                                        args.insert(param_name, json_val);
                                                        scan = &value_str
                                                            [close + "</parameter>".len()..];
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                        scan = &scan[p_start + "<parameter".len()..];
                                    } else {
                                        break;
                                    }
                                }
                                if !tool_name.is_empty() {
                                    tool_calls.push(ToolCallRequest {
                                        id: format!("text_call_{}", call_index),
                                        name: tool_name,
                                        arguments: Value::Object(args),
                                        thought_signature: None,
                                    });
                                    call_index += 1;
                                }
                                // Skip optional </minimax:tool_call> after </invoke>
                                let trimmed_after = after_body.trim_start();
                                rest4 = if trimmed_after
                                    .to_lowercase()
                                    .starts_with("</minimax:tool_call>")
                                {
                                    &trimmed_after["</minimax:tool_call>".len()..]
                                } else {
                                    after_body
                                };
                                continue;
                            }
                        }
                    }
                    // Couldn't parse this <invoke>, skip it
                    pass4_remaining.push_str(&rest4[invoke_start..invoke_start + "<invoke".len()]);
                    rest4 = &rest4[invoke_start + "<invoke".len()..];
                } else {
                    pass4_remaining.push_str(rest4);
                    break;
                }
            }
            remaining = pass4_remaining;
        }

        let remaining = Self::strip_text_tool_wrappers(&remaining)
            .trim()
            .to_string();
        (remaining, tool_calls)
    }

    /// Parse a non-standard tool call block content.
    /// Handles formats like:
    ///   {tool => "memory_query", args => { --top_k 20 }}
    ///   {"name": "memory_query", "arguments": {"top_k": 20}}
    pub(super) fn parse_nonstandard_tool_block(block: &str, index: u64) -> Option<ToolCallRequest> {
        // Try standard JSON first
        if let Ok(val) = serde_json::from_str::<Value>(block) {
            let name = val
                .get("name")
                .or_else(|| val.get("tool"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let arguments = val
                .get("arguments")
                .or_else(|| val.get("args"))
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));
            return Some(ToolCallRequest {
                id: format!("text_call_{}", index),
                name,
                arguments,
                thought_signature: None,
            });
        }

        // Parse arrow-style: {tool => "name", args => { --key value }}
        // Only strip the outermost brace pair (trim_end_matches is greedy and would strip all)
        let trimmed = block.trim();
        let inner = if trimmed.starts_with('{') && trimmed.ends_with('}') {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };
        let inner = inner.trim();

        // Extract tool name: tool => "name" or tool => name
        let tool_name = Self::extract_arrow_value(inner, "tool")
            .or_else(|| Self::extract_arrow_value(inner, "name"));
        let tool_name = tool_name?;

        // Extract args block
        let args = Self::extract_arrow_args(inner);

        Some(ToolCallRequest {
            id: format!("text_call_{}", index),
            name: tool_name,
            arguments: args,
            thought_signature: None,
        })
    }

    /// Extract a string value from arrow syntax: `key => "value"` or `key => value`
    pub(super) fn extract_arrow_value(text: &str, key: &str) -> Option<String> {
        // Match: key => "value" or key =\u003e "value" (escaped >)
        let patterns = [
            format!("{} =>", key),
            format!("{} =\\u003e", key), // JSON-escaped >
        ];
        for pat in &patterns {
            if let Some(pos) = text.find(pat.as_str()) {
                let after = text[pos + pat.len()..].trim();
                // Quoted value
                if let Some(after) = after.strip_prefix('"') {
                    if let Some(end_quote) = after.find('"') {
                        return Some(after[..end_quote].to_string());
                    }
                }
                // Unquoted value — take until comma or whitespace
                let val: String = after
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != ',' && *c != '}')
                    .collect();
                if !val.is_empty() {
                    return Some(val.trim_matches('"').to_string());
                }
            }
        }
        None
    }

    /// Extract args from arrow syntax: `args => { --key1 val1\n --key2 val2 }`
    /// or `args => {"key": "value"}` (JSON inside)
    pub(super) fn extract_arrow_args(text: &str) -> Value {
        let args_markers = ["args =>", "arguments =>"];
        for marker in &args_markers {
            if let Some(pos) = text.find(marker) {
                let after = text[pos + marker.len()..].trim();
                // Find the args block between { }
                if after.starts_with('{') {
                    // Find matching closing brace
                    let mut depth = 0;
                    let mut end_pos = 0;
                    for (i, ch) in after.char_indices() {
                        match ch {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    end_pos = i;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if end_pos > 0 {
                        let args_block = &after[1..end_pos].trim();
                        // Try JSON first
                        let json_attempt = format!("{{{}}}", args_block);
                        if let Ok(val) = serde_json::from_str::<Value>(&json_attempt) {
                            return val;
                        }
                        // Parse --key value pairs
                        return Self::parse_dash_args(args_block);
                    }
                }
                // Bare value after =>
                let val: String = after
                    .chars()
                    .take_while(|c| *c != ',' && *c != '}')
                    .collect();
                let val = val.trim();
                if !val.is_empty() {
                    let mut map = serde_json::Map::new();
                    map.insert("value".to_string(), Value::String(val.to_string()));
                    return Value::Object(map);
                }
            }
        }
        Value::Object(serde_json::Map::new())
    }

    /// Parse `--key value` or `--key` pairs into a JSON object.
    pub(super) fn parse_dash_args(text: &str) -> Value {
        let mut map = serde_json::Map::new();
        let mut current_key: Option<String> = None;
        let mut current_val_parts: Vec<String> = Vec::new();

        let flush = |key: &mut Option<String>,
                     parts: &mut Vec<String>,
                     map: &mut serde_json::Map<String, Value>| {
            if let Some(k) = key.take() {
                let val_str = parts.join(" ");
                let val_str = val_str.trim().trim_matches('"').trim();
                if val_str.is_empty() {
                    map.insert(k, Value::Bool(true));
                } else if let Ok(n) = val_str.parse::<i64>() {
                    map.insert(k, Value::Number(n.into()));
                } else if let Ok(f) = val_str.parse::<f64>() {
                    if let Some(n) = serde_json::Number::from_f64(f) {
                        map.insert(k, Value::Number(n));
                    } else {
                        map.insert(k, Value::String(val_str.to_string()));
                    }
                } else if val_str == "true" {
                    map.insert(k, Value::Bool(true));
                } else if val_str == "false" {
                    map.insert(k, Value::Bool(false));
                } else {
                    map.insert(k, Value::String(val_str.to_string()));
                }
                parts.clear();
            }
        };

        for token in text.split_whitespace() {
            if let Some(key_name) = token.strip_prefix("--") {
                flush(&mut current_key, &mut current_val_parts, &mut map);
                current_key = Some(key_name.to_string());
            } else if current_key.is_some() {
                current_val_parts.push(token.to_string());
            }
        }
        flush(&mut current_key, &mut current_val_parts, &mut map);

        Value::Object(map)
    }
}
