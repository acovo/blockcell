//! skill_manage 工具的校验与解析辅助函数。
//!
//! 路径/名称校验、frontmatter 解析、信任级别判定、元数据读取等纯函数从
//! `skill_manage.rs` 抽出，供 `SkillManageTool` 的各 action 调用。

use std::path::{Path, PathBuf};

use blockcell_core::Result;
use serde_json::{json, Value};

use crate::security_scan::TrustLevel;

use super::{MAX_DESCRIPTION_LENGTH, VALID_SKILL_NAME_RE, VALID_SKILL_NAME_REGEX};

/// 验证路径组件: 阻止路径遍历 + Skill 名称正则校验
///
/// `is_skill_name`: true 时额外校验名称格式 (仅小写+数字+有限标点)
pub(super) fn validate_path_component(s: &str, is_skill_name: bool) -> Result<()> {
    if s.is_empty() {
        return Err(blockcell_core::Error::Validation(
            "Skill name/category cannot be empty".to_string(),
        ));
    }
    if s.contains('/') || s.contains('\\') || s.contains("..") {
        return Err(blockcell_core::Error::Validation(format!(
            "Skill name/category '{}' contains invalid characters (path separators or '..')",
            s
        )));
    }
    // Skill 名称额外校验: 仅允许小写字母+数字+有限标点
    if is_skill_name && !VALID_SKILL_NAME_REGEX.is_match(s) {
        return Err(blockcell_core::Error::Validation(format!(
            "Invalid skill name '{}': must match {} (lowercase alphanumeric, dots, underscores, hyphens, starting with alphanumeric)",
            s, VALID_SKILL_NAME_RE
        )));
    }
    Ok(())
}

/// 验证 Skill 内部文件路径 (允许 / 分隔符, 但禁止 .. 和反斜杠)
pub(super) fn validate_skill_file_path(s: &str) -> Result<()> {
    if s.is_empty() {
        return Err(blockcell_core::Error::Validation(
            "File path cannot be empty".to_string(),
        ));
    }
    // 禁止路径遍历和反斜杠
    if s.contains("..") || s.contains('\\') {
        return Err(blockcell_core::Error::Validation(format!(
            "File path '{}' contains path traversal or backslash",
            s
        )));
    }
    // 验证每个路径组件不为空
    for component in s.split('/') {
        if component.is_empty() {
            return Err(blockcell_core::Error::Validation(format!(
                "File path '{}' contains empty path component",
                s
            )));
        }
    }
    Ok(())
}

/// Ensure an existing or not-yet-created target remains below `root` after
/// resolving every existing ancestor. This rejects directory symlinks before
/// callers create, overwrite, or remove files through them.
pub(super) fn ensure_confined_target(root: &Path, target: &Path) -> Result<()> {
    let canonical_root = std::fs::canonicalize(root).map_err(blockcell_core::Error::Io)?;
    let mut existing = target;
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| {
            blockcell_core::Error::PermissionDenied(format!(
                "Target '{}' has no existing confined ancestor",
                target.display()
            ))
        })?;
    }

    let canonical_existing = std::fs::canonicalize(existing).map_err(blockcell_core::Error::Io)?;
    if !canonical_existing.starts_with(&canonical_root) {
        return Err(blockcell_core::Error::PermissionDenied(format!(
            "Target '{}' resolves outside '{}'",
            target.display(),
            root.display()
        )));
    }

    if target.exists() {
        let canonical_target = std::fs::canonicalize(target).map_err(blockcell_core::Error::Io)?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err(blockcell_core::Error::PermissionDenied(format!(
                "Target '{}' resolves outside '{}'",
                target.display(),
                root.display()
            )));
        }
    }
    Ok(())
}

/// 查找 Skill 目录 (支持 category/name 和直接 name 两种路径, 支持跨目录搜索)
///
/// 搜索顺序:
/// 1. workspace/skills (主目录)
/// 2. builtin_skills_dir (内置 Skill 目录, 如 ~/.blockcell/skills)
pub(super) fn find_skill_dir(
    name: &str,
    skills_dir: &Path,
    external_dirs: &[PathBuf],
) -> Result<PathBuf> {
    // 验证 name 不含路径遍历
    validate_path_component(name, true)?;

    // 在指定目录列表中搜索 (主目录优先)
    let mut search_dirs: Vec<&Path> = vec![skills_dir];
    for dir in external_dirs {
        if dir != skills_dir && dir.exists() {
            search_dirs.push(dir);
        }
    }

    for dir in &search_dirs {
        // 先尝试直接匹配 ({dir}/{name})
        let direct = dir.join(name);
        if direct.is_dir()
            && !direct.is_symlink()
            && direct.join("SKILL.md").exists()
            && !direct.join("SKILL.md").is_symlink()
        {
            return Ok(direct);
        }

        // 遍历 category 子目录查找
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && !path.is_symlink() {
                    let candidate = path.join(name);
                    if candidate.is_dir()
                        && !candidate.is_symlink()
                        && candidate.join("SKILL.md").exists()
                        && !candidate.join("SKILL.md").is_symlink()
                    {
                        return Ok(candidate);
                    }
                }
            }
        }
    }

    Err(blockcell_core::Error::Skill(format!(
        "Skill '{}' not found in {} (searched {} directories)",
        name,
        skills_dir.display(),
        search_dirs.len()
    )))
}

