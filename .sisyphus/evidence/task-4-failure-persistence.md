# Task 4 failure-path project_id persistence evidence

## Scope checked

- `src-tauri/src/modules/account.rs`
- `src-tauri/src/proxy/token_manager.rs`
- `src-tauri/src/modules/quota.rs`

## Failure-path rule now enforced from code

If a fresh onboarding/runtime project resolution attempt fails, the code now preserves any previously stored real `project_id` unchanged and surfaces the fresh failure instead of treating the retained value as proof of successful re-onboarding.

## Code paths and enforced behavior

### 1. Shared real-ID gate in `src-tauri/src/modules/quota.rs`

- `normalize_real_project_id()` now centralizes the definition of a persistable project ID: trimmed, non-empty string only.
- `fetch_quota_with_cache()` now rejects fresh resolution paths that still end without a real project ID by returning `AppError::Account(...)` instead of building an empty `{}` payload.
- Effect on failure semantics: quota fetches no longer quietly continue after a fresh resolution failure with an empty/default/fallback project payload.

### 2. Quota refresh persistence in `src-tauri/src/modules/account.rs`

- `fetch_quota_with_retry()` now routes success-side writes through `persist_resolved_project_id()`.
- `persist_resolved_project_id()` only writes when the returned ID is a real normalized ID and different from the already stored real ID.
- If fresh quota/onboarding resolution fails, this helper is never called, so the previously stored real `account.token.project_id` remains unchanged.
- If the code does get a fresh real ID but cannot persist it, the helper returns `AppError::Account(...)` instead of warning and continuing. That prevents a false "success" state with a non-durable project ID.

### 3. Runtime account selection in `src-tauri/src/proxy/token_manager.rs`

#### Preferred-account path

- Around the preferred-account `project_id` branch, the code now first checks `normalized_real_project_id(token.project_id.as_deref())`.
- If no stored real ID exists, it resolves a fresh one.
- That fresh ID is passed to `save_project_id()` before the in-memory entry is updated or the request succeeds.
- If persistence fails, the path now returns `Err("Preferred account resolved project_id but failed to persist it: ...")`.
- Effect: an old retained value is not overwritten with junk, and a new runtime resolution is not treated as successful unless the real ID is durably saved.

#### Round-robin/runtime re-resolution path

- The general runtime selection path now uses the same normalized-real-ID check.
- On fresh resolution success, it persists first and only then updates in-memory `entry.project_id`.
- On persistence failure, the account is skipped with `last_error = Some("Project resolution succeeded for ... but failed to persist project_id: ...")` and the request continues searching for another account.
- Effect: a retained old project ID is preserved on fresh failure, but the fresh failure is still surfaced through selection failure/skip logic rather than being counted as successful re-resolution.

#### Warmup/runtime lookup path

- `get_token_by_email()` now treats only normalized real IDs as reusable.
- If it must freshly resolve a project ID, it persists that value first.
- If persistence fails, it returns `Err("[Warmup] Resolved project_id for ... but failed to persist it: ...")`.
- Effect: warmup/runtime paths cannot silently claim success with a merely in-memory project ID.

### 4. `save_project_id()` hardening in `src-tauri/src/proxy/token_manager.rs`

- `save_project_id()` now rejects empty/non-real IDs outright with `Refusing to persist empty or non-real project_id`.
- It returns the normalized saved value so callers can update in-memory state only after disk persistence succeeds.
- Effect: no touched runtime path can write empty/default/fallback `project_id` values through this helper anymore.

## Failure-path outcome summary

- Fresh failure to resolve a real `project_id` no longer writes a fallback/default value.
- Existing stored real `project_id` values are left unchanged on fresh failure because writes are attempted only after successful fresh resolution.
- Retained old values are not treated as evidence that fresh onboarding/runtime re-resolution succeeded; the fresh failure is propagated as an error/skip.
- Persistence failures relevant to success claims are no longer silently ignored in the touched paths.

## Verification evidence used

- File inspection after edits:
  - `src-tauri/src/modules/quota.rs:193-203` and success return path in `fetch_quota_with_cache()`
  - `src-tauri/src/modules/account.rs:1469-1495`, `1572-1575`, `1645-1648`
  - `src-tauri/src/proxy/token_manager.rs:1458-1487`, `1877-1916`, `1982-2002`, `2080-2099`

## Runtime verification blockers

- `lsp_diagnostics` could not run because the environment is missing `rust-analyzer`.
- `cargo check` with the default target directory is still blocked by root-owned artifacts under `src-tauri/target`.
- `cargo check` with isolated target directory `/tmp/antigravity-task4-target` progressed past that blocker and then failed on missing system `pkg-config` / GTK development dependencies (`glib-2.0`, `gobject-2.0`, `gio-2.0`, `gdk-3.0`, `cairo`, `pango`).
- No live onboarding persistence output is claimed in this evidence file.
