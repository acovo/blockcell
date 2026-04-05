use crate::evolution::{SkillLayout, SkillType};

/// Result of static (deterministic) audit — runs before LLM audit.
#[derive(Debug, Clone)]
pub struct StaticAuditResult {
    pub passed: bool,
    pub violations: Vec<StaticViolation>,
}

/// A single static audit violation.
#[derive(Debug, Clone)]
pub struct StaticViolation {
    pub severity: &'static str, // "error" or "warning"
    pub rule: &'static str,
    pub message: String,
}

/// Dangerous patterns for each skill type.
/// Inspired by claude-code's SAFE_COMMANDS / permission layering approach.
const RHAI_DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("remove_dir", "Detected directory removal operation"),
    ("delete_file", "Detected file deletion operation"),
    ("exec(", "Detected shell execution — potential command injection"),
    ("eval(", "Detected eval — potential code injection"),
];

const PYTHON_DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("os.remove(", "Detected os.remove — file deletion"),
    ("os.unlink(", "Detected os.unlink — file deletion"),
    ("shutil.rmtree(", "Detected shutil.rmtree — recursive directory removal"),
    ("subprocess.call(", "Detected subprocess.call — potential shell injection"),
    ("subprocess.Popen(", "Detected subprocess.Popen — potential shell injection"),
    ("os.system(", "Detected os.system — shell execution"),
    ("eval(", "Detected eval — potential code injection"),
    ("exec(", "Detected exec — potential code injection"),
    ("__import__(", "Detected dynamic import — potential security risk"),
];

const LOCAL_SCRIPT_DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf", "Detected recursive removal command"),
    ("sudo ", "Detected privileged command execution"),
    ("curl ", "Detected network download command"),
    ("wget ", "Detected network download command"),
    ("curl | sh", "Detected download-and-execute pattern"),
    ("wget | sh", "Detected download-and-execute pattern"),
    ("eval ", "Detected shell eval — potential command injection"),
    ("exec ", "Detected exec-like command execution"),
];

/// Run static audit on generated code before sending to LLM audit.
///
/// This is a fast, deterministic check that catches obvious dangerous patterns
/// without consuming LLM tokens. Returns immediately if the code is clean.
pub fn static_audit(skill_type: &SkillType, code: &str) -> StaticAuditResult {
    let layout = match skill_type {
        SkillType::Rhai => SkillLayout::RhaiOrchestration,
        SkillType::Python => SkillLayout::Hybrid,
        SkillType::LocalScript => SkillLayout::LocalScript,
        SkillType::PromptOnly => SkillLayout::PromptTool,
    };

    static_audit_with_layout(&layout, skill_type, code)
}

pub fn static_audit_with_layout(
    layout: &SkillLayout,
    skill_type: &SkillType,
    code: &str,
) -> StaticAuditResult {
    let mut violations = Vec::new();

    match layout {
        SkillLayout::RhaiOrchestration => {
            check_patterns(code, RHAI_DANGEROUS_PATTERNS, &mut violations);
            check_rhai_specific(code, &mut violations);
        }
        SkillLayout::PromptTool => {
            check_prompt_only(code, &mut violations);
        }
        SkillLayout::LocalScript => {
            check_patterns(code, LOCAL_SCRIPT_DANGEROUS_PATTERNS, &mut violations);
            check_local_script_specific(code, &mut violations);
        }
        SkillLayout::Hybrid => match skill_type {
            SkillType::Python => {
                check_patterns(code, PYTHON_DANGEROUS_PATTERNS, &mut violations);
                check_python_specific(code, &mut violations);
            }
            SkillType::LocalScript => {
                check_patterns(code, LOCAL_SCRIPT_DANGEROUS_PATTERNS, &mut violations);
                check_local_script_specific(code, &mut violations);
            }
            SkillType::Rhai => {
                check_patterns(code, RHAI_DANGEROUS_PATTERNS, &mut violations);
                check_rhai_specific(code, &mut violations);
            }
            SkillType::PromptOnly => {
                check_prompt_only(code, &mut violations);
            }
        },
    }

    // Common checks for all types
    check_common(code, &mut violations);

    let passed = !violations.iter().any(|v| v.severity == "error");
    StaticAuditResult { passed, violations }
}

/// Check code against a list of dangerous patterns.
fn check_patterns(
    code: &str,
    patterns: &[(&str, &str)],
    violations: &mut Vec<StaticViolation>,
) {
    for &(pattern, description) in patterns {
        if code.contains(pattern) {
            violations.push(StaticViolation {
                severity: "warning",
                rule: "dangerous_operation",
                message: format!("{}: found `{}`", description, pattern),
            });
        }
    }
}

/// Rhai-specific checks.
fn check_rhai_specific(code: &str, violations: &mut Vec<StaticViolation>) {
    // Check for unbounded loops without break
    if (code.contains("loop {") || code.contains("loop{"))
        && !code.contains("break")
    {
        violations.push(StaticViolation {
            severity: "error",
            rule: "infinite_loop",
            message: "Detected `loop {}` without any `break` statement — potential infinite loop".to_string(),
        });
    }

    // Check for while true without break
    if code.contains("while true") && !code.contains("break") {
        violations.push(StaticViolation {
            severity: "error",
            rule: "infinite_loop",
            message: "Detected `while true` without any `break` statement — potential infinite loop".to_string(),
        });
    }

    // Check for JavaScript/TypeScript syntax accidentally generated
    let js_patterns = ["const ", "=> {", "async ", "await ", "require(", "import "];
    for pattern in &js_patterns {
        if code.contains(pattern) {
            violations.push(StaticViolation {
                severity: "error",
                rule: "wrong_language",
                message: format!("Detected non-Rhai syntax `{}` — skill must be pure Rhai", pattern.trim()),
            });
        }
    }
}

