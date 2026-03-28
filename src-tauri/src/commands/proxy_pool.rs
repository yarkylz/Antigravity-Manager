use crate::commands::proxy::ProxyServiceState;
use crate::proxy::proxy_pool;
use std::collections::HashMap;
use tauri::State;

/// Bind an account to a specific proxy
/// [FIX] Use global proxy pool to ensure consistency between UI commands and account operations
#[tauri::command]
pub async fn bind_account_proxy(
    state: State<'_, ProxyServiceState>,
    #[allow(non_snake_case)] accountId: String,
    #[allow(non_snake_case)] proxyId: String,
) -> Result<(), String> {
    tracing::info!(
        "[Bind] bind_account_proxy called: accountId={}, proxyId={}",
        accountId,
        proxyId
    );

    // Use global proxy pool for consistency
    if let Some(pool) = proxy_pool::get_global_proxy_pool() {
        tracing::info!("[Bind] Using global proxy pool");
        pool.bind_account_to_proxy(accountId, proxyId).await
    } else {
        tracing::warn!("[Bind] Global proxy pool not initialized, checking instance...");
        // Fallback to instance pool if global not initialized
        let instance_lock = state.instance.read().await;
        if let Some(instance) = instance_lock.as_ref() {
            tracing::info!("[Bind] Using instance proxy pool");
            instance
                .axum_server
                .proxy_pool_manager
                .bind_account_to_proxy(accountId, proxyId)
                .await
        } else {
            tracing::error!("[Bind] No proxy pool available!");
            Err("Proxy service not running. Please start the proxy service first.".to_string())
        }
    }
}

/// Unbind an account from its proxy
/// [FIX] Use global proxy pool to ensure consistency between UI commands and account operations
#[tauri::command]
pub async fn unbind_account_proxy(
    state: State<'_, ProxyServiceState>,
    #[allow(non_snake_case)] accountId: String,
) -> Result<(), String> {
    // Use global proxy pool for consistency
    if let Some(pool) = proxy_pool::get_global_proxy_pool() {
        pool.unbind_account_proxy(accountId).await;
        Ok(())
    } else {
        // Fallback to instance pool if global not initialized
        let instance_lock = state.instance.read().await;
        if let Some(instance) = instance_lock.as_ref() {
            instance
                .axum_server
                .proxy_pool_manager
                .unbind_account_proxy(accountId)
                .await;
            Ok(())
        } else {
            Err("Service not running".to_string())
        }
    }
}

/// Get the proxy binding for a specific account
/// [FIX] Use global proxy pool to ensure consistency between UI commands and account operations
#[tauri::command]
pub async fn get_account_proxy_binding(
    state: State<'_, ProxyServiceState>,
    account_id: String,
) -> Result<Option<String>, String> {
    // Use global proxy pool for consistency
    if let Some(pool) = proxy_pool::get_global_proxy_pool() {
        Ok(pool.get_account_binding(&account_id))
    } else {
        // Fallback to instance pool if global not initialized
        let instance_lock = state.instance.read().await;
        if let Some(instance) = instance_lock.as_ref() {
            Ok(instance
                .axum_server
                .proxy_pool_manager
                .get_account_binding(&account_id))
        } else {
            Err("Service not running".to_string())
        }
    }
}

/// Get all account proxy bindings
/// [FIX] Use global proxy pool to ensure consistency between UI commands and account operations
#[tauri::command]
pub async fn get_all_account_bindings(
    state: State<'_, ProxyServiceState>,
) -> Result<HashMap<String, String>, String> {
    // Use global proxy pool for consistency
    if let Some(pool) = proxy_pool::get_global_proxy_pool() {
        Ok(pool.get_all_bindings_snapshot())
    } else {
        // Fallback to instance pool if global not initialized
        let instance_lock = state.instance.read().await;
        if let Some(instance) = instance_lock.as_ref() {
            Ok(instance
                .axum_server
                .proxy_pool_manager
                .get_all_bindings_snapshot())
        } else {
            Err("Service not running".to_string())
        }
    }
}
