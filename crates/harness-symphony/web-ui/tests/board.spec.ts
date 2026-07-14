import { expect, test, type Locator, type Page } from "@playwright/test";

function boardItem(id: string, title: string, board_state: string) {
  return {
    id,
    title,
    board_state,
    story_status: board_state === "Done" ? "implemented" : "planned",
    lane: "normal",
    verify: "configured",
    blockers: [],
    unblocks: [],
    parent_id: null,
    children: [],
    hierarchy_depth: 0,
    run_id: null,
    active_run: null,
    reason: board_state === "Ready" ? "ready" : "story visible on the board",
    failure_summary: null,
    recovery_action: null
  };
}

const flowStepIds = ["start", "agent", "validation", "pr", "review", "sync", "done"] as const;

function taskFlow(current: (typeof flowStepIds)[number] = "agent") {
  const currentIndex = flowStepIds.indexOf(current);
  return {
    story_id: "US-090",
    title: "Symphony Active Task Lifecycle Flow",
    state: current === "done" ? "done" : "active",
    current_step: current,
    message: current === "agent" ? "Agent is implementing the task." : "Task lifecycle updated.",
    steps: flowStepIds.map((id, index) => ({
      id,
      state: index < currentIndex ? "complete" : index === currentIndex ? "current" : "pending"
    })),
    recovery_action: null
  };
}

async function expectNoHorizontalOverflow(locator: Locator, label: string) {
  const overflow = await locator.evaluate(
    (element) => Math.ceil(element.scrollWidth) - Math.ceil(element.clientWidth)
  );
  expect(overflow, `${label} horizontal overflow`).toBeLessThanOrEqual(1);
}

async function expectPageNoHorizontalOverflow(page: Page) {
  const overflow = await page.evaluate(
    () => Math.ceil(document.documentElement.scrollWidth) - Math.ceil(window.innerWidth)
  );
  expect(overflow, "page horizontal overflow").toBeLessThanOrEqual(1);
}

async function expectReadableTaskCard(locator: Locator, label: string) {
  const box = await locator.boundingBox();
  expect(box?.height ?? 0, `${label} readable card height`).toBeGreaterThanOrEqual(124);
}

test("board renders task columns and detail controls", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [boardItem("US-052", "Sync Approval And Done Transition", "Ready")]
      })
    });
  });
  await page.route("**/api/tasks/US-052/context", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ story_id: "US-052", content: "# Context" }) });
  });

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Symphony Command Center" })).toBeVisible();
  await expect(page.getByText("Local operations surface")).toBeVisible();
  await expect(page.getByRole("region", { name: "Command status rail" })).toBeVisible();
  const statusRail = page.getByRole("region", { name: "Command status rail" });
  await expect(statusRail.getByText("Planned", { exact: true })).toBeVisible();
  await expect(statusRail.getByText("Agent working", { exact: true })).toBeVisible();
  await expect(statusRail.getByText("Human review", { exact: true })).toBeVisible();
  await expect(statusRail.getByText("Done", { exact: true })).toBeVisible();
  await expect(page.locator("#board")).toHaveClass(/command-board-surface/);
  await expect(page.getByRole("complementary", { name: "Workspace navigation" })).toBeVisible();
  const statusNavigation = page.getByRole("navigation", { name: "Status" });
  await expect(statusNavigation.getByText("Planned", { exact: true })).toBeVisible();
  await expect(statusNavigation.getByText("Agent working", { exact: true })).toBeVisible();
  await expect(statusNavigation.getByText("Human review", { exact: true })).toBeVisible();
  await expect(statusNavigation.getByText("Done", { exact: true })).toBeVisible();
  const planned = page.getByRole("region", { name: "Planned column" });
  const agentWorking = page.getByRole("region", { name: "Agent working column" });
  const humanReview = page.getByRole("region", { name: "Human review column" });
  const done = page.getByRole("region", { name: "Done column" });
  await expect(planned).toContainText("Ready to start");
  await expect(agentWorking).toContainText("Codex owns the next action");
  await expect(humanReview).toContainText("Waiting for your decision");
  await expect(done).toContainText("Accepted and synchronized");
  await expect(page.getByRole("heading", { name: "Blocked", exact: true })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "In Progress", exact: true })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Needs Attention", exact: true })).toHaveCount(0);

  await page.getByRole("textbox", { name: "Find task" }).fill("US-052");
  await expect(planned.getByRole("button", { name: /US-052/ })).toBeVisible();
  const lifecycle = page.getByRole("region", { name: "Active task lifecycle" });
  const lifecycleLabels = await lifecycle.getByRole("listitem").evaluateAll((items) =>
    items.map((item) => item.textContent?.replace(/(complete|current|failed|pending)$/, ""))
  );
  expect(lifecycleLabels).toEqual(["Start", "Agent", "Validation", "Pull request", "Review & merge", "Sync", "Done"]);
  await page.getByRole("button", { name: /US-052/ }).click();

  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(page.getByTestId("task-detail-overlay")).toHaveCSS("position", "fixed");
  await expect(
    detail.getByRole("heading", { name: "Sync Approval And Done Transition" })
  ).toBeVisible();
  await expect(page.getByText("Blocked by")).toBeVisible();
  await expect(page.getByText("Unblocks")).toBeVisible();
  await expect(detail.getByText("Hierarchy")).toBeVisible();
  await expect(detail.getByRole("button", { name: /Start/ })).toBeVisible();
});

test("active task lifecycle is always visible above the command rail", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [], task_flow: null })
    });
  });

  await page.goto("/");

  const flow = page.getByRole("region", { name: "Active task lifecycle" });
  const rail = page.getByRole("region", { name: "Command status rail" });
  await expect(flow).toBeVisible();
  await expect(flow).toContainText("No task is currently running");
  await expect(flow.getByRole("listitem")).toHaveCount(7);
  expect(await flow.evaluate((node, other) => Boolean(node.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING), await rail.elementHandle())).toBe(true);
});

test("active task lifecycle marks the current step", async ({ page }) => {
  const item = boardItem("US-090", "Symphony Active Task Lifecycle Flow", "In Progress");
  item.run_id = "run_us_090";
  item.active_run = "run_us_090";
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [item], task_flow: taskFlow("agent") })
    });
  });

  await page.goto("/");

  const flow = page.getByRole("region", { name: "Active task lifecycle" });
  await expect(flow).toContainText("US-090");
  await expect(flow.getByRole("listitem").nth(1)).toHaveAttribute("aria-current", "step");
  await expect(flow.getByText("Agent is implementing the task.")).toBeVisible();
});

test("status rail bounds a long active run identifier", async ({ page }) => {
  const item = boardItem("US-093", "Agent Runtime Observability And Recovery", "In Progress");
  item.run_id = "run_1783999475145922000_45510_0";
  item.active_run = item.run_id;
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [item], task_flow: taskFlow("agent") })
    });
  });

  await page.goto("/");

  const detail = page
    .getByRole("region", { name: "Command status rail" })
    .getByText("is the only task allowed in progress.");
  await expect(detail).toBeVisible();
  await expectNoHorizontalOverflow(detail, "active-run status detail");
});

test("active task lifecycle shows failure recovery without page overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const item = boardItem("US-090", "Symphony Active Task Lifecycle Flow", "Needs Attention");
  const flow = taskFlow("validation");
  flow.state = "failed";
  flow.message = "Validation command failed.";
  flow.steps[2].state = "failed";
  flow.recovery_action = {
    kind: "execution_retry",
    label: "Retry task",
    endpoint: "/api/tasks/US-090/recover",
    confirmation: "Retry US-090?"
  };
  await page.route("**/api/board", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [item], task_flow: flow }) });
  });

  await page.goto("/");

  const lifecycle = page.getByRole("region", { name: "Active task lifecycle" });
  await expect(lifecycle.getByText("Validation command failed.")).toBeVisible();
  await expect(lifecycle.getByRole("button", { name: "Retry task" })).toBeVisible();
  await expectPageNoHorizontalOverflow(page);
  expect(await lifecycle.locator(".scrollbar-none").evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);
});

