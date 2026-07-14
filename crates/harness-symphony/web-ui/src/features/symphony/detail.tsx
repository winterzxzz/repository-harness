import React from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  GitPullRequestArrow,
  ImagePlus,
  Loader2,
  Play,
  RefreshCw,
  ShieldAlert,
  Trash2,
  X
} from "lucide-react";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Separator } from "../../components/ui/separator";
import { ApiError, fetchEvents, fetchReview } from "./api";
import { ContextViewer } from "./context-viewer";
import { StatusBadge } from "./status-badge";
import type {
  BoardItem,
  FailureSummary,
  PrMergedResponse,
  PrRetryResponse,
  RecoveryAction,
  RequestChangesResponse,
  ReviewFeedback,
  ReviewResponse,
  RunEvent
} from "./types";
import { cn } from "../../lib/utils";
import { agentLabel } from "./constants";
import { RunConsole } from "./run-console";
import { retainRunEvents } from "./run-console-model";

type ConfettiBurst = {
  id: number;
  x: number;
  y: number;
};

export function TaskDetailOverlay({
  children,
  restoreFocusElement,
  onClose
}: {
  children: React.ReactNode;
  restoreFocusElement: HTMLElement | null;
  onClose: () => void;
}) {
  const overlayRef = React.useRef<HTMLDivElement>(null);
  const restoreFocusRef = React.useRef<HTMLElement | null>(null);

  React.useEffect(() => {
    restoreFocusRef.current = restoreFocusElement ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    window.setTimeout(() => {
      focusableElements(overlayRef.current)[0]?.focus();
    }, 0);
    return () => {
      document.body.style.overflow = previousOverflow;
      restoreFocusRef.current?.focus();
    };
  }, [restoreFocusElement]);

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") {
      return;
    }
    const focusable = focusableElements(overlayRef.current);
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex justify-end bg-black/60 backdrop-blur-[4px]"
      data-testid="task-detail-overlay"
      onKeyDown={handleKeyDown}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      {children}
    </div>
  );
}

function focusableElements(root: HTMLElement | null): HTMLElement[] {
  if (!root) {
    return [];
  }
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), details summary, [tabindex]:not([tabindex="-1"])'
    )
  ).filter((element) => !element.hasAttribute("disabled") && element.offsetParent !== null);
}

const confettiPieces = [
  { x: -44, y: -34, color: "#f97316", rotation: "18deg" },
  { x: -28, y: 22, color: "#22c55e", rotation: "-30deg" },
  { x: -10, y: -50, color: "#0ea5e9", rotation: "42deg" },
  { x: 14, y: 28, color: "#eab308", rotation: "-18deg" },
  { x: 34, y: -36, color: "#ec4899", rotation: "28deg" },
  { x: 48, y: 12, color: "#6366f1", rotation: "-42deg" },
  { x: 4, y: -18, color: "#14b8a6", rotation: "12deg" },
  { x: 26, y: -4, color: "#ef4444", rotation: "36deg" }
] as const;

export function ConfettiBurstHost({
  bursts,
  onBurstDone
}: {
  bursts: ConfettiBurst[];
  onBurstDone: (id: number) => void;
}) {
  React.useEffect(() => {
    const timers = bursts.map((burst) => window.setTimeout(() => onBurstDone(burst.id), 900));
    return () => timers.forEach((timer) => window.clearTimeout(timer));
  }, [bursts, onBurstDone]);

  if (bursts.length === 0) {
    return null;
  }

  return (
    <div aria-hidden="true" className="task-close-confetti-host" data-testid="task-close-confetti-host">
      {bursts.map((burst) => (
        <div
          key={burst.id}
          className="task-close-confetti-burst"
          data-testid="task-close-confetti"
          style={{ left: burst.x, top: burst.y }}
        >
          {confettiPieces.map((piece, index) => (
            <span
              key={`${piece.color}-${index}`}
              className="task-close-confetti-piece"
              style={
                {
                  "--confetti-x": `${piece.x}px`,
                  "--confetti-y": `${piece.y}px`,
                  "--confetti-r": piece.rotation,
                  backgroundColor: piece.color
                } as React.CSSProperties
              }
            />
          ))}
        </div>
      ))}
    </div>
  );
}

