# Onboarding Project ID Parity

## TL;DR
> **Summary**: Align Antigravity-Manager’s onboarding project-resolution flow with the CLIProxyAPI reference so onboarding only succeeds when a real `project_id` is acquired under the reference semantics, instead of being masked by fallback behavior.
> **Deliverables**:
> - Shared backend project-resolution contract for onboarding/runtime parity
> - Reference-matching `loadCodeAssist` → `onboardUser` behavior and error semantics
> - Removal of false-success masking in onboarding completion and persistence paths
> - Agent-executed verification scenarios with captured log/account-file evidence
> **Effort**: Medium
> **Parallel**: YES - 2 waves
> **Critical Path**: 1 → 2 → 3 → 4 → 5

## Context
### Original Request
Investigate why onboarding does not pass correctly and use `/home/milton/antigravity-tools/CLIProxyAPI/internal/auth/antigravity/auth.go` as the behavioral reference.

### Interview Summary
- The observed failure pattern is repeated `done=true but no project_id in response`, followed by onboarding appearing to complete anyway.
- The fix target is **exact reference parity**, not a minimal extraction-only patch and not a visibility-only workaround.
- Scope is backend-first: onboarding project lookup, fallback/error semantics, persistence, and runtime parity for the same project-resolution path.
- The user does **not** want new automated tests added as part of this work; verification must rely on agent-executed scenarios, logs, and file-state inspection.

### Metis Review (gaps addressed)
- Lock command contract: onboarding success must mean **real project acquisition**, not merely “quota fetched”.
- Lock fallback policy: `bamboo-precept-lgxtn` must not mask onboarding success paths.
- Lock persistence rules: stale or fallback project IDs must not be saved as if onboarding succeeded.
- Lock runtime parity: onboarding and runtime/token paths must share the same project-resolution semantics rather than diverging.
- Lock verification evidence: every scenario must capture exact logs and account-file state, with no human-only checks.

## Work Objectives
### Core Objective
Make the backend onboarding flow match the reference implementation’s semantics for project acquisition and completion so the system never reports onboarding as successful unless a real `project_id` is obtained and handled consistently across onboarding and runtime usage.

### Deliverables
- A single shared backend contract for project lookup/parsing/error handling used by onboarding and runtime paths.
- Onboarding command behavior that matches the reference for `loadCodeAssist`, `onboardUser`, bounded polling, and terminal failure on `done=true` without `project_id`.
- Persistence behavior that saves only valid resolved project IDs and **retains an already persisted real project_id** on fresh onboarding failure instead of overwriting it.
- Runtime/token flow parity so later requests do not silently diverge from onboarding semantics, with an explicit runtime fallback policy.
- Evidence artifacts proving red-path reproduction and green-path resolution.

### Definition of Done (verifiable conditions with commands)
- `cargo build --manifest-path /home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml` completes successfully.
- `npm run tauri:debug` launches the app with backend debug logging enabled.
- Triggering onboarding for the target account through the app produces one of two valid outcomes only:
  - explicit onboarding failure with no success toast/state and no masked fallback success, or
  - successful onboarding with a real persisted `project_id`.
- Grepping the current log file under `~/.antigravity_tools/logs/` shows no successful onboarding completion line for a session where `done=true but no project_id in response` occurred.
- Inspecting `~/.antigravity_tools/accounts/<account-id>.json` after failure shows no new fallback/default `project_id` persisted as successful onboarding state.
- Runtime follow-up for the same account reuses the resolved persisted `project_id` path or fails explicitly; it does not silently replace onboarding failure with `bamboo-precept-lgxtn` on a success-claiming path.
- If `ABV_DATA_DIR` is set, log/account evidence is collected from that active data root instead of the default `~/.antigravity_tools` paths.

