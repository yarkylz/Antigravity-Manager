use crate::modules;
use crate::modules::oauth;
use std::sync::{Mutex, OnceLock};
use tauri::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::watch;

struct OAuthFlowState {
    auth_url: String,
    #[allow(dead_code)]
    redirect_uri: String,
    state: String,
    cancel_tx: watch::Sender<bool>,
    code_tx: mpsc::Sender<Result<oauth::TokenResponse, String>>,
    code_rx: Option<mpsc::Receiver<Result<oauth::TokenResponse, String>>>,
}

static OAUTH_FLOW_STATE: OnceLock<Mutex<Option<OAuthFlowState>>> = OnceLock::new();

fn get_oauth_flow_state() -> &'static Mutex<Option<OAuthFlowState>> {
    OAUTH_FLOW_STATE.get_or_init(|| Mutex::new(None))
}

fn oauth_fail_html() -> &'static str {
    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
    <html>\
    <body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
    <h1 style='color: red;'>❌ Authorization Failed</h1>\
    <p>Failed to obtain Authorization Code. Please return to the app and try again.</p>\
    </body>\
    </html>"
}

/// Outcome of the full OAuth callback processing (exchange → save → onboard → test).
enum CallbackOutcome {
    /// Account active and working
    Success {
        email: String,
        tier: String,
        model_count: usize,
    },
    /// Account requires verification (e.g. phone/identity)
    Verification {
        email: String,
        verification_url: String,
        message: String,
    },
    /// Account is restricted/forbidden/banned but no verification URL
    Restricted { email: String, message: String },
    /// Something failed during processing
    Error { message: String },
}

fn build_callback_html(outcome: &CallbackOutcome) -> String {
    let body = match outcome {
        CallbackOutcome::Success {
            email,
            tier,
            model_count,
        } => {
            format!(
                "<h1 style='color: #22c55e;'>✅ Authorization Successful!</h1>\
                 <p style='font-size: 1.1em;'>Account: <strong>{}</strong></p>\
                 <p>Tier: <strong>{}</strong> · {} models available</p>\
                 <p style='color: #888; margin-top: 24px;'>You can close this window and return to the application.</p>\
                 <script>setTimeout(function() {{ window.close(); }}, 3000);</script>",
                email, tier, model_count
            )
        }
        CallbackOutcome::Verification {
            email,
            verification_url,
            message,
        } => {
            format!(
                "<h1 style='color: #f59e0b;'>⚠️ Verification Required</h1>\
                 <p style='font-size: 1.1em;'>Account: <strong>{}</strong></p>\
                 <p>{}</p>\
                 <a href='{}' target='_blank' rel='noopener' \
                    style='display: inline-block; margin-top: 16px; padding: 12px 28px; \
                           background: #3b82f6; color: white; text-decoration: none; \
                           border-radius: 8px; font-size: 1.05em;'>\
                    Complete Verification →\
                 </a>\
                 <p style='color: #888; margin-top: 24px;'>After verification, return to the app and test the account again.</p>",
                email, message, verification_url
            )
        }
        CallbackOutcome::Restricted { email, message } => {
            format!(
                "<h1 style='color: #ef4444;'>❌ Account Restricted</h1>\
                 <p style='font-size: 1.1em;'>Account: <strong>{}</strong></p>\
                 <p>{}</p>\
                 <p style='color: #888; margin-top: 24px;'>Please return to the application for details.</p>",
                email, message
            )
        }
        CallbackOutcome::Error { message } => {
            format!(
                "<h1 style='color: #ef4444;'>❌ Authorization Error</h1>\
                 <p>{}</p>\
                 <p style='color: #888; margin-top: 24px;'>Please return to the app and try again.</p>",
                message
            )
        }
    };

    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
         <html>\
         <body style='font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, sans-serif; \
                       text-align: center; padding: 50px; max-width: 600px; margin: 0 auto;'>\
         {}\
         </body>\
         </html>",
        body
    )
}

