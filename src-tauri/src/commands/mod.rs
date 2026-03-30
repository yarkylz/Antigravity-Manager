use crate::models::{Account, AppConfig, QuotaData};
use crate::modules;
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

// 导出 proxy 命令
pub mod proxy;
// 导出 autostart 命令
pub mod autostart;
// 导出 cloudflared 命令
pub mod cloudflared;
// 导出 security 命令 (IP 监控)
pub mod security;
// 导出 proxy_pool 命令
pub mod proxy_pool;
// 导出 user_token 命令
pub mod user_token;

/// 列出所有账号
#[tauri::command]
pub async fn list_accounts() -> Result<Vec<Account>, String> {
    modules::list_accounts()
}

/// 添加账号
#[tauri::command]
pub async fn add_account(
    app: tauri::AppHandle,
    _email: String,
    refresh_token: String,
    custom_label: Option<String>,
    proxy_id: Option<String>,
) -> Result<Account, String> {
    let service = modules::account_service::AccountService::new(
        crate::modules::integration::SystemManager::Desktop(app.clone()),
    );

    let mut account = service.add_account(&refresh_token).await?;

    // Apply custom label and proxy binding if provided (atomic update to avoid TOCTOU race)
    let mut needs_persistence = false;
    let _account_json: Option<serde_json::Value> = None;
    let data_dir = modules::account::get_data_dir()?;
    let account_path = data_dir
        .join("accounts")
        .join(format!("{}.json", account.id));

    if account_path.exists() {
        let content = std::fs::read_to_string(&account_path)
            .map_err(|e| format!("Failed to read account file: {}", e))?;
        let mut json = serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| format!("Failed to parse account file: {}", e))?;

        // Apply custom label if provided
        if let Some(ref label) = custom_label {
            let label = label.trim().to_string();
            if !label.is_empty() {
                // Validate label length (same rule as update_account_label)
                if label.chars().count() > 15 {
                    modules::logger::log_info("Custom label too long, truncating to 15 chars");
                }
                let truncated: String = label.chars().take(15).collect();
                account.custom_label = Some(truncated.clone());
                json["custom_label"] = serde_json::Value::String(truncated);
                needs_persistence = true;
            }
        }

        // Bind proxy if provided
        if let Some(ref pid) = proxy_id {
            if !pid.is_empty() {
                let proxy_state = app.state::<crate::commands::proxy::ProxyServiceState>();
                let instance_lock = proxy_state.instance.read().await;
                if let Some(instance) = instance_lock.as_ref() {
                    match instance
                        .axum_server
                        .proxy_pool_manager
                        .bind_account_to_proxy(account.id.clone(), pid.clone())
                        .await
                    {
                        Ok(_) => {
                            account.proxy_id = Some(pid.clone());
                            account.proxy_bound_at = Some(chrono::Utc::now().timestamp());
                            json["proxy_id"] = serde_json::Value::String(pid.clone());
                            json["proxy_bound_at"] = serde_json::json!(account.proxy_bound_at);
                            needs_persistence = true;
                        }
                        Err(e) => {
                            tracing::warn!(
                                proxy_id = %pid,
                                account_id = %account.id,
                                error = %e,
                                "Proxy binding failed after account was saved — account will appear without proxy"
                            );
                            // Do NOT return Err here: the account is already persisted to disk.
                            // Returning Err would cause the frontend to skip fetchAccounts(),
                            // creating a phantom account (exists on disk but invisible in UI).
                        }
                    }
                    drop(instance_lock);
                }
            }
        }

        if needs_persistence {
            let json_str = serde_json::to_string_pretty(&json)
                .map_err(|e| format!("Failed to serialize account: {}", e))?;
            std::fs::write(&account_path, json_str)
                .map_err(|e| format!("Failed to write account file: {}", e))?;
        }
    }

    // Auto-generate device fingerprint
    auto_generate_device_fingerprint(&account);

    // 自动刷新配额
    let _ = internal_refresh_account_quota(&app, &mut account).await;

    // 重载账号池
    let _ = crate::commands::proxy::reload_proxy_accounts(
        app.state::<crate::commands::proxy::ProxyServiceState>(),
    )
    .await;

    Ok(account)
}

/// Apply custom_label and proxy_id to an account after creation
async fn apply_account_metadata(
    app: &tauri::AppHandle,
    account: &mut crate::models::Account,
    custom_label: Option<String>,
    proxy_id: Option<String>,
) -> Result<(), String> {
    let mut needs_persistence = false;
    let data_dir = modules::account::get_data_dir()?;
    let account_path = data_dir
        .join("accounts")
        .join(format!("{}.json", account.id));

    if account_path.exists() {
        let content = std::fs::read_to_string(&account_path)
            .map_err(|e| format!("Failed to read account file: {}", e))?;
        let mut json = serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| format!("Failed to parse account file: {}", e))?;

        // Apply custom label if provided
        if let Some(ref label) = custom_label {
            let label = label.trim().to_string();
            if !label.is_empty() {
                let truncated: String = label.chars().take(15).collect();
                account.custom_label = Some(truncated.clone());
                json["custom_label"] = serde_json::Value::String(truncated);
                needs_persistence = true;
            }
        }

        // Bind proxy if provided
        if let Some(ref pid) = proxy_id {
            if !pid.is_empty() {
                let proxy_state = app.state::<crate::commands::proxy::ProxyServiceState>();
                let instance_lock = proxy_state.instance.read().await;
                if let Some(instance) = instance_lock.as_ref() {
                    match instance
                        .axum_server
                        .proxy_pool_manager
                        .bind_account_to_proxy(account.id.clone(), pid.clone())
                        .await
                    {
                        Ok(_) => {
                            account.proxy_id = Some(pid.clone());
                            account.proxy_bound_at = Some(chrono::Utc::now().timestamp());
                            json["proxy_id"] = serde_json::Value::String(pid.clone());
                            json["proxy_bound_at"] = serde_json::json!(account.proxy_bound_at);
                            needs_persistence = true;
                        }
                        Err(e) => {
                            tracing::warn!(
                                proxy_id = %pid,
                                account_id = %account.id,
                                error = %e,
                                "Proxy binding failed after OAuth account was saved"
                            );
                        }
                    }
                    drop(instance_lock);
                }
            }
        }

        if needs_persistence {
            let json_str = serde_json::to_string_pretty(&json)
                .map_err(|e| format!("Failed to serialize account: {}", e))?;
            std::fs::write(&account_path, json_str)
                .map_err(|e| format!("Failed to write account file: {}", e))?;
        }
    }

    Ok(())
}

