import { ArrowRight, GitBranch } from "lucide-react";
import type * as React from "react";
import { cn } from "../../lib/utils";
import { bucketId } from "./constants";
import { StatusBadge } from "./status-badge";
import type { BoardBucket, BoardItem } from "./types";

export function ControllerSidebar({
  counts,
  items,
  selectedId,
  onSelect
}: {
  counts: Record<BoardBucket, number>;
  items: BoardItem[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const blockedItems = items.filter((item) => item.board_state === "Blocked");

  return (
    <aside
      aria-label="Workspace navigation"
      className="flex min-h-0 flex-col rounded-xl border border-border/80 bg-card/60 backdrop-blur-md p-2 lg:p-4 shadow-sm lg:sticky lg:top-4 lg:min-h-[calc(100vh-40px)]"
    >
      <div className="mb-1.5 lg:mb-4 rounded-xl border border-border/60 bg-muted/20 p-2 lg:p-3.5 shadow-inner">
        <div className="flex items-center gap-2.5 text-sm font-semibold">
          <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-border bg-background font-mono text-xs font-bold text-primary shadow-sm animate-pulse">
            S
          </span>
          <div className="flex flex-col leading-none">
            <span className="text-sm font-bold tracking-tight text-foreground">Symphony</span>
            <span className="mt-1 text-[10px] text-muted-foreground font-normal">Local run control</span>
          </div>
        </div>
      </div>

      <nav aria-label="Primary" className="scrollbar-none flex gap-1 overflow-x-auto border-t border-border/50 py-1.5 lg:py-3 lg:flex-col lg:overflow-visible">
        <SidebarLabel>Workspace</SidebarLabel>
        <SidebarItem active href="#board" label="Work board" count={String(Object.values(counts).reduce((sum, count) => sum + count, 0))} />
        <details className="min-w-56 rounded-md lg:min-w-0 group">
          <summary className="flex min-h-[34px] cursor-pointer list-none items-center justify-between rounded-lg px-2.5 text-xs font-semibold text-muted-foreground transition-all duration-150 hover:bg-muted/60 hover:text-foreground focus-visible:outline-none">
            <span className="flex items-center gap-1.5">
              <span className="text-[9px] text-muted-foreground/50 transition-transform duration-150 group-open:rotate-90">▶</span>
              <span>Dependencies</span>
            </span>
            <span className="font-mono text-[10px] text-muted-foreground bg-muted/40 border border-border/30 rounded px-1.5 py-0.5">{blockedItems.length}</span>
          </summary>
          <div className="grid gap-2 px-3.5 pb-2 pt-1.5 border-l border-border/30 ml-4 mt-0.5">
            {blockedItems.length > 0 ? (
              blockedItems.slice(0, 4).map((item) => (
                <div key={item.id} className="flex flex-col gap-0.5 text-[11px] text-muted-foreground border-b border-border/10 last:border-0 pb-1.5 last:pb-0">
                  <span className="font-mono font-bold text-foreground/80">{item.id}</span>
                  <span className="truncate text-muted-foreground/70 leading-tight">{item.reason}</span>
                </div>
              ))
            ) : (
              <span className="text-[11px] text-muted-foreground/50 italic">No blocked work</span>
            )}
          </div>
        </details>
        <SidebarItem href="#logs" label="Run logs" count="live" />
      </nav>

      <nav aria-label="Status" className="scrollbar-none mt-0.5 lg:mt-1 flex gap-1 overflow-x-auto border-t border-border/50 py-1.5 lg:py-3 lg:flex-col lg:overflow-visible">
        <SidebarLabel>Status</SidebarLabel>
        <SidebarItem href={`#${bucketId("Drafts")}`} label="Drafts" count={String(counts.Drafts)} />
        <SidebarItem href={`#${bucketId("Active")}`} label="Active" count={String(counts.Active)} />
        <SidebarItem href={`#${bucketId("Ready")}`} label="Ready" count={String(counts.Ready)} />
        <SidebarItem href={`#${bucketId("Done")}`} label="Done" count={String(counts.Done)} />
      </nav>

      <SidebarDependencyGraph items={items} selectedId={selectedId} onSelect={onSelect} />
    </aside>
  );
}

function SidebarDependencyGraph({
  items,
  selectedId,
  onSelect
}: {
  items: BoardItem[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const graphItems = items.filter((item) => item.blockers.length > 0 || item.unblocks.length > 0);
  const edgeCount = graphItems.reduce((sum, item) => sum + item.blockers.length, 0);

  return (
    <section className="mt-2 hidden border-t border-border/50 pt-4 lg:block" aria-label="Dependency graph sidebar">
      <div className="flex items-center justify-between gap-2 px-2.5 mb-2.5">
        <div className="flex items-center gap-2">
          <GitBranch className="size-3.5 text-primary" />
          <h2 className="text-[10px] font-bold tracking-wider text-muted-foreground/80 uppercase">Dependency Graph</h2>
        </div>
        <span className="font-mono text-[10px] text-muted-foreground bg-muted/40 border border-border/30 rounded px-1.5 py-0.5">{edgeCount}</span>
      </div>
      <div className="mt-1 grid max-h-[34vh] gap-2 overflow-auto pr-1 scrollbar-thin" aria-label="Dependency edges">
        {graphItems.length > 0 ? (
          graphItems.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelect(item.id)}
              className={cn(
                "w-full rounded-lg border border-border/60 bg-muted/10 p-2.5 text-left transition-all duration-150 hover:border-primary/40 hover:bg-muted/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer hover:shadow-sm",
                item.id === selectedId && "border-primary bg-primary/[0.03] ring-1 ring-primary/35 shadow-sm"
              )}
            >
              <div className="flex items-center justify-between gap-2">
                <strong className="font-mono text-[11px] font-bold text-foreground/90">{item.id}</strong>
                <StatusBadge state={item.board_state} />
              </div>
              <p className="mt-1.5 line-clamp-2 text-xs font-semibold leading-tight text-foreground/85">{item.title}</p>
              <div className="mt-2.5 grid gap-1.5 text-[10px] leading-5 text-muted-foreground">
                {item.blockers.length > 0 ? <GraphLine left={item.blockers.join(", ")} right={item.id} /> : null}
                {item.unblocks.length > 0 ? <GraphLine left={item.id} right={item.unblocks.join(", ")} /> : null}
              </div>
            </button>
          ))
        ) : (
          <div className="rounded-lg border border-dashed border-border/80 bg-background/50 p-3.5 text-xs leading-5 text-muted-foreground/60 text-center font-medium">
            No dependency edges on the current board.
          </div>
        )}
      </div>
    </section>
  );
}

function GraphLine({ left, right }: { left: string; right: string }) {
  return (
    <div className="flex items-center gap-1.5 text-[10px] font-mono text-muted-foreground/75 bg-muted/40 border border-border/20 px-2 py-0.5 rounded">
      <span className="truncate font-semibold">{left}</span>
      <span className="text-muted-foreground/45 font-sans">→</span>
      <span className="truncate font-semibold">{right}</span>
    </div>
  );
}

function SidebarLabel({ children }: { children: React.ReactNode }) {
  return <p className="hidden px-2.5 py-1.5 text-[10px] font-bold tracking-wider text-muted-foreground/80 uppercase lg:block">{children}</p>;
}

function SidebarItem({
  label,
  count,
  href,
  active = false
}: {
  label: string;
  count: string;
  href: string;
  active?: boolean;
}) {
  return (
    <a
      href={href}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex min-h-[34px] min-w-max items-center justify-between gap-3 rounded-lg px-2.5 text-xs font-semibold text-muted-foreground transition-all duration-150 hover:bg-muted/60 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring lg:min-w-0 cursor-pointer",
        active && "bg-muted/80 text-foreground shadow-sm border border-border/40 font-bold border-l-2 border-l-primary"
      )}
    >
      <span>{label}</span>
      <span className={cn(
        "font-mono text-[10px] text-muted-foreground bg-muted/40 border border-border/30 rounded px-1.5 py-0.5",
        active && "bg-background/80 border-border/50 text-foreground"
      )}>{count}</span>
    </a>
  );
}
