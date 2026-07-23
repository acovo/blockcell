use blockcell_core::{Error, Result};
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub(crate) fn append_limited(output: &mut Vec<u8>, chunk: &[u8], limit: usize) -> bool {
    let remaining = limit.saturating_sub(output.len());
    let take = remaining.min(chunk.len());
    output.extend_from_slice(&chunk[..take]);
    take < chunk.len() || output.len() >= limit
}

pub(crate) async fn read_response_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<(Vec<u8>, bool)> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| Error::Tool(format!("Failed to read response body: {e}")))?
    {
        if append_limited(&mut output, &chunk, limit) {
            truncated = true;
            break;
        }
    }
    Ok((output, truncated))
}

pub(crate) async fn stream_response_to_file(
    mut response: reqwest::Response,
    path: &Path,
    limit: usize,
) -> Result<(usize, bool)> {
    let mut file = tokio::fs::File::create(path).await?;
    let mut written = 0usize;
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| Error::Tool(format!("Failed to read response body: {e}")))?
    {
        let remaining = limit.saturating_sub(written);
        let take = remaining.min(chunk.len());
        file.write_all(&chunk[..take]).await?;
        written += take;
        if take < chunk.len() || written >= limit {
            truncated = true;
            break;
        }
    }
    file.flush().await?;
    Ok((written, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_limited_stops_at_byte_limit() {
        let mut output = Vec::new();
        assert!(!append_limited(&mut output, b"abcd", 6));
        assert!(append_limited(&mut output, b"efgh", 6));
        assert_eq!(output, b"abcdef");
    }
}