/// Python-specific checks.
fn check_python_specific(code: &str, violations: &mut Vec<StaticViolation>) {
    // Check for infinite loops without break
    if code.contains("while True") && !code.contains("break") {
        violations.push(StaticViolation {
            severity: "warning",
            rule: "infinite_loop",
            message: "Detected `while True` without `break` — potential infinite loop".to_string(),
        });
    }

    // Check for hardcoded credentials
    let secret_patterns = ["password=", "api_key=", "secret=", "token="];
    for pattern in &secret_patterns {
        // Only flag if followed by a string literal (not a variable)
        let search = format!("{}\"", pattern);
        let search2 = format!("{}'", pattern);
        if code.contains(&search) || code.contains(&search2) {
            violations.push(StaticViolation {
                severity: "warning",
                rule: "hardcoded_secret",
                message: format!("Possible hardcoded secret near `{}`", pattern),
            });
        }
    }
}

/// Local-script specific checks.
fn check_local_script_specific(code: &str, violations: &mut Vec<StaticViolation>) {
    if code.contains("rm -rf /") || code.contains("rm -rf ~") {
        violations.push(StaticViolation {
            severity: "error",
            rule: "dangerous_operation",
            message: "Detected destructive recursive delete command".to_string(),
        });
    }

    if code.contains("curl") && code.contains("| sh") {
        violations.push(StaticViolation {
            severity: "error",
            rule: "dangerous_operation",
            message: "Detected download-and-execute shell pattern".to_string(),
        });
    }

    if code.contains("wget") && code.contains("| sh") {
        violations.push(StaticViolation {
            severity: "error",
            rule: "dangerous_operation",
            message: "Detected download-and-execute shell pattern".to_string(),
        });
    }

    let shell_patterns = ["set -e", "set -u", "set -o pipefail"];
    if !shell_patterns.iter().any(|pattern| code.contains(pattern)) {
        violations.push(StaticViolation {
            severity: "warning",
            rule: "shell_hardening",
            message: "Consider using `set -euo pipefail` or equivalent hardening for shell scripts".to_string(),
        });
    }
}

/// PromptOnly-specific checks.
fn check_prompt_only(code: &str, violations: &mut Vec<StaticViolation>) {
    // Content must be substantive
    if code.trim().len() < 100 {
        violations.push(StaticViolation {
            severity: "error",
            rule: "too_short",
            message: format!(
                "SKILL.md content is too short ({} chars, minimum 100)",
                code.trim().len()
            ),
        });
    }

    // Must have at least one heading
    if !code.contains('#') {
        violations.push(StaticViolation {
            severity: "warning",
            rule: "no_structure",
            message: "SKILL.md has no markdown headings — document should be structured".to_string(),
        });
    }
}

/// Common checks for all skill types.
fn check_common(code: &str, violations: &mut Vec<StaticViolation>) {
    // Check for extremely large generated code (likely garbage)
    if code.len() > 100_000 {
        violations.push(StaticViolation {
            severity: "error",
            rule: "too_large",
            message: format!(
                "Generated code is too large ({} bytes, max 100KB)",
                code.len()
            ),
        });
    }

    // Check for empty content
    if code.trim().is_empty() {
        violations.push(StaticViolation {
            severity: "error",
            rule: "empty_content",
            message: "Generated code is empty".to_string(),
        });
    }
}

/// Format static audit result as a human-readable string (for feedback to LLM on retry).
pub fn format_static_audit_feedback(result: &StaticAuditResult) -> String {
    if result.passed && result.violations.is_empty() {
        return "Static audit passed with no issues.".to_string();
    }

    let mut feedback = String::from("Static audit issues found:\n");
    for v in &result.violations {
        feedback.push_str(&format!("- [{}] {}: {}\n", v.severity, v.rule, v.message));
    }
    feedback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rhai_clean_code_passes() {
        let code = r#"
let result = call_tool("web_search", #{ query: "test" });
let text = result.content;
print(text);
"#;
        let result = static_audit(&SkillType::Rhai, code);
        assert!(result.passed);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_rhai_infinite_loop_detected() {
        let code = r#"
loop {
    let x = 1;
}
"#;
        let result = static_audit(&SkillType::Rhai, code);
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.rule == "infinite_loop"));
    }

    #[test]
    fn test_rhai_js_syntax_detected() {
        let code = r#"
const x = 42;
let fn_result = async () => { await something(); };
"#;
        let result = static_audit(&SkillType::Rhai, code);
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.rule == "wrong_language"));
    }

    #[test]
    fn test_python_dangerous_patterns() {
        let code = r#"
import os
os.system("rm -rf /")
"#;
        let result = static_audit(&SkillType::Python, code);
        assert!(result.violations.iter().any(|v| v.rule == "dangerous_operation"));
    }

    #[test]
    fn test_prompt_only_too_short() {
        let code = "# Hello\nShort.";
        let result = static_audit(&SkillType::PromptOnly, code);
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.rule == "too_short"));
    }

    #[test]
    fn test_empty_content_fails() {
        let result = static_audit(&SkillType::Rhai, "   ");
        assert!(!result.passed);
        assert!(result.violations.iter().any(|v| v.rule == "empty_content"));
    }

    #[test]
    fn test_format_feedback() {
        let result = StaticAuditResult {
            passed: false,
            violations: vec![StaticViolation {
                severity: "error",
                rule: "infinite_loop",
                message: "Detected loop without break".to_string(),
            }],
        };
        let feedback = format_static_audit_feedback(&result);
        assert!(feedback.contains("infinite_loop"));
        assert!(feedback.contains("error"));
    }
}
