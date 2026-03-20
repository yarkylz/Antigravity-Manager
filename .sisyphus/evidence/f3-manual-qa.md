# F3 manual QA execution review

## Scope and standard used

This review was executed as a Final Verification Wave F3 manual-QA pass only.

Required scope from the plan:

- repeat the onboarding HTTP/manual flows from Tasks 1 and 3
- run the runtime follow-up flow from Task 5
- capture what ran, what was blocked, and whether execution evidence matches the promised onboarding/runtime outcomes

This report distinguishes between:

- **fresh execution in this session**
- **inherited evidence from Tasks 1-5**

## Fresh environment checks that ran in this session

### 1. Current checkout identity

Fresh reads in this session showed:

- current repo `package.json` version: `4.1.30`
- active plan: `.sisyphus/plans/onboarding-project-id-parity.md`

### 2. Reachable host service identity

Fresh commands run:

```bash
curl -i --max-time 10 http://127.0.0.1:8045/health
curl -i --max-time 10 http://127.0.0.1:8045/api/health
```

Observed result from both:

```http
HTTP/1.1 200 OK
{"status":"ok","version":"4.1.28"}
```

Interpretation supported by evidence:

- a service is reachable on `127.0.0.1:8045`
- that service reports version `4.1.28`
- the checked-out repo is version `4.1.30`
- therefore the reachable service is **not proven to be the current working tree under review**

### 3. Reachable service data root and auth

Fresh checks:

```bash
curl -i --max-time 10 http://127.0.0.1:8045/api/system/data-dir
curl -sS -i --max-time 15 -H "Authorization: Bearer MYSUPERSECUREWEBPASSWORD" http://127.0.0.1:8045/api/system/data-dir
```

Observed result:

- unauthenticated request returned `401 Unauthorized`
- authenticated request returned:

```http
HTTP/1.1 200 OK
"/root/.antigravity_tools"
```

Fresh readable config from `/home/milton/.antigravity_tools/gui_config.json` showed:

- `proxy.port = 8045`
- `proxy.auth_mode = all_except_health`
- `proxy.api_key = sk-123apiKey228LOLXD`
- `proxy.admin_password = MYSUPERSECUREWEBPASSWORD`

### 4. Current worktree launch attempt

Fresh command run:

```bash
npm run tauri:debug
```

Observed blocker:

```text
error: failed to write `/home/milton/antigravity-tools/Antigravity-Manager/src-tauri/target/debug/.fingerprint/...`
Caused by:
  Permission denied (os error 13)
```

This is the same root-owned `src-tauri/target` blocker already documented in prior evidence. Because of this failure, a fresh desktop/app run from the current checkout could **not** be started in this session.

## Fresh onboarding-flow attempts in this session

### 1. Account inventory on the reachable host service

Fresh command run:

```bash
curl -sS -i --max-time 15 -H "Authorization: Bearer MYSUPERSECUREWEBPASSWORD" http://127.0.0.1:8045/api/accounts
```

Observed facts:

- authenticated admin account listing succeeded
- the reachable host service currently has `14` accounts
- current account id is `294806ba-6f8c-46f9-896b-ecac3ea8db68`
- the Task 1 baseline account `9502583d-8eaf-4464-bfa7-271dde305e56` is **not present** on this reachable service

This means the original baseline onboarding account/path from Task 1 could not be replayed on the only reachable host service.

### 2. Concrete onboarding endpoint replay attempt

Fresh command run against the current account on the reachable host service:

```bash
curl -sS -i --max-time 20 -X POST \
  -H "Authorization: Bearer MYSUPERSECUREWEBPASSWORD" \
  http://127.0.0.1:8045/api/accounts/294806ba-6f8c-46f9-896b-ecac3ea8db68/onboard
```

Observed result:

```http
HTTP/1.1 405 Method Not Allowed
allow: GET,HEAD
```

What this proves:

- the reachable host service does **not** provide a usable `POST /api/accounts/:accountId/onboard` replay path for this session
- no onboarding JSON contract was produced in this session
- no fresh success/failure onboarding semantics could be verified live on the reachable service

### 3. Log/account-file capture blockers for onboarding

Fresh direct reads attempted:

- `/root/.antigravity_tools/accounts`
- `/root/.antigravity_tools/logs`

Observed result for both:

```text
EACCES: permission denied
```

Therefore this session could not directly inspect:

- root-owned account JSON state
- root-owned log files
- persisted `token.project_id`

## Fresh runtime follow-up attempts in this session

### 1. Runtime proxy status

Fresh command run:

