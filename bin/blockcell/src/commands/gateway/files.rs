use super::*;
// ---------------------------------------------------------------------------
// P2: File management endpoints
// ---------------------------------------------------------------------------

pub(super) const MAX_FILE_CONTENT_BYTES: u64 = 10 * 1024 * 1024;
const MAX_FILE_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILE_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
pub(super) const MAX_FILE_UPLOAD_BODY_BYTES: usize = 16 * 1024 * 1024;

fn file_size_within_limit(size: u64, limit: u64) -> bool {
    size <= limit
}

fn utf8_upload_within_limit(content: &str, limit: usize) -> bool {
    content.len() <= limit
}

fn base64_upload_within_decoded_limit(content: &str, limit: usize) -> bool {
    let normalized_len = content.chars().filter(|ch| !ch.is_whitespace()).count();
    let padding = content
        .chars()
        .rev()
        .take_while(|ch| *ch == '=')
        .count()
        .min(2);
    let decoded_upper_bound = normalized_len.div_ceil(4).saturating_mul(3);
    decoded_upper_bound.saturating_sub(padding) <= limit
}

fn resolve_workspace_upload_target(
    workspace: &std::path::Path,
    relative: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let resolved_workspace = blockcell_core::path_policy::resolve_for_policy(workspace);
    let resolved_target =
        blockcell_core::path_policy::resolve_for_policy(&workspace.join(relative));
    if !resolved_target.starts_with(&resolved_workspace) {
        return Err("Access denied: upload path escapes workspace".to_string());
    }
    Ok(resolved_target)
}

async fn reject_if_file_too_large(path: &std::path::Path, limit: u64) -> Option<Response> {
    match tokio::fs::metadata(path).await {
        Ok(meta) if !file_size_within_limit(meta.len(), limit) => Some(
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("File too large: maximum {} bytes", limit),
            )
                .into_response(),
        ),
        Ok(_) => None,
        Err(e) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Metadata error: {}", e),
            )
                .into_response(),
        ),
    }
}

fn payload_too_large_response(limit: u64) -> Response {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        format!("File too large: maximum {} bytes", limit),
    )
        .into_response()
}

// `Response` is the gateway-wide error type (axum's response), so the large
// `Err` variant is intentional here rather than something to box.
#[allow(clippy::result_large_err)]
fn bounded_file_response_bytes(bytes: Vec<u8>, limit: u64) -> Result<Vec<u8>, Response> {
    if file_size_within_limit(bytes.len() as u64, limit) {
        Ok(bytes)
    } else {
        Err(payload_too_large_response(limit))
    }
}

#[allow(clippy::result_large_err)]
fn bounded_file_response_string(content: String, limit: u64) -> Result<String, Response> {
    if file_size_within_limit(content.len() as u64, limit) {
        Ok(content)
    } else {
        Err(payload_too_large_response(limit))
    }
}

async fn read_file_bytes_limited(path: &std::path::Path, limit: u64) -> Result<Vec<u8>, Response> {
    use tokio::io::AsyncReadExt;

    let file = tokio::fs::File::open(path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Read error: {}", e),
        )
            .into_response()
    })?;
    let mut reader = file.take(limit.saturating_add(1));
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Read error: {}", e),
        )
            .into_response()
    })?;
    bounded_file_response_bytes(bytes, limit)
}

async fn read_file_string_limited(path: &std::path::Path, limit: u64) -> Result<String, Response> {
    let bytes = read_file_bytes_limited(path, limit).await?;
    let content = String::from_utf8(bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Read error: {}", e),
        )
            .into_response()
    })?;
    bounded_file_response_string(content, limit)
}

#[derive(Deserialize)]
pub(super) struct FileListQuery {
    #[serde(default = "default_file_path")]
    path: String,
    #[serde(default)]
    agent: Option<String>,
}

fn default_file_path() -> String {
    ".".to_string()
}

