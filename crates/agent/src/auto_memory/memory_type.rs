//! 记忆类型定义
//!
//! 四种持久化记忆类型及其存储路径。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use blockcell_core::{Paths, Result};

/// 记忆类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MemoryType {
    /// 用户记忆 - 角色、偏好、知识背景
    User,
    /// 项目记忆 - 工作内容、目标、事件
    Project,
    /// 反馈记忆 - 用户纠正、工作指导
    Feedback,
    /// 引用记忆 - 外部系统资源指针
    Reference,
}

impl MemoryType {
    /// 获取所有记忆类型
    pub fn all() -> Vec<Self> {
        vec![Self::User, Self::Project, Self::Feedback, Self::Reference]
    }

    /// 获取记忆类型名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Feedback => "feedback",
            Self::Reference => "reference",
        }
    }

    /// 获取记忆文件名
    pub fn filename(&self) -> &'static str {
        match self {
            Self::User => "user.md",
            Self::Project => "project.md",
            Self::Feedback => "feedback.md",
            Self::Reference => "reference.md",
        }
    }

    /// Canonical durable-memory file after Layer 5 source consolidation.
    pub fn canonical_filename(&self) -> &'static str {
        match self {
            Self::User => "USER.md",
            Self::Project | Self::Feedback | Self::Reference => "MEMORY.md",
        }
    }

    /// 获取记忆类型描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::User => "用户角色、偏好、知识背景",
            Self::Project => "项目工作、目标、事件",
            Self::Feedback => "用户纠正、工作指导",
            Self::Reference => "外部系统资源指针",
        }
    }

    /// 获取记忆类型用途说明
    pub fn usage_guide(&self) -> &'static str {
        match self {
            Self::User => {
                "记录用户的永久信息，用于个性化 AI 助手：\n\
                - 用户角色和职责\n\
                - 技术背景和知识领域\n\
                - 代码风格偏好\n\
                - 沟通习惯\n\
                - 常见请求模式\n\n\
                **何时保存**: 当用户提供个人信息或表达偏好时\n\
                **如何使用**: 为用户提供个性化建议和解释"
            }
            Self::Project => {
                "记录项目相关的持久信息：\n\
                - 项目目标和里程碑\n\
                - 当前进度和状态\n\
                - 重要决策和决策理由\n\
                - 技术栈和架构选择\n\
                - 团队成员和职责分配\n\n\
                **何时保存**: 当讨论项目进度、决策或计划时\n\
                **如何使用**: 保持项目上下文连贯性"
            }
            Self::Feedback => {
                "记录用户的纠正和指导，改进 AI 行为：\n\
                - 用户明确纠正的行为\n\
                - 用户要求的工作方式\n\
                - 成功或失败的方法记录\n\
                - 需要避免的做法\n\n\
                **格式**: 每条包含规则 + 原因 + 应用场景\n\n\
                **何时保存**: 当用户说 \"不要做X\" 或 \"保持做Y\" 时\n\
                **如何使用**: 在后续对话中遵循用户指导"
            }
            Self::Reference => {
                "记录外部系统资源的指针：\n\
                - 文档链接和位置\n\
                - API 端点和配置\n\
                - 外部系统访问方式\n\
                - 重要文件路径\n\n\
                **何时保存**: 当用户提到外部资源时\n\
                **如何使用**: 快速定位和访问外部系统"
            }
        }
    }

    /// 获取记忆文件模板
    pub fn template(&self) -> &'static str {
        match self {
            Self::User => {
                "---\n\
                name: user_memory\n\
                description: 用户角色、偏好、知识背景\n\
                type: user\n\
                ---\n\n\
                # User Memory\n\n\
                此文件记录关于用户的永久信息。\n\n\
                ## Role and Responsibilities\n\n\
                _用户的主要角色和工作职责._\n\n\
                ## Technical Background\n\n\
                _用户的技术背景和知识领域._\n\n\
                ## Preferences\n\n\
                _用户的偏好和习惯._\n\n\
                ## Common Requests\n\n\
                _用户常见的请求模式._"
            }
            Self::Project => {
                "---\n\
                name: project_memory\n\
                description: 项目工作、目标、事件\n\
                type: project\n\
                ---\n\n\
                # Project Memory\n\n\
                此文件记录项目相关的持久信息。\n\n\
                ## Project Goals\n\n\
                _项目的主要目标和里程碑._\n\n\
                ## Current Status\n\n\
                _当前进度和状态._\n\n\
                ## Key Decisions\n\n\
                _重要决策及其理由._\n\n\
                ## Team Structure\n\n\
                _团队成员和职责._"
            }
            Self::Feedback => {
                "---\n\
                name: feedback_memory\n\
                description: 用户纠正、工作指导\n\
                type: feedback\n\
                ---\n\n\
                # Feedback Memory\n\n\
                此文件记录用户的纠正和指导。\n\n\
                **格式**: 每条记录包含规则、原因和应用场景。\n\n\
                ## Work Guidance\n\n\
                _用户要求的工作方式._\n\n\
                ## Corrections\n\n\
                _用户纠正的行为._\n\n\
                ## Approaches to Avoid\n\n\
                _需要避免的做法._"
            }
            Self::Reference => {
                "---\n\
                name: reference_memory\n\
                description: 外部系统资源指针\n\
                type: reference\n\
                ---\n\n\
                # Reference Memory\n\n\
                此文件记录外部系统资源的指针。\n\n\
                ## Documentation\n\n\
                _重要文档链接和位置._\n\n\
                ## External Systems\n\n\
                _外部系统访问方式._\n\n\
                ## API References\n\n\
                _API 端点和配置._"
            }
        }
    }
}