/// Auto-generate and bind a device fingerprint for a newly created account (if none exists).
/// Failure is non-fatal: the account is already created, so we just log the error.
fn auto_generate_device_fingerprint(account: &Account) {
    if account.device_profile.is_none() {
        modules::logger::log_info(&format!(
            "Auto-generating device fingerprint for new account: {}",
            account.email
        ));
        if let Err(e) = modules::account::bind_device_profile(&account.id, "generate") {
            modules::logger::log_info(&format!(
                "Auto device fingerprint generation failed for {}: {}",
                account.email, e
            ));
        }
    }
}

/// 删除账号
/// 删除账号
#[tauri::command]
pub async fn delete_account(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_id: String,
) -> Result<(), String> {
    let service = modules::account_service::AccountService::new(
        crate::modules::integration::SystemManager::Desktop(app.clone()),
    );
    service.delete_account(&account_id)?;

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    Ok(())
}

/// 批量删除账号
#[tauri::command]
pub async fn delete_accounts(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_ids: Vec<String>,
) -> Result<(), String> {
    modules::logger::log_info(&format!(
        "收到批量删除请求，共 {} 个账号",
        account_ids.len()
    ));
    modules::account::delete_accounts(&account_ids).map_err(|e| {
        modules::logger::log_error(&format!("批量删除失败: {}", e));
        e
    })?;

    // 强制同步托盘
    crate::modules::tray::update_tray_menus(&app);

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    Ok(())
}

/// 重新排序账号列表
/// 根据传入的账号ID数组顺序更新账号排列
#[tauri::command]
pub async fn reorder_accounts(
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_ids: Vec<String>,
) -> Result<(), String> {
    modules::logger::log_info(&format!(
        "收到账号重排序请求，共 {} 个账号",
        account_ids.len()
    ));
    modules::account::reorder_accounts(&account_ids).map_err(|e| {
        modules::logger::log_error(&format!("账号重排序失败: {}", e));
        e
    })?;

    // Reload pool to reflect new order if running
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;
    Ok(())
}

/// 切换账号
#[tauri::command]
pub async fn switch_account(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_id: String,
) -> Result<(), String> {
    let service = modules::account_service::AccountService::new(
        crate::modules::integration::SystemManager::Desktop(app.clone()),
    );

    service.switch_account(&account_id).await?;

    // 同步托盘
    crate::modules::tray::update_tray_menus(&app);

    // [FIX #820] Notify proxy to clear stale session bindings and reload accounts
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    Ok(())
}

/// 获取当前账号
#[tauri::command]
pub async fn get_current_account() -> Result<Option<Account>, String> {
    // println!("🚀 Backend Command: get_current_account called"); // Commented out to reduce noise for frequent calls, relies on frontend log for frequency
    // Actually user WANTS to see it.
    modules::logger::log_info("Backend Command: get_current_account called");

    let account_id = modules::get_current_account_id()?;

    if let Some(id) = account_id {
        // modules::logger::log_info(&format!("   Found current account ID: {}", id));
        modules::load_account(&id).map(Some)
    } else {
        modules::logger::log_info("   No current account set");
        Ok(None)
    }
}

/// 导出账号（包含 refresh_token）
use crate::models::AccountExportResponse;

#[tauri::command]
pub async fn export_accounts(account_ids: Vec<String>) -> Result<AccountExportResponse, String> {
    modules::account::export_accounts_by_ids(&account_ids)
}

/// 内部辅助功能：在添加或导入账号后自动刷新一次额度
async fn internal_refresh_account_quota(
    app: &tauri::AppHandle,
    account: &mut Account,
) -> Result<QuotaData, String> {
    modules::logger::log_info(&format!("自动触发刷新配额: {}", account.email));

    // 使用带重试的查询 (Shared logic)
    match modules::account::fetch_quota_with_retry(account).await {
        Ok(quota) => {
            // 更新账号配额
            let _ = modules::update_account_quota(&account.id, quota.clone());
            // 更新托盘菜单
            crate::modules::tray::update_tray_menus(app);
            Ok(quota)
        }
        Err(e) => {
            modules::logger::log_warn(&format!("自动刷新配额失败 ({}): {}", account.email, e));
            Err(e.to_string())
        }
    }
}

/// 查询账号配额
#[tauri::command]
pub async fn fetch_account_quota(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_id: String,
) -> crate::error::AppResult<QuotaData> {
    modules::logger::log_info(&format!("手动刷新配额请求: {}", account_id));
    let mut account =
        modules::load_account(&account_id).map_err(crate::error::AppError::Account)?;

    // 使用带重试的查询 (Shared logic)
    let quota = modules::account::fetch_quota_with_retry(&mut account).await?;

    // 4. 更新账号配额
    modules::update_account_quota(&account_id, quota.clone())
        .map_err(crate::error::AppError::Account)?;

    crate::modules::tray::update_tray_menus(&app);

    // 5. 同步到运行中的反代服务（如果已启动）
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        let _ = instance.token_manager.reload_account(&account_id).await;
    }

    Ok(quota)
}

pub use modules::account::RefreshStats;

/// 刷新所有账号配额 (内部实现)
pub async fn refresh_all_quotas_internal(
    proxy_state: &crate::commands::proxy::ProxyServiceState,
    app_handle: Option<tauri::AppHandle>,
) -> Result<RefreshStats, String> {
    let stats = modules::account::refresh_all_quotas_logic().await?;

    // 同步到运行中的反代服务（如果已启动）
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        let _ = instance.token_manager.reload_all_accounts().await;
    }

    // 发送全局刷新事件给 UI (如果需要)
    if let Some(handle) = app_handle {
        use tauri::Emitter;
        let _ = handle.emit("accounts://refreshed", ());
    }

    Ok(stats)
}

