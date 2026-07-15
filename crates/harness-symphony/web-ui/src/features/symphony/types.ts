export type AgentId = "codex" | "opencode";

export type SettingsResponse = {
  default_agent: AgentId;
};

export type BoardState =
  | "Ready"
  | "Blocked"
  | "In Progress"
  | "Review"
  | "Needs Attention"
  | "Done";

export type BoardBucket = "Drafts" | "Active" | "Ready" | "Done";

export type TaskFlowStepId = "start" | "agent" | "validation" | "pr" | "review" | "sync" | "done";
export type TaskFlowStepState = "pending" | "current" | "complete" | "failed";
export type TaskFlowState = "active" | "waiting" | "failed" | "done";

export type TaskFlow = {
  story_id: string;
  title: string;
  state: TaskFlowState;
  current_step: TaskFlowStepId | null;
  message: string;
  pr_status: string;
  steps: Array<{ id: TaskFlowStepId; state: TaskFlowStepState }>;
  recovery_action: RecoveryAction | null;
};

export type FailureSummary = {
  category: string;
  reason: string;
  latest_event: string | null;
  latest_error: string | null;
  run_id: string;
  evidence_artifacts: string[];
  next_action: string;
};

export type RecoveryAction = {
  kind: "execution_retry" | "pr_retry";
  label: string;
  endpoint: string;
  confirmation: string;
};

export type BoardItem = {
  id: string;
  title: string;
  board_state: BoardState;
  story_status: string;
  lane: string;
  verify: string;
  blockers: string[];
  unblocks: string[];
  parent_id: string | null;
  children: string[];
  hierarchy_depth: number;
  run_id: string | null;
  active_run: string | null;
  reason: string;
  failure_summary: FailureSummary | null;
  recovery_action: RecoveryAction | null;
};

export type BoardResponse = {
  items: BoardItem[];
  task_flow: TaskFlow | null;
};

export type GuidedIntakeDraft = {
  idea: string;
  audience: string;
  outcome: string;
  non_goals: string;
  validation: string;
};

export type CreatedStoryResponse = {
  story_id: string;
  title: string;
  status: string;
};

export type NormalizedRunEvent = { sequence: number; timestamp: string; agent: string; kind: string; stage: string; message: string };
export type RunEvent = NormalizedRunEvent | unknown;

export type EventsResponse = {
  run_id: string;
  events: RunEvent[];
  last_sequence: number;
  reset_required: boolean;
};

export type ReviewResponse = {
  run_id: string;
  story_id: string;
  status: string;
  agent: string;
  outcome: string | null;
  summary: string | null;
  result: unknown | null;
  validation: unknown | null;
  changed_files: string[];
  changeset_preview: string | null;
  pr_url: string | null;
  pr_status: string;
  reviewed_at: number | null;
  reviewer_note: string | null;
  artifact_paths: string[];
  events: RunEvent[];
  suggested_next_action: string;
  failure_summary: FailureSummary | null;
  recovery_action: RecoveryAction | null;
  request_changes: ReviewFeedback | null;
};

export type ReviewFeedback = {
  reason: string;
  reason_path: string;
  evidence: ReviewEvidence[];
};

export type ReviewEvidence = {
  path: string;
  url: string;
  content_type: string;
  size: number;
};

export type ContextResponse = {
  story_id: string;
  content: string;
};

export type TraceItem = {
  id: number;
  story_id: string | null;
  summary: string;
  outcome: string;
  created_at: string;
  duration_seconds: number | null;
  harness_friction: string | null;
};

export type TraceResponse = {
  traces: TraceItem[];
  total: number;
};

export type ToolItem = {
  provider: string;
  name: string;
  kind: string;
  capability: string | null;
  status: string;
  description: string;
  responsibility: string;
  command: string;
  source: string;
  since: string;
  scan_target: string | null;
  checked_at: string | null;
};

export type ToolsResponse = {
  tools: ToolItem[];
};

export type SyncResponse = {
  run_id: string;
  applied: boolean;
};

export type ApproveResponse = {
  run_id: string;
  reviewed_at: number;
};

export type PrMergedResponse = {
  run_id: string;
  pr_status: string;
};

export type PrRetryResponse = {
  run_id: string;
  pr_status: string;
  pr_url: string | null;
};

export type RequestChangesResponse = {
  source_run_id: string;
  run_id: string;
  story_id: string;
  status: string;
  feedback: {
    reason_path: string;
    evidence_paths: string[];
  };
};
