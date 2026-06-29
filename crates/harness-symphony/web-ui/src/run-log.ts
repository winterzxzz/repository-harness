export type RunLogEntry = {
  kind: "message" | "progress" | "raw";
  source: string;
  title: string;
  message: string;
  timestamp?: string;
  method?: string;
};

type DeltaBuffer = {
  itemId: string;
  text: string;
  timestamp?: string;
};

const lifecycleTitles: Record<string, string> = {
  "thread/started": "Conversation started",
  "turn/started": "Run started",
  "turn/diff/updated": "Workspace diff updated",
  "turn/completed": "Run finished",
  "thread/status/changed": "Thread status changed"
};

export function formatRunLog(events: unknown[]): RunLogEntry[] {
  const entries: RunLogEntry[] = [];
  let deltaBuffer: DeltaBuffer | null = null;

  function flushDelta() {
    if (!deltaBuffer) {
      return;
    }
    entries.push({
      kind: "message",
      source: "Assistant",
      title: "Assistant message",
      message: compactText(deltaBuffer.text) || "Assistant message received.",
      timestamp: deltaBuffer.timestamp,
      method: "item/agentMessage/delta"
    });
    deltaBuffer = null;
  }

  events.forEach((event, index) => {
    const method = getString(event, ["method"]);
    const params = getValue(event, ["params"]);
    const timestamp = timestampFrom(event);

    if (method === "item/agentMessage/delta") {
      const itemId = getString(params, ["itemId"]) ?? getString(params, ["item", "id"]) ?? "assistant";
      const text = textFrom(params) ?? "";
      if (!deltaBuffer || deltaBuffer.itemId !== itemId) {
        flushDelta();
        deltaBuffer = { itemId, text: "", timestamp };
      }
      deltaBuffer.text += text;
      return;
    }

    flushDelta();

    if (method && lifecycleTitles[method]) {
      entries.push({
        kind: "progress",
        source: "Codex",
        title: lifecycleTitles[method],
        message: lifecycleMessage(method, params),
        timestamp,
        method
      });
      return;
    }

    if (isUserMessage(method, params)) {
      entries.push({
        kind: "message",
        source: "User",
        title: "User message",
        message: textFrom(params) ?? "User input received.",
        timestamp,
        method
      });
      return;
    }

    const completedMessage = completedItemMessage(method, params, timestamp);
    if (completedMessage) {
      entries.push(completedMessage);
      return;
    }

    entries.push(rawEntry(event, method, index, timestamp));
  });

  flushDelta();
  return entries;
}

function lifecycleMessage(method: string, params: unknown): string {
  if (method === "turn/completed") {
    const status = getString(params, ["turn", "status"]) ?? "unknown";
    const error = getString(params, ["turn", "error", "message"]);
    return error ? `Turn ended with status ${status}: ${error}` : `Turn ended with status ${status}.`;
  }
  if (method === "turn/started") {
    return "Codex started working on this task.";
  }
  if (method === "turn/diff/updated") {
    return "Codex updated the working tree diff.";
  }
  if (method === "thread/status/changed") {
    const status = getString(params, ["status"]) ?? getString(params, ["thread", "status"]) ?? "updated";
    return `Conversation status changed to ${status}.`;
  }
  if (method === "thread/started") {
    const threadId = getString(params, ["thread", "id"]);
    return threadId ? `Conversation ${threadId} is ready.` : "Conversation is ready.";
  }
  return "Codex reported progress.";
}

function completedItemMessage(method: string | undefined, params: unknown, timestamp?: string): RunLogEntry | null {
  if (method !== "item/completed") {
    return null;
  }
  const itemType = getString(params, ["item", "type"]) ?? getString(params, ["type"]);
  const text = textFrom(getValue(params, ["item"])) ?? textFrom(params);
  if (itemType && itemType.toLowerCase().includes("agent") && text) {
    return {
      kind: "message",
      source: "Assistant",
      title: "Assistant message",
      message: compactText(text),
      timestamp,
      method
    };
  }
  return {
    kind: "progress",
    source: "Codex",
    title: "Item completed",
    message: itemType ? `${humanize(itemType)} completed.` : "Codex completed an item.",
    timestamp,
    method
  };
}

function rawEntry(event: unknown, method: string | undefined, index: number, timestamp?: string): RunLogEntry {
  const responseId = getString(event, ["id"]) ?? getNumber(event, ["id"])?.toString();
  const error = textFrom(getValue(event, ["error"]));
  const text = textFrom(getValue(event, ["params"])) ?? textFrom(getValue(event, ["result"]));
  const keys = objectKeys(event);
  return {
    kind: "raw",
    source: "Raw event",
    title: method ? humanize(method) : responseId ? `Response ${responseId}` : `Event ${index + 1}`,
    message: compactText(error ?? text ?? (keys.length > 0 ? `Unsupported event payload with keys: ${keys.join(", ")}.` : "Unsupported event payload.")),
    timestamp,
    method
  };
}

function isUserMessage(method: string | undefined, params: unknown): boolean {
  if (method?.toLowerCase().includes("usermessage")) {
    return true;
  }
  const itemType = getString(params, ["item", "type"]) ?? getString(params, ["type"]);
  return itemType?.toLowerCase().includes("user") ?? false;
}

function textFrom(value: unknown): string | null {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    const text = value.map(textFrom).filter(Boolean).join("");
    return text.length > 0 ? text : null;
  }
  if (!isRecord(value)) {
    return null;
  }
  for (const key of ["delta", "text", "message", "content", "output_text"]) {
    const text = textFrom(value[key]);
    if (text) {
      return text;
    }
  }
  return null;
}

function timestampFrom(value: unknown): string | undefined {
  const raw =
    getString(value, ["params", "timestamp"]) ??
    getString(value, ["params", "createdAt"]) ??
    getString(value, ["params", "turn", "startedAt"]) ??
    getString(value, ["params", "turn", "completedAt"]) ??
    getString(value, ["params", "item", "createdAt"]) ??
    getString(value, ["result", "turn", "startedAt"]) ??
    getString(value, ["result", "turn", "completedAt"]);
  if (raw) {
    return formatTimestamp(raw);
  }
  const numeric =
    getNumber(value, ["params", "timestamp"]) ??
    getNumber(value, ["params", "createdAt"]) ??
    getNumber(value, ["params", "turn", "startedAt"]) ??
    getNumber(value, ["params", "turn", "completedAt"]) ??
    getNumber(value, ["result", "turn", "startedAt"]) ??
    getNumber(value, ["result", "turn", "completedAt"]);
  return numeric === undefined ? undefined : formatTimestamp(numeric);
}

function formatTimestamp(value: string | number): string {
  const date = typeof value === "number" ? new Date(value < 10_000_000_000 ? value * 1000 : value) : new Date(value);
  return Number.isNaN(date.getTime()) ? String(value) : date.toLocaleString();
}

function compactText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function humanize(value: string): string {
  return value
    .replace(/[/_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function getString(value: unknown, path: string[]): string | undefined {
  const found = getValue(value, path);
  return typeof found === "string" ? found : undefined;
}

function getNumber(value: unknown, path: string[]): number | undefined {
  const found = getValue(value, path);
  return typeof found === "number" ? found : undefined;
}

function getValue(value: unknown, path: string[]): unknown {
  let current = value;
  for (const segment of path) {
    if (!isRecord(current)) {
      return undefined;
    }
    current = current[segment];
  }
  return current;
}

function objectKeys(value: unknown): string[] {
  return isRecord(value) ? Object.keys(value).slice(0, 4) : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
