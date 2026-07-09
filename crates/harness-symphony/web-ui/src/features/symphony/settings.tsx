import React from "react";
import { Bot, Check } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { cn } from "../../lib/utils";
import { agents } from "./constants";
import type { AgentId } from "./types";

export function SettingsPanel({
  defaultAgent,
  saving,
  error,
  onSave
}: {
  defaultAgent: AgentId;
  saving: boolean;
  error: string | null;
  onSave: (agent: AgentId) => Promise<void>;
}) {
  const [selected, setSelected] = React.useState<AgentId>(defaultAgent);

  React.useEffect(() => {
    setSelected(defaultAgent);
  }, [defaultAgent]);

  return (
    <Card className="max-w-2xl rounded-xl border border-border bg-card p-5 shadow-sm">
      <div className="flex items-start gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-lg border border-border bg-muted/40">
          <Bot className="size-4.5" />
        </span>
        <div className="min-w-0">
          <h2 className="text-base font-bold tracking-tight text-foreground">Run agent</h2>
          <p className="mt-0.5 text-sm font-medium leading-relaxed text-muted-foreground">
            The default agent that runs a Ready story. Picking an agent from a card&apos;s run
            menu also updates this default.
          </p>
        </div>
      </div>

      <fieldset className="mt-4 flex flex-col gap-2" aria-label="Default run agent">
        {agents.map((agent) => (
          <label
            key={agent.id}
            className={cn(
              "flex cursor-pointer items-center gap-3 rounded-lg border p-3 transition-colors duration-150",
              selected === agent.id
                ? "border-primary bg-primary/5 ring-1 ring-ring/25"
                : "border-border bg-background/40 hover:bg-muted/30"
            )}
          >
            <input
              type="radio"
              name="default-agent"
              value={agent.id}
              checked={selected === agent.id}
              onChange={() => setSelected(agent.id)}
              className="sr-only"
            />
            <span
              className={cn(
                "grid size-5 shrink-0 place-items-center rounded-full border",
                selected === agent.id ? "border-primary bg-primary text-primary-foreground" : "border-border bg-background"
              )}
              aria-hidden="true"
            >
              {selected === agent.id ? <Check className="size-3" /> : null}
            </span>
            <span className="min-w-0">
              <span className="block text-sm font-bold text-foreground">{agent.label}</span>
              <span className="block text-xs font-medium text-muted-foreground">
                {agent.id === "codex" ? "Codex app-server run inside the Symphony worktree." : "OpenCode headless run inside the Symphony worktree."}
              </span>
            </span>
          </label>
        ))}
      </fieldset>

      {error ? (
        <p role="alert" className="mt-3 text-sm font-semibold text-destructive">
          {error}
        </p>
      ) : null}

      <div className="mt-4 flex items-center gap-3">
        <Button
          type="button"
          className="h-9 px-4 text-xs cursor-pointer"
          disabled={saving || selected === defaultAgent}
          onClick={() => void onSave(selected)}
        >
          {saving ? "Saving..." : "Save default agent"}
        </Button>
        <span className="text-xs font-medium text-muted-foreground">
          Applies to new runs started from the board.
        </span>
      </div>
    </Card>
  );
}