test("guided intake drafts a story before required proof is present", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("tab", { name: "Guided Intake" }).click();
  await expect(page.getByRole("heading", { name: "Guided Intake" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Draft story preview" })).toBeVisible();

  await page.getByRole("textbox", { name: "Rough idea" }).fill("Make review evidence easier to scan");
  await expect(page.getByRole("region", { name: "Draft story preview" })).toContainText("Make review evidence easier to scan");

  await page.getByRole("textbox", { name: "Who benefits from this work?" }).fill("Maintainers reviewing local Symphony runs");
  await page.getByRole("button", { name: "Next question" }).click();
  await page.getByRole("textbox", { name: "What should be true when this succeeds?" }).fill("They can approve or reject a run without opening raw artifacts first");
  await page.getByRole("button", { name: "Next question" }).click();
  await page.getByRole("textbox", { name: "What should stay out of scope?" }).fill("No automatic Symphony run start");

  const preview = page.getByRole("region", { name: "Draft story preview" });
  await expect(preview).toContainText("Maintainers reviewing local Symphony runs");
  await expect(preview).toContainText("They can approve or reject a run without opening raw artifacts first");
  await expect(preview).toContainText("No automatic Symphony run start");
  await expect(preview).toContainText("Normal lane");
  await expect(page.getByRole("button", { name: "Create story" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Start" })).toHaveCount(0);
});

test("guided intake creates a story after explicit confirmation", async ({ page }) => {
  let created = false;
  let createRequested = false;
  let startRequested = false;

  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Create a durable Harness story");
    await dialog.accept();
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: created ? [boardItem("US-075-DRAFT", "Make review evidence easier to scan", "Ready")] : []
      })
    });
  });
  await page.route("**/api/intake", async (route) => {
    expect(route.request().method()).toBe("POST");
    const body = route.request().postDataJSON();
    expect(body).toMatchObject({
      idea: "Make review evidence easier to scan",
      audience: "Maintainers reviewing local Symphony runs",
      outcome: "They can approve or reject a run without opening raw artifacts first",
      non_goals: "No automatic Symphony run start",
      validation: "npm --prefix crates/harness-symphony/web-ui run e2e"
    });
    createRequested = true;
    created = true;
    await route.fulfill({
      status: 201,
      contentType: "application/json",
      body: JSON.stringify({
        story_id: "US-075-DRAFT",
        title: "Make review evidence easier to scan",
        status: "planned"
      })
    });
  });
  await page.route("**/api/tasks/**/start", async (route) => {
    startRequested = true;
    await route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: "unexpected" }) });
  });

  await page.goto("/");
  await page.getByRole("tab", { name: "Guided Intake" }).click();
  await page.getByRole("textbox", { name: "Rough idea" }).fill("Make review evidence easier to scan");
  await page.getByRole("textbox", { name: "Who benefits from this work?" }).fill("Maintainers reviewing local Symphony runs");
  await page.getByRole("button", { name: "Next question" }).click();
  await page.getByRole("textbox", { name: "What should be true when this succeeds?" }).fill("They can approve or reject a run without opening raw artifacts first");
  await page.getByRole("button", { name: "Next question" }).click();
  await page.getByRole("textbox", { name: "What should stay out of scope?" }).fill("No automatic Symphony run start");
  await page.getByRole("button", { name: "Next question" }).click();
  await page.getByRole("textbox", { name: "What proof should show it worked?" }).fill("npm --prefix crates/harness-symphony/web-ui run e2e");

  await expect(page.getByRole("button", { name: "Create story" })).toBeEnabled();
  await page.getByRole("button", { name: "Create story" }).click();

  await expect.poll(async () => createRequested).toBe(true);
  const successToast = page.getByRole("region", { name: "Notifications" }).getByRole("alert").filter({ hasText: "Story created" });
  await expect(successToast).toBeVisible();
  await expect(successToast).toContainText("US-075-DRAFT");
  await expect(page.getByRole("region", { name: "Planned column" }).getByRole("button", { name: /US-075-DRAFT/ })).toBeVisible();
  expect(startRequested).toBe(false);
});

test("task detail close button closes popup and plays bounded confetti", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [boardItem("US-062", "Task Detail Close Confetti", "Ready")]
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-062/ }).click();

  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("heading", { name: "Task Detail Close Confetti" })).toBeVisible();
  await detail.getByRole("button", { name: "Close selected work detail" }).click();

  await expect(detail).toBeHidden();
  await expect(page.getByTestId("task-close-confetti")).toBeVisible();
  await expect(page.getByRole("button", { name: /US-062/ })).toBeVisible();
  await expect(page.getByTestId("task-close-confetti-host")).toHaveCount(0, { timeout: 2000 });

  await page.getByRole("button", { name: /US-062/ }).click();
  await expect(page.getByRole("dialog", { name: "Selected work detail" })).toBeVisible();
});

test("task detail traps focus, closes with escape, and restores opener focus", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [boardItem("US-069", "Modal Focus Contract", "Ready")]
      })
    });
  });

  await page.goto("/");
  const opener = page.getByRole("button", { name: /US-069/ });
  await opener.click();

  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail).toBeVisible();
  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press("Tab");
    await expect
      .poll(async () =>
        detail.evaluate((element) => element.contains(document.activeElement))
      )
      .toBe(true);
  }

  await page.keyboard.press("Escape");
  await expect(detail).toBeHidden();
  await expect(opener).toBeFocused();
});

test("task detail close keeps working with reduced motion confetti suppressed", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [boardItem("US-062", "Task Detail Close Confetti", "Ready")]
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-062/ }).click();

  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail).toBeVisible();
  await detail.getByRole("button", { name: "Close selected work detail" }).click();

  await expect(detail).toBeHidden();
  await expect(page.getByTestId("task-close-confetti-host")).toHaveCount(0);
});

test("reduced motion suppresses operational spinner animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [boardItem("US-069", "Reduced Motion Active Run", "In Progress")]
      })
    });
  });

  await page.goto("/");
  const spinner = page.locator("#bucket-active svg").first();
  await expect(spinner).toBeVisible();
  await expect
    .poll(async () => spinner.evaluate((element) => getComputedStyle(element).animationName))
    .toBe("none");
});

test("summary strip pulses while a Symphony run is active", async ({ page }) => {
  const activeItem = boardItem("US-069", "Active Run Heartbeat", "In Progress");
  activeItem.run_id = "run_active";
  activeItem.active_run = "run_active";

  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [activeItem] })
    });
  });

  await page.goto("/");

  const activeMetric = page
    .getByRole("region", { name: "Command status rail" })
    .getByText("Agent working", { exact: true })
    .locator("xpath=ancestor::*[contains(@class, 'rounded-xl')][1]");
  await expect(activeMetric).toHaveClass(/border-blue-500\/30/);
  await expect(activeMetric).toHaveClass(/bg-blue-500\/5/);
  await expect(activeMetric.locator("span").first()).toHaveClass(/motion-safe:animate-pulse/);
});

test("summary strip keeps agent working neutral while Symphony is idle", async ({ page }) => {
  await page.goto("/");

  const activeMetric = page
    .getByRole("region", { name: "Command status rail" })
    .getByText("Agent working", { exact: true })
    .locator("xpath=ancestor::*[contains(@class, 'rounded-xl')][1]");
  await expect(activeMetric).toHaveClass(/border-border/);
  await expect(activeMetric).toHaveClass(/bg-card/);
  await expect(activeMetric).toHaveClass(/text-muted-foreground/);
  await expect(activeMetric).not.toHaveClass(/border-blue-500\/30/);
});