/// Run the full post-callback flow: exchange code → save account → onboard → test.
/// Returns dynamic HTML to display in the browser.
async fn process_oauth_callback(
    code: &str,
    redirect_uri: &str,
    _app_handle: Option<tauri::AppHandle>,
) -> (CallbackOutcome, Result<oauth::TokenResponse, String>) {
    // Step 1: Exchange authorization code for tokens
    let token_res = match oauth::exchange_code(code, redirect_uri).await {
        Ok(t) => t,
        Err(e) => {
            return (
                CallbackOutcome::Error {
                    message: format!("Token exchange failed: {}", e),
                },
                Err(e),
            );
        }
    };

    let refresh_token = match &token_res.refresh_token {
        Some(rt) => rt.clone(),
        None => {
            return (
                CallbackOutcome::Error {
                    message: "No Refresh Token received. Please revoke access and try again."
                        .to_string(),
                },
                Err("No refresh token".to_string()),
            );
        }
    };

    // Step 2: Get user info
    let temp_account_id = uuid::Uuid::new_v4().to_string();
    let user_info = match modules::oauth::get_user_info(
        &token_res.access_token,
        Some(&temp_account_id),
    )
    .await
    {
        Ok(info) => info,
        Err(e) => {
            return (
                CallbackOutcome::Error {
                    message: format!("Failed to get user info: {}", e),
                },
                Err(e),
            );
        }
    };

    let email = user_info.email.clone();

    // Step 3: Fetch project ID
    let project_id = crate::proxy::project_resolver::fetch_project_id(&token_res.access_token)
        .await
        .ok();

    // Step 4: Save account
    let token_data = crate::models::TokenData::new(
        token_res.access_token.clone(),
        refresh_token,
        token_res.expires_in,
        Some(email.clone()),
        project_id,
        None,
    );

    let account =
        match modules::upsert_account(email.clone(), user_info.get_display_name(), token_data) {
            Ok(a) => a,
            Err(e) => {
                return (
                    CallbackOutcome::Error {
                        message: format!("Failed to save account: {}", e),
                    },
                    Err(e),
                );
            }
        };

    let account_id = account.id.clone();

    modules::logger::log_info(&format!(
        "[OAuthCallback] Account saved: {}, starting onboard+test",
        email
    ));

    // Step 5: Onboard — refresh token + resolve cloud project
    let token = match modules::oauth::ensure_fresh_token(&account.token, Some(&account_id)).await {
        Ok(new_token) => {
            if new_token.access_token != account.token.access_token {
                let mut updated = account.clone();
                updated.token = new_token.clone();
                let _ = modules::account::save_account(&updated);
            }
            new_token.access_token
        }
        Err(e) => {
            return (
                CallbackOutcome::Error {
                    message: format!("Token refresh failed during onboarding: {}", e),
                },
                Ok(token_res.clone()),
            );
        }
    };

    // Step 6: Test — fetch quota to determine account status
    match modules::quota::fetch_quota(&token, &email, Some(&account_id)).await {
        Ok((quota_data, _)) => {
            let _ = modules::update_account_quota(&account_id, quota_data.clone());

            if quota_data.is_forbidden {
                let validation_url = quota_data.validation_url.clone();
                let raw_error = quota_data.forbidden_reason.clone();
                let _ = modules::account::mark_account_forbidden(
                    &account_id,
                    "OAuth callback: 403 Forbidden",
                    validation_url.as_deref(),
                    raw_error.as_deref(),
                );

                if let Some(url) = validation_url {
                    (
                        CallbackOutcome::Verification {
                            email,
                            verification_url: url,
                            message:
                                "Account access denied (403 Forbidden). Verification required."
                                    .to_string(),
                        },
                        Ok(token_res.clone()),
                    )
                } else {
                    (
                        CallbackOutcome::Restricted {
                            email,
                            message: "Account access denied (403 Forbidden).".to_string(),
                        },
                        Ok(token_res.clone()),
                    )
                }
            } else if let Some(ref reason) = quota_data.restriction_reason {
                let validation_url = quota_data.validation_url.clone();
                let raw_error = quota_data.forbidden_reason.clone();
                let _ = modules::account::mark_account_forbidden(
                    &account_id,
                    &format!("Restricted: {}", reason),
                    validation_url.as_deref(),
                    raw_error.as_deref(),
                );

                if let Some(url) = validation_url {
                    (
                        CallbackOutcome::Verification {
                            email,
                            verification_url: url,
                            message: format!("Account is restricted: {}", reason),
                        },
                        Ok(token_res.clone()),
                    )
                } else {
                    (
                        CallbackOutcome::Restricted {
                            email,
                            message: format!("Account is restricted: {}", reason),
                        },
                        Ok(token_res.clone()),
                    )
                }
            } else {
                let _ = modules::account::clear_account_forbidden(&account_id);
                let model_count = quota_data.models.len();
                let tier = quota_data
                    .subscription_tier
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                (
                    CallbackOutcome::Success {
                        email,
                        tier,
                        model_count,
                    },
                    Ok(token_res.clone()),
                )
            }
        }
        Err(e) => {
            // Quota fetch failed — account was saved, but we can't determine status.
            // Still a partial success.
            modules::logger::log_warn(&format!(
                "[OAuthCallback] Quota fetch failed for {}: {}",
                email, e
            ));
            (
                CallbackOutcome::Success {
                    email,
                    tier: "Unknown".to_string(),
                    model_count: 0,
                },
                Ok(token_res.clone()),
            )
        }
    }
}

