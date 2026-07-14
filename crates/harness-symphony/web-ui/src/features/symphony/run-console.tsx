import React from "react";
import { ArrowDown, CheckCircle2, CircleAlert, Terminal } from "lucide-react";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { agentLabel } from "./constants";
import {
  buildConsoleTranscript,
  type ConsoleBlock,
  type ConsoleCommand
} from "./run-console-model";
import type { RunEvent } from "./types";

export function RunConsole({
  events,
  live = false,
  agent
}: {
  events: RunEvent[];
  live?: boolean;
  agent?: string;
}) {
  const scrollRef = React.useRef<HTMLDivElement>(null);
  const [following, setFollowing] = React.useState(true);
  const transcript = React.useMemo(
    () => buildConsoleTranscript(events, { agentName: agentLabel(agent ?? "codex") }),
    [agent, events]
  );

  const scrollToTail = React.useCallback((behavior: ScrollBehavior = "smooth") => {
    const scrollback = scrollRef.current;
    if (!scrollback) {
      return;
    }
    scrollback.scrollTo({ top: scrollback.scrollHeight, behavior });
  }, []);

  React.useEffect(() => {
    if (!live || !following) {
      return;
    }
    const frame = window.requestAnimationFrame(() => scrollToTail("auto"));
    return () => window.cancelAnimationFrame(frame);
  }, [following, live, scrollToTail, transcript]);

  function resumeFollowing() {
    setFollowing(true);
    scrollToTail();
  }

  return (
    <section className="flex flex-col gap-3 border-b border-border p-4" role="region" aria-label="Run console">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Terminal aria-hidden="true" className="size-4 text-primary" />
          <h3 className="text-lg font-bold tracking-tight text-foreground">Run console</h3>
          <Badge tone={live ? "info" : "neutral"}>{live ? "Live" : "Recorded"}</Badge>
        </div>
        {live && !following ? (
          <Button type="button" variant="outline" onClick={resumeFollowing} aria-label="Jump to latest output">
            <ArrowDown aria-hidden="true" data-icon="inline-start" />
            Latest output
          </Button>
        ) : null}
      </div>
      <p className="text-xs leading-5 text-muted-foreground">
        Read-only command output and agent milestones. Scroll up to pause live following.
      </p>
      <div
        ref={scrollRef}
        data-testid="run-console-scrollback"
        className="max-h-[28rem] overflow-y-auto rounded-md bg-slate-950 text-slate-100 shadow-inner"
        onScroll={(event) => {
          const element = event.currentTarget;
          const atTail = element.scrollHeight - element.scrollTop - element.clientHeight <= 24;
          setFollowing(atTail);
        }}
      >
        {transcript.length > 0 ? (
          <div className="divide-y divide-slate-800/90">
            {transcript.map((block) => <ConsoleRow key={`${block.kind}-${block.id}`} block={block} />)}
          </div>
        ) : (
          <div className="flex min-h-28 items-center justify-center px-4 py-8 text-sm text-slate-400">
            Waiting for command output and agent milestones…
          </div>
        )}
      </div>
      <span className="sr-only" aria-live={live ? "polite" : undefined}>
        {live && transcript.length > 0 ? `Run console updated. ${transcript.length} transcript entries.` : ""}
      </span>
    </section>
  );
}

function ConsoleRow({ block }: { block: ConsoleBlock }) {
  if (block.kind === "command") {
    return <CommandRow command={block} />;
  }
  if (block.kind === "message") {
    return (
      <article className="grid gap-2 bg-slate-900/75 px-4 py-3 text-sm">
        <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-slate-400">
          <span className="font-semibold text-sky-300">{block.source}</span>
          {block.timestamp ? <time>{block.timestamp}</time> : null}
        </div>
        <p className="whitespace-pre-wrap break-words font-mono leading-6 text-slate-100">{block.text}</p>
      </article>
    );
  }
  return (
    <div className="flex flex-wrap items-start gap-x-3 gap-y-1 px-4 py-3 text-sm">
      <CheckCircle2 aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-emerald-400" />
      <span className="font-semibold text-slate-300">{block.label}</span>
      <span className="min-w-0 flex-1 break-words text-slate-200">{block.text}</span>
      {block.timestamp ? <time className="text-xs text-slate-500">{block.timestamp}</time> : null}
    </div>
  );
}

function CommandRow({ command }: { command: ConsoleCommand }) {
  return (
    <article className={command.failed ? "bg-red-950/20" : "bg-slate-950"}>
      <div className="flex flex-wrap items-start justify-between gap-3 bg-slate-900 px-4 py-2.5 font-mono text-sm">
        <div className="flex min-w-0 items-start gap-2">
          <span aria-hidden="true" className="shrink-0 select-none text-emerald-400">$</span>
          <code className="break-all text-slate-100">{command.command}</code>
        </div>
        {command.exitCode !== null ? (
          <span className={command.failed ? "flex items-center gap-1.5 text-xs font-semibold text-red-300" : "flex items-center gap-1.5 text-xs font-semibold text-emerald-300"}>
            {command.failed ? <CircleAlert aria-hidden="true" className="size-3.5" /> : <CheckCircle2 aria-hidden="true" className="size-3.5" />}
            Exit {command.exitCode}
          </span>
        ) : (
          <span className="text-xs text-slate-500">Running</span>
        )}
      </div>
      {command.output ? (
        <pre className="m-0 whitespace-pre-wrap break-words px-4 py-3 font-mono text-xs leading-5 text-slate-300">{command.output}</pre>
      ) : (
        <p className="m-0 px-4 py-3 text-xs text-slate-500">No output yet.</p>
      )}
    </article>
  );
}
