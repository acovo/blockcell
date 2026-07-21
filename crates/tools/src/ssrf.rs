use blockcell_core::{Error, Result};
use std::net::{IpAddr, ToSocketAddrs};

/// Set this env var to `1`/`true` to allow requests to private/internal
/// addresses (disables the SSRF guard). Off by default.
pub(crate) const SSRF_ALLOW_ENV: &str = "BLOCKCELL_HTTP_ALLOW_PRIVATE";

pub(crate) fn private_network_allowed() -> bool {
    std::env::var(SSRF_ALLOW_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub(crate) fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6
                    .to_ipv4()
                    .map(|v4| is_blocked_ip(&IpAddr::V4(v4)))
                    .unwrap_or(false)
        }
    }
}

pub(crate) fn host_is_blocked(host: &str) -> bool {
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_blocked_ip(&ip);
    }
    match (host, 0u16).to_socket_addrs() {
        Ok(addrs) => addrs.into_iter().any(|a| is_blocked_ip(&a.ip())),
        Err(_) => false,
    }
}

fn ssrf_denied(host: &str) -> Error {
    Error::PermissionDenied(format!(
        "Refusing to request private/internal address ({host}). Set {SSRF_ALLOW_ENV}=1 to override."
    ))
}

pub(crate) async fn ensure_url_allowed(url: &str) -> Result<()> {
    if private_network_allowed() {
        return Ok(());
    }

    let parsed =
        reqwest::Url::parse(url).map_err(|e| Error::Validation(format!("Invalid URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Error::Validation("URL must use http or https".to_string()));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| Error::Validation("URL has no host".to_string()))?;
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);

    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_blocked_ip(&ip) {
            Err(ssrf_denied(host))
        } else {
            Ok(())
        };
    }

    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| Error::Tool(format!("DNS resolution failed for {host}: {e}")))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(&addr.ip()) {
            return Err(ssrf_denied(host));
        }
    }
    if !any {
        return Err(Error::Tool(format!(
            "DNS resolution returned no addresses for {host}"
        )));
    }
    Ok(())
}

pub(crate) fn redirect_policy(follow: bool) -> reqwest::redirect::Policy {
    if !follow {
        return reqwest::redirect::Policy::none();
    }

    let allow_private = private_network_allowed();
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("too many redirects");
        }
        if !allow_private {
            if let Some(host) = attempt.url().host_str() {
                if host_is_blocked(host) {
                    return attempt.error("redirect to private/internal address blocked");
                }
            }
        }
        attempt.follow()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssrf_ip_classification_blocks_internal_ranges() {
        for ip in [
            "127.0.0.1",
            "169.254.169.254",
            "10.0.0.1",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
        ] {
            assert!(is_blocked_ip(&ip.parse().unwrap()), "{ip} must be blocked");
        }
        assert!(!is_blocked_ip(&"8.8.8.8".parse().unwrap()));
    }

    #[tokio::test]
    async fn ssrf_initial_url_rejects_non_http_schemes() {
        let error = ensure_url_allowed("file:///etc/passwd")
            .await
            .expect_err("file URLs must be rejected");
        assert!(error.to_string().contains("http or https"));
    }
}
