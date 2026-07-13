# Symphony Board Status Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Present Symphony's four board statuses as Planned, Agent working, Human review, and Done without changing the existing task lifecycle or internal state machine.

**Architecture:** Keep `BoardBucket` and `bucketForItem` as stable internal contracts. Centralize all user-facing bucket labels, descriptions, icons, and tones in presentation metadata, then consume that metadata in the status rail, board headers, sidebar, accessible names, and tests. The backend response and seven-step `ActiveTaskFlow` remain untouched.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, lucide-react, Playwright, Rust workspace validation.

---

## File Map

- Modify `crates/harness-symphony/web-ui/src/features/symphony/constants.ts`: define shared user-facing bucket presentation metadata while retaining stable internal keys and grouping.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/board.tsx`: render ownership labels and descriptions in the command rail and board headers.
- Modify `crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx`: use the same ownership labels and stable internal anchors.
- Modify `crates/harness-symphony/web-ui/tests/board.spec.ts`: prove visible labels, exception placement, unchanged lifecycle order, and responsive bounds.
- Modify `docs/stories/US-091-symphony-board-status-ownership.md`: record final validation evidence only after implementation passes.

### Task 1: Lock the visible contract with failing browser tests

**Files:**
- Modify: `crates/harness-symphony/web-ui/tests/board.spec.ts:61-122`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Replace the old column-label assertions with the approved ownership contract**

In `test("board renders task columns and detail controls", ...)`, assert the four visible columns and status-rail labels:

```ts
const statusRail = page.getByRole("region", { name: "Command status rail" });
await expect(statusRail.getByText("Planned", { exact: true })).toBeVisible();
await expect(statusRail.getByText("Agent working", { exact: true })).toBeVisible();
await expect(statusRail.getByText("Human review", { exact: true })).toBeVisible();
await expect(statusRail.getByText("Done", { exact: true })).toBeVisible();

await expect(page.getByRole("region", { name: "Planned column" })).toBeVisible();
await expect(page.getByRole("region", { name: "Agent working column" })).toBeVisible();
await expect(page.getByRole("region", { name: "Human review column" })).toBeVisible();
await expect(page.getByRole("region", { name: "Done column" })).toBeVisible();
```

- [ ] **Step 2: Assert ownership microcopy and stable internal grouping**

Add focused assertions against the existing board fixture:

```ts
await expect(page.getByRole("region", { name: "Planned column" })).toContainText("Ready to start");
await expect(page.getByRole("region", { name: "Agent working column" })).toContainText("Codex owns the next action");
await expect(page.getByRole("region", { name: "Human review column" })).toContainText("Waiting for your decision");
await expect(page.getByRole("region", { name: "Done column" })).toContainText("Accepted and synchronized");

await expect(page.getByRole("region", { name: "Planned column" }).getByRole("button", { name: /US-052/ })).toBeVisible();
```

- [ ] **Step 3: Preserve the lifecycle contract in the same test suite**

Keep the existing lifecycle assertions and add an exact ordered-label check if one is not already present:

```ts
const lifecycle = page.getByRole("region", { name: "Active task lifecycle" });
await expect(lifecycle).toContainText("StartAgentValidationPull requestReview & mergeSyncDone");
```

- [ ] **Step 4: Run the focused test and verify RED**

Run:

```bash
npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "board renders task columns and detail controls"
```

Expected: FAIL because `Planned column`, `Agent working column`, and `Human review column` do not exist yet.

- [ ] **Step 5: Commit the failing contract test**

```bash
git add crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "test: define ownership-oriented board statuses"
```

### Task 2: Centralize bucket presentation metadata

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/constants.ts:1-40`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Add one typed presentation contract keyed by internal bucket**

Keep `buckets` and `bucketForItem` unchanged. Replace the standalone `bucketIcon` export with metadata that includes the icon:

```ts
import { CheckCircle2, Circle, GitPullRequestArrow, Loader2, type LucideIcon } from "lucide-react";

export type BucketPresentation = {
  label: string;
  description: string;
  icon: LucideIcon;
};

export const bucketPresentation: Record<BoardBucket, BucketPresentation> = {
  Drafts: {
    label: "Planned",
    description: "Ready to start · blocked work stays visible",
    icon: Circle
  },
  Active: {
    label: "Agent working",
    description: "Codex owns the next action",
    icon: Loader2
  },
  Ready: {
    label: "Human review",
    description: "Waiting for your decision",
    icon: GitPullRequestArrow
  },
  Done: {
    label: "Done",
    description: "Accepted and synchronized",
    icon: CheckCircle2
  }
};
```

- [ ] **Step 2: Add a visible-label helper without changing anchor identity**

```ts
export function bucketLabel(bucket: BoardBucket): string {
  return bucketPresentation[bucket].label;
}

export function bucketId(bucket: BoardBucket): string {
  return `bucket-${bucket.toLowerCase().replace(/\s+/g, "-")}`;
}
```