test("board loading and failure states expose accessibility semantics", async ({ page }) => {
  let releaseBoard!: () => void;
  const boardReady = new Promise<void>((resolve) => {
    releaseBoard = resolve;
  });
  await page.route("**/api/board", async (route) => {
    await boardReady;
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [boardItem("US-069", "Accessible Board State", "Ready")] })
    });
  });

  const gotoPromise = page.goto("/");
  await expect(page.locator("#board")).toHaveAttribute("aria-busy", "true");
  await expect(page.getByRole("status")).toContainText("Loading Symphony board.");
  releaseBoard();
  await gotoPromise;
  await expect(page.locator("#board")).toHaveAttribute("aria-busy", "false");
  await expect(page.getByRole("status")).toContainText("Symphony board loaded.");

  await page.route("**/api/board", async (route) => {
    await route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: "board unavailable" }) });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("board unavailable");
});

test("ready task delete action confirms, retires, and refreshes the board", async ({ page }) => {
  let retired = false;
  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Retire US-064 Ready Work Story Delete Action");
    await dialog.accept();
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: retired ? [] : [boardItem("US-064", "Ready Work Story Delete Action", "Ready")]
      })
    });
  });
  await page.route("**/api/tasks/US-064/retire", async (route) => {
    expect(route.request().method()).toBe("POST");
    retired = true;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ story_id: "US-064", status: "retired" })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-064/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });

  await expect(detail.getByRole("button", { name: "Delete work story" })).toBeVisible();
  await detail.getByRole("button", { name: "Delete work story" }).click();

  await expect.poll(async () => retired).toBe(true);
  await expect(detail).toBeHidden();
  await expect(page.getByRole("button", { name: /US-064/ })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Planned column" }).getByText("No planned tasks")).toBeVisible();
});

test("ready card runs codex from the board without opening detail", async ({ page }) => {
  let started = false;
  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Run US-076 with Codex");
    await dialog.accept();
  });
  await page.route("**/api/board", async (route) => {
    const item = boardItem("US-076", "Run Ready Story From Board Card", started ? "In Progress" : "Ready");
    item.active_run = started ? "run_us_076" : null;
    item.run_id = started ? "run_us_076" : null;
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [item] })
    });
  });
  await page.route("**/api/tasks/US-076/start", async (route) => {
    expect(route.request().method()).toBe("POST");
    started = true;
    await route.fulfill({
      status: 202,
      contentType: "application/json",
      body: JSON.stringify({ run_id: "run_us_076", story_id: "US-076", status: "started" })
    });
  });

  await page.goto("/");

  const plannedColumn = page.getByRole("region", { name: "Planned column" });
  const readyCard = plannedColumn.getByTestId("task-card").filter({ hasText: "US-076" });
  const runButton = readyCard.getByRole("button", { name: "Run with Codex" });
  await expect(runButton).toBeVisible();
  const plannedColumnBox = await plannedColumn.boundingBox();
  const runControlBox = await runButton.locator("..").boundingBox();
  const runButtonBox = await runButton.boundingBox();
  expect(plannedColumnBox?.width ?? 0, "Planned column keeps readable action width").toBeGreaterThanOrEqual(220);
  expect(runControlBox?.width ?? 0, "Run split control width").toBeGreaterThanOrEqual(184);
  expect(runButtonBox?.height ?? 0, "Run with Codex button height").toBeGreaterThanOrEqual(34);
  await expectNoHorizontalOverflow(runButton, "Run with Codex button");
  await runButton.click();

  await expect.poll(async () => started).toBe(true);
  await expect(page.getByRole("dialog", { name: "Selected work detail" })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "Agent working column" }).getByRole("button", { name: /US-076/ })).toBeVisible();
});

test("agent dropdown runs with opencode and remembers the choice", async ({ page }) => {
  let startBody: unknown = null;
  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Run US-078 with OpenCode");
    await dialog.accept();
  });
  await page.route("**/api/settings", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ default_agent: "codex" })
    });
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [boardItem("US-078", "OpenCode Agent Selection", "Ready")] })
    });
  });
  await page.route("**/api/tasks/US-078/start", async (route) => {
    startBody = route.request().postDataJSON();
    await route.fulfill({
      status: 202,
      contentType: "application/json",
      body: JSON.stringify({ run_id: "run_us_078", story_id: "US-078", status: "started", agent: "opencode" })
    });
  });

  await page.goto("/");

  const readyCard = page.getByRole("region", { name: "Planned column" }).getByTestId("task-card").filter({ hasText: "US-078" });
  await expect(readyCard.getByRole("button", { name: "Run with Codex" })).toBeVisible();
  await readyCard.getByRole("button", { name: "Choose agent" }).click();
  await page.getByRole("menuitem", { name: "Run with OpenCode" }).click();

  await expect.poll(async () => startBody).toEqual({ agent: "opencode" });
  await expect(readyCard.getByRole("button", { name: "Run with OpenCode" })).toBeVisible();
});

test("settings view saves the default agent and relabels the run button", async ({ page }) => {
  let savedAgent: string | null = null;
  await page.route("**/api/settings", async (route) => {
    if (route.request().method() === "PUT") {
      const payload = route.request().postDataJSON() as { default_agent: string };
      savedAgent = payload.default_agent;
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({ default_agent: payload.default_agent })
      });
      return;
    }
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ default_agent: savedAgent ?? "codex" })
    });
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [boardItem("US-078", "OpenCode Agent Selection", "Ready")] })
    });
  });

  await page.goto("/");

  await page.getByRole("tab", { name: "Settings" }).click();
  await page.getByRole("radio", { name: /OpenCode/ }).check({ force: true });
  await page.getByRole("button", { name: "Save default agent" }).click();

  await expect.poll(async () => savedAgent).toBe("opencode");

  await page.getByRole("tab", { name: "Work Board" }).click();
  const readyCard = page.getByRole("region", { name: "Planned column" }).getByTestId("task-card").filter({ hasText: "US-078" });
  await expect(readyCard.getByRole("button", { name: "Run with OpenCode" })).toBeVisible();
});

test("task detail renders context pack with fenced code", async ({ page }) => {
  let contextRequested = false;
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [boardItem("US-080", "Context Viewer Surface", "Ready")]
      })
    });
  });
  await page.route("**/api/tasks/US-080/context", async (route) => {
    contextRequested = true;
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        story_id: "US-080",
        content: "# Context Pack\n\nRead this first.\n\n```bash\ncargo test -p harness-symphony\n```"
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-080/ }).click();

  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("heading", { name: "Context pack" })).toBeVisible();
  await expect(detail.getByText("Read this first.")).toBeVisible();
  await expect(detail.getByText("bash")).toBeVisible();
  await expect(detail.getByText("cargo test -p harness-symphony")).toBeVisible();
  await expect.poll(async () => contextRequested).toBe(true);
});

test("trace explorer loads and filters trace records", async ({ page }) => {
  let requestedUrl = "";
  await page.route("**/api/board", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [] }) });
  });
  await page.route("**/api/traces**", async (route) => {
    requestedUrl = route.request().url();
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        total: 1,
        traces: [
          {
            id: 7,
            story_id: "US-080",
            summary: "Trace target",
            outcome: "completed",
            created_at: "2026-07-09 10:00:00",
            duration_seconds: 12,
            harness_friction: "none"
          }
        ]
      })
    });
  });

  await page.goto("/");
  await page.getByRole("tab", { name: "Trace Explorer" }).click();
  await expect(page.getByRole("heading", { name: "Trace Explorer" })).toBeVisible();
  await page.getByRole("textbox", { name: "Trace story filter" }).fill("US-080");
  await page.getByRole("button", { name: "Apply trace filter" }).click();

  await expect.poll(async () => requestedUrl).toContain("story_id=US-080");
  await expect(page.getByRole("region", { name: "Trace results" })).toContainText("Trace target");
  await expect(page.getByRole("region", { name: "Trace results" })).toContainText("completed");
});