/// GET /v1/files — list directory contents
pub(super) async fn handle_files_list(
    State(state): State<GatewayState>,
    Query(params): Query<FileListQuery>,
) -> impl IntoResponse {
    let agent_id = match resolve_requested_agent_id(&state.config, params.agent.as_deref()) {
        Ok(agent_id) => agent_id,
        Err(err) => return Json(serde_json::json!({ "error": err })),
    };
    let workspace = state.paths.for_agent(&agent_id).workspace();
    let target = if params.path == "." || params.path.is_empty() {
        workspace.to_path_buf()
    } else {
        workspace.join(&params.path)
    };

    // Security: ensure path is within workspace
    let canonical = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            if !target.exists() {
                return Json(serde_json::json!({ "error": "Path not found" }));
            }
            target.clone()
        }
    };
    let ws_canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    if !canonical.starts_with(&ws_canonical) {
        return Json(serde_json::json!({ "error": "Access denied: path outside workspace" }));
    }

    if !target.is_dir() {
        return Json(serde_json::json!({ "error": "Not a directory" }));
    }

    // 使用 tokio::fs 异步 API 避免阻塞工作线程
    let mut entries = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&target).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            let meta = entry.metadata().await.ok();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            });

            // Relative path from workspace
            let rel_path = entry
                .path()
                .strip_prefix(&workspace)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            let file_type = if is_dir {
                "directory".to_string()
            } else {
                match ext.as_str() {
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "image",
                    "mp3" | "wav" | "m4a" | "flac" | "ogg" => "audio",
                    "mp4" | "mkv" | "webm" | "avi" => "video",
                    "pdf" => "pdf",
                    "json" | "jsonl" => "json",
                    "md" | "txt" | "log" | "csv" | "yaml" | "yml" | "toml" | "xml" | "html"
                    | "css" | "js" | "ts" | "py" | "rs" | "sh" | "rhai" => "text",
                    "xlsx" | "xls" | "docx" | "pptx" => "office",
                    "zip" | "tar" | "gz" | "tgz" => "archive",
                    "db" | "sqlite" => "database",
                    _ => "file",
                }
                .to_string()
            };

            entries.push(serde_json::json!({
                "name": name,
                "path": rel_path,
                "is_dir": is_dir,
                "size": size,
                "type": file_type,
                "modified": modified,
            }));
        }
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| {
        let a_dir = a.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        let b_dir = b.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        match (b_dir, a_dir) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => {
                let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                a_name.cmp(b_name)
            }
        }
    });

    let count = entries.len();
    Json(serde_json::json!({
        "path": params.path,
        "entries": entries,
        "count": count,
    }))
}

#[derive(Deserialize)]
pub(super) struct FileContentQuery {
    path: String,
    #[serde(default)]
    agent: Option<String>,
}

/// GET /v1/files/content — read file content
pub(super) async fn handle_files_content(
    State(state): State<GatewayState>,
    Query(params): Query<FileContentQuery>,
) -> Response {
    let agent_id = match resolve_requested_agent_id(&state.config, params.agent.as_deref()) {
        Ok(agent_id) => agent_id,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };
    let workspace = state.paths.for_agent(&agent_id).workspace();
    let target = workspace.join(&params.path);

    // Security check
    let canonical = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };
    let ws_canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    if !canonical.starts_with(&ws_canonical) {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    if !target.is_file() {
        return (StatusCode::NOT_FOUND, "Not a file").into_response();
    }
    if let Some(response) = reject_if_file_too_large(&target, MAX_FILE_CONTENT_BYTES).await {
        return response;
    }

    let ext = target
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // For binary files (images, etc.), return base64 encoded
    let is_binary = matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "svg"
            | "mp3"
            | "wav"
            | "m4a"
            | "mp4"
            | "mkv"
            | "webm"
            | "pdf"
            | "xlsx"
            | "xls"
            | "docx"
            | "pptx"
            | "zip"
            | "tar"
            | "gz"
            | "db"
            | "sqlite"
    );

    let mime_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "json" | "jsonl" => "application/json",
        "html" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        _ => {
            if is_binary {
                "application/octet-stream"
            } else {
                "text/plain"
            }
        }
    };

    if is_binary {
        match read_file_bytes_limited(&target, MAX_FILE_CONTENT_BYTES).await {
            Ok(bytes) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                Json(serde_json::json!({
                    "path": params.path,
                    "encoding": "base64",
                    "mime_type": mime_type,
                    "size": bytes.len(),
                    "content": b64,
                }))
                .into_response()
            }
            Err(response) => response,
        }
    } else {
        match read_file_string_limited(&target, MAX_FILE_CONTENT_BYTES).await {
            Ok(content) => Json(serde_json::json!({
                "path": params.path,
                "encoding": "utf-8",
                "mime_type": mime_type,
                "size": content.len(),
                "content": content,
            }))
            .into_response(),
            Err(response) => response,
        }
    }
}

