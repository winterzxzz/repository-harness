import React from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  GitPullRequestArrow,
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
import { StatusBadge } from "./status-badge";
import type { BoardItem, FailureSummary, PrMergedResponse, PrRetryResponse, RecoveryAction, ReviewResponse, RunEvent } from "./types";
import { cn } from "../../lib/utils";
import { formatRunLog } from "../../run-log";

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
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/60 px-3 py-4 backdrop-blur-[4px] sm:px-5 lg:py-8"
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
  onClose,
  onStart,
  onRetire,
  onRecover,
  onSync,
  onMarkPrMerged,
  onRetryPr
}: {
  item: BoardItem;
  startingId: string | null;
  deletingId: string | null;
  recoveringId: string | null;
  syncingRunId: string | null;
  markingMergedRunId: string | null;
  retryingPrRunId: string | null;
  onClose: (origin?: HTMLElement) => void;
  onStart: (storyId: string) => Promise<void>;
  onRetire: (item: BoardItem) => Promise<void>;
  onRecover: (storyId: string, action: RecoveryAction) => Promise<void>;
  onSync: (runId: string) => Promise<void>;
  onMarkPrMerged: (runId: string) => Promise<PrMergedResponse>;
  onRetryPr: (runId: string, action: RecoveryAction) => Promise<PrRetryResponse>;
}) {
  const [events, setEvents] = React.useState<RunEvent[]>([]);
  const [reviewState, setReviewState] = React.useState<ReviewState>({ status: "idle" });
  const [preservedFailedReview, setPreservedFailedReview] = React.useState<ReviewResponse | null>(null);
  const dialogRef = React.useRef<HTMLElement>(null);
  const review = reviewState.status === "ready" ? reviewState.data : null;
  const reviewRef = React.useRef<ReviewResponse | null>(null);

  React.useEffect(() => {
    reviewRef.current = review;
  }, [review]);

  React.useEffect(() => {
    dialogRef.current?.focus();
  }, [item.id]);

  React.useEffect(() => {
    setPreservedFailedReview(null);
    setEvents([]);
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
        const data = await fetchEvents(runId, { signal: controller.signal });
        if (!cancelled) {
          setEvents(data.events);
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
          setReviewState({ status: "ready", data });
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
      className="relative max-h-[calc(100dvh-2rem)] min-w-0 w-full max-w-4xl overflow-auto rounded-lg border border-border bg-background shadow-2xl outline-none"
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

      <div className="border-b border-border p-4">
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

      {review ? (
        <div className="border-b border-border p-4">
          <ReviewPanel
            review={review}
            syncing={syncingRunId === review.run_id}
            markingMerged={markingMergedRunId === review.run_id}
            retryingPr={retryingPrRunId === review.run_id}
            onSync={onSync}
            onMarkPrMerged={markReviewPrMerged}
            onRetryPr={retryReviewPr}
          />
        </div>
      ) : null}

      {reviewState.status === "loading" ? <ReviewStatusPanel state={reviewState} /> : null}
      {reviewState.status === "error" ? <ReviewStatusPanel state={reviewState} /> : null}
      {item.active_run && preservedFailedReview ? <PriorFailureEvidence review={preservedFailedReview} /> : null}

      {item.active_run ? <EventLog events={events} live /> : review ? <EventLog events={review.events} /> : null}
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
  onSync,
  onMarkPrMerged,
  onRetryPr
}: {
  review: ReviewResponse;
  syncing: boolean;
  markingMerged: boolean;
  retryingPr: boolean;
  onSync: (runId: string) => Promise<void>;
  onMarkPrMerged: (runId: string) => Promise<void>;
  onRetryPr: (runId: string, action: RecoveryAction) => Promise<void>;
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
    </div>
  );
}

function FailureSummaryPanel({ summary, compact = false }: { summary: FailureSummary; compact?: boolean }) {
  return (
    <div className={cn("rounded-md border border-destructive/30 bg-destructive/10 p-3", compact ? "mt-3" : "")}>
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
    <div>
      <SectionTitle>{title}</SectionTitle>
      <pre className="bounded-text mt-2 max-h-52 max-w-full overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted p-3 text-xs leading-5">
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

function EventLog({ events, live = false }: { events: RunEvent[]; live?: boolean }) {
  const entries = formatRunLog(events).slice(-12);

  return (
    <div id="logs" className="flex flex-col gap-3 p-4" role={live ? "status" : undefined} aria-live={live ? "polite" : undefined}>
      <div className="flex items-baseline justify-between gap-3">
        <SectionTitle>Run communication</SectionTitle>
        <p className="text-xs text-muted-foreground">Raw artifact: APP_SERVER_EVENTS.jsonl</p>
      </div>
      <div className="max-h-80 overflow-auto rounded-md border border-border bg-muted">
        {entries.length > 0 ? (
          entries.map((entry, index) => (
            <div
              key={`${entry.method ?? entry.title}-${index}`}
              className={cn(
                "grid min-h-12 grid-cols-[minmax(0,1fr)] gap-2 border-b border-border/70 px-3 py-3 text-sm last:border-b-0",
                entry.kind === "message" ? "bg-background" : "bg-muted"
              )}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge tone={entry.kind === "message" ? "accent" : entry.kind === "progress" ? "info" : "neutral"}>
                    {entry.source}
                  </Badge>
                  <strong className="font-semibold">{entry.title}</strong>
                </div>
                {entry.timestamp ? <span className="text-xs text-muted-foreground">{entry.timestamp}</span> : null}
              </div>
              <p className="break-words text-sm leading-6 text-muted-foreground">{entry.message}</p>
            </div>
          ))
        ) : (
          <div className="flex min-h-12 items-center px-3 text-sm text-muted-foreground">No run communication yet</div>
        )}
      </div>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border border-border p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="bounded-text mt-1 text-sm font-semibold">{value}</div>
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
      <p className="text-xs font-bold uppercase tracking-widest text-muted-foreground">{title}</p>
      <div className="mt-2 grid min-h-8 gap-2 sm:flex sm:flex-wrap">
        {values.length > 0 ? (
          values.map((value) => (
            <span
              key={value}
              className="bounded-text max-w-full rounded-md border border-border bg-muted px-2 py-1 text-xs font-medium leading-5 text-muted-foreground"
            >
              {value}
            </span>
          ))
        ) : (
          <span className="text-sm text-muted-foreground">{empty}</span>
        )}
      </div>
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h3 className="text-xl font-semibold leading-tight">{children}</h3>;
}