async fn ensure_oauth_flow_prepared(
    app_handle: Option<tauri::AppHandle>,
) -> Result<String, String> {
    // Return URL if flow already exists and is still "fresh" (receiver hasn't been taken)
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        if let Some(s) = state.as_mut() {
            if s.code_rx.is_some() {
                return Ok(s.auth_url.clone());
            } else {
                // Flow is already "in progress" (rx taken), but user requested a NEW one.
                // Force cancel the old one to allow a new attempt.
                let _ = s.cancel_tx.send(true);
                *state = None;
            }
        }
    }

    // Create loopback listeners.
    // Some browsers resolve `localhost` to IPv6 (::1). To avoid "localhost refused connection",
    // we try to listen on BOTH IPv6 and IPv4 with the same port when possible.
    let mut ipv4_listener: Option<TcpListener> = None;
    let mut ipv6_listener: Option<TcpListener> = None;

    // Prefer creating one listener on an ephemeral port first, then bind the other stack to same port.
    // If both are available -> use `http://localhost:<port>` as redirect URI.
    // If only one is available -> use an explicit IP to force correct stack.
    let port: u16;
    match TcpListener::bind("[::1]:0").await {
        Ok(l6) => {
            port = l6
                .local_addr()
                .map_err(|e| format!("failed_to_get_local_port: {}", e))?
                .port();
            ipv6_listener = Some(l6);

            match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                Ok(l4) => ipv4_listener = Some(l4),
                Err(e) => {
                    crate::modules::logger::log_warn(&format!(
                        "failed_to_bind_ipv4_callback_port_127_0_0_1:{} (will only listen on IPv6): {}",
                        port, e
                    ));
                }
            }
        }
        Err(_) => {
            let l4 = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| format!("failed_to_bind_local_port: {}", e))?;
            port = l4
                .local_addr()
                .map_err(|e| format!("failed_to_get_local_port: {}", e))?
                .port();
            ipv4_listener = Some(l4);

            match TcpListener::bind(format!("[::1]:{}", port)).await {
                Ok(l6) => ipv6_listener = Some(l6),
                Err(e) => {
                    crate::modules::logger::log_warn(&format!(
                        "failed_to_bind_ipv6_callback_port_::1:{} (will only listen on IPv4): {}",
                        port, e
                    ));
                }
            }
        }
    }

    let has_ipv4 = ipv4_listener.is_some();
    let has_ipv6 = ipv6_listener.is_some();

    let redirect_uri = if has_ipv4 && has_ipv6 {
        format!("http://localhost:{}/oauth-callback", port)
    } else if has_ipv4 {
        format!("http://127.0.0.1:{}/oauth-callback", port)
    } else {
        format!("http://[::1]:{}/oauth-callback", port)
    };

    let state_str = uuid::Uuid::new_v4().to_string();
    let auth_url = oauth::get_auth_url(&redirect_uri, &state_str);

    // Cancellation signal (supports multiple consumers)
    let (cancel_tx, cancel_rx) = watch::channel(false);
    // Use mpsc instead of oneshot to allow multiple senders (listener OR manual input)
    let (code_tx, code_rx) = mpsc::channel::<Result<String, String>>(1);

    // Start listeners immediately: even if the user authorizes before clicking "Start OAuth",
    // the browser can still hit our callback and finish the flow.
    let app_handle_for_tasks = app_handle.clone();

    if let Some(l4) = ipv4_listener {
        let tx = code_tx.clone();
        let mut rx = cancel_rx.clone();
        let app_handle = app_handle_for_tasks.clone();
        let redir = redirect_uri.clone();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = tokio::select! {
                res = l4.accept() => res.map_err(|e| format!("failed_to_accept_connection: {}", e)),
                _ = rx.changed() => Err("OAuth cancelled".to_string()),
            } {
                let mut buffer = [0u8; 4096];
                let bytes_read = stream.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);

                // [FIX #931/850/778] More robust parsing and detailed logging
                let query_params = request
                    .lines()
                    .next()
                    .and_then(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            Some(parts[1])
                        } else {
                            None
                        }
                    })
                    .and_then(|path| Url::parse(&format!("http://localhost{}", path)).ok())
                    .map(|url| {
                        let mut code = None;
                        let mut state = None;
                        for (k, v) in url.query_pairs() {
                            if k == "code" {
                                code = Some(v.to_string());
                            } else if k == "state" {
                                state = Some(v.to_string());
                            }
                        }
                        (code, state)
                    });

                let (code, received_state) = match query_params {
                    Some((c, s)) => (c, s),
                    None => (None, None),
                };

                if code.is_none() && bytes_read > 0 {
                    crate::modules::logger::log_error(&format!(
                        "OAuth callback failed to parse code. Raw request (first 512 bytes): {}",
                        &request.chars().take(512).collect::<String>()
                    ));
                }

                // Verify state
                let state_valid = {
                    if let Ok(lock) = get_oauth_flow_state().lock() {
                        if let Some(s) = lock.as_ref() {
                            received_state.as_ref() == Some(&s.state)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                let (result, response_html) = match (code, state_valid) {
                    (Some(code), true) => {
                        crate::modules::logger::log_info(
                            "Successfully captured OAuth code from IPv4 listener, processing full flow...",
                        );
                        // Run the full flow (exchange → save → onboard → test) before responding
                        let (outcome, code_result) =
                            process_oauth_callback(&code, &redir, app_handle.clone()).await;
                        (code_result, build_callback_html(&outcome))
                    }
                    (Some(_), false) => {
                        crate::modules::logger::log_error(
                            "OAuth callback state mismatch (CSRF protection)",
                        );
                        (
                            Err("OAuth state mismatch".to_string()),
                            oauth_fail_html().to_string(),
                        )
                    }
                    (None, _) => (
                        Err("Failed to get Authorization Code in callback".to_string()),
                        oauth_fail_html().to_string(),
                    ),
                };

                let _ = stream.write_all(response_html.as_bytes()).await;
                let _ = stream.flush().await;

                if let Some(h) = app_handle {
                    use tauri::Emitter;
                    let _ = h.emit("oauth-callback-received", ());
                }
                let _ = tx.send(result).await;
            }
        });
    }

    if let Some(l6) = ipv6_listener {
        let tx = code_tx.clone();
        let mut rx = cancel_rx;
        let app_handle = app_handle_for_tasks;
        let redir = redirect_uri.clone();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = tokio::select! {
                res = l6.accept() => res.map_err(|e| format!("failed_to_accept_connection: {}", e)),
                _ = rx.changed() => Err("OAuth cancelled".to_string()),
            } {
                let mut buffer = [0u8; 4096];
                let bytes_read = stream.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);

                let query_params = request
                    .lines()
                    .next()
                    .and_then(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            Some(parts[1])
                        } else {
                            None
                        }
                    })
                    .and_then(|path| Url::parse(&format!("http://localhost{}", path)).ok())
                    .map(|url| {
                        let mut code = None;
                        let mut state = None;
                        for (k, v) in url.query_pairs() {
                            if k == "code" {
                                code = Some(v.to_string());
                            } else if k == "state" {
                                state = Some(v.to_string());
                            }
                        }
                        (code, state)
                    });

                let (code, received_state) = match query_params {
                    Some((c, s)) => (c, s),
                    None => (None, None),
                };

                if code.is_none() && bytes_read > 0 {
                    crate::modules::logger::log_error(&format!(
                        "OAuth callback failed to parse code (IPv6). Raw request: {}",
                        &request.chars().take(512).collect::<String>()
                    ));
                }

                // Verify state
                let state_valid = {
                    if let Ok(lock) = get_oauth_flow_state().lock() {
                        if let Some(s) = lock.as_ref() {
                            received_state.as_ref() == Some(&s.state)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                let (result, response_html) = match (code, state_valid) {
                    (Some(code), true) => {
                        crate::modules::logger::log_info(
                            "Successfully captured OAuth code from IPv6 listener, processing full flow...",
                        );
                        // Run the full flow (exchange → save → onboard → test) before responding
                        let (outcome, code_result) =
                            process_oauth_callback(&code, &redir, app_handle.clone()).await;
                        (code_result, build_callback_html(&outcome))
                    }
                    (Some(_), false) => {
                        crate::modules::logger::log_error(
                            "OAuth callback state mismatch (IPv6 CSRF protection)",
                        );
                        (
                            Err("OAuth state mismatch".to_string()),
                            oauth_fail_html().to_string(),
                        )
                    }
                    (None, _) => (
                        Err("Failed to get Authorization Code in callback".to_string()),
                        oauth_fail_html().to_string(),
                    ),
                };

                let _ = stream.write_all(response_html.as_bytes()).await;
                let _ = stream.flush().await;

                if let Some(h) = app_handle {
                    use tauri::Emitter;
                    let _ = h.emit("oauth-callback-received", ());
                }
                let _ = tx.send(result).await;
            }
        });
    }

    // Save state
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        *state = Some(OAuthFlowState {
            auth_url: auth_url.clone(),
            redirect_uri,
            state: state_str,
            cancel_tx,
            code_tx,
            code_rx: Some(code_rx),
        });
    }

    // Send event to frontend (for display/copying link)
    if let Some(h) = app_handle {
        use tauri::Emitter;
        let _ = h.emit("oauth-url-generated", &auth_url);
    }

    Ok(auth_url)
}