test("tool status dashboard lists tools and triggers scan", async ({ page }) => {
  let checked = false;
  await page.route("**/api/board", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [] }) });
  });
  await page.route("**/api/tools", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        tools: [
          {
            provider: "custom",
            name: "deploy-check",
            kind: "cli",
            capability: "deploy-verification",
            status: checked ? "present" : "unknown",
            description: "Verify deploy health before release",
            responsibility: "Verification",
            command: "./scripts/deploy-check.sh",
            source: "registered",
            since: "registered",
            scan_target: null,
            checked_at: checked ? "2026-07-09 10:00:00" : null
          }
        ]
      })
    });
  });
  await page.route("**/api/tools/check", async (route) => {
    checked = true;
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ tools: [{ name: "deploy-check", status: "present" }] })
    });
  });

  await page.goto("/");
  await page.getByRole("tab", { name: "Tool Status" }).click();
  await expect(page.getByRole("heading", { name: "Tool Status" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Tool registry" })).toContainText("deploy-check");
  await expect(page.getByRole("region", { name: "Tool registry" })).toContainText("unknown");

  await page.getByRole("button", { name: "Check tools" }).click();

  await expect.poll(async () => checked).toBe(true);
  await expect(page.getByRole("region", { name: "Tool registry" })).toContainText("present");
});

test("run monitor summarizes active event progress", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    const item = boardItem("US-081", "Run Monitor Progress", "In Progress");
    item.active_run = "run_us_081";
    item.run_id = "run_us_081";
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [item] }) });
  });
  await page.route("**/api/runs/run_us_081/events", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_us_081",
        events: [
          { method: "turn/started", params: { timestamp: "2026-07-09T10:00:00Z" } },
          { method: "turn/diff/updated", params: { timestamp: "2026-07-09T10:01:00Z" } }
        ],
        last_sequence: 2,
        reset_required: false
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-081/ }).click();

  const monitor = page.getByRole("region", { name: "Run monitor" });
  await expect(monitor.getByRole("heading", { name: "Run monitor" })).toBeVisible();
  await expect(monitor).toContainText("Events 2");
  await expect(monitor).toContainText("Execution");
});

test("ready review requests changes with reason and image evidence", async ({ page }) => {
  let requested = false;
  let multipartBody = "";
  let multipartContentType = "";
  const item = boardItem("US-082", "Request Changes With Evidence", "Review");
  item.run_id = "run_us_082";
  await page.route("**/api/board", async (route) => {
    const current = { ...item };
    if (requested) {
      current.board_state = "In Progress";
      current.run_id = "run_replacement_082";
      current.active_run = "run_replacement_082";
      current.reason = "active run run_replacement_082";
    }
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [current] }) });
  });
  await page.route("**/api/tasks/US-082/context", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ story_id: "US-082", content: "# Context" }) });
  });
  await page.route("**/api/runs/run_us_082/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_us_082",
        story_id: "US-082",
        status: "completed",
        agent: "codex",
        outcome: "completed",
        summary: "Ready for review",
        result: null,
        validation: null,
        changed_files: ["src/lib.rs"],
        changeset_preview: "diff",
        pr_url: "https://example.test/pr/82",
        pr_status: "created",
        artifact_paths: [],
        events: [],
        suggested_next_action: "Review evidence.",
        failure_summary: null,
        recovery_action: null,
        request_changes: null
      })
    });
  });
  await page.route("**/api/runs/run_us_082/request-changes", async (route) => {
    multipartContentType = route.request().headers()["content-type"] ?? "";
    multipartBody = route.request().postDataBuffer()?.toString("latin1") ?? "";
    requested = true;
    await route.fulfill({
      status: 202,
      contentType: "application/json",
      body: JSON.stringify({
        source_run_id: "run_us_082",
        run_id: "run_replacement_082",
        story_id: "US-082",
        status: "prepared",
        feedback: {
          reason_path: ".harness/runs/run_replacement_082/feedback/reason.md",
          evidence_paths: [".harness/runs/run_replacement_082/feedback/evidence-01.png"]
        }
      })
    });
  });
  await page.route("**/api/runs/run_replacement_082/events", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_replacement_082",
        events: [],
        last_sequence: 0,
        reset_required: false
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-082/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await detail.getByRole("textbox", { name: "Request changes reason" }).fill("Tighten the mobile spacing");
  await detail.getByLabel("Evidence images").setInputFiles({
    name: "mobile-spacing.png",
    mimeType: "image/png",
    buffer: Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1])
  });
  await expect(detail.getByText("mobile-spacing.png")).toBeVisible();
  await expect(detail.getByRole("img", { name: "Preview mobile-spacing.png" })).toBeVisible();
  await detail.getByRole("button", { name: "Request changes" }).click();

  await expect.poll(() => requested).toBe(true);
  expect(multipartContentType).toContain("multipart/form-data; boundary=");
  expect(multipartBody).toContain('name="reason"');
  expect(multipartBody).toContain("Tighten the mobile spacing");
  expect(multipartBody).toContain('name="evidence"; filename="mobile-spacing.png"');
  await expect(page.getByRole("region", { name: "Agent working column" }).getByRole("button", { name: /US-082/ })).toBeVisible();
});

test("request changes validates image limits and supports removal", async ({ page }) => {
  const item = boardItem("US-083", "Request Changes Validation", "Review");
  item.run_id = "run_us_083";
  await page.route("**/api/board", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [item] }) });
  });
  await page.route("**/api/tasks/US-083/context", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ story_id: "US-083", content: "# Context" }) });
  });
  await page.route("**/api/runs/run_us_083/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_us_083",
        story_id: "US-083",
        status: "completed",
        agent: "codex",
        outcome: "completed",
        summary: "Ready for review",
        result: null,
        validation: null,
        changed_files: [],
        changeset_preview: null,
        pr_url: null,
        pr_status: "not_applicable",
        artifact_paths: [],
        events: [],
        suggested_next_action: "Review evidence.",
        failure_summary: null,
        recovery_action: null,
        request_changes: {
          reason: "Previous spacing pass was incomplete",
          reason_path: ".harness/runs/run_us_083/feedback/reason.md",
          evidence: [
            {
              path: ".harness/runs/run_us_083/feedback/evidence-01.png",
              url: "/api/runs/run_us_083/feedback/evidence-01.png",
              content_type: "image/png",
              size: 9
            }
          ]
        }
      })
    });
  });
  await page.route("**/api/runs/run_us_083/feedback/evidence-01.png", async (route) => {
    await route.fulfill({ contentType: "image/png", body: Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1]) });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-083/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  const submit = detail.getByRole("button", { name: "Request changes" });
  await expect(submit).toBeDisabled();
  await expect(detail.getByText("Previous spacing pass was incomplete")).toBeVisible();
  await expect(detail.getByRole("img", { name: "Request changes evidence 1" })).toBeVisible();

  const input = detail.getByLabel("Evidence images");
  await input.setInputFiles({ name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("notes") });
  await expect(detail.getByRole("alert")).toContainText("PNG, JPEG, or WebP");

  const oversized = Buffer.alloc(5 * 1024 * 1024 + 1);
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(oversized);
  await input.setInputFiles({ name: "oversized.png", mimeType: "image/png", buffer: oversized });
  await expect(detail.getByRole("alert")).toContainText("5 MB");

  const valid = (name: string) => ({
    name,
    mimeType: "image/png",
    buffer: Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1])
  });
  await input.setInputFiles([valid("one.png"), valid("two.png"), valid("three.png"), valid("four.png")]);
  await expect(detail.getByRole("alert")).toContainText("up to 3");

  await input.setInputFiles([valid("one.png"), valid("two.png")]);
  await expect(detail.getByText("2/3 images")).toBeVisible();
  await detail.getByRole("button", { name: "Remove one.png" }).click();
  await expect(detail.getByText("one.png")).toHaveCount(0);
  await expect(detail.getByText("1/3 images")).toBeVisible();
});

