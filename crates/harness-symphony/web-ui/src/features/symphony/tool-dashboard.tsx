import React from "react";
import { AlertTriangle, Loader2, RefreshCw, Wrench } from "lucide-react";
import { Badge, type BadgeTone } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { fetchTools, postCheckTools } from "./api";
import type { ToolItem } from "./types";

type ToolState =
  | { status: "loading" }
  | { status: "ready"; tools: ToolItem[] }
  | { status: "error"; message: string };

export function ToolDashboard() {
  const [state, setState] = React.useState<ToolState>({ status: "loading" });
  const [checking, setChecking] = React.useState(false);

  const loadTools = React.useCallback(async (signal?: AbortSignal) => {
    setState({ status: "loading" });
    try {
      const result = await fetchTools({ signal });
      setState({ status: "ready", tools: result.tools });
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === "AbortError") {
        return;
      }
      setState({ status: "error", message: cause instanceof Error ? cause.message : "Tool request failed" });
    }
  }, []);

  React.useEffect(() => {
    const controller = new AbortController();
    void loadTools(controller.signal);
    return () => controller.abort();
  }, [loadTools]);

  async function checkTools() {
    setChecking(true);
    try {
      await postCheckTools();
      await loadTools();
    } catch (cause) {
      setState({ status: "error", message: cause instanceof Error ? cause.message : "Tool check failed" });
    } finally {
      setChecking(false);
    }
  }

  return (
    <section className="grid gap-4" aria-label="Tool Status workspace">
      <Card className="rounded-xl border border-border bg-card p-5 shadow-sm">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Wrench className="size-4 text-primary" />
              <h2 className="text-xl font-bold tracking-tight text-foreground">Tool Status</h2>
            </div>
            <p className="mt-1 text-sm leading-6 text-muted-foreground">
              Registered optional capabilities and their latest presence scan.
            </p>
          </div>
          <Button type="button" onClick={() => void checkTools()} disabled={checking || state.status === "loading"}>
            {checking ? <Loader2 data-icon="inline-start" className="motion-safe:animate-spin" /> : <RefreshCw data-icon="inline-start" />}
            Check tools
          </Button>
        </div>
      </Card>

      <section className="grid gap-3" aria-label="Tool registry">
        {state.status === "loading" ? (
          <Card className="rounded-xl border border-border bg-card p-5 text-sm text-muted-foreground">
            <Loader2 className="mr-2 inline size-4 motion-safe:animate-spin" />
            Loading tools.
          </Card>
        ) : state.status === "error" ? (
          <Card role="alert" className="rounded-xl border border-destructive/35 bg-destructive/5 p-5 text-sm text-destructive">
            <AlertTriangle className="mr-2 inline size-4" />
            {state.message}
          </Card>
        ) : state.tools.length > 0 ? (
          state.tools.map((tool) => <ToolCard key={`${tool.source}-${tool.name}`} tool={tool} />)
        ) : (
          <Card className="rounded-xl border border-dashed border-border bg-card p-5 text-sm text-muted-foreground">
            No registered tools.
          </Card>
        )}
      </section>
    </section>
  );
}

function ToolCard({ tool }: { tool: ToolItem }) {
  return (
    <Card className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={statusTone(tool.status)}>{tool.status}</Badge>
            <span className="font-mono text-xs font-bold text-muted-foreground">{tool.kind}</span>
            {tool.capability ? <span className="font-mono text-xs font-bold text-primary">{tool.capability}</span> : null}
          </div>
          <h3 className="bounded-text mt-2 text-base font-bold text-foreground">{tool.name}</h3>
          <p className="bounded-text mt-1 text-sm leading-6 text-muted-foreground">{tool.description}</p>
        </div>
        <Badge tone="neutral">{tool.source}</Badge>
      </div>
      <div className="mt-3 grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
        <span className="bounded-text">Responsibility: {tool.responsibility || "unassigned"}</span>
        <span className="bounded-text">Checked: {tool.checked_at ?? "never"}</span>
        <span className="bounded-text sm:col-span-2">Command: {tool.command || "none"}</span>
      </div>
    </Card>
  );
}

function statusTone(status: string): BadgeTone {
  if (status === "present") {
    return "success";
  }
  if (status === "missing") {
    return "danger";
  }
  return "neutral";
}
