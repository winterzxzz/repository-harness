import { AlertTriangle, Check, Circle, Loader2 } from "lucide-react";
import { Card } from "../../components/ui/card";
import { Button } from "../../components/ui/button";
import { cn } from "../../lib/utils";
import { deriveForkedTaskFlow, type ForkLane, type ForkStep } from "./task-flow-model";
import type { RecoveryAction, TaskFlow, TaskFlowStepId, TaskFlowStepState } from "./types";

const labels: Record<TaskFlowStepId, string> = {
  start: "Start",
  agent: "Agent",
  validation: "Validation",
  pr: "Pull request",
  review: "Review",
  sync: "Sync",
  done: "Done"
};

const idleFlow: TaskFlow = {
  story_id: "",
  title: "",
  state: "active",
  current_step: null,
  message: "Symphony is idle. The next task will appear here.",
  pr_status: "missing",
  steps: ["start", "agent", "validation", "pr", "review", "sync", "done"].map((id) => ({
    id: id as TaskFlowStepId,
    state: "pending" as const
  })),
  recovery_action: null
};

export function ActiveTaskFlow({ flow, stale = false, onRecover }: { flow: TaskFlow | null; stale?: boolean; onRecover?: (storyId: string, action: RecoveryAction) => void }) {
  const fork = deriveForkedTaskFlow(flow ?? idleFlow);
  return (
    <Card className="overflow-hidden rounded-xl bg-card p-3 lg:p-4">
      <section aria-label="Active task lifecycle">
        <header className="flex min-w-0 flex-wrap items-center justify-between gap-x-4 gap-y-1">
          <div className="min-w-0">
            <p className="text-xs font-semibold text-muted-foreground">Task lifecycle</p>
            <p className="truncate text-sm font-bold text-foreground">
              {flow ? `${flow.story_id} · ${flow.title}` : "No task is currently running"}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <p className="max-w-[65ch] text-xs font-medium text-muted-foreground">
              {flow?.message ?? idleFlow.message}
            </p>
            {flow?.recovery_action && onRecover ? (
              <Button variant="outline" className="h-8 px-3 text-xs" onClick={() => onRecover(flow.story_id, flow.recovery_action!)}>
                {flow.recovery_action.label}
              </Button>
            ) : null}
          </div>
        </header>
        <div className="scrollbar-none mt-3 overflow-x-auto pb-1">
          <div className="task-flow-fork min-w-[880px]" aria-label="Task lifecycle steps">
            <FlowSegment name="Shared start" steps={fork.head} connectTrailingEdge />
            <div className="task-flow-lanes">
              <FlowLane name="Pull request" lane={fork.prLane} />
              <FlowLane name="Local review" lane={fork.localLane} />
            </div>
            <FlowSegment name="Shared finish" steps={fork.tail} connectLeadingEdge />
          </div>
        </div>
        {stale ? <p role="status" className="mt-1 text-xs font-medium text-amber-700">Unable to refresh; showing the last known task state.</p> : null}
      </section>
    </Card>
  );
}

function FlowSegment({
  name,
  steps,
  connectLeadingEdge = false,
  connectTrailingEdge = false
}: {
  name: string;
  steps: ForkStep[];
  connectLeadingEdge?: boolean;
  connectTrailingEdge?: boolean;
}) {
  return (
    <ol className="task-flow-segment" aria-label={name}>
      {steps.map((step, index) => (
        <FlowStep
          key={step.id}
          step={step}
          label={labels[step.id]}
          connectLeft={index > 0 || connectLeadingEdge}
          connectRight={index < steps.length - 1 || connectTrailingEdge}
        />
      ))}
    </ol>
  );
}

function FlowLane({ name, lane }: { name: "Pull request" | "Local review"; lane: ForkLane }) {
  const statusLabel = lane.status === "not-taken" ? "not taken" : lane.status;
  return (
    <div className={cn("task-flow-lane", lane.status === "not-taken" && "task-flow-lane--not-taken")}>
      <div className="task-flow-lane__heading" aria-hidden="true">
        <span>{name} lane</span>
        <span>{statusLabel}</span>
      </div>
      <ol aria-label={`${name} lane, ${statusLabel}`} data-lane-status={lane.status}>
        {lane.steps.map((step) => (
          <FlowStep
            key={step.id}
            step={step}
            label={name === "Pull request" && step.id === "review" ? "Review & merge" : labels[step.id]}
            connectLeft
            connectRight
          />
        ))}
      </ol>
    </div>
  );
}

function FlowStep({
  step,
  label,
  connectLeft,
  connectRight
}: {
  step: ForkStep;
  label: string;
  connectLeft: boolean;
  connectRight: boolean;
}) {
  const state = step.state;
  const Icon = state === "complete" ? Check : state === "failed" ? AlertTriangle : state === "current" ? Loader2 : Circle;
  const connectorTone = state === "complete" ? "bg-emerald-500" : "bg-border";
  return (
    <li
      aria-current={state === "current" ? "step" : undefined}
      className="relative flex min-w-0 flex-1 flex-col items-center px-1 text-center"
    >
      {connectLeft ? <span aria-hidden="true" className={cn("absolute left-0 right-[calc(50%+14px)] top-3 h-0.5", connectorTone)} /> : null}
      {connectRight ? <span aria-hidden="true" className={cn("absolute left-[calc(50%+14px)] right-0 top-3 h-0.5", connectorTone)} /> : null}
      {state === null ? (
        <span aria-hidden="true" className="relative z-10 mt-[9px] size-1.5 rounded-full bg-muted-foreground/50" />
      ) : (
        <span className={cn(
          "relative z-10 grid size-6 place-items-center rounded-full border bg-card",
          state === "complete" && "border-emerald-600 bg-emerald-600 text-white",
          state === "current" && "border-blue-600 text-blue-700 motion-safe:animate-pulse",
          state === "failed" && "border-destructive bg-destructive text-destructive-foreground",
          state === "pending" && "border-border text-muted-foreground"
        )}>
          <Icon className={cn("size-3.5", state === "current" && "motion-safe:animate-spin")} />
        </span>
      )}
      <span className={cn(
        "mt-1.5 text-[11px] font-semibold",
        state === "current" ? "text-blue-700" : state === "failed" ? "text-destructive" : state === "complete" ? "text-emerald-700" : "text-muted-foreground"
      )}>{label}</span>
      <span className="sr-only">{state ?? "not taken"}</span>
    </li>
  );
}