test("done detail does not offer request changes", async ({ page }) => {
  const item = boardItem("US-084-DONE", "Accepted Work Stays Done", "Done");
  item.run_id = "run_done_084";
  await page.route("**/api/board", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [item] }) });
  });
  await page.route("**/api/tasks/US-084-DONE/context", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ story_id: "US-084-DONE", content: "# Context" }) });
  });
  await page.route("**/api/runs/run_done_084/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_done_084",
        story_id: "US-084-DONE",
        status: "completed",
        agent: "codex",
        outcome: "completed",
        summary: "Accepted",
        result: null,
        validation: null,
        changed_files: [],
        changeset_preview: null,
        pr_url: null,
        pr_status: "merged",
        artifact_paths: [],
        events: [],
        suggested_next_action: "Accepted work remains done.",
        failure_summary: null,
        recovery_action: null,
        request_changes: null
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-084-DONE/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("textbox", { name: "Request changes reason" })).toHaveCount(0);
  await expect(detail.getByRole("button", { name: "Request changes" })).toHaveCount(0);
});

test("delete action is hidden for non-ready tasks", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          boardItem("US-064", "Blocked Delete Guard", "Blocked"),
          boardItem("US-065", "Done Delete Guard", "Done")
        ]
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-064/ }).click();

  let detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("button", { name: "Delete work story" })).toHaveCount(0);
  await detail.getByRole("button", { name: "Close selected work detail" }).click();
  await page.getByRole("button", { name: /US-065/ }).click();
  detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("button", { name: "Delete work story" })).toHaveCount(0);
});

test("sidebar renders dependency graph edges and selects tasks", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            id: "US-056",
            title: "Simplify Kanban-First Controller",
            board_state: "Done",
            story_status: "implemented",
            lane: "normal",
            verify: "configured",
            blockers: [],
            unblocks: ["US-057"],
            parent_id: null,
            children: [],
            hierarchy_depth: 0,
            run_id: null,
            active_run: null,
            reason: "story implemented"
          },
          {
            id: "US-057",
            title: "Dependency Graph Sidebar View",
            board_state: "Ready",
            story_status: "planned",
            lane: "normal",
            verify: "configured",
            blockers: ["US-056"],
            unblocks: ["US-059"],
            parent_id: null,
            children: [],
            hierarchy_depth: 0,
            run_id: null,
            active_run: null,
            reason: "ready"
          },
          {
            id: "US-059",
            title: "Review Surface Density Pass",
            board_state: "Blocked",
            story_status: "planned",
            lane: "normal",
            verify: "configured",
            blockers: ["US-057"],
            unblocks: [],
            parent_id: null,
            children: [],
            hierarchy_depth: 0,
            run_id: null,
            active_run: null,
            reason: "waiting for US-057"
          }
        ]
      })
    });
  });

  await page.goto("/");

  const graph = page.getByRole("region", { name: "Dependency graph sidebar" });
  await expect(graph.getByRole("heading", { name: "Dependency graph" })).toBeVisible();
  await expect(graph.getByLabel("Dependency edges")).toContainText("US-056");
  await expect(graph.getByLabel("Dependency edges")).toContainText("US-057");
  await expect(graph.getByLabel("Dependency edges")).toContainText("US-059");

  await graph.getByRole("button", { name: /US-057 Ready Dependency Graph Sidebar View/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("heading", { name: "Dependency Graph Sidebar View" })).toBeVisible();
  await expect(detail.getByText("US-056")).toBeVisible();
  await expect(detail.getByText("US-059")).toBeVisible();
});

test("board columns stay bounded and scroll dense task lists internally", async ({ page }) => {
  const longToken =
    "BoundedWorkItemCardsNeedToContainThisUnbrokenRunIdentifierFailureCategoryLaneLabelAndBlockerMetadata1234567890";
  const longReadyItem = {
    ...boardItem(`US-068-${longToken}`, `Bounded summary ${longToken} ${longToken}`, "Ready"),
    reason: `Ready because ${longToken} should stay inside the card summary instead of widening the board column.`
  };
  const longAttentionItem = {
    ...boardItem("US-968", `Needs attention ${longToken}`, "Needs Attention"),
    lane: `normal-${longToken}`,
    run_id: `run_${longToken}`,
    reason: `Failure reason ${longToken} remains a compact board summary.`,
    failure_summary: {
      category: `Category-${longToken}`,
      reason: `Reason-${longToken}`,
      latest_event: `Event-${longToken}`,
      latest_error: `Error-${longToken}`,
      run_id: `run_${longToken}`,
      evidence_artifacts: [`.harness/runs/run_${longToken}/RESULT.json`],
      next_action: `Inspect-${longToken}`
    }
  };
  const denseReadyItems = Array.from({ length: 22 }, (_, index) =>
    boardItem(`US-9${String(index).padStart(2, "0")}`, `Dense ready task ${index + 1}`, "Ready")
  );
  const sparseItems = ["Blocked", "In Progress", "Review", "Needs Attention", "Done"].map((state, index) =>
    boardItem(`US-8${index}`, `${state} task`, state)
  );

  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [longReadyItem, ...denseReadyItems, longAttentionItem, ...sparseItems] })
    });
  });

  await page.setViewportSize({ width: 1440, height: 820 });
  await page.goto("/");

  for (const label of ["Planned", "Agent working", "Human review", "Done"]) {
    await expect(page.getByRole("region", { name: `${label} column` })).toBeVisible();
  }

  const plannedColumn = page.getByRole("region", { name: "Planned column" });
  const agentWorkingColumn = page.getByRole("region", { name: "Agent working column" });
  const plannedTasks = page.locator('[aria-label="Planned tasks"]');
  const board = page.locator("#board");
  const longReadyCard = page.getByTestId("task-card").filter({ hasText: `US-068-${longToken}` });
  const longAttentionCard = page.getByTestId("task-card").filter({ hasText: "US-968" });
  const pageScrollHeight = await page.evaluate(() => document.documentElement.scrollHeight);
  const viewportHeight = await page.evaluate(() => window.innerHeight);
  const plannedMetrics = await plannedTasks.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    scrollTop: element.scrollTop
  }));

  expect(plannedMetrics.scrollHeight).toBeGreaterThan(plannedMetrics.clientHeight);
  expect(pageScrollHeight).toBeLessThan(viewportHeight + 280);
  await expectPageNoHorizontalOverflow(page);
  await expectNoHorizontalOverflow(board, "desktop board");
  await expectNoHorizontalOverflow(plannedColumn, "desktop Planned column");
  await expectNoHorizontalOverflow(agentWorkingColumn, "desktop Agent working column");
  await expectNoHorizontalOverflow(longReadyCard, "desktop long ready card");
  await expectNoHorizontalOverflow(longAttentionCard, "desktop long needs attention card");

  await plannedTasks.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });

  await expect(plannedColumn.getByRole("heading", { name: "Planned", exact: true })).toBeVisible();
  await expect(plannedColumn.getByTestId("task-card").filter({ hasText: "Blocked" })).toBeVisible();
  await expect(agentWorkingColumn.getByTestId("task-card").filter({ hasText: "US-968" })).toContainText("Needs attention");
  await expect(page.getByRole("button", { name: /US-921/ })).toBeVisible();
  await expect
    .poll(async () => plannedTasks.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(plannedMetrics.scrollTop);

  await page.setViewportSize({ width: 390, height: 760 });
  await expect(plannedColumn).toBeVisible();
  const boardBox = await board.boundingBox();
  expect(boardBox?.y ?? 9999).toBeLessThan(760);
  const mobilePlannedMetrics = await plannedTasks.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight
  }));
  expect(mobilePlannedMetrics.scrollHeight).toBeGreaterThan(mobilePlannedMetrics.clientHeight);
  await expectPageNoHorizontalOverflow(page);
  await expectNoHorizontalOverflow(board, "mobile board");
  await expectNoHorizontalOverflow(plannedColumn, "mobile Planned column");
  await expectNoHorizontalOverflow(agentWorkingColumn, "mobile Agent working column");
  await expectNoHorizontalOverflow(longReadyCard, "mobile long ready card");
  await expectNoHorizontalOverflow(longAttentionCard, "mobile long needs attention card");
  await plannedColumn.getByRole("button", { name: /US-900/ }).click();
  await expect(page.getByRole("dialog", { name: "Selected work detail" })).toBeVisible();
});