/// 刷新所有账号配额 (Tauri Command)
#[tauri::command]
pub async fn refresh_all_quotas(
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    app_handle: tauri::AppHandle,
) -> Result<RefreshStats, String> {
    refresh_all_quotas_internal(&proxy_state, Some(app_handle)).await
}
/// 获取设备指纹（当前 storage.json + 账号绑定）
#[tauri::command]
pub async fn get_device_profiles(
    account_id: String,
) -> Result<modules::account::DeviceProfiles, String> {
    modules::get_device_profiles(&account_id)
}

/// 绑定设备指纹（capture: 采集当前；generate: 生成新指纹），并写入 storage.json
#[tauri::command]
pub async fn bind_device_profile(
    account_id: String,
    mode: String,
) -> Result<crate::models::DeviceProfile, String> {
    modules::bind_device_profile(&account_id, &mode)
}

/// 预览生成一个指纹（不落盘）
#[tauri::command]
pub async fn preview_generate_profile() -> Result<crate::models::DeviceProfile, String> {
    Ok(crate::modules::device::generate_profile())
}

/// 使用给定指纹直接绑定
#[tauri::command]
pub async fn bind_device_profile_with_profile(
    account_id: String,
    profile: crate::models::DeviceProfile,
) -> Result<crate::models::DeviceProfile, String> {
    modules::bind_device_profile_with_profile(&account_id, profile, Some("generated".to_string()))
}

/// 将账号已绑定的指纹应用到 storage.json
#[tauri::command]
pub async fn apply_device_profile(
    account_id: String,
) -> Result<crate::models::DeviceProfile, String> {
    modules::apply_device_profile(&account_id)
}

/// 恢复最早的 storage.json 备份（近似“原始”状态）
#[tauri::command]
pub async fn restore_original_device() -> Result<String, String> {
    modules::restore_original_device()
}

/// 列出指纹版本
#[tauri::command]
pub async fn list_device_versions(
    account_id: String,
) -> Result<modules::account::DeviceProfiles, String> {
    modules::list_device_versions(&account_id)
}

/// 按版本恢复指纹
#[tauri::command]
pub async fn restore_device_version(
    account_id: String,
    version_id: String,
) -> Result<crate::models::DeviceProfile, String> {
    modules::restore_device_version(&account_id, &version_id)
}

/// 删除历史指纹（baseline 不可删）
#[tauri::command]
pub async fn delete_device_version(account_id: String, version_id: String) -> Result<(), String> {
    modules::delete_device_version(&account_id, &version_id)
}

/// 打开设备存储目录
#[tauri::command]
pub async fn open_device_folder(app: tauri::AppHandle) -> Result<(), String> {
    let dir = modules::device::get_storage_dir()?;
    let dir_str = dir
        .to_str()
        .ok_or("无法解析存储目录路径为字符串")?
        .to_string();
    app.opener()
        .open_path(dir_str, None::<&str>)
        .map_err(|e| format!("打开目录失败: {}", e))
}

/// 加载配置
#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    modules::load_app_config()
}

/// 保存配置
#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    config: AppConfig,
) -> Result<(), String> {
    modules::save_app_config(&config)?;

    // 通知托盘配置已更新
    let _ = app.emit("config://updated", ());

    // 热更新正在运行的服务
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        // 更新模型映射
        instance.axum_server.update_mapping(&config.proxy).await;
        // 更新上游代理
        instance
            .axum_server
            .update_proxy(config.proxy.upstream_proxy.clone())
            .await;
        // 更新安全策略 (auth)
        instance.axum_server.update_security(&config.proxy).await;
        // 更新 z.ai 配置
        instance.axum_server.update_zai(&config.proxy).await;
        // 更新实验性配置
        instance
            .axum_server
            .update_experimental(&config.proxy)
            .await;
        // 更新调试日志配置
        instance
            .axum_server
            .update_debug_logging(&config.proxy)
            .await;
        // [NEW] 更新 User-Agent 配置
        instance.axum_server.update_user_agent(&config.proxy).await;
        // 更新 Thinking Budget 配置
        crate::proxy::update_thinking_budget_config(config.proxy.thinking_budget.clone());
        // [NEW] 更新全局系统提示词配置
        crate::proxy::update_global_system_prompt_config(config.proxy.global_system_prompt.clone());
        // [NEW] 更新全局图像思维模式配置
        crate::proxy::update_image_thinking_mode(config.proxy.image_thinking_mode.clone());
        // 更新代理池配置
        instance
            .axum_server
            .update_proxy_pool(config.proxy.proxy_pool.clone())
            .await;
        // 更新熔断配置
        instance
            .token_manager
            .update_circuit_breaker_config(config.circuit_breaker.clone())
            .await;
        tracing::debug!("已同步热更新反代服务配置");
    }

    Ok(())
}

// --- OAuth 命令 ---

#[tauri::command]
pub async fn start_oauth_login(
    app_handle: tauri::AppHandle,
    custom_label: Option<String>,
    proxy_id: Option<String>,
) -> Result<Account, String> {
    modules::logger::log_info("开始 OAuth 授权流程...");
    let service = modules::account_service::AccountService::new(
        crate::modules::integration::SystemManager::Desktop(app_handle.clone()),
    );

    let mut account = service.start_oauth_login().await?;

    // 应用 custom label 和 proxy binding
    apply_account_metadata(&app_handle, &mut account, custom_label, proxy_id).await?;

    // Auto-generate device fingerprint
    auto_generate_device_fingerprint(&account);

    // 自动触发刷新额度
    let _ = internal_refresh_account_quota(&app_handle, &mut account).await;

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(
        app_handle.state::<crate::commands::proxy::ProxyServiceState>(),
    )
    .await;

    Ok(account)
}

/// 完成 OAuth 授权（不自动打开浏览器）
#[tauri::command]
pub async fn complete_oauth_login(
    app_handle: tauri::AppHandle,
    custom_label: Option<String>,
    proxy_id: Option<String>,
) -> Result<Account, String> {
    modules::logger::log_info("完成 OAuth 授权流程 (manual)...");
    let service = modules::account_service::AccountService::new(
        crate::modules::integration::SystemManager::Desktop(app_handle.clone()),
    );

    let mut account = service.complete_oauth_login().await?;

    // 应用 custom label 和 proxy binding
    apply_account_metadata(&app_handle, &mut account, custom_label, proxy_id).await?;

    // Auto-generate device fingerprint
    auto_generate_device_fingerprint(&account);

    // 自动触发刷新额度
    let _ = internal_refresh_account_quota(&app_handle, &mut account).await;

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(
        app_handle.state::<crate::commands::proxy::ProxyServiceState>(),
    )
    .await;

    Ok(account)
}

