import React from "react";
import { AlertTriangle, Filter, Loader2, RefreshCw } from "lucide-react";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Input } from "../../components/ui/input";
import { fetchTraces } from "./api";
import type { TraceItem } from "./types";

type TraceState =
  | { status: "loading" }
  | { status: "ready"; traces: TraceItem[]; total: number }
  | { status: "error"; message: string };

export function TraceExplorer() {
  const [storyFilter, setStoryFilter] = React.useState("");
  const [outcomeFilter, setOutcomeFilter] = React.useState("");
  const [state, setState] = React.useState<TraceState>({ status: "loading" });

  const loadTraces = React.useCallback(
    async (signal?: AbortSignal) => {
      setState({ status: "loading" });
      try {
        const result = await fetchTraces(
          {
            storyId: storyFilter.trim() || undefined,
            outcome: outcomeFilter || undefined
          },
          { signal }
        );
        setState({ status: "ready", traces: result.traces, total: result.total });
      } catch (cause) {
        if (cause instanceof DOMException && cause.name === "AbortError") {
          return;
        }
        setState({ status: "error", message: cause instanceof Error ? cause.message : "Trace request failed" });
      }
    },
    [outcomeFilter, storyFilter]
  );

  React.useEffect(() => {
    const controller = new AbortController();
    void loadTraces(controller.signal);
    return () => controller.abort();
  }, [loadTraces]);

  return (
    <section className="grid gap-4" aria-label="Trace Explorer workspace">
      <Card className="rounded-xl border border-border bg-card p-5 shadow-sm">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="min-w-0">
            <h2 className="text-xl font-bold tracking-tight text-foreground">Trace Explorer</h2>
            <p className="mt-1 text-sm leading-6 text-muted-foreground">
              Historical Harness traces, filtered by story and outcome.
            </p>
          </div>
          <div className="grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-[minmax(0,12rem)_minmax(0,10rem)_auto]">
            <label className="min-w-0">
              <span className="sr-only">Trace story filter</span>
              <Input
                aria-label="Trace story filter"
                value={storyFilter}
                onChange={(event) => setStoryFilter(event.target.value)}
                placeholder="Story ID"
              />
            </label>
            <label className="min-w-0">
              <span className="sr-only">Trace outcome filter</span>
              <select
                aria-label="Trace outcome filter"
                className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm font-medium"
                value={outcomeFilter}
                onChange={(event) => setOutcomeFilter(event.target.value)}
              >
                <option value="">Any outcome</option>
                <option value="completed">completed</option>
                <option value="partial">partial</option>
                <option value="blocked">blocked</option>
                <option value="failed">failed</option>
              </select>
            </label>
            <Button type="button" onClick={() => void loadTraces()} disabled={state.status === "loading"}>
              {state.status === "loading" ? <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" /> : <Filter data-icon="inline-start" />}
              Apply trace filter
            </Button>
          </div>
        </div>
      </Card>

      <section className="grid gap-3" aria-label="Trace results">
        {state.status === "loading" ? (
          <Card className="rounded-xl border border-border bg-card p-5 text-sm text-muted-foreground">
            <Loader2 className="mr-2 inline size-4 motion-safe:animate-spin" />
            Loading traces.
          </Card>
        ) : state.status === "error" ? (
          <Card role="alert" className="rounded-xl border border-destructive/35 bg-destructive/5 p-5 text-sm text-destructive">
            <AlertTriangle className="mr-2 inline size-4" />
            {state.message}
          </Card>
        ) : state.traces.length > 0 ? (
          state.traces.map((trace) => <TraceCard key={trace.id} trace={trace} />)
        ) : (
          <Card className="rounded-xl border border-dashed border-border bg-card p-5 text-sm text-muted-foreground">
            No traces match this filter.
          </Card>
        )}
      </section>

      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <RefreshCw className="size-3.5" />
        {state.status === "ready" ? `${state.total} trace records loaded.` : "Trace records load from harness.db."}
      </div>
    </section>
  );
}

function TraceCard({ trace }: { trace: TraceItem }) {
  return (
    <Card className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={trace.outcome === "completed" ? "success" : trace.outcome === "failed" ? "danger" : "neutral"}>
              {trace.outcome}
            </Badge>
            <span className="font-mono text-xs font-bold text-muted-foreground">#{trace.id}</span>
            {trace.story_id ? <span className="font-mono text-xs font-bold text-primary">{trace.story_id}</span> : null}
          </div>
          <h3 className="bounded-text mt-2 text-base font-bold text-foreground">{trace.summary}</h3>
        </div>
        <span className="text-xs font-medium text-muted-foreground">{trace.created_at}</span>
      </div>
      <div className="mt-3 grid gap-2 text-sm text-muted-foreground sm:grid-cols-2">
        <span>Duration: {trace.duration_seconds === null ? "unknown" : `${trace.duration_seconds}s`}</span>
        <span>Friction: {trace.harness_friction ?? "none"}</span>
      </div>
    </Card>
  );
}
