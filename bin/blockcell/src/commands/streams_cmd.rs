use blockcell_core::{Config, Paths};
use reqwest::Method;
use serde_json::Value;

use super::json_store::read_json;

fn read_rules(path: &std::path::Path) -> anyhow::Result<Vec<Value>> {
    read_json(path)
}

fn resolve_subscription_id(rules: &[Value], prefix: &str) -> anyhow::Result<String> {
    if prefix.trim().is_empty() {
        anyhow::bail!("Subscription ID prefix cannot be empty");
    }
    let matches = rules
        .iter()
        .filter_map(|rule| {
            rule.get("id")
                .or_else(|| rule.get("stream_id"))
                .and_then(Value::as_str)
        })
        .filter(|id| id.starts_with(prefix))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => anyhow::bail!("No matching subscription found: {}", prefix),
        [id] => Ok((*id).to_string()),
        _ => anyhow::bail!("Subscription ID prefix '{}' is ambiguous", prefix),
    }
}

fn gateway_base_url(config: &Config) -> String {
    if let Some(public) = config
        .gateway
        .public_api_base
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return public.trim_end_matches('/').to_string();
    }
    let host = match config.gateway.host.trim() {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1".to_string(),
        value if value.contains(':') && !value.starts_with('[') => format!("[{value}]"),
        value => value.to_string(),
    };
    format!("http://{}:{}", host, config.gateway.port)
}

async fn gateway_request(method: Method, path: &str) -> anyhow::Result<Value> {
    let paths = Paths::new_configured();
    let config = Config::load_or_default(&paths)?;
    let url = format!("{}{}", gateway_base_url(&config), path);
    let client = reqwest::Client::new();
    let mut request = client.request(method, &url);
    if let Some(token) = config
        .gateway
        .api_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|error| anyhow::anyhow!("Gateway request failed at {}: {}", url, error))?;
    let status = response.status();
    let body = response.text().await?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| anyhow::anyhow!("Gateway returned invalid JSON: {}", error))?;
    if !status.is_success() {
        anyhow::bail!("Gateway returned {}: {}", status, value);
    }
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        anyhow::bail!("Gateway stream operation failed: {}", error);
    }
    Ok(value)
}

/// List all stream subscriptions (from persisted rules).
pub async fn list() -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let subs_file = paths.workspace().join("streams").join("subscriptions.json");

    if !subs_file.exists() {
        println!("(No active stream subscriptions)");
        return Ok(());
    }

    let rules = read_rules(&subs_file)?;

    if rules.is_empty() {
        println!("(No active stream subscriptions)");
        return Ok(());
    }

    println!();
    println!("📡 Stream subscriptions ({} total)", rules.len());
    println!();
    println!(
        "  {:<10} {:<12} {:<40} Auto-restore",
        "ID", "Protocol", "URL"
    );
    println!("  {}", "-".repeat(80));

    for rule in &rules {
        let id = rule["id"].as_str().unwrap_or("?");
        let protocol = rule["protocol"].as_str().unwrap_or("?");
        let url = rule["url"].as_str().unwrap_or("?");
        let auto_restore = rule["auto_restore"].as_bool().unwrap_or(false);

        let short_id_owned: String = id.chars().take(8).collect();
        let short_id = short_id_owned.as_str();
        let short_url: String = url.chars().take(38).collect();
        let url_ellipsis = if url.chars().count() > 38 { ".." } else { "" };

        println!(
            "  {:<10} {:<12} {:<40} {}",
            short_id,
            protocol,
            format!("{}{}", short_url, url_ellipsis),
            if auto_restore { "✓" } else { "✗" }
        );
    }
    println!();

    Ok(())
}

/// Show details for a specific subscription.
pub async fn status(sub_id: &str) -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let subs_file = paths.workspace().join("streams").join("subscriptions.json");

    if !subs_file.exists() {
        println!("(No stream subscriptions)");
        return Ok(());
    }

    let rules = read_rules(&subs_file)?;
    let resolved = resolve_subscription_id(&rules, sub_id)?;
    let rule = rules
        .iter()
        .find(|rule| rule.get("id").and_then(Value::as_str) == Some(resolved.as_str()))
        .ok_or_else(|| anyhow::anyhow!("Resolved subscription disappeared"))?;
    println!();
    println!("📡 Subscription details");
    println!("{}", serde_json::to_string_pretty(rule)?);
    println!();

    Ok(())
}

/// Remove a subscription from the persisted rules.
pub async fn stop(sub_id: &str) -> anyhow::Result<()> {
    let active = gateway_request(Method::GET, "/v1/streams").await?;
    let streams = active
        .get("streams")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Gateway stream list is missing 'streams'"))?;
    let resolved = resolve_subscription_id(streams, sub_id)?;
    let result = gateway_request(
        Method::DELETE,
        &format!("/v1/streams/{}", urlencoding::encode(&resolved)),
    )
    .await?;
    println!("✓ Stream {} stopped: {}", resolved, result);
    Ok(())
}

/// Restore all persisted subscriptions.
pub async fn restore() -> anyhow::Result<()> {
    let paths = Paths::new_configured();
    let subs_file = paths.workspace().join("streams").join("subscriptions.json");

    if !subs_file.exists() {
        println!("(No persisted subscription rules)");
        return Ok(());
    }

    let rules = read_rules(&subs_file)?;

    let restorable: Vec<&Value> = rules
        .iter()
        .filter(|r| r["auto_restore"].as_bool().unwrap_or(false))
        .collect();

    if restorable.is_empty() {
        println!("No subscriptions marked for auto-restore.");
        return Ok(());
    }

    let result = gateway_request(Method::POST, "/v1/streams/restore").await?;
    println!("✓ Stream restore completed: {}", result);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_prefix_must_resolve_uniquely() {
        let rules = vec![
            serde_json::json!({"id": "stream_alpha"}),
            serde_json::json!({"id": "stream_alpine"}),
        ];

        assert!(resolve_subscription_id(&rules, "").is_err());
        assert!(resolve_subscription_id(&rules, "stream_al").is_err());
        assert_eq!(
            resolve_subscription_id(&rules, "stream_alpha").unwrap(),
            "stream_alpha"
        );

        let active = vec![serde_json::json!({"stream_id": "stream_runtime"})];
        assert_eq!(
            resolve_subscription_id(&active, "stream_run").unwrap(),
            "stream_runtime"
        );
    }

    #[test]
    fn gateway_url_uses_loopback_for_wildcard_bind_host() {
        let mut config = blockcell_core::Config::default();
        config.gateway.host = "0.0.0.0".to_string();
        config.gateway.port = 19090;

        assert_eq!(gateway_base_url(&config), "http://127.0.0.1:19090");
    }
}
