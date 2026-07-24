use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use blockcell_core::mcp_config::{McpFileServerConfig, McpResolvedConfig, McpRootConfig};
use blockcell_core::Paths;

fn parse_env_pairs(env_pairs: &[String]) -> anyhow::Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    for pair in env_pairs {
        let Some((key, value)) = pair.split_once('=') else {
            bail!("Invalid --env '{}', expected KEY=VALUE", pair);
        };
        let key = key.trim();
        if key.is_empty() {
            bail!("Environment variable name cannot be empty");
        }
        env.insert(key.to_string(), value.to_string());
    }
    Ok(env)
}

fn github_template(name: String, disabled: bool, no_auto_start: bool) -> McpFileServerConfig {
    let mut env = BTreeMap::new();
    env.insert(
        "GITHUB_PERSONAL_ACCESS_TOKEN".to_string(),
        "${env:GITHUB_PERSONAL_ACCESS_TOKEN}".to_string(),
    );
    McpFileServerConfig {
        name,
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-github".to_string(),
        ],
        env: env.into_iter().collect(),
        cwd: None,
        enabled: !disabled,
        auto_start: Some(!no_auto_start),
        startup_timeout_secs: None,
        call_timeout_secs: None,
    }
}

fn sqlite_template(
    name: String,
    db_path: Option<String>,
    disabled: bool,
    no_auto_start: bool,
) -> McpFileServerConfig {
    let db_path = db_path.unwrap_or_else(|| "/tmp/blockcell.db".to_string());
    McpFileServerConfig {
        name,
        command: "uvx".to_string(),
        args: vec![
            "mcp-server-sqlite".to_string(),
            "--db-path".to_string(),
            db_path,
        ],
        env: Default::default(),
        cwd: None,
        enabled: !disabled,
        auto_start: Some(!no_auto_start),
        startup_timeout_secs: None,
        call_timeout_secs: None,
    }
}

fn filesystem_template(
    name: String,
    paths: Vec<String>,
    disabled: bool,
    no_auto_start: bool,
) -> McpFileServerConfig {
    let roots = if paths.is_empty() {
        vec!["~/Documents".to_string()]
    } else {
        paths
    };
    let mut args = vec![
        "-y".to_string(),
        "@modelcontextprotocol/server-filesystem".to_string(),
    ];
    args.extend(roots);
    McpFileServerConfig {
        name,
        command: "npx".to_string(),
        args,
        env: Default::default(),
        cwd: None,
        enabled: !disabled,
        auto_start: Some(!no_auto_start),
        startup_timeout_secs: None,
        call_timeout_secs: None,
    }
}

fn postgres_template(
    name: String,
    dsn: Option<String>,
    disabled: bool,
    no_auto_start: bool,
) -> McpFileServerConfig {
    let dsn = dsn.unwrap_or_else(|| "postgresql://user:password@localhost:5432/app".to_string());
    McpFileServerConfig {
        name,
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-postgres".to_string(),
            dsn,
        ],
        env: Default::default(),
        cwd: None,
        enabled: !disabled,
        auto_start: Some(!no_auto_start),
        startup_timeout_secs: None,
        call_timeout_secs: None,
    }
}

fn puppeteer_template(name: String, disabled: bool, no_auto_start: bool) -> McpFileServerConfig {
    McpFileServerConfig {
        name,
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-puppeteer".to_string(),
        ],
        env: Default::default(),
        cwd: None,
        enabled: !disabled,
        auto_start: Some(!no_auto_start),
        startup_timeout_secs: None,
        call_timeout_secs: None,
    }
}

fn build_template(
    template: &str,
    name_override: Option<String>,
    db_path: Option<String>,
    filesystem_paths: Vec<String>,
    dsn: Option<String>,
    disabled: bool,
    no_auto_start: bool,
) -> anyhow::Result<McpFileServerConfig> {
    let template = template.trim();
    let name = name_override.unwrap_or_else(|| template.to_string());
    match template {
        "github" => Ok(github_template(name, disabled, no_auto_start)),
        "sqlite" => Ok(sqlite_template(name, db_path, disabled, no_auto_start)),
        "filesystem" => Ok(filesystem_template(name, filesystem_paths, disabled, no_auto_start)),
        "postgres" => Ok(postgres_template(name, dsn, disabled, no_auto_start)),
        "puppeteer" => Ok(puppeteer_template(name, disabled, no_auto_start)),
        _ => bail!(
            "Unknown MCP template '{}'. Supported templates: github, sqlite, filesystem, postgres, puppeteer",
            template
        ),
    }
}