/// GET /v1/files/download — download a file
pub(super) async fn handle_files_download(
    State(state): State<GatewayState>,
    Query(params): Query<FileContentQuery>,
) -> Response {
    let agent_id = match resolve_requested_agent_id(&state.config, params.agent.as_deref()) {
        Ok(agent_id) => agent_id,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };
    let workspace = state.paths.for_agent(&agent_id).workspace();
    let target = workspace.join(&params.path);

    let canonical = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };
    let ws_canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    if !canonical.starts_with(&ws_canonical) {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }
    if let Some(response) = reject_if_file_too_large(&target, MAX_FILE_RESPONSE_BYTES).await {
        return response;
    }

    match read_file_bytes_limited(&target, MAX_FILE_RESPONSE_BYTES).await {
        Ok(bytes) => {
            let filename = target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("download");
            let headers = [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                ),
            ];
            (headers, bytes).into_response()
        }
        Err(response) => response,
    }
}

/// GET /v1/files/serve — serve a file inline with proper Content-Type (for <img>/<audio> tags)
///
/// Security: only files under the requesting agent's `media_dir()` may be served.
/// The request `path` may be either absolute or relative to `media_dir()`; either
/// way it is canonicalized and required to remain inside `media_dir()`. This
/// prevents reading `config.json5`, other agents' workspaces, sessions, or any
/// file outside this agent's media directory.
pub(super) async fn handle_files_serve(
    State(state): State<GatewayState>,
    Query(params): Query<FileContentQuery>,
) -> Response {
    let agent_id = match resolve_requested_agent_id(&state.config, params.agent.as_deref()) {
        Ok(agent_id) => agent_id,
        Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
    };

    // The allowed root is the agent's media directory, not ~/.blockcell/ base.
    let media_dir = state.paths.for_agent(&agent_id).media_dir();
    let media_canonical = media_dir
        .canonicalize()
        .unwrap_or_else(|_| media_dir.clone());

    // Accept absolute paths (as produced by the agent runtime) and paths
    // relative to media_dir. Canonicalize the candidate and enforce that it
    // stays inside media_dir — this is the sole security boundary.
    let candidate = if std::path::Path::new(&params.path).is_absolute() {
        std::path::PathBuf::from(&params.path)
    } else {
        media_dir.join(&params.path)
    };
    let canonical = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };
    if !canonical.starts_with(&media_canonical) {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    if !canonical.is_file() {
        return (StatusCode::NOT_FOUND, "Not a file").into_response();
    }
    let target = canonical.clone();
    if let Some(response) = reject_if_file_too_large(&target, MAX_FILE_RESPONSE_BYTES).await {
        return response;
    }

    let ext = target
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let content_type = match ext.as_str() {
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "heic" | "heif" => "image/heic",
        "tiff" | "tif" => "image/tiff",
        // Audio
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" | "aac" => "audio/aac",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "opus" => "audio/opus",
        "weba" => "audio/webm",
        // Video
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        // Other
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };

    match read_file_bytes_limited(&target, MAX_FILE_RESPONSE_BYTES).await {
        Ok(bytes) => {
            let headers = [
                (header::CONTENT_TYPE, content_type.to_string()),
                (header::CACHE_CONTROL, "public, max-age=3600".to_string()),
            ];
            (headers, bytes).into_response()
        }
        Err(response) => response,
    }
}

