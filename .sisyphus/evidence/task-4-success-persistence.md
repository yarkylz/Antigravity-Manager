# Task 4 success-path project_id persistence evidence

## Scope checked

- `src-tauri/src/modules/account.rs`
- `src-tauri/src/proxy/token_manager.rs`
- `src-tauri/src/modules/quota.rs`

## Success-path rule now enforced from code

Successful resolution paths in the touched files now persist only real resolved `project_id` values, and the touched runtime/onboarding-adjacent paths do not treat fresh resolution as successful until that real ID has been durably written.

## Code paths and enforced behavior

### 1. Real project ID definition in `src-tauri/src/modules/quota.rs`

- `normalize_real_project_id()` defines the only acceptable persisted shape: trimmed and non-empty.
- `extract_project_id_from_value()` now uses that same rule when parsing API responses.
- Result: only real resolved IDs reach the persistence paths covered by this task.

### 2. Quota fetch success path in `src-tauri/src/modules/quota.rs`

- `fetch_quota_with_cache()` normalizes the cached/fresh `project_id` and requires a real value before building the quota payload.
- When the helper is called with an `account_id`, it now runs `persist_project_id_for_account(account_id, project_id.as_deref())` before returning `Ok((quota_data, project_id.clone()))`.
- `persist_project_id_for_account()` loads the account, compares against the already stored real value, and only saves if the new real ID differs.
- Result: onboarding-adjacent success through this helper now implies the returned real project ID has also been persisted to the account file.

### 3. Quota refresh success path in `src-tauri/src/modules/account.rs`

- `fetch_quota_with_retry()` uses `persist_resolved_project_id()` after both the first quota fetch and the 401-refresh retry fetch.
- The helper only writes normalized real IDs and skips no-op rewrites when the stored real value already matches.
- If persistence fails, the function returns `AppError::Account(...)` instead of only logging a warning.
- Result: a refresh path cannot claim success with a newly discovered project ID that was not actually written.

### 4. Runtime/proxy success paths in `src-tauri/src/proxy/token_manager.rs`

#### Preferred account

- Freshly resolved IDs are passed to `save_project_id()` first.
- Only after that succeeds does the code update `entry.project_id` and return success.

#### General round-robin selection

- The same sequence now applies: resolve fresh real ID -> persist it -> update in-memory state -> return account as usable.
- If persistence fails, the account is skipped and the request keeps searching instead of pretending this account is fully usable.

#### Warmup by email

- `get_token_by_email()` now reuses only normalized real stored IDs.
- If it resolves a fresh one, it persists first and only then updates memory/returns success.

### 5. `save_project_id()` contract in `src-tauri/src/proxy/token_manager.rs`

- Rejects empty/non-real IDs.
- Writes only a normalized real string into `content["token"]["project_id"]`.
- Returns the exact normalized persisted value so callers can use the saved value as the postcondition for success.

## Exact touched write paths

1. `src-tauri/src/modules/account.rs`
   - `fetch_quota_with_retry()` success path after first `modules::fetch_quota(...)`
   - `fetch_quota_with_retry()` success path after retry `modules::fetch_quota(...)`
   - Rule: only real resolved IDs are saved; write failure aborts the success path.

2. `src-tauri/src/proxy/token_manager.rs`
   - Preferred-account runtime resolution branch
   - General round-robin runtime resolution branch
   - Warmup `get_token_by_email()` resolution branch
   - `save_project_id()` helper itself
   - Rule: only real resolved IDs are written, and callers do not treat fresh resolution as success until persistence succeeds.

3. `src-tauri/src/modules/quota.rs`
   - `fetch_quota_with_cache()` now persists the real `project_id` for the provided `account_id` before returning success.
   - Rule: quota-fetch success with a fresh/project-cached real ID implies the account JSON reflects that real ID too.

## Verification evidence used

- `cargo fmt --manifest-path "src-tauri/Cargo.toml" --all` completed successfully.
- File inspection after formatting:
  - `src-tauri/src/modules/quota.rs:193-220` plus success return path in `fetch_quota_with_cache()`
  - `src-tauri/src/modules/account.rs:1469-1495`, `1572-1575`, `1645-1648`
  - `src-tauri/src/proxy/token_manager.rs:48-50`, `1458-1487`, `1877-1916`, `1982-2002`, `2080-2099`

## Runtime verification blockers

- `lsp_diagnostics` could not initialize because `rust-analyzer` is unavailable in this environment.
- `cargo check --manifest-path "src-tauri/Cargo.toml"` against the default target dir is blocked by pre-existing root-owned files under `src-tauri/target`.
- `CARGO_TARGET_DIR=/tmp/antigravity-task4-target cargo check --manifest-path "src-tauri/Cargo.toml"` bypassed that old target-permission blocker but still failed because `pkg-config` and required GTK/glib system libraries are missing.
- Because of those environment constraints, this file documents code-backed success semantics rather than fabricated live onboarding output.