/// Pre-generate OAuth URL (does not open browser, does not block waiting for callback)
pub async fn prepare_oauth_url(app_handle: Option<tauri::AppHandle>) -> Result<String, String> {
    ensure_oauth_flow_prepared(app_handle).await
}

/// Cancel current OAuth flow
pub fn cancel_oauth_flow() {
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        if let Some(s) = state.take() {
            let _ = s.cancel_tx.send(true);
            crate::modules::logger::log_info("Sent OAuth cancellation signal");
        }
    }
}

/// Start OAuth flow and wait for callback, then exchange token
pub async fn start_oauth_flow(
    app_handle: Option<tauri::AppHandle>,
) -> Result<oauth::TokenResponse, String> {
    // Ensure URL + listener are ready (this way if the user authorizes first, it won't get stuck)
    let auth_url = ensure_oauth_flow_prepared(app_handle.clone()).await?;

    if let Some(h) = app_handle {
        // Open default browser
        use tauri_plugin_opener::OpenerExt;
        h.opener()
            .open_url(&auth_url, None::<String>)
            .map_err(|e| format!("failed_to_open_browser: {}", e))?;
    }

    // Take code_rx to wait for token
    let mut code_rx = {
        let mut lock = get_oauth_flow_state()
            .lock()
            .map_err(|_| "OAuth state lock corrupted".to_string())?;
        let Some(state) = lock.as_mut() else {
            return Err("OAuth state does not exist".to_string());
        };
        state
            .code_rx
            .take()
            .ok_or_else(|| "OAuth authorization already in progress".to_string())?
    };

    // Wait for TokenResponse (already exchanged by process_oauth_callback)
    // For mpsc, we use recv()
    let token_res = match code_rx.recv().await {
        Some(Ok(token)) => token,
        Some(Err(e)) => return Err(e),
        None => return Err("OAuth flow channel closed unexpectedly".to_string()),
    };

    // Clean up flow state (release cancel_tx, etc.)
    if let Ok(mut lock) = get_oauth_flow_state().lock() {
        *lock = None;
    }

    Ok(token_res)
}