/// POST /v1/files/upload — upload a file to workspace
pub(super) async fn handle_files_upload(
    State(state): State<GatewayState>,
    Query(agent): Query<AgentScopedQuery>,
    Json(req): Json<serde_json::Value>,
) -> Response {
    let agent_id = match resolve_requested_agent_id(&state.config, agent.agent.as_deref()) {
        Ok(agent_id) => agent_id,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err })),
            )
                .into_response()
        }
    };
    let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let encoding = req
        .get("encoding")
        .and_then(|v| v.as_str())
        .unwrap_or("utf-8");

    let rel = match validate_workspace_relative_path(path) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response()
        }
    };

    let upload_within_limit = if encoding == "base64" {
        base64_upload_within_decoded_limit(content, MAX_FILE_UPLOAD_BYTES)
    } else {
        utf8_upload_within_limit(content, MAX_FILE_UPLOAD_BYTES)
    };
    if !upload_within_limit {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "error": format!("Upload too large: maximum {} bytes", MAX_FILE_UPLOAD_BYTES)
            })),
        )
            .into_response();
    }

    let workspace = state.paths.for_agent(&agent_id).workspace();
    let target = match resolve_workspace_upload_target(&workspace, &rel) {
        Ok(target) => target,
        Err(error) => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": error })),
            )
                .into_response()
        }
    };
    let path_echo = rel.to_string_lossy().to_string();
    let content = content.to_string();
    let encoding = encoding.to_string();

    let result = tokio::task::spawn_blocking(move || {
        if let Some(parent) = target.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(format!("{}", e));
            }
        }

        if encoding == "base64" {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(content.as_bytes())
                .map_err(|e| format!("Base64 decode error: {}", e))?;
            std::fs::write(&target, bytes).map_err(|e| format!("{}", e))?;
        } else {
            std::fs::write(&target, content).map_err(|e| format!("{}", e))?;
        }
        Ok(())
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            Json(serde_json::json!({ "status": "uploaded", "path": path_echo })).into_response()
        }
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("{}", e) })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn gateway_upload_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "blockcell-gateway-upload-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        let real = workspace.join("real");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        std::fs::create_dir_all(&real).expect("create real dir");
        symlink(&outside, workspace.join("link")).expect("create symlink");

        let escaped =
            resolve_workspace_upload_target(&workspace, std::path::Path::new("link/new.txt"));
        assert!(escaped.is_err(), "symlink escape must be rejected");

        let allowed =
            resolve_workspace_upload_target(&workspace, std::path::Path::new("real/new.txt"))
                .expect("real workspace target should be allowed");
        assert!(allowed.starts_with(std::fs::canonicalize(&workspace).unwrap()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn file_size_limit_rejects_values_above_limit() {
        assert!(file_size_within_limit(
            MAX_FILE_CONTENT_BYTES,
            MAX_FILE_CONTENT_BYTES
        ));
        assert!(!file_size_within_limit(
            MAX_FILE_CONTENT_BYTES + 1,
            MAX_FILE_CONTENT_BYTES
        ));
    }

    #[test]
    fn base64_upload_size_estimate_rejects_decoded_payload_above_limit() {
        assert!(base64_upload_within_decoded_limit("YWJj", 3));
        assert!(!base64_upload_within_decoded_limit("YWJj", 2));
    }

    #[test]
    fn utf8_upload_size_rejects_payload_above_limit() {
        assert!(utf8_upload_within_limit("abcd", 4));
        assert!(!utf8_upload_within_limit("abcd", 3));
    }

    #[test]
    fn bounded_file_response_rejects_bytes_read_after_metadata_check_limit() {
        let result = bounded_file_response_bytes(vec![0; 5], 4);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_file_bytes_limited_stops_after_limit_and_rejects() {
        let dir = std::env::temp_dir().join(format!(
            "blockcell-file-limit-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        tokio::fs::create_dir_all(&dir)
            .await
            .expect("create temp dir");
        let path = dir.join("large.txt");
        tokio::fs::write(&path, b"12345").await.expect("write file");

        let result = read_file_bytes_limited(&path, 4).await;

        assert!(result.is_err());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