/// 预生成 OAuth 授权链接 (不打开浏览器)
#[tauri::command]
pub async fn prepare_oauth_url(app_handle: tauri::AppHandle) -> Result<String, String> {
    let service = modules::account_service::AccountService::new(
        crate::modules::integration::SystemManager::Desktop(app_handle.clone()),
    );
    service.prepare_oauth_url().await
}

#[tauri::command]
pub async fn cancel_oauth_login() -> Result<(), String> {
    modules::oauth_server::cancel_oauth_flow();
    Ok(())
}

/// 手动提交 OAuth Code (用于 Docker/远程环境无法自动回调时)
#[tauri::command]
pub async fn submit_oauth_code(code: String, state: Option<String>) -> Result<(), String> {
    modules::logger::log_info("收到手动提交 OAuth Code 请求");
    modules::oauth_server::submit_oauth_code(code, state).await
}

// --- 导入命令 ---

#[tauri::command]
pub async fn import_v1_accounts(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    custom_label: Option<String>,
    proxy_id: Option<String>,
) -> Result<Vec<Account>, String> {
    let accounts = modules::migration::import_from_v1().await?;

    // 对导入的账号应用 metadata（如果是单个账号）
    if accounts.len() == 1 {
        let mut account = accounts[0].clone();
        apply_account_metadata(&app, &mut account, custom_label, proxy_id).await?;
    }

    // Auto-generate device fingerprint for all imported accounts
    for account in &accounts {
        auto_generate_device_fingerprint(account);
    }

    // 对导入的账号尝试刷新一波
    for mut account in accounts.clone() {
        let _ = internal_refresh_account_quota(&app, &mut account).await;
    }

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    Ok(accounts)
}

#[tauri::command]
pub async fn import_from_db(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    custom_label: Option<String>,
    proxy_id: Option<String>,
) -> Result<Account, String> {
    // 同步函数包装为 async
    let mut account = modules::migration::import_from_db().await?;

    // 应用 custom label 和 proxy binding
    apply_account_metadata(&app, &mut account, custom_label, proxy_id).await?;

    // Auto-generate device fingerprint
    auto_generate_device_fingerprint(&account);

    // 既然是从数据库导入（即 IDE 当前账号），自动将其设为 Manager 的当前账号
    let account_id = account.id.clone();
    modules::account::set_current_account_id(&account_id)?;

    // 自动触发刷新额度
    let _ = internal_refresh_account_quota(&app, &mut account).await;

    // 刷新托盘图标展示
    crate::modules::tray::update_tray_menus(&app);

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    Ok(account)
}

#[tauri::command]
#[allow(dead_code)]
pub async fn import_custom_db(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    path: String,
) -> Result<Account, String> {
    // 调用重构后的自定义导入函数
    let mut account = modules::migration::import_from_custom_db_path(path).await?;

    // 自动设为当前账号
    let account_id = account.id.clone();
    modules::account::set_current_account_id(&account_id)?;

    // Auto-generate device fingerprint
    auto_generate_device_fingerprint(&account);

    // 自动触发刷新额度
    let _ = internal_refresh_account_quota(&app, &mut account).await;

    // 刷新托盘图标展示
    crate::modules::tray::update_tray_menus(&app);

    // Reload token pool
    let _ = crate::commands::proxy::reload_proxy_accounts(proxy_state).await;

    Ok(account)
}

#[tauri::command]
pub async fn sync_account_from_db(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
) -> Result<Option<Account>, String> {
    // 1. 获取 DB 中的 Refresh Token
    let db_refresh_token = match modules::migration::get_refresh_token_from_db() {
        Ok(token) => token,
        Err(e) => {
            modules::logger::log_info(&format!("自动同步跳过: {}", e));
            return Ok(None);
        }
    };

    // 2. 获取 Manager 当前账号
    let curr_account = modules::account::get_current_account()?;

    // 3. 对比：如果 Refresh Token 相同，说明账号没变，无需导入
    if let Some(acc) = curr_account {
        if acc.token.refresh_token == db_refresh_token {
            // 账号未变，由于已经是周期性任务，我们可以选择性刷新一下配额，或者直接返回
            // 这里为了节省 API 流量，直接返回
            return Ok(None);
        }
        modules::logger::log_info(&format!(
            "检测到账号切换 ({} -> DB新账号)，正在同步...",
            acc.email
        ));
    } else {
        modules::logger::log_info("检测到新登录账号，正在自动同步...");
    }

    // 4. 执行完整导入
    let account = import_from_db(app, proxy_state, None, None).await?;
    Ok(Some(account))
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.contains("..") {
        return Err("非法路径: 不允许目录遍历".to_string());
    }

    // 检查是否指向系统敏感路径 (基础黑名单)
    let lower_path = path.to_lowercase();
    let sensitive_prefixes = [
        "/etc/",
        "/var/spool/cron",
        "/root/",
        "/proc/",
        "/sys/",
        "/dev/",
        "c:\\windows",
        "c:\\users\\administrator",
        "c:\\pagefile.sys",
    ];

    for prefix in sensitive_prefixes {
        if lower_path.starts_with(prefix) {
            return Err(format!("安全拒绝: 禁止访问系统敏感路径 ({})", prefix));
        }
    }

    Ok(())
}

/// 保存文本文件 (绕过前端 Scope 限制)
#[tauri::command]
pub async fn save_text_file(path: String, content: String) -> Result<(), String> {
    validate_path(&path)?;
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {}", e))
}

/// 读取文本文件 (绕过前端 Scope 限制)
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    validate_path(&path)?;
    std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

/// 清理日志缓存
#[tauri::command]
pub async fn clear_log_cache() -> Result<(), String> {
    modules::logger::clear_logs()
}

