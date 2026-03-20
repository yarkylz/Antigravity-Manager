# Task 2 parity audit

## Centralized shared contract

The shared backend project-resolution contract now lives in `src-tauri/src/modules/quota.rs`.

New shared types/functions introduced there:
- `ResolvedCloudProject { project_id, subscription_tier }`
- `ProjectResolutionStage::{LoadCodeAssist, OnboardUser}`
- `ProjectResolutionOutcome::{Resolved, InProgressExhausted, TransportFailure, LoadHttpFailure, OnboardHttpFailure, TerminalMissingProject, ParseFailure}`
- `extract_project_id_from_value()`
- `extract_project_metadata()`
- `resolve_project_with_contract()`

## Parsing semantics

`extract_project_id_from_value()` accepts both reference-supported shapes for `cloudaicompanionProject`:
- plain string project IDs
- object values with `.id`

The parser trims whitespace and rejects empty strings in both cases.

## Control-flow parity

`resolve_project_with_contract()` now matches the reference priority order:
1. call `loadCodeAssist` first
2. if `loadCodeAssist` succeeds but still has no real project, derive onboarding tier metadata from the same response
3. call `onboardUser` only from that successful-without-project state
4. poll `onboardUser` at most 5 times with 2-second waits only while the response is successful and incomplete
5. treat `done=true` without a real project as terminal failure rather than success

## Explicit outcome mapping

The shared contract now distinguishes these states explicitly:
- success with a real project: `ProjectResolutionOutcome::Resolved`
- polling exhaustion / still incomplete after 5 attempts: `ProjectResolutionOutcome::InProgressExhausted`
- transport error from either endpoint: `ProjectResolutionOutcome::TransportFailure { stage, error, ... }`
- non-200 `loadCodeAssist`: `ProjectResolutionOutcome::LoadHttpFailure { status, body_preview, ... }`
- non-200 `onboardUser`: `ProjectResolutionOutcome::OnboardHttpFailure { status, body_preview, ... }`
- terminal `done=true` without project: `ProjectResolutionOutcome::TerminalMissingProject { stage, ... }`
- JSON decode failure on either endpoint: `ProjectResolutionOutcome::ParseFailure { stage, error, ... }`

## Updated call sites in Task 2 scope

### `src-tauri/src/modules/quota.rs`
- `fetch_project_id()` is now a compatibility wrapper over `resolve_project_with_contract()` and no longer owns divergent load/onboard behavior.
- `get_valid_token_for_warmup()` no longer synthesizes `bamboo-precept-lgxtn`; it now returns an explicit warmup error when shared project resolution does not yield a real project.

### `src-tauri/src/proxy/project_resolver.rs`
- `resolve_project()` now directly returns the shared `ProjectResolutionOutcome` from `quota.rs`.
- `fetch_project_id()` remains as a compatibility wrapper for external callers but converts only `Resolved` into `Ok(project_id)` and converts all other states into descriptive `Err(...)` values.

### `src-tauri/src/proxy/token_manager.rs`
- Added `resolve_project_id_for_runtime()` to translate the shared outcome contract into runtime error strings without manufacturing success.
- Preferred-account project lookup now fails explicitly when the shared contract does not yield a usable project.
- Round-robin runtime selection now skips accounts whose project resolution fails instead of injecting a default project ID.
- `get_token_by_email()` now resolves and persists a real project ID when the cached value is absent, rather than defaulting to bamboo.
- `add_account()` now propagates project-resolution failure instead of persisting a fallback project ID.

## Remaining fallback posture after Task 2

Within the Task 2-touched warmup/runtime/shared-resolution paths above, no hidden `bamboo-precept-lgxtn` fallback remains in the shared success contract or in the updated call sites.

## Verification note

The required build evidence is captured separately in `.sisyphus/evidence/task-2-build.txt`. In this environment the host build command may fail before Rust compilation because `pkg-config` / GTK-related system dependencies are unavailable; when that happens the evidence file records the actual command output rather than claiming a successful build.
