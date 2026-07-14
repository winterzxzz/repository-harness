import { CheckCircle2, Circle, GitPullRequestArrow, Loader2, type LucideIcon } from "lucide-react";
import type { AgentId, BoardBucket, BoardItem } from "./types";

export const agents: { id: AgentId; label: string }[] = [
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" }
];

export function agentLabel(agent: string): string {
  const known = agents.find((entry) => entry.id === agent)?.label;
  if (known) return known;
  return agent
    .split(/[-_]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export const buckets: BoardBucket[] = ["Drafts", "Active", "Ready", "Done"];

export type BucketPresentation = {
  label: string;
  description: string;
  icon: LucideIcon;
};

export const bucketPresentation: Record<BoardBucket, BucketPresentation> = {
  Drafts: {
    label: "Planned",
    description: "Ready to start · blocked work stays visible",
    icon: Circle
  },
  Active: {
    label: "Agent working",
    description: "Codex owns the next action",
    icon: Loader2
  },
  Ready: {
    label: "Human review",
    description: "Waiting for your decision",
    icon: GitPullRequestArrow
  },
  Done: {
    label: "Done",
    description: "Accepted and synchronized",
    icon: CheckCircle2
  }
};

export function bucketLabel(bucket: BoardBucket): string {
  return bucketPresentation[bucket].label;
}

export function bucketId(bucket: BoardBucket): string {
  return `bucket-${bucket.toLowerCase().replace(/\s+/g, "-")}`;
}

export function bucketForItem(item: Pick<BoardItem, "board_state">): BoardBucket {
  if (item.board_state === "Ready" || item.board_state === "Blocked") {
    return "Drafts";
  }
  if (item.board_state === "Review") {
    return "Ready";
  }
  if (item.board_state === "Done") {
    return "Done";
  }
  return "Active";
}