### Must Have
- Match the reference priority order: `loadCodeAssist` first, `onboardUser` fallback only if the project is still missing.
- Match the reference parsing tolerance for project fields returned as string or object-with-`id`.
- Match the reference terminal failure rule for `done=true` without `project_id`.
- Match the reference gate for `onboardUser`: call it only after a successful `loadCodeAssist` response that still lacks a project; do not invoke it after transport/parse/non-200 `loadCodeAssist` failures.
- Preserve bounded polling behavior and explicit non-200 failure handling.
- Ensure onboarding result semantics and logs no longer claim success when project acquisition failed.
- Retain previously persisted real `project_id` values on failed re-onboarding, but do not treat that retained value as fresh onboarding success.
- Keep runtime fallback policy narrow: runtime may continue to use an already persisted real `project_id`, but must not synthesize `bamboo-precept-lgxtn` after a failed fresh resolution on paths that would otherwise imply a healthy onboarded account.
- Keep scope to backend parity and the minimum observability needed to prove it.

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- Must NOT redesign the onboarding UI or introduce unrelated frontend changes.
- Must NOT broaden into generic proxy routing cleanup outside project-resolution parity.
- Must NOT add new frontend test infrastructure, Vitest/Jest/Playwright, or new Rust automated test files for this task.
- Must NOT leave any code path where fallback/default project ID can still be interpreted as successful onboarding completion.
- Must NOT silently ignore persistence/write errors in a path whose output is later presented as successful onboarding.
- Must NOT fabricate success by using quota fetch alone as the onboarding success criterion.
- Must NOT overwrite an existing real persisted `project_id` with empty/fallback/default values after failed re-onboarding.

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision: none + existing build/runtime commands only (user explicitly chose no new automated tests)
- QA policy: Every task includes agent-executed red/green scenarios using app launch, log inspection, and account-file inspection.
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Extract shared dependencies as Wave-1 tasks for max parallelism.

Wave 1: discovery hardening task (1)

Wave 2: shared backend contract task (2)

Wave 3: onboarding command/runtime parity/persistence and verification tasks (3-5)

