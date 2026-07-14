import type {
  AgentId,
  BoardItem,
  BoardResponse,
  BoardState,
  ContextResponse,
  CreatedStoryResponse,
  EventsResponse,
  FailureSummary,
  GuidedIntakeDraft,
  PrMergedResponse,
  PrRetryResponse,
  RequestChangesResponse,
  RecoveryAction,
  ReviewEvidence,
  ReviewFeedback,
  ReviewResponse,
  SettingsResponse,
  SyncResponse,
  TaskFlow,
  TaskFlowState,
  TaskFlowStepId,
  TaskFlowStepState,
  ToolItem,
  ToolsResponse,
  TraceItem,
  TraceResponse
} from "./types";

const boardStates: BoardState[] = ["Ready", "Blocked", "In Progress", "Review", "Needs Attention", "Done"];

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status?: number
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function fetchBoard(options?: { signal?: AbortSignal }): Promise<BoardResponse> {
  const response = await fetch("/api/board", { signal: options?.signal });
  return readJson(response, parseBoardResponse, "Board request failed");
}

export async function fetchEvents(runId: string, after?: number, options?: { signal?: AbortSignal }): Promise<EventsResponse> {
  const cursor = after === undefined ? "" : `?after=${after}`;
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/events${cursor}`, { signal: options?.signal });
  return readJson(response, parseEventsResponse, "Events request failed");
}

export async function postCancelRun(runId: string): Promise<void> {
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/cancel`, { method: "POST" });
  await readEmptyOrJson(response, "Cancel failed");
}

export async function fetchReview(runId: string, options?: { signal?: AbortSignal }): Promise<ReviewResponse> {
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/review`, { signal: options?.signal });
  return readJson(response, parseReviewResponse, "Review request failed");
}

export async function fetchContext(storyId: string, options?: { signal?: AbortSignal }): Promise<ContextResponse> {
  const response = await fetch(`/api/tasks/${encodeURIComponent(storyId)}/context`, { signal: options?.signal });
  return readJson(response, parseContextResponse, "Context request failed");
}

export async function fetchTraces(filters?: { storyId?: string; outcome?: string }, options?: { signal?: AbortSignal }): Promise<TraceResponse> {
  const params = new URLSearchParams();
  if (filters?.storyId) {
    params.set("story_id", filters.storyId);
  }
  if (filters?.outcome) {
    params.set("outcome", filters.outcome);
  }
  const query = params.toString();
  const response = await fetch(query ? `/api/traces?${query}` : "/api/traces", { signal: options?.signal });
  return readJson(response, parseTraceResponse, "Trace request failed");
}

export async function fetchTools(options?: { signal?: AbortSignal }): Promise<ToolsResponse> {
  const response = await fetch("/api/tools", { signal: options?.signal });
  return readJson(response, parseToolsResponse, "Tool request failed");
}

export async function postCheckTools(): Promise<void> {
  const response = await fetch("/api/tools/check", { method: "POST" });
  await readJson(response, parseToolCheckResponse, "Tool check failed");
}

export async function postStartTask(storyId: string, agent?: AgentId): Promise<void> {
  const response = await fetch(`/api/tasks/${encodeURIComponent(storyId)}/start`, {
    method: "POST",
    ...(agent
      ? {
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ agent })
        }
      : {})
  });
  await readEmptyOrJson(response, "Start failed");
}

export async function fetchSettings(options?: { signal?: AbortSignal }): Promise<SettingsResponse> {
  const response = await fetch("/api/settings", { signal: options?.signal });
  return readJson(response, parseSettingsResponse, "Settings request failed");
}

export async function putSettings(defaultAgent: AgentId): Promise<SettingsResponse> {
  const response = await fetch("/api/settings", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ default_agent: defaultAgent })
  });
  return readJson(response, parseSettingsResponse, "Settings update failed");
}

export async function postRetireTask(storyId: string): Promise<void> {
  const response = await fetch(`/api/tasks/${encodeURIComponent(storyId)}/retire`, { method: "POST" });
  await readEmptyOrJson(response, "Delete failed");
}

export async function postRecoverTask(action: RecoveryAction): Promise<void> {
  const response = await fetch(action.endpoint, { method: "POST" });
  await readEmptyOrJson(response, "Recovery failed");
}

export async function postSyncRun(runId: string): Promise<SyncResponse> {
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/sync`, { method: "POST" });
  return readJson(response, parseSyncResponse, "Sync failed");
}

