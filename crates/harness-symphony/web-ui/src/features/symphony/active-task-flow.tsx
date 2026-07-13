import { AlertTriangle, Check, Circle, Loader2 } from "lucide-react";
import { Card } from "../../components/ui/card";
import { Button } from "../../components/ui/button";
import { cn } from "../../lib/utils";
import type { RecoveryAction, TaskFlow, TaskFlowStepId, TaskFlowStepState } from "./types";

const stepIds: TaskFlowStepId[] = ["start", "agent", "validation", "pr", "review", "sync", "done"];
const labels: Record<TaskFlowStepId, string> = {
  start: "Start",
  agent: "Agent",
  validation: "Validation",
  pr: "Pull request",
  review: "Review & merge",
  sync: "Sync",
  done: "Done"
};

export function ActiveTaskFlow({ flow, stale = false, onRecover }: { flow: TaskFlow | null; stale?: boolean; onRecover?: (storyId: string, action: RecoveryAction) => void }) {
  const steps = flow?.steps ?? stepIds.map((id) => ({ id, state: "pending" as const }));
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
              {flow?.message ?? "Symphony is idle. The next task will appear here."}
            </p>
            {flow?.recovery_action && onRecover ? (
              <Button variant="outline" className="h-8 px-3 text-xs" onClick={() => onRecover(flow.story_id, flow.recovery_action!)}>
                {flow.recovery_action.label}
              </Button>
            ) : null}
          </div>
        </header>
        <div className="scrollbar-none mt-3 overflow-x-auto pb-1">
          <ol className="flex min-w-[760px] items-start" aria-label="Task lifecycle steps">
            {steps.map((step, index) => (
              <FlowStep key={step.id} id={step.id} state={step.state} last={index === steps.length - 1} />
            ))}
          </ol>
        </div>
        {stale ? <p role="status" className="mt-1 text-xs font-medium text-amber-700">Unable to refresh; showing the last known task state.</p> : null}
      </section>
    </Card>
  );
}

function FlowStep({ id, state, last }: { id: TaskFlowStepId; state: TaskFlowStepState; last: boolean }) {
  const Icon = state === "complete" ? Check : state === "failed" ? AlertTriangle : state === "current" ? Loader2 : Circle;
  return (
    <li
      aria-current={state === "current" ? "step" : undefined}
      className="relative flex min-w-0 flex-1 flex-col items-center px-1 text-center"
    >
      {!last ? <span aria-hidden="true" className={cn("absolute left-[calc(50%+14px)] right-[calc(-50%+14px)] top-3 h-0.5", state === "complete" ? "bg-emerald-500" : "bg-border")} /> : null}
      <span className={cn(
        "relative z-10 grid size-6 place-items-center rounded-full border bg-card",
        state === "complete" && "border-emerald-600 bg-emerald-600 text-white",
        state === "current" && "border-blue-600 text-blue-700 motion-safe:animate-pulse",
        state === "failed" && "border-destructive bg-destructive text-destructive-foreground",
        state === "pending" && "border-border text-muted-foreground"
      )}>
        <Icon className={cn("size-3.5", state === "current" && "motion-safe:animate-spin")} />
      </span>
      <span className={cn("mt-1.5 text-[11px] font-semibold", state === "current" ? "text-blue-700" : state === "failed" ? "text-destructive" : state === "complete" ? "text-emerald-700" : "text-muted-foreground")}>{labels[id]}</span>
      <span className="sr-only">{state}</span>
    </li>
  );
}
