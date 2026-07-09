import { AlertTriangle, CheckCircle2, Circle, GitPullRequestArrow, Loader2, ShieldAlert } from "lucide-react";
import type { AgentId, BoardState } from "./types";

export const agents: { id: AgentId; label: string }[] = [
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" }
];

export function agentLabel(agent: string): string {
  return agents.find((entry) => entry.id === agent)?.label ?? agent;
}

export const states: BoardState[] = ["Ready", "Blocked", "In Progress", "Review", "Needs Attention", "Done"];

export const stateIcon = {
  Ready: Circle,
  Blocked: ShieldAlert,
  "In Progress": Loader2,
  Review: GitPullRequestArrow,
  "Needs Attention": AlertTriangle,
  Done: CheckCircle2
};

export function columnId(state: BoardState): string {
  return `column-${state.toLowerCase().replace(/\s+/g, "-")}`;
}
