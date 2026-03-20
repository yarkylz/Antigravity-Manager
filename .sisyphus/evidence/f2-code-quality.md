# F2 Code Quality Review

Scope reviewed: current backend diff/stat plus the changed backend files in scope only:

- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/modules/account.rs`
- `src-tauri/src/modules/quota.rs`
- `src-tauri/src/proxy/project_resolver.rs`
- `src-tauri/src/proxy/token_manager.rs`

Focus areas reviewed:

- duplicated resolver logic
- fallback leakage / default project synthesis
- ignored persistence errors on success-claiming paths
- unrelated cleanup / scope creep

## Findings

### INFO — Prior onboarding persistence blocker is fixed in `onboard_account`

File: `src-tauri/src/commands/mod.rs`

Evidence:

- `onboard_account(...)` now refreshes the token inline and attempts `crate::modules::account::save_account(&account)` before moving on.
- If that save fails, the function immediately returns `OnboardingResult { success: false, status: Some("token_persistence_error") }` instead of continuing toward the success path.
- The later `success: true` result remains reachable only after that branch is passed, project resolution succeeds, and quota verification completes.

Assessment:

- The previously reported high-severity issue is resolved in the current backend state.
- I no longer see a path in `onboard_account` where refreshed-token persistence failure can still lead to a successful onboarding result.

### INFO — Hidden fallback/default project synthesis remains absent in the reviewed changed scope

Files:

- `src-tauri/src/modules/quota.rs`
- `src-tauri/src/proxy/token_manager.rs`
- `src-tauri/src/proxy/project_resolver.rs`

Evidence:

- `modules/quota.rs::fetch_quota_with_cache()` now requires a real normalized `project_id` and errors if none is available, instead of constructing an empty or fallback payload.
- `modules/quota.rs::get_valid_token_for_warmup()` now errors when shared resolution yields no real `project_id` instead of returning `bamboo-precept-lgxtn`.
- `token_manager.rs` runtime selection and warmup-related paths reject unresolved or unpersistable `project_id` values instead of synthesizing a default project.
- `project_resolver.rs` is now a thin wrapper over the shared explicit contract rather than a fallback-producing side path.

Assessment:

- The major hidden-fallback risk remains fixed in the current reviewed backend diff.
- I did not find a surviving default-project synthesis path in the changed backend scope.

### LOW — `ProjectResolutionOutcome` translation logic is still duplicated, but it is now maintainability-only

Files:

- `src-tauri/src/proxy/project_resolver.rs`
- `src-tauri/src/proxy/token_manager.rs`
- `src-tauri/src/commands/mod.rs`

Evidence:

- `project_resolver.rs` contains `describe_resolution_outcome(...)`, translating each `ProjectResolutionOutcome` variant into text.
- `token_manager.rs` contains `resolve_project_id_for_runtime(...)`, which re-maps the same variants into runtime-facing string errors.
- `commands/mod.rs::onboard_account(...)` still matches the full enum again to produce structured API responses.

Why this matters:

- The interpretation layer is still spread across multiple files, so future enum additions or wording changes can drift.
- That said, the duplication is now serving different consumers (helper-string surface, runtime error surface, structured command result surface), and I did not find evidence that it is currently causing incorrect behavior.

Assessment:

- This remains a code-quality concern, but only at low maintainability severity in the current state.
- It is not reject-worthy on its own for this final backend review.

### INFO — No obvious unrelated cleanup/scope creep found in the changed backend diff

Assessment:

- The touched backend files remain focused on onboarding/project-id parity and related runtime consistency.
- I did not see opportunistic cleanup unrelated to the review scope inside the changed backend files.

## Verdict rationale

The prior blocking defect has been addressed: `onboard_account` no longer reports success after refreshed-token persistence failure. The more critical historical risk area — hidden/default `project_id` synthesis masking failure — also remains absent in the current backend scope.

The only notable remaining issue is duplicated `ProjectResolutionOutcome` translation logic across multiple changed files, but in the current state that reads as maintainability debt rather than a correctness or release-blocking backend risk.

VERDICT: APPROVE
