use blockcell_core::Paths;
use blockcell_tools::{alert_rule::AlertRuleTool, Tool};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json_store::{read_json, update_json};
use super::tools_cmd::build_cli_tool_context;

#[derive(Debug, Serialize, Deserialize)]
struct AlertStore {
    #[serde(default = "default_store_version")]
    version: u32,
    #[serde(default)]
    rules: Vec<Value>,
}

impl Default for AlertStore {
    fn default() -> Self {
        Self {
            version: default_store_version(),
            rules: Vec::new(),
        }
    }
}

fn default_store_version() -> u32 {
    1
}

fn read_alert_store(path: &std::path::Path) -> anyhow::Result<AlertStore> {
    parse_alert_store_value(read_json(path)?)
}

fn parse_alert_store_value(value: Value) -> anyhow::Result<AlertStore> {
    if !value.is_object() {
        anyhow::bail!("Alert store root must be a JSON object");
    }
    Ok(serde_json::from_value(value)?)
}

fn update_alert_store<R, F>(path: &std::path::Path, mutate: F) -> anyhow::Result<R>
where
    F: FnOnce(&mut AlertStore) -> anyhow::Result<R>,
{
    update_json(
        path,
        || serde_json::to_value(AlertStore::default()).expect("serialize default alert store"),
        |raw: &mut Value| {
            let mut store = parse_alert_store_value(std::mem::take(raw))?;
            let result = mutate(&mut store)?;
            *raw = serde_json::to_value(store)?;
            Ok(result)
        },
    )
}

fn enabled_rule_ids(store: &AlertStore) -> anyhow::Result<Vec<&str>> {
    store
        .rules
        .iter()
        .filter(|rule| rule.get("enabled").and_then(Value::as_bool).unwrap_or(true))
        .map(|rule| {
            rule.get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| anyhow::anyhow!("Alert rule is missing a non-empty id"))
        })
        .collect()
}

/// List all alert rules.
pub async fn list() -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let rules_file = paths.workspace().join("alerts").join("rules.json");

    if !rules_file.exists() {
        println!("(No alert rules. Use agent chat or `blockcell alerts add` to create one.)");
        return Ok(());
    }

    let store = read_alert_store(&rules_file)?;
    let rules = &store.rules;

    if rules.is_empty() {
        println!("(No alert rules)");
        return Ok(());
    }

    println!();
    println!("🔔 Alert rules ({} total)", rules.len());
    println!();
    println!(
        "  {:<10} {:<20} {:<10} {:<12} Condition",
        "ID", "Name", "Enabled", "Operator"
    );
    println!("  {}", "-".repeat(70));

    for rule in rules {
        let id = rule["id"].as_str().unwrap_or("?");
        let name = rule["name"].as_str().unwrap_or("?");
        let enabled = rule["enabled"].as_bool().unwrap_or(true);
        let operator = rule["operator"].as_str().unwrap_or("?");
        let threshold = &rule["threshold"];

        let short_id = if id.len() > 8 { &id[..8] } else { id };
        let short_name: String = name.chars().take(18).collect();

        println!(
            "  {:<10} {:<20} {:<10} {:<12} {}",
            short_id,
            short_name,
            if enabled { "✓" } else { "✗" },
            operator,
            threshold
        );
    }
    println!();

    Ok(())
}

/// Show alert trigger history.
pub async fn history(limit: usize) -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let history_file = paths.workspace().join("alerts").join("history.json");

    if !history_file.exists() {
        println!("(No alert trigger history)");
        return Ok(());
    }

    let entries: Vec<Value> = read_json(&history_file)?;

    if entries.is_empty() {
        println!("(No alert trigger history)");
        return Ok(());
    }

    let show_count = entries.len().min(limit);
    let recent = &entries[entries.len().saturating_sub(limit)..];

    println!();
    println!(
        "📜 Alert trigger history (showing {}, {} total)",
        show_count,
        entries.len()
    );
    println!();

    for entry in recent.iter().rev() {
        let rule_name = entry["rule_name"].as_str().unwrap_or("?");
        let triggered_at: String = if let Some(s) = entry["triggered_at"].as_str() {
            s.to_string()
        } else if let Some(ms) = entry["triggered_at_ms"].as_i64() {
            use chrono::{TimeZone, Utc};
            Utc.timestamp_millis_opt(ms)
                .single()
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "?".to_string())
        } else {
            "?".to_string()
        };
        let value = &entry["value"];

        println!("  🔔 {} — value: {} — {}", rule_name, value, triggered_at);
    }
    println!();

    Ok(())
}

