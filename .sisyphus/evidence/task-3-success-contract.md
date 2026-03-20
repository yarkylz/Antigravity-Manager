# Task 3 success contract evidence

## Success path summary

After the Task 3 fix, `onboard_account` reaches `success: true` only when all of the following are true in `src-tauri/src/commands/mod.rs`:

1. token refresh succeeds
2. `modules::quota::resolve_project_with_contract(...)` returns `ProjectResolutionOutcome::Resolved(project)`
3. `modules::quota::fetch_quota_with_cache(..., Some(&resolved_project.project_id), ...)` succeeds
4. the returned quota data is not marked forbidden

This is the only branch that now emits the onboarding completion log and success payload.

## Real project ID requirement

The success path stores the project-resolution result in `resolved_project` and later interpolates:

- success details: `Project ID: {resolved_project.project_id}, Subscription: {tier}`
- quota fetch input: `Some(&resolved_project.project_id)`

That means success details can only contain a project ID that came from the shared Task 2 resolver contract. The old masked-success pattern where success could still happen after `done=true` without a real `project_id` is no longer reachable.

## Completion log integrity

The success log remains:

- `Onboarding completed for {email}: {model_count} models, tier: {tier}`

But it is now conditional in practice because it sits only inside the resolved-project + successful-quota + non-forbidden branch.

As a result, project-resolution failure sessions do **not** emit:

- `Onboarding completed for ...`

This directly closes the Task 1 false-success symptom where terminal project acquisition failure still produced a completion log.

## Tier handling on success

The success branch computes tier as:

1. `quota_data.subscription_tier`
2. fallback to `resolved_project.subscription_tier`
3. final UI fallback to `Unknown`

This fallback order affects only the subscription label. It does **not** relax the project requirement because the project ID itself must already exist in `resolved_project.project_id` before the success branch is reachable.

## Success/failure boundary

The command now has a clean contract boundary:

- token refresh failure => `token_error`
- project acquisition failure => project-resolution-specific failure statuses
- quota-layer failure after real project acquisition => `forbidden` or `error`
- onboarding success => only after real project acquisition and successful quota verification

## Runtime note

This evidence describes the corrected success contract from the code that was changed. It does not claim a full runtime execution in this environment, because prior task notes already document host Rust verification blockers (`target` permission issues and missing `pkg-config`).