/// Завершить OAuth flow без открытия браузера.
/// Предполагается, что пользователь открыл ссылку вручную (или ранее была открыта),
/// а мы только ждём callback и обмениваем code на token.
pub async fn complete_oauth_flow(
    app_handle: Option<tauri::AppHandle>,
) -> Result<oauth::TokenResponse, String> {
    // Ensure URL + listeners exist
    let _ = ensure_oauth_flow_prepared(app_handle).await?;

    // Take receiver to wait for token
    let (mut code_rx, _redirect_uri) = {
        let mut lock = get_oauth_flow_state()
            .lock()
            .map_err(|_| "OAuth state lock corrupted".to_string())?;
        let Some(state) = lock.as_mut() else {
            return Err("OAuth state does not exist".to_string());
        };
        let rx = state
            .code_rx
            .take()
            .ok_or_else(|| "OAuth authorization already in progress".to_string())?;
        (rx, state.redirect_uri.clone())
    };

    let token_res = match code_rx.recv().await {
        Some(Ok(token)) => token,
        Some(Err(e)) => return Err(e),
        None => return Err("OAuth flow channel closed unexpectedly".to_string()),
    };

    if let Ok(mut lock) = get_oauth_flow_state().lock() {
        *lock = None;
    }

    Ok(token_res)
}

