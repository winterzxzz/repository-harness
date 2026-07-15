import type { NormalizedRunEvent, RunEvent } from "./types";

export type RunAgentInfo = {
  adapter: string | null;
  model: string | null;
  command: string | null;
};

const STARTED_PREFIX = "agent process started: ";

function isNormalized(event: RunEvent): event is NormalizedRunEvent {
  return (
    typeof event === "object" &&
    event !== null &&
    typeof (event as NormalizedRunEvent).kind === "string" &&
    typeof (event as NormalizedRunEvent).stage === "string" &&
    typeof (event as NormalizedRunEvent).message === "string"
  );
}

// The runner appends "agent process started: <resolved command>" as the first
// lifecycle event of the agent stage; the adapter binary and its -m/--model
// (or codex -c model=...) flag are the per-run source of truth.
export function deriveRunAgentInfo(events: RunEvent[]): RunAgentInfo {
  const started = events.find(
    (event): event is NormalizedRunEvent =>
      isNormalized(event) &&
      event.kind === "lifecycle" &&
      event.stage === "agent" &&
      event.message.startsWith(STARTED_PREFIX)
  );
  if (!started) {
    return { adapter: null, model: null, command: null };
  }
  const command = started.message.slice(STARTED_PREFIX.length).trim();
  const tokens = command.split(/\s+/).filter(Boolean);
  const adapter = tokens[0] ?? null;
  let model: string | null = null;
  for (let index = 0; index < tokens.length - 1; index += 1) {
    const token = tokens[index];
    const next = tokens[index + 1];
    if (token === "-m" || token === "--model") {
      model = next;
      break;
    }
    if (token === "-c" && next.startsWith("model=")) {
      model = next.slice("model=".length);
      break;
    }
  }
  return { adapter, model, command: command.length > 0 ? command : null };
}
