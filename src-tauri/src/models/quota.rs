use serde::{Deserialize, Serialize};

/// 模型配额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuota {
    pub name: String,
    pub percentage: i32, // 剩余百分比 0-100
    pub reset_time: String,

    // -- 动态参数解析与持久化 --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_thinking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_mime_types: Option<std::collections::HashMap<String, bool>>,
}

/// 配额数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaData {
    pub models: Vec<ModelQuota>,
    pub last_updated: i64,
    #[serde(default)]
    pub is_forbidden: bool,
    /// 禁止访问的原因 (403 详细信息)
    #[serde(default)]
    pub forbidden_reason: Option<String>,
    /// 订阅等级 (FREE/PRO/ULTRA)
    #[serde(default)]
    pub subscription_tier: Option<String>,
    /// 受限原因 (e.g. "UNSUPPORTED_LOCATION")
    #[serde(default)]
    pub restriction_reason: Option<String>,
    /// 验证链接 URL (从 ineligibleTiers 获取)
    #[serde(default)]
    pub validation_url: Option<String>,
    /// 模型淘汰重定向规则表 (old_model_id -> new_model_id)
    #[serde(default)]
    pub model_forwarding_rules: std::collections::HashMap<String, String>,
    /// [NEW] Account is blocked due to unsupported location (subset of is_forbidden)
    #[serde(default)]
    pub is_location_blocked: bool,
}

impl QuotaData {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            last_updated: chrono::Utc::now().timestamp(),
            is_forbidden: false,
            forbidden_reason: None,
            subscription_tier: None,
            restriction_reason: None,
            validation_url: None,
            model_forwarding_rules: std::collections::HashMap::new(),
            is_location_blocked: false,
        }
    }

    pub fn add_model(&mut self, model: ModelQuota) {
        self.models.push(model);
    }
}

impl Default for QuotaData {
    fn default() -> Self {
        Self::new()
    }
}
