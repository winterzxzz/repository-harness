# Request Changes With Image Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users request changes on Ready work with a required reason and optional screenshot evidence, immediately launching a replacement run for the same story while preserving prior run history.

**Architecture:** Add a focused bounded-upload module for binary HTTP requests and multipart validation. Refactor run preparation so filesystem preparation happens before one atomic state transaction that inserts the replacement run and rejects the source run; then expose the operation through the local API and Ready review UI.

**Tech Stack:** Rust TCP HTTP server, rusqlite transactions, serde contracts, React 19, TypeScript, Playwright, Electron desktop smoke.

---

## File Map

- Create `crates/harness-symphony/src/upload.rs`: bounded request reader, HTTP parsing, multipart parsing, image signature validation, and upload limits.
- Modify `crates/harness-symphony/src/main.rs`: register the upload module.
- Modify `crates/harness-symphony/src/state.rs`: atomically insert a replacement run and reject the source run.
- Modify `crates/harness-symphony/src/run.rs`: feedback-aware contract, artifact writing, replacement preparation, and rollback helpers.
- Modify `crates/harness-symphony/src/agent.rs`: explicitly instruct agents to read the reason and inspect evidence files.
- Modify `crates/harness-symphony/src/web.rs`: binary routing, request-changes API, safe feedback asset serving, review metadata, and integration tests.
- Modify Web UI types, API, application state, detail panel, and Playwright tests.
- Update US-084 durable proof after verification.

### Task 1: Bounded Binary Requests And Multipart Validation

**Files:**
- Create: `crates/harness-symphony/src/upload.rs`
- Modify: `crates/harness-symphony/src/main.rs`
- Modify: `crates/harness-symphony/src/web.rs:342`
- Test: `crates/harness-symphony/src/upload.rs`

- [ ] **Step 1: Write failing upload tests**

Add tests covering the real contract:

```rust
#[test]
fn request_changes_reads_body_larger_than_legacy_buffer() {
    let body = vec![b'x'; 12_000];
    let request = request_bytes("POST", "/upload", "application/octet-stream", &body);
    let parsed = read_http_request(&mut std::io::Cursor::new(request)).unwrap();
    assert_eq!(parsed.body, body);
}

#[test]
fn request_changes_refuses_content_length_above_ceiling() {
    let request = b"POST /upload HTTP/1.1\r\nContent-Length: 20000000\r\n\r\n";
    let error = read_http_request(&mut std::io::Cursor::new(request)).unwrap_err();
    assert!(error.to_string().contains("request body exceeds"));
}

#[test]
fn request_changes_parses_reason_and_valid_png() {
    let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
    let request = multipart_request("boundary", "Fix mobile spacing", &[("proof.png", "image/png", &png)]);
    let feedback = parse_request_changes(&request).unwrap();
    assert_eq!(feedback.reason, "Fix mobile spacing");
    assert_eq!(feedback.evidence[0].extension, "png");
}

#[test]
fn request_changes_rejects_misleading_image_mime() {
    let request = multipart_request("boundary", "Fix it", &[("proof.png", "image/png", b"not png")]);
    assert!(parse_request_changes(&request).unwrap_err().to_string().contains("unsupported image signature"));
}
```

Also test empty and 2,001-character reasons, duplicate reason fields, four images, a file above 5 MB, JPEG, WebP, missing boundary, malformed headers, truncated body, and unknown multipart fields.