/// 根据 Skill 目录位置确定信任级别
///
/// - builtin_skills_dir 下的 Skill → Builtin (最宽松)
/// - workspace/skills 下的 Skill → Trusted (默认)
/// - 其他位置 → Community (较严格)
pub(super) fn determine_trust_level(
    skill_dir: &Path,
    builtin_skills_dir: Option<&Path>,
) -> TrustLevel {
    if let Some(builtin) = builtin_skills_dir {
        if skill_dir.starts_with(builtin) {
            return TrustLevel::Builtin;
        }
    }
    // workspace/skills 下的 Skill 默认为 Trusted
    TrustLevel::Trusted
}

/// 从 SKILL.md 内容中提取 YAML frontmatter
pub fn extract_frontmatter(content: &str) -> Value {
    let trimmed = content.trim();

    // 检查是否有 YAML frontmatter (--- ... ---)
    if !trimmed.starts_with("---") {
        return json!({});
    }

    let rest = &trimmed[3..];
    if let Some(end_idx) = rest.find("---") {
        let frontmatter = &rest[..end_idx];
        let mut meta = serde_json::Map::new();

        for line in frontmatter.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim().to_string();
                let val = val.trim();
                // 处理常见类型
                if val == "true" {
                    meta.insert(key, Value::Bool(true));
                } else if val == "false" {
                    meta.insert(key, Value::Bool(false));
                } else if let Ok(num) = val.parse::<i64>() {
                    meta.insert(key, Value::Number(num.into()));
                } else if val.starts_with('[') && val.ends_with(']') {
                    // 简单数组解析: [a, b, c]
                    let items: Vec<Value> = val[1..val.len() - 1]
                        .split(',')
                        .map(|s| Value::String(s.trim().to_string()))
                        .collect();
                    meta.insert(key, Value::Array(items));
                } else {
                    // 去除引号
                    let val = val
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .unwrap_or(val);
                    let val = val
                        .strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                        .unwrap_or(val);
                    meta.insert(key, Value::String(val.to_string()));
                }
            }
        }

        return Value::Object(meta);
    }

    json!({})
}

/// 验证 frontmatter 必须包含 name 和 description 字段
pub(super) fn validate_frontmatter(frontmatter: &serde_json::Value) -> Result<()> {
    let name = frontmatter.get("name").and_then(|v| v.as_str());
    let description = frontmatter.get("description").and_then(|v| v.as_str());

    if name.is_none() || name.is_none_or(|n| n.trim().is_empty()) {
        return Err(blockcell_core::Error::Validation(
            "Skill frontmatter must contain a non-empty 'name' field".to_string(),
        ));
    }
    if description.is_none() || description.is_none_or(|d| d.trim().is_empty()) {
        return Err(blockcell_core::Error::Validation(
            "Skill frontmatter must contain a non-empty 'description' field".to_string(),
        ));
    }
    // 描述长度限制
    if description.is_some_and(|d| d.len() > MAX_DESCRIPTION_LENGTH) {
        return Err(blockcell_core::Error::Validation(format!(
            "Skill description exceeds maximum length of {} characters",
            MAX_DESCRIPTION_LENGTH
        )));
    }
    Ok(())
}

/// 验证 SKILL.md 内容在 frontmatter 之后有 body 内容
/// (防止创建只有 frontmatter 没有实际内容的空 Skill)
pub(super) fn validate_skill_body(content: &str) -> Result<()> {
    // 提取 body: 去掉 frontmatter 后的内容
    let body = if let Some(rest) = content.trim().strip_prefix("---") {
        if let Some(end_idx) = rest.find("---") {
            rest[end_idx + 3..].trim()
        } else {
            content.trim()
        }
    } else {
        content.trim()
    };

    // Body 必须有实质内容 (至少 10 个非空白字符)
    let non_whitespace_count = body.chars().filter(|c| !c.is_whitespace()).count();
    if non_whitespace_count < 10 {
        return Err(blockcell_core::Error::Validation(
            "Skill content must have body text after frontmatter (at least 10 non-whitespace characters)".to_string(),
        ));
    }
    Ok(())
}

/// 读取 meta.json
pub(super) fn read_meta_json(skill_dir: &Path) -> Value {
    let meta_path = skill_dir.join("meta.json");
    if meta_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<Value>(&content) {
                return meta;
            }
        }
    }

    // 回退到 meta.yaml
    let yaml_path = skill_dir.join("meta.yaml");
    if yaml_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&yaml_path) {
            let mut meta = serde_json::Map::new();
            for line in content.lines() {
                if let Some((key, val)) = line.split_once(':') {
                    let key = key.trim().to_string();
                    let val = val.trim();
                    if val == "true" {
                        meta.insert(key, Value::Bool(true));
                    } else if val == "false" {
                        meta.insert(key, Value::Bool(false));
                    } else {
                        meta.insert(key, Value::String(val.to_string()));
                    }
                }
            }
            if !meta.is_empty() {
                return Value::Object(meta);
            }
        }
    }

    json!({})
}

/// 列出子目录中的文件
pub(super) fn list_subdir_files(dir: &Path) -> Vec<String> {
    if !dir.exists() {
        return Vec::new();
    }

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.push(name.to_string());
                }
            }
        }
    }
    files.sort();
    files
}