/// 清理 Antigravity 应用缓存
/// 用于解决登录失败、版本验证错误等问题
#[tauri::command]
pub async fn clear_antigravity_cache() -> Result<modules::cache::ClearResult, String> {
    modules::cache::clear_antigravity_cache(None)
}

/// 获取 Antigravity 缓存路径列表（用于预览）
#[tauri::command]
pub async fn get_antigravity_cache_paths() -> Result<Vec<String>, String> {
    Ok(modules::cache::get_existing_cache_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// 打开数据目录
#[tauri::command]
pub async fn open_data_folder() -> Result<(), String> {
    let path = modules::account::get_data_dir()?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        use crate::utils::command::CommandExtWrapper;
        std::process::Command::new("explorer")
            .creation_flags_windows()
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    Ok(())
}

/// 获取数据目录绝对路径
#[tauri::command]
pub async fn get_data_dir_path() -> Result<String, String> {
    let path = modules::account::get_data_dir()?;
    Ok(path.to_string_lossy().to_string())
}

/// 显示主窗口
#[tauri::command]
pub async fn show_main_window(window: tauri::Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())
}

/// 设置窗口主题（用于同步 Windows 标题栏按钮颜色）
#[tauri::command]
pub async fn set_window_theme(window: tauri::Window, theme: String) -> Result<(), String> {
    use tauri::Theme;

    let tauri_theme = match theme.as_str() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None, // system default
    };

    window.set_theme(tauri_theme).map_err(|e| e.to_string())
}

/// 获取 Antigravity 可执行文件路径
#[tauri::command]
pub async fn get_antigravity_path(bypass_config: Option<bool>) -> Result<String, String> {
    // 1. 优先从配置查询 (除非明确要求绕过)
    if bypass_config != Some(true) {
        if let Ok(config) = crate::modules::config::load_app_config() {
            if let Some(path) = config.antigravity_executable {
                if std::path::Path::new(&path).exists() {
                    return Ok(path);
                }
            }
        }
    }

    // 2. 执行实时探测
    match crate::modules::process::get_antigravity_executable_path() {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Err("未找到 Antigravity 安装路径".to_string()),
    }
}

/// 获取 Antigravity 启动参数
#[tauri::command]
pub async fn get_antigravity_args() -> Result<Vec<String>, String> {
    match crate::modules::process::get_args_from_running_process() {
        Some(args) => Ok(args),
        None => Err("未找到正在运行的 Antigravity 进程".to_string()),
    }
}

/// 检测更新响应结构
pub use crate::modules::update_checker::UpdateInfo;

/// 检测 GitHub releases 更新
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    modules::logger::log_info("收到前端触发的更新检查请求");
    crate::modules::update_checker::check_for_updates().await
}

#[tauri::command]
pub async fn should_check_updates() -> Result<bool, String> {
    let settings = crate::modules::update_checker::load_update_settings()?;
    Ok(crate::modules::update_checker::should_check_for_updates(
        &settings,
    ))
}

#[tauri::command]
pub async fn update_last_check_time() -> Result<(), String> {
    crate::modules::update_checker::update_last_check_time()
}

/// 检测是否通过 Homebrew Cask 安装
#[tauri::command]
pub async fn check_homebrew_installation() -> Result<bool, String> {
    Ok(crate::modules::update_checker::is_homebrew_installed())
}

/// 通过 Homebrew Cask 升级应用
#[tauri::command]
pub async fn brew_upgrade_cask() -> Result<String, String> {
    modules::logger::log_info("收到前端触发的 Homebrew 升级请求");
    crate::modules::update_checker::brew_upgrade_cask().await
}

/// 获取更新设置
#[tauri::command]
pub async fn get_update_settings() -> Result<crate::modules::update_checker::UpdateSettings, String>
{
    crate::modules::update_checker::load_update_settings()
}

/// 保存更新设置
#[tauri::command]
pub async fn save_update_settings(
    settings: crate::modules::update_checker::UpdateSettings,
) -> Result<(), String> {
    crate::modules::update_checker::save_update_settings(&settings)
}

/// 切换账号的反代禁用状态
#[tauri::command]
pub async fn toggle_proxy_status(
    app: tauri::AppHandle,
    proxy_state: tauri::State<'_, crate::commands::proxy::ProxyServiceState>,
    account_id: String,
    enable: bool,
    reason: Option<String>,
) -> Result<(), String> {
    modules::logger::log_info(&format!(
        "切换账号反代状态: {} -> {}",
        account_id,
        if enable { "启用" } else { "禁用" }
    ));

    // 1. 读取账号文件
    let data_dir = modules::account::get_data_dir()?;
    let account_path = data_dir
        .join("accounts")
        .join(format!("{}.json", account_id));

    if !account_path.exists() {
        return Err(format!("账号文件不存在: {}", account_id));
    }

    let content =
        std::fs::read_to_string(&account_path).map_err(|e| format!("读取账号文件失败: {}", e))?;

    let mut account_json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析账号文件失败: {}", e))?;

    // 2. 更新 proxy_disabled 字段
    if enable {
        // 启用反代
        account_json["proxy_disabled"] = serde_json::Value::Bool(false);
        account_json["proxy_disabled_reason"] = serde_json::Value::Null;
        account_json["proxy_disabled_at"] = serde_json::Value::Null;
    } else {
        // 禁用反代
        let now = chrono::Utc::now().timestamp();
        account_json["proxy_disabled"] = serde_json::Value::Bool(true);
        account_json["proxy_disabled_at"] = serde_json::Value::Number(now.into());
        account_json["proxy_disabled_reason"] =
            serde_json::Value::String(reason.unwrap_or_else(|| "用户手动禁用".to_string()));
    }

    // 3. 保存到磁盘
    let json_str = serde_json::to_string_pretty(&account_json)
        .map_err(|e| format!("序列化账号数据失败: {}", e))?;
    std::fs::write(&account_path, json_str).map_err(|e| format!("写入账号文件失败: {}", e))?;

    modules::logger::log_info(&format!(
        "账号反代状态已更新: {} ({})",
        account_id,
        if enable { "已启用" } else { "已禁用" }
    ));

    // 4. 如果反代服务正在运行,立刻同步到内存池（避免禁用后仍被选中）
    {
        let instance_lock = proxy_state.instance.read().await;
        if let Some(instance) = instance_lock.as_ref() {
            // 如果禁用的是当前固定账号，则自动关闭固定模式（内存 + 配置持久化）
            if !enable {
                let pref_id = instance.token_manager.get_preferred_account().await;
                if pref_id.as_deref() == Some(&account_id) {
                    instance.token_manager.set_preferred_account(None).await;

                    if let Ok(mut cfg) = crate::modules::config::load_app_config() {
                        if cfg.proxy.preferred_account_id.as_deref() == Some(&account_id) {
                            cfg.proxy.preferred_account_id = None;
                            let _ = crate::modules::config::save_app_config(&cfg);
                        }
                    }
                }
            }

            instance
                .token_manager
                .reload_account(&account_id)
                .await
                .map_err(|e| format!("同步账号失败: {}", e))?;
        }
    }

    // 5. 更新托盘菜单
    crate::modules::tray::update_tray_menus(&app);

    Ok(())
}

