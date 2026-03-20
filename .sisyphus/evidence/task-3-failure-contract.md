# Task 3 failure contract evidence

## Scope

This evidence documents the `onboard_account` failure branches in `src-tauri/src/commands/mod.rs` after the Task 3 command-contract fix.

## Root contract change

`onboard_account` no longer treats `modules::quota::get_valid_token_for_warmup()` as a single opaque gate. The command now performs these steps explicitly:

1. refresh token with `crate::modules::oauth::ensure_fresh_token(...)`
2. resolve project with `modules::quota::resolve_project_with_contract(...)`
3. fetch quota only after `ProjectResolutionOutcome::Resolved(project)`

That change matters because the shared Task 2 project-resolution contract exposes distinct `ProjectResolutionOutcome` branches, while the old warmup helper flattened those failures into a generic string and let onboarding success be inferred later from quota fetch success.

## Project-resolution failure status mapping

The following `OnboardingResult.status` values are now returned for project-resolution failures:

### `project_poll_exhausted`

Returned for:

- `ProjectResolutionOutcome::InProgressExhausted { .. }`

Behavior:

- `success: false`
- message: `Project acquisition did not finish before polling timed out`
- details explain that `onboardUser` stayed in progress through the maximum polling attempts

Why this removes false success:

- the command returns immediately before quota fetch
- no `Onboarding completed for ...` log is emitted
- no fallback/default project ID is ever interpolated

### `project_missing_after_done`

Returned for:

- `ProjectResolutionOutcome::TerminalMissingProject { stage: ProjectResolutionStage::OnboardUser, .. }`

Behavior:

- `success: false`
- message: `Project resolution completed without a real project ID after onboarding`
- details state that `onboardUser` returned a terminal response without `project_id`

Why this removes false success:

- this is the exact `done=true but no project_id in response` family of failure proven in Task 1 baseline
- the command exits before quota fetch and before any completion log

### `project_resolution_failed`

Returned for:

- `ProjectResolutionOutcome::TerminalMissingProject { stage: ProjectResolutionStage::LoadCodeAssist, .. }`
- `ProjectResolutionOutcome::TransportFailure { .. }`
- `ProjectResolutionOutcome::LoadHttpFailure { .. }`
- `ProjectResolutionOutcome::OnboardHttpFailure { .. }`
- `ProjectResolutionOutcome::ParseFailure { .. }`

Behavior:

- `success: false`
- messages are stage-specific (`loadCodeAssist`, `onboardUser`, transport, parse)
- details include the stage-specific failure text and subscription tier when present

Why this removes false success:

- non-200 `loadCodeAssist`, parse failures, transport failures, and equivalent onboarding-stage acquisition failures now terminate onboarding as project-resolution failures instead of being able to drift into a later success path

## Non-project-resolution failures kept distinct

The command still preserves separate non-project statuses:

- `token_error` for token refresh failure before project resolution begins
- `forbidden` for quota fetch success that proves the account is forbidden
- `error` for downstream quota-fetch failure after a real project was resolved

This keeps agent verification able to distinguish:

- token refresh failure
- project acquisition failure
- downstream quota failure

## Evidence from the changed command flow

In the updated `onboard_account` implementation:

- every `ProjectResolutionOutcome` non-success branch returns `OnboardingResult { success: false, ... }`
- the quota fetch call uses `fetch_quota_with_cache(..., Some(&resolved_project.project_id), ...)`, so it is only reached after a real resolved project ID exists
- the completion log and success details are physically located only in the `Resolved(project)` + successful quota path

## Runtime note

This evidence is code-contract evidence only. No successful runtime reproduction is claimed here because the environment already records host Rust verification blockers around root-owned target artifacts and missing `pkg-config`.