fn validate_server_name(name: &str) -> anyhow::Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed != name || matches!(trimmed, "." | "..") {
        bail!("Invalid MCP server name '{}'", name);
    }
    if trimmed
        .chars()
        .any(|ch| ch == '/' || ch == '\\' || ch.is_control())
    {
        bail!(
            "Invalid MCP server name '{}': use a single file-safe name",
            name
        );
    }
    Ok(trimmed)
}

fn server_file_path(paths: &Paths, name: &str) -> anyhow::Result<PathBuf> {
    let name = validate_server_name(name)?;
    Ok(paths.mcp_dir().join(format!("{}.json", name)))
}

fn load_root_or_default(paths: &Paths) -> anyhow::Result<McpRootConfig> {
    if paths.mcp_config_file().exists() {
        Ok(McpRootConfig::load(&paths.mcp_config_file())?)
    } else {
        Ok(McpRootConfig::default())
    }
}

fn save_server_file(path: &Path, cfg: &McpFileServerConfig) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(cfg)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn source_for_server(paths: &Paths, name: &str) -> Option<String> {
    if let Ok(file_path) = server_file_path(paths, name) {
        if file_path.exists() {
            return Some(file_path.display().to_string());
        }
    }
    if load_root_or_default(paths)
        .ok()
        .and_then(|root| root.servers.get(name).cloned())
        .is_some()
    {
        Some(paths.mcp_config_file().display().to_string())
    } else {
        None
    }
}

pub async fn list() -> anyhow::Result<()> {
    let paths = Paths::new();
    paths.ensure_dirs()?;
    let resolved = McpResolvedConfig::load_merged(&paths)?;

    println!();
    println!("🔌 MCP servers ({} total)", resolved.servers.len());
    println!();

    let mut names: Vec<_> = resolved.servers.keys().cloned().collect();
    names.sort();
    for name in names {
        let server = resolved.servers.get(&name).expect("server config");
        let source = source_for_server(&paths, &name).unwrap_or_else(|| "<unknown>".to_string());
        println!(
            "  {:<14} {:<8} autoStart={:<5} {:<16} {}",
            name,
            if server.enabled {
                "enabled"
            } else {
                "disabled"
            },
            server.auto_start,
            server.command,
            source
        );
    }
    println!();
    println!("Changes apply after restarting `blockcell agent` or `blockcell gateway`.");
    Ok(())
}

pub async fn show(name: &str) -> anyhow::Result<()> {
    let paths = Paths::new();
    let resolved = McpResolvedConfig::load_merged(&paths)?;
    let server = resolved
        .servers
        .get(name)
        .ok_or_else(|| anyhow!("MCP server '{}' not found", name))?;

    println!();
    println!("🔌 {}", name);
    if let Some(source) = source_for_server(&paths, name) {
        println!("  source: {}", source);
    }
    println!();
    println!("{}", serde_json::to_string_pretty(server)?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn add(
    template_or_name: &str,
    raw: bool,
    name: Option<String>,
    command: Option<String>,
    args: Vec<String>,
    env: Vec<String>,
    cwd: Option<String>,
    db_path: Option<String>,
    filesystem_paths: Vec<String>,
    dsn: Option<String>,
    force: bool,
    disabled: bool,
    no_auto_start: bool,
    startup_timeout_secs: Option<u64>,
    call_timeout_secs: Option<u64>,
) -> anyhow::Result<()> {
    let paths = Paths::new();
    paths.ensure_dirs()?;

    let cfg = if raw {
        let name = name.ok_or_else(|| anyhow!("--name is required with --raw"))?;
        let command = command.ok_or_else(|| anyhow!("--command is required with --raw"))?;
        McpFileServerConfig {
            name,
            command,
            args,
            env: parse_env_pairs(&env)?.into_iter().collect(),
            cwd,
            enabled: !disabled,
            auto_start: Some(!no_auto_start),
            startup_timeout_secs,
            call_timeout_secs,
        }
    } else {
        let mut cfg = build_template(
            template_or_name,
            name,
            db_path,
            filesystem_paths,
            dsn,
            disabled,
            no_auto_start,
        )?;
        if !env.is_empty() {
            cfg.env = parse_env_pairs(&env)?.into_iter().collect();
        }
        if cwd.is_some() {
            cfg.cwd = cwd;
        }
        if !args.is_empty() {
            cfg.args = args;
        }
        if startup_timeout_secs.is_some() {
            cfg.startup_timeout_secs = startup_timeout_secs;
        }
        if call_timeout_secs.is_some() {
            cfg.call_timeout_secs = call_timeout_secs;
        }
        cfg
    };

    let path = server_file_path(&paths, &cfg.name)?;
    if path.exists() && !force {
        bail!(
            "MCP server '{}' already exists at {}. Use --force to overwrite.",
            cfg.name,
            path.display()
        );
    }

    save_server_file(&path, &cfg)?;
    println!("✓ MCP server '{}' saved to {}", cfg.name, path.display());
    println!("Restart `blockcell agent` or `blockcell gateway` to apply changes.");
    Ok(())
}

pub async fn remove(name: &str) -> anyhow::Result<()> {
    let paths = Paths::new();
    let file_path = server_file_path(&paths, name)?;
    if file_path.exists() {
        std::fs::remove_file(&file_path)?;
        println!("✓ Removed MCP server '{}' ({})", name, file_path.display());
        println!("Restart `blockcell agent` or `blockcell gateway` to apply changes.");
        return Ok(());
    }

    let mut root = load_root_or_default(&paths)?;
    if root.servers.remove(name).is_some() {
        root.save(&paths.mcp_config_file())?;
        println!(
            "✓ Removed MCP server '{}' from {}",
            name,
            paths.mcp_config_file().display()
        );
        println!("Restart `blockcell agent` or `blockcell gateway` to apply changes.");
        return Ok(());
    }

    bail!("MCP server '{}' not found", name)
}

fn update_root_enabled(root: &mut McpRootConfig, name: &str, enabled: bool) -> bool {
    let Some(server) = root.servers.get_mut(name) else {
        return false;
    };
    server.enabled = enabled;
    true
}

fn update_file_enabled(path: &Path, enabled: bool) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let content = std::fs::read_to_string(path)?;
    let mut file_cfg: McpFileServerConfig = serde_json::from_str(&content)?;
    file_cfg.enabled = enabled;
    save_server_file(path, &file_cfg)?;
    Ok(true)
}

pub async fn set_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    let paths = Paths::new();
    let file_path = server_file_path(&paths, name)?;
    if update_file_enabled(&file_path, enabled)? {
        println!(
            "✓ MCP server '{}' {}",
            name,
            if enabled { "enabled" } else { "disabled" }
        );
        println!("Restart `blockcell agent` or `blockcell gateway` to apply changes.");
        return Ok(());
    }

    let mut root = load_root_or_default(&paths)?;
    if update_root_enabled(&mut root, name, enabled) {
        root.save(&paths.mcp_config_file())?;
        println!(
            "✓ MCP server '{}' {}",
            name,
            if enabled { "enabled" } else { "disabled" }
        );
        println!("Restart `blockcell agent` or `blockcell gateway` to apply changes.");
        return Ok(());
    }

    bail!("MCP server '{}' not found", name)
}

