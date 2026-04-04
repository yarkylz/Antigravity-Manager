export interface Account {
    id: string;
    email: string;
    name?: string;
    token: TokenData;
    device_profile?: DeviceProfile;
    device_history?: DeviceProfileVersion[];
    quota?: QuotaData;
    disabled?: boolean;
    disabled_reason?: string;
    disabled_at?: number;
    proxy_disabled?: boolean;
    proxy_disabled_reason?: string;
    proxy_disabled_at?: number;
    protected_models?: string[];
    custom_label?: string;  // 用户自定义标签
    proxy_id?: string;  // 绑定的代理ID
    proxy_bound_at?: number;  // 代理绑定时间
    validation_blocked?: boolean;
    validation_blocked_until?: number;
    validation_blocked_reason?: string;
    validation_url?: string;
    raw_error_response?: string;  // Raw API error response for debugging (Show Raw)
    location_blocked?: boolean;  // Account blocked due to unsupported location (403)
    ban_blocked?: boolean;  // Account blocked due to TOS violation (403)
    age_blocked?: boolean;  // Account blocked due to age restriction (under 18) (403)
    created_at: number;
    last_used: number;
}

export interface TokenData {
    access_token: string;
    refresh_token: string;
    expires_in: number;
    expiry_timestamp: number;
    token_type: string;
    email?: string;
}

export interface QuotaData {
    models: ModelQuota[];
    last_updated: number;
    is_forbidden?: boolean;
    forbidden_reason?: string;
    subscription_tier?: string;  // 订阅类型: FREE/PRO/ULTRA
    restriction_reason?: string;  // 账号受限原因 (来自 ineligibleTiers)
    validation_url?: string;  // 验证链接 URL
    model_forwarding_rules?: Record<string, string>; // 废弃模型转发表
    is_location_blocked?: boolean;  // Account blocked due to unsupported location
    is_ban_blocked?: boolean;  // Account blocked due to TOS violation
    is_age_blocked?: boolean;  // Account blocked due to age restriction (under 18)
}

export interface ModelQuota {
    name: string;
    percentage: number;
    reset_time: string;
    display_name?: string;
    supports_images?: boolean;
    supports_thinking?: boolean;
    thinking_budget?: number;
    recommended?: boolean;
    max_tokens?: number;
    max_output_tokens?: number;
    supported_mime_types?: Record<string, boolean>;
}

export interface DeviceProfile {
    machine_id: string;
    mac_machine_id: string;
    dev_device_id: string;
    sqm_id: string;
}

export interface DeviceProfileVersion {
    id: string;
    created_at: number;
    label: string;
    profile: DeviceProfile;
    is_current?: boolean;
}