test("done column keeps dense completed task cards readable while scrolling internally", async ({ page }) => {
  const denseDoneItems = Array.from({ length: 48 }, (_, index) => {
    const item = boardItem(
      `US-070-D${String(index).padStart(2, "0")}`,
      `Readable Done card summary ${index + 1}`,
      "Done"
    );
    item.reason = `Completed work item ${index + 1} remains readable in a dense Done column.`;
    item.run_id = index % 2 === 0 ? `run_done_${String(index).padStart(2, "0")}` : null;
    return item;
  });

  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [boardItem("US-070-R", "Ready control", "Ready"), ...denseDoneItems] })
    });
  });

  await page.setViewportSize({ width: 1440, height: 820 });
  await page.goto("/");

  const board = page.locator("#board");
  const doneColumn = page.getByRole("region", { name: "Done column" });
  const doneTasks = page.locator('[aria-label="Done tasks"]');
  const firstDoneCard = doneColumn.getByTestId("task-card").filter({ hasText: "US-070-D00" });
  const secondDoneCard = doneColumn.getByTestId("task-card").filter({ hasText: "US-070-D01" });

  await expect(firstDoneCard).toBeVisible();
  await expect(firstDoneCard).toContainText("configured");
  await expect(firstDoneCard).toContainText("Readable Done card summary 1");
  await expect(firstDoneCard).toContainText("normal");
  await expect(firstDoneCard).toContainText("run_done_00");
  await expect(secondDoneCard).toContainText("No run");
  await expectReadableTaskCard(firstDoneCard, "desktop first Done card");
  await expectReadableTaskCard(secondDoneCard, "desktop second Done card");

  const doneMetrics = await doneTasks.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    scrollTop: element.scrollTop
  }));
  expect(doneMetrics.scrollHeight).toBeGreaterThan(doneMetrics.clientHeight);
  await expectPageNoHorizontalOverflow(page);
  await expectNoHorizontalOverflow(board, "desktop board with dense Done cards");
  await expectNoHorizontalOverflow(doneColumn, "desktop Done column");
  await expectNoHorizontalOverflow(firstDoneCard, "desktop first Done card");

  await doneTasks.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(doneColumn.getByRole("heading", { name: "Done", exact: true })).toBeVisible();
  await expect(doneColumn.getByRole("button", { name: /US-070-D47/ })).toBeVisible();
  await expect
    .poll(async () => doneTasks.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(doneMetrics.scrollTop);

  await page.setViewportSize({ width: 390, height: 760 });
  await doneTasks.evaluate((element) => {
    element.scrollTop = 0;
  });
  await expect(firstDoneCard).toBeVisible();
  await expectReadableTaskCard(firstDoneCard, "mobile first Done card");
  const mobileDoneMetrics = await doneTasks.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight
  }));
  expect(mobileDoneMetrics.scrollHeight).toBeGreaterThan(mobileDoneMetrics.clientHeight);
  await expectPageNoHorizontalOverflow(page);
  await expectNoHorizontalOverflow(board, "mobile board with dense Done cards");
  await expectNoHorizontalOverflow(doneColumn, "mobile Done column");
  await expectNoHorizontalOverflow(firstDoneCard, "mobile first Done card");
});

test("active run polling refreshes terminal review and needs-attention board states", async ({ page }) => {
  let boardReads = 0;
  await page.route("**/api/board", async (route) => {
    boardReads += 1;
    const reviewItem = boardItem("US-069A", "Terminal Review Refresh", boardReads < 2 ? "In Progress" : "Review");
    reviewItem.run_id = boardReads < 2 ? "run_review_active" : "run_review_done";
    reviewItem.active_run = boardReads < 2 ? "run_review_active" : null;
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [reviewItem] }) });
  });
  await page.route("**/api/runs/run_review_done/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_review_done",
        story_id: "US-069A",
        status: "completed",
        outcome: "completed",
        summary: "Ready for review.",
        result: null,
        validation: null,
        changed_files: [],
        changeset_preview: null,
        pr_url: null,
        pr_status: "missing",
        artifact_paths: [],
        suggested_next_action: "Review terminal state.",
        failure_summary: null,
        recovery_action: null,
        events: []
      })
    });
  });

  await page.goto("/");
  await expect(page.getByRole("region", { name: "Human review column" }).getByRole("button", { name: /US-069A/ })).toBeVisible({
    timeout: 5000
  });

  boardReads = 0;
  await page.route("**/api/board", async (route) => {
    boardReads += 1;
    const attentionItem = boardItem("US-069B", "Terminal Attention Refresh", boardReads < 2 ? "In Progress" : "Needs Attention");
    attentionItem.run_id = boardReads < 2 ? "run_attention_active" : "run_attention_failed";
    attentionItem.active_run = boardReads < 2 ? "run_attention_active" : null;
    attentionItem.failure_summary =
      boardReads < 2
        ? null
        : {
            category: "Codex run failure",
            reason: "Terminal failure arrived from the backend.",
            latest_event: "turn/completed",
            latest_error: "failed",
            run_id: "run_attention_failed",
            evidence_artifacts: [".harness/runs/run_attention_failed/RESULT.json"],
            next_action: "Inspect the failed run."
          };
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [attentionItem] }) });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("region", { name: "Agent working column" }).getByRole("button", { name: /US-069B/ })).toBeVisible({
    timeout: 5000
  });
});

test("needs attention tasks show failure reason and evidence", async ({ page }) => {
  const failureSummary = {
    category: "Codex app-server timeout",
    reason: "turn-state query timed out while waiting for Codex.",
    latest_event: "turn/completed status failed",
    latest_error: "turn-state query timed out while waiting for Codex.",
    run_id: "run_timeout",
    evidence_artifacts: [
      ".harness/runs/run_timeout/APP_SERVER_EVENTS.jsonl",
      ".harness/runs/run_timeout/RESULT.json"
    ],
    next_action: "Inspect APP_SERVER_EVENTS.jsonl and retry when safe."
  };

  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            ...boardItem("US-066", "Needs Attention Failure Explanation", "Needs Attention"),
            run_id: "run_timeout",
            reason: failureSummary.reason,
            failure_summary: failureSummary
          }
        ]
      })
    });
  });
  await page.route("**/api/runs/run_timeout/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_timeout",
        story_id: "US-066",
        status: "failed",
        outcome: "failed",
        summary: null,
        result: null,
        validation: null,
        changed_files: [],
        changeset_preview: null,
        pr_url: null,
        pr_status: "missing",
        artifact_paths: failureSummary.evidence_artifacts,
        suggested_next_action: failureSummary.next_action,
        failure_summary: failureSummary,
        events: [{ method: "turn/completed", params: { turn: { status: "failed", error: { message: failureSummary.latest_error } } } }]
      })
    });
  });

  await page.goto("/");

  await expect(page.getByRole("button", { name: /US-066/ })).toContainText(failureSummary.reason);
  await expect(page.getByRole("button", { name: /US-066/ })).toContainText("Codex app-server timeout");

  await page.getByRole("button", { name: /US-066/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });

  await expect(detail.getByText("Codex app-server timeout").first()).toBeVisible();
  await expect(detail.getByText("turn-state query timed out while waiting for Codex.").first()).toBeVisible();
  await expect(detail.getByText("turn/completed status failed").first()).toBeVisible();
  await expect(detail.getByText(".harness/runs/run_timeout/APP_SERVER_EVENTS.jsonl").first()).toBeVisible();
  await expect(detail.getByText("Inspect APP_SERVER_EVENTS.jsonl and retry when safe.").first()).toBeVisible();
});

