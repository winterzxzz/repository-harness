import { CheckCircle2, Circle, GitPullRequestArrow, Loader2 } from "lucide-react";
import type { AgentId, BoardBucket, BoardItem } from "./types";

export const agents: { id: AgentId; label: string }[] = [
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" }
];

export function agentLabel(agent: string): string {
  return agents.find((entry) => entry.id === agent)?.label ?? agent;
}

export const buckets: BoardBucket[] = ["Drafts", "Active", "Ready", "Done"];

export const bucketIcon = {
  Drafts: Circle,
  Active: Loader2,
  Ready: GitPullRequestArrow,
  Done: CheckCircle2
};

export function bucketId(bucket: BoardBucket): string {
  return `bucket-${bucket.toLowerCase().replace(/\s+/g, "-")}`;
}

export function bucketForItem(item: Pick<BoardItem, "board_state">): BoardBucket {
  if (item.board_state === "Ready") {
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
