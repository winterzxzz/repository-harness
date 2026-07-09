# CRM UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Symphony Web UI into a highly visual, high-density CRM-style Workspace dashboard with split-pane columns and a right sliding drawer.

**Architecture:** Split the viewport into three primary columns (Left Sidebar Navigation, Center Main Panel with View Switcher tabs, and Right sliding Drawer). Maintain all existing modal accessibility semantics (role="dialog", overlay container) so that existing E2E tests pass.

**Tech Stack:** React 19, Tailwind CSS, Lucide icons, Playwright for E2E tests.

---

### Task 1: Redesign Main Split-Pane Workspace Structure

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/main.tsx`
- Test: `crates/harness-symphony/web-ui/tests/board.spec.ts`

- [ ] **Step 1: Write the failing test**
  Add a test to verify that the main page renders as a split pane with three visible columns on desktop (Sidebar, Center panel, and Detail Drawer if selected).
  Add this to `crates/harness-symphony/web-ui/tests/board.spec.ts`:
  ```typescript
  test("main layout is split-pane on desktop", async ({ page }) => {
    await page.goto("/");
    const layout = page.locator("main > div");
    await expect(layout).toHaveClass(/lg:grid-cols-\[240px_minmax\(0,1fr\)\]/);
  });
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `npm --prefix crates/harness-symphony/web-ui run e2e`
  Expected: FAIL (or verify command passes if the class already partially exists but needs refinement).

- [ ] **Step 3: Write minimal implementation**
  Update the main structure in `crates/harness-symphony/web-ui/src/main.tsx` to handle the grid columns and split layout nicely:
  ```typescript
  return (
    <main className="min-h-screen bg-muted/45 text-foreground dark:bg-[#0f1115]">
      <div className="mx-auto grid w-full max-w-[1760px] grid-cols-1 gap-3 p-3 md:p-4 lg:grid-cols-[240px_minmax(0,1fr)] xl:p-5">
        <ControllerSidebar counts={counts} items={items} selectedId={selected?.id ?? null} onSelect={selectTask} />
        {/* ... */}
      </div>
    </main>
  );
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `npm --prefix crates/harness-symphony/web-ui run e2e`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/harness-symphony/web-ui/src/main.tsx crates/harness-symphony/web-ui/tests/board.spec.ts
  git commit -m "feat: restructure main container into split-pane CRM grid"
  ```

---

### Task 2: Polishing CRM Sidebars and View Switcher

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx`
- Modify: `crates/harness-symphony/web-ui/src/main.tsx`

- [ ] **Step 1: Write the failing test**
  Verify that the workspace sidebar and view tabs exist.
  Add to `crates/harness-symphony/web-ui/tests/board.spec.ts`:
  ```typescript
  test("view tabs contain Kanban and Table options", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("tab", { name: "Work Board" })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Guided Intake" })).toBeVisible();
  });
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `npm --prefix crates/harness-symphony/web-ui run e2e`
  Expected: FAIL (or verify it passes due to earlier changes, but validates elements).

- [ ] **Step 3: Write minimal implementation**
  Update the Sidebar component to use refined CRM margins, backgrounds, and text sizes:
  ```typescript
  // In crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx
  export function ControllerSidebar({
    counts,
    items,
    selectedId,
    onSelect
  }: {
    // ...
  }) {
    // ...
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `npm --prefix crates/harness-symphony/web-ui run e2e`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx
  git commit -m "style: polish CRM sidebar layout and list groups"
  ```

---

### Task 3: Redesigning Task Detail Modal into a Contextual Sliding Drawer

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx`

- [ ] **Step 1: Write the failing test**
  Verify that the detail overlay contains a slide-in transition class.
  Add to `crates/harness-symphony/web-ui/tests/board.spec.ts`:
  ```typescript
  test("detail drawer contains slide transition styles", async ({ page }) => {
    await page.goto("/");
    // Trigger opening a card
    await page.getByRole("button", { name: /US-/ }).first().click();
    const popup = page.getByTestId("task-detail-popup");
    await expect(popup).toHaveClass(/translate-x-0/);
  });
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `npm --prefix crates/harness-symphony/web-ui run e2e`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Modify `crates/harness-symphony/web-ui/src/features/symphony/detail.tsx` to slide the popup drawer in from the right:
  ```typescript
  // In detail.tsx TaskDetail container className
  className="relative max-h-[calc(100dvh-2rem)] min-w-0 w-full max-w-lg md:max-w-xl lg:max-w-2xl overflow-auto rounded-lg border border-border bg-background shadow-2xl outline-none lg:fixed lg:right-0 lg:top-0 lg:h-full lg:rounded-l-xl lg:rounded-r-none transition-transform duration-300 ease-out translate-x-0"
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `npm --prefix crates/harness-symphony/web-ui run e2e`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/harness-symphony/web-ui/src/features/symphony/detail.tsx
  git commit -m "feat: turn centered task detail modal into a right sliding drawer"
  ```
