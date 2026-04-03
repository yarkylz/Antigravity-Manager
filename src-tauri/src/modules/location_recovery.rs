use crate::modules::{account, config, logger, quota};
use futures::stream::{self, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{self, Duration};

/// Global flag: is the recovery loop currently running a check cycle?
static IS_RUNNING: AtomicBool = AtomicBool::new(false);

/// Start the background location recovery loop.
///
/// Every `interval_minutes` (from config), the loop:
/// 1. Lists all accounts with `location_blocked == true`
/// 2. For each, refreshes the token and pings the quota API
/// 3. If the account is no longer forbidden/location-blocked, clears the state
///
/// Only the quota API is used (lightweight, no content generation).
pub fn start_location_recovery_loop(app_handle: Option<tauri::AppHandle>) {
    tauri::async_runtime::spawn(async move {
        logger::log_info("[LocationRecovery] Background recovery loop started.");

        // Initial delay: wait 60s after startup before first check
        time::sleep(Duration::from_secs(60)).await;

        loop {
            // Load config each iteration to pick up changes
            let interval_secs = match config::load_app_config() {
                Ok(cfg) => {
                    if !cfg.location_recovery.enabled {
                        // Disabled — sleep 60s then re-check config
                        time::sleep(Duration::from_secs(60)).await;
                        continue;
                    }
                    (cfg.location_recovery.interval_minutes as u64) * 60
                }
                Err(_) => {
                    // Config load failed — retry in 60s
                    time::sleep(Duration::from_secs(60)).await;
                    continue;
                }
            };

            // Run one recovery cycle
            run_recovery_cycle(app_handle.clone()).await;

            // Sleep for the configured interval
            time::sleep(Duration::from_secs(interval_secs)).await;
        }
    });
}

/// Run a single recovery cycle — can be triggered manually or by the loop.
pub async fn run_recovery_cycle(app_handle: Option<tauri::AppHandle>) {
    // Prevent concurrent runs
    if IS_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        logger::log_info("[LocationRecovery] Skipping: another cycle is already running.");
        return;
    }

    let result = run_recovery_cycle_inner(app_handle).await;

    IS_RUNNING.store(false, Ordering::SeqCst);

    if let Err(e) = result {
        logger::log_warn(&format!("[LocationRecovery] Cycle failed: {}", e));
    }
}

async fn run_recovery_cycle_inner(
    app_handle: Option<tauri::AppHandle>,
) -> Result<(), String> {
    let accounts = account::list_accounts()?;

    // Filter to location-blocked accounts that are not disabled (disabled = invalid_grant, can't make requests)
    let blocked: Vec<_> = accounts
        .into_iter()
        .filter(|a| a.location_blocked && !a.disabled)
        .collect();

    if blocked.is_empty() {
        // Nothing to recover — silent return
        return Ok(());
    }

    logger::log_info(&format!(
        "[LocationRecovery] Found {} location-blocked accounts, starting recovery check...",
        blocked.len()
    ));

    // Process up to 5 accounts concurrently
    let results: Vec<(String, String, RecoveryOutcome)> = stream::iter(blocked)
        .map(|account| async move {
            let id = account.id.clone();
            let email = account.email.clone();
            let outcome = check_single_account(&account).await;
            (id, email, outcome)
        })
        .buffer_unordered(5)
        .collect()
        .await;

    let mut recovered = 0u32;
    let mut still_blocked = 0u32;
    let mut errors = 0u32;

    for (account_id, email, outcome) in &results {
        match outcome {
            RecoveryOutcome::Recovered => {
                recovered += 1;
                logger::log_info(&format!(
                    "[LocationRecovery] \u{2705} Account {} ({}) is no longer location-blocked!",
                    email, account_id
                ));
                // Clear the forbidden/location state
                let _ = account::clear_account_forbidden(account_id);
            }
            RecoveryOutcome::StillBlocked => {
                still_blocked += 1;
            }
            RecoveryOutcome::Error(e) => {
                errors += 1;
                logger::log_warn(&format!(
                    "[LocationRecovery] Error checking {}: {}",
                    email, e
                ));
            }
        }
    }

    logger::log_info(&format!(
        "[LocationRecovery] Cycle complete: {} recovered, {} still blocked, {} errors",
        recovered, still_blocked, errors
    ));

    // If any accounts were recovered, emit a refresh event to the frontend
    if recovered > 0 {
        if let Some(handle) = app_handle {
            use tauri::Emitter;
            let _ = handle.emit("accounts://refreshed", ());
        }
    }

    Ok(())
}

enum RecoveryOutcome {
    Recovered,
    StillBlocked,
    Error(String),
}

/// Check a single location-blocked account by pinging the quota API.
async fn check_single_account(
    account: &crate::models::Account,
) -> RecoveryOutcome {
    // Step 1: Get a valid (refreshed) token
    let (token, _project_id) = match quota::get_valid_token_for_warmup(account).await {
        Ok(t) => t,
        Err(e) => {
            // Token refresh failure (e.g. invalid_grant) — not recoverable this cycle
            return RecoveryOutcome::Error(format!("Token refresh failed: {}", e));
        }
    };

    // Step 2: Fetch quota — this is the lightweight "ping" to check location status
    match quota::fetch_quota(&token, &account.email, Some(&account.id)).await {
        Ok((quota_data, _)) => {
            if quota_data.is_forbidden {
                // Still forbidden — check if it's still a location block
                let reason_lower = quota_data
                    .forbidden_reason
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase();
                let restriction = quota_data
                    .restriction_reason
                    .as_deref()
                    .unwrap_or("");

                let is_still_location = reason_lower.contains("not currently available in your location")
                    || restriction == "UNSUPPORTED_LOCATION";

                if is_still_location {
                    RecoveryOutcome::StillBlocked
                } else {
                    // Forbidden for a different reason now — don't clear location_blocked,
                    // let the normal 403 flow handle it
                    RecoveryOutcome::StillBlocked
                }
            } else {
                // Not forbidden anymore — account is recovered!
                // Update quota data on disk
                let _ = crate::modules::update_account_quota(&account.id, quota_data);
                RecoveryOutcome::Recovered
            }
        }
        Err(e) => {
            RecoveryOutcome::Error(format!("Quota fetch failed: {}", e))
        }
    }
}

/// Check if a recovery cycle is currently running.
pub fn is_running() -> bool {
    IS_RUNNING.load(Ordering::SeqCst)
}