test("needs attention detail keeps long failure category bounded on mobile", async ({ page }) => {
  const longToken =
    "NeedsAttentionFailureCategoryShouldWrapInsideTheMobileDetailDialogWithoutCreatingHorizontalOverflow1234567890";
  const failureSummary = {
    category: `Category-${longToken}`,
    reason: `Failure reason ${longToken}`,
    latest_event: `turn/completed-${longToken}`,
    latest_error: `error-${longToken}`,
    run_id: `run_${longToken}`,
    evidence_artifacts: [`.harness/runs/run_${longToken}/RESULT.json`],
    next_action: `Inspect ${longToken}`
  };

  await page.setViewportSize({ width: 390, height: 760 });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            ...boardItem("US-066", `Needs Attention ${longToken}`, "Needs Attention"),
            run_id: `run_${longToken}`,
            reason: failureSummary.reason,
            failure_summary: failureSummary
          }
        ]
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-066/ }).click();

  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByText(failureSummary.category)).toBeVisible();
  await expectNoHorizontalOverflow(page.getByTestId("task-detail-popup"), "mobile needs attention detail popup");
  await expectPageNoHorizontalOverflow(page);
});

test("execution recovery retries needs attention work and preserves failed evidence", async ({ page }) => {
  const failureSummary = {
    category: "Codex run failure",
    reason: "Codex turn failed.",
    latest_event: "turn/completed status failed",
    latest_error: "Codex turn failed.",
    run_id: "run_failed",
    evidence_artifacts: [".harness/runs/run_failed/APP_SERVER_EVENTS.jsonl"],
    next_action: "Inspect APP_SERVER_EVENTS.jsonl and retry when safe."
  };
  const recoveryAction = {
    kind: "execution_retry",
    label: "Retry work",
    endpoint: "/api/tasks/US-067/recover",
    confirmation: "Start a new Symphony run for this task? The failed run evidence stays available."
  };
  let recovered = false;

  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Start a new Symphony run");
    await dialog.accept();
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            ...boardItem("US-067", "Needs Attention Recovery Action", recovered ? "In Progress" : "Needs Attention"),
            run_id: recovered ? "run_recovery" : "run_failed",
            active_run: recovered ? "run_recovery" : null,
            reason: recovered ? "active run run_recovery" : failureSummary.reason,
            failure_summary: recovered ? null : failureSummary,
            recovery_action: recovered ? null : recoveryAction
          }
        ]
      })
    });
  });
  await page.route("**/api/runs/run_failed/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_failed",
        story_id: "US-067",
        status: "failed",
        outcome: "failed",
        summary: null,
        result: null,
        validation: null,
        changed_files: [],
        changeset_preview: null,
        pr_url: null,
        pr_status: "missing",
        artifact_paths: failureSummary.evidence_artifacts,
        suggested_next_action: failureSummary.next_action,
        failure_summary: failureSummary,
        recovery_action: recoveryAction,
        events: []
      })
    });
  });
  await page.route("**/api/tasks/US-067/recover", async (route) => {
    recovered = true;
    await route.fulfill({
      status: 202,
      contentType: "application/json",
      body: JSON.stringify({ run_id: "run_recovery", story_id: "US-067", status: "recovering" })
    });
  });
  await page.route("**/api/runs/run_recovery/events", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_recovery",
        events: [
          { method: "turn/started", params: { turn: { status: "inProgress" } } },
          { method: "item/agentMessage/delta", params: { itemId: "retry_msg", delta: "Retry run is now live." } }
        ],
        last_sequence: 2,
        reset_required: false
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-067/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });

  await expect(detail.getByRole("button", { name: "Retry work" })).toBeVisible();
  await detail.getByRole("button", { name: "Retry work" }).click();

  await expect(page.getByRole("button", { name: /US-067/ })).toContainText("active");
  await expect(detail.getByRole("heading", { name: "Prior failed run evidence" })).toBeVisible();
  await expect(detail.getByText(".harness/runs/run_failed/APP_SERVER_EVENTS.jsonl").first()).toBeVisible();
  await expect(detail.getByRole("heading", { name: "Run communication" })).toBeVisible();
  await expect(detail.getByText("Retry run is now live.")).toBeVisible();
});

test("review endpoint failures render explicit alert evidence", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    const item = boardItem("US-069", "Review Error State", "Review");
    item.run_id = "run_review_error";
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [item] }) });
  });
  await page.route("**/api/runs/run_review_error/review", async (route) => {
    await route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: "review exploded" }) });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-069/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("alert")).toContainText("review exploded");
  await expect(detail.getByText("run_review_error", { exact: true }).first()).toBeVisible();
});

test("pr retry recovers completed needs attention runs without rerunning work", async ({ page }) => {
  const failureSummary = {
    category: "PR creation failure",
    reason: "pull request creation failed: gh auth failed",
    latest_event: null,
    latest_error: "pull request creation failed: gh auth failed",
    run_id: "run_pr_failed",
    evidence_artifacts: [".harness/runs/run_pr_failed/SUMMARY.md"],
    next_action: "Retry pull request creation after fixing the reported PR error."
  };
  const recoveryAction = {
    kind: "pr_retry",
    label: "Retry PR creation",
    endpoint: "/api/runs/run_pr_failed/pr-retry",
    confirmation: "Retry pull request creation for this completed run?"
  };
  let prCreated = false;

  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Retry pull request creation");
    await dialog.accept();
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            ...boardItem("US-067", "Needs Attention Recovery Action", prCreated ? "Review" : "Needs Attention"),
            run_id: "run_pr_failed",
            reason: prCreated ? "review pull request" : failureSummary.reason,
            failure_summary: prCreated ? null : failureSummary,
            recovery_action: prCreated ? null : recoveryAction
          }
        ]
      })
    });
  });
  await page.route("**/api/runs/run_pr_failed/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_pr_failed",
        story_id: "US-067",
        status: "completed",
        outcome: "completed",
        summary: "Completed work, PR failed.",
        result: null,
        validation: null,
        changed_files: [],
        changeset_preview: null,
        pr_url: prCreated ? "https://example.test/pr/67" : null,
        pr_status: prCreated ? "created" : "failed",
        artifact_paths: failureSummary.evidence_artifacts,
        suggested_next_action: prCreated ? "Review pull request." : failureSummary.next_action,
        failure_summary: prCreated ? null : failureSummary,
        recovery_action: prCreated ? null : recoveryAction,
        events: []
      })
    });
  });
  await page.route("**/api/runs/run_pr_failed/pr-retry", async (route) => {
    prCreated = true;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ run_id: "run_pr_failed", pr_status: "created", pr_url: "https://example.test/pr/67" })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-067/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });

  await expect(detail.getByRole("button", { name: "Retry PR creation" })).toBeVisible();
  await expect(detail.getByRole("button", { name: /Start/ })).toHaveCount(0);
  await detail.getByRole("button", { name: "Retry PR creation" }).click();

  await expect(detail.getByText("https://example.test/pr/67")).toBeVisible();
  await expect(detail.getByRole("button", { name: "Mark Merged" })).toBeEnabled();
});

