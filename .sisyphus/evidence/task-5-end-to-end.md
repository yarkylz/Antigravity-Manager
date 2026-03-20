# Task 5 end-to-end parity evidence

## Scope

This evidence closes the final Task 5 requirement: runtime/token follow-up behavior must not diverge from the onboarding truth established in Tasks 2-4.

Files inspected for this evidence:

- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/modules/quota.rs`
- `src-tauri/src/proxy/token_manager.rs`
- `.sisyphus/evidence/task-1-red-onboarding-baseline.md`
- `.sisyphus/evidence/task-3-failure-contract.md`
- `.sisyphus/evidence/task-4-failure-persistence.md`

## Original masked-success baseline from Task 1

Task 1 captured the pre-fix bad outcome from a real local run:

- `onboardUser` returned `done=true but no project_id in response`
- the same run still logged `Onboarding completed for pemovet703@gmail.com: 14 models, tier: Unknown`

That meant onboarding truth and later quota/runtime behavior had diverged: the backend had already learned that no real project was obtained, but another path still produced a completion-style success state.

See `.sisyphus/evidence/task-1-red-onboarding-baseline.md` for the original log excerpt.

## Contract after Tasks 2-4

By the time Task 5 started, the code already enforced these parity rules:

1. `resolve_project_with_contract()` in `src-tauri/src/modules/quota.rs` is the shared source of truth for project resolution.
2. `loadCodeAssist` accepts tolerated project shapes (string or object-with-`id`) and only falls through to `onboardUser` after a successful load response that still lacks a project.
3. `done=true` without `project_id` becomes an explicit terminal failure (`ProjectResolutionOutcome::TerminalMissingProject`) instead of a silent success.
4. Onboarding (`onboard_account`) now returns `success: false` for project-acquisition failure states and only reaches the success log/message path after `ProjectResolutionOutcome::Resolved(project)`.
5. Persistence writes only real normalized project IDs and preserves any already stored real `project_id` on fresh failure instead of overwriting it with empty/default/fallback values.

Those behaviors are documented in:

- `.sisyphus/evidence/task-3-failure-contract.md`
- `.sisyphus/evidence/task-4-failure-persistence.md`

## Concrete Task 5 runtime gap found and fixed

One remaining runtime divergence was still present when Task 5 resumed:

- the public quota entrypoint `src-tauri/src/modules/quota.rs::fetch_quota()` always called `fetch_quota_with_cache(..., None, account_id)`
- that meant callers with a valid `account_id` but no explicitly passed cached project still forced a fresh project-resolution attempt every time
- this contradicted the fixed runtime policy boundary: runtime may reuse an already persisted real `project_id`

### Minimal fix applied

`fetch_quota()` now loads the account when `account_id` is available, extracts only a normalized real persisted `token.project_id`, and passes that into `fetch_quota_with_cache()`.

Resulting behavior:

- if a real persisted `project_id` already exists, runtime quota follow-up reuses it
- if no real persisted `project_id` exists, the shared fresh-resolution contract still runs
- if that fresh resolution fails, `fetch_quota_with_cache()` now returns an error instead of manufacturing a successful payload path

## Why silent runtime divergence through default project synthesis is now prevented

### Onboarding side

`src-tauri/src/commands/mod.rs::onboard_account()` now:

- resolves the project first with `resolve_project_with_contract()`
- exits with failure statuses like `project_resolution_failed`, `project_missing_after_done`, or `project_poll_exhausted` on project-acquisition failure
- only fetches quota with `fetch_quota_with_cache(..., Some(&resolved_project.project_id), ...)` after a real project was resolved

So the original Task 1 symptom (`done=true` without project, then success) is no longer represented by the current command contract.

### Runtime side

`src-tauri/src/modules/quota.rs::fetch_quota_with_cache()` now has only two allowed project paths:

1. reuse a normalized real cached/persisted `project_id`
2. perform a fresh shared-contract resolution

If neither yields a real project, it returns:

- `AppError::Account("Missing real project_id for ... after fresh project resolution attempt; retaining any previously stored value unchanged ...")`

It then sends the quota request only with:

```json
{ "project": "<real project id>" }
```

There is no remaining empty-payload success path in the touched code, and no Task 5-inspected path now synthesizes `bamboo-precept-lgxtn` as a healthy follow-up outcome.

## What can be executed in this environment

Fresh command evidence gathered during Task 5:

### Code-backed verification that did run

- `cargo check --manifest-path "/home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml"`
  - failed with `Permission denied (os error 13)` while writing under `src-tauri/target/...`
  - this confirms the pre-existing root-owned default target-dir blocker still exists
- `env CARGO_TARGET_DIR=/tmp/antigravity-task5-target cargo check --manifest-path "/home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml"`
  - progressed past the target-dir permission issue
  - then failed because `pkg-config` and required GTK/glib system libraries are missing in this environment
- direct code inspection of the current Rust files confirmed the runtime/onboarding branches described above

### What could not be honestly claimed here

- no live end-to-end desktop replay of the original Task 1 scenario was completed in this environment during Task 5
- no claim is made that a real app run now succeeds or fails on the host beyond what the captured code and build-blocker evidence supports
- `lsp_diagnostics` could not be completed because `rust-analyzer` is not installed in the current toolchain environment

## End-to-end conclusion supported by code

The current code no longer supports the original mismatch where:

- onboarding truth says no real project was acquired
- but a later runtime/quota path silently succeeds via default project synthesis

Instead, the current backend rules are:

- onboarding success requires a real resolved project
- runtime may reuse an already persisted real project
- fresh runtime resolution failure does not synthesize `bamboo-precept-lgxtn`
- fresh failure is surfaced as an explicit error/skip path instead of a healthy success path