/// Manually evaluate all alert rules once.
pub async fn evaluate() -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let rules_file = paths.workspace().join("alerts").join("rules.json");

    if !rules_file.exists() {
        println!("(No alert rules)");
        return Ok(());
    }

    let store = read_alert_store(&rules_file)?;
    let rule_ids = enabled_rule_ids(&store)?;
    if rule_ids.is_empty() {
        println!("(No enabled alert rules)");
        return Ok(());
    }

    let ctx = build_cli_tool_context(&paths)?;
    let tool = AlertRuleTool;
    let mut failures = Vec::new();
    println!("⏳ Evaluating {} enabled alert rules...", rule_ids.len());
    for rule_id in rule_ids {
        let params = serde_json::json!({"action": "evaluate", "rule_id": rule_id});
        tool.validate(&params)?;
        match tool.execute(ctx.clone(), params).await {
            Ok(result) if result.get("error").is_none() => {
                println!(
                    "  ✓ {} value={} triggered={}",
                    rule_id,
                    result.get("current_value").unwrap_or(&Value::Null),
                    result.get("triggered").unwrap_or(&Value::Bool(false))
                );
            }
            Ok(result) => {
                let error = result
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("evaluation failed");
                println!("  ✗ {} {}", rule_id, error);
                failures.push(format!("{}: {}", rule_id, error));
            }
            Err(error) => {
                println!("  ✗ {} {}", rule_id, error);
                failures.push(format!("{}: {}", rule_id, error));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{} alert evaluation(s) failed", failures.len())
    }
}

/// Add a new alert rule.
pub async fn add(
    name: &str,
    source: &str,
    field: &str,
    operator: &str,
    threshold: &str,
) -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let alerts_dir = paths.workspace().join("alerts");
    std::fs::create_dir_all(&alerts_dir)?;
    let rules_file = alerts_dir.join("rules.json");

    let source: Value = serde_json::from_str(source)
        .map_err(|error| anyhow::anyhow!("--source must be a JSON tool call object: {error}"))?;
    if source.get("tool").and_then(Value::as_str).is_none() {
        anyhow::bail!("--source must contain a string 'tool' field");
    }
    let threshold = threshold
        .parse::<f64>()
        .map_err(|error| anyhow::anyhow!("--threshold must be numeric: {error}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let rule = serde_json::json!({
        "id": id,
        "name": name,
        "enabled": true,
        "source": source,
        "metric_path": field,
        "operator": operator,
        "threshold": threshold,
        "threshold2": null,
        "cooldown_secs": 3600,
        "check_interval_secs": 300,
        "notify": {"channel": "desktop", "template": null, "params": null},
        "on_trigger": [],
        "state": {
            "last_value": null,
            "prev_value": null,
            "last_check_at": null,
            "last_triggered_at": null,
            "trigger_count": 0,
            "last_error": null
        },
        "created_at": now,
        "updated_at": now,
    });
    update_alert_store(&rules_file, |store| {
        store.rules.push(rule);
        Ok(())
    })?;

    println!(
        "✓ Alert rule created: {} ({})",
        name,
        &id.chars().take(8).collect::<String>()
    );
    Ok(())
}

/// Remove an alert rule by ID prefix.
pub async fn remove(rule_id: &str) -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let rules_file = paths.workspace().join("alerts").join("rules.json");

    if !rules_file.exists() {
        println!("(No alert rules)");
        return Ok(());
    }

    let removed_id = update_alert_store(&rules_file, |store| {
        let matches = store
            .rules
            .iter()
            .filter_map(|rule| rule.get("id").and_then(Value::as_str))
            .filter(|id| id.starts_with(rule_id))
            .map(str::to_string)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => anyhow::bail!("No matching rule found: {}", rule_id),
            [id] => {
                let id = id.clone();
                store
                    .rules
                    .retain(|rule| rule.get("id").and_then(Value::as_str) != Some(id.as_str()));
                Ok(id)
            }
            _ => anyhow::bail!("Rule ID prefix '{}' is ambiguous", rule_id),
        }
    })?;

    println!("✓ Removed alert rule {}", removed_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_rule_ids_excludes_disabled_rules() {
        let store = AlertStore {
            version: 1,
            rules: vec![
                serde_json::json!({"id": "enabled", "enabled": true}),
                serde_json::json!({"id": "disabled", "enabled": false}),
            ],
        };

        assert_eq!(enabled_rule_ids(&store).unwrap(), vec!["enabled"]);
    }

    #[test]
    fn alert_store_rejects_legacy_array_shape() {
        assert!(parse_alert_store_value(serde_json::json!([])).is_err());
    }

    #[test]
    fn alert_store_update_does_not_replace_legacy_array() {
        let path = std::env::temp_dir().join(format!(
            "blockcell-alert-store-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, "[]").unwrap();

        assert!(update_alert_store(&path, |_store| Ok(())).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
        let _ = std::fs::remove_file(&path);
    }
}