### Dependency Matrix (full, all tasks)
| Task | Depends On | Blocks |
|---|---|---|
| 1 | - | 2, 3, 4, 5 |
| 2 | 1 | 3, 4, 5 |
| 3 | 1, 2 | 4, 5 |
| 4 | 1, 2, 3 | 5 |
| 5 | 1, 2, 3, 4 | F1-F4 |

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 1 task → `deep`
- Wave 2 → 1 task → `unspecified-high`
- Wave 3 → 3 tasks → `deep`, `unspecified-high`, `general`
- Final Verification Wave → 4 tasks → `oracle`, `unspecified-high`, `deep`

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. Reproduce the masked onboarding failure and freeze the exact failing baseline

  **What to do**: Launch the app in debug mode, trigger onboarding for the affected account through the local admin HTTP API, and capture the exact current failure signature before any code changes. Record the active log file path under the resolved data root, the target account file, the JSON onboarding result returned by the API, and whether `done=true but no project_id in response` is followed by any completion-style log or fallback-driven success semantics.
  **Must NOT do**: Must NOT change code, clear logs, rotate accounts, or “interpret around” ambiguous outcomes. Must NOT proceed until the red path is captured with exact evidence files.

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: Requires disciplined red-path reproduction and evidence capture before implementation.
  - Skills: [`superpowers/systematic-debugging`] — Root-cause-first reproduction discipline.
  - Omitted: [`superpowers/test-driven-development`] — User explicitly chose no new automated tests.

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: [2, 3, 4, 5] | Blocked By: []

  **References** (executor has NO interview context — be exhaustive):
  - Admin API route mapping: `src/utils/request.ts:23-24` — `onboard_account` maps to `POST /api/accounts/:accountId/onboard`.
  - Admin server route base: `src-tauri/src/proxy/server.rs:452-708` — admin endpoints are nested under `/api`.
  - Active data-root API: `src-tauri/src/proxy/server.rs:623, 1768-1773` — `GET /api/system/data-dir` returns the resolved data dir.
  - Command entry: `src-tauri/src/commands/mod.rs:879-899` — `OnboardingResult` contract.
  - Command implementation: `src-tauri/src/commands/mod.rs:901-970` — current completion logic and success message source.
  - Current onboarding/project lookup: `src-tauri/src/modules/quota.rs:117-233` — `fetch_project_id()` with `loadCodeAssist` and onboarding fallback.
  - Current polling/error branch: `src-tauri/src/modules/quota.rs:236-353` — `call_onboard_user()` and `done=true but no project_id in response` log.
  - Fallback masking point: `src-tauri/src/modules/quota.rs:548-582` — `get_valid_token_for_warmup()` uses `bamboo-precept-lgxtn` fallback.
  - Log directory source: `src-tauri/src/modules/logger.rs:17-27, 42-45` — logs live under the active data root’s `logs/app.log*`, defaulting to `~/.antigravity_tools/logs/app.log*` when `ABV_DATA_DIR` is unset.
  - Data directory/account storage: `src-tauri/src/modules/account.rs:373-412` — account JSON files live under the active data root’s `accounts/`, defaulting to `~/.antigravity_tools/accounts/` when `ABV_DATA_DIR` is unset.
  - Reference semantics: `/home/milton/antigravity-tools/CLIProxyAPI/internal/auth/antigravity/auth.go` — `FetchProjectID()` and `OnboardUser()` behavior.

  **Acceptance Criteria** (agent-executable only):
  - [ ] `npm run tauri:debug` starts successfully from `/home/milton/antigravity-tools/Antigravity-Manager`.
  - [ ] A single onboarding attempt is executed for the target account and evidence captures the exact current JSON result.
  - [ ] Evidence proves whether the current run logs `done=true but no project_id in response`.
  - [ ] Evidence proves whether the same run still emits a completion log or success-style API result afterward.
  - [ ] Evidence records the target account JSON state before any fix.

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Red-path onboarding reproduction via admin HTTP API
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Start a dedicated tmux session and run `npm run tauri:debug` from `/home/milton/antigravity-tools/Antigravity-Manager`.
      2. Resolve the active data root by checking whether `ABV_DATA_DIR` is set; otherwise use `~/.antigravity_tools`.
      3. Read `<data-root>/gui_config.json` to capture `proxy.port`, `proxy.api_key`, and `proxy.auth_mode`.
      4. Call `GET http://127.0.0.1:<port>/api/system/data-dir` with `Authorization: Bearer <api_key>` to confirm the active data root.
      5. Call `POST http://127.0.0.1:<port>/api/accounts/<account-id>/onboard` with `Authorization: Bearer <api_key>` to trigger onboarding without UI interaction.
      6. Capture the JSON response and the matching log excerpt under `<data-root>/logs/app.log*`.
    Expected: The current bug is reproducible with exact response/log evidence, including whether false-success masking occurs.
    Evidence: .sisyphus/evidence/task-1-red-onboarding-baseline.md

  Scenario: Baseline account-state capture
    Tool: Bash + Read
    Steps:
      1. Resolve the active data root by checking whether `ABV_DATA_DIR` is set; otherwise use `~/.antigravity_tools`.
      2. Identify the target account JSON at `<data-root>/accounts/<account-id>.json`.
      3. Save the current `token.project_id` and any related quota/subscription fields.
    Expected: Pre-fix persisted state is captured exactly for later diffing.
    Evidence: .sisyphus/evidence/task-1-account-baseline.md
  ```

  **Commit**: NO | Message: `n/a` | Files: []

- [x] 2. Unify backend project-resolution semantics to the reference contract

  **What to do**: Refactor backend project lookup so onboarding and runtime use one shared source of truth for: `loadCodeAssist` first, `onboardUser` only when a successful `loadCodeAssist` response still lacks project data, tolerant parsing of string/object `cloudaicompanionProject`, bounded polling, and terminal failure when `done=true` arrives without a real project. The shared contract must make result states explicit enough for onboarding and runtime to distinguish success, in-progress exhaustion, transport failure, non-200 load failure, and terminal missing-project failure.
  **Must NOT do**: Must NOT add unrelated proxy/routing cleanup, UI changes, or new feature flags. Must NOT preserve hidden fallback behavior inside the shared success contract.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: Multi-file backend refactor with behavior parity constraints.
  - Skills: [`superpowers/systematic-debugging`] — Prevent symptom-only fixes and enforce parity to the reference.
  - Omitted: [`superpowers/test-driven-development`] — No new automated tests requested.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: [3, 4, 5] | Blocked By: [1]

  **References** (executor has NO interview context — be exhaustive):
  - Current deserialization shape: `src-tauri/src/modules/quota.rs:61-93` — `LoadProjectResponse` currently only accepts `Option<String>` for `cloudaicompanionProject` in `loadCodeAssist` responses.
  - Current lookup flow: `src-tauri/src/modules/quota.rs:117-233` — fetch tier + fallback behavior.
  - Current polling flow: `src-tauri/src/modules/quota.rs:236-353` — 5 attempts, 2s delay, terminal log on missing project.
  - Quota consumer path: `src-tauri/src/modules/quota.rs:365-505` — `fetch_quota_with_cache()` accepts missing project and posts `{}` payload.
  - Runtime divergent path: `src-tauri/src/proxy/project_resolver.rs:1-54` — load-only resolver currently errors when no string project is returned.
  - Runtime fallback use: `src-tauri/src/proxy/token_manager.rs:1408-1436` — falls back to `bamboo-precept-lgxtn` if resolver fails.
  - Reference extraction contract: `/home/milton/antigravity-tools/CLIProxyAPI/internal/auth/antigravity/auth.go` — `FetchProjectID()` and `OnboardUser()` accept string/object project shapes and bounded polling semantics.

  **Acceptance Criteria** (agent-executable only):
  - [ ] Shared backend code path exists for project-resolution semantics and is used by both onboarding and runtime consumers.
  - [ ] `loadCodeAssist` parsing accepts both string and object-with-`id` project response shapes.
  - [ ] `onboardUser` is entered only after a successful `loadCodeAssist` response with missing project, not after transport/parse/non-200 load failures.
  - [ ] `done=true` without `project_id` resolves to an explicit failure outcome, not silent success.
  - [ ] Polling remains bounded and matches the reference intent (5 attempts, 2s waits only for incomplete 200 responses).
  - [ ] No hidden fallback/default project ID remains inside the shared “successful project resolution” contract.

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Build after shared-contract refactor
    Tool: Bash
    Steps:
      1. Run `cargo build --manifest-path /home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml`.
    Expected: Backend compiles after shared resolver/parser/error-contract changes.
    Evidence: .sisyphus/evidence/task-2-build.txt

  Scenario: Static parity audit
    Tool: Read
    Steps:
      1. Inspect the updated resolver/parser code paths.
      2. Confirm both onboarding and runtime references point to the same project-resolution contract.
      3. Confirm fallback/default project IDs are outside the shared success contract.
    Expected: Code inspection proves parity-critical semantics are centralized rather than duplicated.
    Evidence: .sisyphus/evidence/task-2-parity-audit.md
  ```

  **Commit**: YES | Message: `refactor(onboarding): unify project resolution semantics` | Files: ["src-tauri/src/modules/quota.rs", "src-tauri/src/proxy/project_resolver.rs", "src-tauri/src/proxy/token_manager.rs"]

