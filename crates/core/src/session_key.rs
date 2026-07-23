use sha2::{Digest, Sha256};

/// 计算 session_key 的稳定哈希后缀（64 位，16 位十六进制字符）。
///
/// 使用 SHA-256 取前 8 字节，保证跨平台、跨 Rust 版本的一致性。
/// 用于文件系统路径生成，避免 [`std::collections::hash_map::DefaultHasher`]
/// 在不同 Rust 版本间不兼容的问题。
///
/// ## 设计决策
/// - 使用 SHA-256 而非 DefaultHasher：保证输出是稳定持久化格式契约
/// - 截取 64 bit（而非 32 bit）：session 数量较大时降低碰撞风险
/// - 所有写入侧和读取侧（如 `session_recall`）必须使用同一实现
pub fn stable_hash_session_key(session_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_key.as_bytes());
    let result = hasher.finalize();
    // 取前 8 字节（64 位）作为十六进制后缀
    // SHA-256 输出固定 32 字节，直接索引构造 [u8; 8] 安全无 panic
    let hash_u64 = u64::from_be_bytes([
        result[0], result[1], result[2], result[3], result[4], result[5], result[6], result[7],
    ]);
    format!("{:016x}", hash_u64)
}

pub fn build_session_key(channel: &str, chat_id: &str) -> String {
    format!("{}:{}", channel, chat_id)
}

/// 将 session_key 转换为无碰撞、可逆且文件系统安全的文件名（stem）。
pub fn session_file_stem(session_key: &str) -> String {
    if session_key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | ':'))
        && !session_key.ends_with('.')
    {
        return legacy_session_file_stem(session_key);
    }

    let mut stem = String::from("s~");
    for byte in session_key.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'-' | b'.') {
            stem.push(*byte as char);
        } else {
            stem.push('~');
            stem.push_str(&format!("{byte:02X}"));
        }
    }
    stem
}

/// 旧版本使用的有碰撞文件名格式，仅用于读取和清理已有会话文件。
pub fn legacy_session_file_stem(session_key: &str) -> String {
    session_key.replace([':', '/', '\\'], "_")
}

fn decode_session_file_stem(file_stem: &str) -> Option<String> {
    let encoded = file_stem.strip_prefix("s~")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// 从文件 stem 中提取 session_id（即 channel 之后的部分）。
///
/// 使用第一个 `_` 作为 channel 与 chat_id 的分界。若 chat_id 中包含 `_`，
/// 不会被错误截断——但后续 `session_title_from_id` 会将所有 `_` 转为 `:`，
/// 导致 chat_id 中原有的 `_` 丢失。参见 [`session_file_stem`] 的已知限制。
pub fn session_id_from_file_stem(file_stem: &str) -> String {
    if let Some(session_key) = decode_session_file_stem(file_stem) {
        return session_key
            .split_once(':')
            .map(|(_, session_id)| session_id.to_string())
            .unwrap_or(session_key);
    }
    file_stem
        .find('_')
        .map(|pos| file_stem[pos + 1..].to_string())
        .unwrap_or_else(|| file_stem.to_string())
}

pub fn session_title_from_id(session_id: &str) -> String {
    if session_id.contains(':') {
        session_id.to_string()
    } else {
        session_id.replace('_', ":")
    }
}

pub fn resolve_session_key_from_id<'a, I>(session_id: &str, file_stems: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let stems: Vec<&str> = file_stems.into_iter().collect();
    for file_stem in &stems {
        if session_id_from_file_stem(file_stem) == session_id {
            return decode_session_file_stem(file_stem)
                .unwrap_or_else(|| file_stem.replace('_', ":"));
        }
    }

    let normalized_id = session_id.replace(':', "_");
    let direct_key = build_session_key("ws", &session_title_from_id(session_id));
    let direct_stem = session_file_stem(&direct_key);

    if stems.iter().any(|stem| **stem == direct_stem) {
        return direct_key;
    }

    for file_stem in stems {
        if file_stem == normalized_id || session_id_from_file_stem(file_stem) == normalized_id {
            return file_stem.replace('_', ":");
        }
    }

    direct_key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_session_key() {
        assert_eq!(build_session_key("ws", "default:123"), "ws:default:123");
    }

    #[test]
    fn test_session_file_stem() {
        let colon = session_file_stem("ws:a:b");
        let underscore = session_file_stem("ws:a_b");
        let slash = session_file_stem("ws:a/b");
        let backslash = session_file_stem("ws:a\\b");

        assert_ne!(colon, underscore);
        assert_ne!(underscore, slash);
        assert_ne!(slash, backslash);
        for stem in [colon, underscore, slash, backslash] {
            assert!(!stem.contains([':', '/', '\\']));
        }
    }

    #[test]
    fn test_session_id_from_file_stem() {
        assert_eq!(
            session_id_from_file_stem(&session_file_stem("ws:default_123")),
            "default_123"
        );
        assert_eq!(session_id_from_file_stem("ws_default_123"), "default_123");
        assert_eq!(session_id_from_file_stem("default_123"), "123");
    }

    #[test]
    fn test_resolve_session_key_from_id_prefers_existing_direct_stem() {
        let stems = ["ws_default_123", "telegram_chat_1"];
        assert_eq!(
            resolve_session_key_from_id("default_123", stems.iter().copied()),
            "ws:default:123"
        );
    }

    #[test]
    fn test_resolve_session_key_from_id_falls_back_to_matching_stem() {
        let stems = ["ws_ws_default_123", "telegram_chat_1"];
        assert_eq!(
            resolve_session_key_from_id("ws_default_123", stems.iter().copied()),
            "ws:ws:default:123"
        );
    }
}
