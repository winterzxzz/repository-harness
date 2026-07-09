import { AlertTriangle, GitPullRequestArrow, Play, PlayCircle, Radio, ShieldAlert } from "lucide-react";
import { Badge } from "../../components/ui/badge";
import { Card } from "../../components/ui/card";
import { Button } from "../../components/ui/button";
import { cn } from "../../lib/utils";
import { columnId, stateIcon, states } from "./constants";
import { StatusBadge, toneForState } from "./status-badge";
import type { BoardItem, BoardState } from "./types";

export function SummaryStrip({
  activeRun,
  counts,
  className
}: {
  activeRun: BoardItem | undefined;
  counts: Record<BoardState, number>;
  className?: string;
}) {
  const metrics = [
    {
      label: "Active run",
      value: activeRun?.id ?? "none",
      detail: activeRun?.active_run ? `${activeRun.active_run} is the only task allowed in progress.` : "No active Symphony run.",
      icon: Radio,
      className: activeRun?.active_run ? "border-blue-500/30 bg-blue-500/5 text-blue-800 dark:text-blue-400" : "border-border bg-card text-muted-foreground"
    },
    {
      label: "Safe to start",
      value: `${counts.Ready} ready`,
      detail: "Ready tasks have no incomplete blockers.",
      icon: PlayCircle,
      className: "border-emerald-500/30 bg-emerald-500/5 text-emerald-800 dark:text-emerald-400"
    },
    {
      label: "Avoid for now",
      value: `${counts.Blocked} blocked`,
      detail: "Blocked work explains its missing dependency before action.",
      icon: ShieldAlert,
      className: "border-zinc-500/25 bg-zinc-500/5 text-zinc-700 dark:text-zinc-400"
    },
    {
      label: "Needs decision",
      value: `${counts.Review} review`,
      detail: "Merge PR first, then approve local sync.",
      icon: GitPullRequestArrow,
      className: "border-violet-500/30 bg-violet-500/5 text-violet-800 dark:text-violet-400"
    }
  ];

  return (
    <section
      aria-label="Command status rail"
      className={cn("scrollbar-none flex gap-2 lg:gap-3 overflow-x-auto md:grid md:grid-cols-2 md:overflow-visible xl:grid-cols-[1.2fr_.9fr_.9fr_1fr]", className)}
    >
      {metrics.map((metric) => {
        const Icon = metric.icon;
        return (
          <Card 
            key={metric.label} 
            className={cn(
              "min-w-[210px] rounded-xl p-3 lg:p-4 md:min-w-0 transition-all duration-200 hover:-translate-y-0.5 hover:shadow-md cursor-default border border-border bg-card", 
              metric.className
            )}
          >
            <div className="flex items-start gap-2.5">
              <span className={cn(
                "grid size-8 lg:size-9 shrink-0 place-items-center rounded-lg border border-current/15 bg-background/40 shadow-sm",
                metric.label === "Active run" && activeRun?.active_run && "motion-safe:animate-pulse"
              )}>
                <Icon className="size-4 lg:size-4.5" />
              </span>
              <div className="min-w-0 flex-1">
                <span className="text-[10px] font-bold uppercase tracking-wider text-current/60">{metric.label}</span>
                <strong className="mt-0.5 lg:mt-1 block truncate text-lg font-bold leading-none tracking-tight text-foreground">{metric.value}</strong>
                <p className="mt-1 lg:mt-1.5 text-xs leading-tight lg:leading-normal text-current/75 font-medium">{metric.detail}</p>
              </div>
            </div>
          </Card>
        );
      })}
    </section>
  );
}