type ReviewState =
  | { status: "idle" }
  | { status: "loading"; runId: string }
  | { status: "ready"; data: ReviewResponse }
  | { status: "error"; runId: string; message: string; statusCode?: number };

export function TaskDetail({
  item,
  startingId,
  deletingId,
  recoveringId,
  syncingRunId,
  markingMergedRunId,
  retryingPrRunId,
  requestingChangesRunId,
  cancellingRunId,
  onClose,
  onStart,
  onRetire,
  onRecover,
  onSync,
  onMarkPrMerged,
  onRetryPr,
  onRequestChanges,
  onCancel
}: {
  item: BoardItem;
  startingId: string | null;
  deletingId: string | null;
  recoveringId: string | null;
  syncingRunId: string | null;
  markingMergedRunId: string | null;
  retryingPrRunId: string | null;
  requestingChangesRunId: string | null;
  cancellingRunId: string | null;
  onClose: (origin?: HTMLElement) => void;
  onStart: (storyId: string) => Promise<void>;
  onRetire: (item: BoardItem) => Promise<void>;
  onRecover: (storyId: string, action: RecoveryAction) => Promise<void>;
  onSync: (runId: string) => Promise<void>;
  onMarkPrMerged: (runId: string) => Promise<PrMergedResponse>;
  onRetryPr: (runId: string, action: RecoveryAction) => Promise<PrRetryResponse>;
  onRequestChanges: (runId: string, reason: string, files: File[]) => Promise<RequestChangesResponse>;
  onCancel: (runId: string) => Promise<void>;
}) {
  const [events, setEvents] = React.useState<RunEvent[]>([]);
  const [reviewState, setReviewState] = React.useState<ReviewState>({ status: "idle" });
  const [preservedFailedReview, setPreservedFailedReview] = React.useState<ReviewResponse | null>(null);
  const dialogRef = React.useRef<HTMLElement>(null);
  const review = reviewState.status === "ready" ? reviewState.data : null;
  const reviewRef = React.useRef<ReviewResponse | null>(null);
  const eventCursorRef = React.useRef<number | undefined>(undefined);

  React.useEffect(() => {
    reviewRef.current = review;
  }, [review]);

  React.useEffect(() => {
    dialogRef.current?.focus();
  }, [item.id]);

  React.useEffect(() => {
    setPreservedFailedReview(null);
    setEvents([]);
    eventCursorRef.current = undefined;
    setReviewState({ status: "idle" });
  }, [item.id]);

  React.useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    let controller: AbortController | undefined;
    const runId = item.active_run;

    async function loadEvents() {
      if (!runId) {
        setEvents([]);
        return;
      }
      controller?.abort();
      controller = new AbortController();
      try {
        const data = await fetchEvents(runId, eventCursorRef.current, { signal: controller.signal });
        if (!cancelled) {
          setEvents((current) =>
            data.reset_required || eventCursorRef.current === undefined
              ? retainRunEvents([], data.events)
              : retainRunEvents(current, data.events)
          );
          eventCursorRef.current = data.last_sequence;
        }
      } catch (cause) {
        if (!cancelled && !(cause instanceof DOMException && cause.name === "AbortError")) {
          setEvents([]);
        }
      } finally {
        if (!cancelled) {
          timer = window.setTimeout(loadEvents, 2000);
        }
      }
    }

    void loadEvents();
    return () => {
      cancelled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
      controller?.abort();
    };
  }, [item.active_run]);

  React.useEffect(() => {
    let cancelled = false;
    const controller = new AbortController();
    const runId = item.run_id ?? item.active_run;
    if (!runId || !["Review", "Needs Attention", "Done"].includes(item.board_state)) {
      const currentReview = reviewRef.current;
      if (item.board_state === "In Progress" && item.active_run && currentReview?.failure_summary) {
        setPreservedFailedReview(currentReview);
      }
      setReviewState({ status: "idle" });
      return;
    }
    const reviewRunId = runId;
    setReviewState({ status: "loading", runId: reviewRunId });

    async function loadReview() {
      try {
        const data = await fetchReview(reviewRunId, { signal: controller.signal });
        if (!cancelled) {
          setReviewState({
            status: "ready",
            data: { ...data, events: retainRunEvents([], data.events) }
          });
          if (data.failure_summary) {
            setPreservedFailedReview(data);
          }
        }
      } catch (cause) {
        if (!cancelled && !(cause instanceof DOMException && cause.name === "AbortError")) {
          setReviewState({
            status: "error",
            runId: reviewRunId,
            message: cause instanceof Error ? cause.message : "Review request failed",
            statusCode: cause instanceof ApiError ? cause.status : undefined
          });
        }
      }
    }

    void loadReview();
    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [item.active_run, item.board_state, item.run_id]);

  const isReady = item.board_state === "Ready";
  const isStarting = startingId === item.id;
  const isDeleting = deletingId === item.id;
  const executionRecovery = item.recovery_action?.kind === "execution_retry" ? item.recovery_action : null;
  const isRecovering = recoveringId === item.id;
  const needsAttention = item.board_state === "Needs Attention";
  const markReviewPrMerged = React.useCallback(
    async (runId: string) => {
      try {
        const result = await onMarkPrMerged(runId);
        setReviewState((current) =>
          current.status === "ready" && current.data.run_id === result.run_id
            ? { status: "ready", data: { ...current.data, pr_status: result.pr_status } }
            : current
        );
      } catch {
        // The parent action owns the visible error state.
      }
    },
    [onMarkPrMerged]
  );
  const retryReviewPr = React.useCallback(
    async (runId: string, action: RecoveryAction) => {
      try {
        const result = await onRetryPr(runId, action);
        setReviewState((current) =>
          current.status === "ready" && current.data.run_id === result.run_id
            ? { status: "ready", data: { ...current.data, pr_status: result.pr_status, pr_url: result.pr_url } }
            : current
        );
      } catch {
        // The parent action owns the visible error state.
      }
    },
    [onRetryPr]
  );

  return (
    <aside
      aria-label="Selected work detail"
      aria-modal="true"
      className="relative h-full w-full max-w-xl md:max-w-2xl overflow-y-auto border-l border-border/80 bg-card/95 backdrop-blur-md shadow-2xl outline-none transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] translate-x-0"
      data-testid="task-detail-popup"
      ref={dialogRef}
      role="dialog"
      tabIndex={-1}
    >
      <Button
        type="button"
        variant="outline"
        size="icon"
        aria-label="Close selected work detail"
        className="sticky top-3 z-10 float-right m-3 bg-background shadow-sm"
        onClick={(event) => onClose(event.currentTarget)}
      >
        <X data-icon="inline-start" />
      </Button>

      <div className="border-b border-border p-4 pr-16">
        <div className="flex min-w-0 items-center justify-between gap-3">
          <StatusBadge state={item.board_state} className="shrink-0" />
          <span className="bounded-text min-w-0 font-mono text-xs font-bold text-muted-foreground">{item.id}</span>
        </div>
        <h2 className="bounded-text mt-3 text-2xl font-semibold leading-tight tracking-tight">{item.title}</h2>
        <p className="bounded-text mt-2 text-sm leading-6 text-muted-foreground">{item.reason}</p>
        {item.failure_summary ? <FailureSummaryPanel summary={item.failure_summary} compact /> : null}
        <div className="mt-4 grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2">
          <Field label="Lane" value={item.lane} />
          <Field label="Proof" value={item.verify} />
          <Field label="Run" value={item.run_id ?? item.active_run ?? "none"} />
          <Field label="Children" value={item.children.length > 0 ? item.children.join(", ") : "none"} />
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          {item.active_run ? (
            <Button
              variant="destructive"
              disabled={cancellingRunId === item.active_run}
              onClick={() => {
                if (
                  window.confirm(
                    `Cancel active run ${item.active_run}? Partial artifacts will be retained.`
                  )
                ) {
                  void onCancel(item.active_run!);
                }
              }}
            >
              {cancellingRunId === item.active_run ? (
                <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" />
              ) : (
                <X data-icon="inline-start" />
              )}
              Cancel run
            </Button>
          ) : null}
          {executionRecovery ? (
            <Button disabled={isRecovering} title={executionRecovery.confirmation} onClick={() => void onRecover(item.id, executionRecovery)}>
              {isRecovering ? (
                <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" />
              ) : (
                <RefreshCw data-icon="inline-start" />
              )}
              {executionRecovery.label}
            </Button>
          ) : needsAttention ? (
            <Button variant="outline" disabled title="Use the recovery action in review evidence">
              <ShieldAlert data-icon="inline-start" />
              Recovery unavailable
            </Button>
          ) : (
            <Button
              disabled={!isReady || isStarting}
              title={isReady ? "Start task" : "Blocked tasks cannot start"}
              onClick={() => void onStart(item.id)}
            >
              {isStarting ? <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" /> : <Play data-icon="inline-start" />}
              {isReady ? "Start work" : item.board_state === "In Progress" ? "One run active" : "Start blocked"}
            </Button>
          )}
          <Button variant="secondary" disabled title="Artifact file opening is not available in the browser controller yet; review artifact paths below.">
            <Clock3 data-icon="inline-start" />
            Open artifacts
          </Button>
          {isReady ? (
            <Button
              variant="outline"
              disabled={isDeleting}
              title="Retire this Ready story"
              onClick={() => void onRetire(item)}
            >
              {isDeleting ? <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" /> : <Trash2 data-icon="inline-start" />}
              Delete work story
            </Button>
          ) : null}
        </div>
      </div>

      <div className="flex flex-col gap-4 border-b border-border p-4">
        <SectionTitle>Dependencies</SectionTitle>
        <ListBlock title="Blocked by" values={item.blockers} empty="No blockers" />
        <ListBlock title="Unblocks" values={item.unblocks} empty="No downstream work in this slice." />
        <HierarchyBlock item={item} />
      </div>

      <ContextViewer storyId={item.id} />

      {review ? (
        <div className="border-b border-border p-4">
          <ReviewPanel
            review={review}
            syncing={syncingRunId === review.run_id}
            markingMerged={markingMergedRunId === review.run_id}
            retryingPr={retryingPrRunId === review.run_id}
            requestingChanges={requestingChangesRunId === review.run_id}
            allowRequestChanges={item.board_state === "Review"}
            onSync={onSync}
            onMarkPrMerged={markReviewPrMerged}
            onRetryPr={retryReviewPr}
            onRequestChanges={onRequestChanges}
          />
        </div>
      ) : null}

      {reviewState.status === "loading" ? <ReviewStatusPanel state={reviewState} /> : null}
      {reviewState.status === "error" ? <ReviewStatusPanel state={reviewState} /> : null}
      {item.active_run && preservedFailedReview ? <PriorFailureEvidence review={preservedFailedReview} /> : null}

      {item.active_run ? (
        <RunConsole events={events} live agent={review?.agent} />
      ) : review ? (
        <RunConsole events={review.events} agent={review.agent} />
      ) : null}
    </aside>
  );
}

function HierarchyBlock({ item }: { item: BoardItem }) {
  return (
    <div className="flex flex-col gap-2">
      <SectionTitle>Hierarchy</SectionTitle>
      <div className="grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2">
        <Field label="Parent" value={item.parent_id ?? "top level"} />
        <Field label="Depth" value={String(item.hierarchy_depth)} />
      </div>
      <ListBlock title="Child tasks" values={item.children} empty="No child tasks" />
    </div>
  );
}

function ReviewPanel({
  review,
  syncing,
  markingMerged,
  retryingPr,
  requestingChanges,
  allowRequestChanges,
  onSync,
  onMarkPrMerged,
  onRetryPr,
  onRequestChanges
}: {
  review: ReviewResponse;
  syncing: boolean;
  markingMerged: boolean;
  retryingPr: boolean;
  requestingChanges: boolean;
  allowRequestChanges: boolean;
  onSync: (runId: string) => Promise<void>;
  onMarkPrMerged: (runId: string) => Promise<void>;
  onRetryPr: (runId: string, action: RecoveryAction) => Promise<void>;
  onRequestChanges: (runId: string, reason: string, files: File[]) => Promise<RequestChangesResponse>;
}) {
  const canMarkMerged = review.pr_status === "created" && review.pr_url !== null;
  const canSync = review.pr_status === "merged" && review.status === "completed";
  const prRecovery = review.recovery_action?.kind === "pr_retry" ? review.recovery_action : null;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <SectionTitle>Review evidence</SectionTitle>
          <p className="bounded-text mt-1 text-sm leading-6 text-muted-foreground">{review.suggested_next_action}</p>
        </div>
        <Badge tone={review.pr_status === "created" ? "accent" : "danger"} className="shrink-0">
          {review.pr_status}
        </Badge>
      </div>

      <div className="grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2">
        <Field label="Outcome" value={review.outcome ?? "unknown"} />
        <Field label="Status" value={review.status} />
        <Field label="Executor" value={agentLabel(review.agent)} />
      </div>

      {review.failure_summary ? <FailureSummaryPanel summary={review.failure_summary} /> : null}

      {review.pr_url ? (
        <a
          className="block break-all rounded-sm border border-border px-3 py-2 text-sm text-primary hover:bg-accent"
          href={review.pr_url}
          target="_blank"
          rel="noreferrer"
        >
          {review.pr_url}
        </a>
      ) : null}

      {review.summary ? <TextBlock title="Summary" text={review.summary} /> : null}
      {review.validation ? <TextBlock title="Validation" text={JSON.stringify(review.validation, null, 2)} /> : null}
      <ListBlock title="Changed files" values={review.changed_files} empty="No changed files listed" />
      {review.changeset_preview ? <TextBlock title="Changeset" text={review.changeset_preview} /> : null}
      <ListBlock title="Artifacts" values={review.artifact_paths} empty="No artifacts found" />
      {review.request_changes ? <RequestChangesHistory feedback={review.request_changes} /> : null}

      <Separator />
      <div className="flex flex-wrap gap-2">
        {prRecovery ? (
          <Button variant="outline" disabled={retryingPr} onClick={() => void onRetryPr(review.run_id, prRecovery)}>
            {retryingPr ? (
              <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" />
            ) : (
              <GitPullRequestArrow data-icon="inline-start" />
            )}
            {prRecovery.label}
          </Button>
        ) : null}
        <Button variant="outline" disabled={!canMarkMerged || markingMerged} onClick={() => void onMarkPrMerged(review.run_id)}>
          {markingMerged ? (
            <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" />
          ) : (
            <GitPullRequestArrow data-icon="inline-start" />
          )}
          Mark Merged
        </Button>
        <Button disabled={!canSync || syncing} onClick={() => void onSync(review.run_id)}>
          {syncing ? <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" /> : <CheckCircle2 data-icon="inline-start" />}
          Approve Sync
        </Button>
      </div>
      {allowRequestChanges ? (
        <RequestChangesForm
          runId={review.run_id}
          submitting={requestingChanges}
          onRequestChanges={onRequestChanges}
        />
      ) : null}
    </div>
  );
}

const MAX_REQUEST_CHANGES_FILES = 3;
const MAX_REQUEST_CHANGES_FILE_BYTES = 5 * 1024 * 1024;
const REQUEST_CHANGES_IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/webp"]);

function RequestChangesForm({
  runId,
  submitting,
  onRequestChanges
}: {
  runId: string;
  submitting: boolean;
  onRequestChanges: (runId: string, reason: string, files: File[]) => Promise<RequestChangesResponse>;
}) {
  const [reason, setReason] = React.useState("");
  const [files, setFiles] = React.useState<File[]>([]);
  const [validationError, setValidationError] = React.useState<string | null>(null);
  const [dragging, setDragging] = React.useState(false);
  const [previews, setPreviews] = React.useState<{ file: File; url: string }[]>([]);

  React.useEffect(() => {
    const nextPreviews = files.map((file) => ({ file, url: URL.createObjectURL(file) }));
    setPreviews(nextPreviews);
    return () => {
      nextPreviews.forEach((preview) => URL.revokeObjectURL(preview.url));
    };
  }, [files]);

  React.useEffect(() => {
    setReason("");
    setFiles([]);
    setValidationError(null);
  }, [runId]);

  function addFiles(incoming: File[]) {
    setValidationError(null);
    if (files.length + incoming.length > MAX_REQUEST_CHANGES_FILES) {
      setValidationError("Attach up to 3 evidence images.");
      return;
    }
    const unsupported = incoming.find((file) => !REQUEST_CHANGES_IMAGE_TYPES.has(file.type));
    if (unsupported) {
      setValidationError(`${unsupported.name} must be PNG, JPEG, or WebP.`);
      return;
    }
    const oversized = incoming.find((file) => file.size > MAX_REQUEST_CHANGES_FILE_BYTES);
    if (oversized) {
      setValidationError(`${oversized.name} exceeds the 5 MB limit.`);
      return;
    }
    setFiles((current) => [...current, ...incoming]);
  }

  function removeFile(index: number) {
    setFiles((current) => current.filter((_, currentIndex) => currentIndex !== index));
    setValidationError(null);
  }

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedReason = reason.trim();
    if (!trimmedReason) {
      setValidationError("Enter a reason before requesting changes.");
      return;
    }
    setValidationError(null);
    try {
      await onRequestChanges(runId, trimmedReason, files);
      setReason("");
      setFiles([]);
    } catch (cause) {
      setValidationError(cause instanceof Error ? cause.message : "Request changes failed");
    }
  }

  const inputId = `request-changes-evidence-${runId}`;
  const reasonLength = reason.length;
  const disabled = submitting || reason.trim().length === 0 || validationError !== null;

  return (
    <form className="rounded-xl border border-violet-200/80 bg-violet-50/45 p-4" onSubmit={(event) => void submit(event)}>
      <div className="flex items-start gap-3">
        <span className="mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-violet-200/60 text-violet-900">
          <RefreshCw className="size-4" aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <h3 className="text-sm font-bold text-foreground">Request changes</h3>
          <p className="mt-1 text-sm leading-6 text-muted-foreground">
            Preserve this run, attach clear evidence, and start a replacement run for the same story.
          </p>
        </div>
      </div>

      <label className="mt-4 grid gap-2">
        <span className="flex items-center justify-between gap-3 text-xs font-bold text-muted-foreground">
          <span>Change reason</span>
          <span aria-live="polite" className="font-mono font-medium">
            {reasonLength}/2000
          </span>
        </span>
        <textarea
          aria-label="Request changes reason"
          className="min-h-24 resize-y rounded-lg border border-input bg-background p-3 text-sm leading-6 text-foreground outline-none transition-colors focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30"
          maxLength={2000}
          placeholder="Describe what is not acceptable and what should change."
          value={reason}
          onChange={(event) => {
            setReason(event.target.value);
            if (validationError?.startsWith("Enter a reason")) {
              setValidationError(null);
            }
          }}
        />
      </label>

      <div className="mt-4 grid gap-2">
        <div className="flex items-center justify-between gap-3 text-xs font-bold text-muted-foreground">
          <span>Evidence images</span>
          <span aria-live="polite">{files.length}/3 images</span>
        </div>
        <div
          aria-label="Image evidence drop zone"
          className={cn(
            "rounded-xl border border-dashed p-4 transition-colors",
            dragging ? "border-violet-500 bg-violet-200/35" : "border-violet-200 bg-background/70"
          )}
          onDragEnter={(event) => {
            event.preventDefault();
            setDragging(true);
          }}
          onDragOver={(event) => event.preventDefault()}
          onDragLeave={() => setDragging(false)}
          onDrop={(event) => {
            event.preventDefault();
            setDragging(false);
            addFiles(Array.from(event.dataTransfer.files));
          }}
        >
          <input
            id={inputId}
            aria-label="Evidence images"
            className="sr-only"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            multiple
            onChange={(event) => {
              addFiles(Array.from(event.currentTarget.files ?? []));
              event.currentTarget.value = "";
            }}
          />
          <label htmlFor={inputId} className="flex cursor-pointer items-center gap-3 rounded-lg outline-none focus-within:ring-2 focus-within:ring-ring">
            <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg bg-violet-200/55 text-violet-900">
              <ImagePlus className="size-4" aria-hidden="true" />
            </span>
            <span className="min-w-0">
              <span className="block text-sm font-semibold text-foreground">Choose images or drop them here</span>
              <span className="mt-0.5 block text-xs leading-5 text-muted-foreground">PNG, JPEG, or WebP; 5 MB each.</span>
            </span>
          </label>
        </div>
      </div>

      {previews.length > 0 ? (
        <div className="mt-3 grid gap-2 sm:grid-cols-2">
          {previews.map(({ file, url }, index) => (
            <div key={`${file.name}-${file.lastModified}-${index}`} className="flex min-w-0 items-center gap-3 rounded-lg border border-border bg-background p-2">
              <img src={url} alt={`Preview ${file.name}`} className="size-14 shrink-0 rounded-md bg-muted object-contain" />
              <div className="min-w-0 flex-1">
                <p className="bounded-text text-xs font-semibold text-foreground">{file.name}</p>
                <p className="mt-1 text-xs text-muted-foreground">{formatFileSize(file.size)}</p>
              </div>
              <Button type="button" variant="ghost" size="icon" aria-label={`Remove ${file.name}`} onClick={() => removeFile(index)}>
                <X aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      ) : null}

      {validationError ? (
        <div role="alert" className="mt-3 flex items-start justify-between gap-3 rounded-lg bg-destructive/10 px-3 py-2 text-sm font-medium text-destructive">
          <span>{validationError}</span>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label="Dismiss evidence error"
            className="size-7 shrink-0 text-destructive hover:bg-destructive/10 hover:text-destructive"
            onClick={() => setValidationError(null)}
          >
            <X aria-hidden="true" />
          </Button>
        </div>
      ) : null}

      <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-md text-xs leading-5 text-muted-foreground">The previous run and its artifacts remain available for review.</p>
        <Button type="submit" variant="outline" disabled={disabled}>
          {submitting ? <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" /> : <RefreshCw data-icon="inline-start" />}
          Request changes
        </Button>
      </div>
    </form>
  );
}

function RequestChangesHistory({ feedback }: { feedback: ReviewFeedback }) {
  return (
    <section aria-label="Previous request changes" className="rounded-xl bg-violet-50/60 p-4">
      <div className="flex items-center gap-2">
        <RefreshCw className="size-4 text-violet-800" aria-hidden="true" />
        <h3 className="text-sm font-bold text-foreground">Previous request changes</h3>
      </div>
      <p className="bounded-text mt-2 text-sm leading-6 text-foreground">{feedback.reason}</p>
      <p className="bounded-text mt-2 font-mono text-xs text-muted-foreground">{feedback.reason_path}</p>
      {feedback.evidence.length > 0 ? (
        <div className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
          {feedback.evidence.map((evidence, index) => (
            <figure key={evidence.path} className="min-w-0 overflow-hidden rounded-lg bg-background">
              <img
                src={evidence.url}
                alt={`Request changes evidence ${index + 1}`}
                className="h-32 w-full bg-muted object-contain"
              />
              <figcaption className="p-2">
                <p className="bounded-text font-mono text-xs text-foreground">{evidence.path.split("/").pop()}</p>
                <p className="mt-1 text-xs text-muted-foreground">{formatFileSize(evidence.size)}</p>
              </figcaption>
            </figure>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function FailureSummaryPanel({ summary, compact = false }: { summary: FailureSummary; compact?: boolean }) {
  return (
    <div className={cn("rounded-xl border border-destructive/35 bg-destructive/5 p-4 shadow-sm", compact ? "mt-4" : "")}>
      <div className="flex min-w-0 flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 max-w-full flex-1 items-center gap-2">
          <AlertTriangle className="size-4 shrink-0 text-destructive" />
          <strong className="bounded-text block min-w-0 max-w-full text-sm font-bold">{summary.category}</strong>
        </div>
        <Badge tone="danger" className="max-w-full break-all">
          {summary.run_id}
        </Badge>
      </div>
      <p className="bounded-text mt-2 text-sm leading-6 text-foreground">{summary.reason}</p>
      <div className="mt-3 grid grid-cols-1 gap-2 md:grid-cols-2">
        <Field label="Latest event" value={summary.latest_event ?? "none"} />
        <Field label="Latest error" value={summary.latest_error ?? "none"} />
      </div>
      <p className="bounded-text mt-3 text-sm leading-6 text-muted-foreground">{summary.next_action}</p>
      {!compact ? (
        <ListBlock title="Failure evidence" values={summary.evidence_artifacts} empty="No evidence artifacts found" />
      ) : null}
    </div>
  );
}

function TextBlock({ title, text }: { title: string; text: string }) {
  return (
    <div className="flex flex-col gap-2">
      <SectionTitle>{title}</SectionTitle>
      <pre className="bounded-text max-h-52 max-w-full overflow-auto whitespace-pre-wrap rounded-xl border border-border/80 bg-background/50 p-4 text-xs font-mono leading-relaxed text-foreground/90 shadow-inner">
        {text}
      </pre>
    </div>
  );
}

function ReviewStatusPanel({ state }: { state: Extract<ReviewState, { status: "loading" | "error" }> }) {
  if (state.status === "loading") {
    return (
      <div className="border-b border-border p-4" role="status" aria-live="polite">
        <div className="flex items-center gap-2 rounded-md border border-border bg-muted p-3 text-sm text-muted-foreground">
          <Loader2 className="size-4 motion-safe:animate-spin" />
          Loading review evidence for {state.runId}.
        </div>
      </div>
    );
  }
  return (
    <div className="border-b border-border p-4">
      <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive" role="alert">
        <strong className="block font-bold">Review evidence unavailable</strong>
        <span className="mt-1 block">
          {state.statusCode ? `HTTP ${state.statusCode}: ` : ""}
          {state.message}
        </span>
        <span className="bounded-text mt-2 block text-destructive">Refresh the board or inspect the run artifacts for {state.runId}.</span>
      </div>
    </div>
  );
}

function PriorFailureEvidence({ review }: { review: ReviewResponse }) {
  return (
    <div className="border-b border-border p-4" aria-label="Prior failed run evidence">
      <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3">
        <SectionTitle>Prior failed run evidence</SectionTitle>
        <p className="mt-1 text-sm leading-6 text-muted-foreground">
          Preserved from {review.run_id} while the active retry streams below.
        </p>
        {review.failure_summary ? <FailureSummaryPanel summary={review.failure_summary} /> : null}
        <ListBlock title="Prior artifacts" values={review.artifact_paths} empty="No prior artifacts found" />
      </div>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-xl border border-border/70 bg-background/30 p-3.5 shadow-sm transition-all hover:bg-background/50">
      <div className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">{label}</div>
      <div className="bounded-text mt-1.5 text-sm font-bold text-foreground leading-snug">{value}</div>
    </div>
  );
}

function ListBlock({
  title,
  values,
  empty
}: {
  title: string;
  values: string[];
  empty: string;
}) {
  return (
    <div>
      <p className="text-xs font-bold uppercase tracking-widest text-muted-foreground/70">{title}</p>
      <div className="mt-2.5 grid min-h-8 gap-2 sm:flex sm:flex-wrap">
        {values.length > 0 ? (
          values.map((value) => (
            <span
              key={value}
              className="bounded-text max-w-full rounded-md border border-border/60 bg-muted/40 px-2.5 py-1 text-xs font-semibold leading-normal text-muted-foreground shadow-sm hover:border-border transition-colors duration-150"
            >
              {value}
            </span>
          ))
        ) : (
          <span className="text-sm text-muted-foreground italic font-medium">{empty}</span>
        )}
      </div>
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h3 className="text-lg font-bold tracking-tight text-foreground">{children}</h3>;
}