The `bucketId` implementation intentionally continues to use internal keys so existing anchors remain stable.

- [ ] **Step 3: Build to expose incomplete consumers**

Run:

```bash
npm --prefix crates/harness-symphony/web-ui run build
```

Expected: TypeScript reports imports of removed `bucketIcon` until Task 3 updates consumers.

- [ ] **Step 4: Commit the presentation model with the consumer change in Task 3**

Do not commit a TypeScript-breaking intermediate state. Continue directly to Task 3.

### Task 3: Render ownership semantics across all status surfaces

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/board.tsx:1-150`
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx:1-75`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Consume shared metadata in the command status rail**

Import `bucketPresentation`. Build metrics from stable bucket keys so count lookup remains unchanged:

```ts
const metrics = buckets.map((bucket) => {
  const presentation = bucketPresentation[bucket];
  const isActive = bucket === "Active";
  return {
    bucket,
    label: presentation.label,
    value: isActive && activeRun?.id ? activeRun.id : `${counts[bucket]} ${presentation.label.toLowerCase()}`,
    detail: isActive && activeRun?.active_run
      ? `${activeRun.active_run} is the only task allowed in progress.`
      : presentation.description,
    icon: presentation.icon
  };
});
```

Retain the current semantic rail classes by keying style selection with `metric.bucket`, not the new label. Retain reduced-motion behavior by checking `metric.bucket === "Active"` before applying `motion-safe:animate-pulse`.

- [ ] **Step 2: Render label and microcopy in each board header**

Inside the `buckets.map` callback:

```tsx
const presentation = bucketPresentation[bucket];
const Icon = presentation.icon;
```

Use visible text for accessibility while retaining the stable internal `id`:

```tsx
<section
  key={bucket}
  id={bucketId(bucket)}
  aria-label={`${presentation.label} column`}
>
  <div className="flex min-h-14 items-start justify-between gap-2 border-b border-border bg-card/60 px-3 py-2">
    <div className="flex min-w-0 items-start gap-2">
      <span className={cn("mt-0.5 grid size-6 shrink-0 place-items-center rounded-md border", bucketIconClass[bucket])}>
        <Icon className={cn("size-3.5", bucket === "Active" && activeRunId && "motion-safe:animate-spin")} />
      </span>
      <div className="min-w-0">
        <h2 className="text-sm font-bold tracking-tight text-foreground">{presentation.label}</h2>
        <p className="truncate text-[10px] font-medium text-muted-foreground">{presentation.description}</p>
      </div>
    </div>
    <Badge tone={bucketTone[bucket]}>{bucketItems.length}</Badge>
  </div>
  <div aria-label={`${presentation.label} tasks`}>{/* existing task list */}</div>
</section>
```

- [ ] **Step 3: Update sidebar labels without changing anchors or counts**

Replace four repeated sidebar items with the stable bucket list:

```tsx
{buckets.map((bucket) => (
  <SidebarItem
    key={bucket}
    href={`#${bucketId(bucket)}`}
    label={bucketPresentation[bucket].label}
    count={String(counts[bucket])}
  />
))}
```

- [ ] **Step 4: Update all test locators that refer to old accessible column names**

Apply these exact locator substitutions throughout `board.spec.ts`:

```text
Drafts column -> Planned column
Active column -> Agent working column
Ready column -> Human review column
Done column -> Done column
```

Do not change fixture `board_state` values or internal bucket expectations.

- [ ] **Step 5: Run build and focused E2E to verify GREEN**

```bash
npm --prefix crates/harness-symphony/web-ui run build
npm --prefix crates/harness-symphony/web-ui run e2e -- --grep "board renders task columns and detail controls"
```

Expected: TypeScript build succeeds and the focused Playwright test passes.

- [ ] **Step 6: Commit the shared presentation implementation**

```bash
git add crates/harness-symphony/web-ui/src/features/symphony/constants.ts \
  crates/harness-symphony/web-ui/src/features/symphony/board.tsx \
  crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx \
  crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "feat: clarify Symphony board status ownership"