/// Manually submit an OAuth code to complete the flow.
/// This is used when the user manually copies the code/URL from the browser
/// because the localhost callback couldn't be reached (e.g. in Docker/remote).
pub async fn submit_oauth_code(
    code_input: String,
    state_input: Option<String>,
) -> Result<(), String> {
    let (tx, redirect_uri) = {
        let lock = get_oauth_flow_state().lock().map_err(|e| e.to_string())?;
        if let Some(state) = lock.as_ref() {
            // Verify state if provided
            if let Some(provided_state) = state_input {
                if provided_state != state.state {
                    return Err("OAuth state mismatch (CSRF protection)".to_string());
                }
            }
            (state.code_tx.clone(), state.redirect_uri.clone())
        } else {
            return Err("No active OAuth flow found".to_string());
        }
    };

    // Extract code if it's a URL
    let code = if code_input.starts_with("http") {
        if let Ok(url) = Url::parse(&code_input) {
            url.query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.to_string())
                .unwrap_or(code_input)
        } else {
            code_input
        }
    } else {
        code_input
    };

    crate::modules::logger::log_info("Received manual OAuth code submission, exchanging...");

    // Exchange code for token before sending through channel
    let token_res = oauth::exchange_code(&code, &redirect_uri).await?;

    // Send TokenResponse to the channel
    tx.send(Ok(token_res))
        .await
        .map_err(|_| "Failed to send token to OAuth flow (receiver dropped)".to_string())?;

    Ok(())
}
/// Manually prepare an OAuth flow without starting listeners.
/// Useful for Web/Docker environments where we only need manual code submission.
pub fn prepare_oauth_flow_manually(
    redirect_uri: String,
    state_str: String,
) -> Result<(String, mpsc::Receiver<Result<oauth::TokenResponse, String>>), String> {
    let auth_url = oauth::get_auth_url(&redirect_uri, &state_str);

    // Check if we can reuse existing state
    if let Ok(mut lock) = get_oauth_flow_state().lock() {
        if let Some(s) = lock.as_mut() {
            // If we already have a code_rx, we can't easily "steal" it again because it's already returned.
            // But if this is a NEW request (different state), we should overwrite.
            // For now, let's just clear and restart to be safe.
            let _ = s.cancel_tx.send(true);
            *lock = None;
        }
    }

    let (cancel_tx, _cancel_rx) = watch::channel(false);
    let (code_tx, code_rx) = mpsc::channel(1);

    if let Ok(mut state) = get_oauth_flow_state().lock() {
        *state = Some(OAuthFlowState {
            auth_url: auth_url.clone(),
            redirect_uri: redirect_uri.clone(),
            state: state_str,
            cancel_tx,
            code_tx,
            code_rx: None, // We return it directly
        });
    }

    Ok((auth_url, code_rx))
}
