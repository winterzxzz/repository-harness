import * as React from "react";
import { cn } from "../../lib/utils";

export type BadgeTone = "neutral" | "muted" | "success" | "info" | "accent" | "danger" | "complete";

const tones: Record<BadgeTone, string> = {
  neutral: "border-border/80 bg-muted/50 text-muted-foreground",
  muted: "border-zinc-500/20 bg-zinc-500/10 text-zinc-600 dark:text-zinc-450 dark:text-zinc-400",
  success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
  info: "border-blue-500/25 bg-blue-500/10 text-blue-700 dark:text-blue-400",
  accent: "border-violet-500/25 bg-violet-500/10 text-violet-700 dark:text-violet-400",
  danger: "border-red-500/25 bg-red-500/10 text-red-700 dark:text-red-450 dark:text-red-400",
  complete: "border-teal-500/25 bg-teal-500/10 text-teal-700 dark:text-teal-400"
};

export function Badge({
  className,
  tone = "neutral",
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & { tone?: BadgeTone }) {
  return (
    <span
      className={cn(
        "inline-flex min-h-6 items-center rounded-md border px-2 py-0.5 text-xs font-medium",
        tones[tone],
        className
      )}
      {...props}
    />
  );
}