export async function postMarkPrMerged(runId: string): Promise<PrMergedResponse> {
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/pr-merged`, { method: "POST" });
  return readJson(response, parsePrMergedResponse, "Merge update failed");
}

export async function postRetryPr(action: RecoveryAction): Promise<PrRetryResponse> {
  const response = await fetch(action.endpoint, { method: "POST" });
  return readJson(response, parsePrRetryResponse, "PR retry failed");
}

export async function postRequestChanges(
  runId: string,
  reason: string,
  files: File[]
): Promise<RequestChangesResponse> {
  const body = new FormData();
  body.append("reason", reason);
  files.forEach((file) => body.append("evidence", file));
  const response = await fetch(`/api/runs/${encodeURIComponent(runId)}/request-changes`, {
    method: "POST",
    body
  });
  return readJson(response, parseRequestChangesResponse, "Request changes failed");
}

export async function postCreateGuidedIntake(draft: GuidedIntakeDraft): Promise<CreatedStoryResponse> {
  const response = await fetch("/api/intake", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(draft)
  });
  return readJson(response, parseCreatedStoryResponse, "Create story failed");
}

async function readJson<T>(response: Response, parser: (value: unknown) => T, fallback: string): Promise<T> {
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    throw new ApiError(errorMessage(body) ?? `${fallback} (${response.status})`, response.status);
  }
  try {
    return parser(body);
  } catch (cause) {
    const detail = cause instanceof Error ? cause.message : "Malformed response";
    throw new ApiError(`${fallback}: ${detail}`, response.status);
  }
}

async function readEmptyOrJson(response: Response, fallback: string): Promise<void> {
  if (response.ok) {
    return;
  }
  const body = await response.json().catch(() => null);
  throw new ApiError(errorMessage(body) ?? `${fallback} (${response.status})`, response.status);
}

function errorMessage(value: unknown): string | null {
  return isRecord(value) && typeof value.error === "string" ? value.error : null;
}

function parseBoardResponse(value: unknown): BoardResponse {
  const record = expectRecord(value, "board response");
  return {
    items: expectArray(record.items, "items").map(parseBoardItem),
    task_flow: record.task_flow === undefined ? null : parseNullable(record.task_flow, parseTaskFlow)
  };
}

const taskFlowStepIds: TaskFlowStepId[] = ["start", "agent", "validation", "pr", "review", "sync", "done"];
const taskFlowStepStates: TaskFlowStepState[] = ["pending", "current", "complete", "failed"];
const taskFlowStates: TaskFlowState[] = ["active", "waiting", "failed", "done"];

function parseTaskFlow(value: unknown): TaskFlow {
  const record = expectRecord(value, "task_flow");
  const state = expectString(record.state, "task_flow.state") as TaskFlowState;
  if (!taskFlowStates.includes(state)) throw new Error("task_flow.state is invalid");
  const currentStep = parseNullableString(record.current_step, "task_flow.current_step") as TaskFlowStepId | null;
  if (currentStep !== null && !taskFlowStepIds.includes(currentStep)) throw new Error("task_flow.current_step is invalid");
  const steps = expectArray(record.steps, "task_flow.steps").map((value, index) => {
    const step = expectRecord(value, `task_flow.steps[${index}]`);
    const id = expectString(step.id, `task_flow.steps[${index}].id`) as TaskFlowStepId;
    const stepState = expectString(step.state, `task_flow.steps[${index}].state`) as TaskFlowStepState;
    if (id !== taskFlowStepIds[index]) throw new Error("task_flow.steps must use canonical order");
    if (!taskFlowStepStates.includes(stepState)) throw new Error(`task_flow.steps[${index}].state is invalid`);
    return { id, state: stepState };
  });
  if (steps.length !== taskFlowStepIds.length) throw new Error("task_flow.steps must contain seven steps");
  return {
    story_id: expectString(record.story_id, "task_flow.story_id"),
    title: expectString(record.title, "task_flow.title"),
    state,
    current_step: currentStep,
    message: expectString(record.message, "task_flow.message"),
    steps,
    recovery_action: parseNullable(record.recovery_action, parseRecoveryAction)
  };
}

function parseBoardItem(value: unknown): BoardItem {
  const record = expectRecord(value, "board item");
  return {
    id: expectString(record.id, "id"),
    title: expectString(record.title, "title"),
    board_state: expectBoardState(record.board_state),
    story_status: expectString(record.story_status, "story_status"),
    lane: expectString(record.lane, "lane"),
    verify: expectString(record.verify, "verify"),
    blockers: parseStringArray(record.blockers, "blockers"),
    unblocks: parseStringArray(record.unblocks, "unblocks"),
    parent_id: parseNullableString(record.parent_id, "parent_id"),
    children: parseStringArray(record.children, "children"),
    hierarchy_depth: typeof record.hierarchy_depth === "number" ? record.hierarchy_depth : 0,
    run_id: parseNullableString(record.run_id, "run_id"),
    active_run: parseNullableString(record.active_run, "active_run"),
    reason: expectString(record.reason, "reason"),
    failure_summary: parseNullable(record.failure_summary, parseFailureSummary),
    recovery_action: parseNullable(record.recovery_action, parseRecoveryAction)
  };
}

function parseFailureSummary(value: unknown): FailureSummary {
  const record = expectRecord(value, "failure_summary");
  return {
    category: expectString(record.category, "category"),
    reason: expectString(record.reason, "reason"),
    latest_event: parseNullableString(record.latest_event, "latest_event"),
    latest_error: parseNullableString(record.latest_error, "latest_error"),
    run_id: expectString(record.run_id, "run_id"),
    evidence_artifacts: parseStringArray(record.evidence_artifacts, "evidence_artifacts"),
    next_action: expectString(record.next_action, "next_action")
  };
}

function parseRecoveryAction(value: unknown): RecoveryAction {
  const record = expectRecord(value, "recovery_action");
  const kind = expectString(record.kind, "kind");
  if (kind !== "execution_retry" && kind !== "pr_retry") {
    throw new Error("recovery_action.kind is invalid");
  }
  return {
    kind,
    label: expectString(record.label, "label"),
    endpoint: expectString(record.endpoint, "endpoint"),
    confirmation: expectString(record.confirmation, "confirmation")
  };
}

function parseEventsResponse(value: unknown): EventsResponse {
  const record = expectRecord(value, "events response");
  return {
    run_id: expectString(record.run_id, "run_id"),
    events: expectArray(record.events, "events"),
    last_sequence: expectNumber(record.last_sequence, "last_sequence"),
    reset_required: record.reset_required === true
  };
}

function parseSettingsResponse(value: unknown): SettingsResponse {
  const record = expectRecord(value, "settings response");
  const agent = expectString(record.default_agent, "default_agent");
  return { default_agent: agent === "opencode" ? "opencode" : "codex" };
}

function parseReviewResponse(value: unknown): ReviewResponse {
  const record = expectRecord(value, "review response");
  return {
    run_id: expectString(record.run_id, "run_id"),
    story_id: expectString(record.story_id, "story_id"),
    status: expectString(record.status, "status"),
    agent: typeof record.agent === "string" ? record.agent : "codex",
    outcome: parseNullableString(record.outcome, "outcome"),
    summary: parseNullableString(record.summary, "summary"),
    result: record.result ?? null,
    validation: record.validation ?? null,
    changed_files: parseStringArray(record.changed_files, "changed_files"),
    changeset_preview: parseNullableString(record.changeset_preview, "changeset_preview"),
    pr_url: parseNullableString(record.pr_url, "pr_url"),
    pr_status: expectString(record.pr_status, "pr_status"),
    artifact_paths: parseStringArray(record.artifact_paths, "artifact_paths"),
    events: expectArray(record.events, "events"),
    suggested_next_action: expectString(record.suggested_next_action, "suggested_next_action"),
    failure_summary: parseNullable(record.failure_summary, parseFailureSummary),
    recovery_action: parseNullable(record.recovery_action, parseRecoveryAction),
    request_changes: parseNullable(record.request_changes, parseReviewFeedback)
  };
}

function parseReviewFeedback(value: unknown): ReviewFeedback {
  const record = expectRecord(value, "request_changes");
  return {
    reason: expectString(record.reason, "request_changes.reason"),
    reason_path: expectString(record.reason_path, "request_changes.reason_path"),
    evidence: expectArray(record.evidence, "request_changes.evidence").map(parseReviewEvidence)
  };
}

function parseReviewEvidence(value: unknown): ReviewEvidence {
  const record = expectRecord(value, "request_changes.evidence item");
  return {
    path: expectString(record.path, "request_changes.evidence.path"),
    url: expectString(record.url, "request_changes.evidence.url"),
    content_type: expectString(record.content_type, "request_changes.evidence.content_type"),
    size: expectNumber(record.size, "request_changes.evidence.size")
  };
}

function parseContextResponse(value: unknown): ContextResponse {
  const record = expectRecord(value, "context response");
  return {
    story_id: expectString(record.story_id, "story_id"),
    content: expectString(record.content, "content")
  };
}

function parseTraceResponse(value: unknown): TraceResponse {
  const record = expectRecord(value, "trace response");
  return {
    traces: expectArray(record.traces, "traces").map(parseTraceItem),
    total: typeof record.total === "number" ? record.total : expectArray(record.traces, "traces").length
  };
}

function parseTraceItem(value: unknown): TraceItem {
  const record = expectRecord(value, "trace item");
  return {
    id: expectNumber(record.id, "id"),
    story_id: parseNullableString(record.story_id, "story_id"),
    summary: expectString(record.summary, "summary"),
    outcome: expectString(record.outcome, "outcome"),
    created_at: expectString(record.created_at, "created_at"),
    duration_seconds: parseNullableNumber(record.duration_seconds, "duration_seconds"),
    harness_friction: parseNullableString(record.harness_friction, "harness_friction")
  };
}

function parseToolsResponse(value: unknown): ToolsResponse {
  const record = expectRecord(value, "tools response");
  return { tools: expectArray(record.tools, "tools").map(parseToolItem) };
}

function parseToolCheckResponse(value: unknown): { tools: unknown[] } {
  const record = expectRecord(value, "tool check response");
  return { tools: expectArray(record.tools, "tools") };
}

function parseToolItem(value: unknown): ToolItem {
  const record = expectRecord(value, "tool item");
  return {
    provider: expectString(record.provider ?? "custom", "provider"),
    name: expectString(record.name, "name"),
    kind: expectString(record.kind, "kind"),
    capability: parseNullableString(record.capability, "capability"),
    status: expectString(record.status, "status"),
    description: expectString(record.description ?? "", "description"),
    responsibility: expectString(record.responsibility ?? "", "responsibility"),
    command: expectString(record.command ?? "", "command"),
    source: expectString(record.source ?? "registered", "source"),
    since: expectString(record.since ?? "", "since"),
    scan_target: parseNullableString(record.scan_target, "scan_target"),
    checked_at: parseNullableString(record.checked_at, "checked_at")
  };
}

function parseSyncResponse(value: unknown): SyncResponse {
  const record = expectRecord(value, "sync response");
  return { run_id: expectString(record.run_id, "run_id"), applied: Boolean(record.applied) };
}

function parsePrMergedResponse(value: unknown): PrMergedResponse {
  const record = expectRecord(value, "pr merged response");
  return { run_id: expectString(record.run_id, "run_id"), pr_status: expectString(record.pr_status, "pr_status") };
}

function parsePrRetryResponse(value: unknown): PrRetryResponse {
  const record = expectRecord(value, "pr retry response");
  return {
    run_id: expectString(record.run_id, "run_id"),
    pr_status: expectString(record.pr_status, "pr_status"),
    pr_url: parseNullableString(record.pr_url, "pr_url")
  };
}

function parseRequestChangesResponse(value: unknown): RequestChangesResponse {
  const record = expectRecord(value, "request changes response");
  const feedback = expectRecord(record.feedback, "request changes feedback");
  return {
    source_run_id: expectString(record.source_run_id, "source_run_id"),
    run_id: expectString(record.run_id, "run_id"),
    story_id: expectString(record.story_id, "story_id"),
    status: expectString(record.status, "status"),
    feedback: {
      reason_path: expectString(feedback.reason_path, "feedback.reason_path"),
      evidence_paths: parseStringArray(feedback.evidence_paths, "feedback.evidence_paths")
    }
  };
}

function parseCreatedStoryResponse(value: unknown): CreatedStoryResponse {
  const record = expectRecord(value, "created story response");
  return {
    story_id: expectString(record.story_id, "story_id"),
    title: expectString(record.title, "title"),
    status: expectString(record.status, "status")
  };
}

function parseNullable<T>(value: unknown, parser: (value: unknown) => T): T | null {
  return value === null || value === undefined ? null : parser(value);
}

function parseNullableString(value: unknown, field: string): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  return expectString(value, field);
}

function parseNullableNumber(value: unknown, field: string): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  return expectNumber(value, field);
}

function parseStringArray(value: unknown, field: string): string[] {
  return expectArray(value, field).map((entry, index) => expectString(entry, `${field}[${index}]`));
}

function expectBoardState(value: unknown): BoardState {
  const found = expectString(value, "board_state");
  if (!boardStates.includes(found as BoardState)) {
    throw new Error(`board_state is invalid: ${found}`);
  }
  return found as BoardState;
}

function expectString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new Error(`${field} must be a string`);
  }
  return value;
}

function expectNumber(value: unknown, field: string): number {
  if (typeof value !== "number") {
    throw new Error(`${field} must be a number`);
  }
  return value;
}

function expectArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${field} must be an array`);
  }
  return value;
}

function expectRecord(value: unknown, field: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`${field} must be an object`);
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