/// 记忆文件名映射
pub const MEMORY_FILE_NAMES: &[(&str, &str)] = &[
    ("user", "user.md"),
    ("project", "project.md"),
    ("feedback", "feedback.md"),
    ("reference", "reference.md"),
];

/// 获取记忆文件路径
pub fn get_memory_file_path(config_dir: &Path, memory_type: MemoryType) -> PathBuf {
    config_dir
        .join("memory")
        .join(memory_type.name())
        .with_extension("md")
}

/// 确保记忆目录存在
pub async fn ensure_memory_dir(config_dir: &Path) -> std::io::Result<()> {
    let memory_dir = config_dir.join("memory");
    tokio::fs::create_dir_all(&memory_dir).await?;

    // 为每种类型创建初始文件（如果不存在）
    for memory_type in MemoryType::all() {
        let file_path = get_memory_file_path(config_dir, memory_type);
        if !tokio::fs::try_exists(&file_path).await? {
            tokio::fs::write(&file_path, memory_type.template()).await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Layer5ConsolidationResult {
    pub migrated: usize,
    pub legacy_files: usize,
}

/// Merge legacy Layer 5 files into the two canonical durable-memory files.
///
/// The operation is intentionally idempotent: normalized content already present
/// in canonical files is skipped, and migrated legacy files are reset to their
/// compatibility templates so durable facts no longer have two writable sources.
pub fn consolidate_legacy_layer5(paths: &Paths) -> Result<Layer5ConsolidationResult> {
    let index = Arc::new(blockcell_storage::KnowledgeIndex::open(
        &paths.knowledge_index_db(),
    )?);
    index.rebuild_from_files(paths)?;
    let mut store = crate::memory_file_store::MemoryFileStore::open(paths)?;
    store.set_knowledge_index(index, "USER.md", "memory/MEMORY.md");

    let mut canonical_text = format!(
        "{}\n{}",
        read_optional_text(&paths.user_md())?,
        read_optional_text(&paths.memory_md())?
    );
    let mut seen = HashSet::new();
    let mut result = Layer5ConsolidationResult::default();

    for memory_type in MemoryType::all() {
        let legacy_path = get_memory_file_path(&paths.base, memory_type);
        let content = match std::fs::read_to_string(&legacy_path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let chunks = legacy_content_chunks(memory_type, &content);
        if chunks.is_empty() {
            continue;
        }
        result.legacy_files += 1;
        tracing::warn!(
            path = %legacy_path.display(),
            canonical = memory_type.canonical_filename(),
            "Legacy Layer 5 memory detected; consolidating into canonical memory"
        );

        let mut section_entries = Vec::new();
        for chunk in chunks {
            let normalized = normalize_content(&chunk);
            if normalized.is_empty()
                || !seen.insert(normalized.clone())
                || normalize_content(&canonical_text).contains(&normalized)
            {
                continue;
            }
            let hash = blockcell_core::stable_hash_session_key(&normalized);
            let scope = if memory_type == MemoryType::User {
                "user"
            } else {
                "workspace"
            };
            let updated = chrono::Utc::now().format("%Y-%m-%d");
            let entry = format!(
                "- [id:layer5-{hash}] [scope:{scope}] [source:inferred] [updated:{updated}] {chunk} <!-- migrated-from:memory/{} -->",
                memory_type.filename()
            );
            if memory_type == MemoryType::User {
                store.add(crate::memory_file_store::MemoryFileTarget::User, &entry)?;
                canonical_text.push_str(&format!("\n{entry}\n"));
            } else {
                section_entries.push(entry);
            }
            result.migrated += 1;
        }
        if !section_entries.is_empty() {
            let block = format!(
                "## {}\n\n{}",
                category_heading(memory_type),
                section_entries.join("\n")
            );
            store.add(crate::memory_file_store::MemoryFileTarget::Memory, &block)?;
            canonical_text.push_str(&format!("\n{block}\n"));
        }
        crate::fs_util::atomic_write(&legacy_path, memory_type.template().as_bytes())?;
    }

    Ok(result)
}

fn read_optional_text(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn category_heading(memory_type: MemoryType) -> &'static str {
    match memory_type {
        MemoryType::User => "User",
        MemoryType::Project => "Project",
        MemoryType::Feedback => "Feedback",
        MemoryType::Reference => "Reference",
    }
}

fn normalize_content(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn legacy_content_chunks(memory_type: MemoryType, content: &str) -> Vec<String> {
    if content.trim().is_empty() || content.trim() == memory_type.template().trim() {
        return Vec::new();
    }
    let template_lines = memory_type
        .template()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<HashSet<_>>();
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !template_lines.contains(line))
        .filter(|line| !line.starts_with('#'))
        .filter(|line| *line != "---")
        .map(|line| {
            line.trim_start_matches('-')
                .trim_start_matches('*')
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockcell_core::Paths;

    #[test]
    fn test_memory_type_all() {
        let types = MemoryType::all();
        assert_eq!(types.len(), 4);
    }

    #[test]
    fn test_memory_type_names() {
        assert_eq!(MemoryType::User.name(), "user");
        assert_eq!(MemoryType::Project.name(), "project");
        assert_eq!(MemoryType::Feedback.name(), "feedback");
        assert_eq!(MemoryType::Reference.name(), "reference");
    }

    #[test]
    fn test_get_memory_file_path() {
        let config_dir = Path::new("/config");
        let path = get_memory_file_path(config_dir, MemoryType::User);
        // Check path components instead of string representation (platform-independent)
        assert!(path.ends_with("user.md"));
        assert!(path.to_str().unwrap().contains("memory"));
    }

    #[test]
    fn test_memory_type_templates() {
        for mt in MemoryType::all() {
            let template = mt.template();
            assert!(template.contains("---")); // YAML frontmatter
            assert!(template.contains("# ")); // Markdown header
        }
    }

    #[tokio::test]
    async fn layer5_legacy_files_consolidate_once_and_injector_reads_only_canonical_files() {
        let paths = Paths::with_base(std::env::temp_dir().join(format!(
            "blockcell-layer5-consolidation-{}",
            uuid::Uuid::new_v4()
        )));
        paths.ensure_dirs().expect("ensure paths");
        let legacy_dir = paths.base.join("memory");
        std::fs::create_dir_all(&legacy_dir).expect("create legacy memory dir");
        std::fs::write(paths.user_md(), "Existing canonical preference.\n").expect("seed USER.md");
        std::fs::write(
            paths.memory_md(),
            "## Feedback\n\nExisting canonical feedback.\n",
        )
        .expect("seed MEMORY.md");
        std::fs::write(
            legacy_dir.join("user.md"),
            "Existing canonical preference.\nLegacy user preference.\n",
        )
        .expect("write legacy user");
        std::fs::write(
            legacy_dir.join("project.md"),
            "Project uses canary deploys.\n",
        )
        .expect("write legacy project");
        std::fs::write(
            legacy_dir.join("feedback.md"),
            "Always report verification results.\n",
        )
        .expect("write legacy feedback");
        std::fs::write(
            legacy_dir.join("reference.md"),
            "Runbook is docs/runbook.md.\n",
        )
        .expect("write legacy reference");

        let first = consolidate_legacy_layer5(&paths).expect("first consolidation");
        assert_eq!(
            std::fs::read_to_string(legacy_dir.join("project.md"))
                .expect("read retired legacy project"),
            MemoryType::Project.template()
        );
        let second = consolidate_legacy_layer5(&paths).expect("second consolidation");

        assert_eq!(first.migrated, 4);
        assert_eq!(second.migrated, 0);
        let user = std::fs::read_to_string(paths.user_md()).expect("read USER.md");
        assert_eq!(user.matches("Existing canonical preference.").count(), 1);
        assert_eq!(user.matches("Legacy user preference.").count(), 1);
        let memory = std::fs::read_to_string(paths.memory_md()).expect("read MEMORY.md");
        assert!(memory.contains("## Project"));
        assert!(memory.contains("## Feedback"));
        assert!(memory.contains("## Reference"));
        assert_eq!(memory.matches("Project uses canary deploys.").count(), 1);
        assert_eq!(
            memory
                .matches("Always report verification results.")
                .count(),
            1
        );
        assert_eq!(memory.matches("Runbook is docs/runbook.md.").count(), 1);

        std::fs::write(
            legacy_dir.join("project.md"),
            "Legacy file changed after consolidation.\n",
        )
        .expect("change legacy file");
        let mut injector = crate::auto_memory::MemoryInjector::default_injector();
        injector
            .load_canonical(&paths)
            .await
            .expect("load canonical memory");
        let injected = injector.build_injection_content();
        assert!(injected.contains("Legacy user preference."));
        assert!(injected.contains("Project uses canary deploys."));
        assert!(!injected.contains("Legacy file changed after consolidation."));
        assert!(injector
            .get_memory(MemoryType::Project)
            .expect("project section")
            .contains("Project uses canary deploys."));
        assert!(injector
            .get_memory(MemoryType::Feedback)
            .expect("feedback section")
            .contains("Always report verification results."));
    }
}
