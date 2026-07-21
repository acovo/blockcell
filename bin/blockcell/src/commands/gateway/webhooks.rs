use super::*;
// ---------------------------------------------------------------------------
// Lark webhook handler (public, no auth)
// ---------------------------------------------------------------------------

/// POST /webhook/lark — receives events from Lark (international) via HTTP callback.
/// This endpoint must be publicly accessible. Configure the URL in the Lark Developer Console
/// under "Event Subscriptions" → "Request URL": https://your-domain/webhook/lark
#[cfg(feature = "lark")]
pub(super) async fn handle_lark_webhook(
    State(state): State<GatewayState>,
    body: String,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    if !state.config.channels.lark.enabled {
        return (StatusCode::OK, axum::Json(serde_json::json!({"code": 0}))).into_response();
    }

    // 根据 webhook 匹配的 account 解析 agent owner，使用其 workspace 的 media_dir。
    // 避免：(1) 进程级 BLOCKCELL_WORKSPACE 竞争；(2) 多账号下媒体写入错误的 agent workspace。
    // 即使 account_id 为 None，也要走 resolve_effective_channel_owner 以支持 channelOwners 级路由。
    let account_id =
        blockcell_channels::lark::resolve_lark_webhook_account_id(&state.config, &body);
    let owner = state
        .config
        .resolve_effective_channel_owner("lark", account_id.as_deref())
        .unwrap_or("default")
        .to_string();
    let media_dir = state.paths.for_agent(&owner).media_dir();

    match blockcell_channels::lark::process_webhook(
        &state.config,
        &body,
        Some(&state.inbound_tx),
        media_dir,
    )
    .await
    {
        Ok(resp_json) => {
            let val: serde_json::Value =
                serde_json::from_str(&resp_json).unwrap_or(serde_json::json!({"code": 0}));
            (StatusCode::OK, axum::Json(val)).into_response()
        }
        Err(e) => {
            let status = if matches!(e, blockcell_core::Error::PermissionDenied(_)) {
                tracing::warn!(error = %e, "Lark webhook authentication rejected");
                StatusCode::UNAUTHORIZED
            } else {
                tracing::error!(error = %e, "Lark webhook processing error");
                StatusCode::BAD_REQUEST
            };
            (status, axum::Json(serde_json::json!({"code": 1}))).into_response()
        }
    }
}

#[cfg(not(feature = "lark"))]
pub(super) async fn handle_lark_webhook(
    State(_state): State<GatewayState>,
    _body: String,
) -> impl IntoResponse {
    axum::Json(serde_json::json!({"code": 0}))
}

// ---------------------------------------------------------------------------
// WeCom webhook handler (public, no auth)
// ---------------------------------------------------------------------------

/// GET/POST /webhook/wecom — receives events from WeCom (企业微信) via HTTP callback.
/// This endpoint must be publicly accessible. Configure the URL in the WeCom admin console
/// under "企业应用" → "接收消息" → "URL": https://your-domain/webhook/wecom
///
/// GET: URL verification (returns echostr if signature valid)
/// POST: Message/event callback
#[cfg(feature = "wecom")]
pub(super) async fn handle_wecom_webhook(
    State(state): State<GatewayState>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    if !state.config.channels.wecom.enabled {
        return (StatusCode::OK, "success".to_string()).into_response();
    }

    let http_method = req.method().as_str().to_uppercase();
    let body = if http_method == "POST" {
        match axum::body::to_bytes(req.into_body(), 1024 * 1024).await {
            Ok(b) => String::from_utf8_lossy(&b).to_string(),
            Err(_) => String::new(),
        }
    } else {
        String::new()
    };

    // 根据 webhook 匹配的 account 解析 agent owner，使用其 workspace 的 media_dir。
    // 即使 account_id 为 None，也要走 resolve_effective_channel_owner 以支持 channelOwners 级路由。
    let account_id = blockcell_channels::wecom::resolve_wecom_webhook_account_id(
        &state.config,
        &http_method,
        &query,
        &body,
    );
    let owner = state
        .config
        .resolve_effective_channel_owner("wecom", account_id.as_deref())
        .unwrap_or("default")
        .to_string();
    let media_dir = state.paths.for_agent(&owner).media_dir();

    let (status, body_str) = blockcell_channels::wecom::process_webhook(
        &state.config,
        &http_method,
        &query,
        &body,
        Some(&state.inbound_tx),
        media_dir,
    )
    .await;

    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::OK),
        body_str,
    )
        .into_response()
}

#[cfg(not(feature = "wecom"))]
pub(super) async fn handle_wecom_webhook(
    State(_state): State<GatewayState>,
    axum::extract::Query(_query): axum::extract::Query<std::collections::HashMap<String, String>>,
    _req: axum::extract::Request,
) -> impl IntoResponse {
    (axum::http::StatusCode::OK, "success")
}

// ---------------------------------------------------------------------------
// QQ webhook handler (public, no auth)
// ---------------------------------------------------------------------------

/// POST /webhook/qq — receives events from QQ Official Bot via HTTP callback.
/// This endpoint must be publicly accessible. Configure the URL in the QQ Bot Developer Console
/// under "Development" → "Event Settings" → "Callback URL": https://your-domain/webhook/qq
#[cfg(feature = "qq")]
pub(super) async fn handle_qq_webhook(
    State(state): State<GatewayState>,
    body: String,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    if !state.config.channels.qq.enabled {
        return (
            StatusCode::OK,
            axum::Json(serde_json::json!({"retcode": 0})),
        )
            .into_response();
    }

    let payload: serde_json::Value = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "QQ webhook invalid JSON");
            return (
                StatusCode::OK,
                axum::Json(serde_json::json!({"retcode": 0})),
            )
                .into_response();
        }
    };

    let qq_channel =
        blockcell_channels::qq::QQChannel::new(state.config.clone(), state.inbound_tx.clone());

    match qq_channel.handle_webhook_payload(&payload).await {
        Ok(resp_json) => (StatusCode::OK, axum::Json(resp_json)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "QQ webhook processing error");
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({"retcode": 1})),
            )
                .into_response()
        }
    }
}

#[cfg(not(feature = "qq"))]
pub(super) async fn handle_qq_webhook(
    State(_state): State<GatewayState>,
    _body: String,
) -> impl IntoResponse {
    axum::Json(serde_json::json!({"retcode": 0}))
}
