use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignedArtifact<'a> {
    schema: &'static str,
    channel: &'a str,
    version: &'a str,
    os: &'a str,
    arch: &'a str,
    url: &'a str,
    sha256: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub channel: String,
    pub version: String,
    pub published_at: String,
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub min_host_version: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub os: String,
    pub arch: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub sig: Option<String>,
}

impl Manifest {
    pub fn get_artifact(&self, os: &str, arch: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.os == os && a.arch == arch)
    }

    /// 返回需要由发布密钥签名的规范化载荷。
    ///
    /// 固定字段顺序的 JSON 同时绑定发布版本、渠道、平台、下载地址和内容摘要，
    /// 防止把旧版本的合法签名产物重新包装成伪造的新版本。
    pub fn signature_payload(&self, artifact: &Artifact) -> Vec<u8> {
        serde_json::to_vec(&SignedArtifact {
            schema: "blockcell-update-v1",
            channel: &self.channel,
            version: &self.version,
            os: &artifact.os,
            arch: &artifact.arch,
            url: &artifact.url,
            sha256: &artifact.sha256,
        })
        .expect("serializing signed update metadata cannot fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            channel: "stable".to_string(),
            version: "1.2.3".to_string(),
            published_at: "2026-07-22T00:00:00Z".to_string(),
            artifacts: vec![Artifact {
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                url: "https://example.com/blockcell".to_string(),
                sha256: "ab".repeat(32),
                sig: None,
            }],
            min_host_version: None,
            notes: String::new(),
        }
    }

    #[test]
    fn signature_payload_binds_release_and_artifact_metadata() {
        let original = manifest();
        let original_payload = original.signature_payload(&original.artifacts[0]);

        let mut changed = original.clone();
        changed.version = "9.9.9".to_string();
        assert_ne!(
            original_payload,
            changed.signature_payload(&changed.artifacts[0])
        );

        let mut changed = original.clone();
        changed.artifacts[0].arch = "aarch64".to_string();
        assert_ne!(
            original_payload,
            changed.signature_payload(&changed.artifacts[0])
        );

        let mut changed = original.clone();
        changed.artifacts[0].sha256 = "cd".repeat(32);
        assert_ne!(
            original_payload,
            changed.signature_payload(&changed.artifacts[0])
        );
    }
}
