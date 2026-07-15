import React from "react";
import ReactDOM from "react-dom/client";
import {
  Activity,
  AlertTriangle,
  PanelTop,
  RefreshCw,
  Search
} from "lucide-react";
import { Button } from "./components/ui/button";
import { Card } from "./components/ui/card";
import { Input } from "./components/ui/input";
import {
  fetchBoard,
  fetchSettings,
  postCreateGuidedIntake,
  postApproveRun,
  postCancelRun,
  postMarkPrMerged,
  postRecoverTask,
  postRequestChanges,
  postRetireTask,
  postRetryPr,
  postStartTask,
  postSyncRun,
  putSettings
} from "./features/symphony/api";
import { BoardGrid, SummaryStrip } from "./features/symphony/board";
import { ActiveTaskFlow } from "./features/symphony/active-task-flow";
import { ConfettiBurstHost, TaskDetail, TaskDetailOverlay } from "./features/symphony/detail";
import { GuidedIntakePanel } from "./features/symphony/intake";
import { SettingsPanel } from "./features/symphony/settings";
import { ToolDashboard } from "./features/symphony/tool-dashboard";
import { TraceExplorer } from "./features/symphony/trace-explorer";
import { agentLabel, bucketForItem, buckets } from "./features/symphony/constants";
import { ControllerSidebar } from "./features/symphony/sidebar";
import { ToastProvider, useToast } from "./features/symphony/toast";
import type {
  AgentId,
  ApproveResponse,
  BoardBucket,
  BoardItem,
  GuidedIntakeDraft,
  PrMergedResponse,
  PrRetryResponse,
  RequestChangesResponse,
  RecoveryAction,
  TaskFlow
} from "./features/symphony/types";
import { cn } from "./lib/utils";
import "./styles.css";

type ConfettiBurst = {
  id: number;
  x: number;
  y: number;
};

type AppView = "board" | "intake" | "traces" | "tools" | "settings";