/// 预热所有可用账号
#[tauri::command]
pub async fn warm_up_all_accounts() -> Result<String, String> {
    modules::quota::warm_up_all_accounts().await
}

/// 预热指定账号
#[tauri::command]
pub async fn warm_up_account(account_id: String) -> Result<String, String> {
    modules::quota::warm_up_account(&account_id).await
}

// --- Onboarding & Test Request ---

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OnboardingResult {
    pub success: bool,
    pub message: String,
    pub status: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TestRequestResult {
    pub success: bool,
    pub status: String,
    pub message: String,
    pub requires_verification: Option<bool>,
    pub verification_url: Option<String>,
    pub is_banned: Option<bool>,
    pub is_forbidden: Option<bool>,
    pub details: Option<String>,
}

/// Onboard account: initialize cloud code project, refresh token, verify access
#[tauri::command]
pub async fn onboard_account(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<OnboardingResult, String> {
    modules::logger::log_info(&format!("Starting onboarding for account: {}", account_id));

    let mut account = modules::load_account(&account_id)?;

    let token =
        match crate::modules::oauth::ensure_fresh_token(&account.token, Some(&account.id)).await {
            Ok(new_token) => {
                if new_token.access_token != account.token.access_token {
                    account.token = new_token.clone();
                    if let Err(error) = crate::modules::account::save_account(&account) {
                        let error_message = format!(
                            "Failed to save refreshed token during onboarding: {}",
                            error
                        );
                        modules::logger::log_warn(&format!(
                            "[Onboarding] Failed to save refreshed token: {}",
                            error
                        ));
                        return Ok(OnboardingResult {
                            success: false,
                            message: error_message.clone(),
                            status: Some("token_persistence_error".to_string()),
                            details: Some(error_message),
                        });
                    } else {
                        modules::logger::log_info(&format!(
                            "[Onboarding] Successfully refreshed and saved new token for {}",
                            account.email
                        ));
                    }
                }

                new_token.access_token
            }
            Err(error) => {
                return Ok(OnboardingResult {
                    success: false,
                    message: format!("Token refresh failed: {}", error),
                    status: Some("token_error".to_string()),
                    details: Some(error),
                });
            }
        };

    let resolved_project = match modules::quota::resolve_project_with_contract(
        &token,
        Some(&account.email),
        Some(&account.id),
    )
    .await
    {
        modules::quota::ProjectResolutionOutcome::Resolved(project) => project,
        modules::quota::ProjectResolutionOutcome::InProgressExhausted {
            subscription_tier, ..
        } => {
            return Ok(OnboardingResult {
                success: false,
                message: "Project acquisition did not finish before polling timed out".to_string(),
                status: Some("project_poll_exhausted".to_string()),
                details: Some(match subscription_tier {
                    Some(tier) => format!(
                        "onboardUser remained in progress after maximum polling attempts; subscription tier: {}",
                        tier
                    ),
                    None => {
                        "onboardUser remained in progress after maximum polling attempts"
                            .to_string()
                    }
                }),
            });
        }
        modules::quota::ProjectResolutionOutcome::TerminalMissingProject {
            stage,
            subscription_tier,
            restriction_reason,
            validation_url,
        } => {
            // Google accepted the request (done=true) but didn't return a project_id.
            // Use a random fallback — project_id is just a routing hint, auth is via Bearer token.
            let fallback = modules::quota::generate_fallback_project_id();
            modules::logger::log_warn(&format!(
                "[Onboarding] {} completed without project_id for {}, using fallback: {}",
                stage.as_str(),
                account.email,
                fallback
            ));
            modules::quota::ResolvedCloudProject {
                project_id: fallback,
                subscription_tier,
                restriction_reason,
                validation_url,
            }
        }
        modules::quota::ProjectResolutionOutcome::TransportFailure {
            stage,
            error,
            subscription_tier,
            ..
        } => {
            return Ok(OnboardingResult {
                success: false,
                message: format!(
                    "Project resolution transport failure during {}",
                    stage.as_str()
                ),
                status: Some("project_resolution_failed".to_string()),
                details: Some(match subscription_tier {
                    Some(tier) => format!(
                        "{} transport failure: {}; subscription tier: {}",
                        stage.as_str(),
                        error,
                        tier
                    ),
                    None => format!("{} transport failure: {}", stage.as_str(), error),
                }),
            });
        }
        modules::quota::ProjectResolutionOutcome::LoadHttpFailure {
            status,
            body_preview,
            subscription_tier,
            ..
        } => {
            return Ok(OnboardingResult {
                success: false,
                message: "Project resolution failed during loadCodeAssist".to_string(),
                status: Some("project_resolution_failed".to_string()),
                details: Some(match subscription_tier {
                    Some(tier) => format!(
                        "loadCodeAssist returned HTTP {}: {}; subscription tier: {}",
                        status, body_preview, tier
                    ),
                    None => format!("loadCodeAssist returned HTTP {}: {}", status, body_preview),
                }),
            });
        }
        modules::quota::ProjectResolutionOutcome::OnboardHttpFailure {
            status,
            body_preview,
            subscription_tier,
            ..
        } => {
            return Ok(OnboardingResult {
                success: false,
                message: "Project resolution failed during onboarding".to_string(),
                status: Some("project_resolution_failed".to_string()),
                details: Some(match subscription_tier {
                    Some(tier) => format!(
                        "onboardUser returned HTTP {}: {}; subscription tier: {}",
                        status, body_preview, tier
                    ),
                    None => format!("onboardUser returned HTTP {}: {}", status, body_preview),
                }),
            });
        }
        modules::quota::ProjectResolutionOutcome::ParseFailure {
            stage,
            error,
            subscription_tier,
            ..
        } => {
            return Ok(OnboardingResult {
                success: false,
                message: format!("Project resolution parse failure during {}", stage.as_str()),
                status: Some("project_resolution_failed".to_string()),
                details: Some(match subscription_tier {
                    Some(tier) => format!(
                        "{} parse failure: {}; subscription tier: {}",
                        stage.as_str(),
                        error,
                        tier
                    ),
                    None => format!("{} parse failure: {}", stage.as_str(), error),
                }),
            });
        }
    };

    match modules::quota::fetch_quota_with_cache(
        &token,
        &account.email,
        Some(&resolved_project.project_id),
        Some(&account_id),
    )
    .await
    {
        Ok((quota_data, _)) => {
            let _ = modules::update_account_quota(&account_id, quota_data.clone());

            if quota_data.is_forbidden {
                let validation_url = quota_data.validation_url.clone();
                let raw_error = quota_data.forbidden_reason.clone();
                let _ = crate::modules::account::mark_account_forbidden(
                    &account_id,
                    "Onboarding: 403 Forbidden",
                    validation_url.as_deref(),
                    raw_error.as_deref(),
                );
                Ok(OnboardingResult {
                    success: false,
                    message: "Account is forbidden (403)".to_string(),
                    status: Some("forbidden".to_string()),
                    details: Some(validation_url.map(|u| format!("The account has been denied access to the API. Verification URL: {}", u)).unwrap_or_else(|| "The account has been denied access to the API".to_string())),
                })
            } else {
                // Account is active — clear any previous forbidden/validation state
                let _ = crate::modules::account::clear_account_forbidden(&account_id);

                let model_count = quota_data.models.len();
                let tier = quota_data
                    .subscription_tier
                    .clone()
                    .or_else(|| resolved_project.subscription_tier.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                modules::logger::log_info(&format!(
                    "Onboarding completed for {}: {} models, tier: {}",
                    account.email, model_count, tier
                ));

                // [FIX #XXXX] Reload proxy token manager to use fresh tokens after onboarding
                let proxy_state = app.state::<crate::commands::proxy::ProxyServiceState>();
                if let Err(e) = crate::commands::proxy::reload_proxy_accounts(proxy_state).await {
                    modules::logger::log_warn(&format!(
                        "[Onboarding] Failed to reload proxy accounts after token refresh: {}",
                        e
                    ));
                }

                Ok(OnboardingResult {
                    success: true,
                    message: format!(
                        "Onboarding completed. {} models available. Tier: {}",
                        model_count, tier
                    ),
                    status: Some("active".to_string()),
                    details: Some(format!(
                        "Project ID: {}, Subscription: {}",
                        resolved_project.project_id, tier
                    )),
                })
            }
        }
        Err(e) => {
            let error_str = format!("{}", e);
            Ok(OnboardingResult {
                success: false,
                message: format!("Onboarding failed: {}", error_str),
                status: Some("error".to_string()),
                details: Some(error_str),
            })
        }
    }
}

/// Test account with a live API request to detect status (active, forbidden, banned, etc.)
#[tauri::command]
pub async fn test_account_request(account_id: String) -> Result<TestRequestResult, String> {
    modules::logger::log_info(&format!(
        "Starting test request for account: {}",
        account_id
    ));

    let account = modules::load_account(&account_id)?;

    // Step 1: Refresh token and fetch project_id
    let (token, project_id) = match modules::quota::get_valid_token_for_warmup(&account).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(TestRequestResult {
                success: false,
                status: "token_error".to_string(),
                message: format!("Token refresh failed: {}", e),
                requires_verification: None,
                verification_url: None,
                is_banned: None,
                is_forbidden: None,
                details: Some(e),
            });
        }
    };

    // Step 2: Fetch quota as a lightweight live test (no tokens consumed)
    match modules::quota::fetch_quota(&token, &account.email, Some(&account_id)).await {
        Ok((quota_data, _)) => {
            let _ = modules::update_account_quota(&account_id, quota_data.clone());

            if quota_data.is_forbidden {
                let validation_url = quota_data.validation_url.clone();
                let raw_error = quota_data.forbidden_reason.clone();
                let _ = crate::modules::account::mark_account_forbidden(
                    &account_id,
                    "Test request: 403 Forbidden",
                    validation_url.as_deref(),
                    raw_error.as_deref(),
                );
                Ok(TestRequestResult {
                    success: false,
                    status: "forbidden".to_string(),
                    message: "Account access denied (403 Forbidden)".to_string(),
                    requires_verification: Some(validation_url.is_some()),
                    verification_url: validation_url,
                    is_banned: None,
                    is_forbidden: Some(true),
                    details: Some(
                        "The account has been denied access. This may indicate the account is banned or requires verification.".to_string(),
                    ),
                })
            } else {
                let model_count = quota_data.models.len();
                let tier = quota_data
                    .subscription_tier
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                // Check if account is restricted (has restriction_reason)
                if let Some(ref reason) = quota_data.restriction_reason {
                    // Extract validation URL from quota_data if available
                    let verification_url = quota_data.validation_url.clone();
                    let raw_error = quota_data.forbidden_reason.clone();

                    let _ = crate::modules::account::mark_account_forbidden(
                        &account_id,
                        &format!("Restricted: {}", reason),
                        verification_url.as_deref(),
                        raw_error.as_deref(),
                    );
                    Ok(TestRequestResult {
                        success: false,
                        status: "restricted".to_string(),
                        message: format!("Account is restricted: {}", reason),
                        requires_verification: Some(verification_url.is_some()),
                        verification_url,
                        is_banned: Some(false),
                        is_forbidden: Some(true),
                        details: Some(format!(
                            "Project: {}, Tier: {}, Models: {}, Reason: {}",
                            project_id, tier, model_count, reason
                        )),
                    })
                } else {
                    // Account is active — clear any previous forbidden/validation state
                    let _ = crate::modules::account::clear_account_forbidden(&account_id);

                    modules::logger::log_info(&format!(
                        "Test request successful for {}: {} models, tier: {}",
                        account.email, model_count, tier
                    ));

                    Ok(TestRequestResult {
                        success: true,
                        status: "active".to_string(),
                        message: format!(
                            "Account is active. {} models available. Tier: {}",
                            model_count, tier
                        ),
                        requires_verification: Some(false),
                        verification_url: None,
                        is_banned: Some(false),
                        is_forbidden: Some(false),
                        details: Some(format!(
                            "Project: {}, Tier: {}, Models: {}",
                            project_id, tier, model_count
                        )),
                    })
                }
            }
        }
        Err(e) => {
            let error_str = format!("{}", e);

            // Detect specific error conditions from response text
            let is_banned = error_str.contains("banned") || error_str.contains("suspended");
            let requires_verification =
                error_str.contains("verification") || error_str.contains("verify");
            let is_forbidden = error_str.contains("403") || error_str.contains("Forbidden");

            Ok(TestRequestResult {
                success: false,
                status: if is_banned {
                    "banned".to_string()
                } else if is_forbidden {
                    "forbidden".to_string()
                } else if requires_verification {
                    "verification_required".to_string()
                } else {
                    "error".to_string()
                },
                message: format!("Test request failed: {}", error_str),
                requires_verification: Some(requires_verification),
                verification_url: None,
                is_banned: Some(is_banned),
                is_forbidden: Some(is_forbidden),
                details: Some(error_str),
            })
        }
    }
}

/// 更新账号自定义标签
#[tauri::command]
pub async fn update_account_label(account_id: String, label: String) -> Result<(), String> {
    // 验证标签长度（按字符数计算，支持中文）
    if label.chars().count() > 15 {
        return Err("标签长度不能超过15个字符".to_string());
    }

    modules::logger::log_info(&format!(
        "更新账号标签: {} -> {:?}",
        account_id,
        if label.is_empty() { "无" } else { &label }
    ));

    // 1. 读取账号文件
    let data_dir = modules::account::get_data_dir()?;
    let account_path = data_dir
        .join("accounts")
        .join(format!("{}.json", account_id));

    if !account_path.exists() {
        return Err(format!("账号文件不存在: {}", account_id));
    }

    let content =
        std::fs::read_to_string(&account_path).map_err(|e| format!("读取账号文件失败: {}", e))?;

    let mut account_json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("解析账号文件失败: {}", e))?;

    // 2. 更新 custom_label 字段
    if label.is_empty() {
        account_json["custom_label"] = serde_json::Value::Null;
    } else {
        account_json["custom_label"] = serde_json::Value::String(label.clone());
    }

    // 3. 保存到磁盘
    let json_str = serde_json::to_string_pretty(&account_json)
        .map_err(|e| format!("序列化账号数据失败: {}", e))?;
    std::fs::write(&account_path, json_str).map_err(|e| format!("写入账号文件失败: {}", e))?;

    modules::logger::log_info(&format!(
        "账号标签已更新: {} ({})",
        account_id,
        if label.is_empty() {
            "已清除".to_string()
        } else {
            label
        }
    ));

    Ok(())
}

