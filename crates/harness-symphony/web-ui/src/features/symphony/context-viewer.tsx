import React from "react";
import { FileText, Loader2 } from "lucide-react";
import { Badge } from "../../components/ui/badge";
import { fetchContext } from "./api";

type ContextState =
  | { status: "loading" }
  | { status: "ready"; storyId: string; content: string }
  | { status: "error"; message: string };

type MarkdownBlock =
  | { kind: "heading"; level: number; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "code"; language: string; code: string };

export function ContextViewer({ storyId, embedded = false }: { storyId: string; embedded?: boolean }) {
  const [state, setState] = React.useState<ContextState>({ status: "loading" });

  React.useEffect(() => {
    const controller = new AbortController();
    setState({ status: "loading" });
    fetchContext(storyId, { signal: controller.signal })
      .then((context) => setState({ status: "ready", storyId: context.story_id, content: context.content }))
      .catch((cause) => {
        if (cause instanceof DOMException && cause.name === "AbortError") {
          return;
        }
        setState({ status: "error", message: cause instanceof Error ? cause.message : "Context request failed" });
      });
    return () => controller.abort();
  }, [storyId]);

  return (
    <section className={embedded ? "" : "border-b border-border p-4"} aria-label="Context pack">
      {embedded ? null : (
        <div className="mb-3 flex min-w-0 items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <FileText className="size-4 shrink-0 text-primary" />
            <h3 className="text-lg font-bold tracking-tight text-foreground">Context pack</h3>
          </div>
          <Badge tone="neutral" className="shrink-0">
            {storyId}
          </Badge>
        </div>
      )}

      {state.status === "loading" ? (
        <div className="flex items-center gap-2 rounded-md border border-border bg-muted p-3 text-sm text-muted-foreground" role="status">
          <Loader2 className="size-4 motion-safe:animate-spin" />
          Loading context.
        </div>
      ) : state.status === "error" ? (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {state.message}
        </div>
      ) : (
        <div className="grid max-h-[32rem] gap-3 overflow-auto rounded-xl border border-border/80 bg-background/40 p-4 shadow-inner">
          <MarkdownPreview content={state.content} />
        </div>
      )}
    </section>
  );
}

function MarkdownPreview({ content }: { content: string }) {
  return (
    <>
      {parseMarkdownBlocks(content).map((block, index) => {
        if (block.kind === "heading") {
          return (
            <p key={`${block.kind}-${index}`} className="bounded-text text-sm font-bold text-foreground">
              {block.text}
            </p>
          );
        }
        if (block.kind === "code") {
          return (
            <div key={`${block.kind}-${index}`} className="overflow-hidden rounded-lg border border-border bg-muted/60">
              <div className="border-b border-border/70 px-3 py-1.5 font-mono text-[11px] font-bold text-muted-foreground">
                {block.language || "text"}
              </div>
              <pre className="max-h-72 overflow-auto p-3 text-xs leading-relaxed text-foreground">
                <code>{block.code}</code>
              </pre>
            </div>
          );
        }
        return (
          <p key={`${block.kind}-${index}`} className="bounded-text text-sm leading-6 text-muted-foreground">
            {block.text}
          </p>
        );
      })}
    </>
  );
}

function parseMarkdownBlocks(content: string): MarkdownBlock[] {
  const blocks: MarkdownBlock[] = [];
  const lines = content.split(/\r?\n/);
  let paragraph: string[] = [];
  let code: string[] | null = null;
  let language = "";

  function flushParagraph() {
    if (paragraph.length === 0) {
      return;
    }
    blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
    paragraph = [];
  }

  for (const line of lines) {
    if (line.startsWith("```")) {
      if (code) {
        blocks.push({ kind: "code", language, code: code.join("\n") });
        code = null;
        language = "";
      } else {
        flushParagraph();
        code = [];
        language = line.slice(3).trim();
      }
      continue;
    }
    if (code) {
      code.push(line);
      continue;
    }
    if (line.trim().length === 0) {
      flushParagraph();
      continue;
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(line);
    if (heading) {
      flushParagraph();
      blocks.push({ kind: "heading", level: heading[1].length, text: heading[2] });
      continue;
    }
    paragraph.push(line.trim().replace(/^[-*]\s+/, "- "));
  }
  flushParagraph();
  if (code) {
    blocks.push({ kind: "code", language, code: code.join("\n") });
  }
  return blocks;
}