function App() {
  const toast = useToast();
  const [items, setItems] = React.useState<BoardItem[]>([]);
  const [taskFlow, setTaskFlow] = React.useState<TaskFlow | null>(null);
  const [taskFlowStale, setTaskFlowStale] = React.useState(false);
  const [view, setView] = React.useState<AppView>("board");
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [confettiBursts, setConfettiBursts] = React.useState<ConfettiBurst[]>([]);
  const [query, setQuery] = React.useState("");
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [startingId, setStartingId] = React.useState<string | null>(null);
  const [deletingId, setDeletingId] = React.useState<string | null>(null);
  const [recoveringId, setRecoveringId] = React.useState<string | null>(null);
  const [syncingRunId, setSyncingRunId] = React.useState<string | null>(null);
  const [approvingRunId, setApprovingRunId] = React.useState<string | null>(null);
  const [markingMergedRunId, setMarkingMergedRunId] = React.useState<string | null>(null);
  const [retryingPrRunId, setRetryingPrRunId] = React.useState<string | null>(null);
  const [requestingChangesRunId, setRequestingChangesRunId] = React.useState<string | null>(null);
  const [cancellingRunId, setCancellingRunId] = React.useState<string | null>(null);
  const [creatingStory, setCreatingStory] = React.useState(false);
  const [intakeError, setIntakeError] = React.useState<string | null>(null);
  const [defaultAgent, setDefaultAgent] = React.useState<AgentId>("codex");
  const [savingSettings, setSavingSettings] = React.useState(false);
  const [settingsError, setSettingsError] = React.useState<string | null>(null);
  const confettiBurstIdRef = React.useRef(0);
  const boardRequestIdRef = React.useRef(0);
  const selectedOpenerRef = React.useRef<HTMLElement | null>(null);
  const prefersReducedMotion = usePrefersReducedMotion();

  const loadBoard = React.useCallback(async (options?: { silent?: boolean }) => {
    const requestId = (boardRequestIdRef.current += 1);
    if (!options?.silent) {
      setLoading(true);
    }
    if (!options?.silent) {
      setError(null);
    }
    try {
      const data = await fetchBoard();
      if (requestId !== boardRequestIdRef.current) {
        return;
      }
      setItems(data.items);
      setTaskFlow(data.task_flow);
      setTaskFlowStale(false);
    } catch (cause) {
      if (requestId === boardRequestIdRef.current && options?.silent) setTaskFlowStale(true);
      if (requestId === boardRequestIdRef.current && !options?.silent) {
        setError(cause instanceof Error ? cause.message : "Board request failed");
      }
    } finally {
      if (requestId === boardRequestIdRef.current && !options?.silent) {
        setLoading(false);
      }
    }
  }, []);

  React.useEffect(() => {
    void loadBoard();
  }, [loadBoard]);

  React.useEffect(() => {
    const controller = new AbortController();
    fetchSettings({ signal: controller.signal })
      .then((settings) => setDefaultAgent(settings.default_agent))
      .catch(() => {
        // Keep the codex fallback when settings are unavailable.
      });
    return () => controller.abort();
  }, []);

  const filtered = React.useMemo(() => {
    const value = query.trim().toLowerCase();
    return items.filter(
      (item) =>
        value.length === 0 ||
        item.id.toLowerCase().includes(value) ||
        item.title.toLowerCase().includes(value)
    );
  }, [items, query]);
  const selected = selectedId ? items.find((item) => item.id === selectedId) ?? null : null;
  const counts = React.useMemo(
    () =>
      Object.fromEntries(buckets.map((bucket) => [bucket, items.filter((item) => bucketForItem(item) === bucket).length])) as
        Record<BoardBucket, number>,
    [items]
  );
  const activeRun = items.find((item) => item.active_run);
  const selectTask = React.useCallback((id: string) => {
    selectedOpenerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setSelectedId(id);
  }, []);

  React.useEffect(() => {
    const timer = window.setInterval(() => {
      void loadBoard({ silent: true });
    }, activeRun?.active_run ? 1500 : 10000);
    return () => window.clearInterval(timer);
  }, [activeRun?.active_run, loadBoard]);

  const clearConfettiBurst = React.useCallback((id: number) => {
    setConfettiBursts((current) => current.filter((burst) => burst.id !== id));
  }, []);

  const closeSelectedTask = React.useCallback(
    (origin?: HTMLElement) => {
      if (origin && !prefersReducedMotion) {
        const rect = origin.getBoundingClientRect();
        const burst: ConfettiBurst = {
          id: (confettiBurstIdRef.current += 1),
          x: rect.left + rect.width / 2,
          y: rect.top + rect.height / 2
        };
        setConfettiBursts((current) => [...current.slice(-2), burst]);
      }
      setSelectedId(null);
    },
    [prefersReducedMotion]
  );

  const startTask = React.useCallback(
    async (storyId: string, agent?: AgentId) => {
      setStartingId(storyId);
      setError(null);
      try {
        await postStartTask(storyId, agent);
        if (agent) {
          setDefaultAgent(agent);
        }
        await loadBoard();
        toast.add({ kind: "success", title: "Run started", description: `${storyId} is now in progress.` });
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Start failed";
        setError(msg);
        toast.add({ kind: "error", title: "Start failed", description: msg });
      } finally {
        setStartingId(null);
      }
    },
    [loadBoard, toast]
  );

  const runTaskFromBoard = React.useCallback(
    async (item: BoardItem, agent?: AgentId) => {
      const label = agentLabel(agent ?? defaultAgent);
      if (!window.confirm(`Run ${item.id} with ${label}? This starts Symphony and allows ${label} to edit the repository.`)) {
        return;
      }
      await startTask(item.id, agent);
    },
    [defaultAgent, startTask]
  );

  const saveDefaultAgent = React.useCallback(
    async (agent: AgentId) => {
      setSavingSettings(true);
      setSettingsError(null);
      try {
        const settings = await putSettings(agent);
        setDefaultAgent(settings.default_agent);
        toast.add({
          kind: "success",
          title: "Default agent saved",
          description: `New runs start with ${agentLabel(settings.default_agent)}.`
        });
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Settings update failed";
        setSettingsError(msg);
        toast.add({ kind: "error", title: "Settings update failed", description: msg });
      } finally {
        setSavingSettings(false);
      }
    },
    [toast]
  );

  const retireTask = React.useCallback(
    async (item: BoardItem) => {
      if (!window.confirm(`Retire ${item.id} ${item.title}? This removes it from active Ready work without deleting history.`)) {
        return;
      }
      setDeletingId(item.id);
      setError(null);
      try {
        await postRetireTask(item.id);
        setSelectedId(null);
        await loadBoard();
        toast.add({ kind: "success", title: "Task retired", description: `${item.id} removed from active board.` });
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Delete failed";
        setError(msg);
        toast.add({ kind: "error", title: "Retire failed", description: msg });
      } finally {
        setDeletingId(null);
      }
    },
    [loadBoard, toast]
  );

  const recoverTask = React.useCallback(
    async (storyId: string, action: RecoveryAction) => {
      if (!window.confirm(action.confirmation)) {
        return;
      }
      setRecoveringId(storyId);
      setError(null);
      try {
        await postRecoverTask(action);
        await loadBoard();
        toast.add({ kind: "success", title: "Recovery applied", description: `${storyId} recovery action completed.` });
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Recovery failed";
        setError(msg);
        toast.add({ kind: "error", title: "Recovery failed", description: msg });
      } finally {
        setRecoveringId(null);
      }
    },
    [loadBoard, toast]
  );

  const syncRun = React.useCallback(
    async (runId: string) => {
      setSyncingRunId(runId);
      setError(null);
      try {
        const result = await postSyncRun(runId);
        if (!result.applied) {
          const msg = "No new changeset was applied for that run.";
          setError(msg);
          toast.add({ kind: "info", title: "Sync skipped", description: msg });
        } else {
          toast.add({ kind: "success", title: "Sync applied", description: `Run ${runId} changeset applied.` });
        }
        await loadBoard();
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Sync failed";
        setError(msg);
        toast.add({ kind: "error", title: "Sync failed", description: msg });
      } finally {
        setSyncingRunId(null);
      }
    },
    [loadBoard, toast]
  );

  const cancelRun = React.useCallback(
    async (runId: string) => {
      setCancellingRunId(runId);
      try {
        await postCancelRun(runId);
        toast.add({
          kind: "success",
          title: "Cancellation requested",
          description: `Run ${runId} is stopping.`
        });
        await loadBoard();
      } catch (cause) {
        toast.add({
          kind: "error",
          title: "Cancel failed",
          description: cause instanceof Error ? cause.message : "Cancel failed"
        });
      } finally {
        setCancellingRunId(null);
      }
    },
    [loadBoard, toast]
  );

  const markPrMerged = React.useCallback(
    async (runId: string): Promise<PrMergedResponse> => {
      setMarkingMergedRunId(runId);
      setError(null);
      try {
        const result = await postMarkPrMerged(runId);
        await loadBoard();
        toast.add({ kind: "success", title: "PR marked merged", description: `Run ${runId} marked as merged.` });
        return result;
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Merge update failed";
        setError(msg);
        toast.add({ kind: "error", title: "Merge update failed", description: msg });
        throw cause;
      } finally {
        setMarkingMergedRunId(null);
      }
    },
    [loadBoard, toast]
  );

  const approveRun = React.useCallback(
    async (runId: string): Promise<ApproveResponse> => {
      setApprovingRunId(runId);
      setError(null);
      try {
        const result = await postApproveRun(runId);
        await loadBoard();
        toast.add({ kind: "success", title: "Run approved", description: `Run ${runId} is ready to sync.` });
        return result;
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Approval failed";
        setError(msg);
        toast.add({ kind: "error", title: "Approval failed", description: msg });
        throw cause;
      } finally {
        setApprovingRunId(null);
      }
    },
    [loadBoard, toast]
  );

  const retryPr = React.useCallback(
    async (runId: string, action: RecoveryAction): Promise<PrRetryResponse> => {
      if (!window.confirm(action.confirmation)) {
        throw new Error("PR retry cancelled");
      }
      setRetryingPrRunId(runId);
      setError(null);
      try {
        const result = await postRetryPr(action);
        await loadBoard();
        toast.add({ kind: "success", title: "PR retry queued", description: `Run ${runId} PR retry initiated.` });
        return result;
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "PR retry failed";
        setError(msg);
        toast.add({ kind: "error", title: "PR retry failed", description: msg });
        throw cause;
      } finally {
        setRetryingPrRunId(null);
      }
    },
    [loadBoard, toast]
  );

  const requestChanges = React.useCallback(
    async (runId: string, reason: string, files: File[]): Promise<RequestChangesResponse> => {
      setRequestingChangesRunId(runId);
      setError(null);
      try {
        const result = await postRequestChanges(runId, reason, files);
        await loadBoard();
        toast.add({
          kind: "success",
          title: "Changes requested",
          description: `${result.story_id} restarted as ${result.run_id}.`
        });
        return result;
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Request changes failed";
        setError(msg);
        toast.add({ kind: "error", title: "Request changes failed", description: msg });
        throw cause;
      } finally {
        setRequestingChangesRunId(null);
      }
    },
    [loadBoard, toast]
  );

  const createGuidedStory = React.useCallback(
    async (draft: GuidedIntakeDraft) => {
      if (!window.confirm("Create a durable Harness story from this guided intake? This writes intake and story records but does not start Symphony.")) {
        return;
      }
      setCreatingStory(true);
      setIntakeError(null);
      try {
        const story = await postCreateGuidedIntake(draft);
        await loadBoard();
        setQuery(story.story_id);
        setView("board");
        toast.add({ kind: "success", title: "Story created", description: `${story.story_id} added to board.` });
      } catch (cause) {
        const msg = cause instanceof Error ? cause.message : "Create story failed";
        setIntakeError(msg);
        toast.add({ kind: "error", title: "Story creation failed", description: msg });
      } finally {
        setCreatingStory(false);
      }
    },
    [loadBoard, toast]
  );

  function switchView(nextView: AppView) {
    setView(nextView);
    if (nextView !== "board") {
      setSelectedId(null);
    }
  }

  return (
    <main className="min-h-screen bg-muted/15 text-foreground">
      <div className="mx-auto grid w-full max-w-[1760px] grid-cols-1 gap-2 lg:gap-4 p-2 md:p-4 lg:grid-cols-[240px_minmax(0,1fr)] xl:p-5">
        <ControllerSidebar counts={counts} items={items} selectedId={selected?.id ?? null} onSelect={selectTask} />

        <div className="flex min-w-0 flex-col gap-2 lg:gap-4">
          <header className="rounded-xl border border-border bg-card/60 backdrop-blur-md p-2 lg:p-4 shadow-sm">
            <div className="flex flex-col gap-1.5 xl:flex-row xl:items-center xl:justify-between">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2 text-xs font-semibold text-muted-foreground">
                  <span className="inline-flex min-h-7 items-center gap-2 rounded-full border border-border/80 bg-muted/50 px-2.5">
                    <PanelTop className="size-3.5" />
                    Local operations surface
                  </span>
                  <span className="inline-flex min-h-7 items-center gap-2 rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2.5 text-emerald-700 dark:text-emerald-400">
                    <Activity className={cn("size-3.5", activeRun?.active_run && "motion-safe:animate-pulse")} />
                    {activeRun?.active_run ? "Run active" : "No active run"}
                  </span>
                </div>
                <h1 className="cmd-heading-glow mt-1 lg:mt-2 text-2xl font-bold tracking-tight text-foreground md:text-[32px]">
                  Symphony Command Center
                </h1>
                <p className="mt-0.5 lg:mt-1 max-w-3xl text-sm font-medium leading-normal lg:leading-relaxed text-muted-foreground">
                  Start safe work, watch the active run, review evidence, and sync accepted changes from one local controller.
                </p>
                <div role="tablist" aria-label="Command Center views" className="mt-2.5 lg:mt-4 flex max-w-full items-center gap-1 overflow-x-auto rounded-xl bg-muted/40 border border-border/50 p-1 text-muted-foreground">
                  <button
                    type="button"
                    role="tab"
                    aria-selected={view === "board"}
                    className={cn(
                      "inline-flex h-7 lg:h-8 items-center justify-center whitespace-nowrap rounded-lg px-4 text-xs font-bold transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 cursor-pointer",
                      view === "board" ? "bg-background text-foreground shadow-sm border border-border/40 font-bold" : "hover:text-foreground/80 hover:bg-muted/30"
                    )}
                    onClick={() => switchView("board")}
                  >
                    Work Board
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={view === "intake"}
                    className={cn(
                      "inline-flex h-7 lg:h-8 items-center justify-center whitespace-nowrap rounded-lg px-4 text-xs font-bold transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 cursor-pointer",
                      view === "intake" ? "bg-background text-foreground shadow-sm border border-border/40 font-bold" : "hover:text-foreground/80 hover:bg-muted/30"
                    )}
                    onClick={() => switchView("intake")}
                  >
                    Guided Intake
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={view === "traces"}
                    className={cn(
                      "inline-flex h-7 lg:h-8 items-center justify-center whitespace-nowrap rounded-lg px-4 text-xs font-bold transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 cursor-pointer",
                      view === "traces" ? "bg-background text-foreground shadow-sm border border-border/40 font-bold" : "hover:text-foreground/80 hover:bg-muted/30"
                    )}
                    onClick={() => switchView("traces")}
                  >
                    Trace Explorer
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={view === "tools"}
                    className={cn(
                      "inline-flex h-7 lg:h-8 items-center justify-center whitespace-nowrap rounded-lg px-4 text-xs font-bold transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 cursor-pointer",
                      view === "tools" ? "bg-background text-foreground shadow-sm border border-border/40 font-bold" : "hover:text-foreground/80 hover:bg-muted/30"
                    )}
                    onClick={() => switchView("tools")}
                  >
                    Tool Status
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={view === "settings"}
                    className={cn(
                      "inline-flex h-7 lg:h-8 items-center justify-center whitespace-nowrap rounded-lg px-4 text-xs font-bold transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 cursor-pointer",
                      view === "settings" ? "bg-background text-foreground shadow-sm border border-border/40 font-bold" : "hover:text-foreground/80 hover:bg-muted/30"
                    )}
                    onClick={() => switchView("settings")}
                  >
                    Settings
                  </button>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2 xl:justify-end">
                <label className="relative block w-full sm:w-80">
                  <span className="sr-only">Find task</span>
                  <Search className="pointer-events-none absolute left-3 top-2.5 size-4 text-muted-foreground" />
                  <Input
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    className="h-10 bg-muted/50 pl-9 border-border/60"
                    placeholder="Find task or story ID"
                    aria-label="Find task"
                  />
                </label>
                <Button variant="outline" onClick={() => void loadBoard()} disabled={loading} className="h-10 bg-card border-border hover:bg-muted/30">
                  <RefreshCw data-icon="inline-start" className={cn(loading && "motion-safe:animate-spin")} />
                  Refresh
                </Button>
              </div>
            </div>
          </header>

          {view === "board" ? <ActiveTaskFlow flow={taskFlow} stale={taskFlowStale} onRecover={(storyId, action) => void recoverTask(storyId, action)} /> : null}
          {view === "board" ? <SummaryStrip activeRun={activeRun} counts={counts} className="order-1 hidden sm:flex md:order-none md:grid" /> : null}

          {view === "board" && error ? (
            <Card role="alert" className="order-1 flex items-center gap-3 border-destructive/35 bg-destructive/5 p-4 text-sm text-destructive md:order-none rounded-xl">
              <AlertTriangle className="size-4 shrink-0" />
              {error}
            </Card>
          ) : null}

          {view === "board" ? (
            <section
              id="board"
              aria-busy={loading}
              className="command-board-surface order-2 grid min-h-[calc(100dvh-438px)] grid-cols-1 gap-4 rounded-xl border border-border bg-background/25 p-3 md:order-none shadow-sm"
            >
              <div className="sr-only" role="status" aria-live="polite">
                {loading
                  ? "Loading Symphony board."
                  : activeRun?.active_run
                    ? `Active run ${activeRun.active_run} is updating.`
                    : "Symphony board loaded."}
              </div>
              <BoardGrid
                items={filtered}
                selectedId={selected?.id ?? null}
                activeRunId={activeRun?.active_run ?? null}
                startingId={startingId}
                defaultAgent={defaultAgent}
                onSelect={selectTask}
                onRun={runTaskFromBoard}
              />
            </section>
          ) : view === "intake" ? (
            <GuidedIntakePanel creating={creatingStory} error={intakeError} onCreate={createGuidedStory} />
          ) : view === "traces" ? (
            <TraceExplorer />
          ) : view === "tools" ? (
            <ToolDashboard />
          ) : (
            <SettingsPanel
              defaultAgent={defaultAgent}
              saving={savingSettings}
              error={settingsError}
              onSave={saveDefaultAgent}
            />
          )}

          <ConfettiBurstHost bursts={confettiBursts} onBurstDone={clearConfettiBurst} />

          {selected ? (
            <TaskDetailOverlay restoreFocusElement={selectedOpenerRef.current} onClose={() => setSelectedId(null)}>
              <TaskDetail
                item={selected}
                startingId={startingId}
                deletingId={deletingId}
                recoveringId={recoveringId}
                syncingRunId={syncingRunId}
                approvingRunId={approvingRunId}
                markingMergedRunId={markingMergedRunId}
                retryingPrRunId={retryingPrRunId}
                requestingChangesRunId={requestingChangesRunId}
                cancellingRunId={cancellingRunId}
                onClose={closeSelectedTask}
                onStart={startTask}
                onRetire={retireTask}
                onRecover={recoverTask}
                onSync={syncRun}
                onApprove={approveRun}
                onMarkPrMerged={markPrMerged}
                onRetryPr={retryPr}
                onRequestChanges={requestChanges}
                onCancel={cancelRun}
              />
            </TaskDetailOverlay>
          ) : null}

          <p className="order-4 text-xs leading-5 text-muted-foreground md:order-none">
            {view === "board"
              ? "Source: local Symphony API responses for board state, run events, review artifacts, PR status, and sync state."
              : view === "intake"
                ? "Source: local draft state until explicit create writes Harness intake and story records."
                : view === "traces"
                  ? "Source: Harness trace records in the durable database."
                  : view === "tools"
                    ? "Source: Harness tool registry and latest presence scan."
                    : "Source: local Symphony settings; the default agent persists in the Symphony state database."}
          </p>
        </div>
      </div>
    </main>
  );
}

function usePrefersReducedMotion() {
  const [prefersReducedMotion, setPrefersReducedMotion] = React.useState(() =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );

  React.useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    function syncPreference() {
      setPrefersReducedMotion(mediaQuery.matches);
    }

    syncPreference();
    mediaQuery.addEventListener("change", syncPreference);
    return () => mediaQuery.removeEventListener("change", syncPreference);
  }, []);

  return prefersReducedMotion;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ToastProvider>
      <App />
    </ToastProvider>
  </React.StrictMode>
);