```bash
curl -sS -i --max-time 20 -H "Authorization: Bearer MYSUPERSECUREWEBPASSWORD" http://127.0.0.1:8045/api/proxy/status
```

Observed result:

```http
HTTP/1.1 200 OK
{"running":true,"port":8045,"base_url":"http://127.0.0.1:8045","active_accounts":14}
```

### 2. Runtime proxy start call

Fresh command run:

```bash
curl -sS -i --max-time 20 -X POST -H "Authorization: Bearer MYSUPERSECUREWEBPASSWORD" http://127.0.0.1:8045/api/proxy/start
```

Observed result:

```http
HTTP/1.1 200 OK
content-length: 0
```

This shows the reachable host service accepted the runtime start request, but this is still evidence from the stale `4.1.28` host service, not from the reviewed `4.1.30` checkout.

### 3. Runtime `/v1/models` request

Fresh command run:

```bash
curl -sS -i --max-time 20 -H "Authorization: Bearer sk-123apiKey228LOLXD" http://127.0.0.1:8045/v1/models
```

Observed result:

- `HTTP/1.1 200 OK`
- a model list payload was returned successfully

What this proves:

- the reachable host runtime path answers `/v1/models`
- API-key auth works for that host runtime path

What it does **not** prove:

- it does not prove onboarding/runtime parity for the current checkout under review
- it does not prove the post-fix runtime behavior for the same account path required by the plan
- it does not prove absence of fallback masking in the reviewed code path, because the host service is version-mismatched and the onboarding replay was unavailable there

## Inherited evidence considered from Tasks 1-5

These previously captured artifacts were reviewed in this session and factored into the final decision:

- `.sisyphus/evidence/task-1-red-onboarding-baseline.md`
- `.sisyphus/evidence/task-1-account-baseline.md`
- `.sisyphus/evidence/task-2-parity-audit.md`
- `.sisyphus/evidence/task-3-failure-contract.md`
- `.sisyphus/evidence/task-3-success-contract.md`
- `.sisyphus/evidence/task-4-failure-persistence.md`
- `.sisyphus/evidence/task-4-success-persistence.md`
- `.sisyphus/evidence/task-5-end-to-end.md`
- `.sisyphus/evidence/task-5-runtime-followup.md`

Key inherited facts:

- Task 1 captured a real historical bad run where `onboardUser: done=true but no project_id in response` was followed by `Onboarding completed ...`
- Tasks 2-5 provide mainly code-contract evidence that the false-success and fallback paths were refactored away
- Tasks 4-5 already documented that live host verification remained blocked by:
  - root-owned `src-tauri/target` artifacts
  - missing `pkg-config` / GTK native dependencies for isolated Rust verification
  - root-owned runtime data and logs

## QA outcome against the plan promise

Plan expectation for F3:

- repeat both onboarding flows
- run runtime follow-up flow
- capture logs/statuses/account JSON state
- confirm execution evidence matches promised onboarding/runtime outcomes

What was honestly achieved in this session:

- **runtime follow-up on a stale host service:** partial success
  - `/api/proxy/status` responded
  - `/api/proxy/start` responded
  - `/v1/models` responded
- **current worktree launch:** failed
  - blocked by root-owned `src-tauri/target`
- **onboarding replay against current worktree:** not executed
  - current worktree could not launch
- **onboarding replay against reachable host service:** failed / not representative
  - baseline account absent
  - attempted onboarding endpoint returned `405 Method Not Allowed`
- **log and account JSON capture:** blocked
  - `/root/.antigravity_tools` permission denied

## Approval analysis

Reasons this F3 wave cannot approve manual QA:

1. The only reachable runtime service in this session is version `4.1.28`, not the reviewed checkout version `4.1.30`.
2. The reviewed checkout could not be launched due to the root-owned `src-tauri/target` permission blocker.
3. The plan-required onboarding HTTP replay could not be completed on either target:
   - not on the current checkout because it never launched
   - not on the reachable host service because the baseline account was absent and the attempted onboarding POST returned `405 Method Not Allowed`
4. Root-owned `/root/.antigravity_tools` prevented the required account JSON and log-file validation.
5. Existing Tasks 2-5 evidence is largely code-contract evidence, not a fresh end-to-end manual QA replay proving the promised onboarding/runtime outcomes in this environment.

## Final judgment

Available evidence is **insufficient** to approve the F3 real manual-QA gate. There is useful partial runtime evidence from a stale host instance, but the required fresh onboarding/runtime replay for the reviewed checkout was not completed and key validation surfaces remained blocked.

VERDICT: REJECT
