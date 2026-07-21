use super::*;

impl DreamConsolidator {
    /// 创建执行器
    pub async fn new(config_dir: &Path) -> std::io::Result<Self> {
        let state = DreamState::load(config_dir).await?;
        Ok(Self {
            config_dir: config_dir.to_path_buf(),
            state,
            gate_config: ConsolidatorConfig::default(),
            provider_pool: None,
            dream_lock: std::sync::Mutex::new(None),
        })
    }

    /// 使用自定义门控配置
    pub fn with_gate_config(mut self, config: ConsolidatorConfig) -> Self {
        self.gate_config = config;
        self
    }

    /// 设置 Provider 池
    ///
    /// 必须在调用 `dream()` 之前设置，否则 Forked Agent 无法执行 LLM 调用
    pub fn with_provider_pool(
        mut self,
        provider_pool: Arc<blockcell_providers::ProviderPool>,
    ) -> Self {
        self.provider_pool = Some(provider_pool);
        self
    }

    /// 检查是否应该执行梦境
    pub async fn should_dream(&mut self) -> GateCheckResult {
        check_gates(&mut self.state, &self.config_dir, &self.gate_config).await
    }
}