pub async fn edit(name: Option<&str>) -> anyhow::Result<()> {
    let paths = Paths::new();
    paths.ensure_dirs()?;
    let target = match name {
        Some(name) => server_file_path(&paths, name)?,
        None => paths.mcp_config_file(),
    };

    if !target.exists() {
        if name.is_some() {
            bail!(
                "MCP server file not found: {}. Use `blockcell mcp add` first.",
                target.display()
            );
        }
        McpRootConfig::default().save(&target)?;
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "open -t".to_string()
            } else {
                "vi".to_string()
            }
        });

    let mut parts = editor.split_whitespace();
    let command = parts.next().context("EDITOR is empty")?;
    let args: Vec<String> = parts.map(|s| s.to_string()).collect();

    let status = std::process::Command::new(command)
        .args(args)
        .arg(&target)
        .status()
        .with_context(|| format!("Failed to launch editor for {}", target.display()))?;

    if !status.success() {
        bail!("Editor exited with status {}", status);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_pairs() {
        let env = parse_env_pairs(&["A=1".to_string(), "B=two".to_string()]).unwrap();
        assert_eq!(env.get("A"), Some(&"1".to_string()));
        assert_eq!(env.get("B"), Some(&"two".to_string()));
    }

    #[test]
    fn test_build_github_template() {
        let cfg = build_template("github", None, None, vec![], None, false, false).unwrap();
        assert_eq!(cfg.name, "github");
        assert_eq!(cfg.command, "npx");
        assert!(cfg.args.iter().any(|arg| arg.contains("server-github")));
    }

    #[test]
    fn test_build_sqlite_template_uses_db_path() {
        let cfg = build_template(
            "sqlite",
            Some("db".to_string()),
            Some("/tmp/test.db".to_string()),
            vec![],
            None,
            false,
            true,
        )
        .unwrap();
        assert_eq!(cfg.name, "db");
        assert_eq!(cfg.command, "uvx");
        assert!(cfg.args.iter().any(|arg| arg == "/tmp/test.db"));
        assert_eq!(cfg.auto_start, Some(false));
    }

    #[test]
    fn mcp_server_names_must_be_single_safe_components() {
        for invalid in ["", ".", "..", "../escape", "nested/name", r"nested\name"] {
            assert!(validate_server_name(invalid).is_err(), "{invalid}");
        }
        assert!(validate_server_name("github-main_1").is_ok());
    }
}