// ============================================================================
// HTTP API 设置命令
// ============================================================================

/// 获取 HTTP API 设置
#[tauri::command]
pub async fn get_http_api_settings() -> Result<crate::modules::http_api::HttpApiSettings, String> {
    crate::modules::http_api::load_settings()
}

/// 保存 HTTP API 设置
#[tauri::command]
pub async fn save_http_api_settings(
    settings: crate::modules::http_api::HttpApiSettings,
) -> Result<(), String> {
    crate::modules::http_api::save_settings(&settings)
}

// ============================================================================
// Token Statistics Commands
// ============================================================================

pub use crate::modules::token_stats::{AccountTokenStats, TokenStatsAggregated, TokenStatsSummary};

#[tauri::command]
pub async fn get_token_stats_hourly(hours: i64) -> Result<Vec<TokenStatsAggregated>, String> {
    crate::modules::token_stats::get_hourly_stats(hours)
}

#[tauri::command]
pub async fn get_token_stats_daily(days: i64) -> Result<Vec<TokenStatsAggregated>, String> {
    crate::modules::token_stats::get_daily_stats(days)
}

#[tauri::command]
pub async fn get_token_stats_weekly(weeks: i64) -> Result<Vec<TokenStatsAggregated>, String> {
    crate::modules::token_stats::get_weekly_stats(weeks)
}