```

### Task 4: Prove exceptions and responsive behavior

**Files:**
- Modify: `crates/harness-symphony/web-ui/tests/board.spec.ts:430-490,1040-1080,1200-1240`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Assert Blocked remains inside Planned**

Use an existing or locally extended board fixture containing a blocked item:

```ts
const planned = page.getByRole("region", { name: "Planned column" });
await expect(planned.getByTestId("task-card").filter({ hasText: "Blocked" })).toBeVisible();
```

If the fixture exposes only the blocker count, assert the existing `Start disabled` and blocker copy instead:

```ts
await expect(planned.getByText("Start disabled")).toBeVisible();
await expect(planned.getByText(/blocker/)).toBeVisible();
```

- [ ] **Step 2: Assert Needs Attention remains inside Agent working**

```ts
const agentWorking = page.getByRole("region", { name: "Agent working column" });
await expect(agentWorking.getByTestId("task-card").filter({ hasText: "Needs Attention" })).toBeVisible();
```

Retain existing failure-summary and recovery-action assertions so semantic parent styling cannot hide the exception.

- [ ] **Step 3: Update responsive locators and retain numeric layout bounds**

Rename local variables such as `draftsColumn` to `plannedColumn` and keep the existing minimum width, card height, internal scrolling, and no-overflow assertions:

```ts
const plannedColumn = page.getByRole("region", { name: "Planned column" });
const plannedColumnBox = await plannedColumn.boundingBox();
expect(plannedColumnBox?.width ?? 0, "Planned column keeps readable action width").toBeGreaterThanOrEqual(220);
```

- [ ] **Step 4: Run the full browser suite**

```bash
npm --prefix crates/harness-symphony/web-ui run e2e
```

Expected: all Playwright tests pass with no old accessible bucket labels.

- [ ] **Step 5: Commit exception and responsive coverage**

```bash
git add crates/harness-symphony/web-ui/tests/board.spec.ts
git commit -m "test: cover ownership status exceptions"
```

### Task 5: Validate, update evidence, and prepare review

**Files:**
- Modify: `docs/stories/US-091-symphony-board-status-ownership.md:55-65`

- [ ] **Step 1: Query equipped validation tools before optional checks**

```bash
scripts/bin/harness-cli query tools --capability design-validation --status present
scripts/bin/harness-cli query tools --capability platform-smoke --status present
```

Expected: run each optional check only when its capability is present; record a clean skip otherwise.

- [ ] **Step 2: Run Web UI and Rust validation**

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE sh -c '
  npm --prefix crates/harness-symphony/web-ui run build &&
  npm --prefix crates/harness-symphony/web-ui run e2e &&
  cargo test -p harness-symphony web -- --nocapture &&
  cargo test --workspace &&
  cargo fmt --check &&
  cargo clippy --workspace -- -D warnings &&
  git diff --check
'
```

Expected: every command exits zero and validation fixtures cannot write
operations into the live Symphony changeset.

- [ ] **Step 3: Run equipped visual/platform checks**

When present:

```bash
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE node .agents/skills/impeccable/scripts/detect.mjs --json \
  crates/harness-symphony/web-ui/src/features/symphony/constants.ts \
  crates/harness-symphony/web-ui/src/features/symphony/board.tsx \
  crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE npm --prefix crates/harness-symphony/web-ui run desktop:smoke
```

Expected: detector returns `[]`; desktop smoke exits zero. If the registry reports an absent capability, record the skip in story evidence.

- [ ] **Step 4: Record exact evidence and durable proof**

Replace the current Evidence sentence in `docs/stories/US-091-symphony-board-status-ownership.md` with the executed commands and results, then run:

```bash
scripts/bin/harness-cli story update --id US-091 --status implemented --unit 1 --integration 1 --e2e 1 --platform 1 --evidence "Ownership status metadata, unchanged lifecycle regression, responsive Playwright coverage, Web build, Rust workspace tests, clippy, design validation, desktop smoke, and diff check passed."
env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE scripts/bin/harness-cli story verify US-091
```

If platform smoke was cleanly unavailable, set `--platform 0` and say why in evidence.

After durable writes, inspect the changeset without rewriting it:

```bash
head -1 ".harness/changesets/${HARNESS_RUN_ID}.changeset.jsonl"
```

Expected: the first record has `"op":"changeset.header"` and the current
`run_id`. Never filter or reconstruct a Harness-generated changeset by hand.

- [ ] **Step 5: Record the implementation trace**

```bash
scripts/bin/harness-cli trace \
  --summary "Implemented Symphony board status ownership" \
  --intake 29 \
  --story US-091 \
  --agent codex \
  --outcome completed \
  --actions "Added shared bucket presentation metadata; updated status rail, board, sidebar, accessible labels, and tests" \
  --read "docs/superpowers/specs/2026-07-13-symphony-board-status-ownership-design.md,crates/harness-symphony/web-ui/src/features/symphony/constants.ts,crates/harness-symphony/web-ui/src/features/symphony/board.tsx,crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx,crates/harness-symphony/web-ui/tests/board.spec.ts" \
  --changed "crates/harness-symphony/web-ui/src/features/symphony/constants.ts,crates/harness-symphony/web-ui/src/features/symphony/board.tsx,crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx,crates/harness-symphony/web-ui/tests/board.spec.ts,docs/stories/US-091-symphony-board-status-ownership.md" \
  --decisions "Preserved internal bucket keys and seven-step lifecycle; exposed ownership-oriented labels only" \
  --friction "none" \
  --notes "All required validation passed."
```

- [ ] **Step 6: Commit evidence and story state**

```bash
git add docs/stories/US-091-symphony-board-status-ownership.md
git commit -m "docs: record Symphony status validation"
```

- [ ] **Step 7: Hand off the completed branch for review**

Confirm `git status --short` contains no unintended files, then report the implementation commits, validation results, and any cleanly skipped optional proof.