- [x] 3. Remove onboarding false-success masking and lock the command contract to real project acquisition

  **What to do**: Update `onboard_account` so success means a real project was resolved under the shared reference contract, not merely that token refresh or downstream quota fetch succeeded. Ensure `done=true but no project_id in response`, bounded poll exhaustion, non-200/parse/transport `loadCodeAssist` failures, and equivalent project-acquisition failures produce a non-successful onboarding result and do not emit “Onboarding completed…” logs or success details with fallback/default project values. Standardize failure statuses so agent verification can distinguish project-resolution failures from token or quota failures.
  **Must NOT do**: Must NOT break the serialized shape of `OnboardingResult` unless absolutely required; prefer preserving `success/message/status/details` while correcting semantics. Must NOT leave any path where the UI can still show a success toast after project acquisition failure.

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: Command-contract correction affects user-visible semantics and must stay tightly aligned with backend truth.
  - Skills: [`superpowers/systematic-debugging`] — Ensures symptom masking is removed at the source rather than patched cosmetically.
  - Omitted: [`superpowers/test-driven-development`] — No new automated tests requested.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: [4, 5] | Blocked By: [1, 2]

  **References** (executor has NO interview context — be exhaustive):
  - Command result shape: `src-tauri/src/commands/mod.rs:879-885` — current `OnboardingResult` fields.
  - Current success semantics: `src-tauri/src/commands/mod.rs:901-970` — success currently follows quota fetch and logs `Onboarding completed...` even if project acquisition was previously masked.
  - HTTP API contract mirror: `src/utils/request.ts:23-24` — web/admin path uses `/api/accounts/:accountId/onboard`.
  - Shared project-resolution semantics from Task 2.
  - Reference behavior: `/home/milton/antigravity-tools/CLIProxyAPI/internal/auth/antigravity/auth.go` — `done=true` without project is error, not completion.

  **Acceptance Criteria** (agent-executable only):
  - [ ] `onboard_account` returns `success: false` for terminal project-acquisition failure states.
  - [ ] `onboard_account` exposes stable project-resolution failure statuses, at minimum distinct statuses for `project_resolution_failed`, `project_missing_after_done`, and `project_poll_exhausted` or equivalent clearly differentiated values.
  - [ ] The onboarding code no longer logs `Onboarding completed for ...` for sessions where project acquisition failed.
  - [ ] The frontend-visible outcome remains warning/error rather than success for those sessions.
  - [ ] Successful onboarding details no longer interpolate fallback/default project IDs as if they were real onboarding output.

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Green-path failure semantics after masked-error fix
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Relaunch `npm run tauri:debug` with the updated code.
      2. Resolve the active data root and read `<data-root>/gui_config.json` to obtain `proxy.port` and `proxy.api_key`.
      3. Trigger onboarding for the same failing account/session pattern via `POST http://127.0.0.1:<port>/api/accounts/<account-id>/onboard`.
      4. Capture the returned JSON status and backend log excerpt.
    Expected: The session yields a non-success onboarding result with a project-resolution-specific status when no real project_id is acquired; no completion log is emitted.
    Evidence: .sisyphus/evidence/task-3-failure-contract.md

  Scenario: Success-path details integrity
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Resolve the active data root and read `<data-root>/gui_config.json` to obtain `proxy.port` and `proxy.api_key`.
      2. Trigger onboarding on an account/path that resolves a real project_id via `POST http://127.0.0.1:<port>/api/accounts/<account-id>/onboard`.
      3. Capture the returned details and completion log.
    Expected: Success details include a real resolved project_id and do not rely on fallback/default project substitution.
    Evidence: .sisyphus/evidence/task-3-success-contract.md
  ```

  **Commit**: YES | Message: `fix(onboarding): require real project acquisition for success` | Files: ["src-tauri/src/commands/mod.rs"]

- [x] 4. Define and enforce explicit project_id persistence rules for success and failure

  **What to do**: Update persistence behavior so only valid resolved project IDs are saved as account state, while failure paths handle prior state explicitly instead of silently persisting fallback/default values. The rule is fixed: if a previously stored real `project_id` exists and fresh onboarding fails, retain that previously stored real value unchanged, surface the fresh onboarding failure, and do not treat the retained value as evidence of successful re-onboarding. Remove or harden any silent write failures relevant to onboarding-success claims.
  **Must NOT do**: Must NOT overwrite account files with fallback/default project IDs as a successful onboarding outcome. Must NOT leave persistence ambiguous after failure. Must NOT silently discard a critical write error if later logic claims the account is fully onboarded.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: Persistence consistency across account storage and runtime cache requires careful state handling.
  - Skills: [`superpowers/systematic-debugging`] — Needed to avoid introducing stale-state regressions.
  - Omitted: [`superpowers/test-driven-development`] — No new automated tests requested.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: [5] | Blocked By: [1, 2, 3]

  **References** (executor has NO interview context — be exhaustive):
  - Account data directory: `src-tauri/src/modules/account.rs:373-412` — account JSON storage location.
  - Quota persistence path: `src-tauri/src/modules/account.rs:1537-1558` — saves updated `project_id` when `fetch_quota()` returns one.
  - Retry persistence path: `src-tauri/src/modules/account.rs:1621-1643` — also saves project after retry.
  - Runtime persistence path: `src-tauri/src/proxy/token_manager.rs:1426-1433, 1902-1919` — runtime saves project_id directly to account JSON.
  - Current onboarding fallback source: `src-tauri/src/modules/quota.rs:573-582` — fallback can mask failure before downstream persistence.

  **Acceptance Criteria** (agent-executable only):
  - [ ] A failed onboarding session does not persist a new fallback/default `project_id` to the account JSON.
  - [ ] If an already-stored real `project_id` exists and fresh onboarding fails, that existing real value is retained unchanged and is not rewritten or presented as fresh onboarding success.
  - [ ] A successful onboarding session persists the resolved real `project_id` exactly once to the account JSON.
  - [ ] Any critical write failure affecting onboarding completion is surfaced in logs and does not coexist with a false success outcome.

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Account-file persistence on failure
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Resolve the active data root by checking whether `ABV_DATA_DIR` is set; otherwise use `~/.antigravity_tools`.
      2. Read the target account JSON before a failing onboarding attempt.
      3. Read `<data-root>/gui_config.json` to obtain `proxy.port` and `proxy.api_key`.
      4. Launch `npm run tauri:debug` and trigger the failing onboarding scenario via `POST http://127.0.0.1:<port>/api/accounts/<account-id>/onboard`.
      5. Re-read the same account JSON and diff `token.project_id`.
    Expected: No fallback/default project_id is newly persisted as successful onboarding state; an existing real persisted value remains unchanged if present.
    Evidence: .sisyphus/evidence/task-4-failure-persistence.md

  Scenario: Account-file persistence on success
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Resolve the active data root by checking whether `ABV_DATA_DIR` is set; otherwise use `~/.antigravity_tools`.
      2. Read the target account JSON before a successful onboarding attempt.
      3. Read `<data-root>/gui_config.json` to obtain `proxy.port` and `proxy.api_key`.
      4. Launch `npm run tauri:debug` and trigger the success onboarding scenario via `POST http://127.0.0.1:<port>/api/accounts/<account-id>/onboard`.
      5. Re-read the same account JSON and confirm the real resolved `token.project_id` is present.
    Expected: Successful onboarding persists the real project_id and only the real project_id.
    Evidence: .sisyphus/evidence/task-4-success-persistence.md
  ```

  **Commit**: YES | Message: `fix(onboarding): harden project id persistence rules` | Files: ["src-tauri/src/modules/account.rs", "src-tauri/src/proxy/token_manager.rs", "src-tauri/src/modules/quota.rs"]

- [x] 5. Align runtime token/project resolution with the onboarding parity contract and prove end-to-end behavior

  **What to do**: Update runtime/token acquisition so it follows the same shared project-resolution semantics as onboarding, including tolerated response shapes, failure handling, and fallback policy boundaries. The runtime policy is fixed: runtime may reuse an already persisted real `project_id`, but must not synthesize `bamboo-precept-lgxtn` after a failed fresh resolution on any path that would otherwise imply a healthy onboarded account. Then run full red/green verification to prove there is no longer a divergence where onboarding fails but later runtime behavior silently succeeds through `bamboo-precept-lgxtn`.
  **Must NOT do**: Must NOT change unrelated scheduling, model routing, or account-rotation logic. Must NOT broaden this into general proxy cleanup. Must NOT finish until both onboarding and runtime evidence are captured for the same account path.

  **Recommended Agent Profile**:
  - Category: `general` — Reason: Final integration across onboarding/runtime with evidence-driven validation.
  - Skills: [`superpowers/systematic-debugging`] — Keeps the verification focused on parity and regression boundaries.
  - Omitted: [`superpowers/test-driven-development`] — No new automated tests requested.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: [F1, F2, F3, F4] | Blocked By: [1, 2, 3, 4]

  **References** (executor has NO interview context — be exhaustive):
  - Runtime resolver divergence: `src-tauri/src/proxy/project_resolver.rs:1-54`.
  - Runtime fallback call site: `src-tauri/src/proxy/token_manager.rs:1408-1436`.
  - Shared resolver/parser/error contract from Task 2.
  - Onboarding command semantics from Task 3.
  - Persistence rules from Task 4.
  - Build/dev commands: `package.json:6-12`, `src-tauri/Cargo.toml:1-84`.
  - Config file location: `src-tauri/src/modules/config.rs:8-18, 102-109` — active proxy config is stored in `<data-root>/gui_config.json`.
  - Log file location: `src-tauri/src/modules/logger.rs:17-27, 42-45`.
  - Proxy startup: `src-tauri/src/commands/proxy.rs:56-135` — desktop command for starting the proxy service when needed.
  - Admin proxy status/start APIs: `src-tauri/src/proxy/server.rs:525, 538, 1441-1472` — `GET /api/proxy/status`, `POST /api/proxy/start`.
  - Proxy routes requiring token/project usage: `src-tauri/src/proxy/server.rs:371-399` — `/health`, `/v1/chat/completions`, `/v1/responses`, `/v1/messages`, `/v1/models`.
  - Runtime token consumers: `src-tauri/src/proxy/handlers/openai.rs:196-230`, `src-tauri/src/proxy/handlers/claude.rs:620-880`, `src-tauri/src/proxy/handlers/gemini.rs:135-166`, `src-tauri/src/proxy/handlers/audio.rs:100-109`.
  - Proxy auth defaults: `src-tauri/src/proxy/config.rs:469-485, 569-577`, `src-tauri/src/proxy/security.rs:13-37`, `src-tauri/src/proxy/middleware/auth.rs:56-139` — local-only auto mode resolves to `Off`, otherwise bearer `api_key` can be used for `/api` and `/v1` routes.

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cargo build --manifest-path /home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml` succeeds after all parity changes.
  - [ ] Running `npm run tauri:debug` and repeating the original failing onboarding scenario no longer produces masked success.
  - [ ] For a successfully onboarded account, a later runtime/token acquisition path uses the same resolved/persisted project semantics rather than silent fallback.
  - [ ] For a failed onboarding path, runtime logs and account state do not incorrectly imply full onboarding completion.
  - [ ] Evidence artifacts include before/after log excerpts and before/after account JSON excerpts.

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: End-to-end red/green parity verification
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Build the backend with `cargo build --manifest-path /home/milton/antigravity-tools/Antigravity-Manager/src-tauri/Cargo.toml`.
      2. Launch `npm run tauri:debug`.
      3. Resolve the active data root and read `<data-root>/gui_config.json` to obtain `proxy.port` and `proxy.api_key`.
      4. Re-run the original failing onboarding scenario via `POST http://127.0.0.1:<port>/api/accounts/<account-id>/onboard` and capture the updated outcome.
      5. Capture the current log excerpt from `<data-root>/logs/app.log*`.
      6. Capture the target account JSON from `<data-root>/accounts/<account-id>.json`.
    Expected: The original masked-success bug is gone; the outcome is now explicit failure or real success only.
    Evidence: .sisyphus/evidence/task-5-end-to-end.md

  Scenario: Runtime follow-up parity check
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Resolve the active data root and read `<data-root>/gui_config.json` to obtain `proxy.port`, `proxy.api_key`, and `proxy.auth_mode`.
      2. Call `GET http://127.0.0.1:<port>/api/proxy/status` and `POST http://127.0.0.1:<port>/api/proxy/start` with `Authorization: Bearer <api_key>` if the proxy is not already running.
      3. Send `GET http://127.0.0.1:<port>/v1/models` with `Authorization: Bearer <api_key>` when auth is enabled, or without auth when local-only `auto/off` mode applies.
      4. Inspect the resulting logs and persisted account JSON again.
    Expected: Runtime behavior matches the shared resolver contract and does not silently reintroduce fallback masking.
    Evidence: .sisyphus/evidence/task-5-runtime-followup.md
  ```

  **Commit**: YES | Message: `fix(proxy): align runtime project resolution with onboarding` | Files: ["src-tauri/src/proxy/project_resolver.rs", "src-tauri/src/proxy/token_manager.rs", "src-tauri/src/modules/quota.rs", "src-tauri/src/commands/mod.rs"]

## Final Verification Wave (MANDATORY — after ALL implementation tasks)
> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.
> **Do NOT auto-proceed after verification. Wait for user's explicit approval before marking work complete.**
> **Never mark F1-F4 as checked before getting user's okay.** Rejection or user feedback -> fix -> re-run -> present again -> wait for okay.
- [ ] F1. Plan Compliance Audit — oracle
- [ ] F2. Code Quality Review — unspecified-high
- [ ] F3. Real Manual QA — unspecified-high (+ playwright if UI)
- [ ] F4. Scope Fidelity Check — deep

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: F1 Plan Compliance Audit
    Tool: Bash + Read
    Steps:
      1. Read this plan and list the files changed by implementation.
      2. Compare changed files and evidence artifacts against each task’s required outcomes.
      3. Save a checklist showing each task requirement as pass/fail.
    Expected: The implementation can be mapped back to every promised plan task with no missing deliverables.
    Evidence: .sisyphus/evidence/f1-plan-compliance.md

  Scenario: F2 Code Quality Review
    Tool: Bash + Read
    Steps:
      1. Read the final diff/stat for changed backend files.
      2. Inspect for duplicated resolver logic, fallback leakage, ignored persistence errors, and unrelated cleanup.
      3. Record any findings and their severity.
    Expected: No unresolved important code-quality issues remain in the changed backend scope.
    Evidence: .sisyphus/evidence/f2-code-quality.md

  Scenario: F3 Real Manual QA via HTTP flows
    Tool: interactive_bash + Bash + Read
    Steps:
      1. Repeat both the red-path and green-path onboarding HTTP scenarios from Tasks 1 and 3.
      2. Run the concrete runtime follow-up proxy request from Task 5.
      3. Capture logs, statuses, and account JSON state.
    Expected: Execution evidence matches all promised onboarding/runtime outcomes.
    Evidence: .sisyphus/evidence/f3-manual-qa.md

  Scenario: F4 Scope Fidelity Check
    Tool: Bash + Read
    Steps:
      1. Compare the changed files against the plan’s declared file scope.
      2. Confirm no UI redesign, no unrelated proxy cleanup, and no new automated test scaffolding were introduced.
      3. Save a scope pass/fail summary.
    Expected: The work stayed within the approved backend parity scope.
    Evidence: .sisyphus/evidence/f4-scope-fidelity.md
  ```

## Commit Strategy
- Commit 1: centralize project-resolution parsing/error semantics.
- Commit 2: apply reference onboarding semantics to the onboarding command path.
- Commit 3: align runtime/token path with the same resolver and fallback policy.
- Commit 4: add minimal observability/contract hardening needed for agent verification.

## Success Criteria
- Onboarding no longer reports success after `done=true but no project_id in response`.
- A valid onboarding success always includes a real resolved `project_id`, persisted according to explicit rules.
- Runtime use of the same account follows the same resolution semantics and does not silently diverge.
- Evidence files capture both the pre-fix red path and post-fix green path for logs and account-file state.