#[tauri::command]
pub async fn get_token_stats_by_account(hours: i64) -> Result<Vec<AccountTokenStats>, String> {
    crate::modules::token_stats::get_account_stats(hours)
}

#[tauri::command]
pub async fn get_token_stats_summary(hours: i64) -> Result<TokenStatsSummary, String> {
    crate::modules::token_stats::get_summary_stats(hours)
}

#[tauri::command]
pub async fn get_token_stats_by_model(
    hours: i64,
) -> Result<Vec<crate::modules::token_stats::ModelTokenStats>, String> {
    crate::modules::token_stats::get_model_stats(hours)
}

#[tauri::command]
pub async fn get_token_stats_model_trend_hourly(
    hours: i64,
) -> Result<Vec<crate::modules::token_stats::ModelTrendPoint>, String> {
    crate::modules::token_stats::get_model_trend_hourly(hours)
}

#[tauri::command]
pub async fn get_token_stats_model_trend_daily(
    days: i64,
) -> Result<Vec<crate::modules::token_stats::ModelTrendPoint>, String> {
    crate::modules::token_stats::get_model_trend_daily(days)
}

#[tauri::command]
pub async fn get_token_stats_account_trend_hourly(
    hours: i64,
) -> Result<Vec<crate::modules::token_stats::AccountTrendPoint>, String> {
    crate::modules::token_stats::get_account_trend_hourly(hours)
}

#[tauri::command]
pub async fn get_token_stats_account_trend_daily(
    days: i64,
) -> Result<Vec<crate::modules::token_stats::AccountTrendPoint>, String> {
    crate::modules::token_stats::get_account_trend_daily(days)
}