export function BoardGrid({
  items,
  selectedId,
  activeRunId,
  startingId,
  onSelect,
  onRun
}: {
  items: BoardItem[];
  selectedId: string | null;
  activeRunId: string | null;
  startingId: string | null;
  onSelect: (id: string) => void;
  onRun: (item: BoardItem) => Promise<void>;
}) {
  return (
    <div className="min-h-0 min-w-0 overflow-x-auto">
      <div className="grid h-[calc(100dvh-244px)] min-h-[410px] min-w-[1500px] grid-cols-[repeat(6,minmax(240px,1fr))] items-stretch gap-2.5 lg:gap-3 max-sm:h-auto max-sm:min-h-0 max-sm:min-w-0 max-sm:grid-cols-1">
        {states.map((state) => {
          const stateItems = items.filter((item) => item.board_state === state);
          const Icon = stateIcon[state];
          return (
            <section
              key={state}
              id={columnId(state)}
              aria-label={`${state} column`}
              className={cn(
                "flex h-full min-h-0 flex-col overflow-hidden rounded-xl border bg-muted/20 shadow-sm max-sm:h-[min(520px,calc(100dvh-180px))] max-sm:min-h-[320px]",
                columnChrome[state]
              )}
            >
              <div className="flex min-h-12 items-center justify-between gap-2 border-b border-border bg-card/60 backdrop-blur-sm px-3 py-2">
                <div className="flex items-center gap-2">
                  <span className={cn("grid size-6 place-items-center rounded-md border", columnIcon[state])}>
                    <Icon className={cn("size-3.5", state === "In Progress" && "motion-safe:animate-spin")} />
                  </span>
                  <h2 className="text-sm font-bold tracking-tight text-foreground">{state}</h2>
                </div>
                <StatusBadge state={state}>{stateItems.length}</StatusBadge>
              </div>
              <div aria-label={`${state} tasks`} className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-2 scrollbar-thin">
                {stateItems.map((item) => (
                  <TaskCard
                    key={item.id}
                    item={item}
                    selected={item.id === selectedId}
                    activeRunId={activeRunId}
                    startingId={startingId}
                    onSelect={onSelect}
                    onRun={onRun}
                  />
                ))}
                {stateItems.length === 0 ? (
                  <div className="flex min-h-24 items-center justify-center rounded-lg border border-dashed border-border/80 bg-card/45 px-3 text-center text-xs text-muted-foreground font-medium">
                    No tasks
                  </div>
                ) : null}
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}

function TaskCard({
  item,
  selected,
  activeRunId,
  startingId,
  onSelect,
  onRun
}: {
  item: BoardItem;
  selected: boolean;
  activeRunId: string | null;
  startingId: string | null;
  onSelect: (id: string) => void;
  onRun: (item: BoardItem) => Promise<void>;
}) {
  const blocked = item.board_state === "Blocked";
  const done = item.board_state === "Done";
  const canRun = item.board_state === "Ready" && item.verify === "configured" && activeRunId === null;
  const runDisabled = item.board_state !== "Ready" || item.verify !== "configured" || activeRunId !== null || startingId === item.id;

  return (
    <div
      className={cn(
        "group block min-h-[136px] w-full min-w-0 shrink-0 overflow-hidden rounded-lg border bg-card p-3 text-left transition-all duration-200 hover:-translate-y-0.5 hover:shadow-md focus-within:ring-2 focus-within:ring-ring cursor-pointer",
        item.board_state === "Ready" && "task-card-ready",
        item.board_state === "Blocked" && "task-card-blocked",
        item.board_state === "In Progress" && "task-card-progress active-run-card",
        item.board_state === "Review" && "task-card-review",
        item.board_state === "Needs Attention" && "task-card-attention",
        item.board_state === "Done" && "task-card-done",
        selected && "border-primary ring-2 ring-ring/25 shadow-md"
      )}
      data-testid="task-card"
    >
      <button 
        type="button" 
        onClick={() => onSelect(item.id)} 
        className="block w-full min-w-0 text-left focus-visible:outline-none cursor-pointer"
      >
        <div className="flex min-w-0 items-center justify-between gap-2">
          <span className="flex min-w-0 items-center gap-1.5 truncate font-mono text-[10px] font-bold text-muted-foreground">
            <span className={cn("size-2 shrink-0 rounded-full", stateDot[item.board_state])} />
            <span className="min-w-0 truncate">{item.id}</span>
          </span>
          <Badge className="max-w-[58%] shrink-0 truncate" tone={item.verify === "configured" ? toneForState(item.board_state) : "neutral"}>
            {item.board_state === "In Progress" ? "active" : item.verify}
          </Badge>
        </div>
        <h3 className="bounded-text mt-2 line-clamp-3 text-sm font-bold leading-tight text-foreground group-hover:text-primary transition-colors duration-150">{item.title}</h3>
        <p className="bounded-text mt-1.5 line-clamp-2 text-xs leading-relaxed text-muted-foreground font-medium">{item.reason}</p>
        {item.failure_summary ? (
          <div className="mt-2.5 flex min-w-0 items-start gap-1.5 overflow-hidden rounded-md border border-destructive/20 bg-destructive/10 px-2 py-1.5 text-xs font-semibold text-destructive">
            <AlertTriangle className="size-3.5 shrink-0 mt-0.5" />
            <span className="bounded-text line-clamp-2 min-w-0 leading-normal">{item.failure_summary.category}</span>
          </div>
        ) : null}
      </button>
      <div className="mt-3 flex min-w-0 flex-wrap gap-1.5 border-t border-border/50 pt-2.5">
        <span className="max-w-full truncate rounded-md border border-border bg-background/50 px-2 py-0.5 text-[10px] font-bold text-muted-foreground">
          {item.board_state === "Ready" ? "Start" : item.board_state === "Blocked" ? "Start disabled" : item.lane}
        </span>
        <span className="max-w-full truncate rounded-md border border-border bg-background/50 px-2 py-0.5 text-[10px] font-bold text-muted-foreground">
          {item.blockers.length > 0 ? `${item.blockers.length} blockers` : item.run_id ?? "No run"}
        </span>
      </div>
      {item.board_state === "Ready" ? (
        <Button
          type="button"
          className="mt-3 h-9 w-full justify-start overflow-hidden px-2.5 text-left text-xs cursor-pointer"
          disabled={runDisabled}
          aria-label="Run with Codex"
          title={canRun ? "Start this Ready story with Codex" : "Cannot start while another run is active or proof is missing"}
          onClick={() => void onRun(item)}
        >
          <Play className="shrink-0 size-3" />
          <span className="min-w-0 truncate">{startingId === item.id ? "Starting..." : "Run with Codex"}</span>
        </Button>
      ) : null}
    </div>
  );
}

const columnChrome: Record<BoardState, string> = {
  Ready: "border-border border-t-2 border-t-emerald-500/80 dark:border-t-emerald-400/80",
  Blocked: "border-border border-t-2 border-t-zinc-400/70 dark:border-t-zinc-600/70",
  "In Progress": "border-border border-t-2 border-t-blue-500/80 dark:border-t-blue-400/80",
  Review: "border-border border-t-2 border-t-violet-500/80 dark:border-t-violet-400/80",
  "Needs Attention": "border-border border-t-2 border-t-red-500/80 dark:border-t-red-400/80",
  Done: "border-border border-t-2 border-t-teal-500/80 dark:border-t-teal-400/80"
};

const columnIcon: Record<BoardState, string> = {
  Ready: "column-icon-ready",
  Blocked: "column-icon-blocked",
  "In Progress": "column-icon-progress",
  Review: "column-icon-review",
  "Needs Attention": "column-icon-attention",
  Done: "column-icon-done"
};

const stateDot: Record<BoardState, string> = {
  Ready: "bg-emerald-555 bg-emerald-500 shadow-sm shadow-emerald-500/50",
  Blocked: "bg-zinc-500 shadow-sm shadow-zinc-500/50",
  "In Progress": "bg-blue-500 shadow-sm shadow-blue-500/50",
  Review: "bg-violet-500 shadow-sm shadow-violet-500/50",
  "Needs Attention": "bg-red-500 shadow-sm shadow-red-500/50",
  Done: "bg-teal-500 shadow-sm shadow-teal-500/50"
};
