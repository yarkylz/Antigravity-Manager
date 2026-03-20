# Task 5 runtime follow-up evidence

## Scope

This evidence focuses on runtime/project behavior after onboarding, using the current code in:

- `src-tauri/src/modules/quota.rs`
- `src-tauri/src/proxy/token_manager.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/proxy/project_resolver.rs`

## Runtime follow-up rule now enforced

The effective runtime policy in the current code is:

- runtime may reuse an already persisted real `project_id`
- runtime may freshly resolve a project only through the shared `resolve_project_with_contract()` semantics
- if that fresh resolution fails, the runtime path fails or skips the account instead of manufacturing a default fallback project

## Runtime call sites that reuse persisted real project IDs

### 1. `src-tauri/src/modules/quota.rs::fetch_quota()`

Task 5 changed this public quota entrypoint so it now:

- loads the account when `account_id` is present
- extracts only a normalized real persisted `token.project_id`
- passes that value into `fetch_quota_with_cache()`

Effect:

- callers such as account quota refresh and `test_account_request` now reuse a persisted real project ID when one already exists
- they no longer force a fresh project-resolution attempt merely because the caller did not separately provide a cached project argument

### 2. Preferred-account selection in `src-tauri/src/proxy/token_manager.rs`

At the preferred-account path (`1458+`):

- `normalized_real_project_id(token.project_id.as_deref())` is checked first
- if present, that persisted real ID is reused immediately
- only missing/non-real values trigger fresh resolution

### 3. General round-robin runtime selection in `src-tauri/src/proxy/token_manager.rs`

At the general token path (`1877+`):

- runtime first checks `token.project_id`
- a normalized real stored ID is reused
- only absent/non-real values trigger fresh resolution

### 4. Warmup token lookup in `src-tauri/src/proxy/token_manager.rs`

At the warmup-by-email path (`2094+`):

- an existing normalized real `project_id` is reused
- only a missing/non-real value triggers fresh resolution

### 5. Quota follow-up after successful onboarding in `src-tauri/src/commands/mod.rs`

`onboard_account()` uses:

- `fetch_quota_with_cache(..., Some(&resolved_project.project_id), Some(&account_id))`

So post-resolution quota fetch reuses the exact real project produced by the successful onboarding contract.

## Runtime call sites that fail/skip instead of manufacturing fallback IDs

### 1. Shared quota path: `src-tauri/src/modules/quota.rs::fetch_quota_with_cache()`

If there is no cached persisted real ID and fresh shared resolution still does not return a real project, the function returns an `AppError::Account(...)` with:

- `Missing real project_id for ... after fresh project resolution attempt; retaining any previously stored value unchanged ...`

It does **not**:

- send `{}` as a quota request payload
- synthesize `bamboo-precept-lgxtn`
- treat the account as successfully resolved

### 2. Preferred-account runtime path

If fresh resolution fails, the preferred-account path returns:

- `Err("Preferred account missing usable project_id: ...")`

That means the request does not proceed as though the account were healthy.

### 3. General round-robin runtime path

If fresh resolution fails for a candidate account, the code:

- logs `Skipping account ... because project_id resolution failed: ...`
- records `last_error`
- adds the account to `attempted`
- continues searching for another usable account

So the runtime path skips the broken account instead of manufacturing a fallback project.

### 4. Warmup/runtime email lookup path

If fresh resolution fails here, the path returns:

- `Err("[Warmup] Missing project_id for ...: ...")`

No fallback project is generated.

### 5. Shared resolver facade

`src-tauri/src/proxy/project_resolver.rs::fetch_project_id()` now converts non-success `ProjectResolutionOutcome` values into explicit errors such as:

- `loadCodeAssist returned HTTP ...`
- `onboardUser returned done=true without a real project_id`
- parse/transport/in-progress exhaustion messages

This preserves the shared contract semantics for callers that use the resolver facade directly.

## Fallback/default project synthesis status

Within the Task 5 allowed/runtime-inspected code:

- no remaining `bamboo-precept-lgxtn` synthesis was found
- no empty-project success payload path remains in the touched quota/runtime logic
- no inspected runtime caller converts fresh shared-resolution failure into a healthy project-bearing success path

## Exact environment blockers preventing full live replay

Fresh Task 5 verification commands produced these blockers:

### 1. Default target directory is still permission-blocked

Command run:

```bash
cargo check --manifest-path "/home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml"
```

Observed failure:

```text
failed to write ... /src-tauri/target/debug/.fingerprint/... Permission denied (os error 13)
```

Meaning:

- the repository still has root-owned artifacts under the default Rust target directory
- this blocks honest host-side verification with the default build location

### 2. Isolated target build is blocked by missing system packages

Command run:

```bash
env CARGO_TARGET_DIR=/tmp/antigravity-task5-target cargo check --manifest-path "/home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml"
```

Observed failure:

- `pkg-config` not found
- GTK/glib-related crates (`glib-sys`, `gobject-sys`, and related system-library detection) fail during build-script execution

Meaning:

- the edited Rust code got past the default target-dir permission blocker when isolated
- but full host Rust verification is still blocked by missing native system dependencies in this environment

### 3. LSP diagnostics blocker

`lsp_diagnostics` could not be completed because the environment lacks `rust-analyzer` in the active toolchain.

## Runtime follow-up conclusion

The current code-backed runtime behavior is:

- reuse persisted real `project_id` values where available
- fresh resolution only through the shared onboarding-parity contract
- explicit error/skip on fresh resolution failure
- no runtime path in the inspected Task 5 scope silently manufactures success through `bamboo-precept-lgxtn`
