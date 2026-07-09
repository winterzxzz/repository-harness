import * as React from "react";
import { cn } from "../../lib/utils";

export type BadgeTone = "neutral" | "muted" | "success" | "info" | "accent" | "danger" | "complete";

const tones: Record<BadgeTone, string> = {
  neutral: "border-border bg-muted text-muted-foreground",
  muted: "border-zinc-300 bg-zinc-100 text-zinc-700 dark:border-zinc-800 dark:bg-zinc-900/50 dark:text-zinc-400",
  success: "border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-900/50 dark:bg-emerald-950/30 dark:text-emerald-400",
  info: "border-blue-200 bg-blue-50 text-blue-800 dark:border-blue-900/50 dark:bg-blue-950/30 dark:text-blue-400",
  accent: "border-violet-200 bg-violet-50 text-violet-800 dark:border-violet-900/50 dark:bg-violet-950/30 dark:text-violet-400",
  danger: "border-red-200 bg-red-50 text-red-800 dark:border-red-900/50 dark:bg-red-950/30 dark:text-red-400",
  complete: "border-teal-200 bg-teal-50 text-teal-800 dark:border-teal-900/50 dark:bg-teal-950/30 dark:text-teal-400"
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