test("artifact control is explicitly unavailable and long review values stay bounded on mobile", async ({ page }) => {
  const longToken =
    "VeryLongReviewArtifactPathChangedFileBlockerChildAndRunIdentifierThatMustWrapInsideTheMobileDialog1234567890";
  await page.setViewportSize({ width: 390, height: 760 });
  await page.route("**/api/board", async (route) => {
    const item = boardItem("US-069", `Long Review ${longToken}`, "Review");
    item.run_id = `run_${longToken}`;
    item.blockers = [`US-BLOCKER-${longToken}`];
    item.children = [`US-CHILD-${longToken}`];
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ items: [item] }) });
  });
  await page.route(`**/api/runs/run_${longToken}/review`, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: `run_${longToken}`,
        story_id: "US-069",
        status: "completed",
        outcome: "completed",
        summary: `Summary ${longToken}`,
        result: null,
        validation: { artifact: longToken },
        changed_files: [`crates/harness-symphony/web-ui/src/${longToken}.tsx`],
        changeset_preview: null,
        pr_url: null,
        pr_status: "missing",
        artifact_paths: [`.harness/runs/run_${longToken}/APP_SERVER_EVENTS.jsonl`],
        suggested_next_action: `Review long values ${longToken}`,
        failure_summary: null,
        recovery_action: null,
        events: []
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-069/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByRole("button", { name: "Open artifacts" })).toBeDisabled();
  await expect(detail.getByRole("button", { name: "Open artifacts" })).toHaveAttribute("title", /not available/);
  await expectNoHorizontalOverflow(detail, "mobile detail dialog");
});

test("review logs render readable chat and progress entries while preserving raw artifacts", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            id: "US-060",
            title: "Human-Readable Chat Logs",
            board_state: "Review",
            story_status: "implemented",
            lane: "normal",
            verify: "configured",
            blockers: [],
            unblocks: [],
            parent_id: null,
            children: [],
            hierarchy_depth: 0,
            run_id: "run_chat",
            active_run: null,
            reason: "review run communication"
          }
        ]
      })
    });
  });
  await page.route("**/api/runs/run_chat/review", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_chat",
        story_id: "US-060",
        status: "completed",
        agent: "claude-subagent",
        outcome: "completed",
        summary: "Readable logs implemented.",
        result: null,
        validation: { commands: [{ command: "npm --prefix crates/harness-symphony/web-ui run build", result: "pass" }] },
        changed_files: ["crates/harness-symphony/web-ui/src/main.tsx"],
        changeset_preview: null,
        pr_url: "https://example.test/pr/60",
        pr_status: "created",
        artifact_paths: [".harness/runs/run_chat/APP_SERVER_EVENTS.jsonl"],
        suggested_next_action: "Review the readable log.",
        events: [
          { sequence: 1, timestamp: "2026-06-27T09:59:59Z", agent: "claude-subagent", kind: "progress", stage: "agent", message: "Tests passing" },
          { method: "thread/started", params: { thread: { id: "thr_chat" }, timestamp: "2026-06-27T10:00:00Z" } },
          { method: "turn/started", params: { turn: { status: "inProgress" } } },
          { method: "item/agentMessage/delta", params: { itemId: "msg_1", delta: "Implemented " } },
          { method: "item/agentMessage/delta", params: { itemId: "msg_1", delta: "readable logs." } },
          { method: "turn/diff/updated", params: {} },
          { method: "turn/completed", params: { turn: { status: "completed" } } },
          { unsupported: true, note: "kept as fallback" }
        ]
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-060/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });

  await expect(detail.getByRole("heading", { name: "Run communication" })).toBeVisible();
  await expect(detail.getByText("Assistant", { exact: true })).toBeVisible();
  await expect(detail.getByText("Implemented readable logs.")).toBeVisible();
  await expect(detail.getByText("Run started")).toBeVisible();
  await expect(detail.getByText("Workspace diff updated")).toBeVisible();
  await expect(detail.getByText("Run finished")).toBeVisible();
  await expect(detail.getByText("Unsupported event payload with keys: unsupported, note.")).toBeVisible();
  await expect(detail.getByText("Executor")).toBeVisible();
  await expect(detail.getByText("Claude Subagent", { exact: true }).first()).toBeVisible();
  await expect(detail.getByText("Tests passing")).toBeVisible();
  await expect(detail.getByText("Raw artifact: RUN_EVENTS.jsonl")).toBeVisible();
  await expect(detail.getByText(".harness/runs/run_chat/APP_SERVER_EVENTS.jsonl")).toBeVisible();
});

test("main layout is split-pane on desktop", async ({ page }) => {
  await page.goto("/");
  const layout = page.locator("main > div");
  await expect(layout).toHaveClass(/lg:grid-cols-\[240px_minmax\(0,1fr\)\]/);
});

test("view tabs contain Kanban and Table options", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("tab", { name: "Work Board" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Guided Intake" })).toBeVisible();
});

test("detail drawer contains slide transition styles", async ({ page }) => {
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [boardItem("US-090", "Slide Transition Detail", "Ready")] })
    });
  });
  await page.route("**/api/tasks/US-090/context", async (route) => {
    await route.fulfill({ contentType: "application/json", body: JSON.stringify({ story_id: "US-090", content: "# Context" }) });
  });

  await page.goto("/");
  // Trigger opening a card by clicking on a card button
  await page.getByRole("button", { name: /US-/ }).first().click();
  const popup = page.getByTestId("task-detail-popup");
  await expect(popup).toHaveClass(/translate-x-0/);
});

test("runtime events poll with a sequence cursor and cancel run", async ({ page }) => {
  const item = boardItem("US-093", "Agent Runtime Observability And Recovery", "In Progress");
  item.run_id = "run_runtime";
  item.active_run = "run_runtime";
  let eventReads = 0;
  let cancelRequested = false;

  page.on("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Cancel active run run_runtime");
    await dialog.accept();
  });
  await page.route("**/api/board", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [item], task_flow: taskFlow("agent") })
    });
  });
  await page.route("**/api/runs/run_runtime/events*", async (route) => {
    eventReads += 1;
    if (eventReads === 1) {
      expect(new URL(route.request().url()).searchParams.has("after")).toBe(false);
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          run_id: "run_runtime",
          events: [
            {
              sequence: 12,
              timestamp: "2026-07-14T10:30:00Z",
              agent: "opencode",
              kind: "output",
              stage: "agent",
              message: "Running cargo test -p harness-symphony"
            }
          ],
          last_sequence: 12,
          reset_required: false
        })
      });
      return;
    }
    expect(new URL(route.request().url()).searchParams.get("after")).toBe("12");
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_runtime",
        events: [
          {
            sequence: 13,
            timestamp: "2026-07-14T10:30:02Z",
            agent: "opencode",
            kind: "message",
            stage: "agent",
            message: "Tests are still running"
          }
        ],
        last_sequence: 13,
        reset_required: false
      })
    });
  });
  await page.route("**/api/runs/run_runtime/cancel", async (route) => {
    expect(route.request().method()).toBe("POST");
    cancelRequested = true;
    await route.fulfill({
      status: 202,
      contentType: "application/json",
      body: JSON.stringify({
        run_id: "run_runtime",
        status: "cancelling",
        cancel_requested: true
      })
    });
  });

  await page.goto("/");
  await page.getByRole("button", { name: /US-093/ }).click();
  const detail = page.getByRole("dialog", { name: "Selected work detail" });
  await expect(detail.getByText("Running cargo test -p harness-symphony")).toBeVisible();
  await expect(detail.getByText("Tests are still running")).toBeVisible({ timeout: 5000 });

  await detail.getByRole("button", { name: "Cancel run" }).click();
  await expect.poll(async () => cancelRequested).toBe(true);
  await expect(
    page.getByRole("region", { name: "Notifications" }).getByText("Cancellation requested")
  ).toBeVisible();
});