- [ ] **Step 2: Run RED**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
```

Expected: compilation fails because the upload APIs are absent.

- [ ] **Step 3: Implement `upload.rs`**

Expose:

```rust
pub const MAX_REASON_CHARS: usize = 2_000;
pub const MAX_EVIDENCE_FILES: usize = 3;
pub const MAX_EVIDENCE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_REQUEST_BODY_BYTES: usize = MAX_EVIDENCE_FILES * MAX_EVIDENCE_BYTES + 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceUpload {
    pub extension: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackSubmission {
    pub reason: String,
    pub evidence: Vec<EvidenceUpload>,
}

pub fn read_http_request(reader: &mut impl std::io::Read) -> Result<HttpRequest, UploadError>;
pub fn parse_http_request(bytes: &[u8]) -> Result<HttpRequest, UploadError>;
pub fn parse_request_changes(request: &HttpRequest) -> Result<FeedbackSubmission, UploadError>;
```

Read headers with a 64 KB ceiling, reject oversized `Content-Length` before
body allocation, read the exact body length, parse multipart with byte-slice
searches, and derive PNG/JPEG/WebP type from signatures instead of client MIME.

- [ ] **Step 4: Route TCP requests through the bounded reader**

Add `mod upload;` in `main.rs`. Change `handle_stream` to read an `HttpRequest`
and route through `handle_http_request`. Preserve existing string-based tests
with this wrapper:

```rust
fn handle_request(config: &ResolvedConfig, request: &str) -> Result<HttpResponse, WebError> {
    let request = crate::upload::parse_http_request(request.as_bytes())?;
    handle_http_request(config, &request)
}
```

Add `Upload(#[from] crate::upload::UploadError)` to `WebError`.

- [ ] **Step 5: Verify and commit**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony web -- --nocapture
git add crates/harness-symphony/src/upload.rs crates/harness-symphony/src/main.rs crates/harness-symphony/src/web.rs
git commit -m "feat(symphony): add bounded feedback uploads"
```

### Task 2: Atomic Replacement Run And Feedback Contract

**Files:**
- Modify: `crates/harness-symphony/src/state.rs`
- Modify: `crates/harness-symphony/src/run.rs`
- Modify: `crates/harness-symphony/src/agent.rs`
- Test: the same Rust modules

- [ ] **Step 1: Write failing state, contract, and prompt tests**

```rust
#[test]
fn request_changes_state_transition_is_atomic() {
    let store = test_store();
    add_completed_run(&store, "run_old", "US-084");
    store.replace_run("run_old", "Needs tighter spacing", new_run("run_new", "US-084")).unwrap();
    assert_eq!(store.show_run("run_old").unwrap().status, "rejected");
    assert_eq!(store.active_run().unwrap().unwrap().run_id, "run_new");
}

#[test]
fn request_changes_contract_contains_feedback_paths() {
    let prepared = prepare_replacement_run(&config, "US-084", replacement_feedback_fixture()).unwrap();
    let contract: RunContract = serde_json::from_str(&fs::read_to_string(prepared.contract_path).unwrap()).unwrap();
    let feedback = contract.request_changes.unwrap();
    assert_eq!(feedback.source_run_id, "run_old");
    assert!(feedback.reason_path.ends_with("/feedback/reason.md"));
    assert_eq!(feedback.evidence_paths.len(), 1);
}

#[test]
fn request_changes_prompt_requires_image_inspection() {
    let prompt = agent_prompt(&config, &prepared_with_feedback());
    assert!(prompt.contains("Read the request-changes reason"));
    assert!(prompt.contains("Inspect every evidence image"));
}
```

Also prove feedback exists in root/worktree run directories and failed state
commit removes incomplete prepared files.

- [ ] **Step 2: Run RED**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
```

- [ ] **Step 3: Add atomic state APIs**

```rust
pub fn replace_run(
    &self,
    source_run_id: &str,
    rejection_reason: &str,
    replacement: NewRunRecord,
) -> Result<(), StateError>;

pub fn remove_run(&self, run_id: &str) -> Result<(), StateError>;
```

`replace_run` uses one SQLite transaction: refuse another active run, verify the
source is completed, insert the prepared replacement, update the source to
`rejected`, then commit.

- [ ] **Step 4: Add feedback-aware run preparation**

```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RequestChangesContract {
    pub source_run_id: String,
    pub reason_path: String,
    pub evidence_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementFeedback {
    pub source_run_id: String,
    pub reason: String,
    pub evidence: Vec<FeedbackFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackFile {
    pub extension: String,
    pub bytes: Vec<u8>,
}

pub fn prepare_replacement_run(
    config: &ResolvedConfig,
    story_id: &str,
    feedback: ReplacementFeedback,
) -> Result<PreparedRun, RunError>;
```

Add optional `request_changes` fields to `RunContract` and `PreparedRun`. Write
generated feedback files to both root and worktree run directories before the
contract/state commit. On failure, remove worktree, branch, run directory, copied
database, and incomplete run state.

- [ ] **Step 5: Extend agent prompt and verify**

Append a feedback-specific prompt instructing the agent to read `reason.md`,
inspect every evidence path, and disclose unsupported image inspection in
`SUMMARY.md`.

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony state::tests -- --nocapture
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony run::tests -- --nocapture
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony agent::tests -- --nocapture
git add crates/harness-symphony/src/state.rs crates/harness-symphony/src/run.rs crates/harness-symphony/src/agent.rs
git commit -m "feat(symphony): prepare replacement runs with feedback"
```

### Task 3: Request Changes API And Safe Feedback Review

**Files:**
- Modify: `crates/harness-symphony/src/web.rs`
- Test: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing endpoint tests**

```rust
#[test]
fn request_changes_creates_replacement_and_preserves_source() {
    let config = ready_review_fixture();
    let response = handle_binary_request(&config, request_changes_fixture("run_old", "Fix spacing", valid_png()));
    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
    assert_eq!(store.show_run("run_old").unwrap().status, "rejected");
    assert!(store.list_runs().unwrap().iter().any(|run| run.story_id == "US-084" && run.run_id != "run_old"));
}

#[test]
fn request_changes_invalid_image_leaves_source_completed() {
    let response = handle_binary_request(&config, invalid_image_request());
    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
    assert_eq!(store.show_run("run_old").unwrap().status, "completed");
}
```

Also assert 409 for Done, stale run, non-runnable story, and active conflict;
assert review metadata contains the reason and safe evidence URL; assert the
scoped evidence GET returns exact image bytes and traversal is refused.

- [ ] **Step 2: Run RED**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
```

- [ ] **Step 3: Add routes and handler**

Add:

```text
POST /api/runs/<run_id>/request-changes
GET  /api/runs/<run_id>/feedback/<generated_filename>
```

The POST requires the matching latest run to be internal `Review` state (shown
in the Ready bucket), unsynced, and backed by a `planned` or `in_progress` story.
Map validated uploads into `ReplacementFeedback`, call
`prepare_replacement_run`, spawn only on success, and return 202:

```rust
#[derive(Debug, Serialize)]
struct RequestChangesResponse {
    source_run_id: String,
    run_id: String,
    story_id: String,
    status: String,
    feedback: RequestChangesPaths,
}
```

The GET route joins only under `.harness/runs/<run_id>/feedback`, accepts only
generated evidence filenames, and serves PNG/JPEG/WebP content types.

- [ ] **Step 4: Add review feedback metadata**

```rust
#[derive(Debug, Serialize)]
struct ReviewFeedback {
    reason: String,
    reason_path: String,
    evidence: Vec<ReviewEvidence>,
}

#[derive(Debug, Serialize)]
struct ReviewEvidence {
    path: String,
    url: String,
    content_type: String,
    size: u64,
}
```

Add `request_changes: Option<ReviewFeedback>` to `ReviewResponse`, loaded only
from the bounded feedback directory and contract metadata.

- [ ] **Step 5: Verify and commit**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony web -- --nocapture
git add crates/harness-symphony/src/web.rs
git commit -m "feat(symphony): expose request changes API"
```

### Task 4: Ready Review Form And Browser Flow

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/types.ts`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/api.ts`
- Modify: `crates/harness-symphony/web-ui/src/main.tsx`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx`
- Modify: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Write failing Playwright tests**

Replace the old reject test with:

```ts
test("ready review requests changes with reason and image evidence", async ({ page }) => {
  // Mock Review internal state in the Ready bucket.
  // Fill Request changes reason, attach a PNG buffer, assert preview/filename,
  // submit, inspect multipart fields, return an Active board item, and assert
  // the card moves to the Active column.
});
```

Add client guard tests for empty reason, four images, unsupported text file, one
file above 5 MB, removal of a selected image, and absence of Request changes on
Done detail.

- [ ] **Step 2: Run RED**

```bash
npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "request changes"
```

- [ ] **Step 3: Add TypeScript contracts and multipart API**

```ts
export type RequestChangesResponse = {
  source_run_id: string;
  run_id: string;
  story_id: string;
  status: string;
  feedback: { reason_path: string; evidence_paths: string[] };
};

export type ReviewFeedback = {
  reason: string;
  reason_path: string;
  evidence: Array<{ path: string; url: string; content_type: string; size: number }>;
};

export async function postRequestChanges(runId: string, reason: string, files: File[]): Promise<RequestChangesResponse> {
  const body = new FormData();
  body.append("reason", reason);
  files.forEach((file) => body.append("evidence", file));
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/request-changes`, { method: "POST", body });
  return readJson(response, parseRequestChangesResponse, "Request changes failed");
}
```

Parse optional historical feedback on review responses.

- [ ] **Step 4: Add application state and form**

Track `requestingChangesRunId` in `main.tsx`; on success toast the replacement
run ID and refresh the board, on failure keep the form state.

In `detail.tsx`, show only for `item.board_state === "Review"`:

- textarea `Request changes reason`, max 2,000;
- drag/drop zone `Evidence images` and multiple PNG/JPEG/WebP file input;
- object-URL thumbnails with cleanup, filename, size, remove control, `n/3`;
- inline count/type/size validation;
- button `Request changes` disabled while invalid/submitting.

Render historical reason and evidence thumbnails from scoped feedback URLs.

- [ ] **Step 5: Verify and commit**

```bash
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "request changes"
npm --prefix crates/harness-symphony/web-ui run e2e
git add crates/harness-symphony/web-ui/src crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "feat(web-ui): request changes with image evidence"
```

### Task 5: Full Verification And Durable Closure

**Files:**
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-084-request-changes-with-image-evidence/overview.md`
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-084-request-changes-with-image-evidence/validation.md`

- [ ] **Step 1: Run full proof**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test -p harness-symphony request_changes -- --nocapture
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
git diff --check
scripts/bin/harness-cli story verify US-084
```

- [ ] **Step 2: Update durable story and decision proof**

Set overview status to `implemented` and write exact validation evidence. Then:

```bash
scripts/bin/harness-cli story update --id US-084 --status implemented --unit 1 --integration 1 --e2e 1 --platform 1 --evidence "Request changes with bounded image evidence validated across Rust upload/state/API tests, browser E2E, desktop smoke, and full workspace checks."
scripts/bin/harness-cli decision verify 0008
```

- [ ] **Step 3: Record the final trace**

Use `scripts/bin/harness-cli trace` with intake `24`, story `US-084`, complete
JSON arrays for actions/read/changed/decisions/errors, any friction found, and a
note that duration/token estimates are unavailable.

- [ ] **Step 4: Commit closure evidence**

```bash
git add docs/stories/epics/E08-symphony-web-ui-controller/US-084-request-changes-with-image-evidence/overview.md docs/stories/epics/E08-symphony-web-ui-controller/US-084-request-changes-with-image-evidence/validation.md
git commit -m "docs: close request changes story"
```

- [ ] **Step 5: Final clean verification**

```bash
scripts/bin/harness-cli story verify US-084
git status --short
```

Expected: story verification passes and the worktree is clean.
